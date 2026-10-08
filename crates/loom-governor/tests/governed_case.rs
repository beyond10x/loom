//! A Canon-backed governor behind Commission's governor and evidence ports issues the frontier
//! Canon decides for a `software.change/1` case (story `canon-governor`).
//!
//! Canon is the oracle: every status this test expects is the one `b10x_canon::eval` gives for the
//! same protocol, case snapshot and evidence, evaluated here directly, beside the governor.
//!
//! `tests/fixtures/chg-1842.fixture.yaml` is a byte-for-byte copy of
//! `fixtures/software-change/chg-1842.fixture.yaml` in beyond10x/engineering-protocols at tag
//! `0.3.0`, the release this crate depends on; `b10x-canon-engineering` does not expose its
//! fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use b10x_canon::eval::{case_from_value, evaluate, evidence_from_value};
use b10x_canon::ir::{Ir, compile};
use b10x_canon::model::Decision;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, FrontierAction,
    ObservationId,
};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use loom_governor::{CanonGovernor, MemoryCaseStore, OpenError};
use serde_json::Value;

const PROTOCOL: &str = "software-change@1";
const PRODUCER: &str = "service:ci";

#[test]
fn a_governed_case_issues_the_frontier_canon_decides() {
    let fixture = Fixture::load();
    let ir = software_change_ir();

    fresh_case(&fixture, &ir);
    revision_update(&fixture, &ir);
    completion(&fixture, &ir);
    fixture_states(&fixture, &ir);
}

/// A case opened with a revision for every declared artifact holds them as given; one missing an
/// artifact is refused, naming it, and so is a protocol the ELS registry does not hold. The first
/// frontier admits `repository.edit` and `tests.run` and blocks `repository.merge` on its
/// precondition claim.
fn fresh_case(fixture: &Fixture, ir: &Ir) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let revisions = fixture.revisions();
    assert_eq!(
        revisions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ir.artifacts
            .keys()
            .map(|id| id.as_str())
            .collect::<BTreeSet<_>>(),
        "the fixture gives a revision for every artifact software.change/1 declares"
    );

    let case = governor
        .open(PROTOCOL, revisions.clone())
        .expect("a case with every declared artifact opens");
    assert_eq!(
        governor
            .revisions(&case)
            .expect("the governor holds the case"),
        revisions,
        "the case holds the revisions it was opened with"
    );

    let mut missing = revisions.clone();
    missing.remove("deployment");
    match governor.open(PROTOCOL, missing) {
        Err(OpenError::MissingRevision { artifact }) => assert_eq!(artifact, "deployment"),
        other => panic!("a revision map without `deployment` is refused naming it, got {other:?}"),
    }
    match governor.open("no-such-protocol@1", revisions) {
        Err(OpenError::UnknownProtocol { .. }) => {}
        other => panic!("a protocol the ELS registry does not hold is refused, got {other:?}"),
    }

    let revision = governor.current_revision(&case).expect("current revision");
    let frontier = governor.frontier(&case).expect("frontier").into_data();
    assert_eq!(frontier.case_id, case);
    assert_eq!(frontier.case_revision, revision);
    let actions = by_name(&frontier.actions);
    assert_eq!(actions["repository.edit"].status, ActionStatus::Admissible);
    assert_eq!(actions["tests.run"].status, ActionStatus::Admissible);
    let merge = actions["repository.merge"];
    assert_eq!(merge.status, ActionStatus::Blocked);
    assert_eq!(
        merge.reasons.len(),
        1,
        "merge is blocked by its one precondition claim: {:?}",
        merge.reasons
    );
    assert!(
        merge.reasons[0].contains("implementation.verified"),
        "the reason names `implementation.verified`: {:?}",
        merge.reasons
    );
}

/// `update_revision` records a new artifact revision and raises the case revision by one; the next
/// frontier is issued for the new revision, and evidence about the superseded implementation no
/// longer verifies it.
fn revision_update(fixture: &Fixture, ir: &Ir) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(PROTOCOL, fixture.revisions())
        .expect("case opens");

    let pass = record("tests-r2", "test_result", "pass", "implementation", "R2");
    submit(&governor, &case, &mut ids, &pass);
    let before = governor.current_revision(&case).expect("current revision");
    let frontier = governor.frontier(&case).expect("frontier").into_data();
    assert_eq!(frontier.case_revision, before);
    let expected = canon_decision(ir, &fixture.case_snapshot(&[]), std::slice::from_ref(&pass));
    assert_actions(
        ir,
        &expected,
        &frontier.actions,
        before,
        "before the update",
    );
    assert_eq!(
        by_name(&frontier.actions)["repository.merge"].status,
        ActionStatus::ApprovalRequired
    );

    governor
        .update_revision(&case, "implementation", "R3")
        .expect("a declared artifact takes a new revision");
    let after = governor.current_revision(&case).expect("current revision");
    assert_eq!(after, before + 1, "the case revision rises by one");
    assert_eq!(
        governor.revisions(&case).expect("revisions")["implementation"],
        "R3"
    );
    let frontier = governor.frontier(&case).expect("frontier").into_data();
    assert_eq!(frontier.case_id, case);
    assert_eq!(
        frontier.case_revision, after,
        "the next frontier is issued for the new revision"
    );
    let expected = canon_decision(
        ir,
        &fixture.case_snapshot(&[("implementation", "R3")]),
        &[pass],
    );
    assert_actions(ir, &expected, &frontier.actions, after, "after the update");
    assert_eq!(
        by_name(&frontier.actions)["repository.merge"].status,
        ActionStatus::Blocked
    );
}

/// `completion` reports no outcome while `accepted`'s claims are unknown, `accepted` once evidence
/// fed through `EvidencePort` makes them true, and never an outcome Canon holds blocked.
fn completion(fixture: &Fixture, ir: &Ir) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(PROTOCOL, fixture.revisions())
        .expect("case opens");
    let snapshot = fixture.case_snapshot(&[]);

    let steps = [
        record("tests-r2", "test_result", "pass", "implementation", "R2"),
        record("provenance-v0", "build_provenance", "", "release", "v0"),
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
    ];
    let mut submitted: Vec<Value> = Vec::new();
    assert_completion(&governor, &case, &canon_decision(ir, &snapshot, &submitted));
    assert!(
        matches!(
            governor.completion(&case).expect("completion"),
            CompletionDetermination::Open(_)
        ),
        "a fresh case reports no outcome"
    );
    for step in &steps {
        submit(&governor, &case, &mut ids, step);
        submitted.push(step.clone());
        assert_completion(&governor, &case, &canon_decision(ir, &snapshot, &submitted));
    }
    match governor.completion(&case).expect("completion") {
        CompletionDetermination::Complete(complete) => assert_eq!(complete.outcome, "accepted"),
        other => panic!("every claim `accepted` requires is true, got {other:?}"),
    }
}

/// Every state of the ELS fixture that adds evidence, fed through `EvidencePort` in order: the
/// frontier lists each action with the status Canon gives it, the capability it needs and the
/// case revision. Authority grants are never the governor's to supply, so the state that adds only
/// a grant is out of it.
fn fixture_states(fixture: &Fixture, ir: &Ir) {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(PROTOCOL, fixture.revisions())
        .expect("case opens");
    let snapshot = fixture.case_snapshot(&[]);

    let mut submitted: Vec<Value> = Vec::new();
    let mut checked = Vec::new();
    let mut skipped = Vec::new();
    let mut granted = false;
    for state in fixture.states() {
        let id = state["id"].as_str().expect("state id").to_owned();
        let authority = state["add_authority"].as_array().expect("add_authority");
        granted |= !authority.is_empty();
        let evidence = state["add_evidence"].as_array().expect("add_evidence");
        if evidence.is_empty() {
            skipped.push(id);
            continue;
        }
        for entry in evidence {
            let mut record = entry["record"].clone();
            record["observed_at"] = entry["observed_at"].clone();
            submit(&governor, &case, &mut ids, &record);
            submitted.push(record);
        }

        let revision = governor.current_revision(&case).expect("current revision");
        let frontier = governor.frontier(&case).expect("frontier").into_data();
        assert_eq!(frontier.case_id, case, "state {id}");
        let expected = canon_decision(ir, &snapshot, &submitted);
        assert_actions(ir, &expected, &frontier.actions, revision, &id);
        assert_eq!(
            frontier.case_revision, revision,
            "state {id}: the frontier is issued for the case revision"
        );
        assert_eq!(
            by_name(&frontier.actions)["repository.merge"]
                .capability
                .as_deref(),
            Some("repository.merge"),
            "state {id}: the frontier names the capability repository.merge needs"
        );
        if !granted {
            // Before any grant, Canon's statuses are the fixture's own expectations.
            let actions = by_name(&frontier.actions);
            for (action, status) in state["expect"]["actions"].as_object().expect("expect") {
                assert_eq!(
                    actions[action.as_str()].status,
                    action_status(status.as_str().expect("status")),
                    "state {id}: {action} as the fixture expects"
                );
            }
        }
        checked.push(id);
    }
    assert_eq!(
        skipped,
        ["merge-approved"],
        "only the grant-only state is out"
    );
    assert_eq!(
        checked,
        [
            "initial",
            "tests-pass-r2",
            "review-rejected",
            "objective-unmet",
            "deployment-unhealthy",
            "tests-fail-r2",
        ],
        "every state that adds evidence is checked"
    );
}

/// The frontier lists every action the protocol declares once, each with Canon's status, the
/// capability it requires and nothing else; `revision` is the case revision it must be issued for.
fn assert_actions(
    ir: &Ir,
    decision: &Decision,
    actions: &[FrontierAction],
    revision: i64,
    context: &str,
) {
    let canon = decision
        .actions
        .as_ref()
        .expect("Canon decides the actions");
    let canon = canon.as_object().expect("actions section");
    let listed: Vec<&str> = actions
        .iter()
        .map(|action| action.action.as_str())
        .collect();
    let declared: Vec<&str> = ir.actions.keys().map(|id| id.as_str()).collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted, declared,
        "{context}: each declared action listed once (revision {revision})"
    );
    for action in actions {
        let name = action.action.as_str();
        let status = canon[name]["status"].as_str().expect("Canon status");
        assert_eq!(
            action.status,
            action_status(status),
            "{context}: {name} has the status Canon gives it"
        );
        let requires: Vec<&str> = ir
            .actions
            .iter()
            .find(|(id, _)| id.as_str() == name)
            .map(|(_, declared)| declared.requires.iter().map(|c| c.as_str()).collect())
            .expect("declared action");
        assert!(
            requires.len() <= 1,
            "{name} requires one capability at most"
        );
        assert_eq!(
            action.capability.as_deref(),
            requires.first().copied(),
            "{context}: {name} names the capability it needs"
        );
        assert_eq!(
            action.reasons.is_empty(),
            status == "admissible",
            "{context}: {name} carries reasons exactly when it is not admissible: {:?}",
            action.reasons
        );
    }
}

/// The governor's completion is the one legitimate outcome of Canon's decision, or open when
/// Canon holds none legitimate.
fn assert_completion(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
    decision: &Decision,
) {
    let outcomes = decision.outcomes.as_ref().expect("Canon decides outcomes");
    let legitimate: Vec<&str> = outcomes
        .as_object()
        .expect("outcomes section")
        .iter()
        .filter(|(_, entry)| entry["status"] == "legitimate")
        .map(|(id, _)| id.as_str())
        .collect();
    match governor.completion(case).expect("completion") {
        CompletionDetermination::Open(_) => assert!(
            legitimate.is_empty(),
            "Canon holds {legitimate:?} legitimate, the governor reports none"
        ),
        CompletionDetermination::Complete(complete) => assert_eq!(
            legitimate,
            [complete.outcome.as_str()],
            "the governor reports an outcome Canon holds legitimate, and only that"
        ),
    }
}

fn action_status(status: &str) -> ActionStatus {
    match status {
        "admissible" => ActionStatus::Admissible,
        "approval-required" => ActionStatus::ApprovalRequired,
        "blocked" => ActionStatus::Blocked,
        other => panic!("unknown action status {other}"),
    }
}

fn by_name(actions: &[FrontierAction]) -> BTreeMap<&str, &FrontierAction> {
    actions
        .iter()
        .map(|action| (action.action.as_str(), action))
        .collect()
}

/// `software.change/1` from the ELS registry, compiled by Canon.
fn software_change_ir() -> Ir {
    let builtin =
        canon_engineering::registry::get("software-change", 1).expect("ELS holds the protocol");
    let protocol = b10x_canon::model::parse(builtin.yaml).expect("Canon parses it");
    compile(&protocol).expect("Canon compiles it")
}

/// Canon's own decision for `case` over `evidence`, with no authority decision supplied.
fn canon_decision(ir: &Ir, case: &Value, evidence: &[Value]) -> Decision {
    let yaml = |value: &Value| serde_yaml_ng::to_value(value).expect("JSON is YAML");
    let case = case_from_value(&yaml(case)).expect("case snapshot");
    let evidence: Vec<_> = evidence
        .iter()
        .map(|record| evidence_from_value(&yaml(record)).expect("evidence record"))
        .collect();
    evaluate(ir, &case, &evidence).expect("Canon evaluates")
}

/// A `canon-evidence/1` record; an empty `result` leaves the result out.
fn record(id: &str, kind: &str, result: &str, subject: &str, revision: &str) -> Value {
    let mut record = serde_json::json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "subject": subject,
        "subject_revision": revision,
    });
    if !result.is_empty() {
        record["result"] = Value::from(result);
    }
    record
}

/// Submits `record` through Commission's `submit_evidence`, carried in the evidence's facts and
/// bound to the case's current revision.
fn submit(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId, ids: &mut Ids, record: &Value) {
    let facts = json::parse(&record.to_string()).expect("facts are JSON");
    let evidence = EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts,
        provenance: json::Value::Object(vec![(
            "source".to_owned(),
            json::Value::Text("governed_case".to_owned()),
        )]),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

/// Distinct UUIDs, in order.
#[derive(Default)]
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> Uuid {
        self.0 += 1;
        Uuid(format!("00000000-0000-4000-8000-{:012x}", self.0))
    }
}

struct Fixture(Value);

impl Fixture {
    fn load() -> Self {
        let manifest =
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest).join("tests/fixtures/chg-1842.fixture.yaml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let fixture: Value = serde_yaml_ng::from_str(&text)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(fixture["format"], "els-fixture/1");
        assert_eq!(fixture["protocol"], "protocols/software-change/1.yaml");
        Self(fixture)
    }

    /// The case's artifact revisions, as the fixture gives them.
    fn revisions(&self) -> BTreeMap<String, String> {
        self.0["case"]["artifacts"]
            .as_object()
            .expect("case artifacts")
            .iter()
            .map(|(artifact, entry)| {
                let revision = entry["revision"].as_str().expect("revision");
                (artifact.clone(), revision.to_owned())
            })
            .collect()
    }

    /// The fixture's `canon-case/1` snapshot, with `changed` artifact revisions replaced.
    fn case_snapshot(&self, changed: &[(&str, &str)]) -> Value {
        let mut case = self.0["case"].clone();
        for (artifact, revision) in changed {
            case["artifacts"][*artifact]["revision"] = Value::from(*revision);
        }
        case
    }

    fn states(&self) -> &[Value] {
        self.0["states"].as_array().expect("states")
    }
}
