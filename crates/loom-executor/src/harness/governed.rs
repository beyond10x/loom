// SPDX-License-Identifier: Apache-2.0

//! The ported loop, run over one frontier of a commission, through Loom's own implementations of
//! its seams (`docs/design/harness-map.md` § Seams a port reuses).
//!
//! Loom's own code, not ported: it names Loom's projection, selection and revalidation, which the
//! five ported modules may not (`tests/adversary2_harness_port.rs`), and it reaches the loop only
//! through the seams the loop already has. One ported type changed for it:
//! [`LoopEvent::Compacted`](crate::harness::turn_loop::LoopEvent::Compacted) carries the usage
//! the provider reported for the summary request.
//!
//! - **The tool list** ([`TurnEnvironmentProvider`]). Before every turn the case's current frontier
//!   is read, from the governor ([`Loom::with_governor`]) or, for a Loom without one, the frontier
//!   the run was handed. It is projected ([`crate::projection`]) and kept as the turn's catalogue
//!   ([`Loom::catalogues`]), and each catalogue entry is
//!   published as one tool under [`tool_name`], in the catalogue's order and with nothing else
//!   beside it: the loop's own tools (an answer schema, delegation, skills, memories) are cleared
//!   from the run's configuration, so no delegate runs, and a narrowing in it only removes entries.
//!   A call naming anything else is refused to the model by the loop's own narrowing, naming it,
//!   and never reaches the selector.
//! - **A tool call** ([`ApprovalPort`]). Every published tool asks before it runs, and Loom is what
//!   answers. The model's call is the selection, by the reasoning model, from that turn's
//!   catalogue; its arguments are the call's; and the pipeline [`AgentExecutor::run`] runs with a
//!   Loom's own selector and generator records the selection and the argument request and
//!   revalidates the selection against the governor's current frontier. An admitted selection
//!   stops the loop at an approval checkpoint immediately before the effect, and Loom returns it as
//!   Commission's `ProposedAction`. A selection the pipeline refuses is denied to the model, which
//!   chooses again on the next turn's catalogue. Loom never runs a frontier action:
//!   [`ToolPort::call`] refuses one, should a call ever reach it. The checkpoint is held for the
//!   run's session, so [`Loom::resume_loop`] can continue from it ([`crate::recovery`]).
//! - **A completed turn** ([`ModelPort`]). Each conversation turn the provider completes is recorded
//!   once into the run's session with the provider items it added, verbatim
//!   (`loom.run.RecordTurn`, [`Loom::turns`]). A turn the endpoint broke off is not recorded. One
//!   run holds a session at a time, and files it when it ends ([`Loom::sessions`]), unless the run
//!   was interrupted ([`Loom::interrupt`]): its session stays `Interrupted`, and a call the
//!   pipeline was selecting when the cancel came is stopped before revalidation and not proposed.
//! - **A compaction** ([`LoopSink`]). The caller's sink receives every event, and each compaction
//!   the loop reports is first recorded on the run's session with the usage the endpoint reported
//!   for its summary request (`loom.run.RecordCompaction`, [`Loom::compactions`],
//!   [`crate::compaction`]). The summary turn is not a turn of the session, and the turn after a
//!   compaction is offered the catalogue of the frontier current then, like any other.
//!
//! A run of [`Loom::run_loop`] starts a conversation of its own and drops any checkpoint held for
//! its session; only [`Loom::resume_loop`] continues one. Not wired yet: budgets as a Commission
//! suspension (a budget that binds is `NoUsefulAction`).

use std::cell::{Cell, OnceCell};
use std::ops::Deref;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::responsibility::{
    Admission, CaseId, Commission, ExecutorOutcome, Frontier, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use serde_json::json as wire_json;

use crate::arguments::{ArgumentContext, ArgumentGenerator};
use crate::compaction::CompactionRecorder;
use crate::harness::turn_loop::{
    AgentLoop, ApprovalDecision, ApprovalPort, ContextPackage, EnvironmentError, LoopCancel,
    LoopConfig, LoopError, LoopOutcome, LoopSink, LoopStop, LoopStopAwaitingApproval, NullLoopSink,
    TurnEnvironment, TurnEnvironmentProvider, TurnEnvironmentRequest,
};
use crate::harness::wire::{
    Approval, Effect, Envelope, Idempotency, InvalidId, Item, ModelPort, Risk, StreamSink,
    ToolCall, ToolName, ToolOutcome, ToolPort, ToolSpec, TurnOutcome, TurnRequest, WireError,
    WireId,
};
use crate::model::behaviour::{SelectionStorage, SessionStorage};
use crate::model::run::obligations::{
    FileSessionBehavior, OpenSessionBehavior, RecordTurnBehavior, ResumeSessionBehavior,
};
use crate::model::run::{
    ActionCatalogue, AnyActionCatalogue, ArgumentRequestId, CatalogueEntry, CatalogueEntryStatus,
    CatalogueId, FileSession, OpenSession, OpenSessionOutcome, RecordTurn, ResumeSession,
    ResumeSessionOutcome, RevalidateSelectionOutcome, RunEnding, SelectionId, SelectionStrategy,
    SessionData, SessionId, SessionSnapshot, SessionState, TurnId, action_catalogue_state,
};
use crate::recovery::{Held, Pending};
use crate::selection::{ActionSelector, Choice, SelectionContext, SelectorError};
use crate::{
    Loom, admits_nothing, case_moved, has_deciding_entry, loom_id, no_useful_action, outage,
    projection,
};

/// The name a frontier action is published under: its id with every character outside
/// `[A-Za-z0-9_-]` replaced by `_`, the class both ported wires publish a tool name in
/// (`responses::project::TOOL_NAME_PATTERN`, `messages::project::TOOL_NAME_PATTERN`).
/// `repository.merge` is published as `repository_merge`.
///
/// # Errors
///
/// [`InvalidId`] for an action whose id is empty or longer than a tool name may be.
pub fn tool_name(action: &str) -> Result<ToolName, InvalidId> {
    ToolName::new(
        action
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>(),
    )
}

/// What one run of [`Loom::run_loop`] is given besides the commission, its frontier and the
/// session.
pub struct LoopPorts<'p> {
    /// The model the loop turns.
    pub model: &'p mut dyn ModelPort,
    /// The run's model name, standing instructions, budget and sampling. Its tools are the
    /// catalogue's and never more, whatever it says: every field that would publish one of the
    /// loop's own tools (an answer schema, delegation, skills, memories) is cleared before the loop
    /// starts, and a narrowing (`admits`) publishes, each turn, only the catalogue entries whose
    /// tool names it lists, so a turn whose catalogue lists none of them publishes no tool.
    pub config: LoopConfig,
    /// Where the loop's events go, as they happen.
    pub sink: &'p mut dyn LoopSink,
}

/// One run of [`Loom::run_loop`] or [`Loom::resume_loop`]: what Loom returns to Commission, and
/// what the loop answered.
#[derive(Debug)]
pub struct LoopRun {
    /// The executor outcome.
    pub outcome: ExecutorOutcome,
    /// What the loop answered, or why it failed; [`None`] for a run refused before the loop was
    /// built.
    pub run: Option<Result<LoopOutcome, LoopError>>,
}

/// Where a governed run's conversation starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Start {
    /// A conversation of its own, on the Loom's prompt; a checkpoint held for the session is
    /// dropped.
    Fresh,
    /// The checkpoint held for the session, or a conversation of its own when none is held.
    Checkpoint,
}

impl<S, G, V> Loom<S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// Runs the ported loop on this Loom's prompt over `frontier`, for `commission`, recording its
    /// turns into `session`, and answers what Loom proposes (see the module docs).
    ///
    /// The model's call of a catalogue action is the selection, by the reasoning model, and its
    /// arguments are the call's: this Loom's own selector and generator are not asked.
    ///
    /// One run holds `session` at a time: it is opened, or resumed when this Loom holds it
    /// `Filed` or `Interrupted`, before the loop starts, and filed when the run ends, however it
    /// ends, unless the run was interrupted ([`Loom::interrupt`]). The run starts a conversation of
    /// its own and drops any checkpoint held for the session ([`Loom::resume_loop`] continues one).
    /// The outcome is:
    ///
    /// - `ProposedAction`, when the loop stopped at the checkpoint of a selection the pipeline
    ///   admitted;
    /// - `Suspended` with `ExternalAvailability`, when the pipeline suspended at a call, when the
    ///   loop failed (a wire that could not answer, a governor that could not, a run that could not
    ///   be described), or when the session is recorded on another wire or held with other data
    ///   (no request is sent);
    /// - `CompletedLocalReasoning`, when the model answered without a call that was proposed;
    /// - `NoUsefulAction`, for a frontier of another case or one that admits nothing, and for a
    ///   session another run holds `Active` (no model is asked); when the model's arguments cannot
    ///   be carried as Commission's JSON (the run ends there); for a run that was interrupted,
    ///   however it stopped; and for any other stop.
    pub fn run_loop(
        &self,
        session: &SessionData,
        ports: LoopPorts<'_>,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> LoopRun {
        self.governed_run(session, ports, commission, frontier, Start::Fresh)
    }

    /// One governed run on `session`, as [`Loom::run_loop`] documents it, starting where `start`
    /// says.
    pub(crate) fn governed_run(
        &self,
        session: &SessionData,
        ports: LoopPorts<'_>,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
        start: Start,
    ) -> LoopRun {
        self.governed(session, ports, commission, frontier, start).0
    }

    /// [`Loom::governed_run`], and whether the case moved off `frontier` in this run: a turn's
    /// catalogue was projected at another case revision than `frontier`'s, or the pipeline refused
    /// a selection `stale-revision` (`CaseMoved`, denied to the model).
    fn governed(
        &self,
        session: &SessionData,
        ports: LoopPorts<'_>,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
        start: Start,
    ) -> (LoopRun, bool) {
        let LoopPorts {
            model,
            config,
            sink,
        } = ports;
        // Every request carries the catalogue and nothing else, so the loop's own tools are not
        // published, whatever the caller asked: these four fields are all that adds one
        // (`AgentLoop::owned_specs`). A delegate in particular could not hold the checkpoint a
        // proposal stops at, so an action it selected would be admitted and never proposed.
        // A narrowing is applied to each turn's catalogue here, as an intersection: the loop's
        // own rule for it was written for a port whose names do not change, and admits every
        // published name once the grant names one the port no longer publishes.
        let admits = config.admits.clone();
        let config = config
            .with_output_schema(None)
            .with_delegation(None)
            .with_skills(None)
            .with_memories(None)
            .with_admitted(None);
        let case = &commission.data().case_id;
        if frontier.data().case_id != *case || admits_nothing(frontier) {
            return (unbuilt(no_useful_action()), false);
        }
        let holder = match self.claim_session(session, model.wire()) {
            Ok(holder) => holder,
            Err(refused) => return (unbuilt(refused), false),
        };
        let id = &session.session_id;
        let taken = self.started(id, start);
        if let Some(changed) = taken
            .as_ref()
            .and_then(|held| held.refuses(case, admits.as_deref()))
        {
            return (self.refused_resume(id, &holder, taken, changed), false);
        }
        let (checkpoint, resuming) = taken
            .clone()
            .map(|held| {
                let call = held.checkpoint.call().clone();
                (held.checkpoint, (call, held.pending))
            })
            .unzip();

        let offers = Offers::default();
        let mut environment = CurrentCatalogue {
            loom: self,
            case,
            handed: frontier,
            session: id,
            admits: admits.as_deref(),
            offers: &offers,
        };
        let mut tools = CatalogueTools { offers: &offers };
        let mut approvals = Proposer {
            loom: self,
            case,
            offers: &offers,
            cancel: holder.cancel.clone(),
            resuming,
            ended: None,
            stale_refused: false,
        };
        let mut model = Recording {
            model,
            loom: self,
            session: id,
            offers: &offers,
            run: holder.run,
            asked: false,
        };
        let mut sink = CompactionRecorder {
            sink,
            loom: self,
            session: id,
        };
        // The session is filed however the run ends, a panic included, unless the run was
        // interrupted, so a later run resumes it.
        let run = match catch_unwind(AssertUnwindSafe(|| {
            let mut run = AgentLoop::new(&mut model, &mut tools, &mut approvals, config)
                .with_environment(&mut environment)
                .with_cancel(holder.cancel.clone());
            match checkpoint {
                Some(checkpoint) => run.resume_asking(checkpoint, &mut sink),
                None => run.run(self.prompt.clone(), &mut sink),
            }
        })) {
            Ok(run) => run,
            Err(panic) => {
                let kept = taken.filter(|_| unanswered(&approvals, &model));
                self.end_session(id, &holder, RunEnding::Failed, kept);
                resume_unwind(panic);
            }
        };
        // A resumed run that ended before its held call was answered (it was never put to the
        // pipeline, and the model was not asked again) leaves the checkpoint as it found it.
        let kept = taken.filter(|_| unanswered(&approvals, &model));
        let stopped_at = match &run {
            Ok(answered) => stopped(answered, approvals.ended.take(), (id, case, admits)),
            Err(error) => (outage(error.to_string()), None),
        };
        let handed = frontier.data().case_revision;
        let moved = approvals.stale_refused
            || offers
                .all()
                .any(|offered| offered.frontier.data().case_revision != handed);
        (self.ended_run(id, &holder, run, stopped_at, kept), moved)
    }

    /// What a run that ended with `run` answers: `outcome`, unless the run was interrupted. The
    /// session is ended holding `held` when the run stopped at a call to resume from, else `kept`.
    fn ended_run(
        &self,
        session: &SessionId,
        holder: &Holder,
        run: Result<LoopOutcome, LoopError>,
        (outcome, held): (ExecutorOutcome, Option<Held>),
        kept: Option<Held>,
    ) -> LoopRun {
        let ending = match &run {
            Ok(answered) if answered.stop == LoopStop::Completed => RunEnding::Answered,
            Ok(_) => RunEnding::Stopped,
            Err(_) => RunEnding::Failed,
        };
        // An interrupted run proposes nothing, whatever it stopped at: nothing it had in flight
        // has left Loom, and its checkpoint is held for the run that resumes the session.
        let interrupted = self.end_session(session, holder, ending, held.or(kept));
        LoopRun {
            outcome: if interrupted {
                no_useful_action()
            } else {
                outcome
            },
            run: Some(run),
        }
    }

    /// A resume refused before its loop is built, because `changed` (`Held::refuses`): a changed
    /// configuration. The session is filed as failed and `held` is held again.
    fn refused_resume(
        &self,
        session: &SessionId,
        holder: &Holder,
        held: Option<Held>,
        changed: &str,
    ) -> LoopRun {
        let error = LoopError::Config(format!(
            "the run configuration changed after the approval checkpoint: {changed}"
        ));
        let outcome = outage(error.to_string());
        self.ended_run(session, holder, Err(error), (outcome, None), held)
    }

    /// The case's current frontier: the governor's, or `handed` for a Loom without one. A governor
    /// that cannot answer, or answers with another case's frontier, is named.
    fn current_frontier(
        &self,
        case: &CaseId,
        handed: &Frontier<frontier_state::Issued>,
    ) -> Result<Frontier<frontier_state::Issued>, String> {
        let Some(governor) = &self.governor else {
            return Ok(Frontier::new(handed.data().clone()));
        };
        let governor: &V::Target = governor;
        let current = governor.frontier(case).map_err(|error| {
            format!(
                "the governor issued no current frontier for case `{}`: {error:?}",
                case.0
            )
        })?;
        if current.data().case_id != *case {
            return Err(format!(
                "the governor issued a frontier for case `{}` as the current frontier of case `{}`",
                current.data().case_id.0,
                case.0
            ));
        }
        Ok(current)
    }
}

impl<S, G, V> Loom<S, G, V> {
    /// Every session [`Loom::run_loop`] opened, in the state it is in: `Active` while a run holds
    /// it, `Filed` once that run ended, `Interrupted` once [`Loom::interrupt`] cancelled it.
    pub fn sessions(&self) -> Vec<SessionSnapshot> {
        self.turn_record().sessions().to_vec()
    }

    /// Takes `session` for a run on `wire`: opens it (`loom.run.OpenSession`), or resumes it when
    /// this Loom holds it `Filed` or `Interrupted` with the same data (`loom.run.ResumeSession`). A
    /// session another run holds `Active` is `session-exists`, and the run proposes nothing
    /// (`NoUsefulAction`); one on another wire, or held with other data, is refused as an outage.
    /// Answers the run that took it, noted as the session's holder under the same lock
    /// [`Loom::interrupt`] takes, so an interrupt reaches the run from the moment the session is
    /// `Active`.
    fn claim_session(
        &self,
        session: &SessionData,
        wire: &WireId,
    ) -> Result<Holder, ExecutorOutcome> {
        let id = &session.session_id.0.0;
        if session.wire != wire.as_str() {
            return Err(outage(format!(
                "session `{id}` is recorded on wire `{}` and the run's model is on `{wire}`",
                session.wire
            )));
        }
        let mut recovery = self.recovery();
        let mut record = self.turn_record();
        match record.open_session(OpenSession {
            session_id: session.session_id.clone(),
            commission_run: session.commission_run.clone(),
            wire: session.wire.clone(),
        }) {
            Ok(OpenSessionOutcome::Opened { .. }) => {}
            Ok(OpenSessionOutcome::SessionExists { .. }) => {
                match SessionStorage::get(&*record, &session.session_id) {
                    Some(held) if held.state == SessionState::Active => {
                        return Err(no_useful_action());
                    }
                    Some(held) if held.data == *session => {
                        match record.resume_session(ResumeSession {
                            session_id: session.session_id.clone(),
                            wire: session.wire.clone(),
                        }) {
                            Ok(ResumeSessionOutcome::Resumed { .. }) => {}
                            resumed => {
                                return Err(outage(format!(
                                    "session `{id}` could not be resumed: {resumed:?}"
                                )));
                            }
                        }
                    }
                    _ => return Err(outage(format!("session `{id}` is held with other data"))),
                }
            }
            Err(unmet) => {
                return Err(outage(format!(
                    "session `{id}` could not be opened: {unmet:?}"
                )));
            }
        }
        drop(record);
        let holder = Holder {
            run: self.next_number(),
            cancel: LoopCancel::new(),
        };
        recovery.start(
            session.session_id.clone(),
            holder.run,
            holder.cancel.clone(),
        );
        Ok(holder)
    }

    /// Where the run that just took `session` starts: the checkpoint held for it, taken
    /// (`Start::Checkpoint` with one held); or a conversation of its own, any checkpoint held for
    /// it dropped.
    fn started(&self, session: &SessionId, start: Start) -> Option<Held> {
        let mut recovery = self.recovery();
        match start {
            Start::Fresh => {
                recovery.release(session);
                None
            }
            Start::Checkpoint => recovery.take(session),
        }
    }

    /// Ends the run `holder`, which ended as `ending`, and answers whether it was interrupted: its
    /// own cancel says, never the session's state, which a later run may have changed. Only while
    /// the run still holds `session` does it hold `held` for the session, or nothing, and file the
    /// session (`loom.run.FileSession`) unless it was interrupted; a run whose session another run
    /// resumed changes nothing. The holder took the session `Active` and only an interrupt, which
    /// cancels it, moves it, so it is `filed` otherwise.
    fn end_session(
        &self,
        session: &SessionId,
        holder: &Holder,
        ending: RunEnding,
        held: Option<Held>,
    ) -> bool {
        // Read under the lock `Loom::interrupt` cancels under, so an interrupt is wholly before or
        // wholly after this ending.
        let mut recovery = self.recovery();
        let interrupted = holder.cancel.is_cancelled();
        if !recovery.stop(session, holder.run) {
            return interrupted;
        }
        match held {
            Some(held) => recovery.hold(held),
            None => recovery.release(session),
        }
        if !interrupted {
            let _ = self.turn_record().file_session(FileSession {
                session_id: session.clone(),
                ending,
            });
        }
        interrupted
    }
}

/// The run that took a session: its number, which says whether it still holds the session, and the
/// cancel [`Loom::interrupt`] raises, which says whether it was interrupted.
struct Holder {
    run: u64,
    cancel: LoopCancel,
}

/// A run refused before its loop was built, answering `outcome`.
fn unbuilt(outcome: ExecutorOutcome) -> LoopRun {
    LoopRun { outcome, run: None }
}

/// Whether a resumed run has not answered the call it resumed from: the call was never put to the
/// pipeline, and the model was not asked again, which the loop does only once every call of the
/// checkpoint's turn has its answer. False for a run that resumed from no checkpoint.
fn unanswered<S, G, V>(
    approvals: &Proposer<'_, S, G, V>,
    model: &Recording<'_, '_, S, G, V>,
) -> bool {
    approvals.resuming.is_some() && !model.asked
}

/// What the loop's stop makes of the run, and the checkpoint held for `session` after it, with the
/// run's `case` and narrowing `admits`. Only a checkpoint the pipeline deferred at is its outcome,
/// and is held when it holds a call a later run can resume; nothing else the loop stops with
/// proposes anything.
fn stopped(
    answered: &LoopOutcome,
    deferred: Option<Ended>,
    (session, case, admits): (&SessionId, &CaseId, Option<Vec<ToolName>>),
) -> (ExecutorOutcome, Option<Held>) {
    match (&answered.stop, deferred) {
        (LoopStop::AwaitingApproval(LoopStopAwaitingApproval { checkpoint_id }), Some(ended))
            if *checkpoint_id == ended.checkpoint =>
        {
            let held =
                ended
                    .pending
                    .zip(answered.checkpoint.clone())
                    .map(|(pending, checkpoint)| Held {
                        session: session.clone(),
                        checkpoint,
                        pending,
                        case: case.clone(),
                        admits,
                    });
            (ended.outcome, held)
        }
        (LoopStop::Completed, _) => (ExecutorOutcome::CompletedLocalReasoning(Unit(true)), None),
        _ => (no_useful_action(), None),
    }
}

/// One turn's catalogue, as the loop was offered it.
struct Offered {
    /// The frontier the catalogue was projected from.
    frontier: Frontier<frontier_state::Issued>,
    catalogue: ActionCatalogue<action_catalogue_state::Projected>,
    /// One tool per catalogue entry the run may see, in the catalogue's order.
    specs: Vec<ToolSpec>,
    /// The turn the catalogue was projected for, and its index in the session.
    turn: TurnId,
    index: i64,
    /// Whether the turn was recorded: a turn is recorded once.
    recorded: Cell<bool>,
    next: OnceCell<Box<Offered>>,
}

impl Offered {
    /// The entry published as `name` this turn.
    fn entry(&self, name: &ToolName) -> Option<&CatalogueEntry> {
        if !self.specs.iter().any(|spec| &spec.name == name) {
            return None;
        }
        self.catalogue
            .data()
            .entries
            .iter()
            .find(|entry| tool_name(&entry.action).is_ok_and(|published| &published == name))
    }
}

/// The catalogues offered in one run, one per turn, in order.
///
/// Append-only, so the tool list handed out for a turn stays borrowed while the next turn's is
/// added: the tool port answers [`ToolPort::specs`] with a borrow, and the environment provider
/// adds a turn through a shared reference. The three seams of one run share it; the loop holds
/// them apart.
#[derive(Default)]
struct Offers {
    first: OnceCell<Box<Offered>>,
}

impl Offers {
    /// Every catalogue offered in this run, in order.
    fn all(&self) -> impl Iterator<Item = &Offered> {
        std::iter::successors(self.first.get().map(Box::as_ref), |at| {
            at.next.get().map(Box::as_ref)
        })
    }

    /// The catalogue offered for the latest turn.
    fn latest(&self) -> Option<&Offered> {
        self.all().last()
    }

    /// Offers the next turn's catalogue.
    fn offer(&self, offered: Offered) {
        let slot = self.latest().map_or(&self.first, |last| &last.next);
        // The latest offer has no successor, so its slot is empty and the offer is kept.
        let _ = slot.set(Box::new(offered));
    }
}

/// The [`TurnEnvironmentProvider`]: the catalogue projected from the case's current frontier.
struct CurrentCatalogue<'r, S, G, V> {
    loom: &'r Loom<S, G, V>,
    case: &'r CaseId,
    handed: &'r Frontier<frontier_state::Issued>,
    session: &'r SessionId,
    /// The caller's narrowing: when there is one, only the entries it names are published.
    admits: Option<&'r [ToolName]>,
    offers: &'r Offers,
}

impl<S, G, V> TurnEnvironmentProvider for CurrentCatalogue<'_, S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// The current frontier, read once, projected for the session's next turn. The context and
    /// inventory revisions are the frontier's id, so a frontier issued again under its id must
    /// project the same catalogue.
    fn refresh(
        &mut self,
        _request: TurnEnvironmentRequest,
    ) -> Result<TurnEnvironment, EnvironmentError> {
        let frontier = self
            .loom
            .current_frontier(self.case, self.handed)
            .map_err(EnvironmentError::Unavailable)?;
        let held = self.loom.turn_record().turns_of(self.session);
        let index = u64::try_from(held).unwrap_or(u64::MAX).saturating_add(1);
        let session = &self.session.0.0;
        let instance = &self.loom.instance;
        let turn = TurnId(loom_id(instance, "turn", session, index));
        let catalogue = projection::project(
            &frontier,
            CatalogueId(loom_id(
                instance,
                "catalogue",
                session,
                self.loom.next_number(),
            )),
            turn.clone(),
        );
        let mut specs = published(&catalogue.data().entries)?;
        if let Some(admits) = self.admits {
            specs.retain(|spec| admits.contains(&spec.name));
        }
        let revision = frontier.data().frontier_id.0.0.clone();
        self.loom.turn_record().offered(
            AnyActionCatalogue::Projected(ActionCatalogue::new(catalogue.data().clone()))
                .snapshot(),
        );
        self.offers.offer(Offered {
            frontier,
            catalogue,
            specs: specs.clone(),
            turn,
            index: i64::try_from(index).unwrap_or(i64::MAX),
            recorded: Cell::new(false),
            next: OnceCell::new(),
        });
        Ok(TurnEnvironment {
            context: ContextPackage::default(),
            tools: specs,
            context_revision: revision.clone(),
            inventory_revision: revision,
        })
    }
}

/// One tool per entry of `entries`, in order. Two actions published under one name are refused,
/// naming both: neither could be told apart from the other.
fn published(entries: &[CatalogueEntry]) -> Result<Vec<ToolSpec>, EnvironmentError> {
    let mut specs: Vec<ToolSpec> = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = tool_name(&entry.action).map_err(|error| {
            EnvironmentError::Invalid(format!(
                "the frontier action `{}` cannot be published as a tool: {error}",
                entry.action
            ))
        })?;
        if let Some((earlier, _)) = entries
            .iter()
            .zip(&specs)
            .find(|(_, spec)| spec.name == name)
        {
            return Err(EnvironmentError::Invalid(format!(
                "the frontier actions `{}` and `{}` are both published as `{name}`",
                earlier.action, entry.action
            )));
        }
        specs.push(spec(entry, name));
    }
    Ok(specs)
}

/// The tool a catalogue entry is published as.
///
/// Loom does not know what a frontier action does (an action's argument schema is undecided), so
/// each one takes any object and is described as the most it could be: a write that is costly to
/// get wrong and not safe to repeat. Every one asks before it runs, whatever the run's unattended
/// ceiling, because the answer is Loom's proposal and Commission's decision.
fn spec(entry: &CatalogueEntry, name: ToolName) -> ToolSpec {
    let status = match entry.status {
        CatalogueEntryStatus::Admissible => "admissible",
        CatalogueEntryStatus::ApprovalRequired => "admissible once authorized",
    };
    ToolSpec {
        name,
        description: format!(
            "The case action `{}`, {status} on the current frontier. Calling it proposes it: \
             Commission rechecks the proposal and runs it, and nothing runs inside this call.",
            entry.action
        ),
        input_schema: wire_json!({"type": "object"}),
        approval: Approval::Required,
        envelope: Envelope {
            effects: vec![Effect::Write],
            risk: Risk::High,
            idempotency: Idempotency::NonIdempotent,
            access: Vec::new(),
        },
    }
}

/// The [`ToolPort`]: the latest turn's catalogue, and no tool that runs.
struct CatalogueTools<'r> {
    offers: &'r Offers,
}

impl ToolPort for CatalogueTools<'_> {
    fn specs(&self) -> &[ToolSpec] {
        self.offers
            .latest()
            .map_or(&[], |offered| offered.specs.as_slice())
    }

    /// Never runs anything: every published tool asks first, and Loom's answer ends the run at the
    /// call or denies it. A call that reached the port anyway is told it did not happen.
    fn call(&mut self, call: &ToolCall) -> ToolOutcome {
        ToolOutcome::failed(format!(
            "`{}` is a case action: Loom proposes it to Commission and never runs it",
            call.name
        ))
    }
}

/// The [`ApprovalPort`]: the selection pipeline, run on the model's call.
struct Proposer<'r, S, G, V> {
    loom: &'r Loom<S, G, V>,
    case: &'r CaseId,
    offers: &'r Offers,
    /// The run's cancel: once [`Loom::interrupt`] raised it, nothing leaves the pipeline.
    cancel: LoopCancel,
    /// The call a resumed run continues from, exactly as the model made it, and what the pipeline
    /// left of it, until the port is first asked about a call.
    resuming: Option<(ToolCall, Pending)>,
    /// The checkpoint the run stopped at.
    ended: Option<Ended>,
    /// Whether the pipeline refused a selection of this run `stale-revision` (`CaseMoved`): the
    /// case moved after the revision it was selected at.
    stale_refused: bool,
}

/// The checkpoint a run stopped at, what Loom returns for it, and the call it holds for a run that
/// resumes the session.
struct Ended {
    checkpoint: String,
    outcome: ExecutorOutcome,
    pending: Option<Pending>,
}

impl<S, G, V> ApprovalPort for Proposer<'_, S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// Selects the entry the call names from the latest turn's catalogue, generates its arguments
    /// from the call, and revalidates the selection. A proposal, a suspension, or arguments
    /// Commission's JSON cannot carry end the run at a checkpoint named by the selection's id; a
    /// refusal is denied to the model. A run interrupted while the call was being selected ends at
    /// that checkpoint before revalidation, the selection in flight and nothing proposed.
    ///
    /// The call a resumed run continues from is not selected again: [`Proposer::resolve`].
    fn decide(&mut self, call: &ToolCall, spec: &ToolSpec) -> ApprovalDecision {
        let offers = self.offers;
        let Some(offered) = offers.latest() else {
            return ApprovalDecision::denied("no catalogue was offered for this turn");
        };
        let Some(entry) = offered.entry(&spec.name) else {
            return ApprovalDecision::denied(format!(
                "`{}` is not in this turn's catalogue",
                spec.name
            ));
        };
        // The held call is the first a resumed run puts to the gate. When the gate refused it
        // before asking (the current catalogue does not publish it), the first call asked about is
        // another, and the held call is not answered for. A call is the held one only when it is
        // that exact call, id, tool and arguments, and calls the held proposal's action: a call
        // that reuses the id for anything else is selected as itself.
        if let Some((held, pending)) = self.resuming.take()
            && held == *call
            && entry.action == pending.proposal.action
        {
            return self.resolve(pending, offered);
        }
        // Numbered across every run of this Loom: a resumed run may project under no new turn
        // index, so a count of its own would repeat an earlier run's ids.
        let number = self.loom.next_number();
        let instance = &self.loom.instance;
        let scope = &offered.catalogue.data().catalogue_id.0.0;
        let selection_id = SelectionId(loom_id(instance, "selection", scope, number));
        let checkpoint = selection_id.0.0.clone();
        let arguments = ModelArguments {
            arguments: &call.arguments,
            uncarried: Cell::new(false),
        };
        let prepared = self.loom.prepare(
            &ModelSelection {
                action: entry.action.clone(),
            },
            &arguments,
            (&offered.frontier, &offered.catalogue),
            (
                selection_id,
                ArgumentRequestId(loom_id(instance, "argument-request", scope, number)),
            ),
        );
        if arguments.uncarried.get() {
            // Arguments the model wrote that Commission cannot carry end the run with no proposal:
            // not an outage, and not a denial the model would retry.
            return self.defer(checkpoint, no_useful_action(), None);
        }
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(outcome) => return self.answer(checkpoint, outcome, None, &entry.action),
        };
        let mut pending = Pending {
            selection: prepared.selection_id.clone(),
            proposal: prepared.proposal(),
            in_flight: true,
        };
        if self.cancel.is_cancelled() {
            // The cancel reached the run while the call was selected: the selection does not
            // leave Loom. The run that resumes the session revalidates it.
            return self.defer(checkpoint, no_useful_action(), Some(pending));
        }
        let outcome = self.loom.finish(self.case, prepared);
        pending.in_flight = false;
        self.answer(checkpoint, outcome, Some(pending), &entry.action)
    }
}

impl<S, G, V> Proposer<'_, S, G, V>
where
    V: Deref,
    V::Target: Governor,
{
    /// Ends the run at `checkpoint` with `outcome`, holding `pending` when the outcome proposes it;
    /// a selection the pipeline refused (`NoUsefulAction`, or `CaseMoved` for one made at a
    /// revision the case has left, which is noted) is denied to the model instead, which chooses
    /// again from the next turn's catalogue, projected from the frontier current then.
    fn answer(
        &mut self,
        checkpoint: String,
        outcome: ExecutorOutcome,
        pending: Option<Pending>,
        action: &str,
    ) -> ApprovalDecision {
        if matches!(outcome, ExecutorOutcome::CaseMoved(_)) {
            self.stale_refused = true;
        }
        if matches!(
            outcome,
            ExecutorOutcome::NoUsefulAction(_) | ExecutorOutcome::CaseMoved(_)
        ) {
            return ApprovalDecision::denied(format!(
                "`{action}` was not proposed: the case's current frontier does not admit it as it \
                 was selected; choose from the tools of the next turn"
            ));
        }
        let pending = pending.filter(|_| matches!(outcome, ExecutorOutcome::ProposedAction(_)));
        self.defer(checkpoint, outcome, pending)
    }

    /// Ends the run at `checkpoint` with `outcome`, holding `pending` for a run that resumes it.
    fn defer(
        &mut self,
        checkpoint: String,
        outcome: ExecutorOutcome,
        pending: Option<Pending>,
    ) -> ApprovalDecision {
        self.ended = Some(Ended {
            checkpoint: checkpoint.clone(),
            outcome,
            pending,
        });
        ApprovalDecision::deferred(checkpoint)
    }

    /// The call a resumed run continues from, against `offered`, the catalogue projected at resume
    /// from the case's current frontier. It is not selected again (`crate::recovery`): a selection
    /// in flight is revalidated first, and either is proposed again only while the case is at the
    /// revision it was selected at and the frontier still admits its action. A refusal is denied to
    /// the model, naming why; an outage of the governor ends the run at the checkpoint, still held.
    /// A run interrupted before this ends at the checkpoint with the call as it found it, a
    /// selection in flight left unrevalidated.
    fn resolve(&mut self, pending: Pending, offered: &Offered) -> ApprovalDecision {
        let checkpoint = pending.selection.0.0.clone();
        if self.cancel.is_cancelled() {
            return self.defer(checkpoint, no_useful_action(), Some(pending));
        }
        let action = pending.proposal.action.clone();
        let selected = SelectionStorage::get(&*self.loom.record(), &pending.selection);
        let Some(selected) = selected else {
            return ApprovalDecision::denied(format!(
                "`{action}` was selected before the run was resumed and is no longer recorded: it \
                 is not proposed; choose from the tools of this turn"
            ));
        };
        if pending.in_flight {
            match self.loom.revalidate(self.case, pending.selection.clone()) {
                Ok(()) => {}
                Err(ExecutorOutcome::NoUsefulAction(_) | ExecutorOutcome::CaseMoved(_)) => {
                    return ApprovalDecision::denied(self.refused(&pending.selection, &action));
                }
                Err(unanswered) => return self.defer(checkpoint, unanswered, Some(pending)),
            }
        }
        // Whatever revalidation answered, or when a Loom with no governor made none: a selection in
        // flight gets every check an admitted one does.
        let frontier = &offered.frontier;
        let current = frontier.data().case_revision;
        if current != selected.data.case_revision {
            return ApprovalDecision::denied(stale(&action, selected.data.case_revision, current));
        }
        let admission = admit(frontier, &action);
        if matches!(admission, Admission::Refused(_))
            || !has_deciding_entry(frontier, &action, &admission)
        {
            return ApprovalDecision::denied(not_admitted(&action));
        }
        let outcome = ExecutorOutcome::ProposedAction(pending.proposal.clone());
        self.defer(
            checkpoint,
            outcome,
            Some(Pending {
                in_flight: false,
                ..pending
            }),
        )
    }

    /// Why the revalidation of `selection` refused it, as the model is told.
    fn refused(&self, selection: &SelectionId, action: &str) -> String {
        let revalidations = self.loom.revalidations();
        let stale_at = revalidations
            .iter()
            .rev()
            .find_map(|outcome| match outcome {
                RevalidateSelectionOutcome::StaleRevision { selection_stale }
                    if selection_stale.selection_id == *selection =>
                {
                    Some((
                        selection_stale.catalogue_revision,
                        selection_stale.case_revision,
                    ))
                }
                _ => None,
            });
        match stale_at {
            Some((selected_at, current)) => stale(action, selected_at, current),
            None => not_admitted(action),
        }
    }
}

/// What the model is told of a held call selected at another case revision than the current one.
fn stale(action: &str, selected_at: i64, current: i64) -> String {
    format!(
        "`{action}` was selected at case revision {selected_at} and the case is now at revision \
         {current}: it is not proposed; choose from the tools of this turn"
    )
}

/// What the model is told of a held call the case's current frontier does not admit.
fn not_admitted(action: &str) -> String {
    format!(
        "`{action}` is not admitted by the case's current frontier: it is not proposed; choose from \
         the tools of this turn"
    )
}

/// The model's call as the selector: it names the action the call names. Membership in the
/// catalogue is still checked by the selection, as for any selector.
struct ModelSelection {
    action: String,
}

impl ActionSelector for ModelSelection {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Ok(Choice {
            action: self.action.clone(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// The model's call as the argument generator: its arguments, as Commission's JSON.
struct ModelArguments<'c> {
    arguments: &'c serde_json::Value,
    /// Whether Commission's JSON could not carry them: what the model wrote, not an unavailable
    /// dependency.
    uncarried: Cell<bool>,
}

impl ArgumentGenerator for ModelArguments<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        json::parse(&self.arguments.to_string()).map_err(|error| {
            self.uncarried.set(true);
            format!("the model's arguments cannot be carried as Commission's JSON: {error}")
        })
    }
}

/// The [`ModelPort`]: the run's model, recording each turn it completes.
struct Recording<'m, 'r, S, G, V> {
    model: &'m mut dyn ModelPort,
    loom: &'r Loom<S, G, V>,
    session: &'r SessionId,
    offers: &'r Offers,
    /// The run's number, which says whether it still holds `session`.
    run: u64,
    /// Whether the loop asked the model for a turn in this run.
    asked: bool,
}

impl<S, G, V> ModelPort for Recording<'_, '_, S, G, V> {
    fn wire(&self) -> &WireId {
        self.model.wire()
    }

    /// The model's turn. The first one completed after a catalogue was offered is that turn of the
    /// session, recorded with its items; another one before the next offer (a summary turn) is not
    /// a turn of the session.
    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked = true;
        let outcome = self.model.turn(request, sink)?;
        // Only into a session this run still holds: an interrupted run whose session another run
        // resumed records nothing there. While it holds it, the session is `Active` unless an
        // interrupt moved it, so the turn is `recorded` or refused `session-not-active`.
        let recovery = self.loom.recovery();
        if let Some(offered) = self.offers.latest()
            && !offered.recorded.replace(true)
            && recovery.holds(self.session, self.run)
        {
            let _ = self.loom.turn_record().record_turn(RecordTurn {
                turn_id: offered.turn.clone(),
                session_id: self.session.clone(),
                index: offered.index,
                items: outcome.items.iter().map(encoded).collect(),
            });
        }
        drop(recovery);
        Ok(outcome)
    }
}

/// An item as the turn records it: its JSON, the encoding a session file stores it in.
fn encoded(item: &Item) -> String {
    // An item is strings, ids and JSON values, so it always encodes; a turn recorded with
    // anything but its own items would be a false record.
    serde_json::to_string(item).expect("a conversation item always encodes as JSON")
}

/// Loom as Commission's agent executor over the ported loop: each run is one [`Loom::run_loop`] on
/// `model`, recorded into one session, with no sink.
pub struct LoopExecutor<'l, S, G, V, M> {
    loom: &'l Loom<S, G, V>,
    model: Mutex<M>,
    config: LoopConfig,
    session: SessionData,
}

impl<'l, S, G, V, M> LoopExecutor<'l, S, G, V, M> {
    /// An executor that runs `loom`'s loop on `model` under `config`, recording into `session`.
    pub fn new(
        loom: &'l Loom<S, G, V>,
        model: M,
        config: LoopConfig,
        session: SessionData,
    ) -> Self {
        Self {
            loom,
            model: Mutex::new(model),
            config,
            session,
        }
    }
}

impl<S, G, V, M> AgentExecutor for LoopExecutor<'_, S, G, V, M>
where
    V: Deref,
    V::Target: Governor,
    M: ModelPort,
{
    /// [`Loom::run_loop`]'s outcome, unless the case moved off `frontier` while the loop ran: a
    /// turn's catalogue, projected from the governor's frontier current then, was at another case
    /// revision than `frontier`'s, or the pipeline refused one of the run's selections
    /// `stale-revision` (the model was told and chose again). A run that then ends with a
    /// proposal, `CompletedLocalReasoning` or `NoUsefulAction` is `CaseMoved`, naming `frontier`'s
    /// revision, so Commission judges it on the case as it is now (`story:moved-case-outcome`): no
    /// proposal leaves that was not selected and admitted at the revision of the frontier the run
    /// was handed. A suspension is returned as it is. Runs take the model one at a time.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut model = self.model.lock().unwrap_or_else(PoisonError::into_inner);
        let (run, moved) = self.loom.governed(
            &self.session,
            LoopPorts {
                model: &mut *model,
                config: self.config.clone(),
                sink: &mut NullLoopSink,
            },
            commission,
            frontier,
            Start::Fresh,
        );
        match run.outcome {
            ExecutorOutcome::ProposedAction(_)
            | ExecutorOutcome::CompletedLocalReasoning(_)
            | ExecutorOutcome::NoUsefulAction(_)
                if moved =>
            {
                case_moved(frontier.data().case_revision)
            }
            outcome => outcome,
        }
    }
}
