//! An evaluation through Loom carries explicit decisions (`canon-decisions/1`), so an outcome that
//! requires a decision can become legitimate: through `loom_governor::evaluate`, whose request
//! carries them, and through `CanonGovernor`, which records them on the held case
//! (story `evaluation-explicit-decisions`, `ess/domains/evaluation.yaml`).

use std::collections::BTreeMap;

use b10x_loom_commission::model::responsibility::{CaseId, CompletionDetermination};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationInput, EvaluationRequest, ExplicitDecision, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{CanonGovernor, CaseInputs, DecisionError, MemoryCaseStore, evaluate};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};

const NAME: &str = "budgeted-investigation@1";

/// `inconclusive` requires the explicit decision `explicitly_inconclusive`; `answered` requires a
/// finding no test here supplies.
const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: budgeted.investigation, revision: 1}
artifacts: {question: {}}
evidence_kinds: {finding: {}}
claims:
  answered:
    true_when:
      evidence: {kind: finding, result: answered, subject: question}
actions:
  investigate: {effect: read}
outcomes:
  answered:
    requires: {claim: answered}
  inconclusive:
    requires: {decision: explicitly_inconclusive}
"#;

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(
            NAME,
            PROTOCOL,
            ProtocolSource {
                kind: SourceKind::Memory,
                location: "explicit-decisions-fixture".into(),
                revision: String::new(),
                path: String::new(),
            },
        )
        .expect("the protocol is admitted");
    catalog
}

fn decision(case_revision: &str) -> ExplicitDecision {
    ExplicitDecision {
        decision: "explicitly_inconclusive".into(),
        outcome: "inconclusive".into(),
        principal: "lead-investigator".into(),
        case_revision: case_revision.into(),
    }
}

/// A `canon-case/1` snapshot at case revision `c2`, terminated `inconclusive`.
fn terminated() -> CaseSnapshot {
    CaseSnapshot(
        json::parse(
            r#"{"format":"canon-case/1","id":"inv-7","protocol":"budgeted.investigation","revision":"c2","artifacts":{"question":{"revision":"q1"}},"termination":"inconclusive"}"#,
        )
        .expect("the snapshot is JSON"),
    )
}

fn request(decisions: Vec<ExplicitDecision>) -> EvaluationRequest {
    EvaluationRequest {
        protocol: ProtocolName(NAME.into()),
        snapshot: terminated(),
        evidence: Vec::new(),
        at: None,
        decisions,
    }
}

fn is_complete_with(determination: &CompletionDetermination, outcome: &str) -> bool {
    matches!(determination, CompletionDetermination::Complete(complete) if complete.outcome == outcome)
}

#[test]
fn an_outcome_requiring_a_decision_is_legitimate_only_with_that_decision() {
    let catalog = catalog();

    // `evaluate`: terminating the case `inconclusive` is refused without the decision ...
    let refused = evaluate(&catalog, &request(Vec::new())).expect_err("no decision is given");
    assert_eq!(refused.input, EvaluationInput::Snapshot, "{refused:?}");
    assert_eq!(refused.code, "illegitimate-termination", "{refused:?}");

    // ... accepted with it, taken at the case's current revision by a principal ...
    let decided = evaluate(&catalog, &request(vec![decision("c2")])).expect("decided at c2");
    assert_eq!(decided.outcome.as_deref(), Some("inconclusive"));
    let mut canon = String::new();
    json::push_value(&mut canon, &decided.canon);
    assert!(
        canon.contains(r#""principals":["lead-investigator"]"#),
        "Canon records who decided: {canon}"
    );

    // ... and refused, as Canon refuses it, when the decision was taken at an older revision.
    let superseded =
        evaluate(&catalog, &request(vec![decision("c1")])).expect_err("superseded at c1");
    assert_eq!(
        superseded.input,
        EvaluationInput::Snapshot,
        "{superseded:?}"
    );
    assert_eq!(
        superseded.code, "illegitimate-termination",
        "{superseded:?}"
    );

    // A decision for an outcome that does not require it is refused naming the decisions.
    let mut unrequired = decision("c2");
    unrequired.outcome = "answered".into();
    let refused = evaluate(&catalog, &request(vec![unrequired])).expect_err("not required");
    assert_eq!(refused.input, EvaluationInput::Decisions, "{refused:?}");
    assert_eq!(refused.code, "undeclared-decision", "{refused:?}");

    // `CanonGovernor`: the held case completes `inconclusive` only once the decision is recorded
    // at its current revision.
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .expect("the catalog is admitted");
    let case: CaseId = governor
        .open(
            NAME,
            BTreeMap::from([("question".into(), "q1".into())]),
            CaseInputs::none(),
        )
        .expect("the case opens");
    assert!(matches!(
        governor.completion(&case).expect("decides"),
        CompletionDetermination::Open(_)
    ));
    governor
        .decide(
            &case,
            "inconclusive",
            "explicitly_inconclusive",
            "lead-investigator",
        )
        .expect("the decision is recorded");
    let completion = governor.completion(&case).expect("decides");
    assert!(
        is_complete_with(&completion, "inconclusive"),
        "{completion:?}"
    );
    // Recording the same decision again changes nothing.
    governor
        .decide(
            &case,
            "inconclusive",
            "explicitly_inconclusive",
            "lead-investigator",
        )
        .expect("the same decision again");
    assert!(is_complete_with(
        &governor.completion(&case).expect("decides"),
        "inconclusive"
    ));

    // A decision Canon refuses is refused and nothing is recorded.
    assert!(matches!(
        governor.decide(&case, "answered", "explicitly_inconclusive", "lead-investigator"),
        Err(DecisionError::Refused { ref code, .. }) if code == "undeclared-decision"
    ));
    assert!(matches!(
        governor.decide(
            &CaseId("case-99".into()),
            "inconclusive",
            "explicitly_inconclusive",
            "p"
        ),
        Err(DecisionError::UnknownCase)
    ));

    // Once the case moves, the decision was taken at an older revision and no longer applies.
    governor
        .update_revision(&case, "question", "q2")
        .expect("the revision moves");
    assert!(matches!(
        governor.completion(&case).expect("decides"),
        CompletionDetermination::Open(_)
    ));
}
