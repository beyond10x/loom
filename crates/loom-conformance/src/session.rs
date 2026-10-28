//! `loom.session.ProposeSessionChange` and `loom.session.RecordFailedSession`, answered by
//! `b10x-loom-session-executor`.
//!
//! `RecordFailedSession` refuses a failure no harness child ends with, as the executor's record
//! does (`failure_refusal`). Any other runs a scripted Claude Code or Codex session that opens and
//! ends as the input's `failure` says, with no reply and no terminal record ([`Failing`]): its
//! exit status or signal, or none measured, and the input's stderr tail on its stderr. The
//! executor suspends the step and records the session as failed, with its own reason; what is
//! published is the `SessionFailed` it returned and the rows it holds.
//!
//! The executor records a session only by running one, so this target runs one: a
//! `SessionExecutor` of `Kind::Claude` over a workspace made for the invocation, its session a
//! scripted Claude Code session ([`Scripted`]) and its session id the input's. The script makes the
//! input's `files` in the scratch copy the session is started in (a null `content` removes a file
//! the workspace holds for it), names the input's `vendor_session_id` as the session's own id, and
//! replies naming the input's `action`. The commission and the frontier the executor runs for carry
//! the input's ids, and the executor records the input's `agent` as the agent it runs for
//! (`SessionExecutor::agent`, as `from_spec` sets it). An input naming `continues` runs the
//! executor `continuing` a predecessor of that id and of the input's commission, the session a
//! host would hand it ([`predecessor`]); the executor records the id it was given.
//!
//! What is published is what the executor recorded through its generated behaviour: the
//! `SessionChangeProposed` event it returned and the `AgentSessions` rows it holds. The executor
//! reads the change from the copy, in path order, so the event's `files` are the input's in path
//! order.
//!
//! `ProposeSessionChange` drives `ClaudeCode` only and `RecordFailedSession` `ClaudeCode` and
//! `Codex`: the kinds the synthesized suite names for each. Any other kind would need its own
//! scripted records, and the target reports it unsupported instead of answering for it.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
};
use b10x_loom_commission::ports::executor::AgentExecutor as _;
use b10x_loom_session_executor::metaharness::protocol::ProcessTermination;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessPoll, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::agent::{AgentId, ToolPolicy};
use b10x_loom_session_executor::model::primitives::Uuid as SessionUuid;
use b10x_loom_session_executor::model::session::{
    AgentSessionState, AgentSessions, ChangedFile, ChildEnd, CommissionId as SessionCommissionId,
    FailureRefusalReason, FrontierId as SessionFrontierId, RecordFailedSessionOutcome,
    SessionFailure, SessionId, SessionKind,
};
use b10x_loom_session_executor::{
    CredentialSource, Kind, ProcessRunner, SessionExecutor, ToolPolicyDefaults as _,
    failure_refusal,
};
use ess_conformance::target::{SemanticCommandResult, TargetError};
use ess_primitives::ids::CorrelationId;

use crate::codec::{self, Input};
use crate::{Live, event, took};

pub(crate) const PROPOSE_SESSION_CHANGE: &str = "loom.session.ProposeSessionChange";
pub(crate) const SESSION_CHANGE_PROPOSED: &str = "loom.session.SessionChangeProposed";
pub(crate) const AGENT_SESSIONS: &str = "loom.session.AgentSessions";
pub(crate) const RECORD_FAILED_SESSION: &str = "loom.session.RecordFailedSession";
pub(crate) const SESSION_FAILED: &str = "loom.session.SessionFailed";
const FAILURE_REFUSED: &str = "loom.session.FailureRefused";

/// The action a failed session's frontier offers; the session names none.
const FAILED_ACTION: &str = "conformance.write";

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
            "cwd": plan.cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow, "mcp_servers": [],
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
    executor.agent = decoded.agent.clone().map(AgentId);
    if let Some(continues) = &decoded.continues {
        executor = executor.continuing(&predecessor(continues, &decoded));
    }

    let outcome = run(&executor, &decoded, &decoded.action);
    if let ExecutorOutcome::Suspended(suspended) = &outcome {
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
            .with("continues", optional_id(proposed.continues.as_ref()))
            .with("action", codec::text_node(&proposed.action))
            .with("files", codec::changed_files_node(&proposed.files)),
        correlation,
    );
    Ok(Some(
        took(PROPOSE_SESSION_CHANGE, "proposed").emitting(published),
    ))
}

/// One call of `executor` for the input's commission, over a frontier of the input's id offering
/// `action`; its outcome.
fn run(executor: &SessionExecutor, decoded: &Decoded, action: &str) -> ExecutorOutcome {
    let case = CaseId("case".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(Uuid(decoded.commission_id.clone())),
        agent_revision_id: AgentRevisionId(Uuid(AGENT_REVISION.to_owned())),
        case_id: case.clone(),
        principal: PrincipalId("conformance".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid(decoded.frontier_id.clone())),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: action.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    });
    executor.run_with_usage(&commission, &frontier).outcome
}

/// A scripted Claude Code or Codex session that opens, its own session id `vendor`'s when there
/// is one, and ends as `failure` says without a reply or a terminal record: its exit status or
/// signal, or none measured, and `failure`'s stderr tail on its stderr, which metaharness hands on
/// because the child left no terminal record.
struct Failing {
    kind: Kind,
    failure: SessionFailure,
    vendor: Option<String>,
}

impl Failing {
    /// The session's opening record: Claude Code's `system` `init`, or the `session_meta` of a
    /// Codex rollout (which always names its id: [`record_failed_session`] drives none without).
    fn opening(&self, cwd: &Path) -> serde_json::Value {
        if self.kind == Kind::Codex {
            let id = self.vendor.clone().unwrap_or_default();
            return serde_json::json!({
                "timestamp": "2026-08-22T10:00:00.000Z", "type": "session_meta",
                "payload": {"id": id, "session_id": id, "cli_version": "0.153.4",
                            "cwd": cwd.display().to_string(), "originator": "codex_exec",
                            "model_provider": "openai"},
            });
        }
        let mut init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": cwd.display().to_string(), "tools": ToolPolicy::claude_default().allow, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default",
        });
        if let Some(vendor) = &self.vendor {
            init["session_id"] = serde_json::Value::String(vendor.clone());
        }
        init
    }
}

impl ProcessRunner for Failing {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let opening = self.opening(plan.cwd);
        let said = self.failure.stderr_tail.clone().unwrap_or_default();
        let inner = ScriptedRunner::of_lines([opening.to_string()], ScriptedLog::new())
            .saying_on_stderr(said)
            .start(plan)?;
        let code = |code: Option<i64>| {
            code.and_then(|code| i32::try_from(code).ok())
                .ok_or_else(|| std::io::Error::other("an end without a status this target makes"))
        };
        let termination = match self.failure.end {
            ChildEnd::Exited => ProcessTermination::Exited {
                code: code(self.failure.code)?,
            },
            ChildEnd::Signaled => ProcessTermination::Signaled {
                signal: code(self.failure.code)?,
            },
            ChildEnd::NotMeasured => ProcessTermination::Unknown,
        };
        Ok(Box::new(Ended { inner, termination }))
    }
}

/// The scripted child, ended as [`Failing`] says.
struct Ended {
    inner: Box<dyn HarnessProcess>,
    termination: ProcessTermination,
}

impl HarnessProcess for Ended {
    fn next_line(&mut self) -> std::io::Result<Option<String>> {
        self.inner.next_line()
    }

    fn poll_line(&mut self) -> std::io::Result<ProcessPoll> {
        self.inner.poll_line()
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.inner.write_line(line)
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.inner.kill()
    }

    fn wait(&mut self) -> std::io::Result<Option<i32>> {
        Ok(match self.termination {
            ProcessTermination::Exited { code } => Some(code),
            ProcessTermination::Signaled { .. } | ProcessTermination::Unknown => None,
        })
    }

    fn termination(&self) -> ProcessTermination {
        self.termination
    }

    fn stderr(&self) -> String {
        self.inner.stderr()
    }
}

/// The refusal `RecordFailedSession` answered, as `loom.session.FailureRefused`, `reason` in the
/// enum's spelling.
fn failure_refused(refused: &RecordFailedSessionOutcome) -> SemanticCommandResult {
    let (branch, reason) = match refused {
        RecordFailedSessionOutcome::UnmeasuredWithCode { error } => ("unmeasured-with-code", error),
        RecordFailedSessionOutcome::MeasuredWithoutCode { error } => {
            ("measured-without-code", error)
        }
        RecordFailedSessionOutcome::Failed { .. } => {
            unreachable!("`failure_refusal` answers a refusal only")
        }
    };
    let spelling = match reason.reason {
        FailureRefusalReason::UnmeasuredWithCode => "UnmeasuredWithCode",
        FailureRefusalReason::MeasuredWithoutCode => "MeasuredWithoutCode",
    };
    took(RECORD_FAILED_SESSION, branch).with_error(crate::error(FAILURE_REFUSED).with(
        "reason",
        ess_primitives::node::Node::Text(spelling.to_owned()),
    ))
}

/// `RecordFailedSession`: a failure no harness child ends with is refused as the executor's
/// record refuses it ([`failure_refusal`]); any other, a scripted session of the input's kind
/// that starts and fails as the input's `failure` says, run by the executor, and what it
/// recorded, the record's reason the executor's own. A status outside what a process reports, a
/// `Harness` session, or a `Codex` one naming no id is unsupported.
pub(crate) fn record_failed_session(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(decoded) = decode_failed(input) else {
        return Ok(None);
    };
    let Some(failure) = codec::session_failure(input, "failure") else {
        return Ok(None);
    };
    if let Some(refused) = failure_refusal(&failure) {
        return Ok(Some(failure_refused(&refused)));
    }
    let kind = match (decoded.kind, &decoded.vendor_session_id) {
        (SessionKind::ClaudeCode, _) => Kind::Claude,
        (SessionKind::Codex, Some(_)) => Kind::Codex,
        (kind, _) => {
            return Err(TargetError::unsupported(
                format!("a `{kind:?}` session"),
                "this target scripts Claude Code sessions and Codex sessions naming their id, \
                 the kinds the suite names",
            ));
        }
    };
    if failure
        .code
        .is_some_and(|code| i32::try_from(code).is_err())
    {
        return Err(TargetError::unsupported(
            format!("the status {:?}", failure.code),
            "a process reports a status or a signal of 32 bits",
        ));
    }
    let invocation = Invocation::make(live.sequence)?;
    let session_id = decoded.session_id.clone();
    let mut executor = SessionExecutor::new(invocation.workspace(), Vec::new(), "fail")
        .of_kind(kind)
        .with_runner(Box::new(Failing {
            kind,
            failure,
            vendor: decoded.vendor_session_id.clone(),
        }))
        .with_session_ids(move || session_id.clone());
    executor.credentials = CredentialSource::None;
    executor.scratch = invocation.0.join("scratch");
    executor.agent = decoded.agent.clone().map(AgentId);
    if let Some(continues) = &decoded.continues {
        executor = executor.continuing(&predecessor(continues, &decoded));
    }

    if !matches!(
        run(&executor, &decoded, FAILED_ACTION),
        ExecutorOutcome::Suspended(_)
    ) {
        return Err(unavailable(
            "the scripted session that fails did not suspend the step".to_owned(),
        ));
    }
    let Some(failed) = executor.failed().pop() else {
        return Err(unavailable(format!(
            "the executor recorded no failed session: {:?}",
            executor.last_report().and_then(|report| report.error)
        )));
    };
    live.sessions.extend(executor.sessions());
    let published = live.publish(
        event(SESSION_FAILED)
            .with("session_id", codec::id(&failed.session_id.0))
            .with("commission_id", codec::id(&failed.commission_id.0))
            .with("frontier_id", codec::id(&failed.frontier_id.0))
            .with("continues", optional_id(failed.continues.as_ref()))
            .with("failure", codec::session_failure_node(&failed.failure)),
        correlation,
    );
    Ok(Some(
        took(RECORD_FAILED_SESSION, "failed").emitting(published),
    ))
}

/// The input of one `RecordFailedSession` but its `failure`: no action and no files.
fn decode_failed(input: &Input) -> Option<Decoded> {
    Some(Decoded {
        session_id: SessionId(codec::uuid(input, "session_id")?),
        kind: codec::session_kind(input, "kind")?,
        commission_id: codec::uuid(input, "commission_id")?.0,
        frontier_id: codec::uuid(input, "frontier_id")?.0,
        vendor_session_id: codec::optional_text(input, "vendor_session_id")?,
        agent: codec::optional_text(input, "agent")?,
        continues: codec::optional_uuid(input, "continues")?.map(SessionId),
        action: String::new(),
        files: Vec::new(),
    })
}

/// The input of one `ProposeSessionChange`.
struct Decoded {
    session_id: SessionId,
    kind: SessionKind,
    commission_id: String,
    frontier_id: String,
    vendor_session_id: Option<String>,
    continues: Option<SessionId>,
    agent: Option<String>,
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
        agent: codec::optional_text(input, "agent")?,
        continues: codec::optional_uuid(input, "continues")?.map(SessionId),
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
        (
            "agent".to_owned(),
            held.agent
                .as_ref()
                .map_or(ess_primitives::node::Node::Null, |agent| {
                    codec::text_node(&agent.0)
                }),
        ),
        ("continues".to_owned(), optional_id(held.continues.as_ref())),
        ("action".to_owned(), codec::text_node(&held.action)),
        ("files".to_owned(), codec::changed_files_node(&held.files)),
        (
            "failure".to_owned(),
            held.failure.as_ref().map_or(
                ess_primitives::node::Node::Null,
                codec::session_failure_node,
            ),
        ),
        ("state".to_owned(), codec::agent_session_state(held.state)),
    ])
}

/// The session `continues` names, as a host holding it would hand it to `continuing`: a row of
/// the input's commission and frontier, its other fields the input's. The executor reads only its
/// id and its commission.
fn predecessor(continues: &SessionId, decoded: &Decoded) -> AgentSessions {
    AgentSessions {
        session_id: continues.clone(),
        kind: decoded.kind,
        commission_id: SessionCommissionId(SessionUuid(decoded.commission_id.clone())),
        frontier_id: SessionFrontierId(SessionUuid(decoded.frontier_id.clone())),
        vendor_session_id: None,
        agent: None,
        continues: None,
        action: decoded.action.clone(),
        files: Vec::new(),
        failure: None,
        state: AgentSessionState::Ended,
    }
}

/// An `Optional<loom.session.SessionId>` as a suite value, an absent one null.
fn optional_id(id: Option<&SessionId>) -> ess_primitives::node::Node {
    id.map_or(ess_primitives::node::Node::Null, |id| codec::id(&id.0))
}
