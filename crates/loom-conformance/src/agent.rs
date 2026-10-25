//! `loom.agent.CheckAgentSpec`, answered by `b10x_loom_agent::check_text` over the plugin
//! catalog (`ProtocolCatalog::plugins()`), and `loom.agent.AdmitSend`, answered by
//! `b10x_loom_agent::admit_send` over its declared inputs alone.
//!
//! The command's one input is the document's `path`, and every refusal is `external:`: the
//! document and the catalog are outside the input. This target plays the file system, as it plays
//! the governor for `not-in-frontier`: a `path` naming a regular file under the repository root is
//! read as it is (the authored scenarios name the fixtures under
//! `ess/scenarios/agent-check/specs/` and `examples/agents/operator.yaml`); a synthesized path
//! names none, and the target reads the fixture of the branch the scenario forced with
//! `configure_external_outcome`, `ess/scenarios/agent-check/specs/<branch>.yaml`, or, forced
//! nothing, `examples/agents/operator.yaml`. The check decides the outcome from what it read:
//! each fixture holds the one defect of its branch, so a check answering another reason fails the
//! scenario. A forced branch holds for the next invocation only.

use std::path::{Path, PathBuf};

use b10x_loom_agent::{
    AdmitSend, AdmitSendOutcome, RefusalReason, SendRefused, SpecRefused, WriteMode, admit_send,
    check_text, send_refusal_spelling, spelling,
};
use ess_conformance::target::{SemanticCommandResult, TargetError};
use ess_primitives::node::Node;
use loom_protocols::ProtocolCatalog;

use crate::codec::{self, Input};
use crate::{Live, error, took};

pub(crate) const CHECK_AGENT_SPEC: &str = "loom.agent.CheckAgentSpec";
const SPEC_REFUSED: &str = "loom.agent.SpecRefused";
pub(crate) const ADMIT_SEND: &str = "loom.agent.AdmitSend";
const SEND_REFUSED: &str = "loom.agent.SendRefused";

/// The refusal branches of `CheckAgentSpec`, each with the reason its fixture is refused for, in
/// declaration order.
const BRANCHES: [(&str, RefusalReason); 8] = [
    ("malformed", RefusalReason::Malformed),
    ("secret-literal", RefusalReason::SecretLiteral),
    ("unknown-protocol", RefusalReason::UnknownProtocol),
    ("unknown-action", RefusalReason::UnknownAction),
    ("unbound-source", RefusalReason::UnboundSource),
    ("http-method", RefusalReason::HttpMethod),
    ("send-without-writes", RefusalReason::SendWithoutWrites),
    ("empty-channel-allow", RefusalReason::EmptyChannelAllow),
];

/// Where the fixtures of the forced branches are, under the repository root.
const FIXTURES: &str = "ess/scenarios/agent-check/specs";

/// The document read when the path names no file and nothing was forced.
const SERVEABLE: &str = "examples/agents/operator.yaml";

/// Whether `branch` is a refusal branch of `CheckAgentSpec` a scenario may force.
pub(crate) fn is_branch(branch: &str) -> bool {
    BRANCHES.iter().any(|(name, _)| *name == branch)
}

/// The repository root, read when the target runs: a test binary may be reused across worktrees
/// that share one build directory.
fn root() -> Result<PathBuf, TargetError> {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        TargetError::unavailable(
            "locating the repository",
            "CARGO_MANIFEST_DIR is unset: run the suite through cargo",
        )
    })?;
    let root = PathBuf::from(manifest).join("../..");
    Ok(root.canonicalize().unwrap_or(root))
}

/// The document the check reads for `path`, as the module documentation says.
fn document(root: &Path, path: &str, forced: Option<&str>) -> Result<String, TargetError> {
    let named = root.join(path);
    let read = if named.is_file() {
        named
    } else if let Some(branch) = forced {
        root.join(FIXTURES).join(format!("{branch}.yaml"))
    } else {
        root.join(SERVEABLE)
    };
    std::fs::read_to_string(&read).map_err(|why| {
        TargetError::unavailable(
            format!("reading {}", read.display()),
            format!("the document cannot be read: {why}"),
        )
    })
}

/// The branch `reason` answers.
fn branch(reason: RefusalReason) -> &'static str {
    BRANCHES
        .iter()
        .find(|(_, held)| *held == reason)
        .map_or_else(
            || unreachable!("every reason has its branch"),
            |(name, _)| name,
        )
}

/// The refusal as `loom.agent.SpecRefused`, `reason` in the enum's spelling.
fn refusal(refused: &SpecRefused) -> SemanticCommandResult {
    took(CHECK_AGENT_SPEC, branch(refused.reason)).with_error(
        error(SPEC_REFUSED)
            .with("reason", Node::Text(spelling(refused.reason).to_owned()))
            .with("member", Node::Text(refused.member.clone()))
            .with("detail", Node::Text(refused.detail.clone())),
    )
}

/// The `loom.agent.WriteMode` field `name`, in the enum's spelling.
fn write_mode(input: &Input, name: &str) -> Option<WriteMode> {
    match codec::text(input, name)?.as_str() {
        "auto" => Some(WriteMode::Auto),
        "approval" => Some(WriteMode::Approval),
        "none" => Some(WriteMode::None),
        _ => None,
    }
}

/// The refusal as `loom.agent.SendRefused`, `reason` in the enum's spelling.
fn send_refusal(branch: &str, refused: &SendRefused) -> SemanticCommandResult {
    took(ADMIT_SEND, branch).with_error(
        error(SEND_REFUSED)
            .with(
                "reason",
                Node::Text(send_refusal_spelling(refused.reason).to_owned()),
            )
            .with("final", Node::Bool(refused.r#final)),
    )
}

/// Answers one `AdmitSend`: `None` when an input is absent or cannot be held by its type.
pub(crate) fn admit_send_command(input: &Input) -> Option<SemanticCommandResult> {
    let request = AdmitSend {
        mode: write_mode(input, "mode")?,
        channel_allowed: codec::boolean(input, "channel_allowed")?,
        proposed: codec::boolean(input, "proposed")?,
        granted: codec::boolean(input, "granted")?,
        sent_in_window: codec::integer(input, "sent_in_window")?,
        max_per_hour: codec::integer(input, "max_per_hour")?,
    };
    Some(match admit_send(&request) {
        AdmitSendOutcome::WritesNone { error } => send_refusal("writes-none", &error),
        AdmitSendOutcome::OutsideAllowList { error } => send_refusal("outside-allow-list", &error),
        AdmitSendOutcome::NoProposal { error } => send_refusal("no-proposal", &error),
        AdmitSendOutcome::RateLimited { error } => send_refusal("rate-limited", &error),
        AdmitSendOutcome::Held => took(ADMIT_SEND, "held"),
        AdmitSendOutcome::Admitted => took(ADMIT_SEND, "admitted"),
    })
}

/// Answers one `CheckAgentSpec`: `None` when the input carries no text `path`.
pub(crate) fn check_agent_spec(
    live: &mut Live,
    input: &Input,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(path) = codec::text(input, "path") else {
        return Ok(None);
    };
    let forced = live.forced_check.take();
    let text = document(&root()?, &path, forced.as_deref())?;
    let catalog = ProtocolCatalog::plugins()
        .map_err(|why| TargetError::unavailable("loading the plugin catalog", why))?;
    let actions = |name: &str| catalog.get(name).map(|entry| entry.actions());
    Ok(Some(match check_text(&text, &actions) {
        Ok(_) => took(CHECK_AGENT_SPEC, "serveable"),
        Err(refused) => refusal(&refused),
    }))
}
