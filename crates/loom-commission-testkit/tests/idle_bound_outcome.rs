//! Acceptance for `story:idle-bound-outcome`: a run the idle bound ended has its own run outcome,
//! `RunOutcome::IdleBound`, naming the idle iterations in a row that ended it, and a run whose
//! frontier admits nothing keeps `NoAdmissibleAction` (`ess/commission/domains/responsibility.yaml`,
//! `RunOutcomeIdleBound`, `JudgeIdleIteration`).
//!
//! The idle rule is the one `runtime`'s module documentation states and does not change: without a
//! step budget, two iterations in a row that admit no new request end the run. On 0.26.0 such a
//! run ended `NoAdmissibleAction`, and a host could not tell it from a run with nothing to do.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId,
    EffectOutcome, EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeCaseMoved,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeIdleBound, RunState, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-20T12:00:00Z";

/// The capability the gated action of [`each_kind_of_idle_iteration_counts_toward_the_idle_bound`]
/// names.
const DEPLOY: &str = "deploy.run";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xd1)),
        agent_revision_id: AgentRevisionId(uuid(0xd2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-idle".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn admissible(name: &str) -> FrontierAction {
    action(name, ActionStatus::Admissible, None)
}

/// The case open at `revision`, its frontier listing exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposed(name: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    }
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(proposed(name))
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

/// The run outcome of a run the idle bound ended after `idle_iterations` idle iterations in a row.
fn idle_bound(idle_iterations: i64) -> RunOutcome {
    RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations })
}

/// The request the loop makes as its `n`-th, for `action`, at `revision`.
fn request(n: u64, run: &RunId, case: &CaseId, revision: i64, action: &str) -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(uuid(0x500 + n)),
        run_id: run.clone(),
        case_id: case.clone(),
        expected_case_revision: revision,
        action: action.to_owned(),
        arguments: proposed(action).arguments,
    }
}

/// The governor calls an iteration makes before it runs the executor.
fn iteration(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Completion(case.clone()),
        GovernorCall::Frontier(case.clone()),
    ]
}

/// The governor calls of a revalidation that is not stale.
fn revalidation(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Frontier(case.clone()),
    ]
}

/// New ids from counters starting at `0x500` and `0x600`, the one trusted time [`NOW`], and no
/// step budget: the idle bound applies.
#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
}

impl Context {
    fn new() -> Self {
        Self {
            requests: 0x500,
            observations: 0x600,
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// A fake effect port that performs every action, moves no case, answers every invocation
/// `Performed` with report `done <action>`, and records each request it was handed.
#[derive(Default)]
struct Effects {
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn invoked(&self) -> Vec<ActionRequestData> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        let report = Value::Text(format!("done {}", data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// Runs one loop with no step budget on a fresh store, under `authority`; returns its end and the
/// store.
fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Effects,
    case: &CaseId,
) -> (LoopEnd, Generated<RunStore>) {
    let mut issued = 0;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let end = run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut runs,
        &mut Context::new(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    (end, runs)
}

fn stored_state(runs: &Generated<RunStore>, run: &RunId) -> RunState {
    RunStorage::get(&runs.ports, run)
        .unwrap_or_else(|| panic!("the loop's Run {} is not stored", run.0.0))
        .state
}

/// An executor that proposes nothing twice, on a frontier that admits an action, ends the run with
/// the idle-bound outcome naming the two idle iterations, and not `NoAdmissibleAction`, which it
/// ended with on 0.26.0. The executor is called twice, nothing is invoked, the case is not loaded
/// again before the bound ends the run, the first iteration is the one step, and the Run is ended,
/// not suspended.
#[test]
fn two_idle_iterations_end_the_run_as_the_idle_bound() {
    let case = CaseId("case-idle-bound".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("edit"), admissible("inspect")])],
    );
    let executor = ScriptedExecutor::new([idle(), idle()]);
    let effects = Effects::default();

    let (end, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
    );

    assert_eq!(
        end.outcome,
        idle_bound(2),
        "the idle bound ends the run, naming the idle iterations"
    );
    assert_ne!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
    assert_eq!(executor.calls().len(), 2, "the executor is called twice");
    assert!(effects.invoked().is_empty(), "nothing is invoked");
    assert!(end.requests.is_empty(), "no request is made");
    assert_eq!(
        governor.calls(),
        [iteration(&case), iteration(&case)].concat(),
        "the case is not loaded again before the idle bound ends the run"
    );
    assert_eq!(end.steps, 1, "the iteration that ended the run is no step");
    assert_eq!(
        stored_state(&runs, &end.run_id),
        RunState::Running,
        "the Run is ended, not suspended"
    );
}

/// A frontier that admits nothing ends the run with `NoAdmissibleAction`, unchanged from 0.26.0,
/// after one executor call that proposed nothing and the one load that checks the case did not
/// move: whether its only action is blocked or it lists none. The idle bound is never reached.
#[test]
fn a_frontier_that_admits_nothing_ends_as_no_admissible_action() {
    for (name, actions) in [
        (
            "only a blocked action",
            vec![action("edit", ActionStatus::Blocked, None)],
        ),
        ("no action at all", Vec::new()),
    ] {
        let case = CaseId(format!("case-admits-nothing-{}", actions.len()));
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [listing(3, actions)]);
        let executor = ScriptedExecutor::new([idle()]);
        let effects = Effects::default();

        let (end, _) = run(
            &governor,
            &executor,
            &StaticAuthorityProvider::new(),
            &effects,
            &case,
        );

        assert_eq!(
            end.outcome,
            RunOutcome::NoAdmissibleAction(Unit(true)),
            "{name}: the frontier admits nothing"
        );
        assert_eq!(executor.calls().len(), 1, "{name}: one executor call");
        assert_eq!(
            governor.calls(),
            [
                iteration(&case),
                vec![GovernorCall::CurrentRevision(case.clone())]
            ]
            .concat(),
            "{name}: the case is loaded once more before the run ends"
        );
        assert_eq!(end.steps, 0, "{name}");
        assert!(effects.invoked().is_empty(), "{name}");
    }
}

/// A request with the action and arguments of one already admitted in the Run is no progress,
/// whatever the effect port answered for it: after the first, new, admission, two such
/// iterations in a row end the run with the idle-bound outcome. Each request is admitted and
/// performed once; nothing is refused.
#[test]
fn a_repeated_admission_counts_toward_the_idle_bound() {
    let case = CaseId("case-repeated-admission".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("edit")])]);
    let executor = ScriptedExecutor::new([proposal("edit"), proposal("edit"), proposal("edit")]);
    let effects = Effects::default();

    let (end, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
    );

    assert_eq!(
        end.outcome,
        idle_bound(2),
        "two repeated admissions in a row end the run on the idle bound"
    );
    assert_eq!(executor.calls().len(), 3);
    let made = vec![
        request(1, &end.run_id, &case, 4, "edit"),
        request(2, &end.run_id, &case, 4, "edit"),
        request(3, &end.run_id, &case, 4, "edit"),
    ];
    assert_eq!(
        effects.invoked(),
        made,
        "each admitted request is performed"
    );
    assert_eq!(end.admitted, made);
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
        ]
        .concat()
    );
    assert_eq!(end.steps, 2, "the third, idle, iteration ends the run");
    assert_eq!(stored_state(&runs, &end.run_id), RunState::Running);
}

/// A newly admitted request starts the count again: an idle iteration, a new admission and then
/// two idle iterations end the run after four executor calls, naming two idle iterations, not
/// three.
#[test]
fn a_newly_admitted_request_starts_the_count_again() {
    let case = CaseId("case-idle-count-restarts".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("edit")])]);
    let executor = ScriptedExecutor::new([idle(), proposal("edit"), idle(), idle()]);
    let effects = Effects::default();

    let (end, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
    );

    assert_eq!(end.outcome, idle_bound(2));
    assert_eq!(
        executor.calls().len(),
        4,
        "the new admission kept the run going"
    );
    assert_eq!(
        effects.invoked(),
        vec![request(1, &end.run_id, &case, 4, "edit")]
    );
    assert_eq!(end.steps, 3);
}

/// Every kind of iteration the module documentation counts idle ends the run on the idle bound
/// when two come in a row, on a frontier that still admits an action: `NoUsefulAction`,
/// `CompletedLocalReasoning`, a proposal the frontier refuses, one an authority verdict denies,
/// and a `CaseMoved` the governor does not bear out. Each run calls the executor twice and
/// invokes nothing. Only the denied proposal's frontier lists the gated action: on any other, a
/// step leaving it unchanged would end the run `AwaitingApproval` first.
#[test]
fn each_kind_of_idle_iteration_counts_toward_the_idle_bound() {
    let denied = AuthorityVerdict::Deny(AuthorityVerdictDeny {
        reason: "not this commission".to_owned(),
    });
    let kinds: [(&str, ExecutorOutcome); 5] = [
        ("no useful action", idle()),
        (
            "completed local reasoning",
            ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        ),
        ("a proposal the frontier refuses", proposal("merge")),
        ("a proposal the authority denies", proposal("deploy")),
        (
            "a move the governor does not bear out",
            ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
                expected_case_revision: 5,
            }),
        ),
    ];
    for (n, (name, first)) in kinds.into_iter().enumerate() {
        let case = CaseId(format!("case-idle-kind-{n}"));
        let mut actions = vec![admissible("edit")];
        if matches!(&first, ExecutorOutcome::ProposedAction(p) if p.action == "deploy") {
            actions.push(action(
                "deploy",
                ActionStatus::ApprovalRequired,
                Some(DEPLOY),
            ));
        }
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [listing(5, actions)]);
        let executor = ScriptedExecutor::new([first, idle()]);
        let authority = StaticAuthorityProvider::new().answer(DEPLOY, denied.clone());
        let effects = Effects::default();

        let (end, _) = run(&governor, &executor, &authority, &effects, &case);

        assert_eq!(end.outcome, idle_bound(2), "{name}");
        assert_eq!(executor.calls().len(), 2, "{name}: two executor calls");
        assert!(effects.invoked().is_empty(), "{name}: nothing is invoked");
        assert_eq!(end.steps, 1, "{name}");
    }
}
