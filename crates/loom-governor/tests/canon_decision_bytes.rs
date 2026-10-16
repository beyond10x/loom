//! The governor decides what the pinned Canon release decides: for each fixture case, the
//! `canon-decision/1` document `loom_governor::evaluate` returns is the one `b10x_canon::eval`'s
//! `evaluate_with` renders for the same protocol, case snapshot, evidence and instant (story
//! `canon-current-release`).
//!
//! `tests/fixtures/chg-1842.fixture.yaml` and
//! `tests/fixtures/security-boundary-independent.fixture.yaml` are byte-for-byte copies of
//! `fixtures/software-change/` in beyond10x/engineering-protocols at tag `0.8.0`, the release this
//! crate depends on. The first is Loom's own protocol (`software.change/1`) over a full change; the
//! second exercises independence on `different_principal`, and both go through the protocol's
//! outcome decisions, which neither side is given explicitly.

use std::path::PathBuf;

use b10x_canon::eval::{Supplied, case_from_value, evaluate_with, evidence_from_value, render};
use b10x_canon::ir::compile;
use loom_governor::evaluate;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::model::primitives::Timestamp;
use loom_protocols::ProtocolCatalog;
use serde_json::Value;

const PROTOCOL: &str = "software-change@1";

#[test]
fn the_governor_decision_equals_the_pinned_canon_decision_bytes() {
    let catalog = ProtocolCatalog::engineering().expect("the engineering catalog");
    let ir = compile(&catalog.get(PROTOCOL).expect("software-change@1").model)
        .expect("software-change@1 compiles");

    let mut compared = 0;
    for name in ["chg-1842", "security-boundary-independent"] {
        let fixture = load(name);
        let at = fixture["at"].as_str().map(str::to_owned);
        let case = fixture["case"].clone();
        let mut records: Vec<Value> = Vec::new();
        for state in fixture["states"].as_array().expect("states") {
            for entry in state["add_evidence"].as_array().into_iter().flatten() {
                let mut record = entry["record"].clone();
                record["observed_at"] = entry["observed_at"].clone();
                records.push(record);
            }

            let governed = evaluate(&catalog, &request(&case, &records, at.as_deref()))
                .unwrap_or_else(|refusal| panic!("{name}: the governor refuses: {refusal:?}"));

            let yaml = |value: &Value| serde_yaml_ng::to_value(value).expect("JSON is YAML");
            let canon_case = case_from_value(&yaml(&case)).expect("case snapshot");
            let canon_records: Vec<_> = records
                .iter()
                .map(|record| evidence_from_value(&yaml(record)).expect("evidence record"))
                .collect();
            let decision = evaluate_with(
                &ir,
                &canon_case,
                &canon_records,
                Supplied {
                    at: at.as_deref(),
                    ..Supplied::default()
                },
            )
            .expect("Canon evaluates");
            let rendered = render(&decision);
            let canon = json::parse(&rendered).expect("Canon's decision is JSON");

            assert_eq!(
                governed.canon, canon,
                "{name}, state {}: the governor's decision differs from Canon's",
                state["id"]
            );
            assert!(
                rendered.ends_with('\n'),
                "Canon's rendering ends in a newline"
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "at least one case was compared");
}

fn load(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.fixture.yaml"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let fixture: Value = serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(fixture["format"], "els-fixture/1");
    assert_eq!(fixture["protocol"], "protocols/software-change/1.yaml");
    fixture
}

fn request(case: &Value, records: &[Value], at: Option<&str>) -> EvaluationRequest {
    let value = |value: &Value| json::parse(&value.to_string()).expect("JSON");
    EvaluationRequest {
        protocol: ProtocolName(PROTOCOL.into()),
        snapshot: CaseSnapshot(value(case)),
        evidence: records
            .iter()
            .map(|record| EvidenceRecord(value(record)))
            .collect(),
        at: at.map(|at| Timestamp(at.into())),
        decisions: Vec::new(),
    }
}
