//! Security review of `story:session-transcript-withholds-stderr-credentials` (loom wave
//! 2026-11-05-w2, unit U4): conformance checks of the walk the `NO_TERMINAL_RECORD` warning now
//! takes before the transcript and the observer hold it.
//!
//! - The walk now reads the child's whole stderr, which metaharness retains unbounded, where the
//!   tail read at most `STDERR_TAIL_BYTES` of it: walking it must stay linear in its length, for
//!   a long line holding many URLs (a compact JSON dump) as for many `-----END` lines.
//! - The single-pass `-----END` fill must withhold exactly what the earlier fill of every line
//!   before it withheld: a seeded property against that earlier algorithm, read through the
//!   transcript's warning and the error's tail.
//! - A token right after a lone `ESC` is read by the walk as an escape sequence that takes the
//!   token's first character, so neither form holds the prefix and the rest of the token is
//!   kept, in the transcript and in the tail alike.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, wrapped so that the child ends as
//! the test says): no harness program runs, no model is called and nothing reaches the network.
//! Every credential-shaped value is joined at run time from `TOKEN_PREFIXES` and a body.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

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
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, SessionExecutor, TOKEN_PREFIXES,
};

const WRITE: &str = "tests.write";
const REFUSED: &str = "error: model wire refused: the declared subset";
const TAIL: &str = "the last lines of its stderr:\n";
const SAID: &str = "the harness produced no terminal record and wrote this on stderr: ";
/// The body of every credential-shaped value here.
const SECRET: &str = "SECRETqrstuvwxyz0123";
/// A bound far above what a linear walk of the stderrs here takes in a debug build.
const BOUND: Duration = Duration::from_secs(10);

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-u4-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn executor(&self, stderr: &str) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::B10x)
                .with_runner(Box::new(Failing {
                    stderr: stderr.to_owned(),
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

/// A scripted b10x-harness child that starts, says a sentence, writes `stderr` and exits 1
/// without a terminal record.
struct Failing {
    stderr: String,
    log: ScriptedLog,
}

impl ProcessRunner for Failing {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let steps = [
            serde_json::json!({"kind": "started", "model": "synthetic", "published_tools": [],
                               "credential_source": "none"}),
            serde_json::json!({"kind": "turn-started", "turn": 1}),
            serde_json::json!({"kind": "text-delta", "text": "I will read src/lib.rs first"}),
        ]
        .iter()
        .map(|line| ScriptStep::line(line.to_string()))
        .collect();
        let inner = ScriptedRunner::new(steps, self.log.clone()).start(plan)?;
        Ok(Box::new(Ended {
            inner,
            stderr: self.stderr.clone(),
        }))
    }
}

struct Ended {
    inner: Box<dyn HarnessProcess>,
    stderr: String,
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
        Ok(Some(1))
    }

    fn termination(&self) -> ProcessTermination {
        ProcessTermination::Exited { code: 1 }
    }

    fn stderr(&self) -> String {
        self.stderr.clone()
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-security-u4".to_owned()),
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

/// What one session over `stderr` left: the error, the transcript, the lines the observer was
/// handed (serialized), the failure record's tail, and how long the call took.
struct Outcome {
    why: String,
    transcript: String,
    observed: Vec<String>,
    record_tail: Option<String>,
    took: Duration,
}

fn session(name: &str, stderr: &str) -> Outcome {
    let scratch = Scratch::new(name);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let into = Rc::clone(&seen);
    let executor = scratch.executor(stderr).with_observer(move |line, _copy| {
        into.borrow_mut()
            .push(serde_json::to_string(line).expect("an event serializes"));
    });
    let case = CaseId("case-security-u4".to_owned());
    let started = Instant::now();
    let step = executor.run_with_usage(&commission(&case), &frontier(&case));
    let took = started.elapsed();
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("the step did not suspend: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!("not ExternalAvailability as text: {:?}", suspended.reason);
    };
    let report = executor.last_report().expect("a report");
    let transcript = String::from_utf8(report.transcript.clone()).expect("a UTF-8 transcript");
    let record_tail = executor
        .sessions()
        .last()
        .and_then(|row| row.failure.clone())
        .and_then(|failure| failure.stderr_tail);
    let observed = seen.borrow().clone();
    Outcome {
        why: why.clone(),
        transcript,
        observed,
        record_tail,
        took,
    }
}

/// The message of the `NO_TERMINAL_RECORD` warning `transcript` holds.
fn stderr_warning(transcript: &str) -> String {
    transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|line| line["event"] == "warning" && line["code"] == NO_TERMINAL_RECORD)
        .and_then(|line| line["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no {NO_TERMINAL_RECORD} warning in {transcript:?}"))
}

fn tail_of(why: &str) -> &str {
    match why.split_once(TAIL) {
        Some((_, tail)) => tail,
        None => panic!("the error carries no stderr tail: {why:?}"),
    }
}

/// The PEM opening `TOKEN_PREFIXES` holds.
fn pem_begin() -> &'static str {
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| prefix.starts_with('-'))
        .expect("a PEM opening")
}

#[test]
fn security_a_long_stderr_line_holding_many_urls_is_walked_in_linear_time() {
    // A compact JSON dump on one stderr line: many URLs and no whitespace. Each `://` the walk
    // finds is read up to the next whitespace, which is the line's end, so the walk of this one
    // line is quadratic in its length; the tail read at most `STDERR_TAIL_BYTES` of it.
    let mut dump = String::from("debug: response {\"links\":[");
    while dump.len() < 128 * 1024 {
        dump.push_str("\"https://h.example/p\",");
    }
    dump.push_str("\"end\"]}");
    let stderr = format!("starting\n{dump}\n{REFUSED}\n");
    let outcome = session("many-urls", &stderr);
    assert_eq!(
        tail_of(&outcome.why),
        REFUSED,
        "the tail is the cause, as today"
    );
    assert!(
        outcome.took < BOUND,
        "a {} byte stderr took {:?} to walk, past {BOUND:?}",
        stderr.len(),
        outcome.took
    );
}

#[test]
fn security_a_stderr_of_many_pem_end_lines_is_walked_in_linear_time() {
    let end = ["-----END ", "PRIVATE", " KEY-----"].concat();
    let mut stderr = String::from("starting\n");
    for at in 0..100_000 {
        stderr.push_str(&format!("{at} {end}\n"));
    }
    stderr.push_str(&format!("{REFUSED}\n"));
    let outcome = session("many-ends", &stderr);
    assert!(
        tail_of(&outcome.why).ends_with(&format!("they hold a credential's shape]\n{REFUSED}")),
        "the tail withholds every line before the last `-----END`: {:?}",
        tail_of(&outcome.why)
    );
    assert!(
        outcome.took < BOUND,
        "a {} byte stderr took {:?} to walk, past {BOUND:?}",
        stderr.len(),
        outcome.took
    );
}

/// The kinds of line the property draws from.
#[derive(Clone, Copy)]
enum Draw {
    Plain,
    Token,
    Begin,
    Body,
    End,
}

/// The earlier `credential_lines` (base 4063fca8), for lines drawn as `draws`: a token or a PEM
/// opening holds a shape itself, a block runs from its opening to its `-----END`, and an
/// `-----END` outside a block withholds every line before it.
fn earlier_withheld(draws: &[Draw]) -> Vec<bool> {
    let mut withheld: Vec<bool> = draws
        .iter()
        .map(|draw| matches!(draw, Draw::Token | Draw::Begin))
        .collect();
    let mut in_block = false;
    for at in 0..draws.len() {
        if matches!(draws[at], Draw::Begin) {
            in_block = true;
        }
        if in_block {
            withheld[at] = true;
        }
        if matches!(draws[at], Draw::End) {
            if !in_block {
                withheld[..=at].iter_mut().for_each(|line| *line = true);
            }
            in_block = false;
        }
    }
    withheld
}

/// `lines` shown with each run of withheld ones replaced by the walk's note.
fn shown(lines: &[String], withheld: &[bool]) -> String {
    let note = |run: usize| match run {
        1 => "[a line withheld: it holds a credential's shape]".to_owned(),
        n => format!("[{n} lines withheld: they hold a credential's shape]"),
    };
    let mut out = Vec::new();
    let mut run = 0;
    for (line, withheld) in lines.iter().zip(withheld) {
        if *withheld {
            run += 1;
            continue;
        }
        if run > 0 {
            out.push(note(run));
            run = 0;
        }
        out.push(line.clone());
    }
    if run > 0 {
        out.push(note(run));
    }
    out.join("\n")
}

#[test]
fn security_the_single_pass_end_fill_withholds_what_the_earlier_fill_withheld() {
    let prefixes: Vec<&str> = TOKEN_PREFIXES
        .iter()
        .copied()
        .filter(|prefix| !prefix.starts_with('-'))
        .collect();
    let end = ["-----END ", "PRIVATE", " KEY-----"].concat();
    let mut state: u64 = 0x5eed_0004;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    for case in 0..40 {
        let count = 1 + next() % 12;
        let mut draws = vec![Draw::Plain];
        let mut lines = vec![format!("starting case {case}")];
        for at in 0..count {
            let draw = match next() % 5 {
                0 => Draw::Plain,
                1 => Draw::Token,
                2 => Draw::Begin,
                3 => Draw::Body,
                _ => Draw::End,
            };
            lines.push(match draw {
                Draw::Plain => format!("progress {at} went well"),
                Draw::Token => format!("value={}{SECRET}", prefixes[next() % prefixes.len()]),
                Draw::Begin => format!("{} PRIVATE KEY-----", pem_begin()),
                Draw::Body => format!("MIIEvQ{at}"),
                Draw::End => end.clone(),
            });
            draws.push(draw);
        }
        draws.push(Draw::Plain);
        lines.push(REFUSED.to_owned());
        let expected = shown(&lines, &earlier_withheld(&draws));
        let stderr = format!("{}\n", lines.join("\n"));
        let outcome = session(&format!("property-{case}"), &stderr);
        assert_eq!(
            stderr_warning(&outcome.transcript),
            format!("{SAID}{expected}"),
            "case {case}: the warning withholds what the earlier walk withheld, of {stderr:?}"
        );
        assert_eq!(
            tail_of(&outcome.why),
            expected,
            "case {case}: and so does the tail, of {stderr:?}"
        );
        assert_eq!(outcome.record_tail.as_deref(), Some(expected.as_str()));
    }
}

#[test]
fn security_a_pem_block_the_tail_window_cuts_from_its_opening_is_withheld_from_the_record() {
    // A PEM opening, more body than the tail's window holds, and no `-----END`: the window holds
    // body lines only, no opening and no end, so the tail keeps them. The whole stderr's walk
    // (the transcript's) withholds them from the opening on.
    let mut stderr = format!("starting\n{} PRIVATE KEY-----\n", pem_begin());
    for at in 0..120 {
        stderr.push_str(&format!("MIIEvQ{SECRET}{at:040}\n"));
    }
    stderr.push_str(&format!("{REFUSED}\n"));
    let outcome = session("cut-pem", &stderr);
    assert!(
        !outcome.transcript.contains(SECRET),
        "the transcript withholds the block"
    );
    for (what, text) in [
        ("error", outcome.why.as_str()),
        ("record tail", outcome.record_tail.as_deref().unwrap_or("")),
    ] {
        assert!(
            !text.contains(SECRET),
            "the {what} holds the PEM block's body: {text:?}"
        );
    }
}

#[test]
fn security_a_token_after_a_lone_escape_reaches_neither_the_transcript_nor_the_record() {
    // `ESC` followed by a character in `0x30..=0x7e` is read as a two-character escape sequence,
    // so the walk drops the token's first character from both forms and neither holds its
    // prefix; the rest of the token, its secret, is kept.
    let prefix = TOKEN_PREFIXES
        .iter()
        .find(|prefix| prefix.starts_with("gh"))
        .expect("a GitHub prefix");
    let stderr = format!("starting\ndebug: value=\u{1b}{prefix}{SECRET}\n{REFUSED}\n");
    let outcome = session("lone-escape", &stderr);
    for (what, text) in [
        ("transcript", outcome.transcript.as_str()),
        ("observed", &outcome.observed.join("\n")),
        ("error", outcome.why.as_str()),
        ("record tail", outcome.record_tail.as_deref().unwrap_or("")),
    ] {
        assert!(
            !text.contains(SECRET),
            "the {what} holds the token's secret: {text:?}"
        );
    }
}
