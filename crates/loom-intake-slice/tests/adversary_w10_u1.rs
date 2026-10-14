//! Adversary, wave 2026-10-14-w1, U1 (story `change-evidence-order`): the slice's evidence order
//! and the governor's trusted producer, attacked from outside the unit's own tests.
//!
//! The workspace fixture is a git repository under `CARGO_TARGET_TMPDIR` whose test command is
//! `grep -qx fixed check.txt` (no shell); `check.txt` holds `broken`, so the suite is red until
//! the implementation is fixed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    EvidenceData, EvidenceId, ExecutorOutcomeProposedAction, FrontierData, ObservationId,
    ProposedActionArguments, Truth,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_intake_slice::case::{self, CaseInputs as RunInputs, ChangeKind, Risk};
use b10x_loom_intake_slice::clock::Clock;
use b10x_loom_intake_slice::executor::{LocalExecutor, TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::verifier::TestResultVerifier;
use chrono::{DateTime, FixedOffset, TimeDelta};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};
use serde_json::json;

const SOFTWARE: &str = "software-change@1";
const INTENT: &str = "Make the failing check pass.";
const PRODUCER: &str = "intake-slice/verifier";

/// `loom-governor`'s contract, as this unit rewrote it: "The record's `producer` is the trusted one
/// `submit_evidence` attributed the evidence to, never one its facts name: a producer in the facts
/// is replaced". A replaced member is never read, so what the facts' `producer` holds cannot
/// decide whether the record applies. It does: `canon_record` hands the whole facts to Canon's
/// `evidence_from_value` first and replaces the producer only afterwards, so a facts `producer`
/// that Canon cannot read as a principal (an explicit null, a number, an object) refuses the
/// record and `tests.pass` never holds. The control, the same record with no `producer` member,
/// applies.
#[test]
fn adversary_a_facts_producer_canon_cannot_read_never_decides_whether_the_record_applies() {
    let control = submitted_with_facts_producer(None);
    assert_eq!(
        control,
        Truth::True,
        "the control record, with no producer in its facts, applies"
    );
    for (label, producer) in [
        ("null", json!(null)),
        ("number", json!(7)),
        ("object", json!({"name": "a model"})),
    ] {
        assert_eq!(
            submitted_with_facts_producer(Some(producer)),
            Truth::True,
            "a facts producer that is {label} is replaced by the trusted producer and never \
             read, so the record applies as the control does"
        );
    }
}

/// Opens a software change, submits one passing `test_result` under the trusted producer whose
/// facts carry `producer` as given (none when `None`), and returns `tests.pass`.
fn submitted_with_facts_producer(producer: Option<serde_json::Value>) -> Truth {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(
            SOFTWARE,
            revisions(),
            CaseInputs::try_from_pairs([
                ("risk", "trivial"),
                ("change_kind", "feature"),
                ("affects_security_boundary", "false"),
            ])
            .expect("inputs"),
        )
        .expect("a software change opens");
    let mut facts = json!({
        "format": "canon-evidence/1",
        "id": "result-1",
        "kind": "test_result",
        "subject": "implementation",
        "subject_revision": "r0",
        "result": "pass",
        "observed_at": "2026-10-03T13:20:00Z",
    });
    if let Some(producer) = producer {
        facts["producer"] = producer;
    }
    submit_evidence(
        &governor,
        PRODUCER,
        EvidenceData {
            evidence_id: EvidenceId(Uuid("00000000-0000-8000-8000-000000000001".into())),
            case_id: case.clone(),
            kind: "test_result".into(),
            subject_revision: 1,
            producer: String::new(),
            observation_ids: vec![ObservationId(Uuid(
                "00000000-0000-8000-8000-000000000002".into(),
            ))],
            facts: cjson::parse(&facts.to_string()).expect("facts"),
            provenance: cjson::Value::Null,
        },
    )
    .expect("submitted");
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    claim(&frontier, "tests.pass")
}

/// `red_first.required` asks whether the implementation changed before the regression test first
/// failed. The slice performs both and knows their order; it records each only as the wall-clock
/// second it read, and Canon orders by that alone. A bugfix that changes its implementation
/// first (an `implementation.edit` admitted by the suite's red run), and only then writes its
/// regression test and runs it red, must leave `red_first.required` open. A host clock that steps
/// back between the edit and the red run (an NTP correction, a resumed virtual machine) reorders
/// the two: the red run is recorded before the edit and `red_first.required` is discharged for a
/// bugfix whose implementation changed before any regression test existed.
#[test]
fn adversary_an_edit_before_the_regression_test_is_never_shown_first_by_a_clock_step_back() {
    let fixture = Fixture::new("clock-back");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, SOFTWARE, INTENT, fixture.workspace(), &bugfix())
        .expect("a bugfix case opens");
    // Readings, in the order the executor reads them: the first run, the edit, the second run.
    // The third steps back 50 seconds behind the second.
    let clock = Scripted::new(&[100, 200, 150]);
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        fixture.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    )
    .with_runner(Arc::new(UnconfinedRunner))
    .with_clock(&clock);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);
    let step = |action: &str, arguments: serde_json::Value| {
        let report = executor
            .execute(&proposal(action, &arguments))
            .unwrap_or_else(|error| panic!("{action} is performed: {error}"));
        verifier
            .verify(&report)
            .unwrap_or_else(|error| panic!("{action} is verified: {error}"));
    };

    // The suite fails on the current revision: the edit is admitted.
    step("tests.run", json!({}));
    // The implementation changes first, before any regression test was written.
    step(
        "implementation.edit",
        json!({"files": [{"path": "notes.txt", "contents": "changed first\n"}], "message": "a change"}),
    );
    // Only now is the regression test written and run red.
    step(
        "tests.write",
        json!({"files": [{"path": "tests/regression.txt", "contents": "check.txt reads fixed\n"}], "message": "the regression test"}),
    );
    step("tests.run", json!({}));

    let kinds: Vec<String> = governor
        .evidence(&case)
        .expect("evidence")
        .into_iter()
        .map(|record| record.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            "test_result",
            "implementation_change",
            "test_result",
            "regression_test_result"
        ],
        "the edit's change precedes the regression test's first result in the slice's own order"
    );
    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    assert!(
        obligation_open(&frontier, "red_first.required"),
        "the implementation changed before the regression test first failed, so \
         red_first.required stays open whatever instant the host clock read; \
         regression.shown_first is {:?}",
        claim(&frontier, "regression.shown_first")
    );
}

/// A host clock that reads the given offsets, in seconds after 2026-10-03T13:20:00Z, in order,
/// and fails once they are used up.
struct Scripted {
    readings: Mutex<Vec<i64>>,
}

impl Scripted {
    fn new(seconds: &[i64]) -> Self {
        let mut readings = seconds.to_vec();
        readings.reverse();
        Self {
            readings: Mutex::new(readings),
        }
    }
}

impl Clock for Scripted {
    fn read(&self) -> Result<DateTime<FixedOffset>, String> {
        let offset = self
            .readings
            .lock()
            .expect("readings")
            .pop()
            .ok_or_else(|| "the scripted clock has no reading left".to_owned())?;
        let start = DateTime::parse_from_rfc3339("2026-10-03T13:20:00Z").expect("an instant");
        Ok(start + TimeDelta::seconds(offset))
    }
}

fn bugfix() -> RunInputs {
    RunInputs {
        risk: Risk::Standard,
        change_kind: ChangeKind::Bugfix,
        affects_security_boundary: false,
    }
}

fn revisions() -> BTreeMap<String, String> {
    [
        "intent",
        "system_specification",
        "plan",
        "implementation",
        "release",
        "deployment",
    ]
    .into_iter()
    .map(|artifact| (artifact.to_owned(), "r0".to_owned()))
    .collect()
}

fn claim(frontier: &FrontierData, claim: &str) -> Truth {
    frontier
        .claims
        .iter()
        .find(|entry| entry.claim == claim)
        .unwrap_or_else(|| panic!("the frontier lists `{claim}`"))
        .value
}

fn obligation_open(frontier: &FrontierData, obligation: &str) -> bool {
    frontier
        .obligations
        .iter()
        .find(|entry| entry.obligation == obligation)
        .unwrap_or_else(|| panic!("the frontier lists `{obligation}`"))
        .open
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(cjson::parse(&arguments.to_string()).expect("JSON")),
    }
}

/// A git repository with a local identity and one failing check, under `CARGO_TARGET_TMPDIR`.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary-w10-u1-{name}-{}-{nanos}",
            std::process::id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        let fixture = Self { root, workspace };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").expect("write check.txt");
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "a failing check"]);
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
