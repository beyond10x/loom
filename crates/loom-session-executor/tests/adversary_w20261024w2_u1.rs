//! Adversary cases for story:b10x-text-deltas: a b10x-harness session's reply read from its
//! `text-delta` fragments, a turn's fragments joined with no separator and each turn on a new
//! line.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, b10x-harness's own event
//! lines): no harness program runs, no model is called, no credential file is read and nothing
//! reaches the network.

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
use serde_json::json;

const WRITE: &str = "tests.write";
const EDIT: &str = "implementation.edit";
const SUMMARY: &str = "add tests for remove";
const EDITED: &str = "pub fn a() -> i32 {\n    1\n}\n";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adversary-w20261024w2-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

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

fn action(name: &str) -> String {
    json!({"action": name, "summary": SUMMARY}).to_string()
}

fn deltas(text: &str, size: usize) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut piece = String::new();
    for c in text.chars() {
        if piece.len() + c.len_utf8() > size {
            out.push(json!({"kind": "text-delta", "text": std::mem::take(&mut piece)}));
        }
        piece.push(c);
    }
    if !piece.is_empty() {
        out.push(json!({"kind": "text-delta", "text": piece}));
    }
    out
}

fn delta(text: &str) -> serde_json::Value {
    json!({"kind": "text-delta", "text": text})
}

fn turn(n: u64) -> serde_json::Value {
    json!({"kind": "turn-started", "turn": n})
}

fn usage() -> serde_json::Value {
    json!({"kind": "usage", "model": "synthetic", "input_tokens": 100,
           "output_tokens": 20, "cached_input_tokens": 0})
}

/// The started line, then `body` as given, then the finished line.
fn session(body: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut lines = vec![json!({"kind": "started", "model": "synthetic",
                                "published_tools": [], "credential_source": "none"})];
    lines.extend(body);
    lines.push(json!({"kind": "finished", "stop": {"kind": "completed"}}));
    lines
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xd1)),
        agent_revision_id: AgentRevisionId(uuid(0xd2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-deltas".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    let admissible = |action: &str| FrontierAction {
        action: action.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    };
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf2)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![admissible(WRITE), admissible(EDIT)],
    })
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-adversary-deltas".to_owned());
    executor.run_with_usage(&commission(&case), &frontier(&case))
}

fn proposed(executor: &SessionExecutor) -> String {
    let step = step_of(executor);
    match Proposal::from_outcome(&step.outcome) {
        Some(read) => read.expect("a proposal's arguments").action,
        None => panic!(
            "no proposal: {:?}; the session reported {:?}",
            step.outcome,
            executor.last_report().map(|report| report.error)
        ),
    }
}

/// b10x-harness 0.18.0 retries a turn whose wire failed retriably after it streamed, and says
/// so with `turn-retried` (`harness-loop` `LoopEvent::TurnRetried`: "the turn's stream is to be
/// discarded"). The failed attempt's fragments must not run into the retry's action line.
#[test]
fn a_retried_turns_discarded_fragments_do_not_run_into_the_action_line() {
    let scratch = Scratch::new("retried");
    let line = action(WRITE);
    let mut body = vec![turn(1)];
    body.extend(deltas(&line[..20], 4));
    body.push(json!({"kind": "turn-retried", "turn": 1, "attempt": 1,
                     "reason": "the stream ended early"}));
    body.extend(deltas(&line, 4));
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

/// The same with a discarded attempt that streamed prose only, cut mid-line.
#[test]
fn a_retried_turns_discarded_prose_does_not_run_into_the_action_line() {
    let scratch = Scratch::new("retried-prose");
    let mut body = vec![turn(1)];
    body.extend(deltas("The tests are writ", 5));
    body.push(json!({"kind": "turn-retried", "turn": 1, "attempt": 1,
                     "reason": "the stream ended early"}));
    body.extend(deltas(&action(WRITE), 6));
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

/// A retried turn whose retry sends the action line in one fragment, as every b10x-harness test
/// before this story did: before the story each fragment was its own line, so this read.
#[test]
fn a_retried_turn_whose_retry_sends_the_action_line_whole_still_reads() {
    let scratch = Scratch::new("retried-whole");
    let body = vec![
        turn(1),
        delta("The tests are writ"),
        json!({"kind": "turn-retried", "turn": 1, "attempt": 1,
               "reason": "the stream ended early"}),
        delta(&action(WRITE)),
        usage(),
    ];
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

#[test]
fn fragments_before_any_turn_started_are_one_text() {
    let scratch = Scratch::new("no-turn");
    let mut body = deltas(&action(WRITE), 3);
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

#[test]
fn text_before_the_first_turn_does_not_run_into_the_first_turn() {
    let scratch = Scratch::new("pre-turn");
    let mut body = deltas("preamble without a newline", 5);
    body.push(turn(1));
    body.extend(deltas(&action(WRITE), 5));
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

#[test]
fn a_turn_with_tool_calls_then_a_turn_with_the_action_line() {
    let scratch = Scratch::new("tools");
    let mut body = vec![turn(1)];
    body.extend(deltas("Let me read the file", 4));
    body.push(json!({"kind": "tool-arguments-delta", "call_id": "c-1", "delta": "{}"}));
    body.push(usage());
    body.push(
        json!({"kind": "tool-requested", "call_id": "c-1", "name": "file_read",
                     "arguments": {"path": "src/lib.rs"}}),
    );
    body.push(json!({"kind": "tool-completed", "call_id": "c-1", "failed": false}));
    body.push(turn(2));
    body.extend(deltas(&action(WRITE), 7));
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

/// The last action line wins, as it did before: an earlier turn's action line followed by a
/// later turn of prose still reads the earlier one, and a later action line overrides it.
#[test]
fn the_last_action_line_across_turns_is_read() {
    let scratch = Scratch::new("earlier");
    let mut body = vec![turn(1)];
    body.extend(deltas(&action(EDIT), 4));
    body.push(usage());
    body.push(turn(2));
    body.extend(deltas("Done, nothing more to add", 4));
    body.push(usage());
    let executor = scratch.executor(session(body));
    assert_eq!(proposed(&executor), EDIT);

    let scratch = Scratch::new("later");
    let mut body = vec![turn(1)];
    body.extend(deltas(&action(EDIT), 4));
    body.push(usage());
    body.push(turn(2));
    body.extend(deltas(&action(WRITE), 4));
    body.push(usage());
    let executor = scratch.executor(session(body));
    assert_eq!(proposed(&executor), WRITE);
}

#[test]
fn a_crlf_split_over_two_fragments_keeps_the_action_line_whole() {
    let scratch = Scratch::new("crlf");
    let line = action(WRITE);
    let body = vec![
        turn(1),
        delta("Written.\r"),
        delta("\n"),
        delta(&line[..10]),
        delta(&line[10..]),
        delta("\r\n"),
        usage(),
    ];
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

#[test]
fn prose_ending_mid_line_then_a_newline_then_the_action_line_in_one_turn() {
    let scratch = Scratch::new("mid-line");
    let line = action(WRITE);
    let body = vec![
        turn(1),
        delta("I wrote the te"),
        delta("sts"),
        delta(&format!(".\n{}", &line[..5])),
        delta(&line[5..]),
        usage(),
    ];
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}

/// An empty turn after the action line (an answer nudge, a stop hook) leaves it read.
#[test]
fn an_empty_last_turn_leaves_the_action_line_read() {
    let scratch = Scratch::new("empty-last");
    let mut body = vec![turn(1)];
    body.extend(deltas(&action(WRITE), 4));
    body.push(usage());
    body.push(turn(2));
    body.push(delta(""));
    body.push(usage());
    let executor = scratch.executor(session(body));

    assert_eq!(proposed(&executor), WRITE);
}
