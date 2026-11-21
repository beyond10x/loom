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
use b10x_loom_runlog::{AppendStep, RunLog, RunLogError, log_file_name, verify};
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

/// The environment variable that makes the two-process test's binary an appender over the
/// directory it names.
const APPENDER_DIR: &str = "LOOM_RUNLOG_APPENDER_DIR";
/// The name each appender child writes into its events.
const APPENDER_NAME: &str = "LOOM_RUNLOG_APPENDER_NAME";
const TWO_PROCESSES: &str = "two_processes_appending_200_lines_each_leave_400_that_verify";
const LINES_PER_PROCESS: usize = 200;

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
        }),
        6 => RunLogEvent::ChildClosed(ChildClosed {
            child: format!("child-{marker}"),
            outcome: (index % 20 == 6).then(|| "approved".to_owned()),
        }),
        7 => RunLogEvent::AuthorityRecorded(AuthorityRecorded {
            capability: "repository.merge".to_owned(),
            decision: format!("allow-{marker}"),
            principal: "human:operator".to_owned(),
        }),
        8 => RunLogEvent::Stopped(Stopped {
            stop: format!("approval-{marker}"),
            next: vec!["approve".to_owned(), "decline".to_owned()],
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
    let mut lines: Vec<Vec<u8>> = bytes.split(|byte| *byte == b'\n').map(<[u8]>::to_vec).collect();
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
        .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".tmp-"))
        .collect()
}

#[test]
fn fifty_events_verify_with_a_gapless_seq_and_each_prev_the_digest_of_the_line_before() {
    let (log, path) = log_with("fifty", 50);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(format!("runlog-case-{}.jsonl", hex(CASE.as_bytes())).as_str()),
        "the log sits beside the case file `case-<hex>.json` under the same stem"
    );
    let bytes = fs::read(&path).expect("the log file reads");
    let raw = lines(&bytes);
    assert_eq!(raw.len(), 50, "one line per event");
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
            assert!(!object.contains_key("prev"), "line 0 names no previous line");
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
    assert_eq!(read.len(), 50);
    for (index, line) in read.iter().enumerate() {
        assert_eq!(line.seq, i64::try_from(index).expect("small"));
        assert_eq!(line.at, at(index));
        assert_eq!(line.event, event(index), "line {index} holds the event appended");
        let prev = (index > 0).then(|| sha256_hex(&raw[index - 1]));
        assert_eq!(line.prev, prev);
    }
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
            assert_eq!((refused.refusal, refused.line), (RunLogRefusal::BrokenChain, 4));
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
    assert_eq!(refused(verify(&joined(&first))), (RunLogRefusal::BrokenChain, 0));

    let mut later = raw.clone();
    let text = String::from_utf8(later[2].clone()).expect("utf-8");
    let prev = format!(",\"prev\":\"{}\"", sha256_hex(&raw[1]));
    assert!(text.contains(&prev), "line 2 names line 1");
    later[2] = text.replacen(&prev, "", 1).into_bytes();
    assert_eq!(refused(verify(&joined(&later))), (RunLogRefusal::BrokenChain, 2));
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
        ("a changed byte", joined(&changed), (RunLogRefusal::BrokenChain, 4)),
        ("a deleted line", joined(&deleted), (RunLogRefusal::SequenceGap, 3)),
        ("swapped lines", joined(&swapped), (RunLogRefusal::SequenceGap, 3)),
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

#[test]
fn two_processes_appending_200_lines_each_leave_400_that_verify() {
    if let Ok(dir) = std::env::var(APPENDER_DIR) {
        let name = std::env::var(APPENDER_NAME).expect("the parent names the appender");
        let log = RunLog::open(PathBuf::from(dir)).expect("the store opens in the child");
        for index in 0..LINES_PER_PROCESS {
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

    let dir = store_dir("two-processes");
    let log = RunLog::open(&dir).expect("the store opens");
    let spawn = |name: &str| -> Child {
        Command::new(std::env::current_exe().expect("the test binary"))
            .args([TWO_PROCESSES, "--exact", "--nocapture", "--test-threads=1"])
            .env(APPENDER_DIR, &dir)
            .env(APPENDER_NAME, name)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the child starts")
    };
    let children = [("first", spawn("first")), ("second", spawn("second"))];
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
    assert_eq!(raw.len(), 2 * LINES_PER_PROCESS, "every append is one line");
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
    let expected: Vec<usize> = (0..LINES_PER_PROCESS).collect();
    assert_eq!(seen.get("first"), Some(&expected), "each process's lines in its order");
    assert_eq!(seen.get("second"), Some(&expected), "each process's lines in its order");
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
        assert_eq!(seen.last(), Some(&step), "{step:?}: the append reached the step");

        let after = fs::read(&path).expect("the log still reads");
        let verified = verify(&after).expect("the log verifies after the crash");
        if step.renamed() {
            assert_eq!(verified.len(), EXISTING + 1, "{step:?}: the new line is whole");
            assert_eq!(verified[EXISTING].event, event(EXISTING));
            assert!(after.starts_with(&before), "{step:?}: earlier lines unchanged");
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
        let expected = if step.renamed() { EXISTING + 2 } else { EXISTING + 1 };
        assert_eq!(lines_now.len(), expected, "{step:?}: n or n+1 lines, then one more");
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
            assert!(!path.exists(), "nothing was written while the lock was held");
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
        })
        .expect("the case is written");
    let log = RunLog::open(&dir).expect("the run log opens over the same directory");
    log.append(&id, at(0), event(0)).expect("the append is taken");

    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("reads")
        .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
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
