//! Adversary a1 of wave 2026-10-28-w1 against story:executor-reports-harness-child-failure
//! (merge `8feb6719d`): the stderr tail's credential walk and bound, the reason a session whose
//! child failed suspends with, and what its record keeps of the cause.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, wrapped so that the child ends
//! as the test says): no harness program runs, no model is called and nothing reaches the network.
//! Credential-shaped values are joined at run time so that no secret scanner reads one here.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::protocol::ProcessTermination;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessPoll, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::AgentSessions;
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, STDERR_TAIL_BYTES, SessionExecutor,
};

const WRITE: &str = "tests.write";
const REFUSED: &str = "error: model wire refused: Unsupported: Messages field is outside the \
                       declared subset: message_delta.usage.cost";
const TAIL: &str = "the last lines of its stderr:\n";
const SECRET: &str = "SECRETabcdefghijklmnop";

fn shaped(parts: &[&str]) -> String {
    parts.concat()
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adv-a1-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, lines: Vec<serde_json::Value>, stderr: &str, end: End) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
                .with_runner(Box::new(Failing {
                    lines,
                    stderr: stderr.to_owned(),
                    end,
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

#[derive(Debug, Clone, Copy)]
enum End {
    Exited(i32),
    Signaled(i32),
}

struct Failing {
    lines: Vec<serde_json::Value>,
    stderr: String,
    end: End,
    log: ScriptedLog,
}

impl ProcessRunner for Failing {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let steps = self
            .lines
            .iter()
            .map(|line| ScriptStep::line(line.to_string()))
            .collect();
        let inner = ScriptedRunner::new(steps, self.log.clone()).start(plan)?;
        Ok(Box::new(Ended {
            inner,
            stderr: self.stderr.clone(),
            end: self.end,
        }))
    }
}

struct Ended {
    inner: Box<dyn HarnessProcess>,
    stderr: String,
    end: End,
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
        Ok(match self.end {
            End::Exited(code) => Some(code),
            End::Signaled(_) => None,
        })
    }

    fn termination(&self) -> ProcessTermination {
        match self.end {
            End::Exited(code) => ProcessTermination::Exited { code },
            End::Signaled(signal) => ProcessTermination::Signaled { signal },
        }
    }

    fn stderr(&self) -> String {
        self.stderr.clone()
    }
}

/// A b10x-harness session that opens, starts a turn and streams `text`, with no terminal record.
fn saying(text: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
        serde_json::json!({"kind": "turn-started", "turn": 1}),
        serde_json::json!({"kind": "text-delta", "text": text}),
    ]
}

fn silent_session() -> Vec<serde_json::Value> {
    saying("I will read src/lib.rs first")
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adv-a1".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(case: &CaseId) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf3)),
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

fn why(executor: &SessionExecutor) -> String {
    let case = CaseId("case-adv-a1".to_owned());
    let step = executor.run_with_usage(&commission(&case), &frontier(&case));
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("the step did not suspend: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!("not ExternalAvailability as text: {:?}", suspended.reason);
    };
    why.clone()
}

fn last_row(executor: &SessionExecutor) -> AgentSessions {
    executor
        .sessions()
        .last()
        .cloned()
        .expect("a recorded session")
}

/// A colouring child (the unit's own fixture `control_characters_on_stderr_do_not_reach_the_error`
/// feeds an ANSI-coloured line) writes an SGR sequence right before a token: the `m` closing
/// `ESC[1m` is a letter, so the token's prefix is not read as starting a word and the line is
/// shown, the escape replaced and the token whole.
#[test]
fn adversary_an_ansi_escape_right_before_a_token_does_not_hide_it_from_the_walk() {
    let scratch = Scratch::new("ansi");
    let secret = shaped(&["sk-", "ant-api03-", SECRET]);
    let stderr = format!("\u{1b}[2mdebug\u{1b}[0m key=\u{1b}[1m{secret}\u{1b}[0m\n{REFUSED}\n");
    let executor = scratch.executor(silent_session(), &stderr, End::Exited(1));
    let why = why(&executor);

    assert!(!why.contains(SECRET), "the credential is repeated: {why:?}");
    let tail = last_row(&executor)
        .failure
        .and_then(|failure| failure.stderr_tail)
        .unwrap_or_default();
    assert!(
        !tail.contains(SECRET),
        "the record repeats the credential: {tail:?}"
    );
}

/// GitHub's installation, OAuth, user-to-server and refresh tokens (`ghs_`, `gho_`, `ghu_`,
/// `ghr_`) are the same family as the `ghp_` the walk withholds; an installation token is what a
/// GitHub App (a bot) pushes with.
#[test]
fn adversary_a_github_installation_token_on_stderr_is_withheld() {
    let scratch = Scratch::new("ghs");
    let stderr = format!(
        "debug: GH_TOKEN={}\n{REFUSED}\n",
        shaped(&["ghs", "_", SECRET])
    );
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    assert!(!why.contains(SECRET), "the credential is repeated: {why:?}");
}

/// A child killed while it drew a progress meter (carriage returns, no final line break): the
/// last line is under the bound as written, over it once each `\r` is replaced by U+FFFD (three
/// bytes), and the second pass then keeps no line at all. The child said something; the reason
/// and the record say it said nothing, and no note stands in for the line.
#[test]
fn adversary_a_last_line_widened_past_the_bound_by_replaced_controls_still_leaves_a_tail() {
    let scratch = Scratch::new("widened");
    let mut stderr = String::from("fetching model weights\n");
    for n in 0..800 {
        stderr.push_str(&format!("\r{:3}%", n % 100));
    }
    assert!(
        stderr.len() <= STDERR_TAIL_BYTES,
        "under the bound as written"
    );
    let executor = scratch.executor(silent_session(), &stderr, End::Signaled(9));
    let why = why(&executor);

    let failure = last_row(&executor).failure.expect("a failure");
    assert!(
        failure
            .stderr_tail
            .as_deref()
            .is_some_and(|tail| !tail.is_empty()),
        "the child wrote {} bytes on stderr, the record keeps none of them: {failure:?}",
        stderr.len()
    );
    assert!(why.contains(TAIL), "the reason carries no tail: {why:?}");
}

/// A child whose stream is cut inside its JSON line (the model wire refused mid-message) exits 1
/// and says why on stderr; the reason is "the session's JSON line does not read" alone, with
/// neither the exit status nor the stderr tail that the record of the same session holds.
#[test]
fn adversary_a_child_cut_off_inside_its_json_line_reports_its_end_and_stderr_tail() {
    let scratch = Scratch::new("cut-json");
    let lines = saying("I will write src/lib.rs\n{\"action\": \"tests.wr");
    let executor = scratch.executor(lines, &format!("{REFUSED}\n"), End::Exited(1));
    let why = why(&executor);

    let failure = last_row(&executor).failure.expect("a failure");
    assert!(
        failure
            .stderr_tail
            .as_deref()
            .is_some_and(|tail| tail.ends_with(REFUSED)),
        "the record holds the cause: {failure:?}"
    );
    assert!(
        why.contains("exited with status 1"),
        "the reason names no exit status: {why:?}"
    );
    assert!(
        why.contains(REFUSED),
        "the reason holds no stderr tail though the record does: {why:?}"
    );
}

/// `loom.session.SessionFailed` is "a session that started and proposed nothing, recorded with
/// why, so a caller reading the record sees the same cause on a repeat" (`session.yaml`). Two
/// sessions that failed for different reasons (no action line; an action line that does not
/// read) leave the same `failure`: the record keeps how the child ended, not why the session
/// failed.
#[test]
fn adversary_two_sessions_failed_for_different_reasons_leave_different_records() {
    let scratch = Scratch::new("causes");
    let unreplied = scratch.executor(silent_session(), "", End::Exited(0));
    let first = why(&unreplied);
    let malformed = scratch.executor(saying("{\"action\": oops}"), "", End::Exited(0));
    let second = why(&malformed);
    assert_ne!(first, second, "two different reasons");

    assert_ne!(
        last_row(&unreplied).failure,
        last_row(&malformed).failure,
        "the records cannot tell {first:?} from {second:?}"
    );
}

/// The cut at the window's first byte falls inside a four-byte character, and a line ends in a
/// bare carriage return: the tail is whole lines, within the bound, the last one kept.
#[test]
fn adversary_a_window_starting_inside_a_multibyte_character_keeps_whole_lines() {
    let scratch = Scratch::new("utf8");
    let mut stderr = String::new();
    while stderr.len() < STDERR_TAIL_BYTES * 2 {
        stderr.push_str("\u{1f600}\u{1f600}\u{1f600} wait\r\n");
    }
    stderr.push_str(&format!("{REFUSED}\r\n"));
    let executor = scratch.executor(silent_session(), &stderr, End::Exited(1));
    let _ = why(&executor);

    let tail = last_row(&executor)
        .failure
        .and_then(|failure| failure.stderr_tail)
        .expect("a tail");
    assert!(tail.len() <= STDERR_TAIL_BYTES, "bounded: {}", tail.len());
    assert!(tail.ends_with(REFUSED), "the last line: {tail:?}");
    assert!(
        tail.lines()
            .all(|line| line == REFUSED || line == "\u{1f600}\u{1f600}\u{1f600} wait"),
        "whole lines only: {tail:?}"
    );
}
