//! A host declares, when it opens a case, the independence dimensions each evidence producer
//! satisfies; the governor puts them in the case's `canon-case/1` snapshot, so a claim whose
//! evidence match demands independence (`independent_on`) is decided on a declared producer's
//! records, and the declarations are held with the case, across a `FileCaseStore` restart.

use std::collections::BTreeMap;
use std::path::PathBuf;

use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    Truth,
};
use b10x_loom_commission::model::{
    json,
    primitives::{Timestamp, Uuid},
};
use b10x_loom_commission::ports::evidence::{EvidencePort, ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use loom_governor::{
    CanonGovernor, CaseIndependence, CaseInputs, FallibleCaseStore, FileCaseStore, MemoryCaseStore,
    OpenError,
};

const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: reviewed.delivery, revision: 1}
artifacts: {implementation: {}}
evidence_kinds:
  review: {}
independence_dimensions:
  different_agent_run: {description: the run that wrote the candidate is not the one reviewing it}
claims:
  reviewed:
    true_when:
      evidence: {kind: review, result: pass, subject: implementation, independent_on: [different_agent_run]}
actions:
  merge:
    effect: write
    precondition: {claim: reviewed}
outcomes:
  merged:
    requires: {claim: reviewed}
"#;

const NAME: &str = "reviewed-delivery@1";
const REVIEWER: &str = "reviewer-run-2";

fn governor<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store)
        .with_protocol_yaml(NAME, PROTOCOL)
        .unwrap()
}

fn revisions() -> BTreeMap<String, String> {
    [("implementation".into(), "r1".into())].into()
}

fn declared() -> CaseIndependence {
    CaseIndependence::none()
        .with(REVIEWER, ["different_agent_run"])
        .unwrap()
}

/// Submits one passing review of the implementation, attributed to `REVIEWER`.
fn passing_review<G: EvidencePort + ObservationPort>(governor: &G, case: &CaseId) {
    let observation = ObservationId(Uuid("00000000-0000-0000-0000-0000000000a1".into()));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:review".into(),
            subject: "implementation".into(),
            observed_at: Timestamp("2026-10-16T10:00:00Z".into()),
            payload: json::Value::Null,
        }),
    )
    .unwrap();
    let facts = json::parse(
        r#"{"format":"canon-evidence/1","id":"review-1","kind":"review","subject":"implementation","subject_revision":"r1","result":"pass"}"#,
    )
    .unwrap();
    submit_evidence(
        governor,
        REVIEWER,
        EvidenceData {
            evidence_id: EvidenceId(Uuid("00000000-0000-0000-0000-0000000000b1".into())),
            case_id: case.clone(),
            kind: "review".into(),
            subject_revision: 1,
            producer: String::new(),
            facts,
            provenance: json::Value::Null,
            observation_ids: vec![observation],
        },
    )
    .unwrap();
}

/// The merge action's status and the `reviewed` claim's value on the case's frontier.
fn merge_and_claim<G: Governor>(governor: &G, case: &CaseId) -> (ActionStatus, Truth) {
    let frontier = governor.frontier(case).unwrap().into_data();
    let merge = frontier
        .actions
        .iter()
        .find(|action| action.action == "merge")
        .unwrap()
        .status;
    let reviewed = frontier
        .claims
        .iter()
        .find(|claim| claim.claim == "reviewed")
        .unwrap()
        .value;
    (merge, reviewed)
}

#[test]
fn a_record_from_a_declared_independent_producer_satisfies_an_independent_on_claim() {
    let declaring = governor(MemoryCaseStore::default());
    let case = declaring
        .open_with_independence(NAME, revisions(), CaseInputs::none(), declared())
        .unwrap();
    passing_review(&declaring, &case);
    assert_eq!(
        merge_and_claim(&declaring, &case),
        (ActionStatus::Admissible, Truth::True)
    );
    assert_eq!(declaring.independence(&case).unwrap(), declared());

    let silent = governor(MemoryCaseStore::default());
    let case = silent.open(NAME, revisions(), CaseInputs::none()).unwrap();
    passing_review(&silent, &case);
    assert_eq!(
        merge_and_claim(&silent, &case),
        (ActionStatus::Blocked, Truth::Unknown)
    );
    assert_eq!(
        silent.independence(&case).unwrap(),
        CaseIndependence::none()
    );
}

#[test]
fn a_dimension_the_protocol_does_not_declare_is_refused_naming_it_and_nothing_is_stored() {
    let governor = governor(MemoryCaseStore::default());
    let undeclared = CaseIndependence::none()
        .with(REVIEWER, ["different_agent_run", "different_vendor"])
        .unwrap();
    assert_eq!(
        governor.open_case_with_independence(
            CaseId("case-1".into()),
            NAME,
            revisions(),
            CaseInputs::none(),
            undeclared,
        ),
        Err(OpenError::UndeclaredDimension {
            producer: REVIEWER.into(),
            dimension: "different_vendor".into(),
        })
    );
    assert_eq!(
        governor.protocol(&CaseId("case-1".into())),
        Err(b10x_loom_commission::model::responsibility::GovernorError::UnknownCase)
    );
}

#[test]
fn a_producer_that_is_not_an_identifier_is_refused_naming_it() {
    let governor = governor(MemoryCaseStore::default());
    let spaced = CaseIndependence::none()
        .with("reviewer run", ["different_agent_run"])
        .unwrap();
    assert_eq!(
        governor.open_with_independence(NAME, revisions(), CaseInputs::none(), spaced),
        Err(OpenError::InvalidProducer {
            producer: "reviewer run".into(),
        })
    );
}

#[test]
fn a_producer_or_a_dimension_declared_twice_is_refused_never_replaced() {
    assert_eq!(
        declared().with(REVIEWER, ["different_agent_run"]),
        Err(OpenError::RepeatedProducer {
            producer: REVIEWER.into(),
        })
    );
    assert_eq!(
        CaseIndependence::none().with(REVIEWER, ["different_agent_run", "different_agent_run"]),
        Err(OpenError::RepeatedDimension {
            producer: REVIEWER.into(),
            dimension: "different_agent_run".into(),
        })
    );
}

/// A fresh directory under the target directory, removed first if a previous run left it.
fn fresh_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_declarations_survive_a_file_case_store_reopened() {
    let dir = fresh_dir("independence-file-store");
    let case = {
        let first = governor(FileCaseStore::open(&dir).unwrap());
        first
            .open_with_independence(NAME, revisions(), CaseInputs::none(), declared())
            .unwrap()
    };
    let reopened = governor(FileCaseStore::open(&dir).unwrap());
    assert_eq!(reopened.independence(&case).unwrap(), declared());
    passing_review(&reopened, &case);
    assert_eq!(
        merge_and_claim(&reopened, &case),
        (ActionStatus::Admissible, Truth::True)
    );
}

/// The one case file in `dir`, as JSON, and its path.
fn case_document(dir: &std::path::Path) -> (PathBuf, serde_json::Value) {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("case-"))
        })
        .collect();
    assert_eq!(files.len(), 1, "one file per case: {files:?}");
    let text = std::fs::read_to_string(&files[0]).unwrap();
    (files[0].clone(), serde_json::from_str(&text).unwrap())
}

/// A case stored with no `independence`, as Loom 0.23.0 and earlier wrote every case, is read
/// with no producer declared: exactly how those versions evaluated it.
#[test]
fn a_case_stored_without_declarations_reads_as_declaring_none() {
    let dir = fresh_dir("independence-absent");
    let case = governor(FileCaseStore::open(&dir).unwrap())
        .open_with_independence(NAME, revisions(), CaseInputs::none(), declared())
        .unwrap();
    let (path, mut document) = case_document(&dir);
    assert_eq!(
        document["independence"],
        serde_json::json!([{"producer": REVIEWER, "dimensions": ["different_agent_run"]}]),
        "the store writes every declaration"
    );
    document.as_object_mut().unwrap().remove("independence");
    std::fs::write(&path, document.to_string()).unwrap();

    let reopened = governor(FileCaseStore::open(&dir).unwrap());
    assert_eq!(
        reopened.independence(&case).unwrap(),
        CaseIndependence::none()
    );
    passing_review(&reopened, &case);
    assert_eq!(
        merge_and_claim(&reopened, &case),
        (ActionStatus::Blocked, Truth::Unknown)
    );
}

/// A held case whose declarations name a dimension its protocol does not declare is never
/// decided and its declarations are never reported.
#[test]
fn a_stored_declaration_of_an_undeclared_dimension_is_never_decided() {
    let dir = fresh_dir("independence-undeclared");
    let case = governor(FileCaseStore::open(&dir).unwrap())
        .open_with_independence(NAME, revisions(), CaseInputs::none(), declared())
        .unwrap();
    let (path, mut document) = case_document(&dir);
    document["independence"] =
        serde_json::json!([{"producer": REVIEWER, "dimensions": ["different_vendor"]}]);
    std::fs::write(&path, document.to_string()).unwrap();

    let reopened = governor(FileCaseStore::open(&dir).unwrap());
    use b10x_loom_commission::model::responsibility::GovernorError;
    assert_eq!(
        reopened.independence(&case),
        Err(GovernorError::GovernorUnavailable)
    );
    assert!(matches!(
        reopened.frontier(&case),
        Err(GovernorError::GovernorUnavailable)
    ));
}
