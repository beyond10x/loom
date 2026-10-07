//! Adversary pass 2 on `story:effect-invocation` (wave 2026-10-07-w2), against `09c1c9e`.
//!
//! The correction filters the frontier an executor is handed action by action (`offered` in
//! `crates/loom-commission/src/runtime.rs`) and derives the run outcome from that handed frontier,
//! while the approval gate, the request and `LoopEnd::last_frontier` keep reading the governor's
//! frontier. These cases hold the split to what the story, the decision on
//! `decision-blocker:gated-unbound-action-visibility` (B) and the runtime's own documentation say:
//!
//! - the decision: "An action behind an authority gate, such as `repository.merge` on the local
//!   slice, stays in the frontier an executor sees". The shipped governor lists `repository.merge`
//!   `Blocked`, naming the capability `repository.merge`, until `implementation.verified` holds
//!   (`crates/loom-governor/tests/governed_case.rs`). The filter asks only whether an entry is
//!   `ApprovalRequired`, so it drops that merge;
//! - the runtime's documentation (step 6): "an action the executor was never offered keeps no run
//!   going". The approval gate still compares the governor's whole frontier, so a change in an
//!   action the executor never saw takes the run off its gate;
//! - the runtime's documentation (step 8): the effect observation carries `attempt` only "when the
//!   effect was invoked through a Connector".

use std::iter::repeat_n;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ActionRequestId, ActionStatus,
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ConnectorInstanceId, ConnectorOperationId, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction,
    FrontierData, ObservationData, ObservationId, PrincipalId, ProposedActionArguments, RunId,
    RunOutcome, RunOutcomeAwaitingApproval, Unit, action_binding_state, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::connector::ConnectorEffects;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::fake_invoker::RecordingInvoker;

const INSPECT: &str = "repository.inspect";
const EDIT: &str = "repository.edit";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const DEPLOY: &str = "service.deploy";
const SEARCH: &str = "logs.search";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: case.clone(),
        principal: PrincipalId("principal-b".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn binding(id: &CommissionId, action: &str) -> ActionBinding<action_binding_state::Declared> {
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: action.to_owned(),
        },
        commission_id: id.clone(),
        instance_id: ConnectorInstanceId("host".to_owned()),
        operation_id: ConnectorOperationId(format!("{action}.op")),
    })
}

fn entry(
    name: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
    }
}

fn admissible(name: &str) -> FrontierAction {
    entry(name, ActionStatus::Admissible, None, &[])
}

fn gated(name: &str) -> FrontierAction {
    entry(name, ActionStatus::ApprovalRequired, Some(name), &[])
}

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
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

#[derive(Default)]
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
        Timestamp("2026-10-07T13:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// An effect port performing exactly `performed`, answering every request `Performed` with
/// `attempt`.
struct Performs {
    performed: Vec<&'static str>,
    attempt: Option<&'static str>,
}

impl EffectPort for Performs {
    fn performs(&self, action: &str) -> bool {
        self.performed.contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if !self.performs(&request.data().action) {
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: "not performed".to_owned(),
            }));
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("done".to_owned()),
            attempt: self.attempt.map(|attempt| {
                b10x_loom_commission::model::responsibility::ConnectorAttemptId(attempt.to_owned())
            }),
        }))
    }
}

struct Ran {
    result: Result<LoopEnd, LoopError>,
    handed: Vec<FrontierData>,
    observations: Vec<ObservationData>,
}

impl Ran {
    fn end(&self) -> &LoopEnd {
        self.result
            .as_ref()
            .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
    }
}

fn run<F: EffectPort>(
    case: &CaseId,
    effects: &F,
    answers: Vec<Answer>,
    script: Vec<ExecutorOutcome>,
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
        &commission(case),
        &mut runs,
        &mut Context::default(),
    );
    Ran {
        result,
        handed: executor.frontiers(),
        observations: governor.observations(),
    }
}

/// The local slice's frontier before the tests pass, as the shipped governor issues it for
/// `software-change@1`: every declared action in id order, `repository.merge` `Blocked` by its
/// precondition and naming the capability it requires (`crates/loom-governor/src/lib.rs`, the
/// frontier; `crates/loom-governor/tests/governed_case.rs` asserts both the status and the
/// capability). The slice's effect port performs `repository.inspect`, `repository.edit` and
/// `tests.run` and nothing else (`crates/loom-intake-slice/src/effect.rs`, `performs`).
///
/// The decision on `decision-blocker:gated-unbound-action-visibility` (B) names this action on
/// this slice: it "stays in the frontier an executor sees". The story's filter removes only an
/// action the port does not perform "and that needs no authority"; this one needs
/// `repository.merge`. The slice's executor prints the frontier it is handed on every step
/// (`frontier: … repository.merge (blocked) …`, `website/docs/guides/run-an-intent.md`) and tells
/// the model why a merge it selects is refused from that frontier.
#[test]
fn the_merge_the_slice_lists_blocked_stays_in_the_frontier_the_executor_sees() {
    let case = CaseId("case-slice".to_owned());
    let merge = entry(
        MERGE,
        ActionStatus::Blocked,
        Some(MERGE),
        &[r#"{"claim":"implementation.verified","is":true,"value":"unknown"}"#],
    );
    let issued = vec![
        admissible(EDIT),
        admissible(INSPECT),
        merge.clone(),
        admissible(TESTS_RUN),
    ];
    let slice = Performs {
        performed: vec![INSPECT, EDIT, TESTS_RUN],
        attempt: None,
    };
    let ran = run(
        &case,
        &slice,
        vec![listing(1, issued.clone())],
        vec![idle(), idle()],
    );
    let end = ran.end();
    assert_eq!(
        end.last_frontier.as_ref().map(|frontier| &frontier.actions),
        Some(&issued),
        "precondition: the governor's frontier"
    );
    assert!(!ran.handed.is_empty(), "the executor never ran");
    for handed in &ran.handed {
        assert!(
            handed.actions.contains(&merge),
            "the executor was handed a frontier without the gated `{MERGE}` the governor lists \
             blocked: {:?}",
            handed
                .actions
                .iter()
                .map(|listed| (listed.action.as_str(), listed.status))
                .collect::<Vec<_>>()
        );
    }
}

/// An action the executor is never offered keeps no run going (`runtime.rs`, step 6). Here the
/// executor is handed the same frontier twice, `repository.edit` admissible and `service.deploy`
/// awaiting approval, and does nothing with it. Only `logs.search`, unbound, ungated and so never
/// offered, changes between the two frontiers the governor issues at revision 1 (its index goes
/// stale). The run should stop at the gate as the run without `logs.search` does:
/// `AwaitingApproval (service.deploy)` after one executor call. The gate compares the governor's
/// whole frontier, so the change it never showed the executor takes the run off the gate, the
/// executor is asked again about an unchanged frontier, and the idle rule ends the run
/// `NoAdmissibleAction`.
#[test]
fn an_action_never_offered_does_not_take_the_run_off_its_approval_gate() {
    let effects_for = |case: &CaseId| {
        let own = commission(case);
        let id = own.data().commission_id.clone();
        (own, [binding(&id, EDIT), binding(&id, DEPLOY)])
    };

    let case = CaseId("case-gate".to_owned());
    let (own, bindings) = effects_for(&case);
    let invoker = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&own, bindings, &invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let before = listing(1, vec![admissible(EDIT), gated(DEPLOY), admissible(SEARCH)]);
    let after = listing(
        1,
        vec![
            admissible(EDIT),
            gated(DEPLOY),
            entry(SEARCH, ActionStatus::Blocked, None, &["index stale"]),
        ],
    );
    let ran = run(
        &case,
        &effects,
        repeat_n(before, 3).chain([after]).collect(),
        vec![idle(), idle()],
    );

    let control_case = CaseId("case-gate-control".to_owned());
    let (control_own, control_bindings) = effects_for(&control_case);
    let control_invoker = RecordingInvoker::new();
    let control_effects = ConnectorEffects::new(&control_own, control_bindings, &control_invoker)
        .unwrap_or_else(|error| panic!("refused: {error}"));
    let control = run(
        &control_case,
        &control_effects,
        vec![listing(1, vec![admissible(EDIT), gated(DEPLOY)])],
        vec![idle(), idle()],
    );
    assert_eq!(
        (&control.end().outcome, control.handed.len()),
        (
            &RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
                actions: vec![DEPLOY.to_owned()],
            }),
            1
        ),
        "precondition: without `{SEARCH}` the run stops at the gate after one executor call"
    );

    let shown: Vec<Vec<FrontierAction>> = ran
        .handed
        .iter()
        .map(|handed| handed.actions.clone())
        .collect();
    assert!(
        shown.windows(2).all(|pair| pair[0] == pair[1]),
        "precondition: every frontier the executor was handed lists the same actions: {shown:?}"
    );
    assert_eq!(
        (&ran.end().outcome, ran.handed.len()),
        (&control.end().outcome, control.handed.len()),
        "the run outcome and the executor calls, when only an action the executor is never \
         offered changes"
    );
    assert!(invoker.calls().is_empty(), "{:?}", invoker.calls());
}

/// The effect observation carries `report` "with, when the effect was invoked through a Connector,
/// the one `attempt` the invocation produced" (`runtime.rs`, step 8). An effect port that reaches
/// no Connector, as the local slice's, answers `Performed` naming none, and its observation then
/// has no `attempt` member at all. The Connector case is held by `effect_invocation`; nothing
/// holds this one.
#[test]
fn a_performed_effect_naming_no_attempt_is_observed_without_an_attempt() {
    let case = CaseId("case-local".to_owned());
    let local = Performs {
        performed: vec![INSPECT],
        attempt: None,
    };
    let ran = run(
        &case,
        &local,
        vec![listing(1, vec![admissible(INSPECT)])],
        vec![proposal(INSPECT), idle(), idle()],
    );
    let end = ran.end();
    assert_eq!(
        end.effects.len(),
        1,
        "precondition: one effect: {:?}",
        end.effects
    );
    let effect: Vec<&ObservationData> = ran
        .observations
        .iter()
        .filter(|observed| observed.source == "effect")
        .collect();
    assert_eq!(effect.len(), 1, "{:?}", ran.observations);
    assert_eq!(
        effect[0].payload.member("report"),
        Some(&Value::Text("done".to_owned())),
        "the observation carries the report"
    );
    assert_eq!(
        effect[0].payload.member("attempt"),
        None,
        "an effect invoked through no Connector is observed without an attempt: {:?}",
        effect[0].payload
    );
}
