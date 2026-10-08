//! Adversary cases for [`FileCaseStore`] (story `governor-restart-durability`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, GovernorError, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{EvidenceError, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{
    CanonGovernor, CaseState, FallibleCaseStore, FileCaseStore, HeldEvidence, MemoryCaseStore,
};
use serde_json::Value;

const PROTOCOL: &str = "software-change@1";
const ROLE: &str = "LOOM_ADVERSARY_STALE_TEMP_ROLE";
const DIR: &str = "LOOM_ADVERSARY_STALE_TEMP_DIR";
const STALE_TEST: &str =
    "a_temporary_file_left_by_an_earlier_life_with_the_same_pid_does_not_fail_a_write";

/// A case id `is_identifier` accepts and the in-memory store holds is one the file store holds too.
/// The file name is `case-` + two hex digits per id byte + `.json`, so an id past 122 bytes (41
/// three-byte characters) names a file longer than the 255 bytes a Linux file name may be.
#[test]
fn a_case_id_the_governor_accepts_is_one_the_file_store_can_hold() {
    let long_ascii = "c".repeat(123);
    let cjk = "案".repeat(41);
    for id in [long_ascii, cjk] {
        let case = CaseId(id.clone());
        let memory = CanonGovernor::new(MemoryCaseStore::default());
        assert_eq!(
            memory.open_case(case.clone(), PROTOCOL, fixture_revisions()),
            Ok(()),
            "the in-memory governor takes the id ({} bytes)",
            id.len()
        );
        let dir = store_dir(&format!("long-id-{}", id.len()));
        let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
        assert_eq!(
            governor.open_case(case.clone(), PROTOCOL, fixture_revisions()),
            Ok(()),
            "the file-backed governor takes the same id ({} bytes)",
            id.len()
        );
        assert_eq!(governor.current_revision(&case), Ok(1));
    }
}

/// A case file carrying a member `loom.governor.StoredCase` does not declare is not a document this
/// store wrote; reading it as if the member were not there gives a silently different case.
#[test]
fn a_case_file_with_an_undeclared_member_is_refused() {
    let dir = store_dir("foreign-member");
    let store = FileCaseStore::open(&dir).expect("the store opens");
    let case = CaseId("case-1".to_owned());
    assert_eq!(
        store.insert(plain_state(&case)).map_err(|e| e.to_string()),
        Ok(true)
    );
    let file = only_case_file(&dir);
    let text = std::fs::read_to_string(&file).unwrap();
    let foreign = text.replacen('{', r#"{"format":"loom.governor/2","closed":true,"#, 1);
    std::fs::write(&file, foreign).unwrap();

    let read = FileCaseStore::open(&dir).unwrap().get(&case);
    assert!(
        read.is_err(),
        "a case file from another format version is an error, not a case: {read:?}"
    );
}

/// A case file naming `revision` twice is not a document this store wrote; reading the first and
/// ignoring the second is a silently different case.
#[test]
fn a_case_file_naming_a_member_twice_is_refused() {
    let dir = store_dir("duplicate-member");
    let store = FileCaseStore::open(&dir).expect("the store opens");
    let case = CaseId("case-1".to_owned());
    assert_eq!(
        store.insert(plain_state(&case)).map_err(|e| e.to_string()),
        Ok(true)
    );
    let file = only_case_file(&dir);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.ends_with('}'));
    let doubled = format!("{},\"revision\":7}}", &text[..text.len() - 1]);
    std::fs::write(&file, doubled).unwrap();

    let read = FileCaseStore::open(&dir).unwrap().get(&case);
    assert!(
        read.is_err(),
        "a case file naming `revision` twice is an error, not revision 1: {read:?}"
    );
}

/// A process killed between writing its temporary file and renaming it leaves `.tmp-<pid>-<n>`.
/// A later process with the same pid (a container's PID 1, say) starts its counter at 0 again, and
/// `create_new` refuses the name: the write fails although nothing is wrong with the store.
#[test]
fn a_temporary_file_left_by_an_earlier_life_with_the_same_pid_does_not_fail_a_write() {
    if let Ok(role) = std::env::var(ROLE) {
        assert_eq!(role, "child");
        let dir = PathBuf::from(std::env::var(DIR).expect("the parent names the store directory"));
        // The earlier life, same pid, died after writing its first temporary file.
        std::fs::write(
            dir.join(format!(".tmp-{}-0", std::process::id())),
            "{\"id\":\"half",
        )
        .unwrap();
        let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
        let opened = governor.open(PROTOCOL, fixture_revisions());
        assert!(
            opened.is_ok(),
            "the first write after a restart succeeds: {opened:?}"
        );
        return;
    }
    let dir = store_dir("stale-temp");
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([STALE_TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, "child")
        .env(DIR, &dir)
        .output()
        .expect("the child starts");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 passed") || stdout.contains("1 failed"),
        "the child ran: {stdout}"
    );
    assert!(
        output.status.success(),
        "the child failed: {}\n{}\n{}",
        output.status,
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A known divergence, held as one: the generated JSON reader refuses a document nested past 64
/// levels, and evidence facts sit inside a case document, so facts nested past 61 levels (the
/// boundary this test measures) are refused by the file store (the governor answers
/// `GovernorUnavailable` and the case is left as it was) while the in-memory store keeps them.
/// Facts at 61 levels are kept by both.
#[test]
fn facts_nested_past_sixty_one_levels_are_refused_by_the_file_store_alone() {
    let dir = store_dir("deep-facts");
    let memory = CanonGovernor::new(MemoryCaseStore::default());
    let file = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
    let case = CaseId("case-deep".to_owned());
    memory
        .open_case(case.clone(), PROTOCOL, fixture_revisions())
        .unwrap();
    file.open_case(case.clone(), PROTOCOL, fixture_revisions())
        .unwrap();
    let evidence = |n: u64, depth: usize| EvidenceData {
        evidence_id: EvidenceId(uuid(n)),
        case_id: case.clone(),
        kind: "note".to_owned(),
        subject_revision: 1,
        producer: String::new(),
        observation_ids: vec![ObservationId(uuid(n + 100))],
        facts: json::parse(&format!("{}{}", "[".repeat(depth), "]".repeat(depth)))
            .expect("Commission's reader takes the facts"),
        provenance: json::Value::Null,
    };

    for depth in [1, 61] {
        let record = evidence(depth as u64, depth);
        assert!(submit_evidence(&memory, "service:ci", record.clone()).is_ok());
        assert_eq!(
            submit_evidence(&file, "service:ci", record).map_err(|e| format!("{e:?}")),
            Ok(()),
            "facts {depth} levels deep fit the case document's 64"
        );
    }
    let kept = file.evidence(&case).expect("evidence");

    for depth in [62, 63] {
        let record = evidence(depth as u64, depth);
        let in_memory = submit_evidence(&memory, "service:ci", record.clone());
        assert!(
            in_memory.is_ok(),
            "the memory store keeps it: {in_memory:?}"
        );
        let on_file = submit_evidence(&file, "service:ci", record);
        assert_eq!(
            on_file,
            Err(EvidenceError::Governor(GovernorError::GovernorUnavailable)),
            "known divergence: facts {depth} levels deep put the case document past the JSON \
             reader's 64-level limit, so the file store refuses the write where the memory store \
             keeps the record"
        );
        assert_eq!(
            file.evidence(&case),
            Ok(kept.clone()),
            "the refused write leaves the case as it was"
        );
    }
}

/// Every awkward value round-trips exactly through a fresh store over the same directory.
#[test]
fn an_awkward_case_and_its_observations_round_trip_exactly() {
    let dir = store_dir("awkward");
    let ids = [
        "../..",
        "/etc/passwd",
        "a\\b",
        "\u{0}\u{1f}\u{7f}\u{2028}\"",
        "😀ü",
        "case-1",
    ];
    let facts = json::parse(
        r#"{"z":{},"a":[],"n":[0,-0,1E+2,-0.0e-0,123456789012345678901234567890,1.50,007],
            "t":"\u0000\u001f😀\/\b\f","":null,"dup":1,"dup":2}"#,
    )
    .expect("the reader takes it");
    let mut states = Vec::new();
    {
        let store = FileCaseStore::open(&dir).expect("the store opens");
        for (index, id) in ids.iter().enumerate() {
            let case = CaseId((*id).to_owned());
            let state = CaseState {
                id: case.clone(),
                protocol: format!("p\"{id}@1"),
                revision: if index % 2 == 0 { i64::MAX } else { i64::MIN },
                artifacts: BTreeMap::from([
                    ("\u{0}".to_owned(), "\u{10ffff}".to_owned()),
                    ("b".to_owned(), String::new()),
                ]),
                evidence: (0..3)
                    .map(|n| HeldEvidence {
                        data: EvidenceData {
                            evidence_id: EvidenceId(uuid(10 - n)),
                            case_id: case.clone(),
                            kind: format!("k{n}"),
                            subject_revision: i64::MIN + n as i64,
                            producer: String::new(),
                            observation_ids: Vec::new(),
                            facts: if n == 1 {
                                facts.clone()
                            } else {
                                json::Value::Array(Vec::new())
                            },
                            provenance: json::Value::Object(Vec::new()),
                        },
                        applies: n % 2 == 0,
                    })
                    .collect(),
            };
            assert_eq!(
                store.insert(state.clone()).map_err(|e| e.to_string()),
                Ok(true)
            );
            states.push(state);
        }
        for n in 0..3 {
            store
                .observe(observation(n, facts.clone()))
                .map_err(|e| e.to_string())
                .unwrap();
        }
    }
    let reopened = FileCaseStore::open(&dir).expect("the store reopens");
    for state in &states {
        assert_eq!(
            reopened.get(&state.id).map_err(|e| e.to_string()),
            Ok(Some(state.clone()))
        );
    }
    reopened.observe(observation(3, json::Value::Null)).unwrap();
    let expected: Vec<ObservationData> = (0..3)
        .map(|n| observation(n, facts.clone()))
        .chain([observation(3, json::Value::Null)])
        .collect();
    assert_eq!(
        FileCaseStore::open(&dir)
            .unwrap()
            .observations()
            .map_err(|e| e.to_string()),
        Ok(expected)
    );
}

/// Two stores over one directory, on two threads, lose no update and no observation.
#[test]
fn two_stores_over_one_directory_lose_nothing() {
    let dir = store_dir("two-stores");
    let case = CaseId("case-1".to_owned());
    FileCaseStore::open(&dir)
        .unwrap()
        .insert(plain_state(&case))
        .unwrap();
    std::thread::scope(|scope| {
        for thread in 0..2 {
            let dir = dir.clone();
            let case = case.clone();
            scope.spawn(move || {
                let store = FileCaseStore::open(&dir).unwrap();
                for n in 0..40 {
                    store
                        .update(&case, |state| state.revision += 1)
                        .unwrap()
                        .unwrap();
                    store
                        .observe(observation(thread * 100 + n, json::Value::Null))
                        .unwrap();
                }
            });
        }
    });
    let store = FileCaseStore::open(&dir).unwrap();
    assert_eq!(store.get(&case).unwrap().unwrap().revision, 81);
    assert_eq!(store.observations().unwrap().len(), 80);
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".tmp-"))
        .collect();
    assert_eq!(
        names,
        Vec::<String>::new(),
        "no temporary file is left behind"
    );
}

/// A corrupted observations file is an error on read and on observe, and is never replaced by a
/// list holding only the new observation.
#[test]
fn a_corrupted_observations_file_is_an_error_and_is_kept() {
    let dir = store_dir("corrupt-observations");
    let store = FileCaseStore::open(&dir).unwrap();
    store.observe(observation(1, json::Value::Null)).unwrap();
    let path = dir.join("observations.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let truncated = &text[..text.len() / 2];
    std::fs::write(&path, truncated).unwrap();
    assert!(store.observations().is_err());
    assert!(store.observe(observation(2, json::Value::Null)).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), truncated);
    std::fs::write(&path, "").unwrap();
    assert!(
        store.observations().is_err(),
        "an empty file is not an empty list"
    );
    std::fs::write(&path, [0xff, 0xfe, b'[', b']']).unwrap();
    assert!(
        store.observations().is_err(),
        "a file that is not UTF-8 is an error"
    );
}

fn observation(n: u64, payload: json::Value) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(uuid(1000 + n)),
        source: "ci\u{0}".to_owned(),
        subject: "😀".to_owned(),
        observed_at: Timestamp("2026-10-04T11:00:00Z".to_owned()),
        payload,
    }
}

fn plain_state(case: &CaseId) -> CaseState {
    CaseState {
        id: case.clone(),
        protocol: PROTOCOL.to_owned(),
        revision: 1,
        artifacts: BTreeMap::from([("intent".to_owned(), "i1".to_owned())]),
        evidence: Vec::new(),
    }
}

fn only_case_file(dir: &Path) -> PathBuf {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("case-"))
        })
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    files[0].clone()
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("restart-durability-adversary")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_revisions() -> BTreeMap<String, String> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    let fixture: Value = serde_yaml_ng::from_str(&std::fs::read_to_string(path).unwrap())
        .expect("the fixture reads");
    fixture["case"]["artifacts"]
        .as_object()
        .expect("case artifacts")
        .iter()
        .map(|(artifact, entry)| {
            (
                artifact.clone(),
                entry["revision"].as_str().expect("revision").to_owned(),
            )
        })
        .collect()
}
