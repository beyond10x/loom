//! A host chooses how a case completes when several declared outcomes are legitimate at once
//! (story `first-declared-completion`, `loom.governor.CompletionRule` in
//! `ess/domains/governor.yaml`): `ExactlyOne`, the default, leaves the case open; `FirstDeclared`
//! completes on the first legitimate outcome in the protocol's declaration order, never the order
//! of the decision document's map, and never on an outcome Canon holds blocked.

use std::collections::BTreeMap;

use b10x_loom_commission::model::responsibility::{CaseId, CompletionDetermination};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, CaseInputs, CompletionRule, MemoryCaseStore};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};

const NAME: &str = "two-outcomes@1";

/// `b_first` is declared before `a_second`, so declaration order differs from name order (the
/// order of the decision document's map). Each is legitimate once its own decision is recorded.
const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: two.outcomes, revision: 1}
artifacts: {question: {}}
evidence_kinds: {finding: {}}
claims:
  answered:
    true_when:
      evidence: {kind: finding, result: answered, subject: question}
actions:
  investigate: {effect: read}
outcomes:
  b_first:
    requires: {decision: take_b}
  a_second:
    requires: {decision: take_a}
"#;

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(
            NAME,
            PROTOCOL,
            ProtocolSource {
                kind: SourceKind::Memory,
                location: "completion-rule-fixture".into(),
                revision: String::new(),
                path: String::new(),
            },
        )
        .expect("the protocol is admitted");
    catalog
}

/// A governor admitting the protocol through the host catalog.
fn catalog_governor() -> CanonGovernor<MemoryCaseStore> {
    CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog())
        .expect("the catalog is admitted")
}

/// A governor admitting the protocol as a host-registered definition.
fn registered_governor() -> CanonGovernor<MemoryCaseStore> {
    CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(NAME, PROTOCOL)
        .expect("the protocol is registered")
}

fn open(governor: &CanonGovernor<MemoryCaseStore>) -> CaseId {
    governor
        .open(
            NAME,
            BTreeMap::from([("question".into(), "q1".into())]),
            CaseInputs::none(),
        )
        .expect("the case opens")
}

fn decide(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, outcome: &str, name: &str) {
    governor
        .decide(case, outcome, name, "lead-investigator")
        .expect("the decision is recorded");
}

fn completion(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> CompletionDetermination {
    governor.completion(case).expect("the governor decides")
}

fn completed_with(determination: &CompletionDetermination) -> Option<&str> {
    match determination {
        CompletionDetermination::Complete(complete) => Some(complete.outcome.as_str()),
        CompletionDetermination::Open(_) => None,
    }
}

#[test]
fn first_declared_completes_on_the_first_declared_legitimate_outcome() {
    for (admission, build) in [
        ("catalog", catalog_governor as fn() -> _),
        ("registered", registered_governor),
    ] {
        // Under the default, two legitimate outcomes leave the case open.
        let exactly_one = build();
        let case = open(&exactly_one);
        decide(&exactly_one, &case, "b_first", "take_b");
        decide(&exactly_one, &case, "a_second", "take_a");
        let open_case = completion(&exactly_one, &case);
        assert_eq!(
            completed_with(&open_case),
            None,
            "{admission}: {open_case:?}"
        );

        // Chosen explicitly, `ExactlyOne` is the default.
        let explicit = build().with_completion_rule(CompletionRule::ExactlyOne);
        let case = open(&explicit);
        decide(&explicit, &case, "a_second", "take_a");
        decide(&explicit, &case, "b_first", "take_b");
        let open_case = completion(&explicit, &case);
        assert_eq!(
            completed_with(&open_case),
            None,
            "{admission}: {open_case:?}"
        );

        // Under `FirstDeclared` the same case completes `b_first`, declared first, whichever
        // decision was recorded first and although `a_second` sorts first by name.
        for order in [
            [("b_first", "take_b"), ("a_second", "take_a")],
            [("a_second", "take_a"), ("b_first", "take_b")],
        ] {
            let first_declared = build().with_completion_rule(CompletionRule::FirstDeclared);
            let case = open(&first_declared);
            for (outcome, name) in order {
                decide(&first_declared, &case, outcome, name);
            }
            let complete = completion(&first_declared, &case);
            assert_eq!(
                completed_with(&complete),
                Some("b_first"),
                "{admission}, decided {order:?}: {complete:?}"
            );
        }
    }
}

#[test]
fn a_blocked_outcome_is_never_completed_under_first_declared() {
    for (admission, build) in [
        ("catalog", catalog_governor as fn() -> _),
        ("registered", registered_governor),
    ] {
        let governor = build().with_completion_rule(CompletionRule::FirstDeclared);
        let case = open(&governor);

        // Nothing is legitimate: both outcomes are blocked and the case stays open.
        let nothing = completion(&governor, &case);
        assert_eq!(completed_with(&nothing), None, "{admission}: {nothing:?}");

        // `b_first`, declared first, is still blocked; only `a_second` is legitimate, and the case
        // completes on it, never on the blocked one.
        decide(&governor, &case, "a_second", "take_a");
        let complete = completion(&governor, &case);
        assert_eq!(
            completed_with(&complete),
            Some("a_second"),
            "{admission}: {complete:?}"
        );

        // Once the case moves, both decisions were taken at an older revision: both outcomes are
        // blocked again and nothing completes.
        decide(&governor, &case, "b_first", "take_b");
        governor
            .update_revision(&case, "question", "q2")
            .expect("the revision moves");
        let reopened = completion(&governor, &case);
        assert_eq!(completed_with(&reopened), None, "{admission}: {reopened:?}");
    }
}

#[test]
fn with_one_legitimate_outcome_both_rules_agree() {
    for rule in [CompletionRule::ExactlyOne, CompletionRule::FirstDeclared] {
        let governor = registered_governor().with_completion_rule(rule);
        let case = open(&governor);
        decide(&governor, &case, "b_first", "take_b");
        let complete = completion(&governor, &case);
        assert_eq!(
            completed_with(&complete),
            Some("b_first"),
            "{rule:?}: {complete:?}"
        );
    }
}
