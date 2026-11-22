//! Adversary pass 1, wave 2026-11-21-w3, unit U1 (`story:hash-chained-run-log`).
//!
//! Cases the unit's own suite does not reach: the contract document read as the specification,
//! a middle line that does not read but is named by the line after it, the read-back guard of an
//! append, a log entry that is not a regular file, members a line must not hold, bytes around and
//! inside the lines, case ids that try to leave the store, the log file's mode, and appends from
//! many threads. Lines are counted from 0, as `seq` counts them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_runlog::{RunLog, RunLogError, digest, encode, log_file_name, verify};
use loom::primitives::Timestamp;
use loom::runlog::{
    ActionFinished, ActionStarted, CaseOpened, Discarded, RunLogEvent, RunLogLine, RunLogRefusal,
    RunLogRefused, Terminated,
};
use loom_governor::{
    CaseIndependence, CaseInputs, CaseState, FallibleCaseStore as _, FileCaseStore,
};
use serde_json::{Value, json};

const CASE: &str = "adv-u1";

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-w20261121w3-u1")
        .join(name);
    if dir.symlink_metadata().is_ok() {
        fs::remove_dir_all(&dir).expect("an earlier run's directory is removed");
    }
    dir
}

fn at(index: usize) -> Timestamp {
    Timestamp(format!(
        "2026-11-21T11:{:02}:{:02}Z",
        (index / 60) % 60,
        index % 60
    ))
}

fn event(index: usize) -> RunLogEvent {
    match index % 3 {
        0 => RunLogEvent::CaseOpened(CaseOpened {
            case: format!("case-{index}"),
            protocol: "software-change@1".to_owned(),
        }),
        1 => RunLogEvent::Discarded(Discarded {
            action: format!("action-{index}"),
            reason: format!("reason-{index}"),
        }),
        _ => RunLogEvent::ActionStarted(ActionStarted {
            action: format!("action-{index}"),
        }),
    }
}

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

fn lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut lines: Vec<Vec<u8>> = bytes.split(|b| *b == b'\n').map(<[u8]>::to_vec).collect();
    if bytes.ends_with(b"\n") {
        lines.pop();
    }
    lines
}

fn joined(lines: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    bytes
}

fn refusal(result: Result<Vec<RunLogLine>, RunLogRefused>) -> (RunLogRefusal, i64) {
    match result {
        Ok(lines) => panic!("an altered log verified as {} lines", lines.len()),
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

fn contract() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/contracts/run-log-v1.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("the contract is JSON")
}

/// A value of the contract's stated type.
fn sample(stated: &Value) -> Value {
    let kind = stated["type"].as_str().expect("a stated type");
    match kind {
        "String" => json!("v"),
        "Integer" => json!(7),
        "Timestamp" => json!("2026-11-21T11:00:00Z"),
        "List<String>" => json!(["a", "b"]),
        list if list.starts_with("List<") && stated["fields"].is_object() => {
            let mut item = serde_json::Map::new();
            for (name, field) in stated["fields"].as_object().expect("fields") {
                item.insert(name.clone(), sample(field));
            }
            json!([Value::Object(item)])
        }
        other => panic!("the contract states a type this case does not know: {other}"),
    }
}

/// Line 0 of `kind` carrying every field the contract states, optional ones included.
fn contract_line(contract: &Value, kind: &str) -> serde_json::Map<String, Value> {
    let tag = contract["line"]["tag"].as_str().expect("a tag");
    let mut line = serde_json::Map::new();
    line.insert("seq".to_owned(), json!(0));
    line.insert("at".to_owned(), json!("2026-11-21T11:00:00Z"));
    line.insert(tag.to_owned(), json!(kind));
    for (name, stated) in contract["kinds"][kind]["fields"]
        .as_object()
        .expect("fields")
    {
        line.insert(name.clone(), sample(stated));
    }
    line
}

fn one_line(line: &serde_json::Map<String, Value>) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(line).expect("serializes");
    bytes.push(b'\n');
    bytes
}

/// The contract document, read as the specification of a line: every kind it names reads with
/// every field it states, writes back as exactly those members, loses a required field only by
/// being refused, and takes no member the contract does not state. `ActionFinished` holds exactly
/// one of `exit` and `refusal` (the domain's "one of the two", which the contract cannot state):
/// it is read twice, with every field and `exit` alone, and with every field and `refusal` alone;
/// the line with both and the line with neither are refused.
#[test]
fn every_kind_of_the_contract_reads_and_writes_exactly_the_fields_it_states() {
    let contract = contract();
    let kinds = contract["kinds"].as_object().expect("kinds");
    assert_eq!(kinds.len(), 10, "the story names ten events");
    for (kind, body) in kinds {
        let full = contract_line(&contract, kind);
        let exclusive = ["exit", "refusal"];
        let variants: Vec<serde_json::Map<String, Value>> = if kind == "ActionFinished" {
            for member in exclusive {
                assert!(full.contains_key(member), "the contract states `{member}`");
            }
            assert_eq!(
                refusal(verify(&one_line(&full))),
                (RunLogRefusal::CutLine, 0),
                "{kind}: both an exit and a refusal"
            );
            let mut neither = full.clone();
            neither.remove("exit");
            neither.remove("refusal");
            assert_eq!(
                refusal(verify(&one_line(&neither))),
                (RunLogRefusal::CutLine, 0),
                "{kind}: neither an exit nor a refusal"
            );
            exclusive
                .iter()
                .map(|dropped| {
                    let mut line = full.clone();
                    line.remove(*dropped);
                    line
                })
                .collect()
        } else {
            vec![full]
        };
        for line in variants {
            let read = verify(&one_line(&line)).unwrap_or_else(|refused| {
                panic!("{kind}: a line the contract states is refused: {refused:?}")
            });
            assert_eq!(read.len(), 1, "{kind}");
            let written: Value =
                serde_json::from_str(&encode(&read[0])).expect("encode writes JSON");
            assert_eq!(
                written,
                Value::Object(line.clone()),
                "{kind}: the line writes back as read"
            );

            for (name, stated) in body["fields"].as_object().expect("fields") {
                if !line.contains_key(name) {
                    continue;
                }
                let mut without = line.clone();
                without.remove(name);
                let result = verify(&one_line(&without));
                if kind == "ActionFinished" && exclusive.contains(&name.as_str()) {
                    assert_eq!(
                        refusal(result),
                        (RunLogRefusal::CutLine, 0),
                        "{kind}: `{name}`, the only one of exit and refusal, left out"
                    );
                } else if stated["optional"] == json!(true) {
                    assert!(
                        result.is_ok(),
                        "{kind}: optional `{name}` left out is refused: {result:?}"
                    );
                } else {
                    assert_eq!(
                        refusal(result),
                        (RunLogRefusal::CutLine, 0),
                        "{kind}: required `{name}` left out"
                    );
                }
                let mut wrong = line.clone();
                wrong.insert(name.clone(), json!(true));
                assert_eq!(
                    refusal(verify(&one_line(&wrong))),
                    (RunLogRefusal::CutLine, 0),
                    "{kind}: `{name}` of the wrong type"
                );
            }
            let mut extra = line.clone();
            extra.insert("not_in_the_contract".to_owned(), json!("x"));
            assert_eq!(
                refusal(verify(&one_line(&extra))),
                (RunLogRefusal::CutLine, 0),
                "{kind}: a member the contract does not state"
            );
        }
    }
    let mut unknown = contract_line(&contract, "Terminated");
    unknown.insert("kind".to_owned(), json!("Bogus"));
    assert_eq!(
        refusal(verify(&one_line(&unknown))),
        (RunLogRefusal::CutLine, 0)
    );
}

/// A line before the last that does not read is refused even when the line after it names its
/// digest: the crate documentation says it "is itself named otherwise". The unit's suite only
/// covers an unreadable line the next line does not name.
#[test]
fn a_middle_line_that_does_not_read_is_refused_when_the_next_line_names_it() {
    for garbage_at in [0usize, 2] {
        let count = garbage_at + 2;
        let (log, path) = log_with(&format!("unreadable-named-{garbage_at}"), count);
        let original = fs::read(&path).expect("reads");
        let mut raw = lines(&original);
        let mut last = verify(&original)
            .expect("verifies")
            .pop()
            .expect("a last line");
        let garbage = b"{\"this is\":\"not a run log line\"}".to_vec();
        last.prev = Some(digest(&garbage));
        raw[garbage_at] = garbage;
        raw[count - 1] = encode(&last).into_bytes();
        let altered = joined(&raw);
        assert_eq!(
            refusal(verify(&altered)),
            (
                RunLogRefusal::BrokenChain,
                i64::try_from(garbage_at).expect("small")
            ),
            "an unreadable line {garbage_at} named by the line after it"
        );
        fs::write(&path, &altered).expect("writes");
        assert!(
            matches!(
                log.append(CASE, at(count), event(count)),
                Err(RunLogError::Refused(_))
            ),
            "an append to it is refused"
        );
        assert_eq!(
            fs::read(&path).expect("reads"),
            altered,
            "and changes no byte"
        );
    }
}

/// The read-back guard: a line that would not read back (an `at` that is no RFC 3339 instant) is
/// refused before anything is written, so the log keeps verifying and takes the next append.
#[test]
fn an_append_whose_at_is_no_instant_is_refused_and_writes_nothing() {
    let (log, path) = log_with("bad-at", 2);
    let before = fs::read(&path).expect("reads");
    for text in [
        "yesterday",
        "",
        "2026-11-21 11:00:00",
        "2026-02-30T11:00:00Z",
    ] {
        match log.append(CASE, Timestamp(text.to_owned()), event(2)) {
            Err(RunLogError::Store(_)) => {}
            other => panic!("an `at` of {text:?} was not refused as a store error: {other:?}"),
        }
        assert_eq!(
            fs::read(&path).expect("reads"),
            before,
            "{text:?}: no byte changed"
        );
        assert!(
            temporaries(log.dir()).is_empty(),
            "{text:?}: no temporary file left"
        );
    }
    let line = log
        .append(CASE, at(2), event(2))
        .expect("the next append is taken");
    assert_eq!(line.seq, 2);

    let (fresh, fresh_path) = log_with("bad-at-first", 0);
    assert!(
        fresh
            .append(CASE, Timestamp("now".to_owned()), event(0))
            .is_err()
    );
    assert!(
        !fresh_path.exists(),
        "a refused first append creates no log"
    );
}

/// Only a regular file is a log: a symlink to a log kept elsewhere, a directory and a FIFO at the
/// log's name are each refused by a read and an append, never followed, and nothing changes.
#[test]
fn a_log_entry_that_is_not_a_regular_file_is_refused_not_followed() {
    let (_elsewhere, target) = log_with("link-target", 3);
    let target_bytes = fs::read(&target).expect("reads");

    let dir = store_dir("link-store");
    let log = RunLog::open(&dir).expect("opens");
    let path = log.path(CASE);
    std::os::unix::fs::symlink(&target, &path).expect("a symlink at the log's name");
    assert!(
        matches!(log.read(CASE), Err(RunLogError::Store(_))),
        "a symlinked log is read"
    );
    assert!(
        matches!(
            log.append(CASE, at(3), event(3)),
            Err(RunLogError::Store(_))
        ),
        "a symlinked log takes an append"
    );
    assert!(
        fs::symlink_metadata(&path)
            .expect("entry")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read(&target).expect("reads"),
        target_bytes,
        "the link's target is untouched"
    );
    fs::remove_file(&path).expect("removes the link");

    fs::create_dir(&path).expect("a directory at the log's name");
    assert!(matches!(log.read(CASE), Err(RunLogError::Store(_))));
    assert!(matches!(
        log.append(CASE, at(0), event(0)),
        Err(RunLogError::Store(_))
    ));
    assert!(path.is_dir());
    fs::remove_dir(&path).expect("removes the directory");

    let made = Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "a FIFO at the log's name");
    let log = Arc::new(log);
    let (sent, received) = mpsc::channel();
    let reader = Arc::clone(&log);
    thread::spawn(move || {
        let read = matches!(reader.read(CASE), Err(RunLogError::Store(_)));
        let append = matches!(
            reader.append(CASE, at(0), event(0)),
            Err(RunLogError::Store(_))
        );
        let _ = sent.send((read, append));
    });
    let outcome = received
        .recv_timeout(Duration::from_secs(10))
        .expect("a read of a FIFO log returns instead of blocking under the lock");
    assert_eq!(
        outcome,
        (true, true),
        "a FIFO log is refused by a read and an append"
    );
}

/// A last line holding a member twice, a member its kind does not declare, or the same member
/// spelt through an escape twice, is refused: two readers would read two different lines.
#[test]
fn a_last_line_with_a_member_twice_or_an_undeclared_member_is_refused() {
    let (_log, path) = log_with("members", 3);
    let raw = lines(&fs::read(&path).expect("reads"));
    let last = String::from_utf8(raw[2].clone()).expect("utf-8");
    assert!(
        last.contains("\"kind\":\"ActionStarted\""),
        "line 2 is ActionStarted"
    );
    let body = last.strip_suffix('}').expect("an object");
    for altered in [
        format!("{body},\"action\":\"another\"}}"),
        format!("{body},\"act\\u0069on\":\"another\"}}"),
        format!("{body},\"seq\":2}}"),
        format!("{body},\"reason\":\"from another kind\"}}"),
        format!("{body},\"prev\":null}}"),
    ] {
        let mut tampered = raw.clone();
        tampered[2] = altered.clone().into_bytes();
        assert_eq!(
            refusal(verify(&joined(&tampered))),
            (RunLogRefusal::CutLine, 2),
            "{altered}"
        );
    }
}

/// Bytes around the lines: each of these is refused, naming the line it reaches.
#[test]
fn bytes_around_and_inside_the_lines_are_refused_where_the_writer_never_writes_them() {
    let (_log, path) = log_with("edges", 3);
    let bytes = fs::read(&path).expect("reads");
    let raw = lines(&bytes);

    let mut blank_end = bytes.clone();
    blank_end.push(b'\n');
    assert_eq!(
        refusal(verify(&blank_end)),
        (RunLogRefusal::CutLine, 3),
        "a blank line at the end"
    );

    let mut trailing = bytes.clone();
    trailing.extend_from_slice(b"x");
    assert_eq!(
        refusal(verify(&trailing)),
        (RunLogRefusal::CutLine, 3),
        "bytes after the last line feed"
    );

    let mut blank_middle = raw.clone();
    blank_middle.insert(1, Vec::new());
    assert_eq!(
        refusal(verify(&joined(&blank_middle))),
        (RunLogRefusal::SequenceGap, 2),
        "a blank line inside"
    );

    let crlf: Vec<Vec<u8>> = raw.iter().map(|l| [l.as_slice(), b"\r"].concat()).collect();
    assert_eq!(
        refusal(verify(&joined(&crlf))),
        (RunLogRefusal::BrokenChain, 1),
        "CR LF line ends"
    );

    let mut bom = raw.clone();
    bom[0] = [b"\xef\xbb\xbf".as_slice(), &raw[0]].concat();
    assert_eq!(
        refusal(verify(&joined(&bom))),
        (RunLogRefusal::BrokenChain, 1),
        "a byte-order mark"
    );

    let mut latin1 = raw.clone();
    let inside = latin1[1].len() - 2;
    latin1[1].insert(inside, 0xe9);
    assert_eq!(
        refusal(verify(&joined(&latin1))),
        (RunLogRefusal::BrokenChain, 2),
        "a non-UTF-8 byte"
    );

    assert_eq!(
        refusal(verify(b"\n")),
        (RunLogRefusal::CutLine, 0),
        "a lone line feed"
    );
    assert_eq!(verify(b"").expect("an empty log"), Vec::new());

    for (seq, expected) in [
        ("\"0\"", RunLogRefusal::CutLine),
        ("-1", RunLogRefusal::SequenceGap),
        ("9007199254740993", RunLogRefusal::SequenceGap),
    ] {
        let line0 = String::from_utf8(raw[0].clone()).expect("utf-8").replacen(
            "\"seq\":0",
            &format!("\"seq\":{seq}"),
            1,
        );
        let mut one = line0.into_bytes();
        one.push(b'\n');
        assert_eq!(refusal(verify(&one)), (expected, 0), "seq {seq}");
    }
}

/// What nothing after it can hold: the last line may be re-spelt or have its event changed and the
/// log still verifies. `AGENTS.md` § Run log says "a changed byte is named by the line after it";
/// the last line has no line after it. This case pins that limit (green), so the prose can be held
/// to it.
#[test]
fn the_last_line_is_held_by_nothing_after_it() {
    let (log, path) = log_with("last-line", 3);
    let bytes = fs::read(&path).expect("reads");
    let raw = lines(&bytes);
    let last = String::from_utf8(raw[2].clone()).expect("utf-8");
    for altered in [
        last.replace("action-2", "action-X"),
        last.replacen("\"seq\":2", "\"seq\":2.0", 1),
        last.replacen("\"seq\":2", "\"seq\" : 20e-1", 1),
        format!(" {last}\r"),
    ] {
        let mut tampered = raw.clone();
        tampered[2] = altered.clone().into_bytes();
        let read =
            verify(&joined(&tampered)).expect("an altered last line verifies: nothing names it");
        assert_eq!(read.len(), 3, "{altered}");
        fs::write(&path, joined(&tampered)).expect("writes");
        let next = log
            .append(CASE, at(3), event(3))
            .expect("and takes the next append");
        assert_eq!(
            next.prev,
            Some(digest(altered.as_bytes())),
            "chained over the altered bytes"
        );
        fs::write(&path, &bytes).expect("restores");
    }
}

/// No case id names a file outside the store or one longer than a file name may be, and distinct
/// ids name distinct logs, at the 100-byte boundary too.
#[test]
fn a_case_id_names_a_log_inside_the_store_and_no_two_ids_one_log() {
    let dir = store_dir("ids");
    let log = RunLog::open(&dir).expect("opens");
    let ids: Vec<String> = vec![
        String::new(),
        ".".to_owned(),
        "..".to_owned(),
        "../outside".to_owned(),
        "/etc/passwd".to_owned(),
        "a\0b".to_owned(),
        "a/b".to_owned(),
        "x".repeat(100),
        "x".repeat(101),
        "é".repeat(50),
        "é".repeat(51),
        "\u{10FFFF}".repeat(64),
    ];
    let mut names = BTreeSet::new();
    for id in &ids {
        let name = log_file_name(id);
        assert!(
            name.starts_with("runlog-case-") && name.ends_with(".jsonl"),
            "{id:?}: {name}"
        );
        assert!(
            !name.contains('/') && !name.contains('\0') && name.len() <= 255,
            "{id:?}: {name}"
        );
        assert_eq!(log.path(id).parent(), Some(dir.as_path()), "{id:?}");
        assert_eq!(
            name.contains("sha256-"),
            id.len() > 100,
            "{id:?}: the form follows the length"
        );
        names.insert(name);
    }
    assert_eq!(names.len(), ids.len(), "two ids share a log");
    for id in &ids {
        log.append(id, at(0), event(0))
            .unwrap_or_else(|e| panic!("{id:?}: {e}"));
    }
    let on_disk = fs::read_dir(&dir)
        .expect("reads")
        .filter(|e| {
            e.as_ref()
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with("runlog-")
        })
        .count();
    assert_eq!(on_disk, ids.len(), "every log sits in the store directory");
}

/// The log file is written as the store's other files are: same mode as the case file.
#[test]
fn the_log_file_has_the_mode_of_the_case_file_beside_it() {
    let dir = store_dir("mode");
    let store = FileCaseStore::open(&dir).expect("opens");
    store
        .insert(CaseState {
            id: CaseId(CASE.to_owned()),
            protocol: "not-built-in@1".to_owned(),
            revision: 1,
            artifacts: BTreeMap::new(),
            inputs: CaseInputs::none(),
            independence: CaseIndependence::none(),
            evidence: Vec::new(),
            decisions: Vec::new(),
        })
        .expect("the case is written");
    let log = RunLog::open(&dir).expect("opens");
    log.append(CASE, at(0), event(0)).expect("appends");
    let mode = |p: &Path| fs::metadata(p).expect("metadata").permissions().mode() & 0o7777;
    let case = dir.join(format!("{}.json", loom_governor::case_file_stem(CASE)));
    assert_eq!(mode(&log.path(CASE)), mode(&case));
}

/// Eight threads, each with its own `RunLog` over one directory, append `per_thread` lines each to
/// one case: every line lands once, the log verifies. Each append reads the log and syncs its
/// file and directory, so the count sets the time.
fn eight_threads_append(label: &str, per_thread: usize) {
    let dir = store_dir(label);
    RunLog::open(&dir).expect("opens");
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let dir = dir.clone();
            thread::spawn(move || {
                let log = RunLog::open(&dir).expect("opens in a thread");
                for i in 0..per_thread {
                    log.append(
                        CASE,
                        at(i),
                        RunLogEvent::Terminated(Terminated {
                            reason: format!("t{t}-{i}"),
                        }),
                    )
                    .expect("an append from one of eight threads");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("a thread ends");
    }
    let read = RunLog::open(&dir)
        .expect("opens")
        .read(CASE)
        .expect("verifies");
    assert_eq!(read.len(), 8 * per_thread);
    let reasons: BTreeSet<String> = read
        .iter()
        .map(|l| match &l.event {
            RunLogEvent::Terminated(t) => t.reason.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(reasons.len(), 8 * per_thread, "every line once");
}

/// [`eight_threads_append`] with 5 lines a thread.
#[test]
fn eight_threads_with_their_own_run_logs_leave_every_line_once() {
    eight_threads_append("threads", 5);
}

/// [`eight_threads_append`] with 25 lines a thread, the count the case held before, in the slow
/// lane (`task slow`): about 23 s on a loaded host.
#[test]
fn slow_eight_threads_with_their_own_run_logs_leave_every_line_once() {
    eight_threads_append("threads-slow", 25);
}

/// A long line (4 MiB of event text) is written and read back whole.
#[test]
fn a_four_mebibyte_line_is_written_and_read_back_whole() {
    let (log, _path) = log_with("long", 1);
    let reason = "r".repeat(4 << 20);
    log.append(
        CASE,
        at(1),
        RunLogEvent::Terminated(Terminated {
            reason: reason.clone(),
        }),
    )
    .expect("a long line is taken");
    log.append(CASE, at(2), event(2)).expect("and the next");
    let read = log.read(CASE).expect("verifies");
    assert_eq!(
        read[1].event,
        RunLogEvent::Terminated(Terminated { reason })
    );
}

/// `ess/domains/runlog.yaml`, `loom.runlog.ActionFinished`: "its exit status or why it was
/// refused (one of the two)"; the story's Outcome: "action finished (exit or refusal, ...)". An
/// ActionFinished holding both, or neither, is no line the domain declares: an append of one is
/// refused and writes nothing, and a log holding one does not verify.
#[test]
fn an_action_finished_holds_an_exit_or_a_refusal_never_both_and_never_neither() {
    let finished = |exit: Option<i64>, refusal: Option<&str>| {
        RunLogEvent::ActionFinished(ActionFinished {
            action: "build".to_owned(),
            exit,
            refusal: refusal.map(str::to_owned),
            blob_digest: None,
            evidence: Vec::new(),
            revisions: Vec::new(),
        })
    };
    let (log, path) = log_with("exit-or-refusal", 1);
    log.append(CASE, at(1), finished(Some(0), None))
        .expect("an exit alone is taken");
    log.append(CASE, at(2), finished(None, Some("denied")))
        .expect("a refusal alone is taken");
    let before = fs::read(&path).expect("reads");
    for (name, event) in [
        ("both", finished(Some(1), Some("denied"))),
        ("neither", finished(None, None)),
    ] {
        let appended = log.append(CASE, at(3), event.clone());
        let after = fs::read(&path).expect("reads");
        assert!(
            appended.is_err() && after == before,
            "{name}: an ActionFinished with {name} of exit and refusal was appended: {appended:?}"
        );
        let line = RunLogLine {
            seq: 0,
            prev: None,
            at: at(0),
            event,
        };
        let mut bytes = encode(&line).into_bytes();
        bytes.push(b'\n');
        assert!(
            verify(&bytes).is_err(),
            "{name}: a log holding an ActionFinished with {name} of exit and refusal verifies"
        );
    }
}
