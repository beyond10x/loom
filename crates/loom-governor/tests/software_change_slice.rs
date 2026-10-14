//! I-001, the software-change demonstrator (`story:software-change-slice`): the ELS
//! `software.change/1` fixture case `chg-1842`, driven through Commission's runtime
//! (`run_until_blocked`) with Loom as its executor, on the AEP governor (`CanonGovernor`).
//!
//! The case is at implementation revision R2 and holds test evidence for R1 only. One run, one
//! case, six observations, in the story's order:
//!
//! 1. the stale R1 evidence leaves `tests.pass` `Unknown` at R2, not `True`;
//! 2. `repository.merge` is neither admissible in the frontier nor projected as a tool, and a model
//!    that names it anyway proposes nothing;
//! 3. once a passing test result for R2 is admitted, the projected tool set changes and
//!    `repository.merge` is admissible but marked as requiring authority;
//! 4. a merge proposal whose model-generated arguments claim approval is not executed while the
//!    `AuthorityProvider` withholds authority;
//! 5. a proposal made against the superseded case revision is refused by the governor;
//! 6. the trusted action adapter records no invocation of an action the frontier current at the
//!    call does not admit.
//!
//! The model is scripted (a selector and an argument generator), since tests make no model call.
//! The adapter performs every action Loom performs, `repository.merge` included, so what keeps an
//! action from it is the governor, Commission and Loom, never the adapter's own filter. The runtime
//! reads the frontier as Loom's slice does ([`AsTheSliceReads`]): without `release.rollback`,
//! which needs a capability and which no effect of Loom performs.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::action_request::revalidate;
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequest, ActionRequestId, ActionRequestStale, ActionStatus, AgentRevisionId,
    AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, EvidenceData, EvidenceId,
    FrontierData, ObservationData, ObservationId, PrincipalId, RevalidateActionRequestOutcome,
    RunId, RunOutcome, RunOutcomeCaseMovedOn, RunOutcomeNeedsAuthority, Truth, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{EXECUTOR_SOURCE, LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::{AuthorityQuery, StaticAuthorityProvider};
use b10x_loom_executor::model::run::{CatalogueEntry, CatalogueEntryStatus, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::Value;

type Gov = CanonGovernor<MemoryCaseStore>;

const PROTOCOL: &str = "software-change@1";
const CASE: &str = "CHG-1842";
const PRODUCER: &str = "service:ci";
const INSPECT: &str = "repository.inspect";
const EDIT: &str = "implementation.edit";
const TESTS_WRITE: &str = "tests.write";
const ROLLBACK: &str = "release.rollback";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const TESTS_PASS: &str = "tests.pass";
const WITHHELD: &str = "merging CHG-1842 needs the release manager's approval";

#[test]
fn software_change_case_on_the_aep_governor_keeps_merge_behind_fresh_evidence_and_authority() {
    let fixture = Fixture::load();
    let ids = Ids::default();
    let governor = Gov::new(MemoryCaseStore::default());
    let case = CaseId(CASE.to_owned());
    governor
        .open_case(
            case.clone(),
            PROTOCOL,
            fixture.revisions(),
            inputs_of(&fixture.0),
        )
        .expect("the fixture case opens on software.change/1");
    assert_eq!(
        governor.revisions(&case).expect("revisions")["implementation"],
        "R2"
    );
    let initial = fixture.evidence("initial");
    assert!(
        initial
            .iter()
            .any(|record| record["kind"] == "test_result" && record["subject_revision"] == "R1"),
        "the fixture's initial state holds test evidence for R1"
    );
    assert!(
        !initial
            .iter()
            .any(|record| record["kind"] == "test_result" && record["subject_revision"] == "R2"),
        "and none for R2"
    );
    for record in &initial {
        submit(&governor, &case, &ids, record);
    }
    let r2 = governor.current_revision(&case).expect("current revision");

    // 1. Stale R1 evidence: `tests.pass` is unknown at R2, not true.
    let first = AsTheSliceReads(&governor)
        .frontier(&case)
        .expect("frontier")
        .into_data();
    assert_eq!(first.case_revision, r2);
    assert_eq!(
        claim(&first, TESTS_PASS),
        Truth::Unknown,
        "a passing R1 test result says nothing about R2"
    );
    assert_eq!(
        claim(&first, "deployment.healthy"),
        Truth::True,
        "the initial records were read: the deployment observation beside the R1 result applies"
    );
    assert_eq!(
        claim(&r1_control(&fixture, &initial), TESTS_PASS),
        Truth::True,
        "the same records make `tests.pass` true for a case still at R1, so only staleness \
         leaves it unknown at R2"
    );

    // 2. `repository.merge` is neither admissible nor projected.
    assert_eq!(status(&first, MERGE), Some(ActionStatus::Blocked));
    let projected = project(&first);
    assert_eq!(
        projected,
        BTreeMap::from([
            (EDIT.to_owned(), CatalogueEntryStatus::Admissible),
            (INSPECT.to_owned(), CatalogueEntryStatus::Admissible),
            (TESTS_RUN.to_owned(), CatalogueEntryStatus::Admissible),
            (TESTS_WRITE.to_owned(), CatalogueEntryStatus::Admissible),
        ]),
        "the tool set at R2 with R1 evidence leaves merge out"
    );

    let commission = commission(&case);
    let authority = StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: WITHHELD.to_owned(),
        }),
    );
    let adapter = Adapter::new(&governor, &case, &ids);

    // The model names merge first, then runs the tests, then names merge again with arguments
    // that claim it was approved.
    let model = Model::picking([MERGE, TESTS_RUN, MERGE]);
    let loom = Loom::new(
        Selector(&model),
        ClaimsApproval,
        "fix CHG-1842 and merge it",
    )
    .with_governor(&governor)
    .with_instance("chg-1842-first-run");
    let first_run = drive(&governor, &loom, &authority, &adapter, &commission);

    let offered = model.offered();
    assert_eq!(offered.len(), 3, "Loom selected three times: {offered:?}");
    assert_eq!(
        offered[0], projected,
        "the first turn's tools are the catalogue projected from the frontier at R2"
    );
    let executor_steps = executor_outcomes(&governor.observations());
    assert_eq!(
        executor_steps.first().map(String::as_str),
        Some("NoUsefulAction"),
        "the model named merge while it was not projected, and Loom proposed nothing: \
         {executor_steps:?}"
    );
    assert_eq!(
        offered[1], projected,
        "nothing changed the tools before the tests ran"
    );

    // 3. After a passing R2 test result is admitted, the projected tool set changes.
    let second = first_run
        .last_frontier
        .clone()
        .expect("the run read a frontier");
    assert_eq!(
        second.case_revision, r2,
        "admitting evidence does not move the case"
    );
    assert_eq!(claim(&second, TESTS_PASS), Truth::True);
    assert_eq!(status(&second, MERGE), Some(ActionStatus::ApprovalRequired));
    assert_eq!(
        capability(&second, MERGE),
        Some(MERGE),
        "the frontier names the capability merge needs"
    );
    assert_ne!(offered[2], offered[0], "the projected tool set changed");
    assert_eq!(
        offered[2].get(MERGE),
        Some(&CatalogueEntryStatus::ApprovalRequired),
        "merge is projected, marked as requiring authority"
    );
    assert_eq!(offered[2], project(&second));

    // 4. Approval claimed in model-generated arguments is not authority.
    let requests: Vec<(&str, &RevalidateActionRequestOutcome)> = first_run
        .requests
        .iter()
        .map(|made| (made.request.action.as_str(), &made.outcome))
        .collect();
    assert_eq!(
        requests.len(),
        2,
        "the run made two action requests: {requests:?}"
    );
    assert_eq!(
        requests[0],
        (TESTS_RUN, &RevalidateActionRequestOutcome::Admitted)
    );
    let merge_request = &first_run.requests[1];
    assert_eq!(merge_request.request.action, MERGE);
    assert_eq!(
        merge_request.request.arguments.0,
        claimed_approval(),
        "the merge proposal carries the model's claim of approval"
    );
    match &merge_request.outcome {
        RevalidateActionRequestOutcome::NeedsAuthority { error } => {
            assert_eq!(error.capability, MERGE);
        }
        other => panic!("the merge request needs authority, got {other:?}"),
    }
    assert_eq!(
        authority.asked(),
        [AuthorityQuery {
            principal: commission.data().principal.clone(),
            authority_context: commission.data().authority_context.clone(),
            capability: MERGE.to_owned(),
        }],
        "the provider is asked once, about the commission's own principal and context"
    );
    assert_eq!(
        first_run.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: WITHHELD.to_owned(),
        }),
        "the run stops for the authority the provider withholds"
    );
    assert_eq!(
        actions_of(&first_run),
        [TESTS_RUN],
        "only the test run was admitted"
    );
    assert_eq!(
        adapter.invoked(),
        [TESTS_RUN],
        "the adapter never ran the merge"
    );

    // 5. A proposal against the superseded case revision is refused by the governor.
    let model = Model::picking([EDIT]);
    let ungoverned = Loom::new(
        Selector(&model),
        CaseMovesMeanwhile {
            governor: &governor,
            case: &case,
        },
        "fix CHG-1842",
    )
    .with_instance("chg-1842-second-run");
    let second_run = drive(&governor, &ungoverned, &authority, &adapter, &commission);
    let r3 = governor.current_revision(&case).expect("current revision");
    assert_eq!(r3, r2 + 1, "the push to R3 moved the case");
    let superseded = RevalidateActionRequestOutcome::Stale {
        error: ActionRequestStale {
            expected_case_revision: r2,
            current_case_revision: r3,
        },
    };
    let made: Vec<(&str, &RevalidateActionRequestOutcome)> = second_run
        .requests
        .iter()
        .map(|made| (made.request.action.as_str(), &made.outcome))
        .collect();
    assert_eq!(
        made,
        [(EDIT, &superseded)],
        "the edit proposed at R2 is refused once the case is at R3"
    );
    assert!(
        second_run.admitted.is_empty(),
        "nothing was admitted on the superseded revision"
    );
    assert_eq!(
        second_run.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: r2,
            current_case_revision: r3,
        })
    );
    assert_eq!(
        revalidate(
            &governor,
            &ActionRequest::new(merge_request.request.clone())
        )
        .expect("the governor answers"),
        superseded,
        "the merge request made at R2 is refused at R3 too"
    );
    let third = governor.frontier(&case).expect("frontier").into_data();
    assert_eq!(
        claim(&third, TESTS_PASS),
        Truth::Unknown,
        "the R2 result is stale at R3"
    );
    assert_eq!(status(&third, MERGE), Some(ActionStatus::Blocked));

    // 6. The adapter was invoked only for what the frontier current at the call admitted.
    let calls = adapter.calls();
    assert_eq!(
        calls
            .iter()
            .map(|call| call.action.as_str())
            .collect::<Vec<_>>(),
        [TESTS_RUN],
        "one invocation in both runs"
    );
    for call in &calls {
        assert_eq!(
            call.current,
            Some(ActionStatus::Admissible),
            "{} was invoked while the current frontier did not admit it",
            call.action
        );
        assert_eq!(
            call.frontier_revision, call.expected_case_revision,
            "{} was invoked on a frontier for another revision than its request's",
            call.action
        );
    }
}

/// Runs one loop of `commission` with `executor`.
fn drive<E: b10x_loom_commission::ports::executor::AgentExecutor>(
    governor: &Gov,
    executor: &E,
    authority: &StaticAuthorityProvider,
    adapter: &Adapter<'_>,
    commission: &Commission<commission_state::Assigned>,
) -> LoopEnd {
    static RUNS: AtomicU64 = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(|| {
        RunId(uuid(RUNS.fetch_add(1, Ordering::SeqCst)))
    }));
    run_until_blocked(
        &AsTheSliceReads(governor),
        executor,
        authority,
        adapter,
        commission,
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}

/// The `tests.pass` value a case still at implementation R1 gets from `records`.
fn r1_control(fixture: &Fixture, records: &[Value]) -> FrontierData {
    let governor = Gov::new(MemoryCaseStore::default());
    let case = CaseId(format!("{CASE}-R1"));
    let mut revisions = fixture.revisions();
    revisions.insert("implementation".to_owned(), "R1".to_owned());
    governor
        .open_case(case.clone(), PROTOCOL, revisions, inputs_of(&fixture.0))
        .expect("the control case opens");
    let ids = Ids::default();
    for record in records {
        submit(&governor, &case, &ids, record);
    }
    governor.frontier(&case).expect("frontier").into_data()
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    let context = json::parse(r#"{"tenant":"tenant-a","delegation":"change-team"}"#)
        .expect("authority context is JSON");
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: case.clone(),
        principal: PrincipalId("principal:change-agent".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

/// The arguments the model generates for a merge: a claim that it was approved.
fn claimed_approval() -> json::Value {
    json::parse(r#"{"approved":true,"approval":{"decision":"granted","by":"release-manager"}}"#)
        .expect("arguments are JSON")
}

fn claim(frontier: &FrontierData, name: &str) -> Truth {
    frontier
        .claims
        .iter()
        .find(|claim| claim.claim == name)
        .unwrap_or_else(|| panic!("the frontier lists claim {name}: {:?}", frontier.claims))
        .value
}

fn status(frontier: &FrontierData, action: &str) -> Option<ActionStatus> {
    frontier
        .actions
        .iter()
        .find(|listed| listed.action == action)
        .map(|listed| listed.status)
}

fn capability<'f>(frontier: &'f FrontierData, action: &str) -> Option<&'f str> {
    frontier
        .actions
        .iter()
        .find(|listed| listed.action == action)
        .and_then(|listed| listed.capability.as_deref())
}

/// The tool set Loom projects from `frontier`.
fn project(frontier: &FrontierData) -> BTreeMap<String, CatalogueEntryStatus> {
    let id = b10x_loom_executor::model::primitives::Uuid(frontier.frontier_id.0.0.clone());
    let catalogue = b10x_loom_executor::projection::project(
        &b10x_loom_commission::model::responsibility::Frontier::new(frontier.clone()),
        b10x_loom_executor::model::run::CatalogueId(id.clone()),
        b10x_loom_executor::model::run::TurnId(id),
    );
    tools(&catalogue.data().entries)
}

fn tools(entries: &[CatalogueEntry]) -> BTreeMap<String, CatalogueEntryStatus> {
    entries
        .iter()
        .map(|entry| (entry.action.clone(), entry.status))
        .collect()
}

fn actions_of(end: &LoopEnd) -> Vec<&str> {
    end.admitted
        .iter()
        .map(|request| request.action.as_str())
        .collect()
}

/// The `outcome` of every executor observation, in order.
fn executor_outcomes(observations: &[ObservationData]) -> Vec<String> {
    observations
        .iter()
        .filter(|observation| observation.source == EXECUTOR_SOURCE)
        .filter_map(|observation| match &observation.payload {
            json::Value::Object(members) => members.iter().find_map(|(key, value)| match value {
                json::Value::Text(text) if key == "outcome" => Some(text.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Distinct UUIDs, in order.
#[derive(Default)]
struct Ids(AtomicU64);

impl Ids {
    fn next(&self) -> Uuid {
        uuid(0x1000 + self.0.fetch_add(1, Ordering::SeqCst))
    }
}

/// Submits `record` through Commission's `submit_evidence`, bound to the case's current revision.
fn submit(governor: &Gov, case: &CaseId, ids: &Ids, record: &Value) {
    let facts = json::parse(&record.to_string()).expect("facts are JSON");
    let evidence_id = EvidenceId(ids.next());
    let observation = ObservationId(ids.next());
    observe_at(governor, &observation, record);
    let evidence = EvidenceData {
        evidence_id,
        case_id: case.clone(),
        kind: record["kind"].as_str().expect("kind").to_owned(),
        subject_revision: governor.current_revision(case).expect("current revision"),
        producer: String::new(),
        observation_ids: vec![observation],
        facts,
        provenance: json::Value::Object(vec![(
            "source".to_owned(),
            json::Value::Text("software_change_slice".to_owned()),
        )]),
    };
    submit_evidence(governor, PRODUCER, evidence).expect("the governor takes the evidence");
}

/// The model's choices: one scripted pick per selection, recording the tools it was offered.
struct Model {
    picks: Mutex<VecDeque<&'static str>>,
    offered: Mutex<Vec<BTreeMap<String, CatalogueEntryStatus>>>,
}

impl Model {
    fn picking(picks: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            picks: Mutex::new(picks.into_iter().collect()),
            offered: Mutex::default(),
        }
    }

    /// The tools of every selection, in order.
    fn offered(&self) -> Vec<BTreeMap<String, CatalogueEntryStatus>> {
        lock(&self.offered).clone()
    }
}

/// The model as Loom's selector. It names its scripted pick whether or not it is offered; once
/// the script is used up it picks nothing.
struct Selector<'m>(&'m Model);

impl ActionSelector for Selector<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        lock(&self.0.offered).push(tools(candidates));
        lock(&self.0.picks)
            .pop_front()
            .map(|action| Choice {
                action: action.to_owned(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// The model's arguments: for a merge, a claim that it was approved; otherwise none.
struct ClaimsApproval;

impl ArgumentGenerator for ClaimsApproval {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        Ok(if entry.action == MERGE {
            claimed_approval()
        } else {
            json::Value::Object(Vec::new())
        })
    }
}

/// The model's arguments, generated while somebody pushes implementation R3 to the case.
struct CaseMovesMeanwhile<'g> {
    governor: &'g Gov,
    case: &'g CaseId,
}

impl ArgumentGenerator for CaseMovesMeanwhile<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        self.governor
            .update_revision(self.case, "implementation", "R3")
            .map_err(|error| error.to_string())?;
        Ok(json::Value::Object(Vec::new()))
    }
}

/// One invocation of the adapter, with the frontier current at the call.
#[derive(Debug, Clone)]
struct Call {
    action: String,
    expected_case_revision: i64,
    /// The action's status in the frontier current at the call; `None` when it is not listed.
    current: Option<ActionStatus>,
    frontier_revision: i64,
}

/// The trusted action adapter: performs every action of `software.change/1` Loom performs
/// (every one but `release.rollback`) and records
/// each invocation against the frontier current at the call. A test run submits the passing
/// result for the implementation revision the case holds, as the local slice submits evidence from
/// the test command it runs itself.
struct Adapter<'g> {
    governor: &'g Gov,
    case: &'g CaseId,
    ids: &'g Ids,
    calls: Mutex<Vec<Call>>,
}

impl<'g> Adapter<'g> {
    fn new(governor: &'g Gov, case: &'g CaseId, ids: &'g Ids) -> Self {
        Self {
            governor,
            case,
            ids,
            calls: Mutex::default(),
        }
    }

    fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }

    fn invoked(&self) -> Vec<String> {
        self.calls().into_iter().map(|call| call.action).collect()
    }
}

impl EffectPort for Adapter<'_> {
    fn performs(&self, action: &str) -> bool {
        [INSPECT, TESTS_WRITE, EDIT, TESTS_RUN, MERGE].contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let request = request.data();
        let current = self
            .governor
            .frontier(self.case)
            .map_err(|error| EffectError::new(format!("{error:?}")))?
            .into_data();
        lock(&self.calls).push(Call {
            action: request.action.clone(),
            expected_case_revision: request.expected_case_revision,
            current: status(&current, &request.action),
            frontier_revision: current.case_revision,
        });
        if request.action == TESTS_RUN {
            let revisions = self
                .governor
                .revisions(self.case)
                .map_err(|error| EffectError::new(format!("{error:?}")))?;
            let record = serde_json::json!({
                "format": "canon-evidence/1",
                "id": format!("tests-{}", revisions["implementation"]),
                "kind": "test_result",
                "result": "pass",
                "subject": "implementation",
                "subject_revision": revisions["implementation"],
                "observed_at": "2026-10-04T11:00:00Z",
            });
            submit(self.governor, self.case, self.ids, &record);
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: json::Value::Text(format!("performed {}", request.action)),
            attempt: None,
            audit: None,
        }))
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-04T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
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
        assert_eq!(fixture["id"], "chg-1842");
        assert_eq!(fixture["case"]["id"], CASE);
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

    /// The evidence records state `id` adds, each with its `observed_at`.
    fn evidence(&self, id: &str) -> Vec<Value> {
        let state = self.0["states"]
            .as_array()
            .expect("states")
            .iter()
            .find(|state| state["id"] == id)
            .unwrap_or_else(|| panic!("the fixture has state {id}"));
        state["add_evidence"]
            .as_array()
            .expect("add_evidence")
            .iter()
            .map(|entry| {
                let mut record = entry["record"].clone();
                record["observed_at"] = entry["observed_at"].clone();
                record
            })
            .collect()
    }
}

/// The case inputs a fixture's case gives; a value YAML reads as a boolean is its text.
fn inputs_of(fixture: &serde_json::Value) -> loom_governor::CaseInputs {
    let pairs: Vec<(String, String)> = fixture["case"]["inputs"]
        .as_object()
        .expect("case inputs")
        .iter()
        .map(|(input, value)| {
            let value = match value {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (input.clone(), value)
        })
        .collect();
    loom_governor::CaseInputs::try_from_pairs(pairs).expect("distinct inputs")
}

/// The governor as Loom's slice reads it for Commission's runtime (`crates/loom-intake-slice`,
/// `run::Reads`): `release.rollback`, which needs a capability and which no effect of Loom
/// performs, in a run or in a resume, is left out of every frontier, so it is never offered,
/// awaited or held. Everything else, and every write, is the governor's.
struct AsTheSliceReads<'g>(&'g Gov);

impl Governor for AsTheSliceReads<'_> {
    fn current_revision(
        &self,
        case: &CaseId,
    ) -> Result<i64, b10x_loom_commission::model::responsibility::GovernorError> {
        self.0.current_revision(case)
    }

    fn frontier(
        &self,
        case: &CaseId,
    ) -> Result<
        b10x_loom_commission::model::responsibility::Frontier<
            b10x_loom_commission::model::responsibility::frontier_state::Issued,
        >,
        b10x_loom_commission::model::responsibility::GovernorError,
    > {
        let mut data = self.0.frontier(case)?.into_data();
        data.actions.retain(|action| action.action != ROLLBACK);
        Ok(b10x_loom_commission::model::responsibility::Frontier::new(
            data,
        ))
    }

    fn completion(
        &self,
        case: &CaseId,
    ) -> Result<
        b10x_loom_commission::model::responsibility::CompletionDetermination,
        b10x_loom_commission::model::responsibility::GovernorError,
    > {
        self.0.completion(case)
    }
}

impl ObservationPort for AsTheSliceReads<'_> {
    fn observe(
        &self,
        observation: b10x_loom_commission::model::responsibility::Observation<
            b10x_loom_commission::model::responsibility::observation_state::Reported,
        >,
    ) -> Result<(), b10x_loom_commission::model::responsibility::GovernorError> {
        self.0.observe(observation)
    }
}

/// Delivers `observation` at `record`'s `observed_at`, as a host does before it submits the record:
/// the governor takes a record's `observed_at` only as the instant of the observation it cites.
fn observe_at(
    governor: &impl b10x_loom_commission::ports::evidence::ObservationPort,
    observation: &ObservationId,
    record: &Value,
) {
    // The governor refuses a record citing an observation it never received, with or
    // without an `observed_at`; a record without one is observed at a fixed instant.
    let at = record["observed_at"]
        .as_str()
        .unwrap_or("2026-01-01T00:00:00Z");
    governor
        .observe(
            b10x_loom_commission::model::responsibility::Observation::new(
                b10x_loom_commission::model::responsibility::ObservationData {
                    observation_id: observation.clone(),
                    source: "host/fixture".to_owned(),
                    subject: record["subject"]
                        .as_str()
                        .unwrap_or("implementation")
                        .to_owned(),
                    observed_at: b10x_loom_commission::model::primitives::Timestamp(at.to_owned()),
                    payload: json::Value::Null,
                },
            ),
        )
        .expect("the governor keeps the observation");
}
