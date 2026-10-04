//! Adversary cases for the Canon-backed governor (story `canon-governor`, adversary pass 2).
//!
//! Pass 1 (`adversary_governor.rs`) compared every frontier with Canon over records it wrote as
//! JSON. These cases attack what it left: the bridge from Commission's evidence facts to Canon's
//! record, the frontier id's inputs, a second `CaseStore`, observations, and artifact names that
//! only look like declared ones. Canon is the oracle wherever a status is expected.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use b10x_canon::eval::evaluate;
use b10x_canon::ir::{Ir, compile};
use b10x_canon::model::{
    ArtifactId, CASE_FORMAT, Case, CaseArtifact, Declarations, EVIDENCE_FORMAT, EvidenceKindId,
    EvidenceRecord, Revision,
};
use b10x_commission::model::json;
use b10x_commission::model::primitives::{Timestamp, Uuid};
use b10x_commission::model::responsibility::{
    CaseId, CompletionDetermination, EvidenceData, EvidenceId, FrontierData, GovernorError,
    Observation, ObservationData, ObservationId, Truth,
};
use b10x_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_commission::ports::governor::Governor as _;
use b10x_commission_testkit::kits::governor::{Check, GovernorFixture, run};
use governor::{CanonGovernor, CaseState, CaseStore, MemoryCaseStore, OpenError, UpdateError};

const SOFTWARE_CHANGE: &str = "software-change@1";

// ---------------------------------------------------------------------------------------------
// The bridge from Commission facts to a Canon record
// ---------------------------------------------------------------------------------------------

/// A `canon-evidence/1` record that Canon evaluates as given, carried in Commission's facts, has
/// the effect Canon gives it. Each row is one record about the case's current implementation
/// revision; the oracle is Canon over the same record built as a value. Rows whose revision or
/// result carries a character Commission's JSON writer leaves raw and YAML refuses raw (a
/// noncharacter, DEL, a C1 control) are where the governor's text round trip can lose the record.
#[test]
fn a_record_canon_evaluates_has_its_effect_through_the_governor() {
    let ir = ir_of("software-change", 1);
    let rows: [(&str, &str, &str); 4] = [
        (
            "non-ASCII revision",
            "R-e\u{301}-\u{4fee}\u{8ba2}-\u{1f680}",
            "pass",
        ),
        ("noncharacter in the revision", "R\u{ffff}", "pass"),
        ("DEL in the result", "R2", "pass\u{7f}"),
        ("C1 control in the result", "R2", "pass\u{9b}"),
    ];
    let mut divergent = Vec::new();
    for (label, revision, result) in rows {
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let mut revisions = software_change_revisions();
        revisions.insert("implementation".to_owned(), revision.to_owned());
        let case = governor
            .open(SOFTWARE_CHANGE, revisions.clone())
            .unwrap_or_else(|error| {
                panic!("{label}: Canon takes {revision:?} as a revision: {error}")
            });
        let facts = facts(
            "t-1",
            "test_result",
            Some(result),
            "implementation",
            revision,
        );
        submit(&governor, &case, 1, "test_result", facts).expect("taken");

        let record = canon_record(
            "t-1",
            "test_result",
            Some(result),
            "implementation",
            revision,
        );
        let expected = canon_claims(&ir, &case, &revisions, &[record]);
        let issued = claims(&governor.frontier(&case).expect("frontier").into_data());
        if issued != expected {
            divergent.push(format!(
                "{label} (revision {revision:?}, result {result:?}): governor {issued:?}, Canon {expected:?}"
            ));
        }
    }
    assert!(
        divergent.is_empty(),
        "records Canon evaluates lose their effect through the governor:\n{}",
        divergent.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// The frontier id's inputs
// ---------------------------------------------------------------------------------------------

/// The frontier, id included, depends on Canon's inputs only (Atlas ADR 0089 item 4): evidence that
/// does not apply leaves it as it was, and the same records delivered in another order, under other
/// Commission ids, producers, provenance and observations, give the same frontier and completion.
#[test]
fn the_frontier_id_follows_canons_inputs_only() {
    let records: Vec<(&str, &str, Option<&str>, &str, &str)> = vec![
        ("t-1", "test_result", Some("pass"), "implementation", "R2"),
        (
            "rev-1",
            "code_review",
            Some("approved"),
            "implementation",
            "R2",
        ),
        (
            "health-1",
            "operational_observation",
            Some("healthy"),
            "deployment",
            "d0",
        ),
        ("prov-1", "build_provenance", None, "release", "v0"),
        (
            "obj-1",
            "objective_observation",
            Some("satisfied"),
            "intent",
            "i1",
        ),
    ];

    let first = CanonGovernor::new(MemoryCaseStore::default());
    let a = first
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    let mut n = 0;
    for (id, kind, result, subject, revision) in &records {
        n += 1;
        submit(
            &first,
            &a,
            n,
            kind,
            facts(id, kind, *result, subject, revision),
        )
        .expect("taken");
        n += 1;
        let junk = json::Value::Object(vec![("tests.pass".to_owned(), json::Value::Bool(true))]);
        submit(&first, &a, n, "test_result", junk).expect("junk is taken");
    }

    let second = CanonGovernor::new(MemoryCaseStore::default());
    let b = second
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    for (k, (id, kind, result, subject, revision)) in records.iter().rev().enumerate() {
        let k = u64::try_from(k).expect("small");
        let evidence = EvidenceData {
            evidence_id: EvidenceId(uuid(0x500 + k)),
            case_id: b.clone(),
            kind: (*kind).to_owned(),
            subject_revision: 1,
            producer: String::new(),
            observation_ids: vec![
                ObservationId(uuid(0x600 + k)),
                ObservationId(uuid(0x700 + k)),
            ],
            facts: facts(id, kind, *result, subject, revision),
            provenance: json::Value::Object(vec![(
                "run".to_owned(),
                json::Value::Text(format!("run-{k}")),
            )]),
        };
        submit_evidence(&second, "verifier:elsewhere", evidence).expect("taken");
    }

    assert_eq!(a, b, "both governors open case-1");
    let issued_a = first.frontier(&a).expect("frontier").into_data();
    let issued_b = second.frontier(&b).expect("frontier").into_data();
    assert_eq!(
        issued_a, issued_b,
        "the same Canon records give the same frontier, id and all"
    );
    assert_eq!(
        first.completion(&a).expect("completion"),
        second.completion(&b).expect("completion")
    );

    // Evidence that does not apply, on the second governor: the frontier stays as it was.
    let junk = json::Value::Object(vec![("tests.pass".to_owned(), json::Value::Bool(true))]);
    submit(&second, &b, 0x900, "test_result", junk).expect("junk is taken");
    let duplicate = facts("t-1", "test_result", Some("fail"), "implementation", "R2");
    submit(&second, &b, 0x901, "test_result", duplicate).expect("a reused Canon id is taken");
    assert_eq!(
        second.frontier(&b).expect("frontier").into_data(),
        issued_b,
        "evidence that does not apply leaves the frontier, id included, as it was"
    );
}

// ---------------------------------------------------------------------------------------------
// A second CaseStore
// ---------------------------------------------------------------------------------------------

/// A `CaseStore` written from the trait's documentation alone: cases in a list, updated by copy and
/// write-back, shared between governors. Nothing of `MemoryCaseStore` is assumed.
#[derive(Clone, Default)]
struct SharedList(Rc<RefCell<List>>);

#[derive(Default)]
struct List {
    cases: Vec<CaseState>,
    observations: Vec<ObservationData>,
}

impl CaseStore for SharedList {
    fn insert(&self, state: CaseState) -> bool {
        let mut list = self.0.borrow_mut();
        if list.cases.iter().any(|held| held.id == state.id) {
            return false;
        }
        list.cases.push(state);
        true
    }

    fn get(&self, case: &CaseId) -> Option<CaseState> {
        self.0
            .borrow()
            .cases
            .iter()
            .find(|held| &held.id == case)
            .cloned()
    }

    fn update<T>(&self, case: &CaseId, change: impl FnOnce(&mut CaseState) -> T) -> Option<T> {
        let mut list = self.0.borrow_mut();
        let at = list.cases.iter().position(|held| &held.id == case)?;
        let mut copy = list.cases[at].clone();
        let answer = change(&mut copy);
        list.cases[at] = copy;
        Some(answer)
    }

    fn observe(&self, observation: ObservationData) {
        self.0.borrow_mut().observations.push(observation);
    }

    fn observations(&self) -> Vec<ObservationData> {
        self.0.borrow().observations.clone()
    }
}

/// Governors over one shared store act as one: a fresh governor opens the next free id, sees
/// evidence and revisions another recorded, and issues the same frontier (id included) and
/// completion; and a governor over the independent store answers exactly as one over
/// `MemoryCaseStore` fed the same inputs.
#[test]
fn governors_over_one_store_act_as_one() {
    let store = SharedList::default();
    let first = CanonGovernor::new(store.clone());
    let second = CanonGovernor::new(store.clone());
    let reference = CanonGovernor::new(MemoryCaseStore::default());

    let case = first
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    assert_eq!(case, CaseId("case-1".to_owned()));
    let other = second
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("a second governor over the same store opens the next free id");
    assert_eq!(other, CaseId("case-2".to_owned()));
    let mirrored = reference
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    assert_eq!(mirrored, case);

    let steps: Vec<(&str, Option<&str>, &str, &str)> = vec![
        ("test_result", Some("pass"), "implementation", "R2"),
        (
            "operational_observation",
            Some("healthy"),
            "deployment",
            "d0",
        ),
        ("build_provenance", None, "release", "v0"),
        ("objective_observation", Some("satisfied"), "intent", "i1"),
    ];
    for (n, (kind, result, subject, revision)) in steps.iter().enumerate() {
        let id = format!("e-{n}");
        let k = u64::try_from(n).expect("small");
        // Alternate the governor that takes the record.
        let taker = if n % 2 == 0 { &first } else { &second };
        submit(
            taker,
            &case,
            k,
            kind,
            facts(&id, kind, *result, subject, revision),
        )
        .expect("taken");
        submit(
            &reference,
            &case,
            k,
            kind,
            facts(&id, kind, *result, subject, revision),
        )
        .expect("taken");

        let fresh = CanonGovernor::new(store.clone());
        let expected = reference.frontier(&case).expect("frontier").into_data();
        for (who, governor) in [("first", &first), ("second", &second), ("fresh", &fresh)] {
            assert_eq!(
                governor.frontier(&case).expect("frontier").into_data(),
                expected,
                "step {n}: the {who} governor over the shared store"
            );
            assert_eq!(
                governor.completion(&case).expect("completion"),
                reference.completion(&case).expect("completion"),
                "step {n}: the {who} governor's completion"
            );
        }
    }
    assert!(
        matches!(
            first.completion(&case).expect("completion"),
            CompletionDetermination::Complete(_)
        ),
        "the steps make `accepted` legitimate"
    );

    let moved = second
        .update_revision(&case, "implementation", "R3")
        .expect("moves");
    reference
        .update_revision(&case, "implementation", "R3")
        .expect("moves");
    assert_eq!(first.current_revision(&case), Ok(moved));
    assert_eq!(
        first.frontier(&case).expect("frontier").into_data(),
        reference.frontier(&case).expect("frontier").into_data(),
        "after a revision another governor recorded"
    );
    assert_eq!(
        first.evidence(&case).expect("held").len(),
        steps.len(),
        "every record, whichever governor took it"
    );
    assert_eq!(first.current_revision(&other), Ok(1));
}

/// Commission's governor conformance kit, run on a governor over the independent store.
#[test]
fn the_kit_passes_over_an_independent_store() {
    #[derive(Default)]
    struct Fixture {
        held: RefCell<Option<CaseId>>,
    }

    impl GovernorFixture for Fixture {
        type Governor = CanonGovernor<SharedList>;

        fn hold(&self, case: &CaseId) -> Self::Governor {
            let governor = CanonGovernor::new(SharedList::default());
            governor
                .open_case(case.clone(), SOFTWARE_CHANGE, software_change_revisions())
                .expect("the held case opens");
            *self.held.borrow_mut() = Some(case.clone());
            governor
        }

        fn advance(&self, governor: &Self::Governor, case: &CaseId) {
            // A revision the case has never held: the case opens with implementation at `R2`,
            // which `R{next}` would repeat on the first move, and an update to the revision
            // already held changes nothing.
            let next = governor.current_revision(case).expect("held") + 1;
            governor
                .update_revision(case, "implementation", &format!("M{next}"))
                .expect("moves");
        }

        fn observations(&self, governor: &Self::Governor) -> Vec<ObservationData> {
            governor.observations()
        }

        fn evidence(&self, governor: &Self::Governor) -> Vec<EvidenceData> {
            let case = self.held.borrow().clone().expect("a held case");
            governor.evidence(&case).expect("held")
        }
    }

    let report = run(&Fixture::default());
    assert_eq!(report.ran(), Check::ALL);
    report.assert_passed();
}

// ---------------------------------------------------------------------------------------------
// Observations
// ---------------------------------------------------------------------------------------------

/// Observations and evidence delivered from several threads at once, for two cases: every
/// observation is kept once, each sender's in the order it sent them, and nothing else; evidence
/// never appears among the observations, and each case holds only its own evidence.
#[test]
fn observations_are_exactly_what_was_observed_never_evidence() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let a = governor
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    let b = governor
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    let senders: u64 = 4;
    let per: u64 = 10;
    std::thread::scope(|scope| {
        for sender in 0..senders {
            let governor = &governor;
            let case = if sender % 2 == 0 {
                a.clone()
            } else {
                b.clone()
            };
            scope.spawn(move || {
                for n in 0..per {
                    let observation = observation(sender, n, &case);
                    let named = observation.observation_id.clone();
                    governor
                        .observe(Observation::new(observation))
                        .expect("observed");
                    let id = format!("s{sender}-{n}");
                    let evidence = EvidenceData {
                        evidence_id: EvidenceId(uuid(0x1_0000 + sender * 0x100 + n)),
                        case_id: case.clone(),
                        kind: "operational_observation".to_owned(),
                        subject_revision: 1,
                        producer: String::new(),
                        observation_ids: vec![named],
                        facts: facts(
                            &id,
                            "operational_observation",
                            Some("healthy"),
                            "deployment",
                            "d0",
                        ),
                        provenance: json::Value::Null,
                    };
                    submit_evidence(governor, "verifier:sensor", evidence).expect("taken");
                }
            });
        }
    });

    let kept = governor.observations();
    let total = usize::try_from(senders * per).expect("small");
    assert_eq!(kept.len(), total, "every observation, and nothing else");
    for sender in 0..senders {
        let case = if sender % 2 == 0 { &a } else { &b };
        let sent: Vec<ObservationData> = (0..per).map(|n| observation(sender, n, case)).collect();
        let source = format!("executor:sender-{sender}");
        let received: Vec<ObservationData> = kept
            .iter()
            .filter(|observation| observation.source == source)
            .cloned()
            .collect();
        assert_eq!(received, sent, "sender {sender}'s observations, in order");
    }

    for (case, parity) in [(&a, 0), (&b, 1)] {
        let held = governor.evidence(case).expect("held");
        assert_eq!(held.len(), total / 2, "{case:?} holds its own evidence");
        let evidence_ids: BTreeSet<String> = held
            .iter()
            .map(|record| record.evidence_id.0.0.clone())
            .collect();
        for record in &held {
            assert_eq!(&record.case_id, case);
            let sender = (u64::from_str_radix(&record.evidence_id.0.0[24..], 16).expect("hex")
                - 0x1_0000)
                / 0x100;
            assert_eq!(
                sender % 2,
                parity,
                "{case:?} holds no other case's evidence"
            );
            assert!(
                kept.iter()
                    .all(|observation| observation.payload != record.facts),
                "evidence facts never appear as an observation payload"
            );
        }
        assert!(
            kept.iter()
                .all(|observation| !evidence_ids.contains(&observation.observation_id.0.0)),
            "no evidence id appears as an observation id"
        );
        let frontier = governor.frontier(case).expect("frontier").into_data();
        assert_eq!(claims(&frontier)["deployment.healthy"], "true");
    }
}

// ---------------------------------------------------------------------------------------------
// Artifact names
// ---------------------------------------------------------------------------------------------

/// An artifact name that only looks like a declared one (case, padding, a homoglyph, full-width
/// letters, a zero-width space, a combining mark) is not that artifact: `update_revision` refuses
/// it naming it as given and moves nothing, and `open` refuses a map that spells a declared
/// artifact that way, naming the declared artifact it misses.
#[test]
fn artifact_names_match_exactly() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(SOFTWARE_CHANGE, software_change_revisions())
        .expect("opens");
    let held = governor.revisions(&case).expect("held");
    let lookalikes = [
        "Implementation",
        "IMPLEMENTATION",
        "implementation ",
        " implementation",
        "implementat\u{456}on",
        "\u{ff49}\u{ff4d}\u{ff50}\u{ff4c}\u{ff45}\u{ff4d}\u{ff45}\u{ff4e}\u{ff54}\u{ff41}\u{ff54}\u{ff49}\u{ff4f}\u{ff4e}",
        "implementation\u{200b}",
        "implementation\u{301}",
    ];
    for name in lookalikes {
        assert_eq!(
            governor.update_revision(&case, name, "R9"),
            Err(UpdateError::UndeclaredArtifact {
                artifact: name.to_owned()
            }),
            "{name:?}"
        );
        let mut revisions = software_change_revisions();
        let revision = revisions.remove("implementation").expect("declared");
        revisions.insert(name.to_owned(), revision);
        assert_eq!(
            governor.open(SOFTWARE_CHANGE, revisions),
            Err(OpenError::MissingRevision {
                artifact: "implementation".to_owned()
            }),
            "{name:?}"
        );
    }
    assert_eq!(governor.current_revision(&case), Ok(1));
    assert_eq!(governor.revisions(&case).expect("held"), held);
    assert_eq!(
        governor.current_revision(&CaseId("case-2".to_owned())),
        Err(GovernorError::UnknownCase),
        "no refused open holds a case"
    );
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn ir_of(name: &str, major: u32) -> Ir {
    let builtin = b10x_els::registry::get(name, major).expect("ELS holds it");
    compile(&builtin.model).expect("compiles")
}

fn software_change_revisions() -> BTreeMap<String, String> {
    [
        ("intent", "i1"),
        ("system_specification", "s1"),
        ("plan", "p1"),
        ("implementation", "R2"),
        ("release", "v0"),
        ("deployment", "d0"),
    ]
    .into_iter()
    .map(|(artifact, revision)| (artifact.to_owned(), revision.to_owned()))
    .collect()
}

/// A `canon-evidence/1` record as Commission facts.
fn facts(id: &str, kind: &str, result: Option<&str>, subject: &str, revision: &str) -> json::Value {
    let text = |value: &str| json::Value::Text(value.to_owned());
    let mut members = vec![
        ("format".to_owned(), text(EVIDENCE_FORMAT)),
        ("id".to_owned(), text(id)),
        ("kind".to_owned(), text(kind)),
        ("subject".to_owned(), text(subject)),
        ("subject_revision".to_owned(), text(revision)),
    ];
    if let Some(result) = result {
        members.push(("result".to_owned(), text(result)));
    }
    json::Value::Object(members)
}

/// The same record as a Canon value.
fn canon_record(
    id: &str,
    kind: &str,
    result: Option<&str>,
    subject: &str,
    revision: &str,
) -> EvidenceRecord {
    EvidenceRecord {
        format: EVIDENCE_FORMAT.to_owned(),
        id: b10x_canon::model::EvidenceId::new(id),
        kind: EvidenceKindId::new(kind),
        result: result.map(str::to_owned),
        subject: ArtifactId::new(subject),
        subject_revision: Revision::new(revision),
        observed_at: None,
        upstream_revisions: Declarations::default(),
    }
}

/// Every claim's value as Canon decides it over `records`, for the case at `revisions`.
fn canon_claims(
    ir: &Ir,
    case: &CaseId,
    revisions: &BTreeMap<String, String>,
    records: &[EvidenceRecord],
) -> BTreeMap<String, String> {
    let snapshot = Case {
        format: CASE_FORMAT.to_owned(),
        id: b10x_canon::model::CaseId::new(case.0.as_str()),
        protocol: ir.protocol.id.clone(),
        artifacts: Declarations::new(
            revisions
                .iter()
                .map(|(artifact, revision)| {
                    (
                        ArtifactId::new(artifact.as_str()),
                        CaseArtifact {
                            revision: Revision::new(revision.as_str()),
                        },
                    )
                })
                .collect(),
        ),
        termination: None,
        revision: None,
    };
    let decision = evaluate(ir, &snapshot, records).expect("Canon evaluates the record");
    decision
        .claims
        .iter()
        .map(|(claim, entry)| (claim.as_str().to_owned(), entry.value.to_string()))
        .collect()
}

fn claims(frontier: &FrontierData) -> BTreeMap<String, String> {
    frontier
        .claims
        .iter()
        .map(|claim| {
            let value = match claim.value {
                Truth::True => "true",
                Truth::False => "false",
                Truth::Unknown => "unknown",
            };
            (claim.claim.clone(), value.to_owned())
        })
        .collect()
}

/// Submits `facts` of `kind` for `case` through Commission, under evidence id `n`.
fn submit<S: CaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    n: u64,
    kind: &str,
    facts: json::Value,
) -> Result<(), b10x_commission::ports::evidence::EvidenceError> {
    let evidence = EvidenceData {
        evidence_id: EvidenceId(uuid(n)),
        case_id: case.clone(),
        kind: kind.to_owned(),
        subject_revision: governor.current_revision(case).expect("held"),
        producer: String::new(),
        observation_ids: vec![ObservationId(uuid(0x8000_0000 + n))],
        facts,
        provenance: json::Value::Null,
    };
    submit_evidence(governor, "service:ci", evidence)
}

/// Observation `n` of `sender`, about `case`.
fn observation(sender: u64, n: u64, case: &CaseId) -> ObservationData {
    ObservationData {
        observation_id: ObservationId(uuid(0x2_0000 + sender * 0x100 + n)),
        source: format!("executor:sender-{sender}"),
        subject: case.0.clone(),
        observed_at: Timestamp(format!("2026-10-04T00:{sender:02}:{n:02}Z")),
        payload: json::Value::Object(vec![(
            "step".to_owned(),
            json::Value::Number(n.to_string()),
        )]),
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}
