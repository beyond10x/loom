//! Acceptance for `story:record-copy-between-cases`: an action bound to a `copy` executor,
//! performed through Commission's own loop (`run_until_blocked`) and the copy effect port, copies
//! the latest applying record of the binding's kind about the current revision of its subject from
//! another case of the same tree into the case that performs it.
//!
//! The fixture tree (`tests/fixtures/record-copy/`): a `direction@1` root `case-1` that opens the
//! `epic@1` children `case-1.a` and `case-1.b`; `audit@1` is registered and holds no case of the
//! tree. Cases live in a `FileCaseStore` under `CARGO_TARGET_TMPDIR`. No model, no network; one
//! case starts a stub program that exits at once.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use b10x_loom_bindings::{COPY_SOURCE, CopyEffects, PROGRAM_SOURCE, ProgramEffects, check, decode};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, EvidenceId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Truth,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, EngineConfinement, EngineRunner, TestRunner,
};
use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason};
use loom_governor::{
    CanonGovernor, CaseIndependence, CaseInputs, CaseState, ChildOpening, FallibleCaseStore,
    FileCaseStore,
};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::json;

const DIRECTION: &str = include_str!("fixtures/record-copy/direction-1.yaml");
const EPIC: &str = include_str!("fixtures/record-copy/epic-1.yaml");
const AUDIT: &str = include_str!("fixtures/record-copy/audit-1.yaml");
/// The copy action of an epic.
const READ: &str = "roadmap.read";
const KIND: &str = "roadmap_approval";
const ROOT: &str = "case-1";
const FIRST: &str = "case-1.a";
const SECOND: &str = "case-1.b";
const T1: &str = "2026-11-21T10:00:00Z";
const T2: &str = "2026-11-21T11:00:00Z";
const T3: &str = "2026-11-21T12:00:00Z";

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    for (name, yaml) in [
        ("direction@1", DIRECTION),
        ("epic@1", EPIC),
        ("audit@1", AUDIT),
    ] {
        catalog
            .add_yaml(
                name,
                yaml,
                ProtocolSource {
                    kind: SourceKind::Memory,
                    location: "record-copy-fixture".into(),
                    revision: String::new(),
                    path: String::new(),
                },
            )
            .unwrap();
    }
    catalog
}

/// A scratch directory under the build directory; `store/` holds the cases.
struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("bound-copy-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let root = dir.path().to_owned();
        std::fs::create_dir(root.join("store")).unwrap();
        Self { _dir: dir, root }
    }

    fn store(&self) -> PathBuf {
        self.root.join("store")
    }
}

type Gov = CanonGovernor<FileCaseStore>;

fn governor_over<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store).with_catalog(&catalog()).unwrap()
}

fn governor(scratch: &Scratch) -> Gov {
    governor_over(FileCaseStore::open(scratch.store()).unwrap())
}

fn owner_independent() -> CaseIndependence {
    CaseIndependence::none()
        .with("owner", ["different_principal"])
        .unwrap()
}

fn id(case: &str) -> CaseId {
    CaseId(case.to_owned())
}

/// Opens the root `root` (`direction@1`) and its epic children `<root>.a` and `<root>.b`, `a`
/// declaring `owner` independent, `b` with `b_independence`.
fn tree_with<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    root: &str,
    b_independence: CaseIndependence,
) {
    governor
        .open_case(
            id(root),
            "direction@1",
            BTreeMap::from([
                ("roadmap".to_owned(), "roadmap-1".to_owned()),
                ("plan".to_owned(), "plan-1".to_owned()),
                ("brief".to_owned(), "brief-1".to_owned()),
            ]),
            CaseInputs::none(),
        )
        .unwrap();
    let revision = governor.current_revision(&id(root)).unwrap();
    let opened = governor
        .open_children(
            &id(root),
            "epics.open",
            revision,
            &[
                ChildOpening {
                    name: "a".into(),
                    protocol: "epic@1".into(),
                    independence: owner_independent(),
                },
                ChildOpening {
                    name: "b".into(),
                    protocol: "epic@1".into(),
                    independence: b_independence,
                },
            ],
            Vec::new(),
            T1,
        )
        .unwrap();
    assert_eq!(opened, [id(&format!("{root}.a")), id(&format!("{root}.b"))]);
}

/// [`tree_with`], both children declaring `owner` independent.
fn tree<S: FallibleCaseStore>(governor: &CanonGovernor<S>, root: &str) {
    tree_with(governor, root, owner_independent());
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn next() -> u64 {
    ISSUED.fetch_add(1, Ordering::Relaxed)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A record to submit: of [`KIND`] about `subject` (the case's current revision of it unless
/// `revision` names one), with `result`, observed at `at`, by `producer`, and the facts `extra`.
struct Record {
    subject: &'static str,
    revision: Option<&'static str>,
    result: &'static str,
    at: &'static str,
    producer: &'static str,
    extra: serde_json::Value,
}

fn approval(at: &'static str) -> Record {
    Record {
        subject: "roadmap",
        revision: None,
        result: "approved",
        at,
        producer: "owner",
        extra: json!({}),
    }
}

/// Submits `record` into `case` through the evidence port, citing an observation at its instant;
/// answers its evidence id.
fn hold<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str, record: Record) -> String {
    let case = id(case);
    let n = next();
    let observation = ObservationId(uuid(next()));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:record-copy".into(),
            subject: record.subject.into(),
            observed_at: Timestamp(record.at.into()),
            payload: Value::Null,
        }),
    )
    .unwrap();
    let revision = record.revision.map_or_else(
        || governor.revisions(&case).unwrap()[record.subject].clone(),
        str::to_owned,
    );
    let mut facts = json!({
        "format": "canon-evidence/1",
        "id": format!("approval-{n}"),
        "kind": KIND,
        "subject": record.subject,
        "subject_revision": revision,
        "result": record.result,
        "observed_at": record.at,
    });
    for (name, value) in record.extra.as_object().unwrap() {
        facts[name] = value.clone();
    }
    submit_evidence(
        governor,
        record.producer,
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: KIND.into(),
            subject_revision: governor.current_revision(&case).unwrap(),
            producer: record.producer.into(),
            observation_ids: vec![observation],
            facts: json::parse(&facts.to_string()).unwrap(),
            provenance: Value::Null,
        },
    )
    .unwrap();
    uuid(n).0
}

/// The epic's document: [`READ`] bound to `copy` of [`KIND`] from `from`.
fn copy_bindings(from: &str) -> BindingsDocument {
    let text = format!(
        "format: loom.bindings/1\nprotocol: epic@1\nactions:\n\
         \x20 - action: {READ}\n    executor: {{kind: copy, evidence: {KIND}, from: {from}}}\n"
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    let catalog = catalog();
    check(&document, "epic@1", &catalog.get("epic@1").unwrap().model)
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

struct Context;

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(uuid(next()))
    }
    fn observation_id(&mut self) -> ObservationId {
        ObservationId(uuid(next()))
    }
    fn now(&mut self) -> Timestamp {
        Timestamp(T3.to_owned())
    }
    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// One step of `case` in which the executor proposes `action` with `arguments`, performed by
/// `effects`.
fn try_drive<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &dyn EffectPort,
    case: &str,
    action: &str,
    arguments: &serde_json::Value,
) -> Result<LoopEnd, LoopError> {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(next())),
        agent_revision_id: AgentRevisionId(uuid(next())),
        case_id: id(case),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: action.to_owned(),
            arguments: ProposedActionArguments(json::parse(&arguments.to_string()).unwrap()),
        },
    )]);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(next()))));
    run_until_blocked(
        governor,
        &executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission,
        &mut runs,
        &mut Context,
    )
}

/// One copy step of [`READ`] in `case` with no arguments, through a port built for `from`.
fn copy_step<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str, from: &str) -> LoopEnd {
    let document = copy_bindings(from);
    let effects = CopyEffects::new(governor, id(case), &document, &catalog()).unwrap();
    try_drive(governor, &effects, case, READ, &json!({}))
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn only_effect(end: &LoopEnd) -> &EffectOutcome {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    &end.effects[0]
}

fn performed(end: &LoopEnd) -> serde_json::Value {
    match only_effect(end) {
        EffectOutcome::Performed(performed) => plain(&performed.report),
        other => panic!("not performed: {other:?}"),
    }
}

fn refusal(end: &LoopEnd) -> String {
    match only_effect(end) {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

fn plain(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Number(spelling) => serde_json::from_str(spelling).unwrap(),
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Array(items) => serde_json::Value::Array(items.iter().map(plain).collect()),
        Value::Object(members) => serde_json::Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// The records of [`KIND`] `case` holds, with their facts.
fn held<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> Vec<(EvidenceData, serde_json::Value)> {
    governor
        .evidence(&id(case))
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == KIND)
        .map(|record| {
            let facts = plain(&record.facts);
            (record, facts)
        })
        .collect()
}

/// The one record of [`KIND`] `case` holds.
fn the_copy<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> (EvidenceData, serde_json::Value) {
    let mut records = held(governor, case);
    assert_eq!(records.len(), 1, "{records:#?}");
    records.remove(0)
}

fn copy_observations<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> Vec<ObservationData> {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == COPY_SOURCE)
        .collect()
}

fn claim<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str, name: &str) -> Truth {
    governor
        .frontier(&id(case))
        .unwrap()
        .data()
        .claims
        .iter()
        .find(|entry| entry.claim == name)
        .unwrap()
        .value
}

/// Asserts `end` is the refusal `reason` and that `case` holds no record of [`KIND`] and no copy
/// observation was written.
fn refused_writing_nothing<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    end: &LoopEnd,
    case: &str,
    reason: &str,
) {
    let refused = refusal(end);
    assert!(
        refused.starts_with(&format!("{reason}: action {READ}")),
        "{refused}"
    );
    assert_eq!(held(governor, case), Vec::new());
    assert_eq!(copy_observations(governor), Vec::new());
}

// ---------------------------------------------------------------------------------------------
// Acceptance

#[test]
fn a_record_of_the_first_child_is_copied_into_the_second_with_its_result_producer_instant_and_revision()
 {
    // `from` names the protocol id or the registration.
    for from in ["epic", "epic@1"] {
        let scratch = Scratch::new();
        let governor = governor(&scratch);
        tree(&governor, ROOT);
        let source = hold(&governor, FIRST, approval(T1));
        let report = performed(&copy_step(&governor, SECOND, from));
        let (record, facts) = the_copy(&governor, SECOND);
        assert_eq!(record.producer, "owner", "{from}");
        assert_eq!(record.kind, KIND);
        assert_eq!(facts["kind"], KIND);
        assert_eq!(facts["subject"], "roadmap");
        assert_eq!(facts["subject_revision"], "roadmap-1");
        assert_eq!(facts["result"], "approved");
        assert_eq!(facts["observed_at"], T1);
        assert_ne!(record.evidence_id.0.0, source);
        assert_eq!(report["copied"]["source_case"], FIRST);
        assert_eq!(report["copied"]["source_evidence"], source.as_str());
        // The source keeps its record; nothing reaches the root.
        assert_eq!(held(&governor, FIRST).len(), 1);
        assert_eq!(held(&governor, ROOT), Vec::new());
    }
}

#[test]
fn a_record_about_an_earlier_revision_is_not_copied_and_alone_is_refused_no_source_record() {
    // A later record about the earlier revision is passed over for the current one.
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, approval(T1));
    governor
        .update_revision(&id(FIRST), "roadmap", "roadmap-2")
        .unwrap();
    hold(
        &governor,
        FIRST,
        Record {
            revision: Some("roadmap-1"),
            result: "stale",
            ..approval(T3)
        },
    );
    let current = hold(
        &governor,
        FIRST,
        Record {
            result: "current",
            ..approval(T2)
        },
    );
    performed(&copy_step(&governor, SECOND, "epic"));
    let (_, facts) = the_copy(&governor, SECOND);
    assert_eq!(facts["result"], "current");
    assert_eq!(facts["subject_revision"], "roadmap-2");
    assert_eq!(
        plain(&copy_observations(&governor)[0].payload)["source_evidence"],
        current.as_str()
    );

    // With only records about an earlier revision the step is refused.
    let scratch = Scratch::new();
    let governor = self::governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, approval(T1));
    governor
        .update_revision(&id(FIRST), "roadmap", "roadmap-2")
        .unwrap();
    let end = copy_step(&governor, SECOND, "epic");
    refused_writing_nothing(&governor, &end, SECOND, "NoSourceRecord");
}

#[test]
fn of_two_records_at_the_current_revision_the_later_is_copied() {
    // Received in either order, the record with the later instant is copied.
    for order in [[T1, T2], [T2, T1]] {
        let scratch = Scratch::new();
        let governor = governor(&scratch);
        tree(&governor, ROOT);
        for at in order {
            hold(
                &governor,
                FIRST,
                Record {
                    result: if at == T2 { "later" } else { "earlier" },
                    ..approval(at)
                },
            );
        }
        performed(&copy_step(&governor, SECOND, "epic"));
        let (_, facts) = the_copy(&governor, SECOND);
        assert_eq!(facts["result"], "later", "{order:?}");
        assert_eq!(facts["observed_at"], T2, "{order:?}");
    }
}

#[test]
fn an_upstream_entry_the_receiving_protocol_does_not_declare_is_dropped_and_named() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    hold(
        &governor,
        ROOT,
        Record {
            extra: json!({"upstream_revisions": {"plan": "plan-1", "brief": "brief-1"}}),
            ..approval(T1)
        },
    );
    let report = performed(&copy_step(&governor, SECOND, "direction"));
    let (_, facts) = the_copy(&governor, SECOND);
    assert_eq!(facts["upstream_revisions"], json!({"brief": "brief-1"}));
    assert_eq!(report["copied"]["dropped_upstream"], json!(["plan"]));
    let observations = copy_observations(&governor);
    assert_eq!(
        plain(&observations[0].payload)["dropped_upstream"],
        json!(["plan"])
    );
}

#[test]
fn the_copy_cites_an_observation_of_the_copy_effect_naming_the_source_at_its_instant() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    let source = hold(&governor, FIRST, approval(T2));
    performed(&copy_step(&governor, SECOND, "epic"));
    let (record, _) = the_copy(&governor, SECOND);
    let observations = copy_observations(&governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let observation = &observations[0];
    assert_eq!(
        record.observation_ids,
        std::slice::from_ref(&observation.observation_id)
    );
    assert_eq!(observation.source, COPY_SOURCE);
    assert_eq!(observation.observed_at.0, T2);
    let payload = plain(&observation.payload);
    assert_eq!(payload["case"], SECOND);
    assert_eq!(payload["action"], READ);
    assert_eq!(payload["source_case"], FIRST);
    assert_eq!(payload["source_evidence"], source.as_str());
    // The governor took the record: its instant is its observation's.
    governor.check_evidence(&record).unwrap();
}

#[test]
fn a_record_the_source_holds_as_not_applying_is_not_copied() {
    let not_applying = || Record {
        result: "not-applying",
        extra: json!({"note": "a member canon-evidence/1 does not declare"}),
        ..approval(T2)
    };
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, approval(T1));
    hold(&governor, FIRST, not_applying());
    let applies: Vec<bool> = governor
        .case_state(&id(FIRST))
        .unwrap()
        .evidence
        .iter()
        .map(|held| held.applies)
        .collect();
    assert_eq!(applies, [true, false]);
    performed(&copy_step(&governor, SECOND, "epic"));
    let (_, facts) = the_copy(&governor, SECOND);
    assert_eq!(facts["result"], "approved");

    let scratch = Scratch::new();
    let governor = self::governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, not_applying());
    let end = copy_step(&governor, SECOND, "epic");
    refused_writing_nothing(&governor, &end, SECOND, "NoSourceRecord");
}

#[test]
fn the_receiving_case_judges_the_copy_by_its_own_independence_map() {
    // The first child lists `owner` as independent, the second lists no producer.
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree_with(&governor, ROOT, CaseIndependence::none());
    hold(&governor, FIRST, approval(T1));
    assert_eq!(claim(&governor, FIRST, "roadmap.approved"), Truth::True);
    performed(&copy_step(&governor, SECOND, "epic"));
    let (record, _) = the_copy(&governor, SECOND);
    assert_eq!(record.producer, "owner");
    assert_eq!(claim(&governor, SECOND, "roadmap.approved"), Truth::Unknown);

    // The same copy into a case whose map lists `owner` satisfies the claim.
    let scratch = Scratch::new();
    let governor = self::governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, approval(T1));
    performed(&copy_step(&governor, SECOND, "epic"));
    assert_eq!(claim(&governor, SECOND, "roadmap.approved"), Truth::True);
}

#[test]
fn the_tree_and_the_held_records_with_their_applying_state_are_read_through_the_governor() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    tree(&governor, "case-2");
    hold(&governor, FIRST, approval(T1));
    let members: Vec<String> = governor
        .tree(&id(SECOND))
        .unwrap()
        .into_iter()
        .map(|member| member.case.0)
        .collect();
    assert_eq!(members, [ROOT, FIRST, SECOND]);
    let state: CaseState = governor.case_state(&id(FIRST)).unwrap();
    assert_eq!(state.evidence.len(), 1);
    assert!(state.evidence[0].applies);
    assert_eq!(state.artifacts["roadmap"], "roadmap-1");
}

#[test]
fn copy_due_is_true_before_the_copy_and_false_after_and_a_step_while_false_writes_nothing() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    let document = copy_bindings("epic");
    let effects = CopyEffects::new(&governor, id(SECOND), &document, &catalog()).unwrap();
    // No source record: not due.
    assert!(!effects.copy_due(READ).unwrap());
    hold(&governor, FIRST, approval(T1));
    assert!(effects.copy_due(READ).unwrap());
    performed(&try_drive(&governor, &effects, SECOND, READ, &json!({})).unwrap());
    assert!(!effects.copy_due(READ).unwrap());
    let records = held(&governor, SECOND);
    let observations = copy_observations(&governor);
    let report = performed(&try_drive(&governor, &effects, SECOND, READ, &json!({})).unwrap());
    assert_eq!(report["copied"], serde_json::Value::Null, "{report}");
    assert_eq!(held(&governor, SECOND), records);
    assert_eq!(copy_observations(&governor), observations);
    // A later record of the source is due again, and its copy is the second record.
    hold(&governor, FIRST, approval(T2));
    assert!(effects.copy_due(READ).unwrap());
    performed(&try_drive(&governor, &effects, SECOND, READ, &json!({})).unwrap());
    assert_eq!(held(&governor, SECOND).len(), 2);
    assert!(!effects.copy_due(READ).unwrap());
    // An action not bound to copy is never due.
    assert!(!effects.copy_due("roadmap.approve").unwrap());
}

#[test]
fn a_subject_the_receiving_protocol_does_not_declare_is_refused_subject_undeclared() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    hold(
        &governor,
        ROOT,
        Record {
            subject: "plan",
            ..approval(T1)
        },
    );
    let end = copy_step(&governor, SECOND, "direction");
    refused_writing_nothing(&governor, &end, SECOND, "SubjectUndeclared");
}

#[test]
fn a_source_protocol_with_no_case_in_the_tree_is_refused_no_source_case() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    // A case of `audit@1` in another tree of the same store holds a matching record.
    governor
        .open_case(
            id("case-9"),
            "audit@1",
            BTreeMap::from([("roadmap".to_owned(), "roadmap-1".to_owned())]),
            CaseInputs::none(),
        )
        .unwrap();
    hold(&governor, "case-9", approval(T1));
    let end = copy_step(&governor, SECOND, "audit");
    refused_writing_nothing(&governor, &end, SECOND, "NoSourceCase");
    // The receiving case itself is no source: the only epic case of a tree of one child.
    let scratch = Scratch::new();
    let governor = self::governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, SECOND, approval(T1));
    let refused = refusal(&copy_step(&governor, SECOND, "epic"));
    // `case-1.a` is a source case, holding no record.
    assert!(refused.starts_with("NoSourceRecord"), "{refused}");
}

#[test]
fn a_matching_record_in_another_tree_of_the_same_store_is_not_copied() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    tree(&governor, "case-2");
    hold(&governor, "case-2.a", approval(T1));
    hold(&governor, "case-2", approval(T1));
    for from in ["epic", "direction"] {
        let end = copy_step(&governor, SECOND, from);
        refused_writing_nothing(&governor, &end, SECOND, "NoSourceRecord");
    }
}

// ---------------------------------------------------------------------------------------------
// The step's guards

/// A store over a `FileCaseStore` whose updates fail while `failing` is set: an evidence write
/// that a restart interrupts.
struct Interrupted {
    inner: FileCaseStore,
    failing: AtomicBool,
}

impl FallibleCaseStore for Interrupted {
    type Error = String;
    fn insert(&self, state: CaseState) -> Result<bool, String> {
        self.inner.insert(state).map_err(|error| error.to_string())
    }
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, String> {
        self.inner.get(case).map_err(|error| error.to_string())
    }
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, String> {
        if self.failing.load(Ordering::Relaxed) {
            return Err("the process ended before the write".into());
        }
        self.inner
            .update(case, change)
            .map_err(|error| error.to_string())
    }
    fn observe(&self, observation: ObservationData) -> Result<(), String> {
        self.inner
            .observe(observation)
            .map_err(|error| error.to_string())
    }
    fn observations(&self) -> Result<Vec<ObservationData>, String> {
        self.inner.observations().map_err(|error| error.to_string())
    }
}

#[test]
fn a_restart_between_the_observation_and_the_record_is_completed_without_a_second_record() {
    let scratch = Scratch::new();
    let store = |failing| Interrupted {
        inner: FileCaseStore::open(scratch.store()).unwrap(),
        failing: AtomicBool::new(failing),
    };
    let before = governor_over(store(false));
    tree(&before, ROOT);
    hold(&before, FIRST, approval(T1));
    before.store().failing.store(true, Ordering::Relaxed);
    let document = copy_bindings("epic");
    {
        let effects = CopyEffects::new(&before, id(SECOND), &document, &catalog()).unwrap();
        let interrupted = try_drive(&before, &effects, SECOND, READ, &json!({}));
        assert!(interrupted.is_err(), "{interrupted:?}");
    }
    assert_eq!(copy_observations(&before).len(), 1);
    assert_eq!(held(&before, SECOND), Vec::new());
    drop(before);
    // The restarted host: a governor over the same store directory.
    let after = governor(&scratch);
    let effects = CopyEffects::new(&after, id(SECOND), &document, &catalog()).unwrap();
    assert!(effects.copy_due(READ).unwrap());
    performed(&try_drive(&after, &effects, SECOND, READ, &json!({})).unwrap());
    let observations = copy_observations(&after);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let (record, _) = the_copy(&after, SECOND);
    assert_eq!(
        record.observation_ids,
        std::slice::from_ref(&observations[0].observation_id)
    );
    performed(&try_drive(&after, &effects, SECOND, READ, &json!({})).unwrap());
    assert_eq!(held(&after, SECOND).len(), 1);
    assert_eq!(copy_observations(&after).len(), 1);
}

#[test]
fn no_request_argument_selects_the_source() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    tree(&governor, "case-2");
    let source = hold(&governor, FIRST, approval(T1));
    let decoy = hold(
        &governor,
        "case-2.a",
        Record {
            result: "decoy",
            ..approval(T3)
        },
    );
    let document = copy_bindings("epic");
    let effects = CopyEffects::new(&governor, id(SECOND), &document, &catalog()).unwrap();
    let arguments = json!({
        "from": "direction",
        "evidence": "other_kind",
        "kind": "other_kind",
        "source_case": "case-2.a",
        "source_evidence": decoy,
    });
    performed(&try_drive(&governor, &effects, SECOND, READ, &arguments).unwrap());
    let (_, facts) = the_copy(&governor, SECOND);
    assert_eq!(facts["result"], "approved");
    assert_eq!(
        plain(&copy_observations(&governor)[0].payload)["source_evidence"],
        source.as_str()
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs a Linux host process")]
fn a_bound_program_writes_no_observation_of_the_copy_source() {
    let scratch = Scratch::new();
    let workspace = scratch.root.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    // A stub that ends at once, printing a report that claims the copy source.
    let program = scratch.root.join("claims-copy.sh");
    std::fs::write(
        &program,
        "#!/bin/sh\nprintf '{\"source\":\"loom/copy\",\"case\":\"case-1.b\",\"source_case\":\"case-1.a\"}'\n",
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    let text = format!(
        "format: loom.bindings/1\nprotocol: epic@1\nactions:\n\
         \x20 - action: roadmap.approve\n    executor: {{kind: program, argv: [{}]}}\n    evidence:\n\
         \x20     - {{kind: {KIND}, subject: roadmap, producer: owner, from: /source, results: {{\"*\": approved}}}}\n",
        program.display()
    );
    let document = decode(&text).unwrap();
    let runner: Arc<dyn TestRunner> = Arc::new(
        EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap(),
    );
    let effects = ProgramEffects::new(
        &governor,
        id(FIRST),
        &document,
        &workspace,
        &scratch.store(),
        Some(runner),
    )
    .unwrap();
    performed(&try_drive(&governor, &effects, FIRST, "roadmap.approve", &json!({})).unwrap());
    let sources: Vec<String> = governor
        .try_observations()
        .unwrap()
        .into_iter()
        .map(|observation| observation.source)
        .filter(|source| source.starts_with("loom/"))
        .collect();
    assert_eq!(sources, [PROGRAM_SOURCE]);
    // The program's record is copied, and the copy is the first observation of the copy source.
    let copy = copy_bindings("epic");
    let copier = CopyEffects::new(&governor, id(SECOND), &copy, &catalog()).unwrap();
    assert!(copier.copy_due(READ).unwrap());
}

#[test]
fn a_from_the_catalog_does_not_name_is_refused_when_the_port_is_built() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    let document = copy_bindings("unknown@1");
    let refused = CopyEffects::new(&governor, id(SECOND), &document, &catalog())
        .map(|_| ())
        .expect_err("no registration and no protocol id");
    assert_eq!(refused.0.reason, Reason::ProtocolMismatch, "{refused}");
    assert_eq!(refused.0.action.as_deref(), Some(READ));
    assert_eq!(refused.0.member, "executor.from");
}

#[test]
fn the_copy_port_starts_no_process() {
    let source = include_str!("../src/copy.rs");
    for named in [
        "process::Command",
        "Command::new",
        "run_child",
        "TestRunner",
        "spawn(",
    ] {
        assert!(!source.contains(named), "copy.rs names {named}");
    }
}

/// The store directory holds nothing outside the governor's files: the copy writes no file of its
/// own.
#[test]
fn a_copy_writes_no_file_beside_the_cases() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    tree(&governor, ROOT);
    hold(&governor, FIRST, approval(T1));
    let listing = |dir: &Path| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let before = listing(&scratch.store());
    performed(&copy_step(&governor, SECOND, "epic"));
    assert_eq!(listing(&scratch.store()), before);
}
