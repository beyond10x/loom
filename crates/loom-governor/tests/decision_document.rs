//! `CanonGovernor::decision_document` returns the `canon-decision/1` document behind the frontier
//! and the completion the governor issues for a held case: the whole document Canon wrote,
//! explanation included, at the case's current revision, from the one evaluation those make and
//! never a second one (story `governor-exposes-canon-decision`).
//!
//! Two oracles hold it. The frontier id is derived from the case, its revision and Canon's
//! rendering of the decision, so a document whose rendering gives the id of the frontier issued at
//! the same revision is the one that frontier was issued from. `evaluate` is the oracle for the
//! bytes: for the same protocol, evidence, decisions and time it returns the same document.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp as ObservedAt, Uuid};
use b10x_loom_commission::model::responsibility::{
    CaseId, CompletionDetermination, EvidenceData, EvidenceId, GovernorError, Observation,
    ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort as _, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, EvidenceRecord, ExplicitDecision, ProtocolName,
};
use loom_governor::model::json;
use loom_governor::model::primitives::Timestamp;
use loom_governor::{
    CanonGovernor, CaseIndependence, CaseInputs, CaseState, CaseStore, EvaluationTime,
    FallibleCaseStore, MemoryCaseStore, evaluate,
};
use loom_protocols::ProtocolCatalog;
use serde_json::{Value, json};

const PROTOCOL: &str = "software-change@1";
const CASE: &str = "case-1";
const PRODUCER: &str = "service:ci";
/// The trusted evaluation time of `tests/fixtures/chg-1842.fixture.yaml`.
const AT: &str = "2026-10-04T12:00:00Z";

/// At every revision the case passes through (opened, after evidence, after an artifact moved),
/// the document's rendering gives the id of the frontier issued at that revision.
#[test]
fn the_document_is_the_one_the_frontier_id_hashes() {
    let governor = admitted(CanonGovernor::new(MemoryCaseStore::default()));
    let case = open(&governor);

    let mut seen: Vec<(i64, String)> = Vec::new();
    let mut check = |governor: &CanonGovernor<MemoryCaseStore>, state: &str| {
        let frontier = governor.frontier(&case).expect("a frontier").into_data();
        let document = governor
            .decision_document(&case)
            .unwrap_or_else(|error| panic!("{state}: no document: {error:?}"));
        assert_eq!(
            document.member("format"),
            Some(&json::Value::Text("canon-decision/1".into())),
            "{state}: the document is Canon's decision"
        );
        assert_eq!(
            document.member("case"),
            Some(&json::Value::Text(CASE.into())),
            "{state}: the document is the held case's"
        );
        assert!(
            document.member("explanation").is_some(),
            "{state}: the document carries Canon's explanation"
        );
        let rendered = canonical(&document);
        assert_eq!(
            frontier.frontier_id.0.0,
            frontier_id(CASE, frontier.case_revision, &rendered),
            "{state}: the document's rendering gives the id of the frontier issued at revision {}",
            frontier.case_revision
        );
        assert!(
            seen.iter().all(|(_, earlier)| *earlier != rendered),
            "{state}: the document moved with the case"
        );
        seen.push((frontier.case_revision, rendered));
    };

    check(&governor, "opened");
    for (n, record) in fixture_records().iter().enumerate() {
        submit(&governor, &case, n, record);
    }
    check(&governor, "after evidence");
    governor
        .update_revision(&case, "implementation", "R3")
        .expect("the implementation moves");
    check(&governor, "after the implementation moved");

    let revisions: Vec<i64> = seen.iter().map(|(revision, _)| *revision).collect();
    assert_eq!(revisions, [1, 1, 2], "the case revisions checked");
}

/// For the same protocol, evidence, explicit decisions and trusted time, the document is byte for
/// byte the `canon-decision/1` document `evaluate` returns.
#[test]
fn the_document_equals_what_evaluate_returns_for_the_same_snapshot() {
    let catalog = ProtocolCatalog::bundled().expect("the bundled catalog");
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&catalog)
        .expect("the catalog is admitted")
        .with_evaluation_time(trusted_time);
    let case = open(&governor);
    let records = fixture_records();
    for (n, record) in records.iter().enumerate() {
        submit(&governor, &case, n, record);
    }
    governor
        .decide(&case, "declined", "explicitly_declined", "lead-reviewer")
        .expect("the decision is recorded");

    let document = governor.decision_document(&case).expect("a document");
    let decided = evaluate(
        &catalog,
        &EvaluationRequest {
            protocol: ProtocolName(PROTOCOL.into()),
            snapshot: CaseSnapshot(parse(&snapshot())),
            evidence: records
                .iter()
                .map(|record| {
                    // The producer `submit_evidence` attributed each record to, as the governor
                    // gives it to Canon.
                    let mut record = record.clone();
                    record["producer"] = Value::from(PRODUCER);
                    EvidenceRecord(parse(&record))
                })
                .collect(),
            at: Some(Timestamp(AT.into())),
            decisions: vec![ExplicitDecision {
                decision: "explicitly_declined".into(),
                outcome: "declined".into(),
                principal: "lead-reviewer".into(),
                case_revision: "r1".into(),
            }],
        },
    )
    .expect("the snapshot is decided");

    assert_eq!(
        compact(&document),
        compact(&decided.canon),
        "the governor's document is the one `evaluate` returns"
    );
    assert_eq!(document, decided.canon);

    // The inputs reached the document: the decision made `declined` the completion, the time kept
    // the deployment observation fresh, and the record about a superseded revision is excluded.
    assert_eq!(decided.outcome.as_deref(), Some("declined"));
    assert!(
        matches!(
            governor.completion(&case).expect("a completion"),
            CompletionDetermination::Complete(complete) if complete.outcome == "declined"
        ),
        "the governor completes the case `declined`"
    );
    let text = compact(&document);
    for expected in [
        r#""principals":["lead-reviewer"]"#,
        r#""deployment.healthy":{"value":"true"}"#,
        r#""evidence":"tests-r1""#,
        r#""explanation":"#,
    ] {
        assert!(
            text.contains(expected),
            "the document holds {expected}: {text}"
        );
    }
}

/// A case whose held inputs or independence declarations do not satisfy its protocol is never
/// decided: `decision_document` refuses it `GovernorUnavailable`, as `frontier` refuses it. A case
/// the governor does not hold is `UnknownCase` in both.
#[test]
fn a_case_the_governor_cannot_decide_returns_no_document() {
    let rows: [(&str, Unusable); 5] = [
        ("no inputs", |state| state.inputs = CaseInputs::none()),
        ("a declared input missing", |state| {
            state.inputs = CaseInputs::try_from_pairs([
                ("change_kind", "refactor"),
                ("affects_security_boundary", "false"),
            ])
            .expect("distinct inputs");
        }),
        ("an undeclared input", |state| {
            state.inputs = state
                .inputs
                .clone()
                .with("priority", "high")
                .expect("a new input");
        }),
        ("an unlisted value", |state| {
            state.inputs = CaseInputs::try_from_pairs([
                ("risk", "standard"),
                ("change_kind", "hotfix"),
                ("affects_security_boundary", "false"),
            ])
            .expect("distinct inputs");
        }),
        ("an undeclared independence dimension", |state| {
            state.independence = CaseIndependence::none()
                .with("reviewer", ["different_team"])
                .expect("one dimension");
        }),
    ];

    for (label, unusable) in rows {
        let store = Arc::new(MemoryCaseStore::default());
        let governor = admitted(CanonGovernor::new(Shared(Arc::clone(&store))));
        let case = open(&governor);
        assert!(
            governor.decision_document(&case).is_ok(),
            "{label}: the case as opened is decided"
        );

        CaseStore::update(&*store, &case, unusable).expect("the case is held");

        assert_eq!(
            governor.frontier(&case).map(|_| ()),
            Err(GovernorError::GovernorUnavailable),
            "{label}: the frontier is refused"
        );
        assert_eq!(
            governor.decision_document(&case),
            Err(GovernorError::GovernorUnavailable),
            "{label}: the document is refused as the frontier is"
        );
    }

    let governor = admitted(CanonGovernor::new(MemoryCaseStore::default()));
    let unheld = CaseId("case-9".into());
    assert_eq!(
        governor.frontier(&unheld).map(|_| ()),
        Err(GovernorError::UnknownCase)
    );
    assert_eq!(
        governor.decision_document(&unheld),
        Err(GovernorError::UnknownCase)
    );
}

/// One call is one evaluation: the trusted time, asked once per evaluation, is asked once.
#[test]
fn one_call_asks_for_one_evaluation() {
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&asked);
    let governor = admitted(CanonGovernor::new(MemoryCaseStore::default())).with_evaluation_time(
        move |_: &CaseId| -> Result<Option<String>, GovernorError> {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Some(AT.to_owned()))
        },
    );
    let case = open(&governor);
    let before = asked.load(Ordering::SeqCst);
    governor.decision_document(&case).expect("a document");
    assert_eq!(
        asked.load(Ordering::SeqCst) - before,
        1,
        "one call evaluates the case once"
    );
}

/// Under a trusted time that moves at every evaluation, the document of the most recent
/// evaluation (a frontier's, a completion's) is read without another one while the case is held as
/// that evaluation found it; evidence received, a decision another governor recorded over the same
/// store and a new artifact revision each make the next read evaluate the case once again.
#[test]
fn a_kept_document_is_read_without_an_evaluation_until_the_case_moves() {
    let asked = Arc::new(AtomicUsize::new(0));
    let clock = Arc::clone(&asked);
    let store = Arc::new(MemoryCaseStore::default());
    let governor = admitted(CanonGovernor::new(Shared(Arc::clone(&store)))).with_evaluation_time(
        move |_: &CaseId| -> Result<Option<String>, GovernorError> {
            let second = clock.fetch_add(1, Ordering::SeqCst);
            Ok(Some(format!("2026-10-04T12:00:{second:02}Z")))
        },
    );
    let other = admitted(CanonGovernor::new(Shared(Arc::clone(&store))));
    let case = open(&governor);
    let evaluations = || asked.load(Ordering::SeqCst);

    let frontier = governor.frontier(&case).expect("a frontier").into_data();
    let before = evaluations();
    let issued = governor.decision_document(&case).expect("a document");
    assert_eq!(
        frontier_id(CASE, frontier.case_revision, &canonical(&issued)),
        frontier.frontier_id.0.0,
        "the document read after the frontier is the one its id hashes"
    );
    assert_eq!(
        governor.decision_document(&case),
        Ok(issued.clone()),
        "read again, the same document"
    );
    assert_eq!(evaluations(), before, "no read evaluated the case");

    governor.completion(&case).expect("a completion");
    let before = evaluations();
    let completed = governor.decision_document(&case).expect("a document");
    assert_eq!(
        evaluations(),
        before,
        "the completion's document is read as kept"
    );
    assert_ne!(
        completed, issued,
        "the completion's evaluation, at a later instant, is the most recent"
    );

    submit(&governor, &case, 0, &fixture_records()[2]);
    let before = evaluations();
    let received = governor.decision_document(&case).expect("a document");
    assert_eq!(
        evaluations() - before,
        1,
        "evidence received: evaluated once"
    );
    assert!(
        compact(&received).contains(r#""tests.pass":{"value":"true"}"#),
        "the document holds the evidence received: {}",
        compact(&received)
    );

    other
        .decide(&case, "declined", "explicitly_declined", "lead-reviewer")
        .expect("the other governor records the decision");
    let before = evaluations();
    let decided = governor.decision_document(&case).expect("a document");
    assert_eq!(
        evaluations() - before,
        1,
        "a decision recorded: evaluated once"
    );
    assert!(
        compact(&decided).contains(r#""principals":["lead-reviewer"]"#),
        "the document holds the decision the other governor recorded: {}",
        compact(&decided)
    );
    assert_eq!(
        governor.current_revision(&case),
        Ok(1),
        "neither moved the case revision"
    );

    governor
        .update_revision(&case, "implementation", "R3")
        .expect("the implementation moves");
    let before = evaluations();
    governor.decision_document(&case).expect("a document");
    assert_eq!(
        evaluations() - before,
        1,
        "an artifact moved: evaluated once"
    );
    let before = evaluations();
    governor.decision_document(&case).expect("a document");
    assert_eq!(evaluations(), before, "and kept again");
}

/// A change to a held case that leaves it unusable under its protocol.
type Unusable = fn(&mut CaseState);

/// A [`CaseStore`] shared with the test, so a held case can be changed behind the governor as a
/// durable store could hand it back.
struct Shared(Arc<MemoryCaseStore>);

impl CaseStore for Shared {
    fn insert(&self, state: CaseState) -> bool {
        CaseStore::insert(&*self.0, state)
    }

    fn get(&self, case: &CaseId) -> Option<CaseState> {
        CaseStore::get(&*self.0, case)
    }

    fn update<T>(&self, case: &CaseId, change: impl FnOnce(&mut CaseState) -> T) -> Option<T> {
        CaseStore::update(&*self.0, case, change)
    }

    fn observe(&self, observation: ObservationData) {
        CaseStore::observe(&*self.0, observation);
    }

    fn observations(&self) -> Vec<ObservationData> {
        CaseStore::observations(&*self.0)
    }
}

/// `governor` over the bundled catalog.
fn admitted<S: FallibleCaseStore>(governor: CanonGovernor<S>) -> CanonGovernor<S> {
    governor
        .with_catalog(&ProtocolCatalog::bundled().expect("the bundled catalog"))
        .expect("the catalog is admitted")
}

/// Opens `case-1` on `software-change@1` with the fixture's artifact revisions and case inputs.
fn open<S: FallibleCaseStore, T: EvaluationTime>(governor: &CanonGovernor<S, T>) -> CaseId {
    let case = CaseId(CASE.into());
    governor
        .open_case(
            case.clone(),
            PROTOCOL,
            fixture_revisions(),
            fixture_inputs(),
        )
        .expect("the case opens");
    case
}

/// Delivers the observation `record` cites and submits it through Commission's port, attributed
/// to `PRODUCER`.
fn submit<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    n: usize,
    record: &Value,
) {
    let observation = ObservationId(uuid(2 * n as u64 + 2));
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "host/fixture".to_owned(),
            subject: record["subject"].as_str().expect("subject").to_owned(),
            observed_at: ObservedAt(record["observed_at"].as_str().expect("instant").to_owned()),
            payload: commission_json::Value::Null,
        }))
        .expect("the governor keeps the observation");
    let evidence = EvidenceData {
        evidence_id: EvidenceId(uuid(2 * n as u64 + 1)),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("revision"),
        producer: String::new(),
        observation_ids: vec![observation],
        facts: commission_json::parse(&record.to_string()).expect("facts are JSON"),
        provenance: commission_json::Value::Object(Vec::new()),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

/// The fixture's evidence records up to `review-rejected`, each with the instant it was observed:
/// a record about a superseded implementation revision (`tests-r1`), a deployment observation the
/// trusted time keeps fresh, and a passing test run and a rejected review of the current revision.
fn fixture_records() -> Vec<Value> {
    fixture()["states"]
        .as_array()
        .expect("states")
        .iter()
        .take(4)
        .flat_map(|state| {
            state["add_evidence"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .map(|entry| {
            let mut record = entry["record"].clone();
            record["observed_at"] = entry["observed_at"].clone();
            record
        })
        .collect()
}

/// The host's trusted time: always [`AT`].
fn trusted_time(_: &CaseId) -> Result<Option<String>, GovernorError> {
    Ok(Some(AT.to_owned()))
}

/// The `canon-case/1` snapshot the governor evaluates for `case-1` at revision 1, each input's
/// value the text the governor gives Canon.
fn snapshot() -> Value {
    let inputs: serde_json::Map<String, Value> = fixture_inputs()
        .iter()
        .map(|(input, value)| (input.to_owned(), Value::from(value)))
        .collect();
    json!({
        "format": "canon-case/1",
        "id": CASE,
        "protocol": "software.change",
        "artifacts": fixture()["case"]["artifacts"],
        "inputs": inputs,
        "revision": "r1",
    })
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
    let fixture = fixture();
    let inputs: Vec<(String, String)> = fixture["case"]["inputs"]
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

/// `tests/fixtures/chg-1842.fixture.yaml`, a copy of engineering-protocols' fixture at the tag this
/// crate depends on.
fn fixture() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chg-1842.fixture.yaml");
    serde_yaml_ng::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("YAML")
}

fn parse(value: &Value) -> json::Value {
    json::parse(&value.to_string()).expect("JSON")
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// `document` written exactly as it was read.
fn compact(document: &json::Value) -> String {
    let mut out = String::new();
    json::push_value(&mut out, document);
    out
}

/// `document` as Canon renders a `canon-decision/1` document (`b10x_canon::eval::render`): object
/// keys in code-point order, members indented by two spaces, `serde_json`'s string escaping, and
/// one newline at the end.
fn canonical(document: &json::Value) -> String {
    let value: Value = serde_json::from_str(&compact(document)).expect("the document is JSON");
    let mut rendered = serde_json::to_string_pretty(&value).expect("JSON renders");
    rendered.push('\n');
    rendered
}

/// The frontier id the governor issues for `case` at `revision` from Canon's rendering `rendered`
/// of its decision: a version 8 UUID from two 64-bit FNV-1a hashes of
/// `<case>\n<revision>\n<rendered>`.
fn frontier_id(case: &str, revision: i64, rendered: &str) -> String {
    let fnv1a = |basis: u64, bytes: &[u8]| {
        bytes.iter().fold(basis, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let input = format!("{case}\n{revision}\n{rendered}");
    let high = fnv1a(0xcbf2_9ce4_8422_2325, input.as_bytes());
    let low = fnv1a(0x6c62_272e_07bb_0142, input.as_bytes());
    let bits = (u128::from(high) << 64 | u128::from(low)) & !(0xf << 76) & !(0x3 << 62)
        | (0x8 << 76)
        | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
