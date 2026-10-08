//! Adversary pass 1 on `story:effect-invocation` (wave 2026-10-07-w2).
//!
//! The unit filters the frontier an executor is handed (`offered` in
//! `crates/loom-commission/src/runtime.rs`): it drops every entry whose action the effect port does
//! not perform and whose status is not `ApprovalRequired`. These cases hold that filter to the two
//! contracts it sits between:
//!
//! - the frontier contract (`docs/commission/contracts/frontier.md` § Invariant): a frontier may list
//!   one action more than once, and the least-authority entry decides. A filter that drops entries
//!   one by one can drop the deciding one, so the executor admits what the governor refuses;
//! - the story's outcome item 2: unbound actions that need no authority are never offered, so they
//!   are not candidates. The run outcome is still derived from the governor's whole frontier, so an
//!   action the executor was never offered keeps the run going (`derive` rule 6, "continue if it
//!   admits some action") until the idle rule or the step budget ends it.
//!
//! The binding cases pin what `ConnectorEffects::new` and `ConnectorEffects::invoke` promise for
//! inputs the unit's own suite does not build.

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ConnectorInstanceId, ConnectorOperationId, EffectOutcome, EffectOutcomeRefused,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction, FrontierData,
    FrontierObligation, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
    RunOutcomeNeedsExternalEvidence, Unit, action_binding_state, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::{BindingError, ConnectorEffects};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::fake_invoker::RecordingInvoker;

const INSPECT: &str = "repository.inspect";
const EDIT: &str = "repository.edit";
const SEARCH: &str = "logs.search";
const MERGE: &str = "repository.merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(n: u64, case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x80 + n)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(
    key: &CommissionId,
    field: &CommissionId,
    action: &str,
) -> ActionBinding<action_binding_state::Declared> {
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: key.clone(),
            action: action.to_owned(),
        },
        commission_id: field.clone(),
        instance_id: ConnectorInstanceId("source-host".to_owned()),
        operation_id: ConnectorOperationId(format!("{action}.op")),
        effect: b10x_loom_commission::model::responsibility::ConnectorOperationEffect::Write,
    })
}

fn entry(
    name: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reason: &str,
) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: if reason.is_empty() {
            Vec::new()
        } else {
            vec![reason.to_owned()]
        },
    }
}

fn admissible(name: &str) -> FrontierAction {
    entry(name, ActionStatus::Admissible, None, "")
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

struct Context {
    budget: Option<usize>,
    requests: u64,
    observations: u64,
}

impl Context {
    fn with_budget(budget: Option<usize>) -> Self {
        Self {
            budget,
            requests: 0,
            observations: 0,
        }
    }
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
        Timestamp("2026-10-07T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

struct Ran {
    result: Result<LoopEnd, LoopError>,
    handed: Vec<FrontierData>,
}

/// One loop over `answers`, the effect port `effects`, and an executor scripted with `script`.
fn run<F: EffectPort>(
    case: &CaseId,
    who: &Commission<commission_state::Assigned>,
    effects: &F,
    answers: Vec<Answer>,
    script: Vec<ExecutorOutcome>,
    budget: Option<usize>,
) -> Ran {
    let governor = FakeGovernor::new();
    governor.script(case.clone(), answers);
    let executor = ScriptedExecutor::new(script);
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        effects,
        who,
        &mut runs,
        &mut Context::with_budget(budget),
    );
    Ran {
        result,
        handed: executor.frontiers(),
    }
}

/// The frontier contract lets a frontier list one action more than once, and the least-authority
/// entry decides (`docs/commission/contracts/frontier.md` § Invariant, rule 1: any `Blocked` entry
/// refuses). Loom projects its catalogue with `admit` over the frontier it is handed
/// (`crates/loom-executor/src/projection.rs`). So the frontier handed over must admit each action
/// it lists exactly as the governor's frontier does; the filter may only remove actions, never
/// change how a kept one is admitted.
///
/// Here `repository.merge` is unbound and listed twice, `Blocked` and `ApprovalRequired`: the
/// governor's frontier refuses it. The filter drops the `Blocked` entry (unperformed, not
/// `ApprovalRequired`) and keeps the `ApprovalRequired` one, so the frontier the executor gets
/// admits the merge as needing authority: Loom would list it "admissible once authorized".
#[test]
fn the_frontier_handed_over_admits_each_kept_action_as_the_governor_frontier_does() {
    let case = CaseId("case-duplicate".to_owned());
    let own = commission(1, &case);
    let id = &own.data().commission_id;
    let invoker = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&own, [binding(id, id, INSPECT)], &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let actions = vec![
        admissible(INSPECT),
        entry(
            MERGE,
            ActionStatus::Blocked,
            None,
            "implementation.verified is Unknown, required True",
        ),
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.merge"),
            "",
        ),
    ];
    let issued = Frontier::new(FrontierData {
        frontier_id: b10x_loom_commission::model::responsibility::FrontierId(uuid(0x700)),
        case_id: case.clone(),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: actions.clone(),
    });
    let governor_admits = admit(&issued, MERGE);
    assert!(
        matches!(
            governor_admits,
            b10x_loom_commission::model::responsibility::Admission::Refused(_)
        ),
        "precondition: the governor's frontier refuses the merge: {governor_admits:?}"
    );

    let ran = run(
        &case,
        &own,
        &effects,
        vec![Answer::at(1).with_items(Vec::new(), Vec::new(), actions)],
        vec![idle(), idle()],
        None,
    );
    assert!(ran.result.is_ok(), "{:?}", ran.result);
    assert!(!ran.handed.is_empty(), "the executor never ran");
    for handed in &ran.handed {
        let handed = Frontier::new(handed.clone());
        for listed in &handed.data().actions {
            assert_eq!(
                admit(&handed, &listed.action),
                admit(&issued, &listed.action),
                "the frontier handed to the executor admits `{}` differently from the governor's \
                 frontier; handed: {:?}",
                listed.action,
                handed.data().actions
            );
        }
    }
    assert!(invoker.calls().is_empty(), "{:?}", invoker.calls());
}

/// The frontier the executor gets lists no admissible action (`repository.edit` is bound but
/// `Blocked`; `logs.search` is admissible but unbound and ungated, so it is never offered), and an
/// obligation is open. Only the governor can move this case. `derive` rule 6 says such a run ends
/// `NeedsExternalEvidence` carrying the open obligations; the runtime instead derives from the
/// governor's whole frontier, where the unoffered `logs.search` counts as admissible, so the run
/// continues and the executor is asked again and again for a frontier it can do nothing with.
///
/// With a step budget there is no idle rule, so the run spends the whole budget and ends
/// `Suspended` for `Budget`: the executor (a model, for Loom) is called once per budgeted step.
#[test]
fn a_frontier_offering_nothing_admissible_does_not_spend_the_step_budget() {
    let case = CaseId("case-budget".to_owned());
    let own = commission(1, &case);
    let id = &own.data().commission_id;
    let invoker = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&own, [binding(id, id, EDIT)], &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let frontier = Answer::at(1).with_items(
        Vec::new(),
        vec![FrontierObligation {
            obligation: "tests.pass".to_owned(),
            open: true,
        }],
        vec![
            admissible(SEARCH),
            entry(EDIT, ActionStatus::Blocked, None, "frozen"),
        ],
    );
    let budget = 4;
    let ran = run(
        &case,
        &own,
        &effects,
        vec![frontier],
        vec![idle(); budget],
        Some(budget),
    );
    let end = ran
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    for handed in &ran.handed {
        assert_eq!(
            handed
                .actions
                .iter()
                .map(|listed| listed.action.as_str())
                .collect::<Vec<_>>(),
            [EDIT],
            "precondition: the unbound action is never offered"
        );
    }
    assert_eq!(
        (&end.outcome, ran.handed.len()),
        (
            &RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
                requirements: vec!["tests.pass".to_owned()],
            }),
            1
        ),
        "the run outcome and the executor calls, on a frontier that offers the executor no \
         admissible action"
    );
    assert!(invoker.calls().is_empty(), "{:?}", invoker.calls());
}

/// The same frontier with no step budget: the idle rule ends the run after a second executor call,
/// `NoAdmissibleAction`, although an obligation is open and `derive` rule 6 names the outcome for
/// that, `NeedsExternalEvidence`.
#[test]
fn a_frontier_offering_nothing_admissible_ends_needing_external_evidence() {
    let case = CaseId("case-unbudgeted".to_owned());
    let own = commission(1, &case);
    let id = &own.data().commission_id;
    let invoker = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&own, [binding(id, id, EDIT)], &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let frontier = Answer::at(1).with_items(
        Vec::new(),
        vec![FrontierObligation {
            obligation: "tests.pass".to_owned(),
            open: true,
        }],
        vec![
            admissible(SEARCH),
            entry(EDIT, ActionStatus::Blocked, None, "frozen"),
        ],
    );
    let ran = run(
        &case,
        &own,
        &effects,
        vec![frontier],
        vec![idle(), idle()],
        None,
    );
    let end = ran
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        (&end.outcome, ran.handed.len()),
        (
            &RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
                requirements: vec!["tests.pass".to_owned()],
            }),
            1
        ),
        "the run outcome and the executor calls, on a frontier that offers the executor no \
         admissible action"
    );
}

/// `ConnectorEffects::new` refuses "a binding of another commission, by its key or its
/// `commission_id`" (`BindingError::OtherCommission`). The unit's suite builds the two cases where
/// `commission_id` names the other commission; none where only the key does, so dropping the key
/// half of the check leaves it green. A binding whose identity is another commission's is still
/// another commission's binding.
#[test]
fn a_binding_keyed_to_another_commission_is_refused_whatever_its_commission_field_says() {
    let case = CaseId("case-keyed".to_owned());
    let own = commission(1, &case);
    let other = commission(2, &case);
    let invoker = RecordingInvoker::new();
    let keyed = binding(
        &other.data().commission_id,
        &own.data().commission_id,
        INSPECT,
    );
    let data = keyed.data().clone();
    assert_eq!(
        ConnectorEffects::new(&own, [keyed], &invoker).err(),
        Some(BindingError::OtherCommission(Box::new(data))),
        "a binding whose key names another commission"
    );
}

/// The port's own refusal of an unbound action (`ConnectorEffects::invoke`, the `None` arm) is
/// reached when a host wraps `ConnectorEffects` in a port that answers `performs` more broadly:
/// the request is refused and the invoker is never called.
#[test]
fn an_unbound_request_reaching_connector_effects_is_refused_and_invokes_nothing() {
    struct Broad<'e, I: b10x_loom_commission::ports::connector::ConnectorInvoker> {
        inner: &'e ConnectorEffects<I>,
    }
    impl<I: b10x_loom_commission::ports::connector::ConnectorInvoker> EffectPort for Broad<'_, I> {
        fn performs(&self, _action: &str) -> bool {
            true
        }
        fn invoke(
            &self,
            commission: &Commission<commission_state::Assigned>,
            request: &AdmittedRequest,
        ) -> Result<EffectOutcome, EffectError> {
            self.inner.invoke(commission, request)
        }
    }

    let case = CaseId("case-broad".to_owned());
    let own = commission(1, &case);
    let id = &own.data().commission_id;
    let invoker = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&own, [binding(id, id, INSPECT)], &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let open = Answer::at(1).with_items(Vec::new(), Vec::new(), vec![admissible(SEARCH)]);
    let ran = run(
        &case,
        &own,
        &Broad { inner: &effects },
        vec![open],
        vec![proposal(SEARCH), idle(), idle()],
        None,
    );
    let end = ran
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        end.effects,
        vec![EffectOutcome::Refused(EffectOutcomeRefused {
            reason: format!("`{SEARCH}` has no binding"),
        })]
    );
    assert!(invoker.calls().is_empty(), "{:?}", invoker.calls());
}
