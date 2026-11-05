//! Security review of `story:stderr-tail-withholds-cut-pem-block` and
//! `story:stderr-walk-withholds-token-after-lone-escape` (loom wave 2026-11-05-w3, unit U2):
//! conformance checks of the stderr walk's third form, a line with its control characters alone
//! dropped, and of the tail that now decides what it withholds over the whole stderr.
//!
//! - The tail withholds at least what the whole stderr's walk withholds on every line of its
//!   window, keeps its byte bound and its notes: a seeded property over stderrs past the window,
//!   with PEM blocks the window cuts, tokens behind escape sequences and lines that a shown
//!   control character makes longer.
//! - A control sequence with a parameter takes a token's first character as its final byte, and
//!   the dropped form then reads the parameter's digit before the prefix: the token is kept.
//! - The dropped form reads an escape sequence's body as text, so a line that shows no URL with a
//!   query, the shown form being what each output keeps, is withheld for one the body holds.
//! - Ordinary coloured log lines, cursor and charset sequences included, are kept as shown.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, wrapped so that the child ends as
//! the test says): no harness program runs, no model is called and nothing reaches the network.
//! Every credential-shaped value is joined at run time from `TOKEN_PREFIXES` and a body.

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
use b10x_loom_session_executor::{
    CredentialSource, Endpoint, Kind, ProcessRunner, STDERR_TAIL_BYTES, SessionExecutor,
    TOKEN_PREFIXES,
};

const WRITE: &str = "tests.write";
const REFUSED: &str = "error: model wire refused: the declared subset";
const TAIL: &str = "the last lines of its stderr:\n";
const SAID: &str = "the harness produced no terminal record and wrote this on stderr: ";
/// The body of every credential-shaped value here.
const SECRET: &str = "SECRETw3u2review0123";
const ONE_WITHHELD: &str = "[a line withheld: it holds a credential's shape]";
const RUN_WITHHELD: &str = " lines withheld: they hold a credential's shape]";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A directory of the case's own under the system's temporary directory, outside the repository,
/// whose memory files metaharness's hermetic floor refuses above a scratch copy.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-w3-u2-{}-{name}",
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
        principal: PrincipalId("principal-security-w3-u2".to_owned()),
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
/// handed (serialized) and the failure record's tail.
struct Outcome {
    why: String,
    transcript: String,
    observed: Vec<String>,
    record_tail: Option<String>,
}

fn session(name: &str, stderr: &str) -> Outcome {
    let scratch = Scratch::new(name);
    let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let into = Rc::clone(&seen);
    let executor = scratch.executor(stderr).with_observer(move |line, _copy| {
        into.borrow_mut()
            .push(serde_json::to_string(line).expect("an event serializes"));
    });
    let case = CaseId("case-security-w3-u2".to_owned());
    let step = executor.run_with_usage(&commission(&case), &frontier(&case));
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

fn is_note(line: &str) -> bool {
    line == ONE_WITHHELD
        || line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(RUN_WITHHELD))
            .is_some_and(|count| {
                !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit())
            })
}

/// The PEM opening `TOKEN_PREFIXES` holds.
fn pem_begin() -> &'static str {
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| prefix.starts_with('-'))
        .expect("a PEM opening")
}

fn github_prefix() -> &'static str {
    TOKEN_PREFIXES
        .iter()
        .find(|prefix| prefix.starts_with("gh"))
        .expect("a GitHub prefix")
}

#[test]
fn security_w3_u2_a_token_after_a_control_sequence_with_a_parameter_reaches_none_of_the_four() {
    // `ESC [ 1` is a control sequence without its final byte: the walk takes the token's first
    // character, `g`, as that byte, so the bare form holds `hp_`, the shown form `U+FFFD hp_`,
    // and the dropped form `[1ghp_`, whose `g` follows the digit `1` and starts no word. No form
    // holds the prefix and the token's secret is kept, as for the lone `ESC` the story closed.
    let stderr = format!(
        "starting\ndebug: value=\u{1b}[1{}{SECRET}\n{REFUSED}\n",
        github_prefix()
    );
    let outcome = session("csi-parameter", &stderr);
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

#[test]
fn security_w3_u2_a_url_followed_by_a_private_mode_sequence_is_kept_as_shown() {
    // `ESC [ ? 25 h` shows the cursor again, as a spinner does when it ends. It is shown as one
    // U+FFFD, so the line as every output keeps it holds a URL without a query; the dropped form
    // reads the sequence's `?` as part of the URL's word and withholds the line.
    let stderr = format!("starting\nfetched https://docs.example/guide\u{1b}[?25h\n{REFUSED}\n");
    let outcome = session("private-mode", &stderr);
    let shown = format!("starting\nfetched https://docs.example/guide\u{fffd}\n{REFUSED}");
    assert_eq!(
        tail_of(&outcome.why),
        shown,
        "the line shows no credential's shape and is kept"
    );
}

#[test]
fn security_w3_u2_a_hyperlink_whose_hidden_target_holds_a_query_is_kept_as_shown() {
    // An OSC 8 hyperlink: its target is a string sequence, shown as one U+FFFD and held by no
    // output. The dropped form reads the target as text, finds a URL with a query in it, and
    // withholds a line whose shown form holds none.
    let stderr = format!(
        "starting\nsee \u{1b}]8;;https://docs.example/e?code=7\u{1b}\\the guide\u{1b}]8;;\u{1b}\\ \
         for more\n{REFUSED}\n"
    );
    let outcome = session("hyperlink", &stderr);
    let shown = format!("starting\nsee \u{fffd}the guide\u{fffd} for more\n{REFUSED}");
    assert_eq!(
        tail_of(&outcome.why),
        shown,
        "the line shows no credential's shape and is kept"
    );
}

#[test]
fn security_w3_u2_ordinary_coloured_log_lines_are_kept_as_shown() {
    let raw = [
        "\u{1b}[2m2026-11-05T10:00:00Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m loom: fetched \
         \u{1b}[4mhttps://docs.example/guide\u{1b}[0m",
        "\u{1b}[?25l",
        "\u{1b}[2K\rdownloading 3 of 7 \u{1b}[1;33m42%\u{1b}[0m",
        "\u{1b}(B\u{1b}[mdone in 3.2s",
        "\u{1b}[33mcommit 1a2b3c4\u{1b}[m (HEAD -> main)",
        "\u{1b}[1mbuild-01\u{1b}[0m:\u{1b}[34m~/work\u{1b}[0m$ cargo test",
        "\u{1b}7\u{1b}[1;1Hstatus: ok\u{1b}8",
        "\u{1b}]0;loom session\u{7}ready",
        "\u{1b}[?25h",
    ];
    let shown = [
        "\u{fffd}2026-11-05T10:00:00Z\u{fffd} \u{fffd} INFO\u{fffd} loom: fetched \
         \u{fffd}https://docs.example/guide\u{fffd}",
        "\u{fffd}",
        "\u{fffd}\u{fffd}downloading 3 of 7 \u{fffd}42%\u{fffd}",
        "\u{fffd}\u{fffd}done in 3.2s",
        "\u{fffd}commit 1a2b3c4\u{fffd} (HEAD -> main)",
        "\u{fffd}build-01\u{fffd}:\u{fffd}~/work\u{fffd}$ cargo test",
        "\u{fffd}\u{fffd}status: ok\u{fffd}",
        "\u{fffd}ready",
        "\u{fffd}",
    ];
    let stderr = format!("starting\n{}\n{REFUSED}\n", raw.join("\n"));
    let expected = format!("starting\n{}\n{REFUSED}", shown.join("\n"));
    let outcome = session("coloured", &stderr);
    assert_eq!(
        tail_of(&outcome.why),
        expected,
        "every line is kept as shown"
    );
    assert_eq!(
        stderr_warning(&outcome.transcript),
        format!("{SAID}{expected}"),
        "and so is the warning"
    );
}

#[test]
fn security_w3_u2_a_block_opened_before_the_window_and_closed_inside_it_is_withheld() {
    // The opening falls before the window, the `-----END` inside it, and lines follow it: the
    // window's block lines are withheld under one note, and what follows the block is kept.
    let mut stderr = format!("starting\n{} PRIVATE KEY-----\n", pem_begin());
    for at in 0..120 {
        stderr.push_str(&format!("MIIEvQ{SECRET}{at:040}\n"));
    }
    stderr.push_str(&["-----END ", "PRIVATE", " KEY-----\n"].concat());
    stderr.push_str(&format!("after the block\n{REFUSED}\n"));
    let outcome = session("closed-in-window", &stderr);
    let tail = tail_of(&outcome.why);
    assert!(!tail.contains(SECRET), "the tail holds the block: {tail:?}");
    assert!(
        tail.ends_with(&format!(
            "they hold a credential's shape]\nafter the block\n{REFUSED}"
        )),
        "the block is one note and what follows it is kept: {tail:?}"
    );
    assert_eq!(tail.split('\n').count(), 3, "and nothing else: {tail:?}");
    assert_eq!(outcome.record_tail.as_deref(), Some(tail));
}

/// What a property line is.
#[derive(Clone, Copy, PartialEq)]
enum Draw {
    Plain,
    Shown,
    Token,
    Escaped,
    Begin,
    Body,
    End,
}

#[test]
fn security_w3_u2_every_line_the_tail_keeps_the_whole_walk_keeps() {
    let prefixes: Vec<&str> = TOKEN_PREFIXES
        .iter()
        .copied()
        .filter(|prefix| !prefix.starts_with('-'))
        .collect();
    // Escape sequences each of which the three forms together see through to a prefix after it.
    let escapes = ["\u{1b}", "\u{1b}[", "\u{1b}[0m", "\u{1b}(", "\u{9b}", "\u{7}"];
    let end = ["-----END ", "PRIVATE", " KEY-----"].concat();
    let mut state: u64 = 0x5eed_0302;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as usize
    };
    for case in 0..48 {
        let count = 20 + next() % 180;
        let mut lines: Vec<String> = vec![format!("line {case}-start ok")];
        let mut at = 0;
        while at < count {
            let draw = match next() % 20 {
                0..=6 => Draw::Plain,
                7..=9 => Draw::Shown,
                10 | 11 => Draw::Token,
                12 | 13 => Draw::Escaped,
                14 => Draw::Begin,
                15..=18 => Draw::Body,
                _ => Draw::End,
            };
            let run = if draw == Draw::Body { 1 + next() % 60 } else { 1 };
            for _ in 0..run {
                let pad = "p".repeat(next() % 120);
                let ending = if next() % 4 == 0 { "\r" } else { "" };
                lines.push(match draw {
                    Draw::Plain => format!("line {case}-{at} ok {pad}{ending}"),
                    Draw::Shown => format!(
                        "line {case}-{at} {}\t\u{7}\u{0}é{pad}{ending}",
                        "é\u{1}".repeat(next() % 40)
                    ),
                    Draw::Token => format!(
                        "line {case}-{at} value={}{SECRET}{ending}",
                        prefixes[next() % prefixes.len()]
                    ),
                    Draw::Escaped => format!(
                        "line {case}-{at} value={}{}{SECRET}{ending}",
                        escapes[next() % escapes.len()],
                        prefixes[next() % prefixes.len()]
                    ),
                    Draw::Begin => format!("{} PRIVATE KEY-----{ending}", pem_begin()),
                    Draw::Body => format!("MIIEvQ{case}x{at}{pad}{ending}"),
                    Draw::End => format!("{end}{ending}"),
                });
                at += 1;
            }
        }
        lines.push(REFUSED.to_owned());
        let stderr = format!("{}\n", lines.join("\n"));
        let outcome = session(&format!("property-{case}"), &stderr);
        let warning = stderr_warning(&outcome.transcript);
        let walked = warning
            .strip_prefix(SAID)
            .unwrap_or_else(|| panic!("case {case}: the warning says {warning:?}"));
        let kept: Vec<&str> = walked.split('\n').filter(|line| !is_note(line)).collect();
        let tail = tail_of(&outcome.why);
        let tail_kept: Vec<&str> = tail.split('\n').filter(|line| !is_note(line)).collect();

        for (what, text) in [
            ("transcript", outcome.transcript.as_str()),
            ("observed", &outcome.observed.join("\n")),
            ("error", outcome.why.as_str()),
            ("record tail", outcome.record_tail.as_deref().unwrap_or("")),
        ] {
            assert!(
                !text.contains(SECRET),
                "case {case}: the {what} holds a token's secret, of {stderr:?}"
            );
        }
        for line in &tail_kept {
            assert!(
                kept.contains(line),
                "case {case}: the tail keeps {line:?}, which the whole walk withholds, of \
                 {stderr:?}"
            );
        }
        assert!(
            kept.ends_with(&tail_kept),
            "case {case}: the tail's lines are the whole walk's last ones: {tail_kept:?} of \
             {kept:?}"
        );
        assert!(
            tail.len() <= STDERR_TAIL_BYTES,
            "case {case}: a tail of {} bytes",
            tail.len()
        );
        assert_eq!(
            outcome.record_tail.as_deref(),
            Some(tail),
            "case {case}: the record keeps the error's tail"
        );
    }
}
