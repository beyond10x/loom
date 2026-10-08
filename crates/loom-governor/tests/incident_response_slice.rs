//! The incident-response vertical slice (story `incident-response-slice`): the ELS `inc-492` case
//! of `incident.response/1`, driven through Commission's runtime and the Canon-backed governor,
//! leaves emergency mode while its cause stays unknown, and the investigation stays open until a
//! cause analysis identifies the cause (story `incident-investigation-open`).
//!
//! The fixture's states are applied in order. Evidence is submitted through Commission's
//! `submit_evidence`; the grant of `release.rollback` is the authority provider's answer, never the
//! governor's; the fixture's move of the service to `s2` is the effect of `release.rollback`, which
//! an executor proposes and `run_until_blocked` revalidates, checks with the provider and hands to
//! the effect port. After each state the governor's frontier is held to the fixture's expected
//! claims, obligations and action statuses.
//!
//! `incident.response/1` defines emergency mode by its action `emergency.leave`: the case is out of
//! emergency mode once that action is admissible, first at `service-restored`. Leaving emergency
//! mode does not close the investigation: there `cause.identified` is unknown, `investigate_cause`
//! is open and `restore_service` is discharged. `investigate_cause` is discharged only at
//! `cause-identified-after-restore`, where a cause analysis of the restored service identifies the
//! cause, and `emergency.leave` is still admissible. The per-state check already holds every state
//! to the fixture; the closing assertions name these outcomes explicitly.
//!
//! `tests/fixtures/inc-492.fixture.yaml` is a byte-for-byte copy of
//! `fixtures/incident-response/inc-492.fixture.yaml` in beyond10x/engineering-protocols at tag
//! `0.3.0`, the release this crate depends on; `b10x-canon-engineering` does not expose its
//! fixtures.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, EvidenceData,
    EvidenceId, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierData, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, Truth, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::submit_evidence;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::Value;

const PROTOCOL: &str = "incident-response@1";
const PRODUCER: &str = "service:monitoring";
const ROLLBACK: &str = "release.rollback";
const LEAVE: &str = "emergency.leave";
const NOW: &str = "2026-10-04T11:30:00Z";

type Store = CanonGovernor<MemoryCaseStore>;

#[test]
fn leaving_emergency_mode_keeps_the_investigation_open_until_the_cause_is_identified() {
    let fixture = Fixture::load();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = governor
        .open(PROTOCOL, fixture.revisions())
        .expect("the fixture's case opens on incident.response/1");
    let commission = commission(&case);
    let mut ids = Ids::default();
    let mut grants: Vec<String> = Vec::new();
    let mut rollbacks = 0;
    let mut applied = Vec::new();
    let mut restored = None;
    let mut first_leave = None;

    for state in fixture.states() {
        let id = state["id"].as_str().expect("state id").to_owned();
        for grant in state["add_authority"].as_array().expect("add_authority") {
            assert_eq!(grant["decision"], "granted", "state {id}: a grant");
            grants.push(grant["capability"].as_str().expect("capability").to_owned());
        }
        if let Some(moves) = state.get("set_revisions") {
            roll_back(&governor, &commission, &grants, moves, &id);
            rollbacks += 1;
        }
        for entry in state["add_evidence"].as_array().expect("add_evidence") {
            let mut record = entry["record"].clone();
            record["observed_at"] = entry["observed_at"].clone();
            submit(&governor, &case, &mut ids, &record);
        }
        let frontier = governor.frontier(&case).expect("frontier").into_data();
        assert_eq!(frontier.case_id, case, "state {id}");
        assert_eq!(
            frontier.case_revision,
            governor.current_revision(&case).expect("current revision"),
            "state {id}: the frontier is issued for the case revision"
        );
        assert_state(&frontier, &state["expect"], &grants, &id);
        if first_leave.is_none() && actions(&frontier)[LEAVE] == ActionStatus::Admissible {
            first_leave = Some(id.clone());
        }
        if id == "service-restored" {
            restored = Some(frontier);
        }
        applied.push(id);
    }
    assert_eq!(
        applied,
        [
            "initial",
            "cause-identified",
            "rollback-approved",
            "rolled-back",
            "release-observed",
            "service-restored",
            "cause-identified-after-restore",
        ],
        "every state of inc-492 is applied, in order"
    );
    assert_eq!(rollbacks, 1, "the service is moved once, by the rollback");

    // Out of emergency mode, the investigation open: the governor's frontier at
    // `service-restored`, the first state at which `emergency.leave` is admissible.
    assert_eq!(
        first_leave.as_deref(),
        Some("service-restored"),
        "{LEAVE} is first admissible at service-restored"
    );
    let frontier = restored.expect("the frontier at service-restored");
    let restored_claims = claims(&frontier);
    assert_eq!(restored_claims["service.healthy"], Truth::True);
    assert_eq!(restored_claims["impact.bounded"], Truth::True);
    assert_eq!(
        restored_claims["cause.identified"],
        Truth::Unknown,
        "service-restored: the cause is not yet identified"
    );
    let restored_obligations = obligations(&frontier);
    assert_eq!(
        restored_obligations.get("investigate_cause"),
        Some(&true),
        "service-restored: leaving emergency mode leaves `investigate_cause` open"
    );
    assert_eq!(
        restored_obligations.get("restore_service"),
        Some(&false),
        "service-restored: `restore_service` is discharged"
    );
    assert_eq!(
        actions(&frontier)[LEAVE],
        ActionStatus::Admissible,
        "service-restored: out of emergency mode, incident.response/1 admits {LEAVE}"
    );

    // The investigation closed: the governor's last frontier, at `cause-identified-after-restore`.
    let frontier = governor.frontier(&case).expect("frontier").into_data();
    assert_eq!(
        claims(&frontier)["cause.identified"],
        Truth::True,
        "cause-identified-after-restore: the cause analysis identifies the cause"
    );
    assert_eq!(
        obligations(&frontier).get("investigate_cause"),
        Some(&false),
        "cause-identified-after-restore: `investigate_cause` is discharged"
    );
    assert_eq!(
        actions(&frontier)[LEAVE],
        ActionStatus::Admissible,
        "cause-identified-after-restore: {LEAVE} is still admissible"
    );
}

/// The fixture's `set_revisions` state: one run of the commission in which the executor proposes
/// `release.rollback`, the runtime checks it with the authority provider, which holds the grants
/// made so far, and the effect port performs it by moving the case to `moves`.
fn roll_back(
    governor: &Store,
    commission: &Commission<commission_state::Assigned>,
    grants: &[String],
    moves: &Value,
    id: &str,
) {
    assert!(
        grants.iter().any(|grant| grant == ROLLBACK),
        "state {id}: {ROLLBACK} is granted before it is executed"
    );
    let moves: BTreeMap<String, String> = moves
        .as_object()
        .expect("set_revisions")
        .iter()
        .map(|(artifact, revision)| {
            (
                artifact.clone(),
                revision.as_str().expect("revision").to_owned(),
            )
        })
        .collect();
    let case = &commission.data().case_id;
    let before = governor.current_revision(case).expect("current revision");
    let frontier = governor.frontier(case).expect("frontier").into_data();
    let rollback = frontier
        .actions
        .iter()
        .find(|action| action.action == ROLLBACK)
        .expect("the frontier lists the rollback");
    assert_eq!(
        rollback.status,
        ActionStatus::ApprovalRequired,
        "state {id}: the governor supplies no authority decision"
    );
    assert_eq!(rollback.capability.as_deref(), Some(ROLLBACK));

    let authority = grants
        .iter()
        .fold(StaticAuthorityProvider::new(), |provider, grant| {
            provider.answer(grant.clone(), AuthorityVerdict::Allow(Unit(true)))
        });
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: ROLLBACK.to_owned(),
            arguments: ProposedActionArguments(json::Value::Null),
        }),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let effects = Rollback {
        governor,
        moves: moves.clone(),
        invoked: Mutex::new(Vec::new()),
    };
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let end = run_until_blocked(
        governor,
        &executor,
        &authority,
        &effects,
        commission,
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("state {id}: the run ends: {error}"));

    assert_eq!(
        authority
            .asked()
            .iter()
            .map(|query| query.capability.as_str())
            .collect::<Vec<_>>(),
        [ROLLBACK],
        "state {id}: the provider is asked for the rollback's capability, once"
    );
    assert_eq!(
        end.admitted
            .iter()
            .map(|request| request.action.as_str())
            .collect::<Vec<_>>(),
        [ROLLBACK],
        "state {id}: the rollback is admitted once"
    );
    assert_eq!(
        end.admitted[0].expected_case_revision, before,
        "state {id}: admitted at the revision it was proposed on"
    );
    assert!(
        matches!(end.effects.as_slice(), [EffectOutcome::Performed(_)]),
        "state {id}: the rollback is performed: {:?}",
        end.effects
    );
    assert_eq!(
        effects
            .invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_slice(),
        [ROLLBACK],
        "state {id}: the effect port is handed the rollback once"
    );
    let revisions = governor.revisions(case).expect("revisions");
    for (artifact, revision) in &moves {
        assert_eq!(
            &revisions[artifact], revision,
            "state {id}: {artifact} moved"
        );
    }
    assert_eq!(
        governor.current_revision(case).expect("current revision"),
        before + 1,
        "state {id}: the rollback raises the case revision by one"
    );
}

/// The frontier holds the fixture's expected claims, obligations and action statuses. The fixture
/// applies its grants to the action statuses; the governor supplies no authority decision, so an
/// action whose capability is granted stays `approval-required` on the frontier.
fn assert_state(frontier: &FrontierData, expect: &Value, grants: &[String], id: &str) {
    let expected_claims: BTreeMap<String, Truth> = expect["claims"]
        .as_object()
        .expect("expected claims")
        .iter()
        .map(|(claim, value)| (claim.clone(), truth(value)))
        .collect();
    assert_eq!(
        claims(frontier),
        expected_claims,
        "state {id}: the claims the fixture expects"
    );

    let expected_obligations: BTreeMap<String, bool> = expect["obligations"]
        .as_object()
        .expect("expected obligations")
        .iter()
        .map(|(obligation, status)| {
            let open = match status.as_str().expect("obligation status") {
                "open" => true,
                "discharged" => false,
                other => panic!("unknown obligation status {other}"),
            };
            (obligation.clone(), open)
        })
        .collect();
    assert_eq!(
        obligations(frontier),
        expected_obligations,
        "state {id}: the obligations the fixture expects"
    );

    let expected_actions: BTreeMap<String, ActionStatus> = expect["actions"]
        .as_object()
        .expect("expected actions")
        .iter()
        .map(|(action, status)| {
            let granted = grants.iter().any(|grant| grant == action);
            let status = status.as_str().expect("action status");
            let status = if granted && status == "admissible" {
                ActionStatus::ApprovalRequired
            } else {
                action_status(status)
            };
            (action.clone(), status)
        })
        .collect();
    assert_eq!(
        actions(frontier),
        expected_actions,
        "state {id}: the action statuses the fixture expects, grants left to the provider"
    );
}

fn claims(frontier: &FrontierData) -> BTreeMap<String, Truth> {
    frontier
        .claims
        .iter()
        .map(|claim| (claim.claim.clone(), claim.value))
        .collect()
}

fn obligations(frontier: &FrontierData) -> BTreeMap<String, bool> {
    frontier
        .obligations
        .iter()
        .map(|obligation| (obligation.obligation.clone(), obligation.open))
        .collect()
}

fn actions(frontier: &FrontierData) -> BTreeMap<String, ActionStatus> {
    frontier
        .actions
        .iter()
        .map(|action| (action.action.clone(), action.status))
        .collect()
}

fn truth(value: &Value) -> Truth {
    match value {
        Value::Bool(true) => Truth::True,
        Value::Bool(false) => Truth::False,
        Value::String(text) if text == "unknown" => Truth::Unknown,
        other => panic!("unknown claim value {other}"),
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

/// The effect port of the slice: it performs `release.rollback` only, by moving the case's
/// artifacts to `moves`.
struct Rollback<'g> {
    governor: &'g Store,
    moves: BTreeMap<String, String>,
    invoked: Mutex<Vec<String>>,
}

impl EffectPort for Rollback<'_> {
    fn performs(&self, action: &str) -> bool {
        action == ROLLBACK
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.clone();
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(action.clone());
        if action != ROLLBACK {
            return Err(EffectError::new(format!("{action} is not performed here")));
        }
        for (artifact, revision) in &self.moves {
            self.governor
                .update_revision(&commission.data().case_id, artifact, revision)
                .map_err(|error| EffectError::new(error.to_string()))?;
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: json::Value::Null,
            attempt: None,
            audit: None,
        }))
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-oncall".to_owned()),
        authority_context: AuthorityContext(json::Value::Null),
    })
}

/// New ids from counters and one trusted time; no step budget.
#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// Submits `record` through Commission's `submit_evidence`, carried in the evidence's facts and
/// bound to the case's current revision.
fn submit(governor: &Store, case: &CaseId, ids: &mut Ids, record: &Value) {
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
            json::Value::Text("incident_response_slice".to_owned()),
        )]),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Distinct UUIDs, in order, above those the loop's context issues.
#[derive(Default)]
struct Ids(u64);

impl Ids {
    fn next(&mut self) -> Uuid {
        self.0 += 1;
        uuid(0x1000 + self.0)
    }
}

struct Fixture(Value);

impl Fixture {
    fn load() -> Self {
        let manifest =
            std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest).join("tests/fixtures/inc-492.fixture.yaml");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let fixture: Value = serde_yaml_ng::from_str(&text)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(fixture["format"], "els-fixture/1");
        assert_eq!(fixture["protocol"], "protocols/incident-response/1.yaml");
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

    fn states(&self) -> &[Value] {
        self.0["states"].as_array().expect("states")
    }
}
