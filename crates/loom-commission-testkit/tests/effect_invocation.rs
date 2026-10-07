//! Acceptance for `story:effect-invocation` (Atlas ADR 0082): after the recheck, the Commission
//! runtime invokes the effect through the action's binding to one Connector operation.
//!
//! The commission's bindings (`commission.responsibility.ActionBinding`) are what the host's
//! composition declares: at most one Connector operation (`instance_id`, `operation_id`) per action
//! id of the commission. `ConnectorEffects` is the `EffectPort` over them; it invokes through the
//! `ConnectorInvoker` port, once per admitted request, and its `Performed` outcome names the one
//! Connector attempt the invocation produced. Before the executor runs, the runtime removes from the
//! frontier it hands over every action the port does not perform and that needs no authority; an
//! action behind an authority gate stays (`decision-blocker:gated-unbound-action-visibility`, B).
//!
//! `effect_invoked_only_through_its_binding` runs acceptance items 1-6 of the story, each under its
//! own name, with the scripted fake governor, the static fake authority provider, the recording
//! executor (`ScriptedExecutor::frontiers`) and the recording invoker (`RecordingInvoker`). A
//! journal notes the governor log's length at each executor, authority and invoker call, so the
//! order of the four is read off one record.

use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, ConnectorAttemptId,
    ConnectorInstanceId, ConnectorOperationId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction, FrontierData,
    ObservationData, ObservationId, PrincipalId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeAwaitingApproval,
    RunOutcomeCompleted, RunOutcomeNeedsAuthority, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::connector::{BindingError, ConnectorEffects, ConnectorInvoker};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::{AuthorityQuery, StaticAuthorityProvider};
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_commission_testkit::fake_invoker::{InvokerCall, RecordingInvoker};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-07T09:00:00Z";

/// A read action the commission binds.
const INSPECT: &str = "repository.inspect";
/// A consequential action the commission binds.
const EDIT: &str = "repository.edit";
/// A consequential action behind an authority gate that the commission binds.
const DEPLOY: &str = "service.deploy";
/// A read action the commission does not bind, and that needs no authority.
const SEARCH: &str = "logs.search";
/// An action behind an authority gate that the commission does not bind.
const MERGE: &str = "repository.merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission_numbered(n: u64, case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80 + n)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-e".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    commission_numbered(1, case)
}

fn binding(
    commission: &CommissionId,
    action: &str,
    instance: &str,
    operation: &str,
) -> ActionBinding<b10x_loom_commission::model::responsibility::action_binding_state::Declared> {
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: commission.clone(),
            action: action.to_owned(),
        },
        commission_id: commission.clone(),
        instance_id: ConnectorInstanceId(instance.to_owned()),
        operation_id: ConnectorOperationId(operation.to_owned()),
    })
}

/// The composition's bindings for `commission`: the read action, the consequential one and the
/// gated one, each to its own operation. `logs.search` and `repository.merge` are unbound.
fn bindings(
    commission: &Commission<commission_state::Assigned>,
) -> Vec<ActionBinding<b10x_loom_commission::model::responsibility::action_binding_state::Declared>>
{
    let id = &commission.data().commission_id;
    vec![
        binding(id, INSPECT, "source-host", "contents.read"),
        binding(id, EDIT, "source-host", "contents.write"),
        binding(id, DEPLOY, "deployer", "rollout.start"),
    ]
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

/// `name` behind the authority gate of the capability of the same name.
fn gated(name: &str) -> FrontierAction {
    action(name, ActionStatus::ApprovalRequired, Some(name))
}

/// The case open at `revision`, its frontier listing exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

fn approval_required(capability: &str) -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: format!("approve {capability}"),
    })
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

/// New ids from counters and the one trusted time [`NOW`]; no step budget.
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

/// One call the journal saw, with the governor log's length when it was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Executor(usize),
    Authority(usize),
    Invoke(usize),
}

/// The order of the executor, authority and invoker calls against the governor's log.
struct Journal<'g> {
    governor: &'g FakeGovernor,
    steps: Mutex<Vec<Step>>,
}

impl Journal<'_> {
    fn note(&self, step: fn(usize) -> Step) {
        let at = self.governor.calls().len();
        self.steps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(step(at));
    }

    fn steps(&self) -> Vec<Step> {
        self.steps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The recording executor, journaled.
struct Executor<'j> {
    journal: &'j Journal<'j>,
    inner: ScriptedExecutor,
}

impl AgentExecutor for Executor<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.journal.note(Step::Executor);
        self.inner.run(commission, frontier)
    }
}

/// The static authority provider, journaled.
struct Authority<'j> {
    journal: &'j Journal<'j>,
    inner: StaticAuthorityProvider,
}

impl AuthorityProvider for Authority<'_> {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        self.journal.note(Step::Authority);
        self.inner.decide(commission, capability)
    }
}

/// The recording invoker, journaled.
struct Invoker<'j> {
    journal: &'j Journal<'j>,
    inner: RecordingInvoker,
}

impl ConnectorInvoker for Invoker<'_> {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.journal.note(Step::Invoke);
        self.inner.invoke(binding, request)
    }
}

/// What one run left behind.
struct Ran {
    result: Result<LoopEnd, LoopError>,
    calls: Vec<GovernorCall>,
    journal: Vec<Step>,
    handed: Vec<FrontierData>,
    asked: Vec<AuthorityQuery>,
    invoked: Vec<InvokerCall>,
    observations: Vec<ObservationData>,
}

impl Ran {
    fn end(&self, name: &str) -> &LoopEnd {
        self.result
            .as_ref()
            .unwrap_or_else(|error| panic!("{name}: the loop failed: {error:?}"))
    }
}

/// Runs one loop of `commission` on `case`, its effect port `ConnectorEffects` over the bindings
/// [`bindings`] declares for `bound`.
fn run_case(
    case: &CaseId,
    commission: &Commission<commission_state::Assigned>,
    bound: &Commission<commission_state::Assigned>,
    answers: Vec<Answer>,
    script: Vec<ExecutorOutcome>,
    authority: StaticAuthorityProvider,
) -> Ran {
    let governor = FakeGovernor::new();
    governor.script(case.clone(), answers);
    let journal = Journal {
        governor: &governor,
        steps: Mutex::new(Vec::new()),
    };
    let executor = Executor {
        journal: &journal,
        inner: ScriptedExecutor::new(script),
    };
    let authority = Authority {
        journal: &journal,
        inner: authority,
    };
    let invoker = Invoker {
        journal: &journal,
        inner: RecordingInvoker::new(),
    };
    let effects = ConnectorEffects::new(bound, bindings(bound), &invoker)
        .unwrap_or_else(|error| panic!("the bindings are refused: {error}"));
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let result = run_until_blocked(
        &governor,
        &executor,
        &authority,
        &effects,
        commission,
        &mut runs,
        &mut Context::default(),
    );
    Ran {
        result,
        calls: governor.calls(),
        journal: journal.steps(),
        handed: executor.inner.frontiers(),
        asked: authority.inner.asked(),
        invoked: invoker.inner.calls(),
        observations: governor.observations(),
    }
}

fn names(frontier: &FrontierData) -> Vec<&str> {
    frontier
        .actions
        .iter()
        .map(|listed| listed.action.as_str())
        .collect()
}

/// The `Performed` outcome the recording invoker answers its `n`-th call with, for `binding`.
fn performed(n: usize, instance: &str, operation: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: RecordingInvoker::report(
            &ConnectorInstanceId(instance.to_owned()),
            &ConnectorOperationId(operation.to_owned()),
        ),
        attempt: Some(RecordingInvoker::attempt(n)),
    })
}

#[test]
fn effect_invoked_only_through_its_binding() {
    bound_request_invoked_once_through_its_binding_after_the_recheck();
    unbound_ungated_action_is_never_handed_to_the_executor();
    frontier_of_only_an_unbound_ungated_action_ends_no_performable_action();
    read_action_takes_the_path_of_a_consequential_one();
    refused_requests_invoke_nothing();
    unbound_gated_action_stays_visible_and_is_never_invoked();
}

/// 1. An admitted request for a bound action is invoked exactly once, through its bound
///    (`instance_id`, `operation_id`), after the governor and authority calls, and its `Performed`
///    outcome names exactly one attempt.
fn bound_request_invoked_once_through_its_binding_after_the_recheck() {
    let name = "1: bound, gated, allowed";
    let case = CaseId("case-deploy".to_owned());
    let open = |revision| listing(revision, vec![admissible(INSPECT), gated(DEPLOY)]);
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        repeat_n(open(3), 5)
            .chain([open(4), open(4).complete("deployed")])
            .collect(),
        vec![proposal(DEPLOY)],
        StaticAuthorityProvider::new().answer(DEPLOY, allow()),
    );
    let end = ran.end(name);

    assert_eq!(
        ran.journal,
        vec![Step::Executor(3), Step::Authority(5), Step::Invoke(5)],
        "{name}: executor after the iteration's reads, authority after the revalidation's, then \
         the invocation"
    );
    assert_eq!(
        ran.calls[..5],
        [iteration(&case), revalidation(&case)].concat(),
        "{name}: the governor calls before the invocation"
    );
    assert_eq!(ran.asked.len(), 1, "{name}: {:?}", ran.asked);
    assert_eq!(ran.asked[0].capability, DEPLOY, "{name}");

    assert_eq!(end.admitted.len(), 1, "{name}: {:?}", end.admitted);
    let admitted = &end.admitted[0];
    assert_eq!(admitted.action, DEPLOY, "{name}");
    assert_eq!(
        ran.invoked,
        vec![InvokerCall {
            instance_id: ConnectorInstanceId("deployer".to_owned()),
            operation_id: ConnectorOperationId("rollout.start".to_owned()),
            request: admitted.clone(),
        }],
        "{name}: invoked exactly once, through its binding"
    );
    let outcome = performed(1, "deployer", "rollout.start");
    assert_eq!(end.effects, vec![outcome], "{name}: the one outcome");
    let EffectOutcome::Performed(done) = &end.effects[0] else {
        panic!("{name}: not performed: {:?}", end.effects);
    };
    assert_eq!(
        done.attempt,
        Some(ConnectorAttemptId("attempt-1".to_owned())),
        "{name}: the attempt the invocation produced"
    );

    let effect: Vec<&ObservationData> = ran
        .observations
        .iter()
        .filter(|observed| observed.source == "effect")
        .collect();
    assert_eq!(effect.len(), 1, "{name}: {:?}", ran.observations);
    assert_eq!(
        effect[0].payload.member("attempt"),
        Some(&Value::Text("attempt-1".to_owned())),
        "{name}: the effect's observation names its attempt"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "deployed".to_owned()
        }),
        "{name}: outcome"
    );
}

/// 2. The executor is never handed an unbound action that needs no authority: with a frontier
///    listing one bound and one unbound ungated action, the frontier it receives lists only the
///    bound one. The governor's frontier is kept as it was read.
fn unbound_ungated_action_is_never_handed_to_the_executor() {
    let name = "2: bound and unbound, ungated";
    let case = CaseId("case-filtered".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![listing(2, vec![admissible(SEARCH), admissible(INSPECT)])],
        vec![idle(), idle()],
        StaticAuthorityProvider::new(),
    );
    let end = ran.end(name);

    assert_eq!(ran.handed.len(), 2, "{name}: executor calls");
    for handed in &ran.handed {
        assert_eq!(names(handed), [INSPECT], "{name}: the frontier handed over");
        assert_eq!(handed.case_revision, 2, "{name}");
    }
    let last = end
        .last_frontier
        .as_ref()
        .unwrap_or_else(|| panic!("{name}: no frontier read"));
    assert_eq!(
        names(last),
        [SEARCH, INSPECT],
        "{name}: the frontier as the governor issued it"
    );
    assert_eq!(
        ran.handed[1].frontier_id, last.frontier_id,
        "{name}: the same frontier, less the unbound action"
    );
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}

/// 3. A frontier whose only action is unbound and ungated ends `NoPerformableAction`, and nothing
///    is invoked.
fn frontier_of_only_an_unbound_ungated_action_ends_no_performable_action() {
    let name = "3: only unbound, ungated";
    let case = CaseId("case-unbound".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![listing(1, vec![admissible(SEARCH)])],
        Vec::new(),
        StaticAuthorityProvider::new(),
    );
    let end = ran.end(name);

    assert_eq!(ran.calls, iteration(&case), "{name}: one frontier");
    assert!(ran.handed.is_empty(), "{name}: the executor ran");
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert!(end.requests.is_empty(), "{name}");
    assert_eq!(
        end.outcome,
        RunOutcome::NoPerformableAction(Unit(true)),
        "{name}: outcome"
    );
}

/// 4. A bound read action (`repository.inspect`) is rechecked and invoked by the same path as a
///    consequential one (`repository.edit`).
fn read_action_takes_the_path_of_a_consequential_one() {
    let name = "4: read and consequential";
    let case = CaseId("case-read".to_owned());
    let open = listing(2, vec![admissible(INSPECT), admissible(EDIT)]);
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        repeat_n(open.clone(), 10)
            .chain([open.complete("edited")])
            .collect(),
        vec![proposal(INSPECT), proposal(EDIT)],
        StaticAuthorityProvider::new(),
    );
    let end = ran.end(name);

    assert_eq!(
        ran.journal,
        vec![
            Step::Executor(3),
            Step::Invoke(5),
            Step::Executor(8),
            Step::Invoke(10)
        ],
        "{name}: each invocation right after its revalidation"
    );
    assert_eq!(
        ran.calls[..10],
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case)
        ]
        .concat(),
        "{name}: the same governor calls for both"
    );
    for (made, revalidated) in end.requests.iter().zip(["read", "consequential"]) {
        assert_eq!(
            made.outcome,
            RevalidateActionRequestOutcome::Admitted,
            "{name}: the {revalidated} request"
        );
    }
    let through: Vec<(&str, &str, &str)> = ran
        .invoked
        .iter()
        .map(|call| {
            (
                call.request.action.as_str(),
                call.instance_id.0.as_str(),
                call.operation_id.0.as_str(),
            )
        })
        .collect();
    assert_eq!(
        through,
        [
            (INSPECT, "source-host", "contents.read"),
            (EDIT, "source-host", "contents.write")
        ],
        "{name}: each through its own binding"
    );
    assert_eq!(
        end.effects,
        vec![
            performed(1, "source-host", "contents.read"),
            performed(2, "source-host", "contents.write")
        ],
        "{name}: one attempt each"
    );
    assert!(ran.asked.is_empty(), "{name}: {:?}", ran.asked);
}

/// 5. A stale request, an authority deny and an approval-required each invoke nothing.
fn refused_requests_invoke_nothing() {
    let name = "5: stale";
    let case = CaseId("case-stale".to_owned());
    let at = |revision| listing(revision, vec![admissible(INSPECT)]);
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        repeat_n(at(5), 3).chain([at(6)]).collect(),
        vec![proposal(INSPECT)],
        StaticAuthorityProvider::new(),
    );
    let end = ran.end(name);
    assert_eq!(end.requests.len(), 1, "{name}");
    assert!(
        matches!(
            end.requests[0].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "{name}: {:?}",
        end.requests[0].outcome
    );
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert!(end.effects.is_empty(), "{name}");

    let gate = || listing(3, vec![admissible(INSPECT), gated(DEPLOY)]);

    let name = "5: denied";
    let case = CaseId("case-denied".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![gate()],
        vec![proposal(DEPLOY), idle()],
        StaticAuthorityProvider::new().answer(
            DEPLOY,
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: "not this week".to_owned(),
            }),
        ),
    );
    let end = ran.end(name);
    assert_eq!(ran.asked.len(), 1, "{name}: {:?}", ran.asked);
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert!(end.admitted.is_empty(), "{name}");
    assert!(end.effects.is_empty(), "{name}");

    let name = "5: approval required";
    let case = CaseId("case-approval".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![gate()],
        vec![proposal(DEPLOY)],
        StaticAuthorityProvider::new().answer(DEPLOY, approval_required(DEPLOY)),
    );
    let end = ran.end(name);
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert!(end.effects.is_empty(), "{name}");
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: format!("approve {DEPLOY}"),
        }),
        "{name}: outcome"
    );
}

/// 6. An unbound action behind an authority gate stays in the frontier the executor receives; the
///    run stops at the gate (`NeedsAuthority` on a proposal, `AwaitingApproval` on an unchanged
///    frontier), and once approved its invocation ends `NoPerformableAction` with nothing invoked.
fn unbound_gated_action_stays_visible_and_is_never_invoked() {
    let gate = || listing(4, vec![admissible(INSPECT), gated(MERGE)]);

    let name = "6: proposal stops at the gate";
    let case = CaseId("case-merge".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![gate()],
        vec![proposal(MERGE)],
        StaticAuthorityProvider::new().answer(MERGE, approval_required(MERGE)),
    );
    let end = ran.end(name);
    assert_eq!(ran.handed.len(), 1, "{name}");
    assert_eq!(
        names(&ran.handed[0]),
        [INSPECT, MERGE],
        "{name}: the gated action stays visible"
    );
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: format!("approve {MERGE}"),
        }),
        "{name}: outcome"
    );

    let name = "6: unchanged frontier stops at the gate";
    let case = CaseId("case-awaiting".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![gate()],
        vec![idle()],
        StaticAuthorityProvider::new(),
    );
    let end = ran.end(name);
    assert_eq!(ran.handed.len(), 1, "{name}");
    assert_eq!(names(&ran.handed[0]), [INSPECT, MERGE], "{name}");
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec![MERGE.to_owned()],
        }),
        "{name}: outcome"
    );

    let name = "6: approved, no binding";
    let case = CaseId("case-approved".to_owned());
    let ran = run_case(
        &case,
        &commission(&case),
        &commission(&case),
        vec![gate()],
        vec![proposal(MERGE)],
        StaticAuthorityProvider::new().answer(MERGE, allow()),
    );
    let end = ran.end(name);
    assert_eq!(ran.asked.len(), 1, "{name}: {:?}", ran.asked);
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
    assert!(end.admitted.is_empty(), "{name}: {:?}", end.admitted);
    assert!(end.effects.is_empty(), "{name}");
    assert_eq!(
        end.outcome,
        RunOutcome::NoPerformableAction(Unit(true)),
        "{name}: outcome"
    );
}

/// The composition declares at most one binding per action id of a commission, and only its own
/// commission's: a second binding for an action, or a binding of another commission, is refused
/// when the port is built.
#[test]
fn bindings_are_one_per_action_of_their_own_commission() {
    let case = CaseId("case-bindings".to_owned());
    let own = commission(&case);
    let other = commission_numbered(2, &case);
    let id = &own.data().commission_id;
    let invoker = RecordingInvoker::new();

    let effects = ConnectorEffects::new(&own, bindings(&own), &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    for (listed, performs) in [
        (INSPECT, true),
        (EDIT, true),
        (DEPLOY, true),
        (SEARCH, false),
        (MERGE, false),
    ] {
        assert_eq!(effects.performs(listed), performs, "{listed}");
    }

    let twice =
        bindings(&own)
            .into_iter()
            .chain([binding(id, EDIT, "mirror-host", "contents.write")]);
    assert_eq!(
        ConnectorEffects::new(&own, twice, &invoker).err(),
        Some(BindingError::Duplicate(EDIT.to_owned())),
        "a second binding for one action"
    );

    let foreign = binding(
        &other.data().commission_id,
        SEARCH,
        "log-store",
        "query.run",
    )
    .into_data();
    assert_eq!(
        ConnectorEffects::new(&own, [ActionBinding::new(foreign.clone())], &invoker).err(),
        Some(BindingError::OtherCommission(foreign)),
        "a binding of another commission"
    );

    let mut split = binding(id, SEARCH, "log-store", "query.run").into_data();
    split.commission_id = other.data().commission_id.clone();
    assert_eq!(
        ConnectorEffects::new(&own, [ActionBinding::new(split.clone())], &invoker).err(),
        Some(BindingError::OtherCommission(split)),
        "a binding whose commission is not its key's"
    );
    assert!(invoker.calls().is_empty());
}

/// A port built for one commission's bindings cannot answer for another commission: the run is
/// suspended with the effect failure, and nothing is invoked.
#[test]
fn an_invocation_for_another_commission_is_not_answered() {
    let name = "another commission";
    let case = CaseId("case-crossed".to_owned());
    let ran = run_case(
        &case,
        &commission_numbered(2, &case),
        &commission(&case),
        vec![listing(1, vec![admissible(INSPECT)])],
        vec![proposal(INSPECT)],
        StaticAuthorityProvider::new(),
    );
    let Err(error) = &ran.result else {
        panic!("{name}: the loop ended: {:?}", ran.result);
    };
    assert!(
        matches!(error.failure, LoopFailure::Effect(_)),
        "{name}: {error:?}"
    );
    assert!(error.suspension_reason.is_some(), "{name}: {error:?}");
    assert!(ran.invoked.is_empty(), "{name}: {:?}", ran.invoked);
}

/// Every `Performed` that `ConnectorEffects` returns names its attempt: an invoker that reports a
/// performed operation without one has failed to answer, so the run is suspended with the effect
/// failure and no effect is recorded as performed.
#[test]
fn a_performed_operation_that_names_no_attempt_is_not_answered() {
    let name = "no attempt";
    let case = CaseId("case-unattempted".to_owned());
    let own = commission(&case);
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(1, vec![admissible(INSPECT)])]);
    let executor = ScriptedExecutor::new([proposal(INSPECT), idle(), idle()]);
    let invoker =
        RecordingInvoker::new().answering([Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
        }))]);
    let effects = ConnectorEffects::new(&own, bindings(&own), &invoker)
        .unwrap_or_else(|error| panic!("{name}: the bindings are refused: {error}"));
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &own,
        &mut runs,
        &mut Context::default(),
    );
    let Err(error) = &result else {
        panic!("{name}: the loop ended: {result:?}");
    };
    assert!(
        matches!(error.failure, LoopFailure::Effect(_)),
        "{name}: {error:?}"
    );
    assert!(error.suspension_reason.is_some(), "{name}: {error:?}");
    assert_eq!(invoker.calls().len(), 1, "{name}: invoked once");
    assert!(
        governor
            .observations()
            .iter()
            .all(|observed| observed.source != "effect"),
        "{name}: an effect was observed: {:?}",
        governor.observations()
    );
}
