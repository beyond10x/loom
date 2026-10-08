//! A case the governor holds in a [`FileCaseStore`] survives the process that held it (story
//! `governor-restart-durability`).
//!
//! The restart is real: the test runs its own binary twice as a child process over one store
//! directory. The first child opens a `software.change/1` case on the `chg-1842` fixture's
//! revisions, submits evidence, records an artifact revision and an observation, and exits. It runs
//! the same script on a governor over a [`MemoryCaseStore`], which never writes anything, and
//! records that governor's answers: the oracle. The second child starts a new governor over the
//! store directory, registering nothing the first did not (`software-change@1` is built in; no
//! protocol definition is stored), and records what it answers. The answers must equal the
//! oracle's, down to the frontier id, which hashes the case and Canon's decision bytes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort as _, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, FallibleCaseStore, FileCaseStore, MemoryCaseStore, OpenError};
use serde_json::Value;

const PROTOCOL: &str = "software-change@1";
const PRODUCER: &str = "service:ci";
const ROLE: &str = "LOOM_RESTART_DURABILITY_ROLE";
const DIR: &str = "LOOM_RESTART_DURABILITY_DIR";
const TEST: &str = "a_case_survives_a_process_restart";

#[test]
fn a_case_survives_a_process_restart() {
    if let Ok(role) = std::env::var(ROLE) {
        let dir = PathBuf::from(std::env::var(DIR).expect("the parent names the store directory"));
        match role.as_str() {
            "write" => write_case(&dir),
            "read" => read_case(&dir),
            other => panic!("unknown child role `{other}`"),
        }
        return;
    }

    let dir = store_dir("restart");
    run_child("write", &dir);
    run_child("read", &dir);

    let (writer, oracle) = pid_and_answers(&dir.join("oracle.txt"));
    let (_, before) = pid_and_answers(&dir.join("before.txt"));
    let (reader, after) = pid_and_answers(&dir.join("after.txt"));
    assert_ne!(writer, reader, "the case was read back by another process");
    assert_eq!(
        before, oracle,
        "before the restart the file-backed governor answers as the in-memory one"
    );
    assert_eq!(
        after, oracle,
        "after the restart current_revision, frontier, completion, revisions, evidence and \
         observations answer exactly as a governor that never stopped"
    );
    assert!(
        after.contains("current_revision: 2"),
        "the artifact revision was recorded:\n{after}"
    );
    assert!(
        after.contains("ApprovalRequired"),
        "the applying record moved repository.merge to approval-required:\n{after}"
    );
    let compact: String = after.split_whitespace().collect();
    for spelling in [
        r#"Number("1.50",)"#,
        r#"Number("1e3",)"#,
        r#"Number("-0",)"#,
        r#"Number("0.10",)"#,
    ] {
        assert!(
            compact.contains(spelling),
            "{spelling} keeps its spelling:\n{after}"
        );
    }

    let held = FileCaseStore::open(&dir)
        .expect("the store opens")
        .get(&CaseId(read(&dir.join("case-id.txt"))))
        .expect("the store reads")
        .expect("the store holds the case");
    assert_eq!(
        held.evidence
            .iter()
            .map(|held| held.applies)
            .collect::<Vec<_>>(),
        [true, false],
        "whether each record applies is kept as it was decided"
    );
}

/// The first life: the script on the file store and on the in-memory oracle.
fn write_case(dir: &Path) {
    let oracle = CanonGovernor::new(MemoryCaseStore::default());
    let case = script(&oracle);
    record(&oracle, &case, &dir.join("oracle.txt"));

    let governor = CanonGovernor::new(FileCaseStore::open(dir).expect("the store opens"));
    assert_eq!(script(&governor), case, "both governors open the same id");
    record(&governor, &case, &dir.join("before.txt"));
    std::fs::write(dir.join("case-id.txt"), &case.0).expect("the case id is written");
}

/// The second life: a new governor over the same directory, then the same questions.
fn read_case(dir: &Path) {
    let governor = CanonGovernor::new(FileCaseStore::open(dir).expect("the store opens"));
    let case = CaseId(read(&dir.join("case-id.txt")));
    record(&governor, &case, &dir.join("after.txt"));
}

/// Opens the case, submits one record that applies and one that does not, records a new artifact
/// revision and an observation.
fn script<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> CaseId {
    let case = governor
        .open(PROTOCOL, fixture_revisions())
        .expect("a case with every declared artifact opens");
    let mut ids = Ids::default();

    let pass = serde_json::json!({
        "format": "canon-evidence/1",
        "id": "tests-r2",
        "kind": "test_result",
        "result": "pass",
        "subject": "implementation",
        "subject_revision": "R2",
    });
    submit(
        governor,
        &case,
        &mut ids,
        &json::parse(&pass.to_string()).unwrap(),
    );
    // Members out of order, a number spelled `1.50` and one spelled `1e3`: kept as they arrived,
    // and set aside because they are no canon-evidence/1 record.
    let odd = json::parse(r#"{"z":1.50,"a":[1e3,-0,{"b":null,"a":true}],"kind":"note"}"#).unwrap();
    submit(governor, &case, &mut ids, &odd);
    governor
        .update_revision(&case, "release", "v1")
        .expect("a declared artifact takes a new revision");

    governor
        .observe(Observation::new(ObservationData {
            observation_id: ObservationId(ids.next()),
            source: "ci".to_owned(),
            subject: case.0.clone(),
            observed_at: Timestamp("2026-10-04T11:00:00Z".to_owned()),
            payload: json::parse(r#"{"run":7,"took":"2.50","ratio":0.10}"#).unwrap(),
        }))
        .expect("the store keeps the observation");
    case
}

/// What the governor answers about `case`, with this process's id on the first line.
fn record<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId, path: &Path) {
    let mut out = format!("{}\n", std::process::id());
    let revision = governor.current_revision(case).expect("current revision");
    let frontier = governor.frontier(case).expect("frontier").into_data();
    let completion = governor.completion(case).expect("completion");
    let revisions = governor.revisions(case).expect("revisions");
    let evidence = governor.evidence(case).expect("evidence");
    let observations = governor.try_observations().expect("observations");
    writeln!(out, "current_revision: {revision}").unwrap();
    writeln!(out, "frontier: {frontier:#?}").unwrap();
    writeln!(out, "completion: {completion:?}").unwrap();
    writeln!(out, "revisions: {revisions:#?}").unwrap();
    writeln!(out, "evidence: {evidence:#?}").unwrap();
    writeln!(out, "observations: {observations:#?}").unwrap();
    std::fs::write(path, out).expect("the answers are written");
}

fn pid_and_answers(path: &Path) -> (String, String) {
    let text = read(path);
    let (pid, answers) = text.split_once('\n').expect("a pid line");
    (pid.to_owned(), answers.to_owned())
}

#[test]
fn a_failed_write_is_an_error_and_leaves_the_case_as_it_was() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = store_dir("failed-write");
    let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
    let case = governor
        .open(PROTOCOL, fixture_revisions())
        .expect("the case opens");
    let before = governor.current_revision(&case).expect("current revision");
    let frontier = governor.frontier(&case).expect("frontier").into_data();

    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    assert!(
        std::fs::write(dir.join("probe"), "x").is_err(),
        "this test needs a directory its user cannot write; run it as a user without \
         CAP_DAC_OVERRIDE"
    );
    let update = governor.update_revision(&case, "release", "v9");
    let mut ids = Ids::default();
    let evidence = governor_evidence(&case, &mut ids, before);
    let submitted = submit_evidence(&governor, PRODUCER, evidence);
    let opened = governor.open(PROTOCOL, fixture_revisions());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        matches!(
            update,
            Err(loom_governor::UpdateError::StoreUnavailable { .. })
        ),
        "a revision the store cannot write is the store's error: {update:?}"
    );
    assert!(
        submitted.is_err(),
        "evidence the store cannot write is refused: {submitted:?}"
    );
    assert!(
        matches!(opened, Err(OpenError::StoreUnavailable { .. })),
        "a case the store cannot write is the store's error, not an id collision: {opened:?}"
    );

    let reread = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
    assert_eq!(reread.current_revision(&case), Ok(before));
    assert_eq!(reread.evidence(&case), Ok(Vec::new()));
    assert_eq!(
        reread.frontier(&case).expect("frontier").into_data(),
        frontier
    );
    let names = file_names(&dir);
    assert_eq!(
        names.iter().filter(|name| name.as_str() != ".lock").count(),
        1,
        "the case's file and the lock, no temporary file left behind: {names:?}"
    );
}

#[test]
fn an_unreadable_case_file_is_an_error_never_a_panic() {
    let dir = store_dir("unreadable");
    let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
    let case = governor
        .open(PROTOCOL, fixture_revisions())
        .expect("the case opens");
    let files: Vec<String> = file_names(&dir)
        .into_iter()
        .filter(|name| name.starts_with("case-"))
        .collect();
    assert_eq!(files.len(), 1, "one file per case: {files:?}");
    std::fs::write(dir.join(&files[0]), "{\"id\":").unwrap();

    let store = FileCaseStore::open(&dir).expect("the store opens");
    assert!(
        store.get(&case).is_err(),
        "a truncated case file is an error"
    );
    assert_eq!(
        governor.current_revision(&case),
        Err(GovernorError::GovernorUnavailable)
    );
    assert_eq!(
        store.insert(sample_state(&case)).map_err(|e| e.to_string()),
        Ok(false),
        "an unreadable case is still held under its id and never replaced"
    );
    assert!(
        store.get(&case).is_err(),
        "the unreadable file is left as it was"
    );
}

#[test]
fn a_case_id_names_one_file_inside_the_store() {
    let dir = store_dir("ids");
    let store = FileCaseStore::open(&dir).expect("the store opens");
    for id in ["../escape", "a/b", "", "CON", "case-1"] {
        let case = CaseId(id.to_owned());
        assert_eq!(
            store.insert(sample_state(&case)).map_err(|e| e.to_string()),
            Ok(true)
        );
        assert_eq!(
            store.get(&case).map_err(|e| e.to_string()),
            Ok(Some(sample_state(&case))),
            "case id {id:?}"
        );
        assert_eq!(
            store.insert(sample_state(&case)).map_err(|e| e.to_string()),
            Ok(false)
        );
    }
    let names = file_names(&dir);
    assert_eq!(
        names
            .iter()
            .filter(|name| name.starts_with("case-"))
            .count(),
        5,
        "one file per case, all inside the store: {names:?}"
    );
    assert!(!dir.parent().unwrap().join("escape").exists());
}

/// The names of the files directly in `dir`, sorted.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn sample_state(case: &CaseId) -> loom_governor::CaseState {
    loom_governor::CaseState {
        id: case.clone(),
        protocol: PROTOCOL.to_owned(),
        revision: 1,
        artifacts: BTreeMap::from([("intent".to_owned(), "i1".to_owned())]),
        evidence: Vec::new(),
    }
}

fn governor_evidence(case: &CaseId, ids: &mut Ids, revision: i64) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: case.clone(),
        kind: "test_result".to_owned(),
        subject_revision: revision,
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts: json::Value::Null,
        provenance: json::Value::Null,
    }
}

fn submit<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    ids: &mut Ids,
    facts: &json::Value,
) {
    let kind = match facts.member("kind") {
        Some(json::Value::Text(kind)) => kind.clone(),
        _ => panic!("facts name a kind"),
    };
    let evidence = EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: case.clone(),
        kind,
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts: facts.clone(),
        provenance: json::parse(r#"{"source":"restart_durability","attempt":1.0}"#).unwrap(),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

/// Runs this test's binary as a child in `role` over `dir` and requires it to exit 0.
fn run_child(role: &str, dir: &Path) {
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(DIR, dir)
        .output()
        .expect("the child starts");
    assert!(
        output.status.success(),
        "the {role} child failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 passed"),
        "the {role} child ran the one test: {stdout}"
    );
}

/// A fresh directory for one test under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("restart-durability")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The `chg-1842` fixture's artifact revisions.
fn fixture_revisions() -> BTreeMap<String, String> {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest).join("tests/fixtures/chg-1842.fixture.yaml");
    let fixture: Value = serde_yaml_ng::from_str(&read(&path)).expect("the fixture reads");
    assert_eq!(fixture["id"], "chg-1842");
    fixture["case"]["artifacts"]
        .as_object()
        .expect("case artifacts")
        .iter()
        .map(|(artifact, entry)| {
            let revision = entry["revision"].as_str().expect("revision");
            (artifact.clone(), revision.to_owned())
        })
        .collect()
}

/// Distinct UUIDs, in order.
#[derive(Default)]
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> Uuid {
        self.0 += 1;
        Uuid(format!("00000000-0000-4000-8000-{:012x}", self.0))
    }
}
