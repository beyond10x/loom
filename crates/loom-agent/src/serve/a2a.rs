//! The A2A side of a served agent (`story:agent-a2a`): [`SpecPlugin`] answers each A2A message
//! with one governed turn: [`Answering`] is its [`b10x_loom_a2a::Answerer`].
//!
//! The turn is the host's turn on an item, as `run_plugin` runs it: the classifier decides the
//! intent, a confidence under the threshold ends it `unclassified`, and the state whose `when`
//! names the intent is the projection. Then it differs in two ways only, so that nothing is sent
//! through A2A:
//!
//! - the turn runs on `inbound-answer@1`, whose `proposed` outcome ends the turn on
//!   `reply.propose`: the proposal is the answer the A2A task returns;
//! - `reply.send` is taken out of the projection and the turn has no send policy, so the effect
//!   port has nothing to send with, whatever the authority grants.
//!
//! Each `RuntimeEvent` of the turn's run reaches the task as it happens. The case and its records
//! are kept in the state directory as every turn's are, under the commission of the item
//! `a2a:<revision>`, the revision `b10x_loom_a2a::item_revision` derives from the message id.
//!
//! The turn's line goes to the state directory's `record.jsonl`, as a polled item's does, under
//! the lock of the host that holds the directory ([`HeldRecord`]): the polling host of the same
//! process, the one `agent serve` holds for a spec that polls nothing, or, when neither holds it,
//! one [`Answering`] opens. Before a turn, `loom.agent.RecordA2aTurn` ([`record_a2a_turn`]) asks
//! whether the record has a line for the item already: then the message was answered, no turn
//! runs and the task ends with that line. A turn that could not run records nothing. A turn that
//! ran but whose line could not be appended is remembered (the last [`MAX_UNRECORDED`] items), and
//! a repeat of its message runs no second turn: its task fails with [`UNRECORDED`].
//!
//! A record that cannot be read or written fails the task with a fixed reason
//! ([`RECORD_UNREADABLE`], [`RECORD_UNWRITABLE`]) that names no path of the host; the full error
//! goes to standard error only. So does a turn that cannot run: its task fails with the fixed
//! reason of its error's kind ([`TURN_STATE`], [`TURN_CONFIG`], [`TURN_POLL`],
//! [`TURN_UNCLASSIFIED`], [`TURN_FAILED`], [`TURN_UNAVAILABLE`]).
//!
//! The listener needs a bearer: [`bearer`] reads it from [`BEARER_ENV`], which the agent's Secret
//! sets; serving refuses to start without it. A spec with `expose.a2a` and no `channel` is served
//! too, polling nothing ([`admit_serve`]).

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Mutex, OnceLock, PoisonError};

use b10x_loom_a2a::{A2aServer, Answerer, Bearer};

pub use b10x_loom_a2a::admit::{
    A2aRefusalReason, A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome,
};
pub use b10x_loom_a2a::{a2a_refusal_spelling, admit_a2a_request};
use b10x_loom_plugin::effects::HttpSources;
use b10x_loom_plugin::project::SEND;
use b10x_loom_plugin::state::{HeldRecord, StateDir};
use b10x_loom_plugin::turn::{TURN_MODEL_BUDGET, TURN_STEP_BUDGET, stopped};
use b10x_loom_plugin::{
    Classification, Host, InboundItem, ItemId, PROTOCOL, Plugin, PluginError, Projection,
    RecordLine, RecordOutcome, TurnOptions, TurnResult, turn_with,
};
use loom_sdk::EventSink;
use loom_sdk::agent::{A2aExposure, AgentSpec};
use loom_sdk::governor::{FileCaseStore, FileSuspensionStore};
use loom_sdk::intake::slice::run::printable;

use super::SpecPlugin;

pub use loom::agent::{
    AdmitServe, AdmitServeOutcome, RecordA2aTurn, RecordA2aTurnOutcome, ServeRefusalReason,
    ServeRefused,
};

/// Decides `loom.agent.RecordA2aTurn` (`ess/domains/agent.yaml`): a message whose item the record
/// has a line for already is `replayed`, answered with that line and no turn; one whose turn ran
/// in this process unrecorded is `unrecorded`, answered with a failure and no second turn; any
/// other is `turned`, its turn's line appended once.
pub fn record_a2a_turn(input: &RecordA2aTurn) -> RecordA2aTurnOutcome {
    if input.recorded {
        RecordA2aTurnOutcome::Replayed
    } else if input.ran_unrecorded {
        RecordA2aTurnOutcome::Unrecorded
    } else {
        RecordA2aTurnOutcome::Turned
    }
}

/// Why a task fails when the record cannot be read: no turn ran.
pub const RECORD_UNREADABLE: &str = "the agent's record cannot be read; no turn ran";

/// Why a task fails when its turn ran and its line could not be appended to the record.
pub const RECORD_UNWRITABLE: &str = "the turn ran and its record line could not be written";

/// Why a repeat of a message fails whose turn ran unrecorded in this process: no second turn runs.
pub const UNRECORDED: &str =
    "this message's turn ran and was not recorded; it is not run a second time";

/// Why a task fails when its turn could not use the agent's state directory.
pub const TURN_STATE: &str = "the agent's state cannot be used";

/// Why a task fails when its turn found the agent's configuration unusable.
pub const TURN_CONFIG: &str = "the agent's configuration cannot be used";

/// Why a task fails when a poll its turn needed failed.
pub const TURN_POLL: &str = "a poll the turn needs failed";

/// Why a task fails when its message could not be classified.
pub const TURN_UNCLASSIFIED: &str = "the message could not be classified";

/// Why a task fails when its turn failed.
pub const TURN_FAILED: &str = "the turn failed";

/// Why a task fails when something its turn needs is unavailable: a model, a source, a service.
pub const TURN_UNAVAILABLE: &str = "something the turn needs is unavailable";

/// The fixed reason a caller is given for a turn that failed with `error`: one per kind of
/// failure, so the kind stays visible while the text, which may name a path of the host, does
/// not reach the caller. The match is exhaustive: a new kind of [`PluginError`] needs its reason.
fn turn_failure(error: &PluginError) -> &'static str {
    match error {
        PluginError::StateInsideCheckout { .. }
        | PluginError::StateInUse { .. }
        | PluginError::State(_) => TURN_STATE,
        PluginError::Config(_) => TURN_CONFIG,
        PluginError::Poll(_) => TURN_POLL,
        PluginError::Classify(_) => TURN_UNCLASSIFIED,
        PluginError::Turn(_) => TURN_FAILED,
        PluginError::Unavailable(_) => TURN_UNAVAILABLE,
    }
}

/// The most items whose turn ran unrecorded an [`Answering`] remembers; past it the oldest is
/// forgotten, and a repeat of its message may run a turn again.
pub const MAX_UNRECORDED: usize = 1024;

/// The items whose turn ran and whose line could not be appended, oldest first.
#[derive(Debug, Default)]
struct Unrecorded(Mutex<VecDeque<ItemId>>);

impl Unrecorded {
    fn held(&self) -> std::sync::MutexGuard<'_, VecDeque<ItemId>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn contains(&self, item: &ItemId) -> bool {
        self.held().contains(item)
    }

    fn remember(&self, item: &ItemId) {
        let mut held = self.held();
        if held.contains(item) {
            return;
        }
        if held.len() == MAX_UNRECORDED {
            held.pop_front();
        }
        held.push_back(item.clone());
    }
}

/// The record an A2A turn is decided on and appended to.
trait TurnRecord {
    fn line_of(&self, item: &ItemId) -> Result<Option<RecordLine>, PluginError>;
    fn append(&self, line: &RecordLine) -> Result<(), PluginError>;
}

impl TurnRecord for HeldRecord {
    fn line_of(&self, item: &ItemId) -> Result<Option<RecordLine>, PluginError> {
        HeldRecord::line_of(self, item)
    }

    fn append(&self, line: &RecordLine) -> Result<(), PluginError> {
        HeldRecord::append(self, line)
    }
}

/// `reason` for the caller; `error`, which may name a path of the host, to standard error. The
/// error's text can hold model output, so each of its control characters but the tab (a newline,
/// a carriage return, the rest of C0, DEL and C1) is printed as a visible escape by the slice's
/// `printable`, and the line stays one line (`story:a2a-stderr-line-escapes-error-text`).
fn failed(reason: &str, error: &PluginError) -> String {
    let shown = printable(&error.to_string());
    eprintln!("b10x-loom: A2A: {reason}: {shown}");
    reason.to_owned()
}

/// Decides `item` on `record` (`loom.agent.RecordA2aTurn`) and, when it is `turned`, runs `turn`
/// and appends its line, remembering the item in `unrecorded` when the append fails.
fn answered(
    record: &dyn TurnRecord,
    unrecorded: &Unrecorded,
    item: &InboundItem,
    turn: impl FnOnce() -> Result<RecordLine, String>,
) -> Result<RecordLine, String> {
    let recorded = record
        .line_of(&item.id)
        .map_err(|error| failed(RECORD_UNREADABLE, &error))?;
    let decided = record_a2a_turn(&RecordA2aTurn {
        recorded: recorded.is_some(),
        ran_unrecorded: unrecorded.contains(&item.id),
    });
    match (decided, recorded) {
        (RecordA2aTurnOutcome::Replayed, Some(line)) => return Ok(line),
        (RecordA2aTurnOutcome::Unrecorded, _) => return Err(UNRECORDED.to_owned()),
        _ => {}
    }
    let line = turn()?;
    if let Err(error) = record.append(&line) {
        unrecorded.remember(&item.id);
        return Err(failed(RECORD_UNWRITABLE, &error));
    }
    Ok(line)
}

/// The environment variable holding the bearer every A2A request but a probe carries.
pub const BEARER_ENV: &str = "LOOM_A2A_BEARER";

/// Why [`SpecPlugin::new`] refuses a spec [`admit_serve`] answers `nothing-to-serve`.
pub const NOTHING_TO_SERVE: &str =
    "neither `channel` nor `expose.a2a` is declared; nothing would be polled or answered";

/// Decides `loom.agent.AdmitServe` (`ess/domains/agent.yaml`): a spec with neither a channel nor
/// `expose.a2a` is refused, then one with both polls and answers, one with a channel polls, and
/// one with `expose.a2a` alone answers A2A and polls nothing.
pub fn admit_serve(input: &AdmitServe) -> AdmitServeOutcome {
    match (input.channel, input.a2a) {
        (false, false) => AdmitServeOutcome::NothingToServe {
            error: ServeRefused {
                reason: ServeRefusalReason::NothingToServe,
            },
        },
        (true, true) => AdmitServeOutcome::PollsAndAnswers,
        (true, false) => AdmitServeOutcome::Polls,
        (false, true) => AdmitServeOutcome::Answers,
    }
}

/// The input of [`admit_serve`] for `spec`: whether it declares a `channel` and `expose.a2a`.
pub fn serve_input(spec: &AgentSpec) -> AdmitServe {
    AdmitServe {
        channel: spec.channel.is_some(),
        a2a: spec
            .expose
            .as_ref()
            .is_some_and(|expose| expose.a2a.is_some()),
    }
}

/// `reason` as the enum `loom.agent.ServeRefusalReason` spells it.
pub fn serve_refusal_spelling(reason: ServeRefusalReason) -> &'static str {
    match reason {
        ServeRefusalReason::NothingToServe => "NothingToServe",
    }
}

/// The bearer of the A2A listener, from `value`, the content of [`BEARER_ENV`].
///
/// # Errors
/// [`PluginError::Config`] when it is unset, empty or blank once the whitespace around it is
/// dropped (a Secret file's trailing newline), or holds a control character; the message names
/// the variable, never a value.
pub fn bearer(value: Option<&str>) -> Result<Bearer, PluginError> {
    value
        .ok_or_else(|| {
            PluginError::Config(format!(
                "`expose.a2a` is declared and {BEARER_ENV} is unset; the listener needs a bearer"
            ))
        })
        .and_then(|value| {
            Bearer::new(value).map_err(|why| {
                PluginError::Config(format!(
                    "`expose.a2a` is declared and {BEARER_ENV} cannot be used ({why}); the listener \
                     needs a bearer"
                ))
            })
        })
}

/// The A2A listener of `exposure`, admitting requests that carry `bearer`.
///
/// # Errors
/// [`PluginError::Config`] for an `expose.a2a` the listener cannot serve: a port outside
/// 1..=65535, or a path that is not absolute or is one the listener answers itself.
pub fn listener(exposure: &A2aExposure, bearer: Bearer) -> Result<A2aServer, PluginError> {
    port(exposure)?;
    A2aServer::new(exposure.clone(), bearer)
        .map_err(|why| PluginError::Config(format!("the agent spec: {why}")))
}

/// The port of `exposure`.
///
/// # Errors
/// [`PluginError::Config`] for a port outside 1..=65535.
pub fn port(exposure: &A2aExposure) -> Result<u16, PluginError> {
    u16::try_from(exposure.port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| {
            PluginError::Config(format!(
                "the agent spec: `expose.a2a.port` {} is no port from 1 to 65535",
                exposure.port
            ))
        })
}

/// The projection an A2A turn runs on: the state's `projection` without `reply.send`. The turn's
/// protocol, [`PROTOCOL`], declares no `reply.send` either; this keeps the A2A turn from sending
/// should the protocol ever change.
pub fn answer_projection(mut projection: Projection) -> Projection {
    projection.actions.retain(|action| action != SEND);
    projection
}

/// A served agent answering A2A messages: [`SpecPlugin`] under a host, its turns recorded in the
/// state directory's record (see the module documentation).
pub struct Answering<'a> {
    plugin: &'a SpecPlugin,
    host: Host<'a>,
    record: OnceLock<HeldRecord>,
    unrecorded: Unrecorded,
}

impl<'a> Answering<'a> {
    /// `plugin` under the host of its configuration, as `run_plugin` builds it.
    pub fn new(plugin: &'a SpecPlugin) -> Self {
        Self::with_host(plugin, Host::new(&plugin.config))
    }

    /// `plugin` under `host`.
    pub fn with_host(plugin: &'a SpecPlugin, host: Host<'a>) -> Self {
        Self {
            plugin,
            host,
            record: OnceLock::new(),
            unrecorded: Unrecorded::default(),
        }
    }

    /// Records every turn in `record`, the record of `plugin`'s state directory a host of this
    /// process holds.
    #[must_use]
    pub fn with_record(self, record: HeldRecord) -> Self {
        let _ = self.record.set(record);
        self
    }

    /// The record turns go to: the one given, else the one a host of this process holds for the
    /// state directory, else the directory opened and held here, until this is dropped.
    fn record(&self) -> Result<&HeldRecord, PluginError> {
        if let Some(record) = self.record.get() {
            return Ok(record);
        }
        let record = match HeldRecord::of(&self.plugin.state) {
            Some(record) => record,
            None => StateDir::open(&self.plugin.state, &self.plugin.config)?.held(),
        };
        Ok(self.record.get_or_init(|| record))
    }

    /// The governed turn on `item` (see the module documentation) and its record line. A turn
    /// that cannot run fails with the fixed reason of its error's kind ([`turn_failure`]); the
    /// error itself goes to standard error.
    fn turn(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        let error = |error: PluginError| failed(turn_failure(&error), &error);
        let (plugin, host) = (self.plugin, &self.host);
        let threshold = host.threshold().map_err(error)?;
        let classification = plugin.classify(host, item).map_err(error)?;
        let sure = classification
            .confidence
            .0
            .trim()
            .parse::<f64>()
            .ok()
            .is_some_and(|confidence| (0.0..=1.0).contains(&confidence) && confidence >= threshold);
        if !sure {
            let turn = TurnResult {
                outcome: RecordOutcome::Unclassified,
                reads: Vec::new(),
                proposal: None,
                proposed_case: None,
                detail: Some(format!(
                    "confidence {} is below the threshold {threshold}",
                    classification.confidence.0
                )),
                steps: Vec::new(),
            };
            return Ok(plugin.result(item, &classification, turn));
        }
        let projection = answer_projection(plugin.project(host, item, &classification));
        let turn = plugin
            .answer_turn(host, item, &classification, &projection, Some(events))
            .map_err(error)?;
        Ok(plugin.result(item, &classification, turn))
    }
}

impl Answerer for Answering<'_> {
    fn answer(&self, item: &InboundItem, events: &mut dyn EventSink) -> Result<RecordLine, String> {
        let record = self
            .record()
            .map_err(|error| failed(RECORD_UNREADABLE, &error))?;
        answered(record, &self.unrecorded, item, || self.turn(item, events))
    }
}

impl SpecPlugin {
    /// One turn of an A2A message (see the module documentation), each `RuntimeEvent` to
    /// `events`; also the turn of one run that cannot send (`crate::run`).
    pub(crate) fn answer_turn(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
        projection: &Projection,
        events: Option<&mut dyn EventSink>,
    ) -> Result<TurnResult, PluginError> {
        if projection.actions.is_empty() {
            return Ok(stopped(
                Vec::new(),
                "no state of the spec admits this intent",
            ));
        }
        let state = |error: &dyn fmt::Display| PluginError::State(error.to_string());
        let store = FileCaseStore::open(&self.state).map_err(|e| state(&e))?;
        let suspensions = FileSuspensionStore::open(&self.state).map_err(|e| state(&e))?;
        let inner = self.spec.loops.as_ref().map(|loops| &loops.inner);
        let step_budget = inner
            .and_then(|inner| usize::try_from(inner.step_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(TURN_STEP_BUDGET);
        let model_budget = inner
            .and_then(|inner| u64::try_from(inner.model_budget).ok())
            .filter(|budget| *budget > 0)
            .unwrap_or(TURN_MODEL_BUDGET);
        let turned = turn_with(
            host,
            self.turn_model()?,
            item,
            classification,
            projection,
            TurnOptions {
                protocol: PROTOCOL,
                authority: &self.authority,
                store,
                suspensions: Some(&suspensions),
                send: None,
                http: Some(HttpSources {
                    bindings: self.http.clone(),
                    reader: &self.reader,
                }),
                commission: Some(self.commission_of(item)),
                step_budget,
                model_budget,
                events: match events {
                    Some(sink) => Some(&mut *sink),
                    None => None,
                },
            },
        )?;
        Ok(turned.result)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// A record whose appends fail, naming a path of the host as a real failure does.
    struct Unwritable;

    impl TurnRecord for Unwritable {
        fn line_of(&self, _item: &ItemId) -> Result<Option<RecordLine>, PluginError> {
            Ok(None)
        }

        fn append(&self, _line: &RecordLine) -> Result<(), PluginError> {
            Err(PluginError::State(
                "~/agents/state/record.jsonl cannot be appended to: no space left".to_owned(),
            ))
        }
    }

    fn item(id: &str) -> InboundItem {
        InboundItem {
            id: ItemId(id.to_owned()),
            revision: id.trim_start_matches("a2a:").to_owned(),
            text: "Is the deploy finished?".to_owned(),
            details: loom::json::Value::Null,
        }
    }

    fn line(item: &InboundItem) -> RecordLine {
        RecordLine {
            item: item.id.clone(),
            intent: None,
            confidence: None,
            outcome: RecordOutcome::Proposed,
            reads: Vec::new(),
            proposal: Some("finished".to_owned()),
            proposed_case: None,
            detail: None,
            steps: Vec::new(),
        }
    }

    /// Security review F3: a turn that ran and whose line could not be appended is not run a
    /// second time for a repeat of its message; the repeat fails with a fixed reason, and neither
    /// answer names the path the append error names.
    #[test]
    fn a_repeat_of_a_turn_that_ran_unrecorded_runs_no_second_turn() {
        let unrecorded = Unrecorded::default();
        let turns = Cell::new(0);
        let first = item("a2a:m-1");
        let turn = |item: &InboundItem| -> Result<RecordLine, String> {
            turns.set(turns.get() + 1);
            Ok(line(item))
        };

        let ran = answered(&Unwritable, &unrecorded, &first, || turn(&first));
        assert_eq!(ran, Err(RECORD_UNWRITABLE.to_owned()));
        let again = answered(&Unwritable, &unrecorded, &first, || turn(&first));
        assert_eq!(again, Err(UNRECORDED.to_owned()));
        assert_eq!(turns.get(), 1, "one governed turn for one messageId");
        for reason in [RECORD_UNWRITABLE, UNRECORDED, RECORD_UNREADABLE] {
            assert!(!reason.contains('/'), "{reason}");
        }

        let other = item("a2a:m-2");
        let _ = answered(&Unwritable, &unrecorded, &other, || turn(&other));
        assert_eq!(turns.get(), 2, "another messageId runs its own turn");
    }

    /// Every kind of [`PluginError`] a turn can fail with, each naming a path of the host, gets a
    /// fixed reason naming no path, one per kind (the three state kinds share one).
    #[test]
    fn every_turn_failure_gives_a_fixed_reason_naming_no_path() {
        let path = "/srv/agents/state/cases";
        let failures = [
            (
                PluginError::StateInsideCheckout {
                    state: path.to_owned(),
                    inside: "/srv/agents".to_owned(),
                },
                TURN_STATE,
            ),
            (
                PluginError::StateInUse {
                    state: path.to_owned(),
                },
                TURN_STATE,
            ),
            (PluginError::State(format!("{path}: denied")), TURN_STATE),
            (PluginError::Config(format!("{path}: denied")), TURN_CONFIG),
            (PluginError::Poll(format!("{path}: denied")), TURN_POLL),
            (
                PluginError::Classify(format!("{path}: denied")),
                TURN_UNCLASSIFIED,
            ),
            (PluginError::Turn(format!("{path}: denied")), TURN_FAILED),
            (
                PluginError::Unavailable(format!("~/agents/routes.yaml and {path}")),
                TURN_UNAVAILABLE,
            ),
        ];
        for (error, reason) in &failures {
            assert!(error.to_string().contains('/'), "{error}");
            assert_eq!(turn_failure(error), *reason, "{error}");
            assert!(!reason.contains('/') && !reason.contains('~'), "{reason}");
        }
        let mut reasons: Vec<&str> = failures.iter().map(|(_, reason)| *reason).collect();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), 6, "one reason per kind: {reasons:?}");
    }

    /// The items remembered are bounded: past [`MAX_UNRECORDED`] the oldest is forgotten.
    #[test]
    fn the_items_run_unrecorded_are_bounded() {
        let unrecorded = Unrecorded::default();
        for index in 0..=MAX_UNRECORDED {
            unrecorded.remember(&ItemId(format!("a2a:m-{index}")));
        }
        assert!(!unrecorded.contains(&ItemId("a2a:m-0".to_owned())));
        assert!(unrecorded.contains(&ItemId(format!("a2a:m-{MAX_UNRECORDED}"))));
        assert_eq!(unrecorded.held().len(), MAX_UNRECORDED);
    }
}
