//! `loom.agent.RunOnce`, answered by `b10x_loom_agent::run::run_once`, the decision of how
//! `b10x-loom agent run` ends (`story:agent-run-once`), over its declared inputs alone: whether
//! the spec was refused, whether `--input` was given, read and blank, whether the turn's send was
//! refused, and whether the turn answered. What the binary does around the decision (the turn,
//! the record, the send and the exit status) is held by `crates/loom-cli/tests/agent_run.rs`,
//! one case per scenario of `ess/scenarios/agent-run-once/`.

use b10x_loom_agent::run::{RunOnce, RunOnceOutcome, RunRefused, run_once, run_refusal_spelling};
use ess_conformance::target::SemanticCommandResult;
use ess_primitives::node::Node;

use crate::codec::{self, Input};
use crate::{error, took};

pub(crate) const RUN_ONCE: &str = "loom.agent.RunOnce";
const RUN_REFUSED: &str = "loom.agent.RunRefused";

/// The refusal as `loom.agent.RunRefused`, `reason` in the enum's spelling.
fn refusal(branch: &str, refused: &RunRefused) -> SemanticCommandResult {
    took(RUN_ONCE, branch).with_error(error(RUN_REFUSED).with(
        "reason",
        Node::Text(run_refusal_spelling(refused.reason).to_owned()),
    ))
}

/// Answers one `RunOnce`: `None` when an input is absent or no boolean.
pub(crate) fn run_once_command(input: &Input) -> Option<SemanticCommandResult> {
    let request = RunOnce {
        spec_refused: codec::boolean(input, "spec_refused")?,
        input_given: codec::boolean(input, "input_given")?,
        input_read: codec::boolean(input, "input_read")?,
        input_blank: codec::boolean(input, "input_blank")?,
        send_refused: codec::boolean(input, "send_refused")?,
        answered: codec::boolean(input, "answered")?,
    };
    Some(match run_once(&request) {
        RunOnceOutcome::SpecRefused { error } => refusal("spec-refused", &error),
        RunOnceOutcome::InputAbsent { error } => refusal("input-absent", &error),
        RunOnceOutcome::InputUnreadable { error } => refusal("input-unreadable", &error),
        RunOnceOutcome::InputEmpty { error } => refusal("input-empty", &error),
        RunOnceOutcome::SendRefused { error } => refusal("send-refused", &error),
        RunOnceOutcome::NoAnswer => took(RUN_ONCE, "no-answer"),
        RunOnceOutcome::Answered => took(RUN_ONCE, "answered"),
    })
}
