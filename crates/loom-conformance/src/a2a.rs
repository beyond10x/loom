//! `loom.agent.AdmitA2aRequest`, answered by `b10x_loom_agent::serve::a2a::admit_a2a_request`
//! (the A2A listener's admission, `b10x_loom_a2a`) over its declared inputs alone: whether the
//! request is a probe, whether it presents a bearer, and whether that bearer is the agent's.
//!
//! `loom.agent.AdmitServe`, answered by `b10x_loom_agent::serve::a2a::admit_serve`, the decision
//! `SpecPlugin::new` takes: whether the spec declares a channel and whether it declares
//! `expose.a2a`.

use b10x_loom_agent::serve::a2a::{
    A2aRefused, AdmitA2aRequest, AdmitA2aRequestOutcome, AdmitServe, AdmitServeOutcome,
    ServeRefused, a2a_refusal_spelling, admit_a2a_request, admit_serve, serve_refusal_spelling,
};
use ess_conformance::target::SemanticCommandResult;
use ess_primitives::node::Node;

use crate::codec::{self, Input};
use crate::{error, took};

pub(crate) const ADMIT_A2A_REQUEST: &str = "loom.agent.AdmitA2aRequest";
const A2A_REFUSED: &str = "loom.agent.A2aRefused";
pub(crate) const ADMIT_SERVE: &str = "loom.agent.AdmitServe";
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
    };
    Some(match admit_a2a_request(&request) {
        AdmitA2aRequestOutcome::MissingBearer { error } => refusal("missing-bearer", &error),
        AdmitA2aRequestOutcome::WrongBearer { error } => refusal("wrong-bearer", &error),
        AdmitA2aRequestOutcome::Probe => took(ADMIT_A2A_REQUEST, "probe"),
        AdmitA2aRequestOutcome::Admitted => took(ADMIT_A2A_REQUEST, "admitted"),
    })
}

/// The refusal as `loom.agent.ServeRefused`, `reason` in the enum's spelling.
fn serve_refusal(branch: &str, refused: &ServeRefused) -> SemanticCommandResult {
    took(ADMIT_SERVE, branch).with_error(error(SERVE_REFUSED).with(
        "reason",
        Node::Text(serve_refusal_spelling(refused.reason).to_owned()),
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
