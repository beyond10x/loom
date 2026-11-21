//! Adversary, wave 2026-11-21-w1, unit 3 (`story:deterministic-default-selector`), the governor
//! half: `CanonGovernor::action_facts` against the pinned Canon's own counting
//! (written on Canon 0.8.0; Canon 0.9.0 left these rules as they were).
//!
//! A kind leaves an action's open evidence only when Canon counts a record of it. These cases
//! build records Canon does not count for anything the protocol reads, and records Canon sets
//! aside as `not_independent` (which the implementation counts on purpose), and drive the
//! deterministic selector over the governor as its port. No model or network call is made.

use std::collections::BTreeMap;
use std::sync::Mutex;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    Truth,
};
use b10x_loom_commission::ports::evidence::{EvidencePort, ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::selection::SelectionContext;
use b10x_loom_executor::{ActionSelector, CaseFacts, DeterministicSelector, StepHistory};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn open_of<'f>(facts: &'f CaseFacts, action: &str) -> &'f [String] {
    &facts
        .actions
        .iter()
        .find(|entry| entry.action == action)
        .unwrap_or_else(|| panic!("the facts name {action}: {facts:?}"))
        .open_evidence
}

/// A history in which nothing was performed and nothing is due.
struct NoHistory;

impl StepHistory for NoHistory {
    fn performed_at(&self, _action: &str, _case_revision: i64) -> Result<bool, String> {
        Ok(false)
    }

    fn due(&self, _action: &str) -> Result<bool, String> {
        Ok(false)
    }
}

/// Submits one `canon-evidence/1` record with the given facts (a JSON object's members after
/// `format`), citing an observation of its own that the governor received first.
fn submit<G>(governor: &G, case: &CaseId, n: u64, kind: &str, subject: &str, members: &str)
where
    G: EvidencePort + ObservationPort,
{
    let observation = ObservationId(uuid(0x100 + n));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:test-runner".into(),
            subject: subject.into(),
            observed_at: Timestamp("2026-10-06T10:00:00Z".into()),
            payload: json::Value::Null,
        }),
    )
    .unwrap();
    let facts = json::parse(&format!(
        r#"{{"format":"canon-evidence/1","id":"{kind}-{n}","kind":"{kind}","subject":"{subject}",{members}}}"#
    ))
    .unwrap();
    submit_evidence(
        governor,
        "trusted:test-runner",
        EvidenceData {
            evidence_id: EvidenceId(uuid(n)),
            case_id: case.clone(),
            kind: kind.to_owned(),
            subject_revision: 1,
            producer: String::new(),
            facts,
            provenance: json::Value::Null,
            observation_ids: vec![observation],
        },
    )
    .unwrap();
}

/// The candidates a frontier offers: every non-blocked action, with its status.
fn candidates<S, T>(governor: &CanonGovernor<S, T>, case: &CaseId) -> Vec<CatalogueEntry>
where
    CanonGovernor<S, T>: Governor,
{
    governor
        .frontier(case)
        .expect("a frontier")
        .into_data()
        .actions
        .into_iter()
        .filter_map(|action| match action.status {
            ActionStatus::Admissible => Some(CatalogueEntry {
                action: action.action,
                status: CatalogueEntryStatus::Admissible,
            }),
            ActionStatus::ApprovalRequired => Some(CatalogueEntry {
                action: action.action,
                status: CatalogueEntryStatus::ApprovalRequired,
            }),
            ActionStatus::Blocked => None,
        })
        .collect()
}

fn claim<S, T>(governor: &CanonGovernor<S, T>, case: &CaseId, name: &str) -> Truth
where
    CanonGovernor<S, T>: Governor,
{
    governor
        .frontier(case)
        .expect("a frontier")
        .into_data()
        .claims
        .into_iter()
        .find(|entry| entry.claim == name)
        .unwrap_or_else(|| panic!("the frontier names claim {name}"))
        .value
}

/// Two artifacts; the claim reads `reviewed` about `implementation` only. The `review` action
/// produces `reviewed`; `merge` needs the claim.
const TWO_SUBJECTS: &str = r#"
format: protocol/1
protocol: {id: adversary.two.subjects, revision: 1}
artifacts: {implementation: {}, docs: {}}
evidence_kinds:
  reviewed: {}
claims:
  reviewed:
    true_when:
      evidence: {kind: reviewed, subject: implementation}
actions:
  review:
    effect: read
    may_produce:
      - evidence: reviewed
  merge:
    effect: write
    precondition: {claim: reviewed}
outcomes:
  accepted:
    requires: {claim: reviewed}
"#;

fn two_subjects() -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("adversary-two-subjects@1", TWO_SUBJECTS)
        .unwrap();
    let revisions: BTreeMap<String, String> = [
        ("implementation".to_owned(), "r1".to_owned()),
        ("docs".to_owned(), "d1".to_owned()),
    ]
    .into();
    let case = governor
        .open("adversary-two-subjects@1", revisions, CaseInputs::none())
        .unwrap();
    (governor, case)
}

/// The acceptance bullet's own words: after the subject's revision moves the kind is back in the
/// action's open evidence. A record of the same kind about another declared artifact, at that
/// artifact's current revision, is read by no claim, so Canon counts it for nothing; the
/// governor still takes the kind out of open evidence, and the selector then never runs the
/// review the claim needs.
#[test]
fn adversary_a_record_about_another_subject_does_not_close_the_kind_once_the_subject_moves() {
    let (governor, case) = two_subjects();
    submit(
        &governor,
        &case,
        1,
        "reviewed",
        "implementation",
        r#""subject_revision":"r1""#,
    );
    submit(
        &governor,
        &case,
        2,
        "reviewed",
        "docs",
        r#""subject_revision":"d1""#,
    );
    assert_eq!(claim(&governor, &case, "reviewed"), Truth::True);
    assert!(open_of(&governor.action_facts(&case).unwrap(), "review").is_empty());

    governor
        .update_revision(&case, "implementation", "r2")
        .unwrap();
    assert_eq!(
        claim(&governor, &case, "reviewed"),
        Truth::Unknown,
        "Canon counts no record for the claim once implementation moved"
    );
    let facts = governor.action_facts(&case).unwrap();
    assert_eq!(
        open_of(&facts, "review"),
        ["reviewed"],
        "the kind is back once its subject moved; the docs record is read by nothing: {facts:?}"
    );
}

/// The same defect seen from the selector: the only way forward is `review`, it is admissible,
/// and the selector over the governor's facts answers that nothing is admissible.
#[test]
fn adversary_the_selector_over_the_governor_runs_the_review_a_claim_still_needs() {
    let (governor, case) = two_subjects();
    submit(
        &governor,
        &case,
        2,
        "reviewed",
        "docs",
        r#""subject_revision":"d1""#,
    );
    assert_eq!(claim(&governor, &case, "reviewed"), Truth::Unknown);
    let offered = candidates(&governor, &case);
    assert_eq!(
        offered,
        [CatalogueEntry {
            action: "review".to_owned(),
            status: CatalogueEntryStatus::Admissible,
        }]
    );
    let selector = DeterministicSelector::new(case.clone(), &governor, NoHistory);
    let context = SelectionContext {
        prompt: "Take the next useful step.".to_owned(),
    };
    assert_eq!(
        selector
            .select(&context, &offered)
            .map(|choice| choice.action),
        Ok("review".to_owned()),
        "no record Canon counts for the claim exists, so review is still useful"
    );
}

/// No claim reads `probed`; the `ship` action's precondition and the outcome read it directly,
/// over the evidence Canon leaves after its exclusion stages. An expired record is not listed
/// under any claim, so the governor's fallback (subject revision only) counts it.
const DIRECT_READ: &str = r#"
format: protocol/1
protocol: {id: adversary.direct.read, revision: 1}
artifacts: {implementation: {}}
evidence_kinds:
  probed: {max_age: 60s}
actions:
  probe:
    effect: read
    may_produce:
      - evidence: probed
  ship:
    effect: write
    precondition:
      evidence: {kind: probed, subject: implementation}
outcomes:
  accepted:
    requires:
      evidence: {kind: probed, subject: implementation}
"#;

/// An expired record of a kind only a precondition and an outcome read: Canon does not count it
/// (`ship` is blocked again), the governor calls the kind present, and the selector stalls with
/// `probe` admissible.
#[test]
fn adversary_an_expired_record_of_a_kind_no_claim_reads_counts_for_nothing() {
    let now = Mutex::new(Some("2026-10-06T10:00:30Z".to_owned()));
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("adversary-direct-read@1", DIRECT_READ)
        .unwrap()
        .with_evaluation_time(|_: &CaseId| Ok(now.lock().unwrap().clone()));
    let case = governor
        .open(
            "adversary-direct-read@1",
            [("implementation".to_owned(), "r1".to_owned())].into(),
            CaseInputs::none(),
        )
        .unwrap();
    submit(
        &governor,
        &case,
        1,
        "probed",
        "implementation",
        r#""subject_revision":"r1","observed_at":"2026-10-06T10:00:00Z""#,
    );
    let fresh: Vec<String> = candidates(&governor, &case)
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    assert_eq!(fresh, ["probe", "ship"], "a fresh record admits ship");
    assert!(open_of(&governor.action_facts(&case).unwrap(), "probe").is_empty());

    *now.lock().unwrap() = Some("2026-10-06T10:02:00Z".into());
    let stale: Vec<String> = candidates(&governor, &case)
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    assert_eq!(
        stale,
        ["probe"],
        "Canon no longer counts the expired record"
    );
    let facts = governor.action_facts(&case).unwrap();
    assert_eq!(
        open_of(&facts, "probe"),
        ["probed"],
        "the governor must not count a record Canon set aside as expired: {facts:?}"
    );
}

/// The implementation's own rule, held: a record Canon sets aside only as `not_independent`
/// still counts. No existing test submits such a record, so dropping the `NotIndependent` filter
/// in `counted_kinds` would pass the suite; this case fails then.
#[test]
fn adversary_a_record_set_aside_only_as_not_independent_still_counts() {
    const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: adversary.independence, revision: 1}
independence_dimensions: {different_principal: {}}
artifacts: {implementation: {}}
evidence_kinds:
  reviewed: {}
claims:
  reviewed:
    true_when:
      evidence: {kind: reviewed, subject: implementation, independent_on: [different_principal]}
actions:
  review:
    effect: read
    may_produce:
      - evidence: reviewed
"#;
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("adversary-independence@1", PROTOCOL)
        .unwrap();
    let case = governor
        .open(
            "adversary-independence@1",
            [("implementation".to_owned(), "r1".to_owned())].into(),
            CaseInputs::none(),
        )
        .unwrap();
    submit(
        &governor,
        &case,
        1,
        "reviewed",
        "implementation",
        r#""subject_revision":"r1""#,
    );
    assert_eq!(
        claim(&governor, &case, "reviewed"),
        Truth::Unknown,
        "the record's producer is declared independent on nothing"
    );
    assert!(
        open_of(&governor.action_facts(&case).unwrap(), "review").is_empty(),
        "set aside only as not_independent, the record counts (the implementor's rule)"
    );
}
