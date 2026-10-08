//! Adversarial cases for `loom_governor::evaluate`, second pass (story `governor-evaluate`): the
//! per-record set-aside path. A termination whose legitimacy needs several records together beside
//! a record that is set aside; set-aside records interleaved with applying, stale and reordered
//! records against `CanonGovernor` for a bundled protocol with several claims; and the cost of the
//! set-aside path at 2000 records against deciding them.

use std::collections::BTreeMap;
use std::time::Instant;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus as FrontierStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId,
    ObservationId, Truth,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    ActionStatus, CaseSnapshot, ClaimValue, EvaluationDecision, EvaluationInput, EvaluationRequest,
    EvidenceRecord, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::{CanonGovernor, MemoryCaseStore, evaluate};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

fn change_revisions() -> BTreeMap<String, String> {
    [
        ("intent", "i1"),
        ("system_specification", "s1"),
        ("plan", "p1"),
        ("implementation", "R2"),
        ("release", "v0"),
        ("deployment", "d0"),
    ]
    .into_iter()
    .map(|(a, r)| (a.to_owned(), r.to_owned()))
    .collect()
}

fn record(id: &str, kind: &str, result: &str, subject: &str, revision: &str) -> Value {
    json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "result": result,
        "subject": subject,
        "subject_revision": revision,
    })
}

/// The four records the `accepted` outcome of `software-change@1` needs together: tests pass on
/// the implementation (verified), build provenance on the release (proven, with verified), a
/// healthy deployment and a satisfied objective.
fn accepting_records() -> Vec<Value> {
    vec![
        record("tests-r2", "test_result", "pass", "implementation", "R2"),
        record("prov-v0", "build_provenance", "present", "release", "v0"),
        record(
            "health-d0",
            "operational_observation",
            "healthy",
            "deployment",
            "d0",
        ),
        record(
            "objective-i1",
            "objective_observation",
            "satisfied",
            "intent",
            "i1",
        ),
    ]
}

fn strays() -> Vec<Value> {
    let mut bad_instant = record(
        "stray-instant",
        "test_result",
        "pass",
        "implementation",
        "R2",
    );
    bad_instant["observed_at"] = json!("yesterday");
    vec![
        record(
            "stray-kind",
            "undeclared_kind",
            "pass",
            "implementation",
            "R2",
        ),
        record(
            "stray-subject",
            "test_result",
            "pass",
            "no_such_artifact",
            "R2",
        ),
        record(
            "stray-ident",
            "test_result",
            "pass",
            "implementation",
            "not an id",
        ),
        bad_instant,
    ]
}

fn snapshot(revisions: &BTreeMap<String, String>, termination: Option<&str>) -> Value {
    let artifacts: serde_json::Map<String, Value> = revisions
        .iter()
        .map(|(artifact, revision)| (artifact.clone(), json!({"revision": revision})))
        .collect();
    let mut case = json!({
        "format": "canon-case/1",
        "id": "case-1",
        "protocol": "software.change",
        "artifacts": artifacts,
        "revision": "r1",
    });
    if let Some(outcome) = termination {
        case["termination"] = json!(outcome);
    }
    case
}

fn request(protocol: &str, snapshot: &Value, records: &[Value]) -> EvaluationRequest {
    let value = |value: &Value| json::parse(&value.to_string()).expect("JSON");
    EvaluationRequest {
        protocol: ProtocolName(protocol.into()),
        snapshot: CaseSnapshot(value(snapshot)),
        evidence: records.iter().map(|r| EvidenceRecord(value(r))).collect(),
        at: None,
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn submit(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, n: u64, record: &Value) {
    let evidence = EvidenceData {
        evidence_id: EvidenceId(uuid(2 * n + 1)),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(uuid(2 * n + 2))],
        facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
        provenance: commission_json::Value::Object(Vec::new()),
    };
    submit_evidence(governor, "service:ci", evidence).expect("the governor takes the evidence");
}

fn reason_text(value: &json::Value) -> Value {
    let mut out = String::new();
    json::push_value(&mut out, value);
    serde_json::from_str(&out).expect("a reason is JSON")
}

type ActionRow = (String, ActionStatus, Option<String>, Vec<Value>);

fn assert_same_as_governor(
    label: &str,
    decision: &EvaluationDecision,
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
) {
    let frontier = governor.frontier(case).expect("the frontier").into_data();
    let expected: Vec<ActionRow> = frontier
        .actions
        .iter()
        .map(|a| {
            (
                a.action.clone(),
                match a.status {
                    FrontierStatus::Admissible => ActionStatus::Admissible,
                    FrontierStatus::ApprovalRequired => ActionStatus::ApprovalRequired,
                    FrontierStatus::Blocked => ActionStatus::Blocked,
                },
                a.capability.clone(),
                a.reasons
                    .iter()
                    .map(|r| serde_json::from_str(r).expect("reason JSON"))
                    .collect(),
            )
        })
        .collect();
    let actual: Vec<ActionRow> = decision
        .actions
        .iter()
        .map(|a| {
            (
                a.action.clone(),
                a.status,
                a.requires.first().cloned(),
                a.reasons.iter().map(reason_text).collect(),
            )
        })
        .collect();
    assert_eq!(actual, expected, "{label}: actions");
    let expected: Vec<(String, ClaimValue)> = frontier
        .claims
        .iter()
        .map(|c| {
            (
                c.claim.clone(),
                match c.value {
                    Truth::True => ClaimValue::True,
                    Truth::False => ClaimValue::False,
                    Truth::Unknown => ClaimValue::Unknown,
                },
            )
        })
        .collect();
    let actual: Vec<(String, ClaimValue)> = decision
        .claims
        .iter()
        .map(|c| (c.claim.clone(), c.value))
        .collect();
    assert_eq!(actual, expected, "{label}: claims");
    let expected: Vec<(String, bool)> = frontier
        .obligations
        .iter()
        .map(|o| (o.obligation.clone(), o.open))
        .collect();
    let actual: Vec<(String, bool)> = decision
        .obligations
        .iter()
        .map(|o| (o.obligation.clone(), o.open))
        .collect();
    assert_eq!(actual, expected, "{label}: obligations");
    let expected = match governor.completion(case).expect("the completion") {
        CompletionDetermination::Complete(complete) => Some(complete.outcome),
        CompletionDetermination::Open(_) => None,
    };
    assert_eq!(decision.outcome, expected, "{label}: completion");
}

/// A terminated snapshot whose termination is legitimate only with four records together, beside
/// each kind of record that is set aside, in every position. Each applying record alone does not
/// make the termination legitimate, so a set-aside filter that judged records against the
/// terminated snapshot (rather than the snapshot without its termination) would drop all four and
/// refuse the snapshot; this case is what tells the two apart.
#[test]
fn adversary2_a_termination_needing_several_records_survives_each_set_aside_record() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = change_revisions();
    let terminated = snapshot(&revisions, Some("accepted"));
    for single in accepting_records() {
        let refusal = evaluate(
            &catalog,
            &request(
                "software-change@1",
                &terminated,
                std::slice::from_ref(&single),
            ),
        )
        .expect_err("one record alone does not make `accepted` legitimate");
        assert_eq!(
            (refusal.input, refusal.code.as_str()),
            (EvaluationInput::Snapshot, "illegitimate-termination"),
            "{single}: {refusal:?}"
        );
    }
    for stray in strays() {
        for position in 0..=accepting_records().len() {
            let mut records = accepting_records();
            records.insert(position, stray.clone());
            let decision = evaluate(
                &catalog,
                &request("software-change@1", &terminated, &records),
            )
            .unwrap_or_else(|r| panic!("{stray} at {position}: refused {r:?}"));
            assert_eq!(
                decision.outcome.as_deref(),
                Some("accepted"),
                "{stray} at {position}"
            );
        }
    }
}

/// The governor receives applying, stale and set-aside records interleaved; `evaluate` is given
/// the same records in reverse and rotated orders. Every order reports what the governor reports,
/// reasons included, and the case is complete through `accepted` in both.
#[test]
fn adversary2_set_aside_and_reordered_records_report_what_the_governor_reports() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let revisions = change_revisions();
    let mut records = Vec::new();
    let strays = strays();
    for (n, applying) in accepting_records().into_iter().enumerate() {
        records.push(applying);
        records.push(strays[n].clone());
    }
    records.push(record(
        "tests-r1",
        "test_result",
        "fail",
        "implementation",
        "R1",
    ));
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .expect("catalog");
    let case = CaseId("case-1".into());
    governor
        .open_case(case.clone(), "software-change@1", revisions.clone())
        .expect("open");
    for (n, r) in records.iter().enumerate() {
        submit(&governor, &case, n as u64, r);
    }
    assert!(
        matches!(
            governor.completion(&case),
            Ok(CompletionDetermination::Complete(ref c)) if c.outcome == "accepted"
        ),
        "the governor completes the case"
    );
    let open = snapshot(&revisions, None);
    let mut orders = vec![records.clone()];
    let mut reversed = records.clone();
    reversed.reverse();
    orders.push(reversed);
    for k in 1..records.len() {
        let mut rotated = records.clone();
        rotated.rotate_left(k);
        orders.push(rotated);
    }
    for (n, order) in orders.iter().enumerate() {
        let decision = evaluate(&catalog, &request("software-change@1", &open, order))
            .unwrap_or_else(|r| panic!("order {n}: refused {r:?}"));
        assert_same_as_governor(&format!("order {n}"), &decision, &governor, &case);
    }
}

/// The set-aside path judges each record once against the snapshot: at 2000 records with one set
/// aside it costs a bounded multiple of deciding the 2000 records. A governor-style cumulative
/// filter (each record judged with every earlier applying record) is quadratic and breaks the
/// bound by orders of magnitude; nothing else in the suite measures this path.
#[test]
fn adversary2_the_set_aside_path_costs_a_bounded_multiple_of_deciding() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let n = 2000;
    let case = json!({
        "format": "canon-case/1", "id": "case-1", "protocol": "system.query",
        "artifacts": {"intent": {"revision": "query-1"}}, "revision": "r1",
    });
    let mut records: Vec<Value> = (0..n)
        .map(|i| {
            record(
                &format!("clock-{i}"),
                "system_time",
                "observed",
                "intent",
                "query-1",
            )
        })
        .collect();
    let decided = request("system-query@1", &case, &records);
    records.insert(
        n / 2,
        record("stray", "undeclared_kind", "observed", "intent", "query-1"),
    );
    let set_aside = request("system-query@1", &case, &records);
    let start = Instant::now();
    let fast = evaluate(&catalog, &decided).expect("decided");
    let decide = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let slow = evaluate(&catalog, &set_aside).expect("decided with one set aside");
    let filter = start.elapsed().as_secs_f64();
    eprintln!(
        "n={n}: decide {decide:.4}s, set aside {filter:.4}s, ratio {:.1}",
        filter / decide
    );
    assert_eq!(slow.outcome, fast.outcome);
    assert_eq!(
        slow.canon, fast.canon,
        "the set-aside record leaves no trace"
    );
    assert!(
        filter <= 40.0 * decide + 0.25,
        "setting one record aside among {n} took {filter:.3}s against {decide:.3}s to decide them"
    );
}
