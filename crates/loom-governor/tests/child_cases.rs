//! Acceptance for `story:child-case-runtime` in the governor: opening child cases through a
//! `case_open` action, the parent-child link, the one `case_outcome` record per child that the
//! parent holds (`open` from the opening, the child's outcome once it is terminated), the stored
//! termination, the tree of cases, and the opening-cycle check at registration.
//!
//! Fixture protocols under `tests/fixtures/child-cases/`: `parent/1` opens `child/1` cases about
//! `idea`, which `child/1` does not declare; `holder/1` opens `drafter/1` cases about `idea`, which
//! `drafter/1` declares and revises. No model, no network; a store directory per test under
//! `CARGO_TARGET_TMPDIR`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, CompletionDeterminationComplete, EvidenceData,
    EvidenceId, Observation, ObservationData, ObservationId, Truth,
};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{
    CHILD_OUTCOME_PRODUCER, CanonGovernor, CaseIndependence, CaseInputs, CaseState, ChildError,
    ChildOpening, FallibleCaseStore, FileCaseStore, OpenError, TerminateError,
};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};

const PARENT: &str = include_str!("fixtures/child-cases/parent-1.yaml");
const CHILD: &str = include_str!("fixtures/child-cases/child-1.yaml");
const HOLDER: &str = include_str!("fixtures/child-cases/holder-1.yaml");
const DRAFTER: &str = include_str!("fixtures/child-cases/drafter-1.yaml");
const TREE: &str = include_str!("fixtures/child-cases/tree-1.yaml");
const EPIC: &str = include_str!("fixtures/child-cases/epic-1.yaml");

const OPEN: &str = "child.open";
const T1: &str = "2026-11-21T12:00:00Z";
const T2: &str = "2026-11-21T13:00:00Z";

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "child-cases-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    for (name, yaml) in [
        ("parent@1", PARENT),
        ("child@1", CHILD),
        ("holder@1", HOLDER),
        ("drafter@1", DRAFTER),
        ("tree@1", TREE),
        ("epic@1", EPIC),
    ] {
        catalog
            .add_yaml(name, yaml, source())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    catalog
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A fresh store directory.
fn dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "child-cases-{}-{}",
        std::process::id(),
        SCRATCH.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn governor_over<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store)
        .with_catalog(&catalog())
        .expect("the fixture catalog is admitted")
}

fn governor() -> (CanonGovernor<FileCaseStore>, PathBuf) {
    let dir = dir();
    (
        governor_over(FileCaseStore::open(&dir).expect("the store opens")),
        dir,
    )
}

fn root<S: FallibleCaseStore>(governor: &CanonGovernor<S>, protocol: &str) -> CaseId {
    let case = CaseId("case-1".into());
    governor
        .open_case(
            case.clone(),
            protocol,
            BTreeMap::from([("idea".to_owned(), "idea-1".to_owned())]),
            CaseInputs::none(),
        )
        .expect("the root opens");
    case
}

fn opening(name: &str) -> ChildOpening {
    ChildOpening {
        name: name.to_owned(),
        protocol: "child@1".to_owned(),
        independence: CaseIndependence::none(),
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

/// The `child_opened` record the opening action's `from: effect` entry gives.
fn opened_record<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId) -> EvidenceData {
    opened_about(governor, case, "idea")
}

/// The `child_opened` record of an opening action about `subject`.
fn opened_about<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    subject: &str,
) -> EvidenceData {
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let revision = governor.revisions(case).unwrap()[subject].clone();
    EvidenceData {
        evidence_id: EvidenceId(uuid(n)),
        case_id: case.clone(),
        kind: "child_opened".into(),
        subject_revision: governor.current_revision(case).unwrap(),
        producer: "loom".into(),
        observation_ids: Vec::new(),
        facts: commission_json::parse(
            &json!({
                "format": "canon-evidence/1",
                "id": format!("opened-{n}"),
                "kind": "child_opened",
                "subject": subject,
                "subject_revision": revision,
                "result": "opened",
                "observed_at": T1,
            })
            .to_string(),
        )
        .unwrap(),
        provenance: commission_json::Value::Null,
    }
}

/// Opens one child per name through `child.open`, with its `child_opened` record.
fn open_children<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    parent: &CaseId,
    names: &[&str],
) -> Vec<CaseId> {
    let openings: Vec<ChildOpening> = names.iter().map(|name| opening(name)).collect();
    governor
        .open_children(
            parent,
            OPEN,
            governor.current_revision(parent).unwrap(),
            &openings,
            vec![opened_record(governor, parent)],
            T1,
        )
        .expect("the children open")
}

fn plain(value: &commission_json::Value) -> Value {
    match value {
        commission_json::Value::Null => Value::Null,
        commission_json::Value::Bool(value) => Value::Bool(*value),
        commission_json::Value::Number(spelling) => serde_json::from_str(spelling).unwrap(),
        commission_json::Value::Text(text) => Value::String(text.clone()),
        commission_json::Value::Array(items) => Value::Array(items.iter().map(plain).collect()),
        commission_json::Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// The facts of every `case_outcome` record the case holds, in the order held.
fn outcomes<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId) -> Vec<Value> {
    governor
        .evidence(case)
        .unwrap()
        .iter()
        .filter(|record| record.kind == "case_outcome")
        .map(|record| plain(&record.facts))
        .collect()
}

fn claim<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId, claim: &str) -> Truth {
    governor
        .frontier(case)
        .unwrap()
        .data()
        .claims
        .iter()
        .find(|entry| entry.claim == claim)
        .unwrap_or_else(|| panic!("the frontier lists {claim}"))
        .value
}

/// Records a verdict of `result` about the child's `work`.
fn verdict<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId, result: &str) {
    record(governor, case, "verdict", "work", result, "worker");
}

/// An observation the governor received, for a submitted record to cite.
fn observed<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> ObservationId {
    let observation = ObservationId(uuid(0x1000_0000 + ISSUED.fetch_add(1, Ordering::Relaxed)));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:child-cases".into(),
            subject: "work".into(),
            observed_at: Timestamp(T1.into()),
            payload: commission_json::Value::Null,
        }),
    )
    .expect("the observation is received");
    observation
}

fn record<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    kind: &str,
    subject: &str,
    result: &str,
    producer: &str,
) {
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let revision = governor.revisions(case).unwrap()[subject].clone();
    submit_evidence(
        governor,
        producer,
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: kind.into(),
            subject_revision: governor.current_revision(case).unwrap(),
            producer: producer.into(),
            observation_ids: vec![observed(governor)],
            facts: commission_json::parse(
                &json!({
                    "format": "canon-evidence/1",
                    "id": format!("record-{n}"),
                    "kind": kind,
                    "subject": subject,
                    "subject_revision": revision,
                    "result": result,
                })
                .to_string(),
            )
            .unwrap(),
            provenance: commission_json::Value::Null,
        },
    )
    .expect("the record is received");
}

// ---- a store that counts writes and fails the ones it is told to ---------------------------------

#[derive(Debug)]
struct Faulty<S> {
    inner: S,
    /// The case ids written by `update`, in order.
    updates: Mutex<Vec<String>>,
    /// The case whose next update fails.
    fail_update_of: Mutex<Option<String>>,
}

impl<S> Faulty<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            updates: Mutex::new(Vec::new()),
            fail_update_of: Mutex::new(None),
        }
    }

    fn fail_next_update_of(&self, case: &CaseId) {
        *self.fail_update_of.lock().unwrap() = Some(case.0.clone());
    }

    fn updates_of(&self, case: &CaseId) -> usize {
        self.updates
            .lock()
            .unwrap()
            .iter()
            .filter(|id| **id == case.0)
            .count()
    }
}

impl<S: FallibleCaseStore> FallibleCaseStore for Faulty<S> {
    type Error = String;
    fn insert(&self, state: CaseState) -> Result<bool, String> {
        self.inner.insert(state).map_err(|e| e.to_string())
    }
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, String> {
        self.inner.get(case).map_err(|e| e.to_string())
    }
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, String> {
        let mut fail = self.fail_update_of.lock().unwrap();
        if fail.as_deref() == Some(case.0.as_str()) {
            *fail = None;
            return Err(format!("the write of {} failed (the process died)", case.0));
        }
        drop(fail);
        self.updates.lock().unwrap().push(case.0.clone());
        self.inner.update(case, change).map_err(|e| e.to_string())
    }
    fn observe(&self, observation: ObservationData) -> Result<(), String> {
        self.inner.observe(observation).map_err(|e| e.to_string())
    }
    fn observations(&self) -> Result<Vec<ObservationData>, String> {
        self.inner.observations().map_err(|e| e.to_string())
    }
}

// ---- opening ---------------------------------------------------------------------------------------

#[test]
fn performing_the_opening_action_opens_the_child_and_writes_one_open_record_in_the_same_write() {
    let dir = dir();
    let governor = governor_over(Faulty::new(FileCaseStore::open(&dir).unwrap()));
    let parent = root(&governor, "parent@1");
    let children = open_children(&governor, &parent, &["review"]);
    assert_eq!(children, [CaseId("case-1.review".into())]);
    let child = &children[0];
    assert_eq!(governor.protocol(child).unwrap(), "child@1");
    let records = outcomes(&governor, &parent);
    assert_eq!(records.len(), 1, "{records:#?}");
    assert_eq!(records[0]["child_case"], "case-1.review");
    assert_eq!(records[0]["subject"], "idea");
    assert_eq!(records[0]["subject_revision"], "idea-1");
    assert_eq!(records[0]["result"], "open");
    // The link, the open record and the `child_opened` record are one write of the parent.
    assert_eq!(governor_store(&governor).updates_of(&parent), 1);
    let held = governor.case_state(&parent).unwrap();
    assert_eq!(held.children.len(), 1);
    assert_eq!(held.children[0].child.0, "case-1.review");
    assert_eq!(held.children[0].action, OPEN);
    let child_state = governor.case_state(child).unwrap();
    assert_eq!(child_state.parent.as_ref(), Some(&held.children[0]));
    assert_eq!(claim(&governor, &parent, "children.passed"), Truth::Unknown);
    assert_eq!(claim(&governor, &parent, "child.opened"), Truth::True);
    let record = governor
        .evidence(&parent)
        .unwrap()
        .into_iter()
        .find(|record| record.kind == "case_outcome")
        .unwrap();
    assert_eq!(record.producer, CHILD_OUTCOME_PRODUCER);
}

fn governor_store<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> &S {
    governor.store()
}

#[test]
fn a_terminated_child_replaces_its_record_with_its_outcome_and_instant() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &child, "passed");
    governor
        .terminate(&child, "passed", T2)
        .expect("passed is legitimate");
    let records = outcomes(&governor, &parent);
    assert_eq!(records.len(), 1, "{records:#?}");
    assert_eq!(records[0]["child_case"], "case-1.review");
    assert_eq!(records[0]["result"], "passed");
    assert_eq!(records[0]["observed_at"], T2);
    assert_eq!(claim(&governor, &parent, "children.passed"), Truth::True);

    let (governor, _dir) = self::governor();
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &child, "failed");
    governor
        .terminate(&child, "failed", T2)
        .expect("failed is legitimate");
    assert_eq!(outcomes(&governor, &parent)[0]["result"], "failed");
    assert_eq!(claim(&governor, &parent, "children.passed"), Truth::False);
}

#[test]
fn the_subject_revision_is_the_childs_last_revision_when_it_declares_the_subject() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "holder@1");
    let child = governor
        .open_children(
            &parent,
            "draft.open",
            1,
            &[ChildOpening {
                name: "draft".into(),
                protocol: "drafter@1".into(),
                independence: CaseIndependence::none(),
            }],
            Vec::new(),
            T1,
        )
        .unwrap()
        .remove(0);
    assert_eq!(governor.revisions(&child).unwrap()["idea"], "idea-1");
    governor.update_revision(&child, "idea", "idea-2").unwrap();
    governor.update_revision(&child, "idea", "idea-3").unwrap();
    assert_eq!(
        outcomes(&governor, &parent)[0]["subject_revision"],
        "idea-1"
    );
    record(&governor, &child, "verdict", "idea", "passed", "drafter");
    governor.terminate(&child, "passed", T2).unwrap();
    assert_eq!(
        outcomes(&governor, &parent)[0]["subject_revision"],
        "idea-3"
    );
}

#[test]
fn the_subject_revision_is_the_parents_at_opening_when_the_child_does_not_declare_it() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    governor.update_revision(&parent, "idea", "idea-2").unwrap();
    verdict(&governor, &child, "passed");
    governor.terminate(&child, "passed", T2).unwrap();
    let records = outcomes(&governor, &parent);
    assert_eq!(records[0]["subject_revision"], "idea-1");
    assert_eq!(records[0]["result"], "passed");
}

#[test]
fn three_children_two_passed_leave_one_open_record_and_the_claim_not_true() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let children = open_children(&governor, &parent, &["a", "b", "c"]);
    assert_eq!(children.len(), 3);
    for child in &children[..2] {
        verdict(&governor, child, "passed");
        governor.terminate(child, "passed", T2).unwrap();
    }
    let records = outcomes(&governor, &parent);
    assert_eq!(records.len(), 3, "{records:#?}");
    let open: Vec<&Value> = records.iter().filter(|r| r["result"] == "open").collect();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["child_case"], "case-1.c");
    assert_ne!(claim(&governor, &parent, "children.passed"), Truth::True);
}

#[test]
fn a_repeated_opening_for_a_name_gets_the_next_numbered_id_and_leaves_the_first_record() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let first = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &first, "passed");
    governor.terminate(&first, "passed", T2).unwrap();
    let before = outcomes(&governor, &parent);
    let second = open_children(&governor, &parent, &["review"]).remove(0);
    let third = open_children(&governor, &parent, &["review"]).remove(0);
    assert_eq!(second.0, "case-1.review.2");
    assert_eq!(third.0, "case-1.review.3");
    let after = outcomes(&governor, &parent);
    assert_eq!(after.len(), 3);
    assert_eq!(after[0], before[0], "the first child's record is unchanged");
}

#[test]
fn a_name_that_is_not_one_plain_segment_is_refused_before_anything_is_written() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    for name in ["", "a.b", "a/b", "a b", "../x"] {
        let refused = governor.open_children(&parent, OPEN, 1, &[opening(name)], Vec::new(), T1);
        assert!(
            matches!(refused, Err(ChildError::InvalidName { .. })),
            "{name:?}: {refused:?}"
        );
    }
    let twice = governor.open_children(
        &parent,
        OPEN,
        1,
        &[opening("a"), opening("a")],
        Vec::new(),
        T1,
    );
    assert!(
        matches!(twice, Err(ChildError::RepeatedName { .. })),
        "{twice:?}"
    );
    assert!(governor.case_state(&parent).unwrap().children.is_empty());
    assert!(outcomes(&governor, &parent).is_empty());
}

// ---- what may open a child ------------------------------------------------------------------------

#[test]
fn only_a_case_open_action_of_the_parents_protocol_at_its_current_revision_opens_a_child() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let no_case_open =
        governor.open_children(&parent, "idea.note", 1, &[opening("a")], Vec::new(), T1);
    assert!(
        matches!(no_case_open, Err(ChildError::NoCaseOpen { .. })),
        "{no_case_open:?}"
    );
    let undeclared = governor.open_children(&parent, "nope", 1, &[opening("a")], Vec::new(), T1);
    assert!(
        matches!(undeclared, Err(ChildError::NoCaseOpen { .. })),
        "{undeclared:?}"
    );
    let stale = governor.open_children(&parent, OPEN, 7, &[opening("a")], Vec::new(), T1);
    assert!(matches!(stale, Err(ChildError::Stale { .. })), "{stale:?}");
    let other = governor.open_children(
        &parent,
        OPEN,
        1,
        &[ChildOpening {
            name: "a".into(),
            protocol: "drafter@1".into(),
            independence: CaseIndependence::none(),
        }],
        Vec::new(),
        T1,
    );
    assert!(
        matches!(other, Err(ChildError::ChildOfAnotherProtocol { .. })),
        "{other:?}"
    );
    let forged = governor.open_children(
        &parent,
        OPEN,
        1,
        &[opening("a")],
        vec![{
            let mut record = opened_record(&governor, &parent);
            record.kind = "case_outcome".into();
            record
        }],
        T1,
    );
    assert!(
        matches!(forged, Err(ChildError::RecordNotProduced { .. })),
        "{forged:?}"
    );
    assert!(governor.case_state(&parent).unwrap().children.is_empty());
    assert!(governor.case_state(&CaseId("case-1.a".into())).is_err());
}

#[test]
fn a_case_outcome_record_submitted_for_a_case_with_children_or_naming_a_held_case_is_refused() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let forged = submit_evidence(
        &governor,
        "attacker",
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: parent.clone(),
            kind: "case_outcome".into(),
            subject_revision: 1,
            producer: "attacker".into(),
            observation_ids: vec![observed(&governor)],
            facts: commission_json::parse(
                &json!({
                    "format": "canon-evidence/1",
                    "id": "forged",
                    "kind": "case_outcome",
                    "subject": "idea",
                    "subject_revision": "idea-1",
                    "result": "passed",
                    "child_case": "case-1.other",
                })
                .to_string(),
            )
            .unwrap(),
            provenance: commission_json::Value::Null,
        },
    );
    assert!(forged.is_err(), "{forged:?}");
    assert_eq!(outcomes(&governor, &parent).len(), 1);
    let _ = child;
}

#[test]
fn terminating_a_case_whose_parent_does_not_hold_its_link_is_refused_before_any_write() {
    let dir = dir();
    let governor = governor_over(FileCaseStore::open(&dir).unwrap());
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &child, "passed");
    // A host that rewrites the parent so it no longer lists the child: the link is forged.
    let store = FileCaseStore::open(&dir).unwrap();
    store
        .update(&parent, |state| state.children.clear())
        .unwrap();
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name().unwrap().to_str().unwrap()
                == format!("{}.json", loom_governor::case_file_stem(&child.0))
        })
        .unwrap();
    let before = std::fs::read(&file).unwrap();
    let refused = governor.terminate(&child, "passed", T2);
    assert!(
        matches!(refused, Err(TerminateError::LinkNotHeld { .. })),
        "{refused:?}"
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(outcomes(&governor, &parent)[0]["result"], "open");
}

#[test]
fn a_case_file_holding_a_link_of_another_case_is_refused_on_read() {
    for (member, field) in [("parent", "child"), ("children", "parent")] {
        let dir = dir();
        let governor = governor_over(FileCaseStore::open(&dir).unwrap());
        let parent = root(&governor, "parent@1");
        let child = open_children(&governor, &parent, &["review"]).remove(0);
        let holder = if member == "parent" { &child } else { &parent };
        let path = dir.join(format!("{}.json", loom_governor::case_file_stem(&holder.0)));
        let mut document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let link = match &mut document[member] {
            serde_json::Value::Array(links) => &mut links[0],
            link => link,
        };
        link[field] = serde_json::Value::from("elsewhere");
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let read = FileCaseStore::open(&dir).unwrap().get(holder);
        let error = read.expect_err(member).to_string();
        assert!(error.contains("link of `elsewhere`"), "{member}: {error}");
    }
}

// ---- termination ------------------------------------------------------------------------------------

#[test]
fn a_terminated_case_admits_no_action_and_completes_with_its_stored_outcome() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &child, "passed");
    governor.terminate(&child, "passed", T2).unwrap();
    let frontier = governor.frontier(&child).unwrap();
    assert!(
        frontier
            .data()
            .actions
            .iter()
            .all(|action| action.status == ActionStatus::Blocked),
        "{:?}",
        frontier.data().actions
    );
    assert_eq!(
        governor.completion(&child).unwrap(),
        CompletionDetermination::Complete(CompletionDeterminationComplete {
            outcome: "passed".into()
        })
    );
    // Terminated twice through the same outcome completes nothing new; another outcome is refused.
    governor.terminate(&child, "passed", T2).unwrap();
    assert_eq!(outcomes(&governor, &parent).len(), 1);
    assert!(matches!(
        governor.terminate(&child, "failed", T2),
        Err(TerminateError::AlreadyTerminated { .. })
    ));
    let refused = governor.open_children(&parent, OPEN, 1, &[opening("x")], Vec::new(), T1);
    assert!(refused.is_ok(), "the parent is open: {refused:?}");
}

#[test]
fn a_stored_termination_survives_a_reopen_and_an_illegitimate_one_leaves_the_file_unchanged() {
    let dir = dir();
    let case = {
        let governor = governor_over(FileCaseStore::open(&dir).unwrap());
        let parent = root(&governor, "parent@1");
        let child = open_children(&governor, &parent, &["review"]).remove(0);
        let path = dir.join(format!("{}.json", loom_governor::case_file_stem(&child.0)));
        let before = std::fs::read(&path).unwrap();
        let refused = governor.terminate(&child, "passed", T2);
        assert!(
            matches!(refused, Err(TerminateError::Refused { .. })),
            "{refused:?}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "byte-identical");
        verdict(&governor, &child, "passed");
        governor.terminate(&child, "passed", T2).unwrap();
        child
    };
    let governor = governor_over(FileCaseStore::open(&dir).unwrap());
    let termination = governor
        .case_state(&case)
        .unwrap()
        .termination
        .expect("held");
    assert_eq!(termination.outcome, "passed");
    assert_eq!(termination.instant.0, T2);
    assert_eq!(
        governor.completion(&case).unwrap(),
        CompletionDetermination::Complete(CompletionDeterminationComplete {
            outcome: "passed".into()
        })
    );
}

// ---- restarts between the two writes ------------------------------------------------------------

#[test]
fn a_restart_between_the_child_opened_and_the_link_recorded_adopts_the_child_and_opens_no_second() {
    let dir = dir();
    let governor = governor_over(Faulty::new(FileCaseStore::open(&dir).unwrap()));
    let parent = root(&governor, "parent@1");
    governor.store().fail_next_update_of(&parent);
    let failed = governor.open_children(
        &parent,
        OPEN,
        1,
        &[opening("review")],
        vec![opened_record(&governor, &parent)],
        T1,
    );
    assert!(failed.is_err(), "{failed:?}");
    assert!(governor.case_state(&CaseId("case-1.review".into())).is_ok());
    assert!(outcomes(&governor, &parent).is_empty());
    // The next process performs the action again.
    let governor = governor_over(FileCaseStore::open(&dir).unwrap());
    let children = open_children(&governor, &parent, &["review"]);
    assert_eq!(children, [CaseId("case-1.review".into())]);
    assert!(
        governor
            .case_state(&CaseId("case-1.review.2".into()))
            .is_err()
    );
    assert_eq!(outcomes(&governor, &parent).len(), 1);
    assert_eq!(governor.tree(&parent).unwrap().len(), 2);
}

#[test]
fn a_restart_between_the_child_terminated_and_the_outcome_written_is_completed_once() {
    let dir = dir();
    let governor = governor_over(Faulty::new(FileCaseStore::open(&dir).unwrap()));
    let parent = root(&governor, "parent@1");
    let child = open_children(&governor, &parent, &["review"]).remove(0);
    verdict(&governor, &child, "passed");
    governor.store().fail_next_update_of(&parent);
    assert!(governor.terminate(&child, "passed", T2).is_err());
    assert!(governor.case_state(&child).unwrap().termination.is_some());
    assert_eq!(outcomes(&governor, &parent)[0]["result"], "open");
    let governor = governor_over(FileCaseStore::open(&dir).unwrap());
    governor
        .terminate(&child, "passed", T2)
        .expect("completes the write");
    let records = outcomes(&governor, &parent);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["result"], "passed");
}

// ---- the tree ----------------------------------------------------------------------------------------

#[test]
fn the_cases_of_a_tree_are_listed_from_any_of_them_with_each_parent_link() {
    let (governor, _dir) = governor();
    let parent = root(&governor, "parent@1");
    let children = open_children(&governor, &parent, &["a", "b"]);
    for from in [&parent, &children[0], &children[1]] {
        let tree = governor.tree(from).unwrap();
        let listed: Vec<(&str, Option<&str>)> = tree
            .iter()
            .map(|case| {
                (
                    case.case.0.as_str(),
                    case.parent.as_ref().map(|link| link.parent.0.as_str()),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                ("case-1", None),
                ("case-1.a", Some("case-1")),
                ("case-1.b", Some("case-1")),
            ]
        );
    }
}

// ---- registration -------------------------------------------------------------------------------------

#[test]
fn a_catalog_in_which_two_protocols_open_each_other_is_refused_naming_the_cycle() {
    let ping = "format: protocol/1\nprotocol: {id: ping, revision: 1}\nartifacts: {thing: {}}\n\
                actions:\n  open: {case_open: {protocol: pong, subject: thing}}\n";
    let pong = "format: protocol/1\nprotocol: {id: pong, revision: 1}\nartifacts: {thing: {}}\n\
                actions:\n  open: {case_open: {protocol: ping, subject: thing}}\n";
    let mut catalog = ProtocolCatalog::default();
    catalog.add_yaml("ping@1", ping, source()).unwrap();
    catalog.add_yaml("pong@1", pong, source()).unwrap();
    let refused = CanonGovernor::new(loom_governor::MemoryCaseStore::default())
        .with_catalog(&catalog)
        .map(|_| ());
    match refused {
        Err(OpenError::InvalidProtocol { problem, .. }) => {
            assert!(problem.contains("ping -> pong -> ping"), "{problem}");
            assert!(problem.contains("case-open-cycle"), "{problem}");
        }
        other => panic!("not refused: {other:?}"),
    }
    let one = CanonGovernor::new(loom_governor::MemoryCaseStore::default())
        .with_protocol_yaml("ping@1", ping)
        .unwrap()
        .with_protocol_yaml("pong@1", pong)
        .map(|_| ());
    assert!(
        matches!(&one, Err(OpenError::InvalidProtocol { problem, .. }) if problem.contains("ping -> pong -> ping")),
        "{one:?}"
    );
}

// ---- trees deeper than one level, children beside other claims, host records -------------------

/// Opens `case` as a root under `protocol` with `artifacts`.
fn open_root<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
    protocol: &str,
    artifacts: &[(&str, &str)],
) -> CaseId {
    let case = CaseId(case.into());
    governor
        .open_case(
            case.clone(),
            protocol,
            artifacts
                .iter()
                .map(|(artifact, revision)| ((*artifact).to_owned(), (*revision).to_owned()))
                .collect(),
            CaseInputs::none(),
        )
        .expect("the root opens");
    case
}

/// Opens one `protocol` child per name through the `case_open` action `action` about `subject`.
fn open_through<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    parent: &CaseId,
    action: &str,
    protocol: &str,
    subject: &str,
    names: &[&str],
) -> Vec<CaseId> {
    let openings: Vec<ChildOpening> = names
        .iter()
        .map(|name| ChildOpening {
            name: (*name).to_owned(),
            protocol: protocol.to_owned(),
            independence: CaseIndependence::none(),
        })
        .collect();
    governor
        .open_children(
            parent,
            action,
            governor.current_revision(parent).unwrap(),
            &openings,
            vec![opened_about(governor, parent, subject)],
            T1,
        )
        .unwrap_or_else(|error| panic!("{action} opens: {error}"))
}

/// Submits, through the evidence port, a `case_outcome` record of `case` about `subject` naming
/// `child` with `result`; whether it was received.
fn submit_outcome<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    subject: &str,
    child: &str,
    result: &str,
) -> bool {
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let revision = governor.revisions(case).unwrap()[subject].clone();
    submit_evidence(
        governor,
        "host",
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: "case_outcome".into(),
            subject_revision: governor.current_revision(case).unwrap(),
            producer: "host".into(),
            observation_ids: vec![observed(governor)],
            facts: commission_json::parse(
                &json!({
                    "format": "canon-evidence/1",
                    "id": format!("host-outcome-{n}"),
                    "kind": "case_outcome",
                    "subject": subject,
                    "subject_revision": revision,
                    "result": result,
                    "child_case": child,
                })
                .to_string(),
            )
            .unwrap(),
            provenance: commission_json::Value::Null,
        },
    )
    .is_ok()
}

/// Ends the `child/1` case `case` passed.
fn pass<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &CaseId) {
    verdict(governor, case, "passed");
    governor
        .terminate(case, "passed", T2)
        .unwrap_or_else(|error| panic!("{} ends passed: {error}", case.0));
}

fn release<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    epic: &CaseId,
) -> (ActionStatus, Vec<String>) {
    let frontier = governor.frontier(epic).unwrap();
    let action = frontier
        .data()
        .actions
        .iter()
        .find(|action| action.action == "release")
        .expect("the frontier lists release")
        .clone();
    (action.status, action.reasons)
}

#[test]
fn a_three_level_tree_resolves_from_the_leaves_up_and_refuses_early_and_forged_outcomes() {
    let (governor, _dir) = governor();
    let root = open_root(&governor, "case-1", "tree@1", &[("roadmap", "roadmap-1")]);
    let epic = open_through(&governor, &root, "epic.open", "epic@1", "roadmap", &["e1"]).remove(0);
    let stories = open_through(
        &governor,
        &epic,
        "story.open",
        "child@1",
        "spec",
        &["s1", "s2"],
    );
    let ids: Vec<&str> = stories.iter().map(|story| story.0.as_str()).collect();
    assert_eq!(ids, ["case-1.e1.s1", "case-1.e1.s2"]);
    assert_eq!(
        governor.case_state(&epic).unwrap().artifacts,
        BTreeMap::from([
            ("roadmap".to_owned(), "roadmap-1".to_owned()),
            ("spec".to_owned(), loom_governor::UNREVISED.to_owned()),
        ])
    );
    let tree: Vec<(String, Option<String>)> = governor
        .tree(&stories[1])
        .unwrap()
        .into_iter()
        .map(|member| (member.case.0, member.parent.map(|link| link.parent.0)))
        .collect();
    assert_eq!(
        tree,
        [
            ("case-1".to_owned(), None),
            ("case-1.e1".to_owned(), Some("case-1".to_owned())),
            ("case-1.e1.s1".to_owned(), Some("case-1.e1".to_owned())),
            ("case-1.e1.s2".to_owned(), Some("case-1.e1".to_owned())),
        ]
    );
    record(
        &governor, &epic, "approval", "roadmap", "approved", "approver",
    );
    record(&governor, &epic, "gate", "spec", "passed", "gate");

    // Neither the epic nor the root ends before its children did.
    let early = governor.terminate(&epic, "passed", T2);
    assert!(
        matches!(early, Err(TerminateError::Refused { .. })),
        "{early:?}"
    );
    let early = governor.terminate(&root, "delivered", T2);
    assert!(
        matches!(early, Err(TerminateError::Refused { .. })),
        "{early:?}"
    );
    // A case_outcome submitted at either level is refused, naming a held case or not.
    for (case, subject, child) in [
        (&root, "roadmap", "case-1.e1"),
        (&root, "roadmap", "case-1.e2"),
        (&epic, "spec", "case-1.e1.s1"),
        (&epic, "spec", "case-1.e1.s3"),
        (&stories[0], "work", "case-1.e1.s1.x"),
    ] {
        assert!(
            !submit_outcome(&governor, case, subject, child, "passed"),
            "{} received a case_outcome naming {child}",
            case.0
        );
    }

    pass(&governor, &stories[0]);
    assert_eq!(claim(&governor, &epic, "children.passed"), Truth::Unknown);
    assert_eq!(claim(&governor, &root, "children.passed"), Truth::Unknown);
    pass(&governor, &stories[1]);
    assert_eq!(claim(&governor, &epic, "children.passed"), Truth::True);
    assert_eq!(
        claim(&governor, &root, "children.passed"),
        Truth::Unknown,
        "the epic has not ended"
    );
    assert!(matches!(
        governor.terminate(&root, "delivered", T2),
        Err(TerminateError::Refused { .. })
    ));

    governor.terminate(&epic, "passed", T2).unwrap();
    assert_eq!(outcomes(&governor, &root)[0]["result"], "passed");
    assert_eq!(outcomes(&governor, &root)[0]["child_case"], "case-1.e1");
    assert_eq!(claim(&governor, &root, "children.passed"), Truth::True);
    // The terminated epic opens no further story; its stories' records stay as they ended.
    let again = governor.open_children(
        &epic,
        "story.open",
        governor.current_revision(&epic).unwrap(),
        &[ChildOpening {
            name: "s3".into(),
            protocol: "child@1".into(),
            independence: CaseIndependence::none(),
        }],
        vec![opened_about(&governor, &epic, "spec")],
        T2,
    );
    assert!(
        matches!(again, Err(ChildError::Terminated { .. })),
        "{again:?}"
    );
    let results: Vec<Value> = outcomes(&governor, &epic)
        .into_iter()
        .map(|facts| facts["result"].clone())
        .collect();
    assert_eq!(results, [json!("passed"), json!("passed")]);

    governor.terminate(&root, "delivered", T2).unwrap();
    assert_eq!(
        governor.completion(&root).unwrap(),
        CompletionDetermination::Complete(CompletionDeterminationComplete {
            outcome: "delivered".into()
        })
    );
}

#[test]
fn children_passed_beside_an_approval_and_a_gate_opens_the_release_and_the_outcome_only_with_all_three()
 {
    const CLAIMS: [&str; 3] = ["roadmap.approved", "gate.passed", "children.passed"];
    for missing in CLAIMS.iter().copied().chain(["none"]) {
        let (governor, _dir) = governor();
        let epic = open_root(
            &governor,
            "case-1",
            "epic@1",
            &[("roadmap", "roadmap-1"), ("spec", "spec-1")],
        );
        let stories = open_through(
            &governor,
            &epic,
            "story.open",
            "child@1",
            "spec",
            &["s1", "s2"],
        );
        if missing != "roadmap.approved" {
            record(
                &governor, &epic, "approval", "roadmap", "approved", "approver",
            );
        }
        if missing != "gate.passed" {
            record(&governor, &epic, "gate", "spec", "passed", "gate");
        }
        pass(&governor, &stories[0]);
        if missing != "children.passed" {
            pass(&governor, &stories[1]);
        }
        let (status, reasons) = release(&governor, &epic);
        let completion = governor.completion(&epic).unwrap();
        if missing == "none" {
            assert_eq!(status, ActionStatus::Admissible, "{reasons:?}");
            assert_eq!(
                completion,
                CompletionDetermination::Complete(CompletionDeterminationComplete {
                    outcome: "passed".into()
                })
            );
            continue;
        }
        assert_eq!(status, ActionStatus::Blocked, "{missing}");
        assert!(
            !matches!(completion, CompletionDetermination::Complete(_)),
            "{missing}: {completion:?}"
        );
        let named = reasons.join(" ");
        for claim_id in CLAIMS {
            assert_eq!(
                named.contains(&format!("\"{claim_id}\"")),
                claim_id == missing,
                "{missing}: {reasons:?}"
            );
        }
        assert_eq!(
            claim(&governor, &epic, missing),
            Truth::Unknown,
            "{missing}"
        );
        assert!(
            matches!(
                governor.terminate(&epic, "passed", T2),
                Err(TerminateError::Refused { .. })
            ),
            "{missing}"
        );
    }
}

#[test]
fn a_host_case_outcome_of_a_lone_case_is_read_by_that_case_alone_and_passes_no_child_it_links_later()
 {
    let (governor, _dir) = governor();
    let lone = root(&governor, "parent@1");
    let sibling = open_root(&governor, "case-2", "parent@1", &[("idea", "idea-1")]);
    // Accepted: no children, no parent, naming no held case (case_composition requires it), and
    // read by that case, as the record of a child it never opened.
    assert!(submit_outcome(
        &governor,
        &lone,
        "idea",
        "case-1.review",
        "passed"
    ));
    assert_eq!(claim(&governor, &lone, "children.passed"), Truth::True);
    // A sibling does not read it.
    assert_eq!(
        claim(&governor, &sibling, "children.passed"),
        Truth::Unknown
    );
    let cousin = open_children(&governor, &sibling, &["review"]).remove(0);
    assert_eq!(
        claim(&governor, &sibling, "children.passed"),
        Truth::Unknown
    );
    pass(&governor, &cousin);
    assert_eq!(claim(&governor, &sibling, "children.passed"), Truth::True);
    // The case later opens a child of the record's name: the child gets the next id and its own
    // record, which the claim waits for; the host's record does not pass it.
    let child = open_children(&governor, &lone, &["review"]).remove(0);
    assert_eq!(child.0, "case-1.review.2");
    assert_eq!(claim(&governor, &lone, "children.passed"), Truth::Unknown);
    assert!(!submit_outcome(
        &governor,
        &lone,
        "idea",
        "case-1.review.2",
        "passed"
    ));
    verdict(&governor, &child, "failed");
    governor.terminate(&child, "failed", T2).unwrap();
    assert_eq!(claim(&governor, &lone, "children.passed"), Truth::False);
    // The child's record is the governor's, not the host's.
    let held = outcomes(&governor, &lone);
    assert_eq!(held.len(), 2);
    assert_eq!(held[1]["child_case"], "case-1.review.2");
    assert_eq!(held[1]["result"], "failed");
}
