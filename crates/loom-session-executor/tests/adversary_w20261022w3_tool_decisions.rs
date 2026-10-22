//! Adversary cases for `story:tool-decisions-read-back` (wave 2026-10-22-w3).
//!
//! Most cases drive `tool_decisions` directly over hand-written `metaharness.event/1` lines, the
//! bytes a session's transcript holds before any vendor record replaces it. One case drives a
//! scripted b10x-harness session (metaharness's `ScriptedRunner`) through `run_with_usage`. No
//! harness program runs, no model is called and nothing reaches the network.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::{CallDecision, DecidedBy, ToolDecision};
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, SessionExecutor, tool_decisions,
};
use serde_json::json;

const EDIT: &str = "implementation.edit";

/// A record of event lines, each framed as `metaharness.event/1` with its own `seq`.
struct Record {
    lines: Vec<String>,
}

impl Record {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }

    fn push(&mut self, mut event: serde_json::Value) -> &mut Self {
        let seq = self.lines.len() as u64 + 1;
        event["format"] = json!("metaharness.event/1");
        event["seq"] = json!(seq);
        event["run"] = json!("run-adversary");
        self.lines.push(event.to_string());
        self
    }

    fn requested(&mut self, call: &str, name: &str) -> &mut Self {
        self.push(json!({
            "event": "tool.requested", "call_id": call, "name": name, "input": {},
            "decision_required": true, "deadline_ms": null, "seam": "hook",
        }))
    }

    fn decided(&mut self, call: &str, decision: serde_json::Value, by: &str) -> &mut Self {
        self.push(json!({
            "event": "tool.decided", "call_id": call, "decision": decision,
            "decided_by": by, "seam": "hook", "latency_ms": null,
        }))
    }

    /// `session.ended` with the census `[allowed, denied, replaced, abstained]` and `by_decider`.
    fn ended(&mut self, counts: [u32; 4], by_decider: serde_json::Value) -> &mut Self {
        let total: u32 = counts.iter().sum();
        self.push(json!({
            "event": "session.ended", "is_error": false, "subtype": "success",
            "stop_reason": null, "terminal_reason": null, "api_error_status": null,
            "num_turns": 1, "duration_ms": null, "duration_api_ms": null, "ttft_ms": null,
            "time_to_request_ms": null, "total_cost_usd": null, "permission_denials": null,
            "subagents_spawned": null, "usage": null, "model_usage": null,
            "census": {
                "allowed": counts[0], "denied": counts[1], "replaced": counts[2],
                "abstained": counts[3], "by_seam": {"hook": total}, "by_decider": by_decider,
            },
        }))
    }

    fn bytes(&self) -> Vec<u8> {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text.into_bytes()
    }
}

fn allow() -> serde_json::Value {
    json!({"decision": "allow"})
}

fn deny(reason: &str) -> serde_json::Value {
    json!({"decision": "deny", "reason": reason})
}

fn decision(call: &str, tool: Option<&str>, decision: CallDecision, by: DecidedBy) -> ToolDecision {
    ToolDecision {
        call_id: call.to_owned(),
        tool: tool.map(str::to_owned),
        decision,
        reason: None,
        decided_by: by,
    }
}

/// Control and mutant catcher: a record whose census adds up reads every decision, each decider
/// and each decision kind mapped to its own name. The unit's tests cover only `Embedder` with
/// `Allow` and `Deny`; swapping any two of `Frame`, `Deadline`, `Adapter`, `Observe`, or
/// `Replace` and `Abstain`, in `tool_decisions` would leave them green.
#[test]
fn adversary_every_decider_and_decision_kind_maps_to_its_own_name() {
    let mut record = Record::new();
    record
        .requested("c1", "Read")
        .decided("c1", allow(), "frame")
        .requested("c2", "Bash")
        .decided("c2", deny("too slow"), "deadline")
        .requested("c3", "Write")
        .decided("c3", deny("uncovered"), "adapter")
        .requested("c4", "Grep")
        .decided("c4", allow(), "observe")
        .requested("c5", "Edit")
        .decided(
            "c5",
            json!({"decision": "replace", "input": {}}),
            "embedder",
        )
        .requested("c6", "Glob")
        .decided("c6", json!({"decision": "abstain"}), "embedder")
        .ended(
            [2, 2, 1, 1],
            json!({"frame": 1, "deadline": 1, "adapter": 1, "observe": 1, "embedder": 2}),
        );

    let decisions = tool_decisions(&record.bytes()).expect("a census that adds up is known");

    let mut denied_deadline = decision("c2", Some("Bash"), CallDecision::Deny, DecidedBy::Deadline);
    denied_deadline.reason = Some("too slow".to_owned());
    let mut denied_adapter = decision("c3", Some("Write"), CallDecision::Deny, DecidedBy::Adapter);
    denied_adapter.reason = Some("uncovered".to_owned());
    assert_eq!(
        decisions,
        vec![
            decision("c1", Some("Read"), CallDecision::Allow, DecidedBy::Frame),
            denied_deadline,
            denied_adapter,
            decision("c4", Some("Grep"), CallDecision::Allow, DecidedBy::Observe),
            decision(
                "c5",
                Some("Edit"),
                CallDecision::Replace,
                DecidedBy::Embedder
            ),
            decision(
                "c6",
                Some("Glob"),
                CallDecision::Abstain,
                DecidedBy::Embedder
            ),
        ]
    );
}

/// A census of no decision and a record holding none is a known none, an empty list.
#[test]
fn adversary_a_zero_census_with_no_decision_is_a_known_none() {
    let mut record = Record::new();
    record.ended([0, 0, 0, 0], json!({}));
    assert_eq!(tool_decisions(&record.bytes()), Some(Vec::new()));
}

/// The census says two calls were allowed and none denied; the record holds one allow and one
/// deny. The totals agree (2 = 2), the decisions do not: the list does not "add up to" the
/// census, so it is unknown.
#[test]
fn adversary_a_census_whose_allow_deny_split_disagrees_is_unknown() {
    let mut record = Record::new();
    record
        .requested("c1", "Bash")
        .decided("c1", allow(), "embedder")
        .requested("c2", "Bash")
        .decided("c2", deny("no"), "embedder")
        .ended([2, 0, 0, 0], json!({"embedder": 2}));

    assert_eq!(
        tool_decisions(&record.bytes()),
        None,
        "a census of 2 allowed and 0 denied does not add up to an allow and a deny"
    );
}

/// The census says the one decision was the frame's; the record says the embedder's. Who decided
/// is the field this story exists to report, and here the record and its census disagree on it.
#[test]
fn adversary_a_census_whose_decider_disagrees_is_unknown() {
    let mut record = Record::new();
    record
        .requested("c1", "Bash")
        .decided("c1", allow(), "embedder")
        .ended([1, 0, 0, 0], json!({"frame": 1}));

    assert_eq!(
        tool_decisions(&record.bytes()),
        None,
        "a census naming the frame does not add up to a decision by the embedder"
    );
}

/// metaharness's codex hook names the tool `""` when the hook's input is not JSON
/// (`metaharness-codex/src/bridge.rs`, `name: ""`). An empty name names no tool, and
/// `loom.session.ToolDecision.tool` is absent when the record names none.
#[test]
fn adversary_an_empty_tool_name_is_no_tool() {
    let mut record = Record::new();
    record
        .requested("c1", "")
        .decided("c1", allow(), "embedder")
        .ended([1, 0, 0, 0], json!({"embedder": 1}));

    let decisions = tool_decisions(&record.bytes()).expect("known");
    assert_eq!(
        decisions,
        vec![decision(
            "c1",
            None,
            CallDecision::Allow,
            DecidedBy::Embedder
        )]
    );
}

/// A large record: 20 000 decided calls read back in order, all of them.
#[test]
fn adversary_a_large_record_reads_every_decision_in_order() {
    const CALLS: u32 = 20_000;
    let mut record = Record::new();
    for n in 0..CALLS {
        let call = format!("c{n}");
        record
            .requested(&call, "Bash")
            .decided(&call, allow(), "embedder");
    }
    record.ended([CALLS, 0, 0, 0], json!({"embedder": CALLS}));

    let decisions = tool_decisions(&record.bytes()).expect("known");
    assert_eq!(decisions.len(), CALLS as usize);
    assert_eq!(decisions[0].call_id, "c0");
    assert_eq!(decisions[19_999].call_id, "c19999");
}

// A scripted b10x-harness session.

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adv-w3-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Session(fn(&Path) -> Vec<ScriptStep>);

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        ScriptedRunner::new((self.0)(plan.cwd), ScriptedLog::new()).start(plan)
    }
}

/// A b10x-harness session making two tool calls, the second denied by the loop's own approver,
/// then naming its action and finishing.
fn b10x_with_calls(_cwd: &Path) -> Vec<ScriptStep> {
    let action = json!({"action": EDIT, "summary": "looked"}).to_string();
    [
        json!({"kind": "started", "model": "synthetic", "published_tools": [],
               "credential_source": "none"}),
        json!({"kind": "tool-requested", "call_id": "t-1", "name": "workspace_read",
               "arguments": {"path": "src/lib.rs"}}),
        json!({"kind": "tool-completed", "call_id": "t-1", "failed": false}),
        json!({"kind": "tool-requested", "call_id": "t-2", "name": "run",
               "arguments": {"argv": ["cat", "~/.ssh/id_ed25519"]}}),
        json!({"kind": "approval-resolved", "call_id": "t-2", "approved": false}),
        json!({"kind": "tool-completed", "call_id": "t-2", "failed": true}),
        json!({"kind": "text-delta", "text": action}),
        json!({"kind": "cost", "micro_usd": 10}),
        json!({"kind": "finished", "stop": {"kind": "completed"}}),
    ]
    .into_iter()
    .map(|line| ScriptStep::line(line.to_string()))
    .collect()
}

/// A b10x-harness session runs in observe mode and every call it makes is a `tool.requested` on
/// no seam, which metaharness never decides, so its census is all zeros. The session made two
/// calls and the loop's approver denied one. The report must not say the decisions are known
/// and there were none: `Some(vec![])` reads as "this session's calls were all accounted for,
/// and there were no calls", which is false here.
#[test]
fn adversary_a_b10x_session_with_calls_does_not_report_a_known_none() {
    let scratch = Scratch::new("b10x");
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let mut executor = SessionExecutor::new(
        scratch.0.join("tree"),
        vec!["src/lib.rs".to_owned()],
        "fix a",
    )
    .of_kind(Kind::B10x)
    .with_runner(Box::new(Session(b10x_with_calls)));
    executor.credentials = CredentialSource::None;
    executor.scratch = scratch.0.join("scratch");
    executor.endpoint = Some(Endpoint {
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        wire: "openai-responses".to_owned(),
        model: "synthetic".to_owned(),
        token_file: token,
        token_pointer: Some("/token".to_owned()),
    });

    let case = CaseId("case-adv-w3".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adv-w3".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    });
    let _ = executor.run_with_usage(&commission, &frontier);

    let report = executor.last_report().expect("a report");
    assert!(report.started, "the session ran: {:?}", report.error);
    let transcript = String::from_utf8_lossy(&report.transcript);
    assert!(
        transcript.contains("t-2"),
        "the record holds the denied call: {transcript}"
    );
    assert_ne!(
        report.tool_decisions,
        Some(Vec::new()),
        "a session that made two calls, one denied, reports a known empty list of decisions"
    );
}
