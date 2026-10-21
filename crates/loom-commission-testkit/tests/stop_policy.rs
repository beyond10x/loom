//! Acceptance for `story:stop-policy-gate-and-idle-bound`: a host chooses, per run, through
//! [`LoopContext::stop_policy`], whether an unchanged frontier listing actions that need approval
//! ends the run `AwaitingApproval` (the approval gate), and how many idle iterations in a row end
//! it `IdleBound`, with or without a step budget (`ess/commission/domains/responsibility.yaml`,
//! `StopPolicy`, `JudgeIdleIteration`).
//!
//! Turning the gate off touches no authority: a proposal of an action that needs approval is still
//! asked about, and one the provider does not allow is never handed to the effect port. A context
//! that does not implement `stop_policy` keeps the stops the runtime had before.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, BreachStep, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeAwaitingApproval, RunOutcomeIdleBound,
    RunOutcomeNeedsAuthority, RunOutcomeSuspended, StopPolicy, SuspensionReason, SuspensionState,
    Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked, run_with_suspension};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-21T09:00:00Z";

/// The action the frontier lists as needing approval, and its capability.
const MERGE: &str = "repository.merge";
const CAPABILITY: &str = "repository.merge";

/// An action the frontier admits, so a step that proposes nothing lets the run go on.
const INSPECT: &str = "repository.inspect";

/// What the provider asks for when it withholds the merge.
const APPROVE: &str = "approve the merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-stop-policy".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn admissible(name: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

/// The case open at `revision`, its frontier listing the merge as needing approval beside an
/// admissible inspection: the approval gate arms after every step on it.
fn gated(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![
            FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            },
            admissible(INSPECT),
        ],
    )
}

/// The case open at `revision`, its frontier listing only admissible actions: no gate arms.
fn open(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![admissible("edit"), admissible(INSPECT)],
    )
}

fn proposed(name: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    }
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn idle_bound(idle_iterations: i64) -> RunOutcome {
    RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations })
}

fn awaiting_merge() -> RunOutcome {
    RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
        actions: vec![MERGE.to_owned()],
    })
}

fn suspended_at(max_steps: usize) -> RunOutcome {
    RunOutcome::Suspended(RunOutcomeSuspended {
        reason: SuspensionReason::Budget(Value::Object(vec![(
            "max_steps".to_owned(),
            Value::Number(max_steps.to_string()),
        )])),
    })
}

fn policy(approval_gate: bool, idle_bound: Option<i64>) -> StopPolicy {
    StopPolicy {
        approval_gate,
        idle_bound,
        on_breach: BreachStep::DiscardStep,
    }
}

/// The ids a context issues, from counters starting at `0x500` and `0x600`.
#[derive(Debug)]
struct Ids {
    requests: u64,
    observations: u64,
}

impl Ids {
    fn new() -> Self {
        Self {
            requests: 0x500,
            observations: 0x600,
        }
    }

    fn request(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }
}

/// A context that chooses its stop policy, with `budget` as its step budget.
#[derive(Debug)]
struct Policied {
    ids: Ids,
    budget: Option<usize>,
    policy: StopPolicy,
}

impl Policied {
    fn new(budget: Option<usize>, policy: StopPolicy) -> Self {
        Self {
            ids: Ids::new(),
            budget,
            policy,
        }
    }
}

impl LoopContext for Policied {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation()
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }

    fn stop_policy(&self) -> StopPolicy {
        self.policy.clone()
    }
}

/// A context that does not implement `stop_policy`, with `budget` as its step budget.
#[derive(Debug)]
struct Unpolicied {
    ids: Ids,
    budget: Option<usize>,
}

impl Unpolicied {
    fn new(budget: Option<usize>) -> Self {
        Self {
            ids: Ids::new(),
            budget,
        }
    }
}

impl LoopContext for Unpolicied {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids.request()
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids.observation()
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// A fake effect port that performs every action, answers every invocation `Performed`, and
/// records each request it was handed.
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

    fn reads(&self, _action: &str) -> bool {
        false
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        Some(invoke_concurrently(self, commission, requests))
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

fn runs() -> RunStore {
    let mut issued = 0;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    })
}

/// Runs one loop on a fresh store with `context`, under `authority`.
fn run<C: LoopContext>(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Effects,
    case: &CaseId,
    context: &mut C,
) -> LoopEnd {
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut Generated::new(runs()),
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// With the gate off and an idle bound of one, a step that proposes nothing on a frontier listing
/// the merge as needing approval ends the run `IdleBound` after that one step, not
/// `AwaitingApproval`; the executor is called once and nothing is invoked. With the gate off and
/// no idle bound set, the unchanged frontier goes to the executor again, and the default bound of
/// two ends the run, where the gate would have ended it after one step.
#[test]
fn an_unchanged_frontier_without_the_gate_is_an_idle_iteration() {
    let case = CaseId("case-gate-off-bound-one".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(4)]);
    let executor = ScriptedExecutor::new([idle()]);
    let effects = Effects::default();

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Policied::new(None, policy(false, Some(1))),
    );

    assert_eq!(end.outcome, idle_bound(1), "the one idle step ends the run");
    assert_ne!(end.outcome, awaiting_merge());
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
    assert!(effects.invoked().is_empty(), "nothing is invoked");

    let case = CaseId("case-gate-off-default-bound".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(4)]);
    let executor = ScriptedExecutor::new([idle(), idle()]);

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Policied::new(None, policy(false, None)),
    );

    assert_eq!(
        end.outcome,
        idle_bound(2),
        "without the gate, the unchanged frontier is judged idle at the default bound"
    );
    assert_eq!(
        executor.calls().len(),
        2,
        "the unchanged frontier is handed to the executor again"
    );
    assert!(effects.invoked().is_empty(), "nothing is invoked");
}

/// With the gate off and a step budget, no idle bound set, a run whose steps leave the gated
/// frontier unchanged runs to its budget and is suspended for it, as a run on a frontier with no
/// gate is: the gate does not end it, and the default bound does not apply with a step budget.
#[test]
fn without_the_gate_a_budgeted_run_on_an_unchanged_frontier_reaches_its_budget() {
    let case = CaseId("case-gate-off-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(4)]);
    let executor = ScriptedExecutor::new([idle(), idle(), idle()]);
    let effects = Effects::default();

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Policied::new(Some(3), policy(false, None)),
    );

    assert_eq!(end.outcome, suspended_at(3));
    assert_eq!(executor.calls().len(), 3, "every step reaches the executor");
    assert_eq!(end.steps, 3);
}

/// With the gate off, an executor that proposes the merge is answered by the authority provider
/// all the same: a provider that asks for approval ends the run `NeedsAuthority`, naming its
/// request, and the effect port is never invoked; under `run_with_suspension` the commission's
/// Suspension holds the request, `Pending`, and nothing is performed. A provider that denies it
/// leaves it unperformed too, and the run goes on to its idle bound.
#[test]
fn an_ungranted_approval_required_action_is_still_refused_without_the_gate() {
    let withheld = || {
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: APPROVE.to_owned(),
        })
    };
    let needs = RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
        request: APPROVE.to_owned(),
    });

    // Without a suspension port.
    let case = CaseId("case-gate-off-withheld".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(6)]);
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(proposed(MERGE))]);
    let authority = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let effects = Effects::default();

    let end = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        &mut Policied::new(None, policy(false, Some(1))),
    );

    assert_eq!(end.outcome, needs, "the run ends needing authority");
    assert_eq!(
        authority.requests().len(),
        1,
        "the provider is asked about the merge"
    );
    assert_eq!(authority.requests()[0].action, MERGE);
    assert_eq!(
        effects.invoked().len(),
        0,
        "the effect port is never invoked for a withheld request"
    );
    assert!(end.admitted.is_empty(), "nothing is admitted");

    // Under `run_with_suspension`.
    let case = CaseId("case-gate-off-withheld-suspended".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(6)]);
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(proposed(MERGE))]);
    let authority = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();

    let end = run_with_suspension(
        &governor,
        &executor,
        &authority,
        &effects,
        &commission(&case),
        &mut runs(),
        &suspensions,
        &mut Policied::new(None, policy(false, Some(1))),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(end.outcome, needs, "the run ends needing authority");
    assert_eq!(
        effects.invoked().len(),
        0,
        "the effect port is never invoked for a withheld request"
    );
    let held = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("no Suspension holds the withheld request"));
    assert_eq!(held.state, SuspensionState::Pending);
    let made = &end.requests[0].request;
    assert_eq!(held.data.action_request_id, made.action_request_id);
    assert_eq!(made.action, MERGE);

    // A provider that denies it.
    let case = CaseId("case-gate-off-denied".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(6)]);
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(proposed(MERGE))]);
    let authority = StaticAuthorityProvider::new().answer(
        CAPABILITY,
        AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: "no merges today".to_owned(),
        }),
    );
    let effects = Effects::default();

    let end = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        &mut Policied::new(None, policy(false, Some(1))),
    );

    assert_eq!(
        end.outcome,
        idle_bound(1),
        "a denied proposal is an idle step, judged against the idle bound"
    );
    assert_eq!(authority.requests().len(), 1, "the provider is asked");
    assert_eq!(
        effects.invoked().len(),
        0,
        "the effect port is never invoked for a denied request"
    );
}

/// With a step budget of ten and an idle bound of one, one executor call that proposes nothing ends
/// the run `IdleBound` after that call, where before only the budget bounded a budgeted run. A
/// bound of three ends it after three idle calls.
#[test]
fn the_idle_bound_applies_with_a_step_budget_when_set() {
    let case = CaseId("case-budget-bound-one".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(2)]);
    let executor = ScriptedExecutor::new([idle()]);
    let effects = Effects::default();

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Policied::new(Some(10), policy(true, Some(1))),
    );

    assert_eq!(end.outcome, idle_bound(1));
    assert_eq!(executor.calls().len(), 1, "one executor call");
    assert!(effects.invoked().is_empty(), "nothing is invoked");

    let case = CaseId("case-budget-bound-three".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(2)]);
    let executor = ScriptedExecutor::new([idle(), idle(), idle()]);

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Policied::new(Some(10), policy(true, Some(3))),
    );

    assert_eq!(end.outcome, idle_bound(3));
    assert_eq!(executor.calls().len(), 3, "three executor calls");
}

/// An idle bound below one is read as one: a host cannot turn the bound off with it.
#[test]
fn an_idle_bound_below_one_is_read_as_one() {
    for (bound, budget) in [(0, None), (-3, None), (0, Some(10)), (i64::MIN, Some(10))] {
        let case = CaseId(format!("case-bound-{bound}-{budget:?}"));
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [open(2)]);
        let executor = ScriptedExecutor::new([idle()]);

        let end = run(
            &governor,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects::default(),
            &case,
            &mut Policied::new(budget, policy(true, Some(bound))),
        );

        assert_eq!(
            end.outcome,
            idle_bound(1),
            "an idle bound of {bound} with step budget {budget:?} is read as one"
        );
        assert_eq!(executor.calls().len(), 1);
    }
}

/// A context that does not implement `stop_policy` keeps today's stops: its policy is the gate on
/// and no idle bound; an unchanged gated frontier ends the run `AwaitingApproval` after one step;
/// without a step budget two idle steps end it `IdleBound`; with one, the budget alone bounds it.
#[test]
fn the_default_stop_policy_keeps_todays_stops() {
    assert_eq!(
        Unpolicied::new(None).stop_policy(),
        policy(true, None),
        "the provided policy keeps the gate and sets no idle bound"
    );
    let effects = Effects::default();

    let case = CaseId("case-default-gate".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated(3)]);
    let executor = ScriptedExecutor::new([idle()]);
    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Unpolicied::new(None),
    );
    assert_eq!(end.outcome, awaiting_merge(), "the gate ends the run");
    assert_eq!(executor.calls().len(), 1);

    let case = CaseId("case-default-idle".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(3)]);
    let executor = ScriptedExecutor::new([idle(), idle()]);
    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Unpolicied::new(None),
    );
    assert_eq!(end.outcome, idle_bound(2), "two idle steps end the run");
    assert_eq!(executor.calls().len(), 2);

    let case = CaseId("case-default-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open(3)]);
    let executor = ScriptedExecutor::new([idle(), idle(), idle()]);
    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut Unpolicied::new(Some(3)),
    );
    assert_eq!(end.outcome, suspended_at(3), "the budget alone bounds it");
    assert_eq!(executor.calls().len(), 3);
    assert!(effects.invoked().is_empty(), "nothing is invoked");
}
