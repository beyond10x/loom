//! A harness child that fails says why: the executor's error names how the child ended (its exit
//! status, or the signal that took it) and, where metaharness hands it on, the last whole lines of
//! what it wrote on standard error, bounded by `STDERR_TAIL_BYTES` and with every line that holds
//! a credential withheld, never repeated.
//!
//! The shape is that of a delivery trial: a b10x-harness child refused by the model wire wrote
//! one sentence on stderr and exited non-zero without a terminal record, and the case recorded only
//! "the session ended without the JSON line naming its action".
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, wrapped so that the child ends
//! as the test says): no harness program runs, no model is called and nothing reaches the network.

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
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, SessionExecutor,
};

/// The bound D5 names; the crate states it once it bounds the tail.
const STDERR_TAIL_BYTES: usize = 4096;

const WRITE: &str = "tests.write";
const TODAY: &str = "the session ended without the JSON line naming its action";
const REFUSED: &str = "error: model wire refused: Unsupported: Messages field is outside the \
                       declared subset: message_delta.usage.cost";
/// Where the error's stderr tail starts.
const TAIL: &str = "the last lines of its stderr:\n";

/// The body of every credential-shaped value here, which no error may repeat.
const SECRET: &str = "SECRETabcdefghijklmnop";

/// A value shaped like a credential, joined at run time from `parts` so that no secret scanner
/// reads one in this source.
fn shaped(parts: &[&str]) -> String {
    parts.concat()
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-child-failure-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    /// A b10x-harness executor over the tree whose child prints `lines`, writes `stderr` and ends
    /// as `end` says, its loop pointed at a closed local port with a scripted token file.
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

/// How the scripted child ends.
#[derive(Debug, Clone, Copy)]
enum End {
    Exited(i32),
    Signaled(i32),
}

/// A scripted b10x-harness child that prints its lines, wrote `stderr`, and ends as `end` says.
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

/// The scripted child, its end and its stderr the test's.
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

/// A session that started and said something, but neither its action line nor a terminal record.
fn silent_session() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                           "credential_source": "none"}),
        serde_json::json!({"kind": "turn-started", "turn": 1}),
        serde_json::json!({"kind": "text-delta", "text": "I will read src/lib.rs first"}),
    ]
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-child-failure".to_owned()),
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

/// Why the step suspended, as its `ExternalAvailability` text, which the report's error repeats;
/// a panic when it did not suspend.
fn why(executor: &SessionExecutor) -> String {
    let case = CaseId("case-child-failure".to_owned());
    let step = executor.run_with_usage(&commission(&case), &frontier(&case));
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("the step did not suspend: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!("not ExternalAvailability as text: {:?}", suspended.reason);
    };
    let reported = executor.last_report().and_then(|report| report.error);
    assert_eq!(
        reported.as_deref(),
        Some(why.as_str()),
        "the report's error"
    );
    why.clone()
}

/// The stderr tail the error carries, or a panic naming the error.
fn tail_of(why: &str) -> &str {
    match why.split_once(TAIL) {
        Some((_, tail)) => tail,
        None => panic!("the error carries no stderr tail: {why:?}"),
    }
}

#[test]
fn a_failed_harness_child_reports_its_exit_status_and_stderr_tail() {
    let scratch = Scratch::new("trial");
    let stderr = format!("loading profile\nconnecting to the endpoint\n{REFUSED}\n");
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    assert!(why.starts_with(TODAY), "today's message leads: {why:?}");
    assert!(
        why.contains("exited with status 1"),
        "the exit status: {why:?}"
    );
    let tail = tail_of(&why);
    assert!(
        tail.ends_with(REFUSED),
        "the last stderr line, whole: {tail:?}"
    );
    assert!(
        tail.contains("connecting to the endpoint\n"),
        "the lines before it within the bound: {tail:?}"
    );
}

#[test]
fn a_huge_stderr_is_cut_to_its_last_whole_lines_within_the_bound() {
    let scratch = Scratch::new("huge");
    let mut stderr = String::new();
    for n in 0..200_000 {
        stderr.push_str(&format!("progress line {n:06}\n"));
    }
    stderr.push_str(REFUSED);
    stderr.push('\n');
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(2)));

    assert!(
        why.contains("exited with status 2"),
        "the exit status: {why:?}"
    );
    let tail = tail_of(&why);
    assert!(
        tail.len() <= STDERR_TAIL_BYTES,
        "the tail is {} bytes, more than the bound {STDERR_TAIL_BYTES}",
        tail.len()
    );
    assert!(
        tail.ends_with(REFUSED),
        "the last line is kept: {:?}",
        &tail[tail.len() - 200..]
    );
    assert!(
        !tail.contains("progress line 000000"),
        "the first lines are cut"
    );
    let first = tail.lines().next().expect("a first line");
    assert!(
        first.starts_with("progress line ") && first.len() == "progress line 000000".len(),
        "the tail starts at a whole line: {first:?}"
    );
    assert!(
        why.len() < STDERR_TAIL_BYTES + 512,
        "the error itself is bounded: {} bytes",
        why.len()
    );
}

#[test]
fn a_last_line_longer_than_the_bound_is_not_cut_into_and_its_credential_not_shown() {
    let scratch = Scratch::new("long-line");
    let secret = shaped(&["sk-", "ant-", SECRET]);
    let stderr = format!(
        "{REFUSED}\n{} {secret}\n",
        "x".repeat(STDERR_TAIL_BYTES * 2)
    );
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    assert!(
        why.contains("exited with status 1"),
        "the exit status: {why:?}"
    );
    assert!(!why.contains(SECRET), "no part of the credential: {why:?}");
    assert!(
        !why.contains("xxxxxxxx"),
        "the over-long line is not cut into: {why:?}"
    );
    assert!(
        why.len() < STDERR_TAIL_BYTES + 512,
        "bounded: {} bytes",
        why.len()
    );
}

#[test]
fn a_credential_on_stderr_is_withheld_and_never_repeated() {
    let scratch = Scratch::new("credential");
    let secrets = [
        shaped(&["sk-", "ant-api03-", SECRET]),
        shaped(&["ghp", "_", SECRET]),
        shaped(&["https://agent:", "hunter2pass", "@proxy.example:8443/v1"]),
        shaped(&["https://api.example/v1/messages", "?q=", SECRET]),
        shaped(&["Bearer ", SECRET]),
        shaped(&["AK", "IA", SECRET]),
    ];
    let mut stderr = String::from("starting\n");
    stderr.push_str(&format!("debug: value={}\n", secrets[0]));
    stderr.push_str(&format!("debug: read \"{}\"\n", secrets[1]));
    stderr.push_str(&format!("debug: proxy {}\n", secrets[2]));
    stderr.push_str(&format!("debug: POST {}\n", secrets[3]));
    stderr.push_str(&format!("debug: header {}\n", secrets[4]));
    stderr.push_str(&format!("debug: aws ({})\n", secrets[5]));
    stderr.push_str(&shaped(&["debug: HEADER bear", "er ", SECRET, "\n"]));
    stderr.push_str(&format!("{REFUSED}\n"));
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    assert!(
        !why.contains("SECRET"),
        "no credential is repeated: {why:?}"
    );
    assert!(
        !why.contains("hunter2pass"),
        "no password is repeated: {why:?}"
    );
    let tail = tail_of(&why);
    assert!(
        tail.ends_with(REFUSED),
        "the line without one is kept: {tail:?}"
    );
    assert!(
        tail.starts_with("starting\n"),
        "the first line is kept: {tail:?}"
    );
    assert!(
        tail.contains("withheld"),
        "a withheld line says so: {tail:?}"
    );
}

#[test]
fn a_private_key_on_stderr_is_withheld_even_when_the_bound_cuts_its_first_line() {
    let scratch = Scratch::new("pem");
    let mut stderr = shaped(&["-----BEGIN ", "PRIVATE", " KEY-----\n"]);
    for _ in 0..200 {
        stderr.push_str(&shaped(&["MIIEvQ", SECRET, "\n"]));
    }
    stderr.push_str(&shaped(&["-----END ", "PRIVATE", " KEY-----\n"]));
    stderr.push_str(&format!("{REFUSED}\n"));
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    assert!(
        !why.contains("MIIEvQ"),
        "no line of the key is repeated: {why:?}"
    );
    assert!(
        tail_of(&why).ends_with(REFUSED),
        "the line after it is kept: {why:?}"
    );
}

#[test]
fn a_child_that_exits_0_without_the_json_line_keeps_todays_message_with_its_status() {
    let scratch = Scratch::new("exit-0");
    let why = why(&scratch.executor(silent_session(), "", End::Exited(0)));

    assert!(why.starts_with(TODAY), "today's message: {why:?}");
    assert!(
        why.contains("exited with status 0"),
        "with its exit status: {why:?}"
    );
    assert!(!why.contains(TAIL), "no stderr, no tail: {why:?}");
}

#[test]
fn a_child_a_signal_took_is_reported_by_its_signal() {
    let scratch = Scratch::new("signal");
    let why = why(&scratch.executor(silent_session(), "killed\n", End::Signaled(9)));

    assert!(why.starts_with(TODAY), "today's message: {why:?}");
    assert!(why.contains("ended by signal 9"), "the signal: {why:?}");
    assert!(tail_of(&why).ends_with("killed"), "its stderr: {why:?}");
}

#[test]
fn a_child_with_a_terminal_record_and_no_json_line_still_names_its_exit_status() {
    let scratch = Scratch::new("terminal");
    let mut lines = silent_session();
    lines.push(serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}}));
    let why = why(&scratch.executor(lines, "", End::Exited(3)));

    assert!(why.starts_with(TODAY), "today's message: {why:?}");
    assert!(
        why.contains("exited with status 3"),
        "the exit status: {why:?}"
    );
}
