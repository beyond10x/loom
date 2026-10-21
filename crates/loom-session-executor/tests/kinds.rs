//! A Codex session and a b10x-harness session driven through `run_with_usage`, as the Claude Code
//! ones are in `proposes.rs`: the kind each is recorded as, the calls the executor decides for
//! Codex and leaves alone for b10x-harness, the home a Codex child is given, what each is charged,
//! and the hermetic floor that holds a Codex session back.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, the lines each vendor prints:
//! a Codex rollout and its hook requests, b10x-harness's own event lines): no harness program
//! runs, no model is called and nothing reaches the network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::SessionKind;
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, HOME_VARIABLES, Kind, ProcessRunner, Proposal, ProposedFile,
    SessionExecutor,
};

const EDIT: &str = "implementation.edit";
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-kinds-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, kind: Kind, runner: Session) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(kind)
                .with_runner(Box::new(runner));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.0.join("scratch");
        executor
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a [`Session`] saw when it was started: its working directory and its environment.
#[derive(Default)]
struct Seen {
    cwd: Option<PathBuf>,
    env: BTreeMap<String, String>,
}

/// A scripted session: started, it keeps what it was started with, edits `src/lib.rs` in its
/// working directory, the copy, and prints the steps `script` makes of that directory, logging
/// every line written to it in `log`.
struct Session {
    script: fn(&Path) -> Vec<ScriptStep>,
    seen: Rc<RefCell<Seen>>,
    log: ScriptedLog,
}

impl Session {
    fn new(script: fn(&Path) -> Vec<ScriptStep>) -> Self {
        Self {
            script,
            seen: Rc::default(),
            log: ScriptedLog::new(),
        }
    }

    fn handles(&self) -> (Rc<RefCell<Seen>>, ScriptedLog) {
        (Rc::clone(&self.seen), self.log.clone())
    }
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        *self.seen.borrow_mut() = Seen {
            cwd: Some(plan.cwd.to_path_buf()),
            env: plan.env.clone(),
        };
        std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
        ScriptedRunner::new((self.script)(plan.cwd), self.log.clone()).start(plan)
    }
}

/// The JSON line a session ends with, naming its action.
fn action_line() -> String {
    serde_json::json!({"action": EDIT, "summary": "fixed a"}).to_string()
}

/// One Codex `PreToolUse` hook request, as metaharness's codex hook relays it to the seam,
/// waiting under the rendezvous name `key` for the decision.
fn hook(key: &str, tool: &str, input: &serde_json::Value, cwd: &Path) -> [ScriptStep; 2] {
    let raw = serde_json::json!({
        "session_id": "codex-1", "turn_id": "t1", "cwd": cwd.display().to_string(),
        "hook_event_name": "PreToolUse", "model": "scripted", "permission_mode": "default",
        "tool_name": tool, "tool_input": input, "tool_use_id": key,
    });
    let line = serde_json::json!({
        "metaharness.codex/1": "hook_request", "key": key, "input": raw,
    });
    [
        ScriptStep::line(line.to_string()),
        ScriptStep::awaiting(key),
    ]
}

/// A Codex rollout written by codex-cli `version`: the session's opening record, four calls the
/// executor decides, the turn's token count (100 input tokens, 40 of them cached, 20 output) and
/// its completion, whose last message names the action.
fn codex_rollout(version: &str, cwd: &Path) -> Vec<ScriptStep> {
    let meta = serde_json::json!({
        "timestamp": "2026-08-22T10:00:00.000Z", "type": "session_meta",
        "payload": {"id": "codex-1", "session_id": "codex-1", "cli_version": version,
                    "cwd": cwd.display().to_string(), "originator": "codex_exec",
                    "model_provider": "openai"},
    });
    let started = serde_json::json!({
        "timestamp": "2026-08-22T10:00:01.000Z", "type": "event_msg",
        "payload": {"type": "task_started", "turn_id": "t1"},
    });
    let tokens = serde_json::json!({
        "timestamp": "2026-08-22T10:00:04.000Z", "type": "event_msg",
        "payload": {"type": "token_count", "info": {"total_token_usage": {
            "input_tokens": 100, "cached_input_tokens": 40, "cache_write_input_tokens": 10,
            "output_tokens": 20, "reasoning_output_tokens": 6, "total_tokens": 120}}},
    });
    let complete = serde_json::json!({
        "timestamp": "2026-08-22T10:00:05.000Z", "type": "event_msg",
        "payload": {"type": "task_complete", "turn_id": "t1", "duration_ms": 4200,
                    "error": null, "last_agent_message": action_line()},
    });
    let mut steps = vec![
        ScriptStep::line(meta.to_string()),
        ScriptStep::line(started.to_string()),
    ];
    steps.extend(hook(
        "call-ls",
        "Bash",
        &serde_json::json!({"command": "ls src"}),
        cwd,
    ));
    steps.extend(hook(
        "call-key",
        "Bash",
        &serde_json::json!({"command": "cat ~/.ssh/id_ed25519"}),
        cwd,
    ));
    steps.extend(hook(
        "call-patch",
        "apply_patch",
        &serde_json::json!({"command": "*** Begin Patch\n*** Update File: src/lib.rs\n*** End Patch\n"}),
        cwd,
    ));
    steps.extend(hook(
        "call-git",
        "apply_patch",
        &serde_json::json!({"command": "*** Begin Patch\n*** Update File: .git/config\n*** End Patch\n"}),
        cwd,
    ));
    steps.push(ScriptStep::line(tokens.to_string()));
    steps.push(ScriptStep::line(complete.to_string()));
    steps
}

fn pinned_codex(cwd: &Path) -> Vec<ScriptStep> {
    codex_rollout("0.153.4", cwd)
}

fn unpinned_codex(cwd: &Path) -> Vec<ScriptStep> {
    codex_rollout("0.999.0", cwd)
}

/// A b10x-harness session's own lines: started, the reply naming the action, one `usage` line
/// per call as `(input, output, cached)`, its cost of 1234 micro-USD, finished.
fn b10x_session(calls: &[(u64, u64, u64)]) -> Vec<ScriptStep> {
    let mut lines = vec![
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
        serde_json::json!({"kind": "text-delta", "text": action_line()}),
    ];
    lines.extend(calls.iter().map(|(input, output, cached)| {
        serde_json::json!({"kind": "usage", "model": "synthetic", "input_tokens": input,
                           "output_tokens": output, "cached_input_tokens": cached})
    }));
    lines.push(serde_json::json!({"kind": "cost", "micro_usd": 1234}));
    lines.push(serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}));
    lines
        .into_iter()
        .map(|line| ScriptStep::line(line.to_string()))
        .collect()
}

/// One call: 100 input tokens, none cached, 20 output.
fn b10x_one_call(_cwd: &Path) -> Vec<ScriptStep> {
    b10x_session(&[(100, 20, 0)])
}

/// Two calls: 100 input tokens of which 40 cached and 20 output, then 50 input, none cached,
/// and 10 output.
fn b10x_two_calls(_cwd: &Path) -> Vec<ScriptStep> {
    b10x_session(&[(100, 20, 40), (50, 10, 0)])
}

/// A b10x-harness executor over `scratch`, its loop pointed at a closed local port on `wire`,
/// started by `session`.
fn b10x_executor(scratch: &Scratch, wire: &str, session: Session) -> SessionExecutor {
    let token = scratch.0.join("token.json");
    std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
    let mut executor = scratch.executor(Kind::B10x, session);
    executor.endpoint = Some(Endpoint {
        base_url: "http://127.0.0.1:9/v1".to_owned(),
        wire: wire.to_owned(),
        model: "synthetic".to_owned(),
        token_file: token,
        token_pointer: Some("/token".to_owned()),
    });
    executor
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-kinds".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-kinds".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// The proposal `step` carries, else a panic naming why the session proposed nothing.
fn proposal(step: &ExecutorStep, executor: &SessionExecutor) -> Proposal {
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments"),
        None => panic!(
            "no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor
                .last_report()
                .map(|report| (report.error, report.rows))
        ),
    }
}

/// Each decision written to the child, as the hook's `permissionDecision` by rendezvous name.
fn decisions(log: &ScriptedLog) -> BTreeMap<String, String> {
    log.written()
        .iter()
        .map(|line| {
            let doc: serde_json::Value = serde_json::from_str(line).expect("a JSON decision");
            (
                doc["call_id"].as_str().expect("a call id").to_owned(),
                doc["response"]["hookSpecificOutput"]["permissionDecision"]
                    .as_str()
                    .expect("a permission decision")
                    .to_owned(),
            )
        })
        .collect()
}

/// A Codex session at the pinned version is recorded as `Codex` and proposes its change; the
/// executor decides each of its calls (a command naming the user's home and a patch to git's
/// state are denied, a listing and a patch to a copied file allowed); its child's home and XDG
/// directories are its own, beside the copy; and it is charged the input it was not served from
/// the cache.
#[test]
fn a_codex_session_is_decided_homed_and_charged_its_uncached_input() {
    let scratch = Scratch::new("codex");
    let session = Session::new(pinned_codex);
    let (seen, log) = session.handles();
    let executor = scratch.executor(Kind::Codex, session);

    let step = step_of(&executor);

    let proposal = proposal(&step, &executor);
    assert_eq!(proposal.action, EDIT);
    assert_eq!(
        proposal.files,
        vec![ProposedFile {
            path: "src/lib.rs".to_owned(),
            content: Some(EDITED.to_owned()),
        }]
    );
    let rows = executor.sessions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, SessionKind::Codex);
    assert_eq!(rows[0].vendor_session_id.as_deref(), Some("codex-1"));
    let decided = decisions(&log);
    assert_eq!(
        decided,
        BTreeMap::from(
            [
                ("call-ls", "allow"),
                ("call-key", "deny"),
                ("call-patch", "allow"),
                ("call-git", "deny"),
            ]
            .map(|(key, said)| (key.to_owned(), said.to_owned()))
        )
    );
    let seen = seen.borrow();
    let cwd = seen.cwd.clone().expect("the session was started");
    let home = cwd.with_file_name("home");
    for (name, under) in HOME_VARIABLES {
        let expected = if under.is_empty() {
            home.clone()
        } else {
            home.join(under)
        };
        assert_eq!(
            seen.env.get(name).map(PathBuf::from),
            Some(expected),
            "{name} in the child's environment"
        );
    }
    let usage = step.usage.expect("the step's usage");
    assert_eq!((usage.input_tokens, usage.output_tokens), (60, 20));
    assert_eq!(usage.duration_ms, 4200);
    assert_eq!(usage.cost_micro_usd, None, "a rollout prices nothing");
    let report = executor.last_report().expect("a report");
    assert_eq!(report.version.as_deref(), Some("0.153.4"));
    assert_eq!(report.cache_read, Some(40));
}

/// A Codex session written by a codex other than the pinned one proposes nothing: its H9 holds
/// the change back, the call suspends for `ExternalAvailability` naming it, and it is no record.
#[test]
fn a_codex_session_outside_the_pin_is_held_by_h9() {
    let scratch = Scratch::new("codex-unpinned");
    let executor = scratch.executor(Kind::Codex, Session::new(unpinned_codex));

    let step = step_of(&executor);

    let why = match &step.outcome {
        ExecutorOutcome::Suspended(suspended) => match &suspended.reason {
            SuspensionReason::ExternalAvailability(Value::Text(why)) => why.clone(),
            other => panic!("suspended for {other:?}"),
        },
        other => panic!("an unpinned codex was not held back: {other:?}"),
    };
    assert!(why.contains("H9"), "{why}");
    assert!(
        executor.sessions().is_empty(),
        "a held session is no record"
    );
    assert!(executor.proposed().is_empty());
}

/// A b10x-harness session is recorded as `Harness` and proposes its change; the executor decides
/// none of its calls (it observes them) and charges it the cost it stated and the tokens of its
/// one `usage` line, which metaharness 0.10.1's b10x adapter states as a `usage` event only.
#[test]
fn a_b10x_session_is_observed_and_charged_its_cost_and_tokens() {
    let scratch = Scratch::new("b10x");
    let session = Session::new(b10x_one_call);
    let (_, log) = session.handles();
    let executor = b10x_executor(&scratch, "openai-responses", session);

    let step = step_of(&executor);

    let proposal = proposal(&step, &executor);
    assert_eq!(proposal.action, EDIT);
    assert_eq!(
        proposal.files,
        vec![ProposedFile {
            path: "src/lib.rs".to_owned(),
            content: Some(EDITED.to_owned()),
        }]
    );
    let rows = executor.sessions();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, SessionKind::Harness);
    assert!(
        log.written().is_empty(),
        "no call of b10x-harness is decided"
    );
    let usage = step.usage.expect("the step's usage");
    assert_eq!(usage.cost_micro_usd, Some(1234));
    assert_eq!((usage.input_tokens, usage.output_tokens), (100, 20));
}

/// A b10x-harness session's calls are summed, each counted once: on the OpenAI Responses wire,
/// whose input figure holds the cached tokens, the input charged is the rest (150 less 40); on
/// the Anthropic Messages wire, whose input figure does not, it is the whole.
#[test]
fn a_b10x_sessions_calls_are_summed_less_the_cached_input_on_responses() {
    for (wire, input) in [("openai-responses", 110), ("anthropic-messages", 150)] {
        let scratch = Scratch::new(&format!("b10x-sum-{wire}"));
        let executor = b10x_executor(&scratch, wire, Session::new(b10x_two_calls));

        let step = step_of(&executor);

        proposal(&step, &executor);
        let usage = step.usage.expect("the step's usage");
        assert_eq!(
            (usage.input_tokens, usage.output_tokens),
            (input, 30),
            "{wire}"
        );
        let report = executor.last_report().expect("a report");
        assert_eq!(report.cache_read, Some(40), "{wire}");
    }
}
