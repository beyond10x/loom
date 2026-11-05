//! A harness child that fails says why: the executor's error names how the child ended (its exit
//! status, or the signal that took it) and, where metaharness hands it on, the last whole lines of
//! what it wrote on standard error, bounded by `STDERR_TAIL_BYTES` and with every line that holds
//! a credential withheld, never repeated: not in the error, the failure record, the session's
//! transcript or the lines its observer is handed.
//!
//! The shape is that of a delivery trial: a b10x-harness child refused by the model wire wrote
//! one sentence on stderr and exited non-zero without a terminal record, and the case recorded only
//! "the session ended without the JSON line naming its action".
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`, wrapped so that the child ends
//! as the test says): no harness program runs, no model is called and nothing reaches the network.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::protocol::ProcessTermination;
use b10x_loom_session_executor::metaharness::warning::NO_TERMINAL_RECORD;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessPoll, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::model::session::{
    AgentSessionState, AgentSessions, ChildEnd, SessionFailure, SessionKind,
};
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, STDERR_TAIL_BYTES, SessionExecutor,
    TOKEN_PREFIXES,
};

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
    /// Metaharness measured no native end (a replayed child).
    Unmeasured,
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
            End::Signaled(_) | End::Unmeasured => None,
        })
    }

    fn termination(&self) -> ProcessTermination {
        match self.end {
            End::Exited(code) => ProcessTermination::Exited { code },
            End::Signaled(signal) => ProcessTermination::Signaled { signal },
            End::Unmeasured => ProcessTermination::Unknown,
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

/// D5's ceiling on the bound.
const _: () = assert!(STDERR_TAIL_BYTES <= 4096);

#[test]
fn a_child_metaharness_measured_no_end_for_is_said_to_have_none() {
    let scratch = Scratch::new("unmeasured");
    let why = why(&scratch.executor(silent_session(), "", End::Unmeasured));

    assert!(why.starts_with(TODAY), "today's message: {why:?}");
    assert!(
        why.contains("metaharness measured no exit status"),
        "never an exit 0 it did not see: {why:?}"
    );
    assert!(!why.contains("status 0"), "no invented status: {why:?}");
}

#[test]
fn control_characters_on_stderr_do_not_reach_the_error() {
    let scratch = Scratch::new("control");
    let stderr = format!("\u{1b}[31m{REFUSED}\u{1b}[0m\u{7}\r\nnext\u{0}line\n");
    let why = why(&scratch.executor(silent_session(), &stderr, End::Exited(1)));

    let tail = tail_of(&why);
    assert!(
        !tail
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t'),
        "no control character but a line break or a tab: {tail:?}"
    );
    assert!(tail.contains(REFUSED), "the text itself is kept: {tail:?}");
}

/// The one session `executor` recorded last, or a panic naming how many it holds.
fn last_row(executor: &SessionExecutor, count: usize) -> AgentSessions {
    let rows = executor.sessions();
    assert_eq!(rows.len(), count, "the sessions recorded: {rows:?}");
    rows.last().cloned().expect("a recorded session")
}

#[test]
fn a_failed_harness_child_records_its_cause() {
    let scratch = Scratch::new("record");
    let stderr = format!("loading profile\nconnecting to the endpoint\n{REFUSED}\n");
    let executor = scratch.executor(silent_session(), &stderr, End::Exited(1));
    let first = why(&executor);

    let row = last_row(&executor, 1);
    let failure = row.failure.clone().expect("the record names its failure");
    assert_eq!(failure.end, ChildEnd::Exited, "how the child ended");
    assert_eq!(failure.code, Some(1), "its exit status");
    assert_eq!(
        failure.stderr_tail.as_deref(),
        Some(tail_of(&first)),
        "the stderr tail the error carries, as walked"
    );
    assert!(
        failure
            .stderr_tail
            .as_deref()
            .is_some_and(|tail| tail.ends_with(REFUSED)),
        "the cause itself: {failure:?}"
    );
    assert_eq!(row.kind, SessionKind::Harness);
    assert_eq!(row.commission_id.0.0, uuid(0xe1).0, "the commission's id");
    assert_eq!(row.frontier_id.0.0, uuid(0xf3).0, "the frontier's id");
    assert_eq!(row.action, "", "a failed session names no action");
    assert!(row.files.is_empty(), "and proposes no file");
    assert_eq!(row.continues, None);
    assert_eq!(row.state, AgentSessionState::Ended);
    assert!(executor.proposed().is_empty(), "nothing was proposed");

    // A repeat of the same refusal reads the same cause off its own record.
    let second = why(&executor);
    assert_eq!(second, first, "the same error on a repeat");
    let again = last_row(&executor, 2);
    assert_ne!(again.session_id, row.session_id, "a session of its own");
    assert_eq!(again.failure, row.failure, "the same cause on a repeat");
}

#[test]
fn a_failed_session_records_a_signal_or_an_end_metaharness_did_not_measure() {
    let scratch = Scratch::new("record-ends");
    let signaled = scratch.executor(silent_session(), "killed\n", End::Signaled(9));
    let _ = why(&signaled);
    assert_eq!(
        last_row(&signaled, 1).failure,
        Some(SessionFailure {
            end: ChildEnd::Signaled,
            code: Some(9),
            stderr_tail: Some("killed".to_owned()),
            reason: Some(
                "the session ended without the JSON line naming its action \
                 (the harness was ended by signal 9)"
                    .to_owned()
            ),
        }),
        "the signal and its stderr"
    );

    let unmeasured = scratch.executor(silent_session(), "", End::Unmeasured);
    let _ = why(&unmeasured);
    assert_eq!(
        last_row(&unmeasured, 1).failure,
        Some(SessionFailure {
            end: ChildEnd::NotMeasured,
            code: None,
            stderr_tail: None,
            reason: Some(
                "the session ended without the JSON line naming its action \
                 (metaharness measured no exit status)"
                    .to_owned()
            ),
        }),
        "no status it did not see, no tail when it said nothing"
    );
}

#[test]
fn a_failed_session_withholds_a_credential_from_its_record() {
    let scratch = Scratch::new("record-credential");
    let stderr = format!(
        "debug: value={}\n{REFUSED}\n",
        shaped(&["sk-", "ant-api03-", SECRET])
    );
    let executor = scratch.executor(silent_session(), &stderr, End::Exited(1));
    let _ = why(&executor);

    let failure = last_row(&executor, 1).failure.expect("a failure");
    let tail = failure.stderr_tail.expect("a stderr tail");
    assert!(!tail.contains(SECRET), "never repeated: {tail:?}");
    assert!(tail.ends_with(REFUSED), "the cause is kept: {tail:?}");
    assert_eq!(
        failure.reason.as_deref(),
        Some(
            "the session ended without the JSON line naming its action \
             (the harness exited with status 1)"
        ),
        "the reason, its stderr tail left to `stderr_tail`"
    );
}

#[test]
fn a_session_refused_before_its_child_started_is_no_record() {
    let scratch = Scratch::new("record-refused");
    let mut executor = scratch.executor(silent_session(), &format!("{REFUSED}\n"), End::Exited(1));
    executor.files = vec!["../outside".to_owned()];
    let _ = why(&executor);

    assert!(
        !executor.last_report().is_some_and(|report| report.started),
        "no child started"
    );
    assert!(executor.sessions().is_empty(), "and nothing is recorded");
}

#[test]
fn a_failed_session_names_its_predecessor_and_leaves_it_to_the_retry() {
    let scratch = Scratch::new("record-lineage");
    let stderr = format!("{REFUSED}\n");
    let earlier = scratch.executor(silent_session(), &stderr, End::Exited(1));
    let _ = why(&earlier);
    let predecessor = last_row(&earlier, 1);

    let executor = scratch
        .executor(silent_session(), &stderr, End::Exited(1))
        .continuing(&predecessor);
    let _ = why(&executor);
    assert_eq!(
        last_row(&executor, 1).continues,
        Some(predecessor.session_id.clone()),
        "the failed session names the session it continues"
    );
    let retry = why(&executor);
    let again = last_row(&executor, 2);
    assert_eq!(
        again.continues,
        Some(predecessor.session_id.clone()),
        "the retry continues what the failed attempt continued: {retry}"
    );
    assert!(again.failure.is_some(), "and fails the same way");
}

/// Every event line a session hands its observer, serialized.
type Observed = Rc<RefCell<Vec<String>>>;

/// `executor` handing each event it receives, serialized, to the list returned beside it.
fn observed(executor: SessionExecutor) -> (SessionExecutor, Observed) {
    let seen: Observed = Rc::new(RefCell::new(Vec::new()));
    let into = Rc::clone(&seen);
    let executor = executor.with_observer(move |line, _copy| {
        into.borrow_mut()
            .push(serde_json::to_string(line).expect("an event serializes"));
    });
    (executor, seen)
}

/// The message of the `NO_TERMINAL_RECORD` warning `transcript` holds, or a panic naming it.
fn stderr_warning(transcript: &str) -> String {
    transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|line| line["event"] == "warning" && line["code"] == NO_TERMINAL_RECORD)
        .and_then(|line| line["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no {NO_TERMINAL_RECORD} warning in {transcript:?}"))
}

/// A stderr between a first line and the cause, which hold no credential, of a line for each
/// prefix of `TOKEN_PREFIXES` but a PEM block's opening, a URL with user information, a prefix an
/// escape sequence splits and a PEM block, each joined at run time with [`SECRET`]; and how many
/// lines of it hold a credential's shape.
fn credential_stderr() -> (String, usize) {
    let mut stderr = String::from("starting\n");
    let mut held = 0;
    for prefix in TOKEN_PREFIXES
        .iter()
        .filter(|prefix| !prefix.starts_with('-'))
    {
        stderr.push_str(&format!("debug: value={prefix}{SECRET}\n"));
        held += 1;
    }
    stderr.push_str(&shaped(&[
        "debug: proxy https://agent:",
        "hunter2pass",
        "@proxy.example:8443/v1\n",
    ]));
    let (head, rest) = TOKEN_PREFIXES[0].split_at(1);
    stderr.push_str(&format!("debug: read {head}\u{1b}[0m{rest}{SECRET}\n"));
    let begin = TOKEN_PREFIXES
        .iter()
        .find(|prefix| prefix.starts_with('-'))
        .expect("a PEM opening");
    stderr.push_str(&format!("{begin} PRIVATE KEY-----\n"));
    stderr.push_str(&shaped(&["MIIEvQ", SECRET, "\n"]));
    stderr.push_str(&shaped(&["-----END ", "PRIVATE", " KEY-----\n"]));
    held += 5;
    stderr.push_str(&format!("{REFUSED}\n"));
    (stderr, held)
}

#[test]
fn a_credential_on_stderr_reaches_neither_the_transcript_nor_the_observer() {
    let scratch = Scratch::new("transcript-credential");
    let (stderr, held) = credential_stderr();
    let (executor, seen) = observed(scratch.executor(silent_session(), &stderr, End::Exited(1)));
    let why = why(&executor);
    let report = executor.last_report().expect("a report");
    let transcript = String::from_utf8(report.transcript.clone()).expect("a UTF-8 transcript");
    let seen = seen.borrow();

    for (what, text) in [
        ("transcript", transcript.as_str()),
        ("observed", &seen.join("\n")),
    ] {
        for secret in [SECRET, "MIIEvQ", "hunter2pass"] {
            let holding: Vec<&str> = text.lines().filter(|line| line.contains(secret)).collect();
            assert!(
                holding.is_empty(),
                "no line of the {what} holds {secret}: {holding:?}"
            );
        }
    }
    assert!(
        !format!("{:?}", report.trace).contains(SECRET),
        "nor in the trace projected from it: {:?}",
        report.trace
    );
    assert_eq!(
        seen.join("\n"),
        transcript.trim_end(),
        "the observer sees the transcript's lines"
    );
    let warning = stderr_warning(&transcript);
    let shown =
        format!("starting\n[{held} lines withheld: they hold a credential's shape]\n{REFUSED}");
    assert!(
        warning.ends_with(&format!("wrote this on stderr: {shown}")),
        "the warning keeps every other line and says what it withheld: {warning:?}"
    );
    assert_eq!(tail_of(&why), shown, "the error's tail is as today");
    let failure = last_row(&executor, 1).failure.expect("a failure");
    assert_eq!(
        failure.stderr_tail.as_deref(),
        Some(shown.as_str()),
        "the record's tail is as today"
    );
}

#[test]
fn a_stderr_without_a_credential_is_kept_whole_in_the_transcript() {
    let scratch = Scratch::new("transcript-whole");
    let mut stderr = String::new();
    for at in 0..(2 * STDERR_TAIL_BYTES / 40) {
        stderr.push_str(&format!("progress: step {at:>6} of the loop went well\n"));
    }
    stderr.push_str(&format!("{REFUSED}\n"));
    let (executor, seen) = observed(scratch.executor(silent_session(), &stderr, End::Exited(1)));
    let _ = why(&executor);
    let report = executor.last_report().expect("a report");
    let transcript = String::from_utf8(report.transcript.clone()).expect("a UTF-8 transcript");

    assert_eq!(
        stderr_warning(&transcript),
        format!(
            "the harness produced no terminal record and wrote this on stderr: {}",
            stderr.trim_end()
        ),
        "the warning as metaharness wrote it, past the tail's bound"
    );
    assert_eq!(
        seen.borrow().join("\n"),
        transcript.trim_end(),
        "and as the observer sees it"
    );
}

#[test]
fn a_word_that_only_contains_a_prefix_inside_it_is_kept() {
    let scratch = Scratch::new("inside");
    let stderr = "the task-runner failed on disk-full\nsee https://docs.example/errors/wire\n";
    let why = why(&scratch.executor(silent_session(), stderr, End::Exited(1)));

    assert_eq!(
        tail_of(&why),
        stderr.trim_end(),
        "nothing withheld: {why:?}"
    );
}
