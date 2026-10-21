//! `loom.session.ProposeSessionChange`, answered by `b10x-loom-session-executor`.
//!
//! The executor records a session only by running one, so this target runs one: a
//! `SessionExecutor` of `Kind::Claude` over a workspace made for the invocation, its session a
//! scripted Claude Code session ([`Scripted`]) and its session id the input's. The script makes the
//! input's `files` in the scratch copy the session is started in (a null `content` removes a file
//! the workspace holds for it), names the input's `vendor_session_id` as the session's own id, and
//! replies naming the input's `action`. The commission and the frontier the executor runs for carry
//! the input's ids.
//!
//! What is published is what the executor recorded through its generated behaviour: the
//! `SessionChangeProposed` event it returned and the `AgentSessions` rows it holds. The executor
//! reads the change from the copy, in path order, so the event's `files` are the input's in path
//! order.
//!
//! Only `ClaudeCode` is driven: the synthesized suite names that kind. A `Codex` or `Harness`
//! session would need its own scripted records, and the target reports it unsupported instead of
//! answering for it.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
};
use b10x_loom_commission::ports::executor::AgentExecutor as _;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::{
    AgentSessions, ChangedFile, SessionId, SessionKind,
};
use b10x_loom_session_executor::{CredentialSource, Kind, ProcessRunner, SessionExecutor, TOOLS};
use ess_conformance::target::{SemanticCommandResult, TargetError};
use ess_primitives::ids::CorrelationId;

use crate::codec::{self, Input};
use crate::{Live, event, took};

pub(crate) const PROPOSE_SESSION_CHANGE: &str = "loom.session.ProposeSessionChange";
pub(crate) const SESSION_CHANGE_PROPOSED: &str = "loom.session.SessionChangeProposed";
pub(crate) const AGENT_SESSIONS: &str = "loom.session.AgentSessions";

/// The agent revision the commission is assigned to; the session record does not carry it.
const AGENT_REVISION: &str = "00000000-0000-4000-8000-000000000001";

/// What a file removed by the session held in the workspace before the session ran.
const REMOVED: &str = "held until the session removed it\n";

/// The directory one invocation's workspace and scratch copies are made in, under the system's
/// temporary directory, not under the repository: metaharness refuses a Claude session whose copy
/// has memory files above it. Removed when dropped.
struct Invocation(PathBuf);

impl Invocation {
    fn make(sequence: u64) -> Result<Self, TargetError> {
        let root = std::env::temp_dir().join(format!(
            "loom-conformance-session-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("workspace"))
            .and_then(|()| std::fs::create_dir_all(root.join("scratch")))
            .map_err(|error| unavailable(format!("make {}: {error}", root.display())))?;
        Ok(Self(root))
    }

    fn workspace(&self) -> PathBuf {
        self.0.join("workspace")
    }

    /// Writes into the workspace the file each removal in `files` removes.
    fn hold_removed(&self, files: &[ChangedFile]) -> Result<(), TargetError> {
        for file in files.iter().filter(|file| file.content.is_none()) {
            write(&self.workspace().join(&file.path), REMOVED)?;
        }
        Ok(())
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(at: &Path, text: &str) -> Result<(), TargetError> {
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| unavailable(format!("make {}: {error}", parent.display())))?;
    }
    std::fs::write(at, text)
        .map_err(|error| unavailable(format!("write {}: {error}", at.display())))
}

fn unavailable(detail: String) -> TargetError {
    TargetError::unavailable(format!("invoking `{PROPOSE_SESSION_CHANGE}`"), detail)
}

/// A scripted Claude Code session making `files` and replying `action`, its own session id
/// `vendor`'s when there is one.
struct Scripted {
    files: Vec<ChangedFile>,
    action: String,
    vendor: Option<String>,
}

impl ProcessRunner for Scripted {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        for file in &self.files {
            let at = plan.cwd.join(&file.path);
            match &file.content {
                Some(text) => {
                    if let Some(parent) = at.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(at, text)?;
                }
                None => std::fs::remove_file(at)?,
            }
        }
        let mut init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": TOOLS, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default",
        });
        if let Some(vendor) = &self.vendor {
            init["session_id"] = serde_json::Value::String(vendor.clone());
        }
        let said = serde_json::json!({"action": self.action, "summary": "proposed"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 1, "output_tokens": 1}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "usage": {"input_tokens": 1, "output_tokens": 1},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

/// `ProposeSessionChange`: a scripted session run by the executor, and what it recorded.
pub(crate) fn propose_session_change(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(decoded) = decode(input) else {
        return Ok(None);
    };
    if decoded.kind != SessionKind::ClaudeCode {
        return Err(TargetError::unsupported(
            format!("a `{:?}` session", decoded.kind),
            "this target scripts Claude Code sessions only, the kind the suite names",
        ));
    }
    let invocation = Invocation::make(live.sequence)?;
    invocation.hold_removed(&decoded.files)?;
    let paths = decoded.files.iter().map(|file| file.path.clone()).collect();
    let session_id = decoded.session_id.clone();
    let mut executor = SessionExecutor::new(invocation.workspace(), paths, "propose the change")
        .of_kind(Kind::Claude)
        .with_runner(Box::new(Scripted {
            files: decoded.files.clone(),
            action: decoded.action.clone(),
            vendor: decoded.vendor_session_id.clone(),
        }))
        .with_session_ids(move || session_id.clone());
    executor.credentials = CredentialSource::None;
    executor.scratch = invocation.0.join("scratch");

    let case = CaseId("case".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(Uuid(decoded.commission_id.clone())),
        agent_revision_id: AgentRevisionId(Uuid(AGENT_REVISION.to_owned())),
        case_id: case.clone(),
        principal: PrincipalId("conformance".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid(decoded.frontier_id)),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: decoded.action,
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    });
    let step = executor.run_with_usage(&commission, &frontier);
    if let ExecutorOutcome::Suspended(suspended) = &step.outcome {
        return Err(unavailable(format!(
            "the scripted session failed: {:?}",
            suspended.reason
        )));
    }
    let Some(proposed) = executor.proposed().pop() else {
        return Err(unavailable(
            "the executor proposed the session's change but recorded no session".to_owned(),
        ));
    };
    live.sessions.extend(executor.sessions());
    let published = live.publish(
        event(SESSION_CHANGE_PROPOSED)
            .with("session_id", codec::id(&proposed.session_id.0))
            .with("commission_id", codec::id(&proposed.commission_id.0))
            .with("frontier_id", codec::id(&proposed.frontier_id.0))
            .with("action", codec::text_node(&proposed.action))
            .with("files", codec::changed_files_node(&proposed.files)),
        correlation,
    );
    Ok(Some(
        took(PROPOSE_SESSION_CHANGE, "proposed").emitting(published),
    ))
}

/// The input of one `ProposeSessionChange`.
struct Decoded {
    session_id: SessionId,
    kind: SessionKind,
    commission_id: String,
    frontier_id: String,
    vendor_session_id: Option<String>,
    action: String,
    files: Vec<ChangedFile>,
}

fn decode(input: &Input) -> Option<Decoded> {
    Some(Decoded {
        session_id: SessionId(codec::uuid(input, "session_id")?),
        kind: codec::session_kind(input, "kind")?,
        commission_id: codec::uuid(input, "commission_id")?.0,
        frontier_id: codec::uuid(input, "frontier_id")?.0,
        vendor_session_id: codec::optional_text(input, "vendor_session_id")?,
        action: codec::text(input, "action")?,
        files: codec::changed_files(input, "files")?,
    })
}

/// One `AgentSessions` row.
pub(crate) fn row(held: &AgentSessions) -> ess_conformance::target::ViewRow {
    ess_conformance::target::ViewRow::from([
        ("session_id".to_owned(), codec::id(&held.session_id.0)),
        ("kind".to_owned(), codec::session_kind_name(held.kind)),
        ("commission_id".to_owned(), codec::id(&held.commission_id.0)),
        ("frontier_id".to_owned(), codec::id(&held.frontier_id.0)),
        (
            "vendor_session_id".to_owned(),
            held.vendor_session_id
                .as_deref()
                .map_or(ess_primitives::node::Node::Null, codec::text_node),
        ),
        ("action".to_owned(), codec::text_node(&held.action)),
        ("files".to_owned(), codec::changed_files_node(&held.files)),
        ("state".to_owned(), codec::agent_session_state(held.state)),
    ])
}
