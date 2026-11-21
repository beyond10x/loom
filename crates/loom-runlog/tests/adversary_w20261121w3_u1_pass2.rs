//! Adversary pass 2, wave 2026-11-21-w3, unit U1 (`story:hash-chained-run-log`): the rule that an
//! `ActionFinished` line holds exactly one of `exit` and `refusal`, on every path that reads or
//! writes a line. Lines are counted from 0, as `seq` counts them.

use std::fs;
use std::path::{Path, PathBuf};

use b10x_loom_runlog::{RunLog, RunLogError, digest, encode, verify};
use loom::primitives::Timestamp;
use loom::runlog::{
    ActionFinished, ActionStarted, RunLogEvent, RunLogLine, RunLogRefusal, RunLogRefused,
};

const CASE: &str = "adv-u1-pass2";

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-w20261121w3-u1-pass2")
        .join(name);
    if dir.symlink_metadata().is_ok() {
        fs::remove_dir_all(&dir).expect("an earlier run's directory is removed");
    }
    dir
}

fn at(index: usize) -> Timestamp {
    Timestamp(format!("2026-11-21T12:00:{:02}Z", index % 60))
}

fn started(index: usize) -> RunLogEvent {
    RunLogEvent::ActionStarted(ActionStarted {
        action: format!("action-{index}"),
    })
}

fn finished(exit: Option<i64>, refusal: Option<&str>) -> RunLogEvent {
    RunLogEvent::ActionFinished(ActionFinished {
        action: "build".to_owned(),
        exit,
        refusal: refusal.map(str::to_owned),
        blob_digest: None,
        evidence: Vec::new(),
        revisions: Vec::new(),
    })
}

fn refusal(result: Result<Vec<RunLogLine>, RunLogRefused>) -> (RunLogRefusal, i64) {
    match result {
        Ok(lines) => panic!("the log verified as {} lines", lines.len()),
        Err(refused) => (refused.refusal, refused.line),
    }
}

fn temporaries(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("reads")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with(".tmp-"))
        .collect()
}

/// Line `seq` of an ActionFinished written by hand, with `members` (JSON text) after `action`.
fn finished_line(seq: i64, prev: Option<&str>, members: &str) -> String {
    let prev = prev.map_or(String::new(), |prev| format!(",\"prev\":\"{prev}\""));
    format!(
        "{{\"seq\":{seq}{prev},\"at\":\"2026-11-21T12:00:00Z\",\"kind\":\"ActionFinished\",\
         \"action\":\"build\"{members},\"evidence\":[],\"revisions\":[]}}"
    )
}

/// Each shape that does not hold exactly one exit and one refusal, as the last line and as line 0
/// alone, is refused `CutLine`; each shape with exactly one reads.
#[test]
fn every_shape_without_exactly_one_of_exit_and_refusal_is_refused() {
    for members in [
        ",\"exit\":0",
        ",\"refusal\":\"denied\"",
        ",\"exit\":-1",
        ",\"exit\":3.0",
    ] {
        let mut bytes = finished_line(0, None, members).into_bytes();
        bytes.push(b'\n');
        assert!(verify(&bytes).is_ok(), "{members}: exactly one is read");
    }
    for members in [
        "",
        ",\"exit\":0,\"refusal\":\"denied\"",
        ",\"exit\":null,\"refusal\":\"denied\"",
        ",\"exit\":null",
        ",\"refusal\":null",
        ",\"exit\":0,\"refusal\":null",
        ",\"exit\":0,\"exit\":0",
        ",\"exit\":0,\"exit\":1",
        ",\"refusal\":\"a\",\"refusal\":\"b\"",
        ",\"exit\":\"0\"",
        ",\"exit\":true",
        ",\"refusal\":0",
        ",\"exit\":1.5",
        ",\"exit\":0,\"\\u0072efusal\":\"denied\"",
    ] {
        let mut bytes = finished_line(0, None, members).into_bytes();
        bytes.push(b'\n');
        assert_eq!(
            refusal(verify(&bytes)),
            (RunLogRefusal::CutLine, 0),
            "{members}"
        );
    }
    for kind_line in [
        "{\"seq\":0,\"at\":\"2026-11-21T12:00:00Z\",\"kind\":\"ActionStarted\",\"action\":\"a\",\"refusal\":\"denied\"}",
        "{\"seq\":0,\"at\":\"2026-11-21T12:00:00Z\",\"kind\":\"Terminated\",\"reason\":\"r\",\"exit\":0}",
    ] {
        let mut bytes = kind_line.as_bytes().to_vec();
        bytes.push(b'\n');
        assert_eq!(
            refusal(verify(&bytes)),
            (RunLogRefusal::CutLine, 0),
            "an exit or a refusal on another kind: {kind_line}"
        );
    }
}

/// A line before the last holding both, which the next line names correctly, is refused
/// `BrokenChain` naming that line itself; as line 0 and line 2 of a log.
#[test]
fn a_middle_action_finished_with_both_is_named_itself() {
    for at_line in [0usize, 2] {
        let mut text = String::new();
        let mut prev: Option<String> = None;
        for index in 0..=at_line + 1 {
            let line = if index == at_line {
                finished_line(
                    i64::try_from(index).expect("small"),
                    prev.as_deref(),
                    ",\"exit\":0,\"refusal\":\"denied\"",
                )
            } else {
                encode(&RunLogLine {
                    seq: i64::try_from(index).expect("small"),
                    prev: prev.clone(),
                    at: at(index),
                    event: started(index),
                })
            };
            prev = Some(digest(line.as_bytes()));
            text.push_str(&line);
            text.push('\n');
        }
        assert_eq!(
            refusal(verify(text.as_bytes())),
            (
                RunLogRefusal::BrokenChain,
                i64::try_from(at_line).expect("small")
            ),
            "both at line {at_line}, named by the line after it"
        );
    }
}

/// An append of either refused shape, to an absent log and to a log of three lines, is a store
/// error, leaves the log byte-identical (or absent) and no temporary file; the log then takes an
/// append of exactly one.
#[test]
fn an_append_of_both_or_neither_writes_nothing_and_leaves_no_temporary() {
    let dir = store_dir("append");
    let log = RunLog::open(&dir).expect("opens");
    let path = log.path(CASE);
    for (name, event) in [
        ("both", finished(Some(0), Some("denied"))),
        ("neither", finished(None, None)),
    ] {
        match log.append(CASE, at(0), event) {
            Err(RunLogError::Store(_)) => {}
            other => panic!("{name} on an absent log: {other:?}"),
        }
        assert!(!path.exists(), "{name}: no log was created");
        assert!(temporaries(&dir).is_empty(), "{name}: no temporary file");
    }
    for index in 0..3 {
        log.append(CASE, at(index), started(index))
            .expect("appends");
    }
    let before = fs::read(&path).expect("reads");
    for (name, event) in [
        ("both", finished(Some(0), Some("denied"))),
        ("neither", finished(None, None)),
    ] {
        match log.append(CASE, at(3), event) {
            Err(RunLogError::Store(_)) => {}
            other => panic!("{name}: {other:?}"),
        }
        assert_eq!(
            fs::read(&path).expect("reads"),
            before,
            "{name}: no byte changed"
        );
        assert!(temporaries(&dir).is_empty(), "{name}: no temporary file");
    }
    let line = log
        .append(CASE, at(3), finished(None, Some("")))
        .expect("an append of exactly one is taken");
    assert_eq!(line.seq, 3);
    assert_eq!(log.read(CASE).expect("verifies").len(), 4);
}
