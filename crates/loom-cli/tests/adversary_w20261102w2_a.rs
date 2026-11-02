//! Adversary cases for `story:agent-run-once` (wave 2026-11-02-w2, unit A): `b10x-loom agent run
//! --spec <FILE> --state <DIR> --input <FILE>` held to its acceptance at the edges the unit's own
//! cases (`agent_run.rs`) leave open.
//!
//! - `--input` that is a named pipe, a symbolic link (to a regular file, to a directory, dangling),
//!   exactly 1048576 bytes and one byte more, not UTF-8, only Unicode whitespace, or unreadable;
//! - a spec with `expose.a2a`, and one with a channel whose binding declares no write;
//! - the state directory held by a running `agent serve`;
//! - runs inside the hour against `writes.max_per_hour: 2`, and a write Connectors refuses;
//! - the budgets of a turn that can send;
//! - the exit status of an unclassified item, an intent no state admits and a declined item;
//! - the record line and the state's handled item.
//!
//! Every case runs the binary, as `agent_run.rs` does, under a bound of 120 s so a run that waits
//! on its input fails instead of hanging the suite. The models are the fixture catalog's anonymous
//! routes at a loopback socket this file opens; it answers each request with the next recorded
//! response and counts every request, so "no model call" is a count of zero. No case reaches the
//! network, a credential or a Codex login.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/catalog-route/catalog.toml");
const CLASSIFIER: &str = "example/Classifier-Model";
const AGENT: &str = "example/Agent-Model";
const CHANNEL: &str = "C0FIXTURE1";
const WRITE: &str = "message.post";
const TASK: &str = "Summarise what the runbooks say about the deploy key.";
const REPLY: &str = "The runbook on rotating the deploy key is the one to follow.";
const A2A_TOKEN: &str = "adversary-a2a-bearer-9d41";
/// The cap `agent run --help` states for `--input`.
const MAX_INPUT: usize = 1_048_576;

/// The classifier's answer: `intent` at `confidence`.
fn classified(intent: &str, confidence: f64) -> Vec<u8> {
    sse(
        CLASSIFIER,
        "classify_item",
        json!({"intent": intent, "hints": ["docs"], "confidence": confidence}),
    )
}

fn select(action: &str) -> Vec<u8> {
    sse(AGENT, "select_action", json!({ "action": action }))
}

/// The responses of one turn that reads, proposes and, when `send` is true, sends the proposal
/// to the channel (as `agent_run.rs` records them).
fn answered(send: bool) -> Vec<Vec<u8>> {
    let mut responses = vec![
        classified("ask", 0.9),
        select("source.read"),
        sse(
            AGENT,
            "action_arguments",
            json!({"source": "docs", "kind": "search", "input": {"query": "deploy key"}}),
        ),
        select("reply.propose"),
        sse(AGENT, "action_arguments", json!({"text": REPLY})),
    ];
    if send {
        responses.push(select("reply.send"));
        responses.push(sse(
            AGENT,
            "action_arguments",
            json!({"channel": CHANNEL, "text": REPLY}),
        ));
    }
    responses
}

// ---------------------------------------------------------------------------------------------
// --input at its edges: every refusal exits 1 naming the file, before any model call and before
// the state directory is made (`agent-run-once/input-missing`).

/// Asserts the refusal of an input: exit 1, the path named, nothing printed, no model asked, no
/// state directory.
fn assert_input_refused(scratch: &Scratch, input: &Path, case: &str) {
    let models = Models::new(Vec::new());

    let ran = scratch.run(&models.base_url, Some(input));

    assert!(!ran.timed_out, "{case}: the run did not end in 120 s");
    assert_eq!(ran.code, Some(1), "{case}: {}\n{}", ran.stdout, ran.stderr);
    let named = input.display().to_string();
    assert!(
        ran.stderr.contains(&named),
        "{case}: names {named}: {}",
        ran.stderr
    );
    assert!(ran.stdout.is_empty(), "{case}: {}", ran.stdout);
    assert!(models.finish().is_empty(), "{case}: no model was called");
    assert!(!scratch.state().exists(), "{case}: no state directory");
}

/// A named pipe nobody writes is refused at once: the run neither waits for a writer nor reads.
#[test]
fn adversary_fifo_input_is_refused_without_waiting_for_a_writer() {
    let scratch = Scratch::new("fifo", &Spec::default());
    let fifo = scratch.root.join("task.fifo");
    let made = Command::new("/usr/bin/mkfifo").arg(&fifo).status().unwrap();
    assert!(made.success(), "mkfifo");

    assert_input_refused(&scratch, &fifo, "fifo");
}

/// A symbolic link that dangles, and one to a directory, are refused naming the link.
#[test]
fn adversary_symlinked_input_dangling_or_to_a_directory_is_refused() {
    let scratch = Scratch::new("links-refused", &Spec::default());
    let dangling = scratch.root.join("dangling.txt");
    std::os::unix::fs::symlink(scratch.root.join("no-such-task.txt"), &dangling).unwrap();
    let to_directory = scratch.root.join("to-directory.txt");
    std::os::unix::fs::symlink(scratch.root.join("home"), &to_directory).unwrap();

    assert_input_refused(&scratch, &dangling, "dangling");
    assert_input_refused(&scratch, &to_directory, "to a directory");
}

/// A symbolic link to a regular file is followed (`regular_file::open`): its target's text is the
/// item and the run answers.
#[test]
fn adversary_symlinked_input_to_a_regular_file_runs() {
    let scratch = Scratch::new("link-runs", &Spec::default());
    let target = scratch.input(TASK);
    let link = scratch.root.join("task-link.txt");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&link));

    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    let requests = models.finish();
    assert_eq!(requests.len(), 5, "classified, read, proposed");
    assert!(requests[0].contains(TASK), "{}", requests[0]);
}

/// A file of exactly 1048576 bytes is within the cap: it is read whole, its last word reaches the
/// classifier, and the turn answers.
#[test]
fn adversary_input_of_exactly_the_cap_runs_whole() {
    let scratch = Scratch::new("at-cap", &Spec::default());
    let tail = " END-OF-TASK-7f3a";
    let head = format!("{TASK} ");
    let filler = "x".repeat(MAX_INPUT - head.len() - tail.len());
    let text = format!("{head}{filler}{tail}");
    assert_eq!(text.len(), MAX_INPUT);
    let input = scratch.root.join("task.txt");
    fs::write(&input, &text).unwrap();
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&input));

    assert!(!ran.timed_out, "the run did not end in 120 s");
    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    let requests = models.finish();
    assert_eq!(requests.len(), 5, "classified, read, proposed");
    assert!(
        requests[0].contains("END-OF-TASK-7f3a"),
        "the whole text reaches the classifier"
    );
    assert_eq!(scratch.record().len(), 1);
}

/// One byte more than 1048576 is refused naming the file and the cap.
#[test]
fn adversary_input_one_byte_over_the_cap_is_refused() {
    let scratch = Scratch::new("over-cap", &Spec::default());
    let input = scratch.root.join("task.txt");
    fs::write(&input, "a".repeat(MAX_INPUT + 1)).unwrap();
    let models = Models::new(Vec::new());

    let ran = scratch.run(&models.base_url, Some(&input));

    assert_eq!(ran.code, Some(1), "{}\n{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains(&input.display().to_string()),
        "{}",
        ran.stderr
    );
    assert!(
        ran.stderr.contains("1048576"),
        "names the cap: {}",
        ran.stderr
    );
    assert!(models.finish().is_empty(), "no model was called");
    assert!(!scratch.state().exists(), "no state directory");
}

/// Bytes that are not UTF-8 are refused naming the file.
#[test]
fn adversary_input_that_is_not_utf8_is_refused() {
    let scratch = Scratch::new("not-utf8", &Spec::default());
    let input = scratch.root.join("task.txt");
    fs::write(&input, b"Summarise \xff\xfe the runbooks.\n").unwrap();

    assert_input_refused(&scratch, &input, "not UTF-8");
}

/// A text of Unicode whitespace only (no-break space, em space, ideographic space, line
/// separator, vertical tab, form feed) holds no text.
#[test]
fn adversary_input_of_unicode_whitespace_only_is_refused() {
    let scratch = Scratch::new("unicode-blank", &Spec::default());
    let input = scratch.input("\u{00A0}\u{2003}\u{3000}\u{2028}\r\n\t\u{000B}\u{000C}");

    assert_input_refused(&scratch, &input, "unicode whitespace");
}

/// A regular file the process may not read is refused naming the file.
#[test]
fn adversary_input_without_read_permission_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("no-read", &Spec::default());
    let input = scratch.input(TASK);
    fs::set_permissions(&input, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&input).is_ok() {
        // A process that reads it anyway (root) cannot show the refusal.
        return;
    }

    assert_input_refused(&scratch, &input, "no read permission");
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
}

// ---------------------------------------------------------------------------------------------
// The spec: `expose.a2a`, and a channel without a write.

/// A spec exposing A2A runs one turn and binds nothing: its port, held by this test for the whole
/// run, stays this test's, and no A2A bearer is needed.
#[test]
fn adversary_a_spec_exposing_a2a_runs_once_without_binding_its_port() {
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    held.set_nonblocking(true).unwrap();
    let port = held.local_addr().unwrap().port();
    let scratch = Scratch::new(
        "a2a-spec",
        &Spec {
            expose_port: Some(port),
            ..Spec::default()
        },
    );
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    assert_eq!(ran.stdout.lines().count(), 1, "{}", ran.stdout);
    assert_eq!(models.finish().len(), 5);
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "proposed", "{record:?}");
    assert!(
        matches!(held.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock),
        "nothing connected to the exposed port"
    );
}

/// A spec whose channel binding declares no write answers as a turn that cannot send: it proposes,
/// exits 0, and nothing is written.
#[test]
fn adversary_a_channel_without_a_write_answers_and_sends_nothing() {
    let scratch = Scratch::new(
        "channel-no-write",
        &Spec {
            chat: Some(false),
            ..Spec::default()
        },
    );
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    assert_eq!(ran.stdout.lines().count(), 1, "{}", ran.stdout);
    assert_eq!(models.finish().len(), 5);
    assert!(scratch.invoked(WRITE).is_empty(), "nothing is sent");
    assert!(!scratch.state().join("sends.jsonl").exists());
    assert_eq!(scratch.record()[0]["outcome"], "proposed");
}

// ---------------------------------------------------------------------------------------------
// The state directory held by a running `agent serve`.

/// While `agent serve` holds the state directory, a run on it is refused with exit 1 naming the
/// directory, before any model call, and records nothing.
#[test]
fn adversary_a_state_held_by_a_running_serve_refuses_the_run_before_any_model_call() {
    let port = free_port();
    let scratch = Scratch::new(
        "held",
        &Spec {
            expose_port: Some(port),
            ..Spec::default()
        },
    );
    let serving_models = Models::new(Vec::new());
    let catalog = scratch.root.join("serve-catalog.toml");
    fs::write(
        &catalog,
        CATALOG.replace("http://127.0.0.1:1/v1", &serving_models.base_url),
    )
    .unwrap();
    let mut serve = scratch.command();
    serve
        .args(["agent", "serve", "--spec"])
        .arg(scratch.spec())
        .arg("--state")
        .arg(scratch.state())
        .arg("--catalog")
        .arg(&catalog)
        .args([
            "--classifier-model",
            "lab-classifier",
            "--model",
            "lab-agent",
            "--a2a-address",
            "127.0.0.1",
        ])
        .env("LOOM_A2A_BEARER", A2A_TOKEN)
        .stdin(Stdio::null())
        .stdout(fs::File::create(scratch.root.join("serve.out")).unwrap())
        .stderr(fs::File::create(scratch.root.join("serve.err")).unwrap());
    let mut host = Reaped(serve.spawn().expect("start agent serve"));
    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready(port) {
        assert!(
            host.0.try_wait().unwrap().is_none(),
            "agent serve ended: {}",
            fs::read_to_string(scratch.root.join("serve.err")).unwrap_or_default()
        );
        assert!(Instant::now() < deadline, "agent serve not ready in 60 s");
        std::thread::sleep(Duration::from_millis(50));
    }
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(1), "{}\n{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("held by another host"),
        "{}",
        ran.stderr
    );
    assert!(
        ran.stderr.contains(&scratch.state().display().to_string()),
        "names the directory: {}",
        ran.stderr
    );
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    assert!(models.finish().is_empty(), "no model was called");
    assert!(scratch.record().is_empty(), "nothing is recorded");
    assert!(
        host.0.try_wait().unwrap().is_none(),
        "the serving host still runs"
    );
    drop(host);
    assert!(serving_models.finish().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Sends: `writes.max_per_hour` across runs, a write Connectors refuses, a sending turn's budget.

/// With `writes: {mode: auto, max_per_hour: 2}` two runs inside the hour each send; the third is
/// refused `RateLimited`, recorded, and exits 3.
#[test]
fn adversary_max_per_hour_two_admits_two_runs_and_refuses_the_third() {
    let scratch = Scratch::new(
        "two-per-hour",
        &Spec {
            chat: Some(true),
            writes: Some(("auto", 2)),
            ..Spec::default()
        },
    );
    for (at, expected) in [(1, 0), (2, 0), (3, 3)] {
        let models = Models::new(answered(true));

        let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

        assert_eq!(
            ran.code,
            Some(expected),
            "run {at}: {}\n{}",
            ran.stdout,
            ran.stderr
        );
        assert_eq!(models.finish().len(), 7, "run {at}");
    }
    assert_eq!(scratch.invoked(WRITE).len(), 2, "two writes");
    let sends = fs::read_to_string(scratch.state().join("sends.jsonl")).unwrap();
    assert_eq!(sends.lines().count(), 2, "{sends}");
    let record = scratch.record();
    assert_eq!(record.len(), 3, "{record:?}");
    assert!(
        record[2]["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("RateLimited")),
        "{record:?}"
    );
}

/// A write Connectors refuses (`mutation.classification: refused`, and `not_attempted` as it
/// answers a write without a valid approval) is a final refusal of the send, as
/// `loom.agent.RunOnce` states ("`send_refused` ... or Connectors refused the write") and
/// `agent run --help` states (exit 3 naming "the write's refusal"): the proposal is recorded
/// `not sent`, nothing is counted against `max_per_hour`, and the run exits 3 naming
/// `SendRefused`. The fake answers on stdout with exit status 0; `classified` in
/// `loom-connectors/src/cli/write.rs` reads the same classification from `error.data` either way.
#[test]
fn adversary_a_write_connectors_refuses_exits_3_and_counts_nothing() {
    for fixture in [
        "invoke-route-refused.json",
        "invoke-route-not-attempted.json",
    ] {
        let scratch = Scratch::new(
            "write-refused",
            &Spec {
                chat: Some(true),
                writes: Some(("auto", 5)),
                ..Spec::default()
            },
        );
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        fs::copy(
            crates
                .join("loom-connectors/tests/fixtures/connectors")
                .join(fixture),
            scratch
                .root
                .join(format!("connectors-state/invoke.{WRITE}.json")),
        )
        .unwrap();
        let models = Models::new(answered(true));

        let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

        assert_eq!(models.finish().len(), 7, "{fixture}");
        assert_eq!(
            scratch.invoked(WRITE).len(),
            1,
            "{fixture}: one write invoked"
        );
        let record = scratch.record();
        assert_eq!(record.len(), 1, "{fixture}: {record:?}");
        let detail = record[0]["detail"].as_str().unwrap_or_default().to_owned();
        let counted = fs::read_to_string(scratch.state().join("sends.jsonl"))
            .map(|sends| sends.lines().count())
            .unwrap_or(0);
        assert_eq!(
            (
                ran.code,
                ran.stderr.contains("SendRefused"),
                detail.starts_with("not sent: "),
                counted,
            ),
            (Some(3), true, true, 0),
            "{fixture}: (exit, stderr names SendRefused, recorded not sent, sends counted)\n\
             detail: {detail}\nstdout: {}\nstderr: {}",
            ran.stdout,
            ran.stderr
        );
    }
}

/// A turn that can send stops at `model_budget` as a served turn does: recorded `stopped` naming
/// the budget, no proposal printed, nothing sent, exit 4.
#[test]
fn adversary_a_sending_turn_stops_at_its_model_budget_and_sends_nothing() {
    let scratch = Scratch::new(
        "send-budget",
        &Spec {
            chat: Some(true),
            writes: Some(("auto", 5)),
            model_budget: 2,
            ..Spec::default()
        },
    );
    let mut responses = answered(true);
    responses.truncate(3);
    let models = Models::new(responses);

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(4), "{}\n{}", ran.stdout, ran.stderr);
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    assert_eq!(models.finish().len(), 3);
    assert!(scratch.invoked(WRITE).is_empty(), "nothing is sent");
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "stopped", "{record:?}");
    assert_eq!(
        record[0]["detail"], "the turn used its 2 model calls",
        "{record:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// The exit status of each way a turn ends.

/// An item the classifier is unsure of is recorded `unclassified`; no turn runs, nothing is
/// printed, and the run exits 4 saying so.
#[test]
fn adversary_an_unclassified_task_exits_4_recorded_unclassified() {
    let scratch = Scratch::new("unclassified", &Spec::default());
    let models = Models::new(vec![classified("ask", 0.1)]);

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(4), "{}\n{}", ran.stdout, ran.stderr);
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    assert!(ran.stderr.contains("unclassified"), "{}", ran.stderr);
    assert_eq!(models.finish().len(), 1, "the classifier only");
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "unclassified", "{record:?}");
}

/// An intent no state's `when` names stops the turn before any step: recorded `stopped`, exit 4.
#[test]
fn adversary_an_intent_no_state_admits_exits_4_without_a_turn() {
    let scratch = Scratch::new("no-state", &Spec::default());
    let models = Models::new(vec![classified("find", 0.9)]);

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(4), "{}\n{}", ran.stdout, ran.stderr);
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    assert!(ran.stderr.contains("stopped"), "{}", ran.stderr);
    assert_eq!(models.finish().len(), 1, "the classifier only");
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "stopped", "{record:?}");
}

/// A declined item is an answer: recorded `declined`, no proposal line, exit 0.
#[test]
fn adversary_a_declined_task_exits_0_and_prints_nothing() {
    let scratch = Scratch::new("declined", &Spec::default());
    let models = Models::new(vec![
        classified("task", 0.9),
        select("reply.decline"),
        sse(
            AGENT,
            "action_arguments",
            json!({"reason": "a task is handed to a person"}),
        ),
    ]);

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    assert!(ran.stdout.is_empty(), "no proposal: {}", ran.stdout);
    assert_eq!(models.finish().len(), 3);
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(record[0]["outcome"], "declined", "{record:?}");
}

// ---------------------------------------------------------------------------------------------
// The record: the line a handled item gets, and the item kept as handled.

/// The run's line is the record line of a handled item, each member the turn's: the item the
/// printed line names, its intent and confidence, the outcome, the read, the proposal; and the
/// state keeps the item as handled.
#[test]
fn adversary_the_record_line_is_the_one_a_handled_item_gets() {
    let scratch = Scratch::new("record-line", &Spec::default());
    let models = Models::new(answered(false));

    let ran = scratch.run(&models.base_url, Some(&scratch.input(TASK)));

    assert_eq!(ran.code, Some(0), "{}\n{}", ran.stdout, ran.stderr);
    models.finish();
    let item = ran
        .stdout
        .split_whitespace()
        .next()
        .expect("a printed line")
        .to_owned();
    assert!(item.starts_with("run-"), "{item}");
    let record = scratch.record();
    assert_eq!(record.len(), 1, "{record:?}");
    assert_eq!(
        record[0],
        json!({
            "item": item,
            "intent": "ask",
            "confidence": 0.9,
            "outcome": "proposed",
            "reads": [{"source": "docs", "kind": "search"}],
            "proposal": REPLY,
        }),
    );
    let state: Value =
        serde_json::from_str(&fs::read_to_string(scratch.state().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(state["handled"], json!([item]), "{state}");
    assert_eq!(state["failing"], json!([]), "{state}");
}

// ---------------------------------------------------------------------------------------------
// Fixtures.

/// What the spec of a case declares beyond the common part.
struct Spec {
    /// A channel binding allowing [`CHANNEL`], `Some(true)` with a `write`.
    chat: Option<bool>,
    /// `writes: {mode, max_per_hour}`; with a write, `reply.send` joins the actions and the
    /// capabilities.
    writes: Option<(&'static str, i64)>,
    /// `expose.a2a` on this port.
    expose_port: Option<u16>,
    step_budget: i64,
    model_budget: i64,
}

impl Default for Spec {
    fn default() -> Self {
        Self {
            chat: None,
            writes: None,
            expose_port: None,
            step_budget: 8,
            model_budget: 16,
        }
    }
}

/// How one run ended: its exit status (none when killed), whether the bound killed it, and what it
/// printed.
struct Ran {
    code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

static RUNS: AtomicU64 = AtomicU64::new(0);

/// A scratch directory under `CARGO_TARGET_TMPDIR`, as `agent_run.rs` builds it; removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str, spec: &Spec) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary-run-{label}-{}-{}",
            std::process::id(),
            now.as_nanos()
        ));
        for directory in ["home", "connectors-state"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let fake = root.join("connectors");
        install_program(
            &crates.join("loom-connectors/tests/fixtures/connectors/fake-connectors"),
            &fake,
        );
        let connectors_state = root.join("connectors-state");
        let fixtures = crates.join("loom-plugin-slack/tests/fixtures");
        for verb in ["describe", "invoke"] {
            fs::copy(
                fixtures.join(format!("{verb}-docs.search.json")),
                connectors_state.join(format!("{verb}.docs.search.json")),
            )
            .unwrap();
        }
        let writes = crates.join("loom-connectors/tests/fixtures/connectors");
        let describe = fs::read_to_string(writes.join("describe-write.json")).unwrap();
        fs::write(
            connectors_state.join(format!("describe.{WRITE}.json")),
            describe.replace("\"adapter\":\"chat\"", "\"adapter\":\"slack\""),
        )
        .unwrap();
        fs::copy(
            writes.join("invoke-route-applied.json"),
            connectors_state.join(format!("invoke.{WRITE}.json")),
        )
        .unwrap();
        let scratch = Self { root };
        let sends = spec.chat == Some(true) && spec.writes.is_some();
        let (actions, capabilities) = if sends {
            (
                "[source.read, reply.propose, reply.send, reply.decline]",
                "[datasource.read, reply.propose, reply.send]",
            )
        } else {
            (
                "[source.read, reply.propose, reply.decline]",
                "[datasource.read, reply.propose]",
            )
        };
        let (chat, channel) = match spec.chat {
            Some(write) => (
                format!(
                    "  - {{name: chat, kind: connectors, adapter: slack, connection: slack-fixture, list: conversations.list, get: conversations.history, search: conversations.replies{}}}\n",
                    if write {
                        format!(", write: {WRITE}")
                    } else {
                        String::new()
                    }
                ),
                format!("channel: {{binding: chat, allow: [{CHANNEL}], bot_user_id: U0BOT}}\n"),
            ),
            None => (String::new(), String::new()),
        };
        let writes = spec
            .writes
            .map(|(mode, max)| format!("writes: {{mode: {mode}, max_per_hour: {max}}}\n"))
            .unwrap_or_default();
        let expose = spec
            .expose_port
            .map(|port| {
                format!(
                    "expose:\n  a2a: {{port: {port}, path: /a2a, card: {{name: test-operator, description: Answers about the deploy, skills: [logs]}}}}\n"
                )
            })
            .unwrap_or_default();
        let text = format!(
            "format: loom.agent/1
agent: {{id: test-operator, name: Test operator, brief: Answer questions about the deploy.}}
model:
  routes: routes.yaml
  roles: [{{role: agent, model: small}}, {{role: classifier, model: small}}]
loops:
  outer: {{poll_interval_seconds: 60, sources: [docs], min_age_minutes: 0, lookback_minutes: 120}}
  inner: {{protocol: operator-reply@1, step_budget: {step_budget}, model_budget: {model_budget}, classify_threshold: 0.5}}
states:
  - {{name: answer, when: {{intent: Ask}}, actions: {actions}, sources: [docs]}}
  - {{name: hand-off, when: {{intent: Task}}, actions: [reply.decline], sources: []}}
bindings:
{chat}  - {{name: docs, kind: connectors, adapter: docs, connection: docs-fixture, search: docs.search}}
{channel}{writes}policy: {{capabilities: {capabilities}}}
{expose}runtime:
  state_dir: {state}
  connectors: {{program: {program}, state_dir: {connectors}}}
  confinement: none
",
            step_budget = spec.step_budget,
            model_budget = spec.model_budget,
            state = scratch.state().display(),
            program = fake.display(),
            connectors = connectors_state.display(),
        );
        fs::write(scratch.spec(), text).unwrap();
        scratch
    }

    fn spec(&self) -> PathBuf {
        self.root.join("agent.yaml")
    }

    fn state(&self) -> PathBuf {
        self.root.join("agent-state")
    }

    /// A task file holding `text`.
    fn input(&self, text: &str) -> PathBuf {
        let path = self.root.join("task.txt");
        fs::write(&path, text).unwrap();
        path
    }

    fn command(&self) -> Command {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_b10x-loom"));
        command
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("XDG_DATA_HOME", &home)
            .env_remove("RUST_LOG")
            .env_remove("LOOM_HTTP_CA_FILE")
            .env_remove("LOOM_A2A_BEARER");
        command
    }

    /// `agent run` with the fixture catalog at `base_url` and `input` as `--input`, bounded at
    /// 120 s.
    fn run(&self, base_url: &str, input: Option<&Path>) -> Ran {
        let catalog = self.root.join("catalog.toml");
        fs::write(&catalog, CATALOG.replace("http://127.0.0.1:1/v1", base_url)).unwrap();
        let mut command = self.command();
        command
            .args(["agent", "run", "--spec"])
            .arg(self.spec())
            .arg("--state")
            .arg(self.state())
            .arg("--catalog")
            .arg(&catalog)
            .args([
                "--classifier-model",
                "lab-classifier",
                "--model",
                "lab-agent",
            ]);
        if let Some(input) = input {
            command.arg("--input").arg(input);
        }
        self.bounded(command)
    }

    /// Runs `command` with its output in files beside the scratch, killing it after 120 s.
    fn bounded(&self, mut command: Command) -> Ran {
        let n = RUNS.fetch_add(1, Ordering::SeqCst);
        let out = self.root.join(format!("run-{n}.out"));
        let err = self.root.join(format!("run-{n}.err"));
        command
            .stdin(Stdio::null())
            .stdout(fs::File::create(&out).unwrap())
            .stderr(fs::File::create(&err).unwrap());
        let mut child = command.spawn().expect("start b10x-loom");
        let deadline = Instant::now() + Duration::from_secs(120);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        Ran {
            code: status.and_then(|status| status.code()),
            timed_out: status.is_none(),
            stdout: fs::read_to_string(&out).unwrap_or_default(),
            stderr: fs::read_to_string(&err).unwrap_or_default(),
        }
    }

    /// Every line of `record.jsonl`, as JSON.
    fn record(&self) -> Vec<Value> {
        fs::read_to_string(self.state().join("record.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("a record line is JSON"))
            .collect()
    }

    /// Every run of the fake, its arguments.
    fn argv(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.root.join("connectors-state/argv.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.trim_end_matches('\t')
                    .split('\t')
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    /// The invocations of `operation`: `invoke <connector>/<instance> <operation>`.
    fn invoked(&self, operation: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.iter()
                    .position(|argument| argument == "invoke")
                    .filter(|at| *at == 0 || argv[at - 1] != "operations")
                    .and_then(|at| argv.get(at + 2))
                    .is_some_and(|invoked| invoked == operation)
            })
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(
            self.root.join("task.txt"),
            fs::Permissions::from_mode(0o600),
        );
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A child killed and reaped on drop.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The loopback models: each request answered with the next recorded response (a 500 once they
/// run out), every request kept.
struct Models {
    base_url: String,
    done: Arc<AtomicBool>,
    handle: JoinHandle<Vec<String>>,
}

impl Models {
    fn new(responses: Vec<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut responses = responses.into_iter();
            loop {
                match listener.accept() {
                    Ok((socket, _)) => {
                        seen.push(answer(socket, responses.next()));
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if flag.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            }
            seen
        });
        Self {
            base_url,
            done,
            handle,
        }
    }

    /// Every request made, once the run that made them has ended.
    fn finish(self) -> Vec<String> {
        self.done.store(true, Ordering::SeqCst);
        self.handle.join().expect("the models finished")
    }
}

/// Reads one request from `socket` and answers it with `response`, or a 500 without one.
fn answer(mut socket: TcpStream, response: Option<Vec<u8>>) -> String {
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let read = match socket.read(&mut buffer) {
            Ok(read) => read,
            Err(_) => break,
        };
        request.extend_from_slice(&buffer[..read]);
        if read == 0 || complete(&request) {
            break;
        }
    }
    let answer = match response {
        Some(body) => {
            let mut answer = Vec::from(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            answer.extend_from_slice(&body);
            answer
        }
        None => Vec::from(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ),
    };
    let _ = socket.write_all(&answer);
    let _ = socket.shutdown(std::net::Shutdown::Both);
    String::from_utf8_lossy(&request).into_owned()
}

/// Whether `request` holds its whole body, by its `Content-Length`.
fn complete(request: &[u8]) -> bool {
    let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
    head.split_once("content-length:")
        .and_then(|(_, tail)| tail.split('\r').next())
        .and_then(|value| value.trim().parse::<usize>().ok())
        .is_some_and(|length| request.len() - end - 4 >= length)
}

/// A recorded Chat Completions stream: `model` calls `tool` with `arguments`.
fn sse(model: &str, tool: &str, arguments: Value) -> Vec<u8> {
    let chunk = |delta: Value, finish: Value| {
        json!({"id": format!("chatcmpl-{tool}"), "object": "chat.completion.chunk", "model": model,
               "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
    };
    let events = [
        chunk(
            json!({"role": "assistant", "tool_calls": [{"index": 0, "id": format!("call-{tool}"),
                   "type": "function", "function": {"name": tool, "arguments": ""}}]}),
            Value::Null,
        ),
        chunk(
            json!({"tool_calls": [{"index": 0, "function": {"arguments": arguments.to_string()}}]}),
            Value::Null,
        ),
        chunk(json!({}), json!("tool_calls")),
        json!({"id": format!("chatcmpl-{tool}"), "object": "chat.completion.chunk", "model": model,
               "choices": [], "usage": {"prompt_tokens": 120, "completion_tokens": 7, "total_tokens": 127}}),
    ];
    let mut stream = String::new();
    for event in events {
        stream.push_str(&format!("data: {event}\n\n"));
    }
    stream.push_str("data: [DONE]\n\n");
    stream.into_bytes()
}

/// A port nothing listens on now.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Whether the A2A listener on `port` answers `GET /ready` with 200.
fn ready(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let head = format!(
        "GET /ready HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return false;
    }
    let mut raw = String::new();
    let _ = stream.read_to_string(&mut raw);
    raw.starts_with("HTTP/1.1 200")
}

/// Installs `source` as the executable `target` through `install(1)`, in a child process.
fn install_program(source: &Path, target: &Path) {
    let status = Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
}
