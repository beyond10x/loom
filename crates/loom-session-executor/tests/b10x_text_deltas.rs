//! A b10x-harness session streams its reply in `text-delta` events, each a fragment of the text:
//! the executor reads a turn's deltas as one text, keeping the line breaks they carry, and starts
//! each turn on a new line. The shape is that of a recorded trial whose last turn streamed the
//! action line in 31 deltas of a few bytes each, with no newline.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called and nothing reaches the network.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorStep, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, Proposal, SessionExecutor,
};

const WRITE: &str = "tests.write";
const SUMMARY: &str = "add tests for remove: numbers move up, an unknown number exits 1";
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
            "session-executor-b10x-deltas-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A b10x-harness executor over the tree whose session prints `lines`, its loop pointed at a
    /// closed local port with a scripted token file.
    fn executor(&self, lines: Vec<serde_json::Value>) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
                .with_runner(Box::new(Session {
                    lines,
                    log: ScriptedLog::new(),
                }));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.0.join("scratch");
        let token = self.0.join("token.json");
        std::fs::write(&token, "{\"token\": \"scripted\"}\n").expect("a token file");
        executor.endpoint = Some(Endpoint {
            base_url: "http://127.0.0.1:9/v1".to_owned(),
            wire: "openai-responses".to_owned(),
            model: "synthetic".to_owned(),
            token_file: token,
            token_pointer: Some("/token".to_owned()),
        });
        executor
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A scripted b10x-harness session: started, it edits `src/lib.rs` in its copy and prints its
/// lines.
struct Session {
    lines: Vec<serde_json::Value>,
    log: ScriptedLog,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        std::fs::write(plan.cwd.join("src/lib.rs"), EDITED)?;
        let steps = self
            .lines
            .iter()
            .map(|line| ScriptStep::line(line.to_string()))
            .collect();
        ScriptedRunner::new(steps, self.log.clone()).start(plan)
    }
}

fn action_line() -> String {
    serde_json::json!({"action": WRITE, "summary": SUMMARY}).to_string()
}

/// `text` cut into deltas of at most `size` bytes, on character boundaries.
fn deltas(text: &str, size: usize) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut piece = String::new();
    for c in text.chars() {
        if piece.len() + c.len_utf8() > size {
            out.push(serde_json::json!({"kind": "text-delta", "text": std::mem::take(&mut piece)}));
        }
        piece.push(c);
    }
    if !piece.is_empty() {
        out.push(serde_json::json!({"kind": "text-delta", "text": piece}));
    }
    out
}

fn turn(n: u64) -> serde_json::Value {
    serde_json::json!({"kind": "turn-started", "turn": n})
}

fn usage() -> serde_json::Value {
    serde_json::json!({"kind": "usage", "model": "synthetic", "input_tokens": 100,
                       "output_tokens": 20, "cached_input_tokens": 0})
}

/// A session's lines: started, then `turns` (each its turn-started line, its text deltas and one
/// usage line), finished.
fn session(turns: Vec<Vec<serde_json::Value>>) -> Vec<serde_json::Value> {
    let mut lines = vec![
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
    ];
    for (n, text) in (1..).zip(turns) {
        lines.push(turn(n));
        lines.extend(text);
        lines.push(usage());
    }
    lines.push(serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}));
    lines
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xd1)),
        agent_revision_id: AgentRevisionId(uuid(0xd2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-b10x-deltas".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf2)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: WRITE.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-b10x-deltas".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

/// The action the step proposed and the summary it carries, or a panic naming why there is no
/// proposal.
fn proposed(executor: &SessionExecutor) -> (String, String) {
    let step = step_of(executor);
    let proposal = match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments"),
        None => panic!(
            "no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    };
    (proposal.action, step.summary.unwrap_or_default())
}

#[test]
fn an_action_line_split_over_deltas_is_read_as_one_line() {
    let scratch = Scratch::new("split");
    let line = action_line();
    let pieces = deltas(&line, 4);
    assert!(pieces.len() > 20, "the line is cut into many deltas");
    let executor = scratch.executor(session(vec![Vec::new(), Vec::new(), pieces]));

    assert_eq!(proposed(&executor), (WRITE.to_owned(), SUMMARY.to_owned()));
}

#[test]
fn a_turns_own_line_breaks_are_kept() {
    let scratch = Scratch::new("breaks");
    let text = format!(
        "The tests are written.\nThey cover remove 0 and remove 4.\n{}",
        action_line()
    );
    let executor = scratch.executor(session(vec![deltas(&text, 7)]));

    assert_eq!(proposed(&executor), (WRITE.to_owned(), SUMMARY.to_owned()));
}

#[test]
fn turns_are_separated_by_a_line_break() {
    let scratch = Scratch::new("turns");
    let executor = scratch.executor(session(vec![
        deltas("I will read todo.py first", 5),
        deltas(&action_line(), 9),
    ]));

    assert_eq!(proposed(&executor), (WRITE.to_owned(), SUMMARY.to_owned()));
}
