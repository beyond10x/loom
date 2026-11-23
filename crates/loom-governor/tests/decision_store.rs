//! [`FileDecisionStore`] keeps a decision request, and the records kept beside it, a file each in
//! a store directory, and they survive a restart (`story:decision-request-file-store`).
//!
//! Each test states one Acceptance line of the story; the last four hold the store's own refusals
//! of a second decision, of a file whose bytes it did not write and of a supersession that changes
//! more than the supersession. The two restart tests are real:
//! the test runs its own binary as a child process, twice, over one store directory, selected by
//! the test's name alone so it still selects itself as a module of a crate's one test binary. The
//! concurrent tests give each thread its own store over one directory and hold every thread at a
//! barrier after its read, so every write races every other.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, CaseId as CommissionCaseId, CommissionId, HoldSuspension,
    HoldSuspensionOutcome, ProposedActionArguments, RunId, SuspensionKey, SuspensionReason,
};
use b10x_loom_commission::ports::suspension::SuspensionPort as _;
use b10x_loom_decision::model::{
    ActionId, ArtifactId, ArtifactRevisionCitation, AutonomyLevel, CapabilityId, CaseId,
    CheckVerdict, Citation, Consequence, Decide, DecideOutcome, DecisionClassId, DecisionOption,
    DecisionRequestId, DecisionRequestSnapshot, DecisionRequestState, Escalate, EscalateOutcome,
    EscalationSuppressedRecord, Grant, Ground, GroundSource, Holder, NoSuchRequest, OptionLetter,
    PathLineCitation, Principal, PrincipalKind, ProducedDocument, ProducedRecord, ProposerKind,
    Recommend, RecordCitation, RecordKeyOpen, RecordProduced, RecordStore, Request, RequestOutcome,
    Revision, Strength, Supersede, SuppressionCause, UnblockKind, Unblocks,
};
use b10x_loom_decision::{
    AppendError, DecisionStore, InMemoryDecisionStore, StoreConflict, StoreFailure, Timestamp,
    WriteError, artifact_decision_history, decide, decision_requests, escalate,
    open_decisions_for_principal, recommend, record_produced, request, supersede,
};
use loom_governor::{
    CanonGovernor, CaseInputs, FallibleCaseStore as _, FileCaseStore, FileDecisionStore,
    FileSuspensionStore,
};
use sha2::{Digest as _, Sha256};

const DR1: &str = "DR-IDEA-1.billing-change.merge-1";
const DR2: &str = "DR-IDEA-1.billing-change.merge-2";
const DR3: &str = "DR-IDEA-1.billing-change.merge-3";
const KEY: &str = "change.merge@r9";
const ROLE: &str = "LOOM_DECISION_STORE_ROLE";
const DIR: &str = "LOOM_DECISION_STORE_DIR";
/// How often each race is run, each time in a fresh directory.
const ROUNDS: usize = 8;

fn id(text: &str) -> DecisionRequestId {
    DecisionRequestId(text.to_owned())
}

fn principal(text: &str) -> Principal {
    Principal(text.to_owned())
}

fn letter(text: &str) -> OptionLetter {
    OptionLetter(text.to_owned())
}

fn at(text: &str) -> Timestamp {
    Timestamp(text.to_owned())
}

fn option(name: &str, grant: Grant) -> DecisionOption {
    DecisionOption {
        letter: letter(name),
        does: format!("Option {name}: \"quoted\", a tab\tand \u{e9}."),
        cost: "None.".to_owned(),
        consequence: Consequence {
            admits: vec![ActionId("change.merge".to_owned())],
            blocks: Vec::new(),
            grant: Some(grant),
            answer: None,
            produces: Some(ProducedDocument::CanonAuthority),
        },
    }
}

/// An approval of `change.merge` under `record_key`, addressed to `addressed_to`: A grants, B
/// denies.
fn approval(request_id: &str, record_key: &str, addressed_to: &str) -> Request {
    Request {
        request_id: id(request_id),
        case_id: CaseId("IDEA-1.billing".to_owned()),
        question_key: "IDEA-1.billing|change.merge|".to_owned(),
        record_key: record_key.to_owned(),
        open_for_record: None,
        unblocks: Unblocks {
            kind: UnblockKind::Approval,
            action: ActionId("change.merge".to_owned()),
            capability: Some(CapabilityId("change.merge".to_owned())),
            question_id: None,
            outcome: None,
        },
        subject_artifact: ArtifactId("change".to_owned()),
        subject_revision: Revision("t0r1".to_owned()),
        subject_path: Some("work/change.md".to_owned()),
        question: "Merge the change at revision t0r1?".to_owned(),
        options: vec![option("A", Grant::Granted), option("B", Grant::Denied)],
        autonomy_level: AutonomyLevel::A3,
        decision_class: DecisionClassId("merge".to_owned()),
        holder: Holder::Person,
        eligible: vec![principal("operator"), principal("ana")],
        addressed_to: principal(addressed_to),
        opened_at: at("2026-11-21T11:00:00Z"),
        expires_at: Some(at("2026-11-22T11:00:00Z")),
        supersedes: None,
    }
}

fn decide_input(request_id: &str, option: &str) -> Decide {
    Decide {
        request_id: id(request_id),
        option: Some(letter(option)),
        answer: None,
        principal: principal("operator"),
        principal_kind: PrincipalKind::Person,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: vec![format!("Option {option}.")],
    }
}

fn record(request_id: &str, cause: SuppressionCause, kept_at: &str) -> EscalationSuppressedRecord {
    EscalationSuppressedRecord {
        request_id: id(request_id),
        decision_class: DecisionClassId("merge".to_owned()),
        subject_artifact: ArtifactId("change".to_owned()),
        subject_revision: Revision("t0r1".to_owned()),
        reasons: vec!["Suppressed.".to_owned(), "Twice.".to_owned()],
        best: Some(letter("B")),
        cause,
        kept_at: at(kept_at),
    }
}

/// The Open request `request` stores for `input`, as the in-memory store holds it.
fn opened(input: Request) -> DecisionRequestSnapshot {
    let request_id = input.request_id.clone();
    let mut store = InMemoryDecisionStore::new();
    let answer = request(&mut store, input).expect("the request is applied");
    assert!(
        matches!(answer, RequestOutcome::Requested { .. }),
        "{answer:?}"
    );
    store.read(&request_id).expect("reads").expect("stored")
}

/// `open` decided on `option`, as the in-memory store holds it.
fn decided(open: &DecisionRequestSnapshot, option: &str) -> DecisionRequestSnapshot {
    let mut store = InMemoryDecisionStore::with_requests(vec![open.clone()]);
    let answer =
        decide(&mut store, decide_input(&open.data.request_id.0, option)).expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );
    store
        .read(&open.data.request_id)
        .expect("reads")
        .expect("stored")
}

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("decision-store")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_store(dir: &Path) -> FileDecisionStore {
    FileDecisionStore::open(dir).expect("the store opens")
}

fn read(store: &FileDecisionStore, request_id: &str) -> DecisionRequestSnapshot {
    store
        .read(&id(request_id))
        .expect("the store reads")
        .unwrap_or_else(|| panic!("{request_id} is stored"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The file the story names for `request_id`: `decision-<hex>.json`, or
/// `decision-sha256-<hex of its SHA-256>.json` for an id over 100 bytes.
fn file_name(request_id: &str) -> String {
    if request_id.len() <= 100 {
        format!("decision-{}.json", hex(request_id.as_bytes()))
    } else {
        format!(
            "decision-sha256-{}.json",
            hex(&Sha256::digest(request_id.as_bytes()))
        )
    }
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory lists")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// The child's role and store directory, when this process is a child of a restart test.
fn child() -> Option<(String, PathBuf)> {
    let role = std::env::var(ROLE).ok()?;
    let dir = std::env::var(DIR).expect("the parent names the store directory");
    Some((role, PathBuf::from(dir)))
}

/// Runs this binary as a child selecting `test` by name in `role` over `dir`, and requires it to
/// run that one test and pass.
fn run_child(test: &str, role: &str, dir: &Path) {
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([test, "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(DIR, dir)
        .output()
        .expect("the child starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the {role} child failed: {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the {role} child ran the one test: {stdout}"
    );
}

/// This process's id, then `answers`, into `path`.
fn answer(path: &Path, answers: &str) {
    std::fs::write(path, format!("{}\n{answers}", std::process::id())).expect("written");
}

fn pid_and_answers(path: &Path) -> (String, String) {
    let text = std::fs::read_to_string(path).expect("the child answered");
    let (pid, answers) = text.split_once('\n').expect("a pid line");
    (pid.to_owned(), answers.to_owned())
}

#[test]
fn a_request_survives_a_process_restart() {
    if let Some((role, dir)) = child() {
        let mut store = open_store(&dir);
        if role == "write" {
            store
                .write(None, opened(approval(DR1, KEY, "operator")))
                .expect("the request is stored");
            let answered = decide(&mut store, decide_input(DR1, "B")).expect("applied");
            assert!(
                matches!(answered, DecideOutcome::Decided { .. }),
                "{answered:?}"
            );
        }
        let held = (read(&store, DR1), store.list().expect("lists"));
        answer(&dir.join(format!("{role}.txt")), &format!("{held:#?}"));
        return;
    }

    let dir = store_dir("restart");
    run_child("a_request_survives_a_process_restart", "write", &dir);
    run_child("a_request_survives_a_process_restart", "read", &dir);
    let (writer, written) = pid_and_answers(&dir.join("write.txt"));
    let (reader, reread) = pid_and_answers(&dir.join("read.txt"));
    assert_ne!(
        writer, reader,
        "the request was read back by another process"
    );
    assert_eq!(
        reread, written,
        "the second process reads the request the first one wrote"
    );
    let held = read(&open_store(&dir), DR1);
    assert_eq!(held.state, DecisionRequestState::Decided);
    assert_eq!(held.data.decided_option, Some(letter("B")));
    assert_eq!(held, decided(&opened(approval(DR1, KEY, "operator")), "B"));
}

/// Two threads, each with its own store over `dir`, read what is stored for `request_id`, wait for
/// each other, then each writes its own `next` from what it read.
fn race(
    dir: &Path,
    request_id: &str,
    writes: Vec<DecisionRequestSnapshot>,
) -> Vec<(DecisionRequestSnapshot, Result<(), WriteError>)> {
    let barrier = Arc::new(Barrier::new(writes.len()));
    let handles: Vec<_> = writes
        .into_iter()
        .map(|next| {
            let dir = dir.to_owned();
            let barrier = Arc::clone(&barrier);
            let request_id = id(request_id);
            std::thread::spawn(move || {
                let mut store = open_store(&dir);
                let held = store.read(&request_id).expect("the store reads");
                barrier.wait();
                let written = store.write(held.as_ref(), next.clone());
                (next, written)
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|handle| handle.join().expect("the thread ends"))
        .collect()
}

/// The one write that was kept, and the conflict the other was refused with.
fn one_kept(
    results: Vec<(DecisionRequestSnapshot, Result<(), WriteError>)>,
) -> (DecisionRequestSnapshot, StoreConflict) {
    let mut kept = Vec::new();
    let mut refused = Vec::new();
    for (next, written) in results {
        match written {
            Ok(()) => kept.push(next),
            Err(WriteError::Conflict(conflict)) => refused.push(conflict),
            Err(failure @ WriteError::Failure(_)) => panic!("no write fails: {failure:?}"),
        }
    }
    assert_eq!(kept.len(), 1, "exactly one write is kept: {kept:#?}");
    assert_eq!(refused.len(), 1, "exactly one write is refused");
    (kept.remove(0), refused.remove(0))
}

#[test]
fn two_decides_from_one_open_read_leave_one_decided_request() {
    let open = opened(approval(DR1, KEY, "operator"));
    for round in 0..ROUNDS {
        let dir = store_dir(&format!("two-decides-{round}"));
        open_store(&dir).write(None, open.clone()).expect("stored");

        let (first, conflict) = one_kept(race(
            &dir,
            DR1,
            vec![decided(&open, "A"), decided(&open, "B")],
        ));
        assert_eq!(
            conflict.request_id,
            id(DR1),
            "the conflict names the request"
        );
        assert_eq!(
            conflict.read.as_deref(),
            Some(&open),
            "the state read: Open"
        );
        assert_eq!(
            conflict.found.as_deref(),
            Some(&first),
            "the state found: the first Decide"
        );
        assert_eq!(conflict.record_key_held_by, None);
        assert_eq!(
            read(&open_store(&dir), DR1),
            first,
            "the file holds the first"
        );
    }
}

#[test]
fn two_new_requests_with_one_id_leave_one_request() {
    for round in 0..ROUNDS {
        let dir = store_dir(&format!("one-id-{round}"));
        let mut one = opened(approval(DR1, KEY, "operator"));
        one.data.question = "one".to_owned();
        let mut other = one.clone();
        other.data.question = "other".to_owned();

        let (kept, conflict) = one_kept(race(&dir, DR1, vec![one, other]));
        assert_eq!(conflict.request_id, id(DR1));
        assert_eq!(conflict.read, None, "both read no request");
        assert_eq!(conflict.found.as_deref(), Some(&kept));
        let store = open_store(&dir);
        assert_eq!(store.list().expect("lists"), vec![kept]);
    }
}

#[test]
fn two_new_requests_with_one_record_key_leave_one_and_request_answers_record_key_open() {
    for round in 0..ROUNDS {
        let dir = store_dir(&format!("one-record-key-{round}"));
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = [DR1, DR2]
            .map(|request_id| {
                let dir = dir.clone();
                let barrier = Arc::clone(&barrier);
                let next = opened(approval(request_id, KEY, "operator"));
                std::thread::spawn(move || {
                    let mut store = open_store(&dir);
                    assert_eq!(store.read(&next.data.request_id).expect("reads"), None);
                    barrier.wait();
                    let written = store.write(None, next.clone());
                    (next, written)
                })
            })
            .into_iter()
            .collect();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().expect("the thread ends"))
            .collect();
        let (holder, conflict) = one_kept(results);
        let refused = if holder.data.request_id == id(DR1) {
            DR2
        } else {
            DR1
        };
        assert_eq!(conflict.request_id, id(refused));
        assert_eq!(
            conflict.record_key_held_by,
            Some(holder.data.request_id.clone()),
            "the conflict names the holder of the record key"
        );
        let mut store = open_store(&dir);
        assert_eq!(store.list().expect("lists"), vec![holder.clone()]);

        let answer = request(&mut store, approval(DR3, KEY, "operator")).expect("applied");
        assert_eq!(
            answer,
            RequestOutcome::RecordKeyOpen {
                error: RecordKeyOpen {
                    request_id: id(DR3),
                    open_for_record: holder.data.request_id.clone(),
                },
            }
        );
        assert_eq!(store.list().expect("lists"), vec![holder]);
    }
}

#[test]
fn appended_records_survive_a_process_restart_in_order() {
    let first = record(DR1, SuppressionCause::Escalate, "2026-11-21T12:00:00Z");
    let second = record(DR1, SuppressionCause::Failure, "2026-11-21T12:01:00Z");
    if let Some((role, dir)) = child() {
        let mut store = open_store(&dir);
        if role == "write" {
            store
                .write(None, opened(approval(DR1, KEY, "operator")))
                .expect("stored");
            let before = read(&store, DR1);
            store.append_record(first.clone()).expect("appended");
            store.append_record(second.clone()).expect("appended");
            assert_eq!(read(&store, DR1), before, "an append leaves the request");
        }
        let held = (
            read(&store, DR1),
            store.records(&id(DR1)).expect("the records read"),
        );
        answer(&dir.join(format!("{role}.txt")), &format!("{held:#?}"));
        return;
    }

    let dir = store_dir("records-restart");
    run_child(
        "appended_records_survive_a_process_restart_in_order",
        "write",
        &dir,
    );
    run_child(
        "appended_records_survive_a_process_restart_in_order",
        "read",
        &dir,
    );
    let (writer, written) = pid_and_answers(&dir.join("write.txt"));
    let (reader, reread) = pid_and_answers(&dir.join("read.txt"));
    assert_ne!(
        writer, reader,
        "the records were read back by another process"
    );
    assert_eq!(reread, written);
    let store = open_store(&dir);
    assert_eq!(
        store.records(&id(DR1)).expect("reads"),
        vec![first, second],
        "both records, in the order appended"
    );
    assert_eq!(
        read(&store, DR1),
        opened(approval(DR1, KEY, "operator")),
        "the request is unchanged"
    );
    assert_eq!(store.records(&id(DR2)).expect("reads"), Vec::new());
}

#[test]
fn two_appends_beside_a_decide_leave_the_decided_request_and_both_records() {
    let open = opened(approval(DR1, KEY, "operator"));
    let next = decided(&open, "A");
    let records = [
        record(DR1, SuppressionCause::Veto, "2026-11-21T12:00:00Z"),
        record(DR1, SuppressionCause::Withheld, "2026-11-21T12:01:00Z"),
    ];
    for round in 0..ROUNDS {
        let dir = store_dir(&format!("appends-beside-a-decide-{round}"));
        open_store(&dir).write(None, open.clone()).expect("stored");
        let barrier = Arc::new(Barrier::new(3));
        let mut handles = Vec::new();
        for appended in records.clone() {
            let dir = dir.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let mut store = open_store(&dir);
                barrier.wait();
                store.append_record(appended).expect("appended");
            }));
        }
        {
            let dir = dir.clone();
            let barrier = Arc::clone(&barrier);
            let next = next.clone();
            handles.push(std::thread::spawn(move || {
                let mut store = open_store(&dir);
                let held = read(&store, DR1);
                barrier.wait();
                store.write(Some(&held), next).expect("the Decide is kept");
            }));
        }
        for handle in handles {
            handle.join().expect("the thread ends");
        }

        let mut store = open_store(&dir);
        assert_eq!(read(&store, DR1), next, "the request is Decided");
        let mut kept = store.records(&id(DR1)).expect("reads");
        kept.sort_by(|one, other| one.kept_at.0.cmp(&other.kept_at.0));
        assert_eq!(kept, records.to_vec(), "both records are in the file");

        let refused = store
            .append_record(record(
                "DR-unknown",
                SuppressionCause::Escalate,
                "2026-11-21T12:02:00Z",
            ))
            .expect_err("refused");
        assert_eq!(
            refused,
            AppendError::NoSuchRequest(NoSuchRequest {
                request_id: id("DR-unknown"),
            })
        );
        assert!(
            !file_names(&dir).contains(&file_name("DR-unknown")),
            "nothing is kept for an unknown request"
        );
    }
}

#[test]
fn a_write_into_a_read_only_directory_is_a_store_failure_and_keeps_every_byte() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = store_dir("read-only");
    let open = opened(approval(DR1, KEY, "operator"));
    let mut store = open_store(&dir);
    store.write(None, open.clone()).expect("stored");
    let path = dir.join(file_name(DR1));
    let before = std::fs::read(&path).expect("the file reads");

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    assert!(
        std::fs::write(dir.join("probe"), "x").is_err(),
        "this test needs a directory its user cannot write; run it as a user without \
         CAP_DAC_OVERRIDE"
    );
    let written = store.write(Some(&open), decided(&open, "A"));
    let appended = store.append_record(record(
        DR1,
        SuppressionCause::Failure,
        "2026-11-21T12:00:00Z",
    ));
    let added = store.write(None, opened(approval(DR2, "change.merge@r10", "operator")));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        matches!(written, Err(WriteError::Failure(_))),
        "a write the directory does not take is a store failure: {written:?}"
    );
    assert!(
        matches!(appended, Err(AppendError::Failure(_))),
        "{appended:?}"
    );
    assert!(matches!(added, Err(WriteError::Failure(_))), "{added:?}");
    assert_eq!(
        std::fs::read(&path).expect("the file reads"),
        before,
        "the request file keeps every byte it had"
    );
    assert_eq!(
        file_names(&dir),
        [".lock".to_owned(), file_name(DR1)],
        "no temporary file is left behind"
    );
    let reopened = open_store(&dir);
    assert_eq!(read(&reopened, DR1), open);
    assert_eq!(reopened.records(&id(DR1)).expect("reads"), Vec::new());
}

#[test]
fn a_half_written_temporary_file_of_a_dead_process_is_removed_when_the_store_opens() {
    let open = opened(approval(DR1, KEY, "operator"));
    // The bytes of the request decided, as the store writes them, from another directory.
    let elsewhere = store_dir("half-written-source");
    let mut source = open_store(&elsewhere);
    source.write(None, open.clone()).expect("stored");
    source
        .write(Some(&open), decided(&open, "A"))
        .expect("decided");
    let whole = std::fs::read(elsewhere.join(file_name(DR1))).expect("the file reads");

    let dir = store_dir("half-written");
    open_store(&dir).write(None, open.clone()).expect("stored");
    // A process id no live process has: a child that ended and was reaped.
    let mut ended = Command::new("true").spawn().expect("`true` starts");
    let dead = ended.id();
    ended.wait().expect("`true` ends");
    let temporary = dir.join(format!(".tmp-{dead}-0"));
    std::fs::write(&temporary, &whole[..whole.len() / 2]).expect("the half file is written");

    let store = open_store(&dir);
    assert!(
        !temporary.exists(),
        "the store removed the half-written file"
    );
    assert_eq!(
        read(&store, DR1),
        open,
        "the request reads its earlier state"
    );
    assert_eq!(file_names(&dir), [".lock".to_owned(), file_name(DR1)]);
}

#[test]
fn a_long_id_is_kept_under_a_digest_name_and_a_hundred_byte_id_under_its_hex() {
    let dir = store_dir("long-ids");
    let hundred = format!("DR-{}", "x".repeat(97));
    let longer = format!("DR-{}", "y".repeat(98));
    assert_eq!((hundred.len(), longer.len()), (100, 101));
    let mut store = open_store(&dir);
    let mut kept = Vec::new();
    for (request_id, key) in [(&hundred, "k100"), (&longer, "k101")] {
        let next = opened(approval(request_id, key, "operator"));
        store
            .write(None, next.clone())
            .expect("stored, never refused");
        kept.push(next);
    }
    assert!(file_name(&hundred).starts_with("decision-4452"));
    assert!(file_name(&longer).starts_with("decision-sha256-"));
    let mut expected = vec![".lock".to_owned(), file_name(&hundred), file_name(&longer)];
    expected.sort();
    assert_eq!(file_names(&dir), expected);
    let reopened = open_store(&dir);
    assert_eq!(read(&reopened, &hundred), kept[0]);
    assert_eq!(read(&reopened, &longer), kept[1]);
    assert_eq!(reopened.list().expect("lists"), kept);
}

/// The failure names the file `path`.
fn names_the_file(failure: &StoreFailure, path: &Path) {
    let file = path.file_name().unwrap().to_string_lossy();
    assert!(
        failure.at.contains(file.as_ref()) || failure.why.contains(file.as_ref()),
        "the failure names {file}: {failure:?}"
    );
}

#[test]
fn a_file_holding_another_requests_id_and_a_truncated_file_are_refused_naming_the_file() {
    let dir = store_dir("another-id");
    let mut store = open_store(&dir);
    store
        .write(None, opened(approval(DR1, KEY, "operator")))
        .expect("stored");
    store
        .write(None, opened(approval(DR2, "change.merge@r10", "operator")))
        .expect("stored");
    let path = dir.join(file_name(DR1));
    std::fs::copy(dir.join(file_name(DR2)), &path).expect("copied");
    let copied = std::fs::read(&path).unwrap();
    let refused = store.read(&id(DR1)).expect_err("refused");
    names_the_file(&refused, &path);
    names_the_file(&store.list().expect_err("refused"), &path);
    assert!(
        matches!(
            store.append_record(record(
                DR1,
                SuppressionCause::Escalate,
                "2026-11-21T12:00:00Z"
            )),
            Err(AppendError::Failure(_))
        ),
        "nothing is kept beside a file holding another request"
    );
    assert_eq!(std::fs::read(&path).unwrap(), copied, "never repaired");

    let dir = store_dir("truncated");
    let mut store = open_store(&dir);
    let open = opened(approval(DR1, KEY, "operator"));
    store.write(None, open.clone()).expect("stored");
    let path = dir.join(file_name(DR1));
    let whole = std::fs::read(&path).unwrap();
    std::fs::write(&path, &whole[..whole.len() - 7]).unwrap();
    let refused = store.read(&id(DR1)).expect_err("refused");
    names_the_file(&refused, &path);
    names_the_file(&store.list().expect_err("refused"), &path);
    let written = store.write(Some(&open), decided(&open, "A"));
    assert!(
        matches!(&written, Err(WriteError::Failure(failure)) if failure.at.contains(&file_name(DR1))),
        "{written:?}"
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        whole[..whole.len() - 7],
        "never repaired"
    );
}

/// The `chg-1842` fixture's artifact revisions and case inputs.
fn fixture() -> (BTreeMap<String, String>, CaseInputs) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    let fixture: serde_json::Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(path).expect("the fixture reads"))
            .expect("the fixture parses");
    let revisions = fixture["case"]["artifacts"]
        .as_object()
        .expect("case artifacts")
        .iter()
        .map(|(artifact, entry)| {
            let revision = entry["revision"].as_str().expect("revision");
            (artifact.clone(), revision.to_owned())
        })
        .collect();
    let inputs = fixture["case"]["inputs"]
        .as_object()
        .expect("case inputs")
        .iter()
        .map(|(input, value)| {
            let value = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (input.clone(), value)
        })
        .collect::<Vec<_>>();
    (
        revisions,
        CaseInputs::try_from_pairs(inputs).expect("distinct inputs"),
    )
}

#[test]
fn a_case_a_suspension_and_a_decision_in_one_directory_are_each_read_back_by_their_own_store() {
    let dir = store_dir("one-directory");
    let (revisions, inputs) = fixture();
    let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the case store opens"));
    let case = governor
        .open("software-change@1", revisions, inputs)
        .expect("the case opens");
    let held_case = FileCaseStore::open(&dir)
        .expect("opens")
        .get(&case)
        .expect("reads")
        .expect("held");

    let commission_id = CommissionId(Uuid("00000000-0000-4000-8000-000000000101".to_owned()));
    let hold = HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id: commission_id.clone(),
        run_id: RunId(Uuid("00000000-0000-4000-8000-000000000201".to_owned())),
        run_case_revision: 1,
        action_request_id: Some(ActionRequestId(Uuid(
            "00000000-0000-4000-8000-000000000301".to_owned(),
        ))),
        case_id: CommissionCaseId(case.0.clone()),
        expected_case_revision: Some(1),
        action: Some("repository.merge".to_owned()),
        arguments: Some(ProposedActionArguments(CommissionValue::Null)),
        reason: SuspensionReason::Authority(CommissionValue::Null),
    };
    let suspensions = FileSuspensionStore::open(&dir).expect("the suspension store opens");
    assert!(matches!(
        suspensions.hold(hold),
        HoldSuspensionOutcome::Held { .. }
    ));
    let held_suspension = suspensions
        .get(&commission_id)
        .expect("reads")
        .expect("held");

    let open = opened(approval(DR1, KEY, "operator"));
    open_store(&dir).write(None, open.clone()).expect("stored");

    let names = file_names(&dir);
    assert!(
        names.iter().any(|name| name.starts_with("case-"))
            && names.iter().any(|name| name.starts_with("suspension-"))
            && names.contains(&file_name(DR1)),
        "the three files share the directory: {names:?}"
    );
    assert_eq!(
        FileCaseStore::open(&dir)
            .expect("opens")
            .get(&case)
            .expect("reads"),
        Some(held_case)
    );
    assert_eq!(
        FileSuspensionStore::open(&dir)
            .expect("opens")
            .get(&commission_id)
            .expect("reads"),
        Some(held_suspension)
    );
    let decisions = open_store(&dir);
    assert_eq!(read(&decisions, DR1), open);
    assert_eq!(decisions.list().expect("lists"), vec![open]);
}

/// Grounds citing a path and line, a record id and an artifact revision, each supporting A.
fn grounds() -> Vec<Ground> {
    vec![
        Ground {
            source: GroundSource::Story,
            citation: Citation::PathLine(PathLineCitation {
                path: "docs/merge.md".to_owned(),
                line: 3,
                line_end: Some(5),
                digest: Some("sha256:00".to_owned()),
            }),
            supports: Some(letter("A")),
            how: "The story states the merge.".to_owned(),
        },
        Ground {
            source: GroundSource::PriorDecision,
            citation: Citation::RecordId(RecordCitation {
                store: RecordStore::SdcDecisions,
                record_id: "DR-IDEA-0-1".to_owned(),
            }),
            supports: Some(letter("A")),
            how: "Decided so before.".to_owned(),
        },
        Ground {
            source: GroundSource::Goals,
            citation: Citation::ArtifactRevision(ArtifactRevisionCitation {
                artifact: ArtifactId("goals".to_owned()),
                revision: Revision("g3".to_owned()),
                case: None,
                entry: Some("product/p1".to_owned()),
                digest: None,
            }),
            supports: None,
            how: "The goal names it.".to_owned(),
        },
    ]
}

#[test]
fn open_decisions_for_principal_over_the_file_store_equals_the_in_memory_view() {
    let dir = store_dir("views");
    let mut memory = InMemoryDecisionStore::new();
    let mut file = open_store(&dir);
    let addressed = ["operator", "ana", "operator", "ana", "decider:x"];
    for n in 0..20 {
        let request_id = format!("DR-IDEA-{n}.billing-change.merge-1");
        let input = approval(&request_id, &format!("change.merge@r{n}"), addressed[n % 5]);
        let answered = request(&mut memory, input.clone()).expect("applied");
        assert_eq!(request(&mut file, input).expect("applied"), answered);
        match n % 4 {
            1 => {
                let input = decide_input(&request_id, if n % 8 == 1 { "A" } else { "B" });
                let answered = decide(&mut memory, input.clone()).expect("applied");
                assert_eq!(decide(&mut file, input).expect("applied"), answered);
                let produced = RecordProduced {
                    request_id: id(&request_id),
                    produced: ProducedRecord {
                        document: ProducedDocument::CanonAuthority,
                        record: "change.merge@r9".to_owned(),
                        case_revision: Revision("r9".to_owned()),
                        aep_record: Some(format!("approval:{n}")),
                    },
                };
                let answered = record_produced(&mut memory, produced.clone()).expect("applied");
                assert_eq!(
                    record_produced(&mut file, produced).expect("applied"),
                    answered
                );
            }
            2 => {
                let input = Escalate {
                    request_id: id(&request_id),
                    escalate_to: principal("ana"),
                    reason: "Ana owns billing.".to_owned(),
                };
                let answered = escalate(&mut memory, input.clone()).expect("applied");
                assert!(
                    matches!(answered, EscalateOutcome::Escalated { .. }),
                    "{answered:?}"
                );
                assert_eq!(escalate(&mut file, input).expect("applied"), answered);
            }
            3 => {
                let input = Recommend {
                    request_id: id(&request_id),
                    recommended_option: Some(letter("A")),
                    recommended_answer: None,
                    strength: Strength::Strong,
                    default_on_silence: true,
                    grounds: grounds(),
                    proposed_by: ProposerKind::Rule,
                    check_verdict: CheckVerdict::Verified,
                    check_report: "3 grounds, 3 resolve".to_owned(),
                };
                let answered = recommend(&mut memory, input.clone()).expect("applied");
                assert_eq!(recommend(&mut file, input).expect("applied"), answered);
            }
            _ if n == 8 => {
                let input = Supersede {
                    request_id: id(&request_id),
                    superseded_by: id("DR-IDEA-9.billing-change.merge-1"),
                    reason: "Asked again.".to_owned(),
                };
                let answered = supersede(&mut memory, input.clone()).expect("applied");
                assert_eq!(supersede(&mut file, input).expect("applied"), answered);
            }
            _ => {}
        }
    }
    assert!(
        memory
            .list()
            .expect("lists")
            .iter()
            .any(|held| held.data.grounds == grounds()),
        "a stored request carries every form of citation"
    );

    let file = open_store(&dir);
    assert_eq!(file.list().expect("lists"), memory.list().expect("lists"));
    for who in ["operator", "ana", "decider:x", "nobody"] {
        let from_memory = open_decisions_for_principal(&memory, &principal(who)).expect("reads");
        assert_eq!(
            open_decisions_for_principal(&file, &principal(who)).expect("reads"),
            from_memory,
            "OpenDecisionsForPrincipal for {who}"
        );
    }
    assert!(
        !open_decisions_for_principal(&file, &principal("ana"))
            .expect("reads")
            .is_empty()
    );
    assert_eq!(
        decision_requests(&file).expect("reads"),
        decision_requests(&memory).expect("reads")
    );
    let change = ArtifactId("change".to_owned());
    assert_eq!(
        artifact_decision_history(&file, &change).expect("reads"),
        artifact_decision_history(&memory, &change).expect("reads")
    );
}

/// The store's own refusal (the batch review's criterion): a request once decided is never
/// decided again or changed by a later write, beyond recording its produced record once and
/// being superseded, and a superseded or expired request takes no write.
#[test]
fn a_decided_request_is_never_decided_again_or_changed_by_a_later_write() {
    let dir = store_dir("decided-once");
    let mut store = open_store(&dir);
    let open = opened(approval(DR1, KEY, "operator"));
    store.write(None, open.clone()).expect("stored");
    let first = decided(&open, "A");
    store.write(Some(&open), first.clone()).expect("decided");
    let path = dir.join(file_name(DR1));
    let bytes = std::fs::read(&path).unwrap();

    let again = decided(&open, "B");
    let refused = store.write(Some(&first), again);
    assert!(
        matches!(refused, Err(WriteError::Failure(_))),
        "{refused:?}"
    );
    let mut reopened = first.clone();
    reopened.state = DecisionRequestState::Open;
    let refused = store.write(Some(&first), reopened);
    assert!(
        matches!(refused, Err(WriteError::Failure(_))),
        "{refused:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "nothing was written");

    let answered = record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: ProducedRecord {
                document: ProducedDocument::CanonAuthority,
                record: "change.merge@r9".to_owned(),
                case_revision: Revision("r9".to_owned()),
                aep_record: None,
            },
        },
    )
    .expect("applied");
    assert!(
        format!("{answered:?}").starts_with("Recorded"),
        "{answered:?}"
    );
    let recorded = read(&store, DR1);
    let mut changed = recorded.clone();
    changed.data.produced_record.as_mut().unwrap().record = "change.merge@r10".to_owned();
    let refused = store.write(Some(&recorded), changed);
    assert!(
        matches!(refused, Err(WriteError::Failure(_))),
        "{refused:?}"
    );

    supersede(
        &mut store,
        Supersede {
            request_id: id(DR1),
            superseded_by: id(DR2),
            reason: "Asked again.".to_owned(),
        },
    )
    .expect("applied");
    let superseded = read(&store, DR1);
    assert_eq!(superseded.state, DecisionRequestState::Superseded);
    assert_eq!(superseded.data.decided_option, Some(letter("A")));
    let mut moved = superseded.clone();
    moved.data.superseded_reason = Some("Another reason.".to_owned());
    let refused = store.write(Some(&superseded), moved);
    assert!(
        matches!(refused, Err(WriteError::Failure(_))),
        "{refused:?}"
    );
    assert_eq!(read(&store, DR1), superseded);
}

/// The store's own refusal (the batch review's criterion): a file is read only as the store
/// writes it, so one whose bytes were changed, or that keeps a record of another request, is
/// refused naming the file.
#[test]
fn a_file_whose_bytes_were_changed_is_refused_naming_the_file() {
    let dir = store_dir("changed-bytes");
    let mut store = open_store(&dir);
    let open = opened(approval(DR1, KEY, "operator"));
    store.write(None, open).expect("stored");
    store
        .append_record(record(
            DR1,
            SuppressionCause::Escalate,
            "2026-11-21T12:00:00Z",
        ))
        .expect("appended");
    let path = dir.join(file_name(DR1));
    let text = std::fs::read_to_string(&path).unwrap();

    let spaced = text.replacen(':', ": ", 1);
    std::fs::write(&path, &spaced).unwrap();
    names_the_file(&store.read(&id(DR1)).expect_err("refused"), &path);

    let other = text.replacen(
        &format!("\"request_id\":\"{DR1}\",\"decision_class\""),
        &format!("\"request_id\":\"{DR2}\",\"decision_class\""),
        1,
    );
    assert_ne!(other, text, "the record's request id is rewritten");
    std::fs::write(&path, &other).unwrap();
    names_the_file(&store.records(&id(DR1)).expect_err("refused"), &path);

    std::fs::write(&path, &text).unwrap();
    assert_eq!(store.records(&id(DR1)).expect("reads").len(), 1);
}

/// The store's own refusal (security review of the w7 batch): a decision is an option letter and
/// what it decided is that option's consequence, so a write that supersedes a decided request
/// cannot turn the decided option's grant into a denial.
#[test]
fn a_superseding_write_cannot_turn_the_decided_grant_into_a_denial() {
    let dir = store_dir("superseded-consequence");
    let mut store = open_store(&dir);
    let open = opened(approval(DR1, KEY, "operator"));
    store.write(None, open.clone()).expect("stored");
    let first = decided(&open, "A");
    store.write(Some(&open), first).expect("decided on A");
    let held = read(&store, DR1);
    let grant_of_a = |snapshot: &DecisionRequestSnapshot| {
        snapshot
            .data
            .options
            .iter()
            .find(|option| option.letter.0 == "A")
            .and_then(|option| option.consequence.grant)
    };
    assert_eq!(grant_of_a(&held), Some(Grant::Granted), "A grants");
    let path = dir.join(file_name(DR1));
    let bytes = std::fs::read(&path).unwrap();

    let mut rewritten = superseded_write(&held);
    for option in &mut rewritten.data.options {
        option.consequence.grant = Some(if option.letter.0 == "A" {
            Grant::Denied
        } else {
            Grant::Granted
        });
    }
    let answered = store.write(Some(&held), rewritten);
    let after = read(&store, DR1);
    assert!(
        matches!(answered, Err(WriteError::Failure(_))) && std::fs::read(&path).unwrap() == bytes,
        "a Superseded write that turns the decided option A from a grant into a denial was \
         answered {answered:?}; the store now holds {:?} decided on {:?}, whose consequence is \
         {:?}",
        after.state,
        after.data.decided_option,
        grant_of_a(&after)
    );
}

/// `held` superseded by `DR2`, as `supersede` writes it, and nothing else changed.
fn superseded_write(held: &DecisionRequestSnapshot) -> DecisionRequestSnapshot {
    let mut next = held.clone();
    next.state = DecisionRequestState::Superseded;
    next.data.superseded_by = Some(id(DR2));
    next.data.superseded_reason = Some("Asked again.".to_owned());
    next
}

/// The class of the refusal above: a write that supersedes a decided request changes its state,
/// `superseded_by` and `superseded_reason`, and records a produced record it did not hold yet,
/// and nothing else. Each change below rides on an otherwise well-formed supersession and is
/// refused, writing nothing; the supersession alone, and with its first produced record, is kept.
#[test]
fn a_superseding_write_changes_nothing_but_the_supersession() {
    type Change = fn(&mut DecisionRequestSnapshot);
    let changes: [(&str, Change); 8] = [
        ("options", |next| next.data.options.reverse()),
        ("eligible", |next| next.data.eligible.push(principal("eve"))),
        ("holder", |next| next.data.holder = Holder::Rule),
        ("case_id", |next| {
            next.data.case_id = CaseId("IDEA-2.other".to_owned());
        }),
        ("question", |next| {
            next.data.question = "Deploy the change?".to_owned();
        }),
        ("record_key", |next| {
            next.data.record_key = "change.merge@r10".to_owned();
        }),
        ("subject_revision", |next| {
            next.data.subject_revision = Revision("t0r2".to_owned());
        }),
        ("unblocks", |next| {
            next.data.unblocks.action = ActionId("change.deploy".to_owned());
        }),
    ];
    for (field, change) in changes {
        let dir = store_dir(&format!("superseded-changes-{field}"));
        let mut store = open_store(&dir);
        let open = opened(approval(DR1, KEY, "operator"));
        store.write(None, open.clone()).expect("stored");
        store
            .write(Some(&open), decided(&open, "A"))
            .expect("decided on A");
        let held = read(&store, DR1);
        let path = dir.join(file_name(DR1));
        let bytes = std::fs::read(&path).unwrap();
        let mut next = superseded_write(&held);
        change(&mut next);
        assert_ne!(next.data, superseded_write(&held).data, "{field} changed");
        let answered = store.write(Some(&held), next);
        assert!(
            matches!(answered, Err(WriteError::Failure(_))),
            "a Superseded write that also changes `{field}` was answered {answered:?}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "a refused write of `{field}` wrote nothing"
        );
    }

    let dir = store_dir("superseded-with-its-record");
    let mut store = open_store(&dir);
    let open = opened(approval(DR1, KEY, "operator"));
    store.write(None, open.clone()).expect("stored");
    store
        .write(Some(&open), decided(&open, "A"))
        .expect("decided on A");
    let held = read(&store, DR1);
    let mut next = superseded_write(&held);
    next.data.produced_record = Some(ProducedRecord {
        document: ProducedDocument::CanonAuthority,
        record: "change.merge@r9".to_owned(),
        case_revision: Revision("r9".to_owned()),
        aep_record: None,
    });
    store
        .write(Some(&held), next.clone())
        .expect("a supersession that records its first produced record is kept");
    assert_eq!(read(&store, DR1), next);
}
