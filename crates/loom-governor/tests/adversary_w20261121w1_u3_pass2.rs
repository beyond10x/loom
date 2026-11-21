//! Adversary pass 2, wave 2026-11-21-w1, unit 3 (`story:deterministic-default-selector`), the
//! governor half: `counted_kinds` re-implements three Canon rules (revision binding,
//! freshness, "read by a match"). Each case here compares it with the governor's own Canon
//! decision in the same test, never with a reading of Canon. No model or network call is made.

use std::collections::{BTreeMap, BTreeSet};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, EvidenceData, EvidenceId, GovernorError, Observation, ObservationData,
    ObservationId, Truth,
};
use b10x_loom_commission::ports::evidence::{EvidencePort, ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::selection::SelectionContext;
use b10x_loom_executor::{ActionSelector, CaseFacts, DeterministicSelector, StepHistory};
use loom_governor::{CanonGovernor, CaseInputs, EvaluationTime, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;
use std::sync::Mutex;

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

/// Submits one `canon-evidence/1` record: `members` are the JSON members after `subject`.
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

/// A fixed trusted time, or none.
struct At(Option<String>);

impl EvaluationTime for At {
    fn at(&self, _case: &CaseId) -> Result<Option<String>, GovernorError> {
        Ok(self.0.clone())
    }
}

/// One claim reads `probed` about `implementation`, with no result; an invalidation rule on `spec`
/// names that claim. The kind has a `max_age` of 60 seconds.
const READ_BY_A_CLAIM: &str = r#"
format: protocol/1
protocol: {id: adversary.pass2.grid, revision: 1}
artifacts: {implementation: {}, docs: {}, spec: {}}
evidence_kinds:
  probed: {max_age: 60s}
claims:
  probed:
    true_when:
      evidence: {kind: probed, subject: implementation}
actions:
  probe:
    effect: read
    may_produce:
      - evidence: probed
invalidation:
  spec.moved:
    upstream: spec
    invalidates: [probed]
"#;

/// The three rules against Canon's own decision, over one record: the evaluation instant (none,
/// the observation second, exactly `max_age` later, one second past it, a fraction past it, before
/// the observation), with and without `observed_at`, a current or stale revision of the read
/// subject or a record about another artifact, and no, a current or a moved upstream revision.
/// For a kind one claim reads with no result, Canon counts the record exactly when the claim is
/// true; the kind leaves open evidence exactly then.
#[test]
fn adversary_p2_counted_kinds_agrees_with_canon_over_one_record() {
    let instants = [
        None,
        Some("2026-10-06T10:00:00Z"),
        Some("2026-10-06T10:01:00Z"),
        Some("2026-10-06T10:01:00.999Z"),
        Some("2026-10-06T10:01:01Z"),
        Some("2026-10-06T09:59:55Z"),
    ];
    let observed = [None, Some("2026-10-06T10:00:00Z")];
    let subjects = [
        ("implementation", "r1"),
        ("implementation", "r0"),
        ("docs", "d1"),
    ];
    let upstreams = [None, Some("s1"), Some("s0")];
    let mut disagreements = Vec::new();
    let mut n = 0;
    for at in instants {
        for observed_at in observed {
            for (subject, revision) in subjects {
                for upstream in upstreams {
                    n += 1;
                    let governor = CanonGovernor::new(MemoryCaseStore::default())
                        .with_protocol_yaml("adversary-pass2-grid@1", READ_BY_A_CLAIM)
                        .unwrap()
                        .with_evaluation_time(At(at.map(str::to_owned)));
                    let revisions: BTreeMap<String, String> = [
                        ("implementation".to_owned(), "r1".to_owned()),
                        ("docs".to_owned(), "d1".to_owned()),
                        ("spec".to_owned(), "s1".to_owned()),
                    ]
                    .into();
                    let case = governor
                        .open("adversary-pass2-grid@1", revisions, CaseInputs::none())
                        .unwrap();
                    let mut members = format!(r#""subject_revision":"{revision}""#);
                    if let Some(observed_at) = observed_at {
                        members.push_str(&format!(r#","observed_at":"{observed_at}""#));
                    }
                    if let Some(upstream) = upstream {
                        members
                            .push_str(&format!(r#","upstream_revisions":{{"spec":"{upstream}"}}"#));
                    }
                    submit(&governor, &case, n, "probed", subject, &members);
                    let canon = claim(&governor, &case, "probed") == Truth::True;
                    let loom = open_of(&governor.action_facts(&case).unwrap(), "probe").is_empty();
                    if canon != loom {
                        disagreements.push(format!(
                            "at {at:?}, observed_at {observed_at:?}, {subject}@{revision}, upstream {upstream:?}: Canon counts {canon}, Loom counts {loom}"
                        ));
                    }
                }
            }
        }
    }
    assert!(disagreements.is_empty(), "{disagreements:#?}");
}

/// A kind only an order form reads (`first`). Canon's order forms read every record the evidence
/// check accepted, a record bound to an earlier revision included, so the claim stays true after
/// the subject moves. `counted_kinds` treats the kind as read by no match and applies revision
/// binding, so the kind is open again and the selector takes the action that produced it again,
/// once per revision of its subject.
#[test]
fn adversary_p2_a_kind_only_an_order_form_reads_counts_as_canon_counts_it() {
    const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: adversary.pass2.order, revision: 1}
artifacts: {implementation: {}}
evidence_kinds:
  triage: {}
claims:
  triaged:
    true_when:
      first: {kind: triage, result: done, subject: implementation}
actions:
  triage:
    effect: read
    may_produce:
      - evidence: triage
"#;
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml("adversary-pass2-order@1", PROTOCOL)
        .unwrap();
    let case = governor
        .open(
            "adversary-pass2-order@1",
            [("implementation".to_owned(), "r1".to_owned())].into(),
            CaseInputs::none(),
        )
        .unwrap();
    submit(
        &governor,
        &case,
        1,
        "triage",
        "implementation",
        r#""subject_revision":"r1","result":"done","observed_at":"2026-10-06T10:00:00Z""#,
    );
    assert_eq!(claim(&governor, &case, "triaged"), Truth::True);
    assert!(open_of(&governor.action_facts(&case).unwrap(), "triage").is_empty());
    governor
        .update_revision(&case, "implementation", "r2")
        .unwrap();
    assert_eq!(
        claim(&governor, &case, "triaged"),
        Truth::True,
        "the first triage record decides the claim on every revision"
    );
    let facts = governor.action_facts(&case).unwrap();
    assert!(
        open_of(&facts, "triage").is_empty(),
        "Canon still counts the r1 record for the order form: {facts:?}"
    );
}

/// What the run has done: the actions performed at each case revision.
#[derive(Default)]
struct Performed(Mutex<BTreeMap<i64, BTreeSet<String>>>);

impl StepHistory for Performed {
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .get(&case_revision)
            .is_some_and(|done| done.contains(action)))
    }

    fn due(&self, _action: &str) -> Result<bool, String> {
        Ok(false)
    }
}

/// Loom's bundled `software-change@1`, a feature change: `tests.run` may produce `test_result`
/// and `regression_test_result`, and a run without a regression test command submits only the
/// `test_result` (`crates/loom-cli/tests/slice_run.rs:1442-1444`). Driven step by step with the
/// deterministic selector over the governor, the run inspects, runs the tests (which pass), and
/// then takes `tests.run` again at every step: `regression_test_result` never arrives, so it is
/// always open, and a read sorts before every write. The SDC driver's loop the story names.
#[test]
fn adversary_p2_a_feature_change_on_software_change_does_not_run_the_tests_forever() {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&ProtocolCatalog::bundled().expect("the bundled catalog"))
        .expect("the catalog is admitted");
    let revisions: BTreeMap<String, String> = [
        "intent",
        "system_specification",
        "plan",
        "implementation",
        "release",
        "deployment",
    ]
    .into_iter()
    .map(|artifact| (artifact.to_owned(), "r1".to_owned()))
    .collect();
    let inputs = CaseInputs::none()
        .with("risk", "standard")
        .unwrap()
        .with("change_kind", "feature")
        .unwrap()
        .with("affects_security_boundary", "false")
        .unwrap();
    let case = governor
        .open("software-change@1", revisions, inputs)
        .unwrap();
    let history = Performed::default();
    let selector = DeterministicSelector::new(case.clone(), &governor, &history);
    let context = SelectionContext {
        prompt: "Deliver the change.".to_owned(),
    };
    let mut taken = Vec::new();
    for step in 0..5u64 {
        let frontier = governor.frontier(&case).unwrap().into_data();
        let candidates: Vec<CatalogueEntry> = frontier
            .actions
            .iter()
            .filter(|action| action.status == ActionStatus::Admissible)
            .map(|action| CatalogueEntry {
                action: action.action.clone(),
                status: CatalogueEntryStatus::Admissible,
            })
            .collect();
        let Ok(choice) = selector.select(&context, &candidates) else {
            break;
        };
        history
            .0
            .lock()
            .unwrap()
            .entry(frontier.case_revision)
            .or_default()
            .insert(choice.action.clone());
        if choice.action == "tests.run" {
            submit(
                &governor,
                &case,
                10 + step,
                "test_result",
                "implementation",
                r#""subject_revision":"r1","result":"pass""#,
            );
        }
        taken.push(choice.action);
    }
    assert_eq!(claim(&governor, &case, "tests.pass"), Truth::True);
    let runs = taken.iter().filter(|action| *action == "tests.run").count();
    assert!(
        runs <= 1,
        "tests.run was taken {runs} times with its test_result counted: {taken:?}"
    );
}
