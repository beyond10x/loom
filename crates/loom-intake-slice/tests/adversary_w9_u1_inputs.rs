//! Adversary, wave 2026-10-13-w4, U1 (story `engineering-protocols-070-pin`): the case inputs, the
//! two write actions, `not_applicable` obligations and the slice's frontier filter, attacked from
//! outside the unit's own tests.
//!
//! The workspace fixture is a git repository under `CARGO_TARGET_TMPDIR` whose test command is
//! `grep -qx fixed check.txt`; `check.txt` holds `broken`, so it is the implementation the failing
//! test reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, ExecutorOutcomeProposedAction, GovernorError, ProposedActionArguments,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_slice::case::{self, CaseInputs as RunInputs, ChangeKind, Risk};
use b10x_loom_intake_slice::executor::{LocalExecutor, Report, TestCommand, UnconfinedRunner};
use b10x_loom_intake_slice::run::SliceReads;
use b10x_loom_intake_slice::verifier::TestResultVerifier;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationInput, EvaluationRequest, ProtocolName,
};
use loom_governor::{
    CanonGovernor, CaseInputs, FallibleCaseStore, FileCaseStore, MemoryCaseStore, OpenError,
    evaluate,
};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

const SOFTWARE: &str = "software-change@1";
const INTENT: &str = "Make the failing check pass.";
const PRODUCER: &str = "adversary-w9-u1-verifier";

/// A bugfix may not edit its implementation before a test failed on the current revision. The
/// slice performs `tests.write` exactly as `implementation.edit` (any path, committed, reported as
/// the implementation's revision), and `software.change/1` gives `tests.write` no precondition. So
/// a bugfix whose first write is a `tests.write` of the implementation file the failing test reads
/// changes the implementation before any test failed: the red-first gate is bypassed by naming the
/// other write action. The action is admissible on the frontier the slice reads, so Commission
/// admits it and the slice performs it.
#[test]
fn a_bugfix_cannot_change_its_implementation_through_tests_write_before_a_red_test() {
    let fixture = Fixture::new("bypass");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, SOFTWARE, INTENT, fixture.workspace(), &bugfix())
        .expect("a bugfix case opens");
    let frontier = SliceReads(&governor)
        .frontier(&case)
        .expect("frontier")
        .into_data();
    let status = |action: &str| {
        frontier
            .actions
            .iter()
            .find(|entry| entry.action == action)
            .unwrap_or_else(|| panic!("{action} is on the frontier"))
            .status
    };
    assert_eq!(
        status("implementation.edit"),
        ActionStatus::Blocked,
        "the bugfix may not edit the implementation yet"
    );
    assert_eq!(
        status("tests.write"),
        ActionStatus::Admissible,
        "tests.write is offered and admitted"
    );

    let executor = fixture.executor(&governor, &case);
    let before = fixture.head();
    let written = executor.execute(&proposal(
        "tests.write",
        &json!({"files": [{"path": "check.txt", "contents": "fixed\n"}], "message": "a test"}),
    ));
    assert!(
        written.is_err() && fixture.head() == before && fixture.read("check.txt") == "broken\n",
        "a bugfix's implementation (check.txt, the file the failing test reads) is not changed \
         through tests.write before any test failed; got {written:?}, HEAD moved: {}",
        fixture.head() != before
    );
}

/// The revision bound: a red `test_result` at R1 admits a bugfix's `implementation.edit` at R1,
/// and a `tests.write` that commits moves the implementation to R2, where the R1 result no longer
/// counts: the edit is blocked again until a test fails at R2. Then it is admissible.
#[test]
fn a_committed_tests_write_does_not_carry_an_earlier_red_result_forward() {
    let fixture = Fixture::new("bound");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, SOFTWARE, INTENT, fixture.workspace(), &bugfix())
        .expect("a bugfix case opens");
    let executor = fixture.executor(&governor, &case);
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);
    let edit = |governor: &CanonGovernor<MemoryCaseStore>| {
        governor
            .frontier(&case)
            .expect("frontier")
            .into_data()
            .actions
            .into_iter()
            .find(|entry| entry.action == "implementation.edit")
            .expect("implementation.edit")
            .status
    };
    assert_eq!(edit(&governor), ActionStatus::Blocked, "no red test yet");

    let red = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run");
    verifier.verify(&red).expect("verified").expect("a result");
    assert_eq!(edit(&governor), ActionStatus::Admissible, "red at R1");

    let revision = governor.current_revision(&case).expect("revision");
    let written = executor
        .execute(&proposal(
            "tests.write",
            &json!({"files": [{"path": "tests/regression.txt", "contents": "fixed\n"}]}),
        ))
        .expect("tests.write is performed");
    let Report::Edited { revision: head, .. } = written else {
        panic!("tests.write reports the new HEAD: {written:?}");
    };
    assert_eq!(head, fixture.head());
    assert_eq!(
        governor.revisions(&case).expect("revisions")["implementation"],
        head,
        "tests.write moves the implementation revision"
    );
    assert!(governor.current_revision(&case).expect("revision") > revision);
    assert_eq!(
        edit(&governor),
        ActionStatus::Blocked,
        "the red result about R1 does not admit an edit of R2"
    );

    let red = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run");
    verifier.verify(&red).expect("verified").expect("a result");
    assert_eq!(edit(&governor), ActionStatus::Admissible, "red at R2");
}

/// The open check is exact: an input or value differing only in case, padding or emptiness is
/// refused naming it, and nothing is stored for the refused opening.
#[test]
fn an_input_spelled_otherwise_is_refused_and_nothing_is_stored() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    // The defaults with `input` given `value`: the input's default value replaced in the list
    // the inputs are built from, or one more input, so each input is given once (a second value
    // for one input is refused where the inputs are built).
    let given = |input: &str, value: &str| {
        let mut pairs: Vec<(String, String)> = [
            ("risk", "standard"),
            ("change_kind", "feature"),
            ("affects_security_boundary", "false"),
        ]
        .into_iter()
        .map(|(name, default)| (name.to_owned(), default.to_owned()))
        .collect();
        match pairs.iter_mut().find(|(name, _)| name == input) {
            Some(pair) => value.clone_into(&mut pair.1),
            None => pairs.push((input.to_owned(), value.to_owned())),
        }
        CaseInputs::try_from_pairs(pairs).expect("each input once")
    };
    let undeclared = |input: &str| OpenError::UndeclaredInput {
        input: input.to_owned(),
    };
    let unlisted = |input: &str, value: &str| OpenError::UnlistedInputValue {
        input: input.to_owned(),
        value: value.to_owned(),
    };
    let rows: Vec<(&str, CaseInputs, OpenError)> = vec![
        ("upper", given("Risk", "standard"), undeclared("Risk")),
        ("padded", given("risk ", "standard"), undeclared("risk ")),
        ("empty-name", given("", "standard"), undeclared("")),
        (
            "value-case",
            given("risk", "Standard"),
            unlisted("risk", "Standard"),
        ),
        (
            "value-empty",
            given("change_kind", ""),
            unlisted("change_kind", ""),
        ),
        (
            "bool-case",
            given("affects_security_boundary", "True"),
            unlisted("affects_security_boundary", "True"),
        ),
        (
            "bool-number",
            given("affects_security_boundary", "0"),
            unlisted("affects_security_boundary", "0"),
        ),
    ];
    for (id, inputs, expected) in rows {
        let case = CaseId(format!("refused-{id}"));
        assert_eq!(
            governor.open_case(case.clone(), SOFTWARE, revisions(), inputs),
            Err(expected),
            "{id}"
        );
        assert_eq!(
            governor.current_revision(&case),
            Err(GovernorError::UnknownCase),
            "{id}: nothing is stored"
        );
    }
}

/// `not_applicable` is not open, at every combination of the three inputs: on a fresh case nothing
/// is discharged, so the open obligations are exactly the applicable ones, in the frontier
/// (`FrontierObligation.open`) and in `evaluate` (`DecidedObligation.open`) alike.
#[test]
fn every_input_combination_opens_exactly_the_applicable_obligations() {
    let catalog = ProtocolCatalog::bundled().expect("bundled catalogue");
    for risk in ["trivial", "standard", "elevated", "critical"] {
        for kind in ["feature", "bugfix", "refactor"] {
            for boundary in ["true", "false"] {
                let label = format!("{risk}/{kind}/{boundary}");
                let rank = ["trivial", "standard", "elevated", "critical"]
                    .iter()
                    .position(|level| *level == risk)
                    .unwrap();
                let mut expected = vec!["tests.required"];
                if rank >= 1 {
                    expected.push("review.required");
                }
                if rank >= 2 {
                    expected.push("provenance.required");
                }
                if rank >= 3 {
                    expected.push("health.required");
                }
                if kind == "bugfix" {
                    expected.push("red_first.required");
                }
                if boundary == "true" {
                    expected.push("security_review.required");
                }
                expected.sort_unstable();

                let governor = CanonGovernor::new(MemoryCaseStore::default());
                let inputs = CaseInputs::try_from_pairs([
                    ("risk", risk),
                    ("change_kind", kind),
                    ("affects_security_boundary", boundary),
                ])
                .expect("distinct inputs");
                let case = governor
                    .open(SOFTWARE, revisions(), inputs)
                    .unwrap_or_else(|error| panic!("{label} opens: {error}"));
                let frontier = governor.frontier(&case).expect("frontier").into_data();
                let mut open: Vec<&str> = frontier
                    .obligations
                    .iter()
                    .filter(|obligation| obligation.open)
                    .map(|obligation| obligation.obligation.as_str())
                    .collect();
                open.sort_unstable();
                assert_eq!(open, expected, "{label}: the frontier's open obligations");
                assert_eq!(
                    frontier.obligations.len(),
                    6,
                    "{label}: every obligation listed"
                );

                let mut snapshot = software_snapshot();
                snapshot["inputs"] = json!({"risk": risk, "change_kind": kind, "affects_security_boundary": boundary});
                let decision = evaluate(&catalog, &request(&snapshot))
                    .unwrap_or_else(|refusal| panic!("{label} is decided: {refusal:?}"));
                let mut decided: Vec<&str> = decision
                    .obligations
                    .iter()
                    .filter(|obligation| obligation.open)
                    .map(|obligation| obligation.obligation.as_str())
                    .collect();
                decided.sort_unstable();
                assert_eq!(decided, expected, "{label}: evaluate's open obligations");
            }
        }
    }
}

/// `evaluate` on a software-change snapshot that gives no inputs, or leaves one out, is refused
/// naming the snapshot and the input, never decided with a default.
#[test]
fn evaluate_refuses_a_snapshot_without_its_inputs() {
    let catalog = ProtocolCatalog::bundled().expect("bundled catalogue");
    let mut none = software_snapshot();
    none.as_object_mut().unwrap().remove("inputs");
    let mut partial = software_snapshot();
    partial["inputs"] = json!({"risk": "standard", "change_kind": "bugfix"});
    for (label, snapshot, input) in [
        ("none", none, "affects_security_boundary"),
        ("partial", partial, "affects_security_boundary"),
    ] {
        let refusal = evaluate(&catalog, &request(&snapshot))
            .expect_err("a snapshot without its inputs is refused");
        assert_eq!(refusal.input, EvaluationInput::Snapshot, "{label}");
        assert!(
            refusal.code.contains("missing-input") && refusal.message.contains(input),
            "{label}: refused naming the input: {refusal:?}"
        );
    }
}

/// A stored case whose `inputs` member is present but does not give the protocol's inputs (empty,
/// an unknown extra one, an unlisted value) is never decided: frontier and completion fail toward
/// less authority. And the acceptance's "a stored case without them is refused" holds for the
/// empty list as for the missing member: the governor does not report such a case's inputs as if
/// it had opened with none.
#[test]
fn a_stored_case_with_unusable_inputs_is_never_decided() {
    let rows = [
        ("empty", json!([])),
        (
            "extra",
            json!([
                {"input": "affects_security_boundary", "value": "false"},
                {"input": "change_kind", "value": "bugfix"},
                {"input": "priority", "value": "high"},
                {"input": "risk", "value": "standard"},
            ]),
        ),
        (
            "unlisted",
            json!([
                {"input": "affects_security_boundary", "value": "false"},
                {"input": "change_kind", "value": "hotfix"},
                {"input": "risk", "value": "standard"},
            ]),
        ),
    ];
    let mut failures = Vec::new();
    for (label, stored) in rows {
        let dir = scratch(&format!("store-{label}"));
        let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store opens"));
        let case = governor
            .open(SOFTWARE, revisions(), bugfix_governed())
            .expect("the case opens");
        let path = case_file(&dir);
        let mut document: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["inputs"] = stored;
        std::fs::write(&path, document.to_string()).unwrap();

        let governor = CanonGovernor::new(FileCaseStore::open(&dir).expect("the store reopens"));
        assert!(
            governor.frontier(&case).is_err(),
            "{label}: a frontier is never issued"
        );
        assert!(
            governor.completion(&case).is_err(),
            "{label}: no completion is determined"
        );
        if label == "empty" {
            let store = FileCaseStore::open(&dir).expect("the store reopens");
            let read = store.get(&case);
            let inputs = governor.inputs(&case);
            if read.as_ref().is_ok_and(Option::is_some) || inputs.is_ok() {
                failures.push(format!(
                    "{label}: the case reads (store: {}), and the governor reports its inputs as {:?}",
                    if read.is_ok() { "read" } else { "refused" },
                    inputs.map(|inputs| inputs.iter().map(|(i, v)| format!("{i}={v}")).collect::<Vec<_>>())
                ));
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(
        failures.is_empty(),
        "a software-change case stored without its inputs is refused: {failures:#?}"
    );
}

/// `SliceReads` is public for an embedder's own loop. On a case of a protocol other than
/// `software-change@1` it must not hide an action that awaits approval: an incident's
/// `release.rollback` and `traffic.shift` need a capability, and a host must see them.
#[test]
fn slice_reads_hides_no_approval_on_another_protocol() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(
            "incident-response@1",
            BTreeMap::from([
                ("service".to_owned(), "s1".to_owned()),
                ("release".to_owned(), "r1".to_owned()),
            ]),
            CaseInputs::none(),
        )
        .expect("an incident opens");
    let names = |frontier: &[b10x_loom_commission::model::responsibility::FrontierAction]| {
        frontier
            .iter()
            .map(|entry| entry.action.clone())
            .collect::<Vec<_>>()
    };
    let held = governor.frontier(&case).expect("frontier").into_data();
    assert!(
        names(&held.actions).contains(&"release.rollback".to_owned()),
        "the governor lists the incident's rollback"
    );
    let read = SliceReads(&governor)
        .frontier(&case)
        .expect("frontier")
        .into_data();
    assert_eq!(
        names(&read.actions),
        names(&held.actions),
        "SliceReads hides no action of an incident"
    );
}

fn bugfix() -> RunInputs {
    RunInputs {
        risk: Risk::Standard,
        change_kind: ChangeKind::Bugfix,
        affects_security_boundary: false,
    }
}

fn bugfix_governed() -> CaseInputs {
    CaseInputs::try_from_pairs([
        ("risk", "standard"),
        ("change_kind", "bugfix"),
        ("affects_security_boundary", "false"),
    ])
    .expect("distinct inputs")
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

fn software_snapshot() -> Value {
    let artifacts: serde_json::Map<String, Value> = revisions()
        .into_iter()
        .map(|(artifact, revision)| (artifact, json!({"revision": revision})))
        .collect();
    json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "software.change",
        "artifacts": artifacts,
        "revision": "r1",
        "inputs": {"risk": "standard", "change_kind": "feature", "affects_security_boundary": "false"},
    })
}

fn request(snapshot: &Value) -> EvaluationRequest {
    EvaluationRequest {
        protocol: ProtocolName(SOFTWARE.into()),
        snapshot: CaseSnapshot(loom_governor::model::json::parse(&snapshot.to_string()).unwrap()),
        evidence: Vec::new(),
        at: None,
        decisions: Vec::new(),
    }
}

fn proposal(action: &str, arguments: &Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(cjson::parse(&arguments.to_string()).unwrap()),
    }
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary-w9-u1-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn case_file(dir: &Path) -> PathBuf {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("case-"))
        })
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    files[0].clone()
}

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = scratch(name);
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let fixture = Self { root, workspace };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").unwrap();
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "a failing check"]);
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn executor<'g>(
        &self,
        governor: &'g CanonGovernor<MemoryCaseStore>,
        case: &CaseId,
    ) -> LocalExecutor<'g, MemoryCaseStore> {
        LocalExecutor::new(
            governor,
            case.clone(),
            &self.workspace,
            TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        )
        .with_runner(Arc::new(UnconfinedRunner))
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.workspace.join(file)).unwrap()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
