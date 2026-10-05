//! Adversary cases for the Canon-backed governor (story `canon-governor`, adversary pass 1).
//!
//! Canon is the oracle throughout: every expectation is `b10x_canon::eval::evaluate` over the same
//! protocol, case snapshot and evidence, or a literal taken from the protocol or the ELS fixture.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use b10x_canon::eval::{case_from_value, evaluate, evidence_from_value};
use b10x_canon::ir::{Ir, compile};
use b10x_canon::model::Decision;
use b10x_commission::model::json;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, EvidenceData, EvidenceId, FrontierData,
    GovernorError, ObservationId, Truth,
};
use b10x_commission::ports::evidence::{EvidenceError, submit_evidence};
use b10x_commission::ports::governor::Governor as _;
use governor::{CanonGovernor, MemoryCaseStore, OpenError, UpdateError};
use serde_json::{Value, json};

type Gov = CanonGovernor<MemoryCaseStore>;

const SOFTWARE_CHANGE: &str = "software-change@1";
const INCIDENT_RESPONSE: &str = "incident-response@1";

// ---------------------------------------------------------------------------------------------
// Revision binding
// ---------------------------------------------------------------------------------------------

/// A new revision of an artifact the evidence is not about leaves that evidence applying: tests
/// that passed on the current implementation still verify it after the release moves on.
#[test]
fn an_unrelated_artifact_revision_keeps_implementation_evidence() {
    let ir = ir_of("software-change", 1);
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let pass = record("tests-r2", "test_result", "pass", "implementation", "R2");
    submit_record(&governor, &case, &mut ids, &pass).expect("taken");
    assert_eq!(
        merge_status(&governor, &case),
        ActionStatus::ApprovalRequired
    );

    let before = governor.current_revision(&case).expect("revision");
    let after = governor
        .update_revision(&case, "release", "v1")
        .expect("release takes a new revision");
    assert_eq!(after, before + 1);
    assert_eq!(
        governor.current_revision(&case).expect("revision"),
        before + 1
    );
    assert_eq!(
        merge_status(&governor, &case),
        ActionStatus::ApprovalRequired
    );
    assert_eq!(claim(&governor, &case, "tests.pass"), "true");
    assert_frontier_is_canon(
        &governor,
        &case,
        &ir,
        std::slice::from_ref(&pass),
        "after release v1",
    );
}

/// Evidence about a revision the case no longer holds is taken and kept, never used while the
/// subject is at another revision, and used again when the subject comes back to it.
#[test]
fn evidence_for_a_superseded_revision_is_kept_and_unused() {
    let ir = ir_of("software-change", 1);
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    governor
        .update_revision(&case, "implementation", "R3")
        .expect("R3");

    let stale = record("tests-r2", "test_result", "pass", "implementation", "R2");
    submit_record(&governor, &case, &mut ids, &stale).expect("stale evidence is taken");
    assert_eq!(
        governor.evidence(&case).expect("evidence").len(),
        1,
        "and kept"
    );
    assert_eq!(claim(&governor, &case, "tests.pass"), "unknown");
    assert_eq!(merge_status(&governor, &case), ActionStatus::Blocked);
    assert_frontier_is_canon(&governor, &case, &ir, std::slice::from_ref(&stale), "at R3");

    let back = governor
        .update_revision(&case, "implementation", "R2")
        .expect("back to R2");
    assert_eq!(back, 3, "opened at 1, two revisions recorded");
    assert_eq!(claim(&governor, &case, "tests.pass"), "true");
    assert_eq!(
        merge_status(&governor, &case),
        ActionStatus::ApprovalRequired
    );
    assert_frontier_is_canon(
        &governor,
        &case,
        &ir,
        std::slice::from_ref(&stale),
        "back at R2",
    );
}

/// A refused `update_revision` records nothing and leaves the case revision where it was.
#[test]
fn a_refused_revision_update_moves_nothing() {
    let governor = Gov::new(MemoryCaseStore::default());
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let revisions = governor.revisions(&case).expect("revisions");

    assert_eq!(
        governor.update_revision(&case, "nowhere", "x1"),
        Err(UpdateError::UndeclaredArtifact {
            artifact: "nowhere".to_owned()
        })
    );
    for bad in ["", " R3", "R 3", "R3\n"] {
        assert_eq!(
            governor.update_revision(&case, "implementation", bad),
            Err(UpdateError::InvalidRevision {
                artifact: "implementation".to_owned()
            }),
            "revision {bad:?}"
        );
    }
    assert_eq!(
        governor.update_revision(&CaseId("case-99".to_owned()), "implementation", "R3"),
        Err(UpdateError::UnknownCase)
    );
    assert_eq!(governor.current_revision(&case).expect("revision"), 1);
    assert_eq!(governor.revisions(&case).expect("revisions"), revisions);
}

// ---------------------------------------------------------------------------------------------
// Frontier versus Canon
// ---------------------------------------------------------------------------------------------

/// Over seeded random sequences of evidence and artifact revisions, on both ELS built-ins, every
/// frontier is Canon's decision: each action's status, capability and reasons, each claim's value,
/// each obligation, the case and its revision; and completion is Canon's one legitimate outcome.
/// A replay on a second governor issues the same frontier ids, and two frontiers that differ never
/// share an id.
#[test]
fn the_frontier_is_canons_decision_over_random_evidence() {
    for seed in [0x5eed_0001_u64, 0x5eed_0002, 0x5eed_0003] {
        random_walk(seed, &software_change_pool());
        random_walk(seed, &incident_response_pool());
    }
}

struct Pool {
    protocol: &'static str,
    name: &'static str,
    /// Each artifact with the revisions it can take; the first is the opening one.
    artifacts: Vec<(&'static str, Vec<&'static str>)>,
    /// Each evidence kind, the subject it is usually about, and its results ("" for none).
    kinds: Vec<(&'static str, &'static str, Vec<&'static str>)>,
}

fn software_change_pool() -> Pool {
    Pool {
        protocol: SOFTWARE_CHANGE,
        name: "software-change",
        artifacts: vec![
            ("intent", vec!["i1", "i2"]),
            ("system_specification", vec!["s1", "s2"]),
            ("plan", vec!["p1", "p2"]),
            ("implementation", vec!["R2", "R1", "R3"]),
            ("release", vec!["v0", "v1"]),
            ("deployment", vec!["d0", "d1"]),
        ],
        kinds: vec![
            ("test_result", "implementation", vec!["pass", "fail"]),
            (
                "code_review",
                "implementation",
                vec!["approved", "rejected"],
            ),
            (
                "operational_observation",
                "deployment",
                vec!["healthy", "unhealthy"],
            ),
            ("build_provenance", "release", vec!["", "built"]),
            (
                "objective_observation",
                "intent",
                vec!["satisfied", "unsatisfied"],
            ),
        ],
    }
}

fn incident_response_pool() -> Pool {
    Pool {
        protocol: INCIDENT_RESPONSE,
        name: "incident-response",
        artifacts: vec![
            ("service", vec!["svc1", "svc2"]),
            ("release", vec!["rel1", "rel2"]),
        ],
        kinds: vec![
            ("impact_assessment", "service", vec!["bounded", "unbounded"]),
            (
                "operational_observation",
                "service",
                vec!["healthy", "unhealthy"],
            ),
            ("cause_analysis", "release", vec!["identified", "", "open"]),
        ],
    }
}

fn random_walk(seed: u64, pool: &Pool) {
    let ir = ir_of(pool.name, 1);
    let opening: BTreeMap<String, String> = pool
        .artifacts
        .iter()
        .map(|(artifact, revisions)| ((*artifact).to_owned(), revisions[0].to_owned()))
        .collect();
    let first = Gov::new(MemoryCaseStore::default());
    let second = Gov::new(MemoryCaseStore::default());
    let case = first.open(pool.protocol, opening.clone()).expect("opens");
    let replay = second.open(pool.protocol, opening).expect("opens");
    assert_eq!(case, replay);

    let mut rng = Rng(seed);
    let mut ids_a = Ids::default();
    let mut ids_b = Ids::default();
    let mut records: Vec<Value> = Vec::new();
    let mut by_id: BTreeMap<String, String> = BTreeMap::new();
    for step in 0..120 {
        let context = format!("{} seed {seed:#x} step {step}", pool.protocol);
        if rng.below(4) == 0 {
            let (artifact, revisions) = &pool.artifacts[rng.below(pool.artifacts.len())];
            let revision = revisions[rng.below(revisions.len())];
            let a = first.update_revision(&case, artifact, revision);
            let b = second.update_revision(&replay, artifact, revision);
            assert_eq!(a, b, "{context}");
            assert!(a.is_ok(), "{context}: {a:?}");
        } else {
            let (kind, usual, results) = &pool.kinds[rng.below(pool.kinds.len())];
            // Mostly about its usual subject, sometimes about another declared artifact.
            let subject = if rng.below(5) == 0 {
                pool.artifacts[rng.below(pool.artifacts.len())].0
            } else {
                usual
            };
            let revisions = &pool
                .artifacts
                .iter()
                .find(|(artifact, _)| *artifact == subject)
                .expect("declared")
                .1;
            let revision = revisions[rng.below(revisions.len())];
            let result = results[rng.below(results.len())];
            let id = format!("e{step}");
            let rec = record(&id, kind, result, subject, revision);
            submit_record(&first, &case, &mut ids_a, &rec).expect("taken");
            submit_record(&second, &replay, &mut ids_b, &rec).expect("taken");
            records.push(rec);
        }

        assert_frontier_is_canon(&first, &case, &ir, &records, &context);
        assert_completion_is_canon(&first, &case, &ir, &records, &context);

        let a = first.frontier(&case).expect("frontier").into_data();
        let b = second.frontier(&replay).expect("frontier").into_data();
        assert_eq!(
            a, b,
            "{context}: a replay issues the same frontier, id and all"
        );
        assert!(
            is_uuid_v8(&a.frontier_id.0.0),
            "{context}: {:?}",
            a.frontier_id
        );
        let again = first.frontier(&case).expect("frontier").into_data();
        assert_eq!(a.frontier_id, again.frontier_id, "{context}: asked twice");

        let content = content_key(&a);
        if let Some(previous) = by_id.insert(a.frontier_id.0.0.clone(), content.clone()) {
            assert_eq!(
                previous, content,
                "{context}: two different frontiers share id {}",
                a.frontier_id.0.0
            );
        }
    }
}

/// Everything of a frontier except its id.
fn content_key(frontier: &FrontierData) -> String {
    format!(
        "{:?}|{}|{:?}|{:?}|{:?}",
        frontier.case_id,
        frontier.case_revision,
        frontier.claims,
        frontier.obligations,
        frontier.actions
    )
}

fn is_uuid_v8(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.len() == 36
        && [8, 13, 18, 23].iter().all(|&at| bytes[at] == b'-')
        && text
            .chars()
            .enumerate()
            .all(|(at, c)| [8, 13, 18, 23].contains(&at) || matches!(c, '0'..='9' | 'a'..='f'))
        && bytes[14] == b'8'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
}

/// The incident-response obligation is open until the service is healthy at its current
/// revision, and open again once the service moves on: the frontier's obligations are Canon's.
#[test]
fn the_frontier_lists_each_obligation_as_canon_decides_it() {
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(
            INCIDENT_RESPONSE,
            revs(&[("service", "svc1"), ("release", "rel1")]),
        )
        .expect("opens");
    assert_eq!(
        obligations(&governor, &case),
        [("restore_service".to_owned(), true)]
    );

    let healthy = record(
        "health-1",
        "operational_observation",
        "healthy",
        "service",
        "svc1",
    );
    submit_record(&governor, &case, &mut ids, &healthy).expect("taken");
    assert_eq!(claim(&governor, &case, "service.healthy"), "true");
    assert_eq!(
        obligations(&governor, &case),
        [("restore_service".to_owned(), false)]
    );

    governor
        .update_revision(&case, "service", "svc2")
        .expect("svc2");
    assert_eq!(claim(&governor, &case, "service.healthy"), "unknown");
    assert_eq!(
        obligations(&governor, &case),
        [("restore_service".to_owned(), true)]
    );
}

/// The frontier's claims are the values the ELS fixture expects in every one of its states, the
/// grant-only state included (a grant decides no claim).
#[test]
fn the_frontier_claims_are_the_fixture_claims_in_every_state() {
    let fixture = fixture();
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let mut seen = 0;
    for state in fixture["states"].as_array().expect("states") {
        let id = state["id"].as_str().expect("id");
        for entry in state["add_evidence"].as_array().expect("add_evidence") {
            let mut rec = entry["record"].clone();
            rec["observed_at"] = entry["observed_at"].clone();
            submit_record(&governor, &case, &mut ids, &rec).expect("taken");
        }
        let frontier = governor.frontier(&case).expect("frontier").into_data();
        let actual: BTreeMap<String, &str> = frontier
            .claims
            .iter()
            .map(|c| (c.claim.clone(), truth(&c.value)))
            .collect();
        let expected: BTreeMap<String, &str> = state["expect"]["claims"]
            .as_object()
            .expect("claims")
            .iter()
            .map(|(claim, value)| {
                let value = match value {
                    Value::Bool(true) => "true",
                    Value::Bool(false) => "false",
                    other => other.as_str().expect("value"),
                };
                (claim.clone(), value)
            })
            .collect();
        assert_eq!(actual, expected, "state {id}");
        seen += 1;
    }
    assert_eq!(seen, 7, "every fixture state");
}

// ---------------------------------------------------------------------------------------------
// Completion
// ---------------------------------------------------------------------------------------------

/// `accepted` is reported once its claims are true, and no longer once the implementation or the
/// release moves on, as Canon decides.
#[test]
fn completion_follows_canon_when_a_revision_moves_after_acceptance() {
    let ir = ir_of("software-change", 1);
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let records = [
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
    for rec in &records {
        submit_record(&governor, &case, &mut ids, rec).expect("taken");
    }
    assert_eq!(completion(&governor, &case), Some("accepted".to_owned()));

    governor
        .update_revision(&case, "release", "v1")
        .expect("v1");
    assert_eq!(
        completion(&governor, &case),
        None,
        "release v1 has no provenance"
    );
    assert_completion_is_canon(&governor, &case, &ir, &records, "release v1");

    governor
        .update_revision(&case, "release", "v0")
        .expect("v0");
    assert_eq!(completion(&governor, &case), Some("accepted".to_owned()));
    governor
        .update_revision(&case, "implementation", "R3")
        .expect("R3");
    assert_eq!(completion(&governor, &case), None, "R3 is not verified");
    assert_completion_is_canon(&governor, &case, &ir, &records, "implementation R3");
}

// ---------------------------------------------------------------------------------------------
// Evidence port
// ---------------------------------------------------------------------------------------------

/// Evidence that is not a Canon record, or a record Canon refuses, is taken and kept and never
/// stops an evaluation; evidence that applies still applies after it.
#[test]
fn evidence_that_does_not_apply_is_kept_and_never_breaks_the_case() {
    let ir = ir_of("software-change", 1);
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let mut applied: Vec<Value> = Vec::new();
    let mut kept = 0;

    // Facts that are not a Canon record.
    for facts in [
        json!({"tests.pass": true}),
        json!(null),
        json!([1, 2]),
        json!("pass"),
        json!({"format": "canon-evidence/2", "id": "x", "kind": "test_result",
               "subject": "implementation", "subject_revision": "R2", "result": "pass"}),
    ] {
        submit(&governor, &case, &mut ids, "test_result", &facts).expect("taken");
        kept += 1;
    }
    // A Canon record of a kind the protocol does not declare.
    let undeclared = record("deploy-1", "deploy_log", "ok", "deployment", "d0");
    submit_record(&governor, &case, &mut ids, &undeclared).expect("taken");
    kept += 1;
    // A Canon record about an artifact the protocol does not declare.
    let nowhere = record("nowhere-1", "test_result", "pass", "nowhere", "n1");
    submit_record(&governor, &case, &mut ids, &nowhere).expect("taken");
    kept += 1;
    // A record whose kind is not the evidence's kind.
    let mismatched = record("mismatch-1", "test_result", "pass", "implementation", "R2");
    submit(&governor, &case, &mut ids, "code_review", &mismatched).expect("taken");
    kept += 1;
    assert_eq!(claim(&governor, &case, "tests.pass"), "unknown");
    assert_frontier_is_canon(&governor, &case, &ir, &applied, "after the junk");

    // Two records under one Canon id: the first applies, the second does not.
    let pass = record("t-1", "test_result", "pass", "implementation", "R2");
    let fail = record("t-1", "test_result", "fail", "implementation", "R2");
    submit_record(&governor, &case, &mut ids, &pass).expect("taken");
    submit_record(&governor, &case, &mut ids, &fail).expect("taken");
    applied.push(pass);
    kept += 2;
    assert_eq!(claim(&governor, &case, "tests.pass"), "true");

    // Evidence that applies still applies after all of it.
    let approved = record(
        "review-1",
        "code_review",
        "approved",
        "implementation",
        "R2",
    );
    submit_record(&governor, &case, &mut ids, &approved).expect("taken");
    applied.push(approved);
    kept += 1;
    assert_eq!(claim(&governor, &case, "implementation.reviewed"), "true");
    assert_frontier_is_canon(&governor, &case, &ir, &applied, "after the valid records");
    assert_eq!(
        governor.evidence(&case).expect("evidence").len(),
        kept,
        "all kept"
    );
    assert_eq!(governor.current_revision(&case).expect("revision"), 1);
}

/// Evidence for a case the governor does not hold is refused as an unknown case and kept nowhere.
#[test]
fn evidence_for_an_unknown_case_is_refused_and_kept_nowhere() {
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let unknown = CaseId("case-404".to_owned());
    let rec = record("t-1", "test_result", "pass", "implementation", "R2");
    let facts = json::parse(&rec.to_string()).expect("JSON");
    let evidence = EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: unknown.clone(),
        kind: "test_result".to_owned(),
        subject_revision: 1,
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts,
        provenance: json::Value::Null,
    };
    assert_eq!(
        submit_evidence(&governor, "service:ci", evidence),
        Err(EvidenceError::Governor(GovernorError::UnknownCase))
    );
    assert!(governor.evidence(&case).expect("evidence").is_empty());
    assert_eq!(governor.evidence(&unknown), Err(GovernorError::UnknownCase));
    assert_eq!(claim(&governor, &case, "tests.pass"), "unknown");
}

// ---------------------------------------------------------------------------------------------
// Open
// ---------------------------------------------------------------------------------------------

/// `open_case` never replaces a held case: a second open under its id is refused and the held
/// case keeps its revision, artifact revisions and evidence. `open` skips an id `open_case` took.
#[test]
fn a_held_case_is_never_opened_again() {
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let held = CaseId("case-1".to_owned());
    governor
        .open_case(held.clone(), SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    governor
        .update_revision(&held, "implementation", "R3")
        .expect("R3");
    let rec = record("t-1", "test_result", "pass", "implementation", "R3");
    submit_record(&governor, &held, &mut ids, &rec).expect("taken");

    assert_eq!(
        governor.open_case(held.clone(), SOFTWARE_CHANGE, fixture_revisions()),
        Err(OpenError::CaseExists {
            case: "case-1".to_owned()
        })
    );
    assert_eq!(governor.current_revision(&held).expect("held"), 2);
    assert_eq!(
        governor.revisions(&held).expect("held")["implementation"],
        "R3"
    );
    assert_eq!(governor.evidence(&held).expect("held").len(), 1);
    assert_eq!(claim(&governor, &held, "tests.pass"), "true");

    let next = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    assert_eq!(next, CaseId("case-2".to_owned()));
    assert_eq!(governor.current_revision(&next).expect("held"), 1);
    assert_eq!(governor.current_revision(&held).expect("held"), 2);
}

/// Protocols the ELS registry does not hold under exactly `<name>@<major>`, revision maps with an
/// artifact too many or an invalid revision, and invalid case ids are refused, and a refused open
/// holds nothing.
#[test]
fn refused_opens_hold_nothing() {
    let governor = Gov::new(MemoryCaseStore::default());
    for protocol in [
        "software-change@01",
        "software-change@+1",
        "software-change@2",
        "software-change@",
        "software-change",
        "software.change@1",
        "@1",
        "",
        "software-change@1 ",
        "software-change@1@1",
    ] {
        match governor.open(protocol, fixture_revisions()) {
            Err(OpenError::UnknownProtocol { protocol: named }) => assert_eq!(named, protocol),
            other => panic!("protocol {protocol:?} is refused as unknown, got {other:?}"),
        }
    }

    let mut extra = fixture_revisions();
    extra.insert("extra".to_owned(), "x1".to_owned());
    assert_eq!(
        governor.open(SOFTWARE_CHANGE, extra),
        Err(OpenError::UndeclaredArtifact {
            artifact: "extra".to_owned()
        })
    );
    let mut empty = fixture_revisions();
    empty.insert("plan".to_owned(), String::new());
    assert_eq!(
        governor.open(SOFTWARE_CHANGE, empty),
        Err(OpenError::InvalidRevision {
            artifact: "plan".to_owned()
        })
    );
    // The incident-response map does not fit software-change.
    assert!(matches!(
        governor.open(
            SOFTWARE_CHANGE,
            revs(&[("service", "svc1"), ("release", "rel1")])
        ),
        Err(OpenError::MissingRevision { .. })
    ));
    for bad in ["", " case", "case 1", "case\t1"] {
        assert_eq!(
            governor.open_case(CaseId(bad.to_owned()), SOFTWARE_CHANGE, fixture_revisions()),
            Err(OpenError::InvalidCaseId {
                case: bad.to_owned()
            })
        );
    }
    assert_eq!(
        governor.current_revision(&CaseId("case-1".to_owned())),
        Err(GovernorError::UnknownCase),
        "no refused open holds a case"
    );
    let first = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    assert_eq!(first, CaseId("case-1".to_owned()));
}

// ---------------------------------------------------------------------------------------------
// Authority
// ---------------------------------------------------------------------------------------------

/// Through every state of the ELS fixture, the grant-only one included, the governor never makes
/// `repository.merge` admissible: when its precondition holds it needs approval, with the one
/// reason that no authority decided `repository.merge`.
#[test]
fn the_governor_never_supplies_authority() {
    let fixture = fixture();
    let governor = Gov::new(MemoryCaseStore::default());
    let mut ids = Ids::default();
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let mut approval_seen = false;
    for state in fixture["states"].as_array().expect("states") {
        let id = state["id"].as_str().expect("id");
        for entry in state["add_evidence"].as_array().expect("add_evidence") {
            let mut rec = entry["record"].clone();
            rec["observed_at"] = entry["observed_at"].clone();
            submit_record(&governor, &case, &mut ids, &rec).expect("taken");
        }
        let frontier = governor.frontier(&case).expect("frontier").into_data();
        let merge = frontier
            .actions
            .iter()
            .find(|action| action.action == "repository.merge")
            .expect("merge listed");
        assert_ne!(merge.status, ActionStatus::Admissible, "state {id}");
        if merge.status == ActionStatus::ApprovalRequired {
            approval_seen = true;
            assert_eq!(
                merge.reasons,
                [r#"{"capability":"repository.merge","decision":"none"}"#],
                "state {id}"
            );
        }
    }
    assert!(approval_seen);
}

// ---------------------------------------------------------------------------------------------
// Concurrency
// ---------------------------------------------------------------------------------------------

/// Evidence and revisions recorded from several threads at once are all kept: the case revision
/// rises by one per update, every record is held, and the frontier is Canon's over all of it.
#[test]
fn concurrent_writes_are_all_recorded() {
    let ir = ir_of("software-change", 1);
    let governor = Gov::new(MemoryCaseStore::default());
    let case = governor
        .open(SOFTWARE_CHANGE, fixture_revisions())
        .expect("opens");
    let writers = 4;
    let per = 8;
    std::thread::scope(|scope| {
        for w in 0..writers {
            let governor = &governor;
            let case = &case;
            scope.spawn(move || {
                let mut ids = Ids(1_000 * (w + 1));
                for n in 0..per {
                    let rec = record(
                        &format!("w{w}-{n}"),
                        "operational_observation",
                        "healthy",
                        "deployment",
                        "d0",
                    );
                    submit_record(governor, case, &mut ids, &rec).expect("taken");
                }
            });
            scope.spawn(move || {
                for n in 0..per {
                    governor
                        .update_revision(case, "plan", &format!("p{w}-{n}"))
                        .expect("plan moves");
                }
            });
        }
    });
    let updates = i64::try_from(writers * per).expect("small");
    assert_eq!(
        governor.current_revision(&case).expect("revision"),
        1 + updates
    );
    let held = governor.evidence(&case).expect("evidence");
    assert_eq!(held.len(), usize::try_from(writers * per).expect("small"));
    let records: Vec<Value> = held
        .iter()
        .map(|e| serde_json::from_str(&json_text(&e.facts)).expect("facts"))
        .collect();
    assert_frontier_is_canon(&governor, &case, &ir, &records, "after concurrent writes");
    assert_eq!(claim(&governor, &case, "deployment.healthy"), "true");
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// The frontier the governor issues now is Canon's decision over the case's current revisions and
/// `records`: the case, its revision, each claim, each obligation, and each declared action once
/// with Canon's status, the capability it requires and Canon's reasons.
fn assert_frontier_is_canon(
    governor: &Gov,
    case: &CaseId,
    ir: &Ir,
    records: &[Value],
    context: &str,
) {
    let decision = canon(ir, case, &governor.revisions(case).expect("held"), records);
    let frontier = governor.frontier(case).expect("frontier").into_data();
    assert_eq!(&frontier.case_id, case, "{context}");
    assert_eq!(
        frontier.case_revision,
        governor.current_revision(case).expect("revision"),
        "{context}"
    );

    let claims: BTreeMap<String, String> = frontier
        .claims
        .iter()
        .map(|c| (c.claim.clone(), truth(&c.value).to_owned()))
        .collect();
    let expected: BTreeMap<String, String> = decision
        .claims
        .iter()
        .map(|(id, entry)| (id.as_str().to_owned(), entry.value.to_string()))
        .collect();
    assert_eq!(claims, expected, "{context}: claims");

    let obligations: Vec<(String, bool)> = frontier
        .obligations
        .iter()
        .map(|o| (o.obligation.clone(), o.open))
        .collect();
    let expected: Vec<(String, bool)> = decision
        .obligations
        .as_ref()
        .map(|section| {
            section
                .as_array()
                .expect("array")
                .iter()
                .map(|entry| {
                    (
                        entry["id"].as_str().expect("id").to_owned(),
                        entry["status"] == "open",
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(obligations, expected, "{context}: obligations");

    let canon_actions = decision
        .actions
        .as_ref()
        .and_then(|a| a.as_object())
        .expect("actions");
    let listed: BTreeSet<&str> = frontier.actions.iter().map(|a| a.action.as_str()).collect();
    assert_eq!(listed.len(), frontier.actions.len(), "{context}: each once");
    assert_eq!(
        listed,
        canon_actions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        "{context}: every declared action"
    );
    for action in &frontier.actions {
        let name = action.action.as_str();
        let entry = &canon_actions[name];
        let status = match entry["status"].as_str().expect("status") {
            "admissible" => ActionStatus::Admissible,
            "approval-required" => ActionStatus::ApprovalRequired,
            "blocked" => ActionStatus::Blocked,
            other => panic!("{other}"),
        };
        assert_eq!(action.status, status, "{context}: {name} status");
        let reasons: Vec<String> = entry["reasons"]
            .as_array()
            .map(|reasons| {
                reasons
                    .iter()
                    .map(|r| serde_json::to_string(r).expect("JSON"))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(action.reasons, reasons, "{context}: {name} reasons");
        let requires: Vec<String> = ir
            .actions
            .iter()
            .find(|(id, _)| id.as_str() == name)
            .map(|(_, declared)| {
                declared
                    .requires
                    .iter()
                    .map(|c| c.as_str().to_owned())
                    .collect()
            })
            .expect("declared");
        assert!(requires.len() <= 1, "{name}");
        assert_eq!(
            action.capability,
            requires.first().cloned(),
            "{context}: {name} capability"
        );
    }
}

/// Completion is `Complete` with Canon's one legitimate outcome, or `Open` when there is none.
fn assert_completion_is_canon(
    governor: &Gov,
    case: &CaseId,
    ir: &Ir,
    records: &[Value],
    context: &str,
) {
    let decision = canon(ir, case, &governor.revisions(case).expect("held"), records);
    let legitimate: Vec<String> = decision
        .outcomes
        .as_ref()
        .and_then(|o| o.as_object())
        .map(|outcomes| {
            outcomes
                .iter()
                .filter(|(_, entry)| entry["status"] == "legitimate")
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default();
    let expected = match legitimate.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    };
    assert_eq!(
        completion(governor, case),
        expected,
        "{context}: completion"
    );
}

/// Canon's decision for `case` at `revisions` over the records whose Canon id comes first.
fn canon(
    ir: &Ir,
    case: &CaseId,
    revisions: &BTreeMap<String, String>,
    records: &[Value],
) -> Decision {
    let artifacts: serde_json::Map<String, Value> = revisions
        .iter()
        .map(|(artifact, revision)| (artifact.clone(), json!({ "revision": revision })))
        .collect();
    let snapshot = json!({
        "format": "canon-case/1",
        "id": case.0,
        "protocol": ir.protocol.id.as_str(),
        "artifacts": artifacts,
    });
    let yaml = |value: &Value| serde_yaml_ng::to_value(value).expect("JSON is YAML");
    let snapshot = case_from_value(&yaml(&snapshot)).expect("snapshot");
    let mut seen = BTreeSet::new();
    let evidence: Vec<_> = records
        .iter()
        .filter(|rec| seen.insert(rec["id"].as_str().expect("id").to_owned()))
        .map(|rec| evidence_from_value(&yaml(rec)).expect("record"))
        .collect();
    evaluate(ir, &snapshot, &evidence).expect("Canon evaluates")
}

fn completion(governor: &Gov, case: &CaseId) -> Option<String> {
    match governor.completion(case).expect("completion") {
        CompletionDetermination::Complete(complete) => Some(complete.outcome),
        CompletionDetermination::Open(_) => None,
    }
}

fn merge_status(governor: &Gov, case: &CaseId) -> ActionStatus {
    governor
        .frontier(case)
        .expect("frontier")
        .into_data()
        .actions
        .into_iter()
        .find(|a| a.action == "repository.merge")
        .expect("merge listed")
        .status
}

fn claim(governor: &Gov, case: &CaseId, name: &str) -> &'static str {
    let frontier = governor.frontier(case).expect("frontier").into_data();
    let value = frontier
        .claims
        .iter()
        .find(|c| c.claim == name)
        .unwrap_or_else(|| panic!("claim {name} listed"))
        .value;
    truth(&value)
}

fn obligations(governor: &Gov, case: &CaseId) -> Vec<(String, bool)> {
    governor
        .frontier(case)
        .expect("frontier")
        .into_data()
        .obligations
        .into_iter()
        .map(|o| (o.obligation, o.open))
        .collect()
}

fn truth(value: &Truth) -> &'static str {
    match value {
        Truth::True => "true",
        Truth::False => "false",
        Truth::Unknown => "unknown",
    }
}

fn ir_of(name: &str, major: u32) -> Ir {
    let builtin = b10x_els::registry::get(name, major).expect("ELS holds it");
    compile(&b10x_canon::model::parse(builtin.yaml).expect("parses")).expect("compiles")
}

fn revs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, r)| ((*a).to_owned(), (*r).to_owned()))
        .collect()
}

fn fixture() -> Value {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest).join("tests/fixtures/chg-1842.fixture.yaml");
    let text = std::fs::read_to_string(&path).expect("fixture");
    serde_yaml_ng::from_str(&text).expect("fixture YAML")
}

fn fixture_revisions() -> BTreeMap<String, String> {
    fixture()["case"]["artifacts"]
        .as_object()
        .expect("artifacts")
        .iter()
        .map(|(a, entry)| {
            (
                a.clone(),
                entry["revision"].as_str().expect("rev").to_owned(),
            )
        })
        .collect()
}

fn record(id: &str, kind: &str, result: &str, subject: &str, revision: &str) -> Value {
    let mut rec = json!({
        "format": "canon-evidence/1",
        "id": id,
        "kind": kind,
        "subject": subject,
        "subject_revision": revision,
    });
    if !result.is_empty() {
        rec["result"] = Value::from(result);
    }
    rec
}

fn submit_record(
    governor: &Gov,
    case: &CaseId,
    ids: &mut Ids,
    rec: &Value,
) -> Result<(), EvidenceError> {
    let kind = rec["kind"].as_str().expect("kind").to_owned();
    submit(governor, case, ids, &kind, rec)
}

fn submit(
    governor: &Gov,
    case: &CaseId,
    ids: &mut Ids,
    kind: &str,
    facts: &Value,
) -> Result<(), EvidenceError> {
    let evidence = EvidenceData {
        evidence_id: EvidenceId(ids.next()),
        case_id: case.clone(),
        kind: kind.to_owned(),
        subject_revision: governor.current_revision(case).expect("held"),
        producer: String::new(),
        observation_ids: vec![ObservationId(ids.next())],
        facts: json::parse(&facts.to_string()).expect("JSON"),
        provenance: json::Value::Null,
    };
    submit_evidence(governor, "service:ci", evidence)
}

fn json_text(value: &json::Value) -> String {
    let mut text = String::new();
    json::push_value(&mut text, value);
    text
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

/// xorshift64*, fixed seed.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let value = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        usize::try_from(value % u64::try_from(n).expect("small")).expect("small")
    }
}
