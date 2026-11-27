//! `loom.agent.AdmitA2aRequest`, answered by `b10x_loom_agent::serve::a2a::admit_a2a_request`
//! (the A2A listener's admission, `b10x_loom_a2a`) over its declared inputs alone: whether the
//! request is a probe, whether it presents a bearer, whether that bearer is the agent's, and
//! whether its method is `SendStreamingMessage`.
//!
//! `loom.agent.RecordA2aTurn`, answered by `b10x_loom_agent::serve::a2a::record_a2a_turn`, the
//! decision `Answering` takes before an A2A turn: whether the record has a line for its item, and
//! whether a turn of the item ran in this process unrecorded.
//!
//! `loom.agent.AdmitServe`, answered by `b10x_loom_agent::serve::a2a::admit_serve`, the decision
//! `SpecPlugin::new` takes: whether the spec declares a channel and whether it declares
//! `expose.a2a`.

use b10x_loom_agent::serve::a2a::{
    A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome, AdmitServe, AdmitServeOutcome,
    RecordA2aTurn, RecordA2aTurnOutcome, ServeRefused, a2a_refusal_spelling, admit_a2a_request,
    admit_serve, record_a2a_turn, serve_refusal_spelling,
};
use b10x_loom_executor::model::agent::obligations::{
    AdmitServedSessionTurnBehavior, EndServedSessionTurnBehavior,
};
use b10x_loom_executor::model::agent::{
    AdmitServedSessionTurn, AdmitServedSessionTurnOutcome, EndServedSessionTurn,
    EndServedSessionTurnOutcome,
};
use b10x_loom_executor::model::behaviour::Generated;
use ess_conformance::target::SemanticCommandResult;
use ess_primitives::node::Node;

use crate::codec::{self, Input};
use crate::{error, took};

pub(crate) const ADMIT_A2A_REQUEST: &str = "loom.agent.AdmitA2aRequest";
const A2A_REFUSED: &str = "loom.agent.A2aRefused";
pub(crate) const RECORD_A2A_TURN: &str = "loom.agent.RecordA2aTurn";
pub(crate) const ADMIT_SERVE: &str = "loom.agent.AdmitServe";
pub(crate) const ADMIT_SERVED_SESSION_TURN: &str = "loom.agent.AdmitServedSessionTurn";
pub(crate) const END_SERVED_SESSION_TURN: &str = "loom.agent.EndServedSessionTurn";
const SERVE_REFUSED: &str = "loom.agent.ServeRefused";

/// The refusal as `loom.agent.A2aRefused`, `reason` in the enum's spelling.
fn refusal(branch: &str, refused: &A2aRefused) -> SemanticCommandResult {
    took(ADMIT_A2A_REQUEST, branch).with_error(error(A2A_REFUSED).with(
        "reason",
        Node::Text(a2a_refusal_spelling(refused.reason).to_owned()),
    ))
}

/// Answers one `AdmitA2aRequest`: `None` when an input is absent or no boolean.
pub(crate) fn admit_a2a_request_command(input: &Input) -> Option<SemanticCommandResult> {
    let request = AdmitA2aRequest {
        probe: codec::boolean(input, "probe")?,
        bearer_presented: codec::boolean(input, "bearer_presented")?,
        bearer_matches: codec::boolean(input, "bearer_matches")?,
        stream: codec::boolean(input, "stream")?,
    };
    Some(match admit_a2a_request(&request) {
        AdmitA2aRequestOutcome::MissingBearer { error } => refusal("missing-bearer", &error),
        AdmitA2aRequestOutcome::WrongBearer { error } => refusal("wrong-bearer", &error),
        AdmitA2aRequestOutcome::Probe => took(ADMIT_A2A_REQUEST, "probe"),
        AdmitA2aRequestOutcome::Streamed => took(ADMIT_A2A_REQUEST, "streamed"),
        AdmitA2aRequestOutcome::Admitted => took(ADMIT_A2A_REQUEST, "admitted"),
    })
}

/// Answers one `RecordA2aTurn`: `None` when an input is absent or no boolean.
pub(crate) fn record_a2a_turn_command(input: &Input) -> Option<SemanticCommandResult> {
    let turn = RecordA2aTurn {
        recorded: codec::boolean(input, "recorded")?,
        ran_unrecorded: codec::boolean(input, "ran_unrecorded")?,
    };
    Some(match record_a2a_turn(&turn) {
        RecordA2aTurnOutcome::Replayed => took(RECORD_A2A_TURN, "replayed"),
        RecordA2aTurnOutcome::Unrecorded => took(RECORD_A2A_TURN, "unrecorded"),
        RecordA2aTurnOutcome::Turned => took(RECORD_A2A_TURN, "turned"),
    })
}

/// The refusal as `loom.agent.ServeRefused`, `reason` in the enum's spelling.
fn serve_refusal(branch: &str, refused: &ServeRefused) -> SemanticCommandResult {
    took(ADMIT_SERVE, branch).with_error(error(SERVE_REFUSED).with(
        "reason",
        Node::Text(serve_refusal_spelling(refused.reason).to_owned()),
    ))
}

/// Runs the generated admission decision used by the session host before accepting inbound session work.
pub(crate) fn admit_served_session_turn(input: &Input) -> Option<SemanticCommandResult> {
    let command = AdmitServedSessionTurn {
        kind: codec::session_kind(input, "kind")?,
        a2a_only: codec::boolean(input, "a2a_only")?,
        workspace_ready: codec::boolean(input, "workspace_ready")?,
        policy_ready: codec::boolean(input, "policy_ready")?,
    };
    let outcome = Generated::new(())
        .admit_served_session_turn(command)
        .expect("all session admission guards are total over the decoded inputs");
    Some(took(
        ADMIT_SERVED_SESSION_TURN,
        match outcome {
            AdmitServedSessionTurnOutcome::Unsupported => "unsupported",
            AdmitServedSessionTurnOutcome::NoIngress => "no-ingress",
            AdmitServedSessionTurnOutcome::InvalidWorkspace => "invalid-workspace",
            AdmitServedSessionTurnOutcome::RefusedPolicy => "refused-policy",
            AdmitServedSessionTurnOutcome::ClaudeSession => "claude-session",
            AdmitServedSessionTurnOutcome::CodexSession => "codex-session",
        },
    ))
}

/// Runs the same generated completion decision as the governed session's record writer.
pub(crate) fn end_served_session_turn(input: &Input) -> Option<SemanticCommandResult> {
    let command = EndServedSessionTurn {
        suspended: codec::boolean(input, "suspended")?,
        proposed_change: codec::boolean(input, "proposed_change")?,
    };
    let outcome = Generated::new(())
        .end_served_session_turn(command)
        .expect("all session completion guards are total over the decoded inputs");
    Some(took(
        END_SERVED_SESSION_TURN,
        match outcome {
            EndServedSessionTurnOutcome::Held => "held",
            EndServedSessionTurnOutcome::Proposed => "proposed",
            EndServedSessionTurnOutcome::CompletedWithoutChange => "completed-without-change",
        },
    ))
}

/// Answers one `AdmitServe`: `None` when an input is absent or no boolean.
pub(crate) fn admit_serve_command(input: &Input) -> Option<SemanticCommandResult> {
    let request = AdmitServe {
        channel: codec::boolean(input, "channel")?,
        a2a: codec::boolean(input, "a2a")?,
    };
    Some(match admit_serve(&request) {
        AdmitServeOutcome::NothingToServe { error } => serve_refusal("nothing-to-serve", &error),
        AdmitServeOutcome::PollsAndAnswers => took(ADMIT_SERVE, "polls-and-answers"),
        AdmitServeOutcome::Polls => took(ADMIT_SERVE, "polls"),
        AdmitServeOutcome::Answers => took(ADMIT_SERVE, "answers"),
    })
}
