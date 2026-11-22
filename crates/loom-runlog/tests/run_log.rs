//! The run log of a case (story `hash-chained-run-log`): one JSON line per event in
//! `runlog-<stem>.jsonl` beside the case file, each line naming the SHA-256 of the line before it,
//! appended atomically under the store's lock, and a reader that refuses a cut, shortened,
//! reordered or changed log, naming the first line that fails.
//!
//! Lines are counted from 0, as `seq` counts them: "line 3" is the fourth line of the file.

use std::collections::BTreeMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_runlog::{AppendStep, RunLog, RunLogError, encode, log_file_name, verify};
use loom::primitives::Timestamp;
use loom::runlog::{
    ActionFinished, ActionStarted, AuthorityRecorded, CaseOpened, ChildClosed, ChildOpened,
    Discarded, Evaluated, RevisionMove, RunLogEvent, RunLogLine, RunLogRefusal, RunLogRefused,
    Stopped, Terminated,
};
use loom_governor::{
    CaseIndependence, CaseInputs, CaseState, FallibleCaseStore as _, FileCaseStore, LockedDir,
};
use sha2::{Digest as _, Sha256};

const CASE: &str = "chg-1842";

/// The evidence id of a child's `case_outcome` record in these tests' child lines.
const RECORD: &str = "5842f400-803c-84aa-847f-3704886e183b";

/// The environment variable that makes the two-process test's binary an appender over the
/// directory it names.
const APPENDER_DIR: &str = "LOOM_RUNLOG_APPENDER_DIR";
/// The name each appender child writes into its events.
const APPENDER_NAME: &str = "LOOM_RUNLOG_APPENDER_NAME";

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("run-log")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("an earlier run's directory is removed");
    }
    dir
}

fn at(index: usize) -> Timestamp {
    Timestamp(format!(
        "2026-11-21T10:{:02}:{:02}Z",
        (index / 60) % 60,
        index % 60
    ))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Event `index`, cycling through every kind; each carries the marker `m<index>` in a member.
fn event(index: usize) -> RunLogEvent {
    let marker = format!("m{index:03}");
    match index % 10 {
        0 => RunLogEvent::CaseOpened(CaseOpened {
            case: format!("case-{marker}"),
            protocol: "software-change@1".to_owned(),
        }),
        1 => RunLogEvent::Evaluated(Evaluated {
            case_revision: i64::try_from(index).expect("small"),
            frontier: format!("frontier-{marker}"),
            decision_digest: sha256_hex(marker.as_bytes()),
        }),
        2 => RunLogEvent::ActionStarted(ActionStarted {
            action: format!("action-{marker}"),
        }),
        3 => RunLogEvent::ActionFinished(ActionFinished {
            action: format!("action-{marker}"),
            exit: Some(0),
            refusal: None,
            blob_digest: Some(sha256_hex(b"blob")),
            evidence: vec![
                "6f1c1a52-8d43-4a43-9b3e-0d4f5a1f7c11".to_owned(),
                format!("evidence-{marker}"),
            ],
            revisions: vec![
                RevisionMove {
                    artifact: "change".to_owned(),
                    from: None,
                    to: "r1".to_owned(),
                },
                RevisionMove {
                    artifact: "review".to_owned(),
                    from: Some("r1".to_owned()),
                    to: "r2".to_owned(),
                },
            ],
        }),
        4 => RunLogEvent::Discarded(Discarded {
            action: format!("action-{marker}"),
            reason: "a \"quoted\" reason\nover two lines, ünïcode and a tab\t".to_owned(),
        }),
        5 => RunLogEvent::ChildOpened(ChildOpened {
            child: format!("child-{marker}"),
            protocol: "review@1".to_owned(),
            record: (index % 20 == 5).then(|| RECORD.to_owned()),
            record_digest: (index % 20 == 5)
                .then(|| sha256_hex(format!("open {marker}").as_bytes())),
        }),
        6 => RunLogEvent::ChildClosed(ChildClosed {
            child: format!("child-{marker}"),
            outcome: (index % 20 == 6).then(|| "approved".to_owned()),
            record: (index % 20 == 6).then(|| RECORD.to_owned()),
            record_digest: (index % 20 == 6)
                .then(|| sha256_hex(format!("close {marker}").as_bytes())),
        }),
        7 => RunLogEvent::AuthorityRecorded(AuthorityRecorded {
            capability: "repository.merge".to_owned(),
            decision: format!("allow-{marker}"),
            principal: "human:operator".to_owned(),
        }),
        8 => RunLogEvent::Stopped(Stopped {
            stop: format!("approval-{marker}"),
            next: vec!["approve".to_owned(), "decline".to_owned()],
            budget: None,
        }),
        _ => RunLogEvent::Terminated(Terminated {
            reason: format!("completed-{marker}"),
        }),
    }
}

/// A log over a fresh directory holding `count` events of [`CASE`].
fn log_with(name: &str, count: usize) -> (RunLog, PathBuf) {
    let dir = store_dir(name);
    let log = RunLog::open(&dir).expect("the store opens");
    for index in 0..count {
        log.append(CASE, at(index), event(index))
            .expect("an append to a log that verifies is taken");
    }
    let path = log.path(CASE);
    (log, path)
}

/// The lines of `bytes`, each without its line feed; a last line without one is kept.
fn lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = bytes
        .split(|byte| *byte == b'\n')
        .map(<[u8]>::to_vec)
        .collect();
    if bytes.ends_with(b"\n") {
        lines.pop();
    }
    lines
}

/// The lines joined back, each ended with a line feed.
fn joined(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    bytes
}

fn refused(result: Result<Vec<RunLogLine>, RunLogRefused>) -> (RunLogRefusal, i64) {
    match result {
        Ok(lines) => panic!("a tampered log verified as {} lines", lines.len()),
        Err(refused) => {
            assert!(
                !refused.detail.is_empty(),
                "a refusal says what is wrong with the line"
            );
            (refused.refusal, refused.line)
        }
    }
}

fn temporaries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("the store directory reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with(".tmp-"))
        .collect()
}

/// `count` events appended to one log: one JSON line each, `seq` gapless from 0, each `prev` the
/// SHA-256 of the line before, read back and verified. Each append syncs, so the count sets the
/// time.
fn events_verify_with_a_gapless_seq(label: &str, count: usize) {
    let (log, path) = log_with(label, count);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(format!("runlog-case-{}.jsonl", hex(CASE.as_bytes())).as_str()),
        "the log sits beside the case file `case-<hex>.json` under the same stem"
    );
    let bytes = fs::read(&path).expect("the log file reads");
    let raw = lines(&bytes);
    assert_eq!(raw.len(), count, "one line per event");
    assert!(bytes.ends_with(b"\n"), "every line ends with a line feed");
    for (index, line) in raw.iter().enumerate() {
        let value: serde_json::Value =
            serde_json::from_slice(line).expect("each line is one JSON document");
        let object = value.as_object().expect("each line is a JSON object");
        assert_eq!(
            object.get("seq").and_then(serde_json::Value::as_u64),
            Some(u64::try_from(index).expect("small")),
            "seq is gapless from 0"
        );
        if index == 0 {
            assert!(
                !object.contains_key("prev"),
                "line 0 names no previous line"
            );
        } else {
            assert_eq!(
                object.get("prev").and_then(serde_json::Value::as_str),
                Some(sha256_hex(&raw[index - 1]).as_str()),
                "prev of line {index} is the SHA-256 of line {}",
                index - 1
            );
        }
    }

    let read = log.read(CASE).expect("the log verifies");
    let verified = verify(&bytes).expect("the bytes verify");
    assert_eq!(read, verified);
    assert_eq!(read.len(), count);
    for (index, line) in read.iter().enumerate() {
        assert_eq!(line.seq, i64::try_from(index).expect("small"));
        assert_eq!(line.at, at(index));
        assert_eq!(
            line.event,
            event(index),
            "line {index} holds the event appended"
        );
        let prev = (index > 0).then(|| sha256_hex(&raw[index - 1]));
        assert_eq!(line.prev, prev);
    }
}

/// [`events_verify_with_a_gapless_seq`] with 20 events.
#[test]
fn twenty_events_verify_with_a_gapless_seq_and_each_prev_the_digest_of_the_line_before() {
    events_verify_with_a_gapless_seq("twenty", 20);
}

/// [`events_verify_with_a_gapless_seq`] with 50 events, the count the case held before, in the
/// slow lane (`task slow`): about 8 s on a loaded host.
#[test]
fn slow_fifty_events_verify_with_a_gapless_seq_and_each_prev_the_digest_of_the_line_before() {
    events_verify_with_a_gapless_seq("fifty", 50);
}

#[test]
fn an_append_returns_the_line_it_wrote() {
    let (log, path) = log_with("returned", 2);
    let line = log
        .append(CASE, at(2), event(2))
        .expect("the append is taken");
    let bytes = fs::read(&path).expect("the log file reads");
    let raw = lines(&bytes);
    assert_eq!(line.seq, 2);
    assert_eq!(line.prev, Some(sha256_hex(&raw[1])));
    assert_eq!(line.event, event(2));
    assert_eq!(verify(&bytes).expect("verifies").last(), Some(&line));
}

#[test]
fn an_absent_log_reads_as_no_line_and_its_first_append_is_line_0() {
    let (log, path) = log_with("absent", 0);
    assert!(!path.exists(), "nothing is written before the first append");
    assert_eq!(log.read(CASE).expect("an absent log reads"), Vec::new());
    let line = log.append(CASE, at(0), event(0)).expect("the first append");
    assert_eq!((line.seq, line.prev), (0, None));
}

#[test]
fn a_changed_byte_in_line_3_is_refused_broken_chain_naming_line_4() {
    let (log, path) = log_with("changed-byte", 6);
    let original = lines(&fs::read(&path).expect("reads"));

    // A byte inside a member's value: line 3 still reads, and line 4 no longer names it.
    let mut changed = original.clone();
    let marker = changed[3]
        .windows(4)
        .position(|window| window == b"m003")
        .expect("line 3 carries its marker");
    changed[3][marker] = b'n';
    assert_eq!(
        refused(verify(&joined(&changed))),
        (RunLogRefusal::BrokenChain, 4)
    );
    fs::write(&path, joined(&changed)).expect("writes");
    match log.read(CASE) {
        Err(RunLogError::Refused(refused)) => {
            assert_eq!(
                (refused.refusal, refused.line),
                (RunLogRefusal::BrokenChain, 4)
            );
        }
        other => panic!("the reader took a changed log: {other:?}"),
    }

    // A byte that leaves line 3 no JSON at all: still line 4's broken chain.
    let mut unreadable = original.clone();
    let last = unreadable[3].len() - 1;
    unreadable[3][last] = b']';
    assert_eq!(
        refused(verify(&joined(&unreadable))),
        (RunLogRefusal::BrokenChain, 4)
    );
}

#[test]
fn deleting_line_3_is_refused_sequence_gap_naming_line_3() {
    let (log, path) = log_with("deleted-line", 6);
    let mut deleted = lines(&fs::read(&path).expect("reads"));
    deleted.remove(3);
    assert_eq!(
        refused(verify(&joined(&deleted))),
        (RunLogRefusal::SequenceGap, 3)
    );
    fs::write(&path, joined(&deleted)).expect("writes");
    assert!(matches!(
        log.read(CASE),
        Err(RunLogError::Refused(RunLogRefused {
            refusal: RunLogRefusal::SequenceGap,
            line: 3,
            ..
        }))
    ));
}

#[test]
fn swapping_lines_3_and_4_is_refused_sequence_gap_naming_line_3() {
    let (_log, path) = log_with("swapped-lines", 6);
    let mut swapped = lines(&fs::read(&path).expect("reads"));
    swapped.swap(3, 4);
    assert_eq!(
        refused(verify(&joined(&swapped))),
        (RunLogRefusal::SequenceGap, 3)
    );
}

#[test]
fn truncating_the_last_line_in_the_middle_is_refused_cut_line_naming_it() {
    let (log, path) = log_with("cut-line", 6);
    let bytes = fs::read(&path).expect("reads");
    let raw = lines(&bytes);
    let start = bytes.len() - raw[5].len() - 1;
    let cut = &bytes[..start + raw[5].len() / 2];
    assert_eq!(refused(verify(cut)), (RunLogRefusal::CutLine, 5));

    // A last line whole but for its line feed is cut too: an append ends every line with one.
    let unterminated = &bytes[..bytes.len() - 1];
    assert_eq!(refused(verify(unterminated)), (RunLogRefusal::CutLine, 5));

    fs::write(&path, cut).expect("writes");
    assert!(matches!(
        log.read(CASE),
        Err(RunLogError::Refused(RunLogRefused {
            refusal: RunLogRefusal::CutLine,
            line: 5,
            ..
        }))
    ));
}

#[test]
fn a_prev_on_line_0_or_none_on_a_later_line_is_a_broken_chain() {
    let (_log, path) = log_with("prev-presence", 3);
    let raw = lines(&fs::read(&path).expect("reads"));

    let mut first = raw.clone();
    let text = String::from_utf8(first[0].clone()).expect("utf-8");
    first[0] = text
        .replacen(
            "{\"seq\":0,",
            &format!("{{\"seq\":0,\"prev\":\"{}\",", sha256_hex(b"")),
            1,
        )
        .into_bytes();
    assert_eq!(
        refused(verify(&joined(&first))),
        (RunLogRefusal::BrokenChain, 0)
    );

    let mut later = raw.clone();
    let text = String::from_utf8(later[2].clone()).expect("utf-8");
    let prev = format!(",\"prev\":\"{}\"", sha256_hex(&raw[1]));
    assert!(text.contains(&prev), "line 2 names line 1");
    later[2] = text.replacen(&prev, "", 1).into_bytes();
    assert_eq!(
        refused(verify(&joined(&later))),
        (RunLogRefusal::BrokenChain, 2)
    );
}

#[test]
fn an_append_to_a_log_that_does_not_verify_is_refused_and_changes_no_byte() {
    let (log, path) = log_with("refused-append", 6);
    let bytes = fs::read(&path).expect("reads");
    let raw = lines(&bytes);

    let mut changed = raw.clone();
    let marker = changed[3]
        .windows(4)
        .position(|window| window == b"m003")
        .expect("marker");
    changed[3][marker] = b'n';
    let mut deleted = raw.clone();
    deleted.remove(3);
    let mut swapped = raw.clone();
    swapped.swap(3, 4);
    let start = bytes.len() - raw[5].len() - 1;
    let cut = bytes[..start + raw[5].len() / 2].to_vec();

    for (tampering, tampered, expected) in [
        (
            "a changed byte",
            joined(&changed),
            (RunLogRefusal::BrokenChain, 4),
        ),
        (
            "a deleted line",
            joined(&deleted),
            (RunLogRefusal::SequenceGap, 3),
        ),
        (
            "swapped lines",
            joined(&swapped),
            (RunLogRefusal::SequenceGap, 3),
        ),
        ("a cut last line", cut, (RunLogRefusal::CutLine, 5)),
    ] {
        fs::write(&path, &tampered).expect("writes");
        let reader = refused(verify(&tampered));
        assert_eq!(reader, expected, "{tampering}: the reader's refusal");
        match log.append(CASE, at(6), event(6)) {
            Err(RunLogError::Refused(refused)) => assert_eq!(
                (refused.refusal, refused.line),
                reader,
                "{tampering}: the append is refused with the reader's refusal"
            ),
            other => panic!("{tampering}: an append to a log that does not verify: {other:?}"),
        }
        assert_eq!(
            fs::read(&path).expect("reads"),
            tampered,
            "{tampering}: the refused append changed no byte"
        );
        assert!(
            temporaries(log.dir()).is_empty(),
            "{tampering}: the refused append left no temporary file"
        );
    }
}

/// Two processes over one store, each appending `per_process` lines: the log holds every line once,
/// each a whole JSON document, each process's in its order, and it verifies.
///
/// The test binary is each appender: `test` (the calling test's own name) runs again in a child
/// with [`APPENDER_DIR`] set, and that run appends instead of starting children. Both children
/// open the store and then wait for the parent's start file, so their appends overlap from the
/// first one on, whatever the count.
fn two_processes_append(test: &str, per_process: usize) {
    let start_file = |dir: &Path| dir.with_extension("start");
    if let Ok(dir) = std::env::var(APPENDER_DIR) {
        let name = std::env::var(APPENDER_NAME).expect("the parent names the appender");
        let dir = PathBuf::from(dir);
        let log = RunLog::open(&dir).expect("the store opens in the child");
        let waited = std::time::Instant::now();
        while !start_file(&dir).exists() {
            assert!(
                waited.elapsed() < Duration::from_secs(30),
                "the parent never wrote the start file"
            );
            thread::sleep(Duration::from_millis(1));
        }
        for index in 0..per_process {
            log.append(
                CASE,
                at(index),
                RunLogEvent::ActionStarted(ActionStarted {
                    action: format!("{name}-{index}"),
                }),
            )
            .expect("an append from one of two processes is taken");
        }
        return;
    }

    let dir = store_dir(test);
    let log = RunLog::open(&dir).expect("the store opens");
    let _ = fs::remove_file(start_file(&dir));
    let spawn = |name: &str| -> Child {
        Command::new(std::env::current_exe().expect("the test binary"))
            .args([test, "--exact", "--nocapture", "--test-threads=1"])
            .env(APPENDER_DIR, &dir)
            .env(APPENDER_NAME, name)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the child starts")
    };
    let children = [("first", spawn("first")), ("second", spawn("second"))];
    fs::write(start_file(&dir), b"").expect("the start file is written");
    for (name, child) in children {
        let output = child.wait_with_output().expect("the child ends");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "the {name} appender failed: {}\n{stdout}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let bytes = fs::read(log.path(CASE)).expect("the log reads");
    let raw = lines(&bytes);
    assert_eq!(raw.len(), 2 * per_process, "every append is one line");
    for line in &raw {
        let value: serde_json::Value =
            serde_json::from_slice(line).expect("each line is a complete JSON document");
        assert!(value.is_object(), "each line is a JSON object");
    }
    let read = log.read(CASE).expect("the log verifies");
    let mut seen: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for line in &read {
        let RunLogEvent::ActionStarted(started) = &line.event else {
            panic!("an appender wrote only ActionStarted");
        };
        let (name, index) = started.action.rsplit_once('-').expect("name-index");
        seen.entry(name.to_owned())
            .or_default()
            .push(index.parse().expect("an index"));
    }
    let expected: Vec<usize> = (0..per_process).collect();
    assert_eq!(
        seen.get("first"),
        Some(&expected),
        "each process's lines in its order"
    );
    assert_eq!(
        seen.get("second"),
        Some(&expected),
        "each process's lines in its order"
    );
}

/// The default lane's count: 25 lines a process, overlapping from the start file on.
#[test]
fn two_processes_appending_25_lines_each_leave_50_that_verify() {
    two_processes_append(
        "two_processes_appending_25_lines_each_leave_50_that_verify",
        25,
    );
}

/// The same with 200 lines a process, in the slow lane (`task slow`): about 50 s on 4 busy
/// threads, since every append syncs.
#[test]
fn slow_two_processes_appending_200_lines_each_leave_400_that_verify() {
    two_processes_append(
        "slow_two_processes_appending_200_lines_each_leave_400_that_verify",
        200,
    );
}

#[test]
fn a_crash_at_each_step_of_an_append_leaves_n_or_n_plus_1_lines_that_verify() {
    const EXISTING: usize = 3;
    for step in AppendStep::ALL {
        let name = format!("crash-{step:?}").to_lowercase();
        let (log, path) = log_with(&name, EXISTING);
        let before = fs::read(&path).expect("reads");

        let mut seen = Vec::new();
        let stopped = log
            .append_until(CASE, at(EXISTING), event(EXISTING), &mut |reached| {
                seen.push(reached);
                if reached == step {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .expect("a crash is no error of the append");
        assert_eq!(stopped, None, "{step:?}: the append stopped there");
        assert_eq!(
            seen.last(),
            Some(&step),
            "{step:?}: the append reached the step"
        );

        let after = fs::read(&path).expect("the log still reads");
        let verified = verify(&after).expect("the log verifies after the crash");
        if step.renamed() {
            assert_eq!(
                verified.len(),
                EXISTING + 1,
                "{step:?}: the new line is whole"
            );
            assert_eq!(verified[EXISTING].event, event(EXISTING));
            assert!(
                after.starts_with(&before),
                "{step:?}: earlier lines unchanged"
            );
        } else {
            assert_eq!(after, before, "{step:?}: before the rename no byte changed");
            assert_eq!(
                temporaries(log.dir()).len(),
                1,
                "{step:?}: the crash left its temporary file, as a dead process would"
            );
        }
        drop(log);

        let reopened = RunLog::open(path.parent().expect("a directory")).expect("reopens");
        assert!(
            temporaries(reopened.dir()).is_empty(),
            "{step:?}: opening the store again removes what the crash left"
        );
        let next = reopened
            .append(CASE, at(EXISTING + 1), event(EXISTING + 1))
            .expect("the log takes the next append after a crash");
        let lines_now = reopened.read(CASE).expect("verifies");
        let expected = if step.renamed() {
            EXISTING + 2
        } else {
            EXISTING + 1
        };
        assert_eq!(
            lines_now.len(),
            expected,
            "{step:?}: n or n+1 lines, then one more"
        );
        assert_eq!(lines_now.last(), Some(&next));
    }
}

#[test]
fn an_append_does_not_return_while_the_governors_lock_is_held() {
    let dir = store_dir("held-lock");
    let log = RunLog::open(&dir).expect("the store opens");
    let path = log.path(CASE);
    let held = LockedDir::open(dir.clone()).expect("the governor's directory opens");
    let (returned, appended) = mpsc::channel();
    let handle = held
        .locked(|| {
            let handle = thread::spawn(move || {
                let result = log.append(CASE, at(0), event(0));
                returned.send(()).expect("the test waits");
                result
            });
            assert!(
                appended.recv_timeout(Duration::from_millis(500)).is_err(),
                "the append returned while the governor held the lock"
            );
            assert!(
                !path.exists(),
                "nothing was written while the lock was held"
            );
            handle
        })
        .expect("the governor takes the lock");
    appended
        .recv_timeout(Duration::from_secs(30))
        .expect("the append returns once the lock is released");
    let line = handle
        .join()
        .expect("the appending thread ends")
        .expect("the append is taken");
    assert_eq!(line.seq, 0);
}

#[test]
fn the_log_of_a_case_id_over_100_bytes_sits_beside_its_case_sha256_file() {
    let dir = store_dir("long-id");
    let id = "x".repeat(101);
    let store = FileCaseStore::open(&dir).expect("the case store opens");
    store
        .insert(CaseState {
            id: CaseId(id.clone()),
            protocol: "not-built-in@1".to_owned(),
            revision: 1,
            artifacts: BTreeMap::new(),
            inputs: CaseInputs::none(),
            independence: CaseIndependence::none(),
            evidence: Vec::new(),
            decisions: Vec::new(),
            parent: None,
            children: Vec::new(),
            termination: None,
        })
        .expect("the case is written");
    let log = RunLog::open(&dir).expect("the run log opens over the same directory");
    log.append(&id, at(0), event(0))
        .expect("the append is taken");

    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    let stem = format!("case-sha256-{}", sha256_hex(id.as_bytes()));
    assert_eq!(
        names,
        vec![format!("{stem}.json"), format!("runlog-{stem}.jsonl")],
        "the case file and its log share the stem"
    );
    assert_eq!(log_file_name(&id), format!("runlog-{stem}.jsonl"));
    assert_eq!(log.path(&id), dir.join(format!("runlog-{stem}.jsonl")));
}

#[test]
fn a_rename_that_fails_removes_its_temporary_file_and_changes_no_byte() {
    let (log, path) = log_with("failed-rename", 3);
    let before = fs::read(&path).expect("reads");
    let aside = path.with_extension("aside");
    // Once the temporary file is synced, a non-empty directory takes the log's name, so the
    // rename over it fails as a write does: the append must clean up after itself.
    let result = log.append_until(CASE, at(3), event(3), &mut |reached| {
        if reached == AppendStep::Synced {
            fs::rename(&path, &aside).expect("the log moves aside");
            fs::create_dir(&path).expect("a directory takes its name");
            fs::write(path.join("occupant"), b"x").expect("the directory is not empty");
        }
        ControlFlow::Continue(())
    });
    assert!(
        matches!(result, Err(RunLogError::Store(_))),
        "a failed rename is a store failure: {result:?}"
    );
    assert!(
        temporaries(log.dir()).is_empty(),
        "the failed append removed its temporary file"
    );
    fs::remove_dir_all(&path).expect("the directory goes");
    fs::rename(&aside, &path).expect("the log comes back");
    assert_eq!(
        fs::read(&path).expect("reads"),
        before,
        "no byte of the log changed"
    );
    assert_eq!(log.read(CASE).expect("verifies").len(), 3);
}

#[test]
fn two_unreadable_lines_in_a_row_are_named_by_the_second() {
    let (_log, path) = log_with("two-unreadable", 6);
    let mut garbled = lines(&fs::read(&path).expect("reads"));
    for index in [1, 2] {
        let last = garbled[index].len() - 1;
        garbled[index][last] = b']';
    }
    assert_eq!(
        refused(verify(&joined(&garbled))),
        (RunLogRefusal::BrokenChain, 2),
        "line 2 does not read, so it names no line before it: the break is line 2"
    );
}

// Story `child-record-in-the-run-log`: a child line names the child's `case_outcome` record in the
// parent, `record` and `record_digest`, both or neither.

/// The members of a `child opened` line of these tests, beside its `kind`.
const OPENED: &str = r#""kind":"ChildOpened","child":"chg-1842.review","protocol":"review@1""#;
/// The members of a `child closed` line of these tests, beside its `kind`.
const CLOSED: &str = r#""kind":"ChildClosed","child":"chg-1842.review","outcome":"approved""#;
/// A line before the child line and one after it, so the child line is never the last.
const FIRST: &str = r#""kind":"CaseOpened","case":"chg-1842","protocol":"software-change@1""#;
const LAST: &str = r#""kind":"Stopped","stop":"AwaitingPerson","next":[]"#;

/// A log of one line per body (`kind` and the event's members), each with its `seq`, `at` and the
/// `prev` this test computes: the chain holds whatever the members are.
fn chain(bodies: &[String]) -> Vec<u8> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for (index, body) in bodies.iter().enumerate() {
        let prev = lines
            .last()
            .map(|line| format!("\"prev\":\"{}\",", sha256_hex(line)))
            .unwrap_or_default();
        let line = format!(
            "{{\"seq\":{index},{prev}\"at\":\"{}\",{body}}}",
            at(index).0
        );
        lines.push(line.into_bytes());
    }
    joined(&lines)
}

/// The log `FIRST`, the child line `child` with `members` after its own, `LAST`.
fn child_log(child: &str, members: &str) -> Vec<u8> {
    chain(&[
        FIRST.to_owned(),
        format!("{child}{members}"),
        LAST.to_owned(),
    ])
}

fn refusal(bytes: &[u8]) -> RunLogRefused {
    match verify(bytes) {
        Ok(lines) => panic!("the log verified as {} lines: {lines:?}", lines.len()),
        Err(refused) => refused,
    }
}

/// The child line of `kind` (`ChildOpened` or `ChildClosed`) holding `record` and `record_digest`.
fn child_event(kind: &str, record: Option<&str>, record_digest: Option<&str>) -> RunLogEvent {
    let record = record.map(str::to_owned);
    let record_digest = record_digest.map(str::to_owned);
    if kind == "ChildOpened" {
        RunLogEvent::ChildOpened(ChildOpened {
            child: "chg-1842.review".to_owned(),
            protocol: "review@1".to_owned(),
            record,
            record_digest,
        })
    } else {
        RunLogEvent::ChildClosed(ChildClosed {
            child: "chg-1842.review".to_owned(),
            outcome: Some("approved".to_owned()),
            record,
            record_digest,
        })
    }
}

/// An append of `event` after one line is refused as a line that would not read back, and no
/// byte of the log changes.
fn append_is_refused(label: &str, event: RunLogEvent) {
    let (log, path) = log_with(label, 1);
    let before = fs::read(&path).expect("reads");
    let result = log.append(CASE, at(1), event);
    assert!(
        matches!(result, Err(RunLogError::Store(_))),
        "{label}: the append is refused: {result:?}"
    );
    assert_eq!(
        fs::read(&path).expect("reads"),
        before,
        "{label}: no byte changed"
    );
}

#[test]
fn a_child_line_with_record_but_no_record_digest_is_refused() {
    let digest = sha256_hex(b"facts");
    for (kind, child) in [("ChildOpened", OPENED), ("ChildClosed", CLOSED)] {
        // With both members the line reads: the refusal below is the missing member's.
        let whole = child_log(
            child,
            &format!(",\"record\":\"{RECORD}\",\"record_digest\":\"{digest}\""),
        );
        let lines = verify(&whole).unwrap_or_else(|refused| panic!("{kind}: {refused:?}"));
        assert_eq!(
            lines[1].event,
            child_event(kind, Some(RECORD), Some(&digest))
        );

        let refused = refusal(&child_log(child, &format!(",\"record\":\"{RECORD}\"")));
        assert_eq!(
            (refused.refusal, refused.line),
            (RunLogRefusal::BrokenChain, 1),
            "{kind}: {refused:?}"
        );
        assert!(
            refused.detail.contains("record_digest:"),
            "{kind}: the detail names the missing member: {}",
            refused.detail
        );
        append_is_refused(
            &format!("record-alone-{kind}"),
            child_event(kind, Some(RECORD), None),
        );
    }
}

#[test]
fn a_child_line_with_record_digest_but_no_record_is_refused() {
    let digest = sha256_hex(b"facts");
    for (kind, child) in [("ChildOpened", OPENED), ("ChildClosed", CLOSED)] {
        let refused = refusal(&child_log(
            child,
            &format!(",\"record_digest\":\"{digest}\""),
        ));
        assert_eq!(
            (refused.refusal, refused.line),
            (RunLogRefusal::BrokenChain, 1),
            "{kind}: {refused:?}"
        );
        assert!(
            refused.detail.contains("record:"),
            "{kind}: the detail names the missing member: {}",
            refused.detail
        );
        append_is_refused(
            &format!("digest-alone-{kind}"),
            child_event(kind, None, Some(&digest)),
        );
    }
}

#[test]
fn a_record_digest_that_is_no_sha256_is_refused() {
    let digest = sha256_hex(b"facts");
    let not_sha256 = [
        ("upper case", digest.to_uppercase()),
        ("63 digits", digest[..63].to_owned()),
        ("65 digits", format!("{digest}0")),
        ("no hex digit", format!("g{}", &digest[1..])),
        ("empty", String::new()),
    ];
    for (kind, child) in [("ChildOpened", OPENED), ("ChildClosed", CLOSED)] {
        for (what, bad) in &not_sha256 {
            let refused = refusal(&child_log(
                child,
                &format!(",\"record\":\"{RECORD}\",\"record_digest\":\"{bad}\""),
            ));
            assert_eq!(
                (refused.refusal, refused.line),
                (RunLogRefusal::BrokenChain, 1),
                "{kind}, {what}: {refused:?}"
            );
            assert!(
                refused.detail.contains("record_digest:"),
                "{kind}, {what}: the detail names the member: {}",
                refused.detail
            );
            append_is_refused(
                &format!("bad-digest-{kind}-{}", what.replace(' ', "-")),
                child_event(kind, Some(RECORD), Some(bad)),
            );
        }
    }
}

/// The first six lines of a root case's log as Loom wrote it before this change (the stub cycle
/// of 2026-11-22 under the tree commands at `2c0a6f48`): no child line names a record.
const OLD_LOG: &str = concat!(
    "{\"seq\":0,\"at\":\"2026-11-22T10:00:00Z\",\"kind\":\"CaseOpened\",\"case\":\"IDEA-1\",\"protocol\":\"sdc@8\"}\n",
    "{\"seq\":1,\"prev\":\"894ae22a948ec27be07abe7c28116b52eaeb824dc6d0490b73876a139a1d114e\",\"at\":\"2026-11-22T10:02:00Z\",\"kind\":\"Evaluated\",\"case_revision\":1,\"frontier\":\"0eacfce8-16f1-8d0c-99c3-260dee165509\",\"decision_digest\":\"9142a67e29fab11e18bc167b7f6bd3556cfcd4f9cde7a3ff0d0eabcb295707d3\"}\n",
    "{\"seq\":2,\"prev\":\"3267a9b8e964a514e88aac74ef38bb325e79d80e90bddab519c587eca2519720\",\"at\":\"2026-11-22T10:02:00Z\",\"kind\":\"ActionFinished\",\"action\":\"intake.open\",\"exit\":0,\"evidence\":[\"194fee46-1446-8629-9928-392f08baa58b\"],\"revisions\":[]}\n",
    "{\"seq\":3,\"prev\":\"93b6b338daa67364967efbf2a02c10e6e8f01989ca441cccdfca19e0a7e7711c\",\"at\":\"2026-11-22T10:02:00Z\",\"kind\":\"ChildOpened\",\"child\":\"IDEA-1.intake\",\"protocol\":\"sdc-intake@9\"}\n",
    "{\"seq\":4,\"prev\":\"12ec10b922354cb87a13d69c82436827a4659de798c2f8200fdd79c9948c2eeb\",\"at\":\"2026-11-22T10:02:00Z\",\"kind\":\"ChildClosed\",\"child\":\"IDEA-1.intake\",\"outcome\":\"passed\"}\n",
    "{\"seq\":5,\"prev\":\"1a25c9b872567de9e230391b31fd61aee862a85f8a1d1e14a4eb3d209f0efaa5\",\"at\":\"2026-11-22T10:02:00Z\",\"kind\":\"Evaluated\",\"case_revision\":1,\"frontier\":\"9dacb14c-3f3e-8f49-9323-5ec83150bd94\",\"decision_digest\":\"8eee3bcc0b231fb214b8fd53070e999665ad05c52c461704671c25125c982b62\"}\n",
);

#[test]
fn an_old_log_without_the_record_members_still_verifies() {
    let read = verify(OLD_LOG.as_bytes()).expect("a log written before the change verifies");
    assert_eq!(read.len(), 6);
    assert_eq!(
        read[3].event,
        RunLogEvent::ChildOpened(ChildOpened {
            child: "IDEA-1.intake".to_owned(),
            protocol: "sdc-intake@9".to_owned(),
            record: None,
            record_digest: None,
        })
    );
    assert_eq!(
        read[4].event,
        RunLogEvent::ChildClosed(ChildClosed {
            child: "IDEA-1.intake".to_owned(),
            outcome: Some("passed".to_owned()),
            record: None,
            record_digest: None,
        })
    );
    // Each line reads back as the bytes it was read from.
    for (line, raw) in read.iter().zip(lines(OLD_LOG.as_bytes())) {
        assert_eq!(encode(line).as_bytes(), raw.as_slice());
    }
}

/// A log of `CaseOpened`, a `child opened` and a `child closed` line naming [`RECORD`] with two
/// digests, and a `stopped` line.
fn log_with_records(label: &str) -> (RunLog, PathBuf, [String; 2]) {
    let digests = [sha256_hex(b"opened facts"), sha256_hex(b"closed facts")];
    let dir = store_dir(label);
    let log = RunLog::open(&dir).expect("the store opens");
    let events = [
        RunLogEvent::CaseOpened(CaseOpened {
            case: CASE.to_owned(),
            protocol: "software-change@1".to_owned(),
        }),
        child_event("ChildOpened", Some(RECORD), Some(&digests[0])),
        child_event("ChildClosed", Some(RECORD), Some(&digests[1])),
        RunLogEvent::Stopped(Stopped {
            stop: "AwaitingPerson".to_owned(),
            next: Vec::new(),
            budget: None,
        }),
    ];
    for (index, event) in events.into_iter().enumerate() {
        log.append(CASE, at(index), event)
            .unwrap_or_else(|error| panic!("line {index} is taken: {error}"));
    }
    let path = log.path(CASE);
    (log, path, digests)
}

#[test]
fn the_hash_chain_verifies_over_child_lines_with_a_record() {
    let (log, path, digests) = log_with_records("chain-records");
    let read = log.read(CASE).expect("the log verifies");
    assert_eq!(read.len(), 4);
    assert_eq!(
        read[1].event,
        child_event("ChildOpened", Some(RECORD), Some(&digests[0]))
    );
    assert_eq!(
        read[2].event,
        child_event("ChildClosed", Some(RECORD), Some(&digests[1]))
    );

    let raw = lines(&fs::read(&path).expect("reads"));
    for (index, digest) in [(1usize, &digests[0]), (2, &digests[1])] {
        // One hex digit of the digest changed to another: the line still reads, the next line
        // no longer names it.
        let mut changed = raw.clone();
        let text = String::from_utf8(changed[index].clone()).expect("utf-8");
        let first = digest.as_bytes()[0];
        let other = if first == b'0' { "1" } else { "0" };
        let tampered = format!("{other}{}", &digest[1..]);
        assert!(
            text.contains(digest.as_str()),
            "line {index} holds its digest"
        );
        changed[index] = text.replacen(digest.as_str(), &tampered, 1).into_bytes();
        let refused = refusal(&joined(&changed));
        assert_eq!(
            (refused.refusal, refused.line),
            (
                RunLogRefusal::BrokenChain,
                i64::try_from(index + 1).expect("small")
            ),
            "a changed byte in line {index}'s record_digest: {refused:?}"
        );
    }
}

#[test]
fn a_child_line_with_a_record_round_trips_byte_for_byte() {
    let (_log, path, digests) = log_with_records("round-trip-records");
    let bytes = fs::read(&path).expect("reads");
    let raw = lines(&bytes);
    // The members after the line's own, in declaration order: `record`, then `record_digest`.
    let expected = [
        format!(
            "{{\"seq\":1,\"prev\":\"{}\",\"at\":\"{}\",{OPENED},\"record\":\"{RECORD}\",\"record_digest\":\"{}\"}}",
            sha256_hex(&raw[0]),
            at(1).0,
            digests[0]
        ),
        format!(
            "{{\"seq\":2,\"prev\":\"{}\",\"at\":\"{}\",{CLOSED},\"record\":\"{RECORD}\",\"record_digest\":\"{}\"}}",
            sha256_hex(&raw[1]),
            at(2).0,
            digests[1]
        ),
    ];
    assert_eq!(String::from_utf8_lossy(&raw[1]), expected[0]);
    assert_eq!(String::from_utf8_lossy(&raw[2]), expected[1]);
    let read = verify(&bytes).expect("verifies");
    for index in [1, 2] {
        assert_eq!(
            encode(&read[index]).as_bytes(),
            raw[index].as_slice(),
            "line {index} encodes to the bytes it was read from"
        );
    }
}

/// What a reader written against the shape of the child lines before this change reads from a
/// line: the members it knows, by name, every other member ignored.
fn old_shape(line: &[u8]) -> (String, String, Option<String>, Option<String>) {
    let value: serde_json::Value = serde_json::from_slice(line).expect("a JSON line");
    let text = |name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    (
        text("kind").expect("a kind"),
        text("child").expect("a child"),
        text("protocol"),
        text("outcome"),
    )
}

#[test]
fn a_reader_that_ignores_unknown_members_reads_the_new_child_lines() {
    let (_log, path, _) = log_with_records("old-shape-reader");
    let raw = lines(&fs::read(&path).expect("reads"));
    assert_eq!(
        old_shape(&raw[1]),
        (
            "ChildOpened".to_owned(),
            "chg-1842.review".to_owned(),
            Some("review@1".to_owned()),
            None
        )
    );
    assert_eq!(
        old_shape(&raw[2]),
        (
            "ChildClosed".to_owned(),
            "chg-1842.review".to_owned(),
            None,
            Some("approved".to_owned())
        )
    );
    // The new lines hold the old members unchanged; only `record` and `record_digest` are new.
    let old_members: [&[&str]; 2] = [&["child", "protocol"], &["child", "outcome"]];
    for (line, old) in raw[1..3].iter().zip(old_members) {
        let value: serde_json::Value = serde_json::from_slice(line).expect("a JSON line");
        let added: Vec<&str> = value
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .filter(|name| !["seq", "prev", "at", "kind"].contains(name) && !old.contains(name))
            .collect();
        assert_eq!(added, ["record", "record_digest"]);
    }
}

/// The log `FIRST`, a `stopped` line with `stop`, no decision and `members` after its own, `LAST`.
fn stopped_log(stop: &str, members: &str) -> Vec<u8> {
    chain(&[
        FIRST.to_owned(),
        format!(r#""kind":"Stopped","stop":"{stop}","next":[]{members}"#),
        LAST.to_owned(),
    ])
}

/// The `stopped` line of `stop` naming `budget`.
fn stopped(stop: &str, budget: Option<&str>) -> RunLogEvent {
    RunLogEvent::Stopped(Stopped {
        stop: stop.to_owned(),
        next: Vec::new(),
        budget: budget.map(str::to_owned),
    })
}

/// Asserts the log `bytes` is refused `BrokenChain` at its line 1, the detail naming `budget`.
fn refused_at_the_budget(bytes: &[u8], what: &str) {
    let refused = refusal(bytes);
    assert_eq!(
        (refused.refusal, refused.line),
        (RunLogRefusal::BrokenChain, 1),
        "{what}: {refused:?}"
    );
    assert!(
        refused.detail.contains("budget:"),
        "{what}: the detail names the member: {}",
        refused.detail
    );
}

#[test]
fn a_stopped_line_with_a_budget_and_another_stop_is_refused() {
    // On a `BudgetSpent` stop the line reads: the refusals below are the other stops'.
    let whole = stopped_log("BudgetSpent", r#","budget":"MaxSteps""#);
    let lines = verify(&whole).unwrap_or_else(|refused| panic!("{refused:?}"));
    assert_eq!(lines[1].event, stopped("BudgetSpent", Some("MaxSteps")));
    for (index, stop) in [
        "AwaitingPerson",
        "AwaitingAuthority",
        "Stuck",
        "Terminal",
        "budgetspent",
        "",
    ]
    .into_iter()
    .enumerate()
    {
        refused_at_the_budget(
            &stopped_log(stop, r#","budget":"MaxSteps""#),
            &format!("stop {stop:?}"),
        );
        append_is_refused(
            &format!("budget-beside-stop-{index}"),
            stopped(stop, Some("MaxSteps")),
        );
    }
}

#[test]
fn a_stopped_line_with_an_unknown_budget_is_refused() {
    for (index, budget) in [
        "cancel",
        "max_steps",
        "maxWallSeconds",
        "MaxRuns",
        "MaxTurns",
        "MaxTokens",
        "Steps",
        "MaxSteps ",
        "",
    ]
    .into_iter()
    .enumerate()
    {
        refused_at_the_budget(
            &stopped_log("BudgetSpent", &format!(r#","budget":"{budget}""#)),
            &format!("budget {budget:?}"),
        );
        append_is_refused(
            &format!("unknown-budget-{index}"),
            stopped("BudgetSpent", Some(budget)),
        );
    }
    refused_at_the_budget(
        &stopped_log("BudgetSpent", r#","budget":1"#),
        "a budget that is no string",
    );
}

/// A one-leg case's log as Loom wrote it before `budget` existed: two `BudgetSpent` stops, the
/// last line one of them, and another stop between.
const OLD_STOPPED_LOG: &str = concat!(
    "{\"seq\":0,\"at\":\"2026-11-22T08:00:00Z\",\"kind\":\"CaseOpened\",\"case\":\"root\",\"protocol\":\"tour-leg@1\"}\n",
    "{\"seq\":1,\"prev\":\"a0a4172d05916fef90ac7ea88c28ee4db19e9f90c986bc1a5a97e61c6229422e\",\"at\":\"2026-11-22T08:00:00Z\",\"kind\":\"Stopped\",\"stop\":\"BudgetSpent\",\"next\":[]}\n",
    "{\"seq\":2,\"prev\":\"dda9cc2f78d2901217d8a8c88e74d31f2e109b829a0baf618c1e5c980f0dc552\",\"at\":\"2026-11-22T08:00:01Z\",\"kind\":\"Stopped\",\"stop\":\"AwaitingPerson\",\"next\":[\"the guide approves the route\"]}\n",
    "{\"seq\":3,\"prev\":\"1f4b9b9bec6db1dbb4e638cd8327996b7a7524f5d25cc5a3ae884a0f3324c91a\",\"at\":\"2026-11-22T08:00:02Z\",\"kind\":\"Stopped\",\"stop\":\"BudgetSpent\",\"next\":[]}\n",
);

#[test]
fn an_old_log_without_a_budget_still_verifies() {
    let read =
        verify(OLD_STOPPED_LOG.as_bytes()).expect("a log written before the change verifies");
    assert_eq!(read.len(), 4);
    assert_eq!(read[1].event, stopped("BudgetSpent", None));
    assert_eq!(
        read[2].event,
        RunLogEvent::Stopped(Stopped {
            stop: "AwaitingPerson".to_owned(),
            next: vec!["the guide approves the route".to_owned()],
            budget: None,
        })
    );
    assert_eq!(read[3].event, stopped("BudgetSpent", None));
    // Each line reads back as the bytes it was read from: an absent budget is left out.
    for (line, raw) in read.iter().zip(lines(OLD_STOPPED_LOG.as_bytes())) {
        assert_eq!(encode(line).as_bytes(), raw.as_slice());
    }
}

#[test]
fn the_budget_spellings_are_the_three_of_the_story() {
    use loom::tree::SpentBudget;
    // The story's list, in the order a run asks the budgets before a step.
    let story = ["Cancel", "MaxSteps", "MaxWallSeconds"];
    // Every variant, by an exhaustive match: a fourth one does not compile here.
    let spelled = |budget: SpentBudget| match budget {
        SpentBudget::Cancel => "Cancel",
        SpentBudget::MaxSteps => "MaxSteps",
        SpentBudget::MaxWallSeconds => "MaxWallSeconds",
    };
    let variants = [
        SpentBudget::Cancel,
        SpentBudget::MaxSteps,
        SpentBudget::MaxWallSeconds,
    ];
    for (index, (budget, name)) in variants.into_iter().zip(story).enumerate() {
        assert_eq!(spelled(budget), name);
        assert_eq!(format!("{budget:?}"), name, "the variant is the spelling");
        let bytes = stopped_log("BudgetSpent", &format!(r#","budget":"{name}""#));
        let read = verify(&bytes).unwrap_or_else(|refused| panic!("{name}: {refused:?}"));
        assert_eq!(read[1].event, stopped("BudgetSpent", Some(name)));
        assert_eq!(encode(&read[1]).as_bytes(), lines(&bytes)[1].as_slice());
        let (log, _) = log_with(&format!("budget-spelling-{index}"), 1);
        log.append(CASE, at(1), stopped("BudgetSpent", Some(name)))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let written = log.read(CASE).expect("the log verifies");
        assert_eq!(written[1].event, stopped("BudgetSpent", Some(name)));
    }
}
