//! Adversary cases for `story:first-declared-completion`: `CompletionRule::FirstDeclared` takes the
//! first legitimate outcome in the admitted definition's declaration order, per protocol, never a
//! blocked one; the rule is host configuration, so a case written to a `FileCaseStore` completes by
//! the rule of the governor reading it; a registered name cannot be admitted again with another
//! order; the built-in `software-change@1` completes under either rule as it did; and the
//! stateless `evaluate` keeps `ExactlyOne`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use b10x_loom_commission::model::responsibility::{CaseId, CompletionDetermination};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, ExplicitDecision, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{
    CanonGovernor, CaseInputs, CompletionRule, EvaluationTime, FallibleCaseStore, FileCaseStore,
    MemoryCaseStore, evaluate,
};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::{Value, json};

const NAME: &str = "three-outcomes@1";
const REORDERED: &str = "three-reordered@1";

/// Declaration order `z_first`, `m_second`, `a_third`: the reverse of name order.
const PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: three.outcomes, revision: 1}
artifacts: {question: {}}
evidence_kinds: {finding: {}}
claims:
  answered:
    true_when:
      evidence: {kind: finding, result: answered, subject: question}
actions:
  investigate: {effect: read}
outcomes:
  z_first:
    requires: {decision: take_z}
  m_second:
    requires: {decision: take_m}
  a_third:
    requires: {decision: take_a}
"#;

/// The same outcomes declared in name order, under another id.
const REORDERED_PROTOCOL: &str = r#"
format: protocol/1
protocol: {id: three.reordered, revision: 1}
artifacts: {question: {}}
evidence_kinds: {finding: {}}
claims:
  answered:
    true_when:
      evidence: {kind: finding, result: answered, subject: question}
actions:
  investigate: {effect: read}
outcomes:
  a_third:
    requires: {decision: take_a}
  m_second:
    requires: {decision: take_m}
  z_first:
    requires: {decision: take_z}
"#;

fn source() -> ProtocolSource {
    ProtocolSource {
        kind: SourceKind::Memory,
        location: "adversary-completion-fixture".into(),
        revision: String::new(),
        path: String::new(),
    }
}

fn catalog() -> ProtocolCatalog {
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(NAME, PROTOCOL, source())
        .expect("the protocol is admitted");
    catalog
        .add_yaml(REORDERED, REORDERED_PROTOCOL, source())
        .expect("the reordered protocol is admitted");
    catalog
}

fn registered<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store)
        .with_protocol_yaml(NAME, PROTOCOL)
        .expect("the protocol is registered")
        .with_protocol_yaml(REORDERED, REORDERED_PROTOCOL)
        .expect("the reordered protocol is registered")
}

fn open<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    protocol: &str,
) -> CaseId {
    governor
        .open(
            protocol,
            BTreeMap::from([("question".into(), "q1".into())]),
            CaseInputs::none(),
        )
        .expect("the case opens")
}

fn decide<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    outcomes: &[&str],
) {
    for outcome in outcomes {
        let decision = format!("take_{}", &outcome[..1]);
        governor
            .decide(case, outcome, &decision, "lead-investigator")
            .expect("the decision is recorded");
    }
}

fn completed_with<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
) -> Option<String> {
    match governor.completion(case).expect("the governor decides") {
        CompletionDetermination::Complete(complete) => Some(complete.outcome),
        CompletionDetermination::Open(_) => None,
    }
}

/// With three outcomes, `FirstDeclared` takes the first legitimate one in declaration order,
/// whichever of them are legitimate, for catalog and registered admission alike.
#[test]
fn adversary_first_declared_follows_declaration_order_over_three_outcomes() {
    let catalog = catalog();
    for (admission, build) in [
        (
            "catalog",
            Box::new(|| {
                CanonGovernor::new(MemoryCaseStore::default())
                    .with_catalog(&catalog)
                    .expect("the catalog is admitted")
            }) as Box<dyn Fn() -> CanonGovernor<MemoryCaseStore>>,
        ),
        (
            "registered",
            Box::new(|| registered(MemoryCaseStore::default())),
        ),
    ] {
        for (legitimate, expected) in [
            (&["a_third", "m_second"][..], "m_second"),
            (&["a_third", "z_first"][..], "z_first"),
            (&["a_third", "m_second", "z_first"][..], "z_first"),
            (&["a_third"][..], "a_third"),
        ] {
            let governor = build().with_completion_rule(CompletionRule::FirstDeclared);
            let case = open(&governor, NAME);
            decide(&governor, &case, legitimate);
            assert_eq!(
                completed_with(&governor, &case).as_deref(),
                Some(expected),
                "{admission}, legitimate {legitimate:?}"
            );
        }
    }
}

/// Each protocol completes in its own declaration order: the same decisions on a case of the
/// protocol declaring the outcomes in name order complete on `a_third`.
#[test]
fn adversary_first_declared_takes_each_protocols_own_order() {
    let governor =
        registered(MemoryCaseStore::default()).with_completion_rule(CompletionRule::FirstDeclared);
    let declared = open(&governor, NAME);
    let reordered = open(&governor, REORDERED);
    for case in [&declared, &reordered] {
        decide(&governor, case, &["a_third", "m_second", "z_first"]);
    }
    assert_eq!(
        completed_with(&governor, &declared).as_deref(),
        Some("z_first")
    );
    assert_eq!(
        completed_with(&governor, &reordered).as_deref(),
        Some("a_third")
    );
}

/// A name already registered cannot be registered again, so its declaration order cannot be
/// replaced by another definition's; a catalog-admitted governor takes no registration at all.
#[test]
fn adversary_a_registered_order_cannot_be_replaced() {
    let again = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(NAME, PROTOCOL)
        .expect("the protocol is registered")
        .with_protocol_yaml(NAME, REORDERED_PROTOCOL);
    assert!(again.is_err(), "a second registration under one name");

    let after_catalog = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog())
        .expect("the catalog is admitted")
        .with_protocol_yaml(NAME, REORDERED_PROTOCOL);
    assert!(after_catalog.is_err(), "a registration beside the catalog");
}

/// The rule is the reading governor's, never the case's: a case written under `FirstDeclared` to a
/// `FileCaseStore` stays open for a new governor over the same directory under the default, and
/// completes on the first declared legitimate outcome under `FirstDeclared`.
#[test]
fn adversary_a_stored_case_completes_by_the_rule_of_the_governor_reading_it() {
    let dir = store_dir("rule-is-the-readers");
    let case = {
        let writer = registered(FileCaseStore::open(&dir).expect("the store opens"))
            .with_completion_rule(CompletionRule::FirstDeclared);
        let case = open(&writer, NAME);
        decide(&writer, &case, &["a_third", "m_second"]);
        assert_eq!(completed_with(&writer, &case).as_deref(), Some("m_second"));
        case
    };

    let default = registered(FileCaseStore::open(&dir).expect("the store opens again"));
    assert_eq!(completed_with(&default, &case), None);

    let first_declared = registered(FileCaseStore::open(&dir).expect("the store opens again"))
        .with_completion_rule(CompletionRule::FirstDeclared);
    assert_eq!(
        completed_with(&first_declared, &case).as_deref(),
        Some("m_second")
    );
}

/// The built-in `software-change@1` decides the same under either rule, whether the governor admits
/// the bundled catalog or none: an untouched case stays open (no error) and a declined one
/// completes `declined`.
#[test]
fn adversary_the_built_in_protocol_completes_alike_under_both_rules() {
    let bundled = ProtocolCatalog::bundled().expect("the bundled catalog");
    for rule in [CompletionRule::ExactlyOne, CompletionRule::FirstDeclared] {
        for admission in ["bundled catalog", "built in"] {
            let governor = if admission == "built in" {
                CanonGovernor::new(MemoryCaseStore::default())
            } else {
                CanonGovernor::new(MemoryCaseStore::default())
                    .with_catalog(&bundled)
                    .expect("the catalog is admitted")
            }
            .with_completion_rule(rule);
            let case = governor
                .open("software-change@1", fixture_revisions(), fixture_inputs())
                .expect("the case opens");
            assert_eq!(
                completed_with(&governor, &case),
                None,
                "{rule:?}, {admission}"
            );
            governor
                .decide(&case, "declined", "explicitly_declined", "lead-reviewer")
                .expect("the decision is recorded");
            assert_eq!(
                completed_with(&governor, &case).as_deref(),
                Some("declined"),
                "{rule:?}, {admission}"
            );
        }
    }
}

/// `evaluate` takes no completion rule: two legitimate outcomes are no outcome, as `ExactlyOne`
/// decides, and one is that outcome.
#[test]
fn adversary_evaluate_keeps_exactly_one() {
    let catalog = catalog();
    for (decisions, expected) in [
        (&["a_third", "m_second"][..], None),
        (&["a_third", "m_second", "z_first"][..], None),
        (&["m_second"][..], Some("m_second")),
    ] {
        let decided = evaluate(
            &catalog,
            &EvaluationRequest {
                protocol: ProtocolName(NAME.into()),
                snapshot: CaseSnapshot(parse(&json!({
                    "format": "canon-case/1",
                    "id": "case-1",
                    "protocol": "three.outcomes",
                    "artifacts": {"question": {"revision": "q1"}},
                    "revision": "r1",
                }))),
                evidence: Vec::new(),
                at: None,
                decisions: decisions
                    .iter()
                    .map(|outcome| ExplicitDecision {
                        decision: format!("take_{}", &outcome[..1]),
                        outcome: (*outcome).into(),
                        principal: "lead-investigator".into(),
                        case_revision: "r1".into(),
                    })
                    .collect(),
            },
        )
        .expect("the snapshot is decided");
        assert_eq!(decided.outcome.as_deref(), expected, "{decisions:?}");
    }
}

fn parse(value: &Value) -> json::Value {
    json::parse(&value.to_string()).expect("JSON")
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("adversary-w20261021w7-completion")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("the old store is removed");
    }
    std::fs::create_dir_all(&dir).expect("the store directory");
    dir
}

/// `tests/fixtures/chg-1842.fixture.yaml`.
fn fixture() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    serde_yaml_ng::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("YAML")
}

fn fixture_revisions() -> BTreeMap<String, String> {
    fixture()["case"]["artifacts"]
        .as_object()
        .expect("artifacts")
        .iter()
        .map(|(artifact, entry)| {
            (
                artifact.clone(),
                entry["revision"].as_str().expect("revision").to_owned(),
            )
        })
        .collect()
}

fn fixture_inputs() -> CaseInputs {
    let inputs: Vec<(String, String)> = fixture()["case"]["inputs"]
        .as_object()
        .expect("inputs")
        .iter()
        .map(|(input, value)| {
            let value = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (input.clone(), value)
        })
        .collect();
    CaseInputs::try_from_pairs(inputs).expect("distinct inputs")
}
