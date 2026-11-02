//! One run of a `loom.agent/1` document on one item: `b10x-loom agent run --spec <FILE> --state
//! <DIR> --input <FILE>` (`story:agent-run-once`).
//!
//! The item is the task text the caller gives ([`run_item`]), and the plugin host handles it as
//! `agent serve` handles a polled item (`b10x_loom_plugin::run_item`): the classifier decides its
//! intent, the state whose `when` names it is the projection, the turn runs under the spec's
//! authority and budgets, and one line is appended to `record.jsonl` in the state directory, the
//! item kept as handled. Nothing is polled and no A2A is answered, so the spec needs neither a
//! `channel` nor `expose.a2a` ([`SpecPlugin::for_run`]). [`RunItem`] picks the turn:
//!
//! - a spec whose channel binding declares a `write` runs a served turn on the inner loop's
//!   protocol with the send policy, and in `writes.mode: auto` performs the send `AdmitSend`
//!   admits in the turn, to the first channel of `channel.allow`, which [`run_item`] names as the
//!   item's channel; `approval` holds it for `b10x-loom approve` and `agent resume` as a served
//!   turn does;
//! - any other runs as an A2A turn does: on `inbound-answer@1`, `reply.send` out of the
//!   projection and no send policy, so it ends on its proposal and sends nothing.
//!
//! How the run ends is `loom.agent.RunOnce` ([`run_once`]), over the refusals of the spec and the
//! input and the record line ([`ended`]).

use chrono::{DateTime, Utc};

use b10x_loom_plugin::turn::NOT_SENT;
use b10x_loom_plugin::{
    Classification, Host, InboundItem, ItemId, Objective, Plugin, PluginError, PluginState, Poll,
    Projection, RecordLine, RecordOutcome, TurnModel, TurnResult,
};
use llm_core::Model;
use loom::json::Value;
use loom_sdk::agent::AgentSpec;

pub use loom::agent::{RunOnce, RunOnceOutcome, RunRefusalReason, RunRefused};

use crate::serve::SpecPlugin;
use crate::serve::a2a::answer_projection;

/// How the id of a run's item starts; the instant of the run follows.
pub const RUN_ITEM_PREFIX: &str = "run-";

/// The revision of a run's item: each run is an item of its own, so it has one revision.
pub const RUN_ITEM_REVISION: &str = "1";

/// Decides `loom.agent.RunOnce` (`ess/domains/agent.yaml`): the first refusal in declaration
/// order (the spec, then the input absent, unreadable or blank, then a refused send), then
/// `no-answer` for a turn that ended stopped or unclassified, else `answered`.
pub fn run_once(input: &RunOnce) -> RunOnceOutcome {
    let refused = |reason: RunRefusalReason| RunRefused { reason };
    if input.spec_refused {
        return RunOnceOutcome::SpecRefused {
            error: refused(RunRefusalReason::SpecRefused),
        };
    }
    if !input.input_given {
        return RunOnceOutcome::InputAbsent {
            error: refused(RunRefusalReason::InputAbsent),
        };
    }
    if !input.input_read {
        return RunOnceOutcome::InputUnreadable {
            error: refused(RunRefusalReason::InputUnreadable),
        };
    }
    if input.input_blank {
        return RunOnceOutcome::InputEmpty {
            error: refused(RunRefusalReason::InputEmpty),
        };
    }
    if input.send_refused {
        return RunOnceOutcome::SendRefused {
            error: refused(RunRefusalReason::SendRefused),
        };
    }
    if !input.answered {
        return RunOnceOutcome::NoAnswer;
    }
    RunOnceOutcome::Answered
}

/// `reason` as the enum `loom.agent.RunRefusalReason` spells it.
pub fn run_refusal_spelling(reason: RunRefusalReason) -> &'static str {
    match reason {
        RunRefusalReason::SpecRefused => "SpecRefused",
        RunRefusalReason::InputAbsent => "InputAbsent",
        RunRefusalReason::InputUnreadable => "InputUnreadable",
        RunRefusalReason::InputEmpty => "InputEmpty",
        RunRefusalReason::SendRefused => "SendRefused",
    }
}

/// The input of [`run_once`] for a run whose spec and input were accepted and whose turn the
/// record line `line` says: the send refused when the turn proposed and its detail starts with
/// [`NOT_SENT`]; answered when it ended proposed, declined or with a proposed case.
pub fn ended(line: &RecordLine) -> RunOnce {
    let send_refused = line.outcome == RecordOutcome::Proposed
        && line
            .detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with(NOT_SENT));
    let answered = matches!(
        line.outcome,
        RecordOutcome::Proposed | RecordOutcome::Declined | RecordOutcome::ProposedCase
    );
    RunOnce {
        spec_refused: false,
        input_given: true,
        input_read: true,
        input_blank: false,
        send_refused,
        answered,
    }
}

/// The item of one run at `at`: `text`, the id [`RUN_ITEM_PREFIX`] and the instant to the
/// nanosecond, revision [`RUN_ITEM_REVISION`], and, for a spec with a channel, the first channel
/// `channel.allow` lists as the item's `channel`: the one a send goes to.
pub fn run_item(spec: &AgentSpec, text: &str, at: DateTime<Utc>) -> InboundItem {
    let mut details = Vec::new();
    if let Some(channel) = spec
        .channel
        .as_ref()
        .and_then(|channel| channel.allow.first())
    {
        details.push(("channel".to_owned(), Value::Text(channel.clone())));
    }
    InboundItem {
        id: ItemId(format!(
            "{RUN_ITEM_PREFIX}{}",
            at.format("%Y%m%dT%H%M%S%.9fZ")
        )),
        revision: RUN_ITEM_REVISION.to_owned(),
        text: text.to_owned(),
        details: Value::Object(details),
    }
}

/// [`run_item`] now.
pub fn run_item_now(spec: &AgentSpec, text: &str) -> InboundItem {
    run_item(spec, text, Utc::now())
}

/// The plugin of one run: the [`SpecPlugin`]'s hooks, its turn the served one when the spec can
/// send ([`SpecPlugin::sends`]) and the A2A one otherwise (see the module documentation).
pub struct RunItem<'a>(&'a SpecPlugin);

impl<'a> RunItem<'a> {
    /// The run of `plugin`.
    pub fn new(plugin: &'a SpecPlugin) -> Self {
        Self(plugin)
    }
}

impl Plugin for RunItem<'_> {
    fn name(&self) -> &str {
        self.0.name()
    }

    /// A run polls nothing: its one item is the caller's.
    fn poll(
        &self,
        _host: &Host<'_>,
        state: &PluginState,
        _objectives: &[Objective],
    ) -> Result<Poll, PluginError> {
        Ok(Poll {
            items: Vec::new(),
            cursors: state.cursors.clone(),
        })
    }

    fn classify(&self, host: &Host<'_>, item: &InboundItem) -> Result<Classification, PluginError> {
        self.0.classify(host, item)
    }

    fn project(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
    ) -> Projection {
        let projection = self.0.project(host, item, classification);
        if self.0.sends() {
            projection
        } else {
            answer_projection(projection)
        }
    }

    fn turn(
        &self,
        host: &Host<'_>,
        item: &InboundItem,
        classification: &Classification,
        projection: &Projection,
    ) -> Result<TurnResult, PluginError> {
        if self.0.sends() {
            self.0.turn(host, item, classification, projection)
        } else {
            self.0
                .answer_turn(host, item, classification, projection, None)
        }
    }

    fn result(
        &self,
        item: &InboundItem,
        classification: &Classification,
        turn: TurnResult,
    ) -> RecordLine {
        self.0.result(item, classification, turn)
    }

    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        self.0.classifier()
    }

    fn turn_model(&self) -> Result<TurnModel<'_>, PluginError> {
        self.0.turn_model()
    }
}
