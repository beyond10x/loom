//! Boundaries of the read fan-out (`story:dw-read-fanout`,
//! `architecture-decision-record:read-fan-out`).
//!
//! The runtime's module docs state the Run's invariant: a Run holds the case at the revision it
//! started at, "or at a revision its own effect moved the case to". A fan-out whose revalidation
//! answers `Stale` for one read is the runtime's own proof that somebody else moved the case
//! before any read was invoked. These cases attack that, panics in one read, a read that needs
//! authority by revalidation, a port that reads but invokes no fan-out, and the step budget.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use b10x_loom_commission::model::responsibility::{
    ActionBinding, ActionBindingData, ActionBindingKey, ConnectorInstanceId,
    ConnectorOperationEffect, ConnectorOperationId, action_binding_state,
};
use b10x_loom_commission::ports::connector::{ConnectorEffects, ConnectorInvoker};
use b10x_loom_commission_testkit::fake_invoker::RecordingInvoker;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, ConnectorAuditRef, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, FrontierAction,
    ObservationId, PrincipalId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId,
    RunOutcome, RunOutcomeCaseMovedOn, SuspensionReason, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x91)),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adv".to_owned()),
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

fn gated(name: &str, capability: &str) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: Some(capability.to_owned()),
        reasons: Vec::new(),
    }
}

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposed(action: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(Value::Object(vec![(
            "source".to_owned(),
            Value::Text(action.to_owned()),
        )])),
    }
}

fn reads(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions.iter().map(|a| proposed(a)).collect(),
    })
}

fn performed(action: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text(format!("read {action}")),
        attempt: None,
        audit: Some(ConnectorAuditRef(format!("audit-{action}"))),
        usage: None,
    })
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x500 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x600 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-15T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A port that reads and performs `actions`; `panics` panics when invoked; `concurrent: false`
/// keeps the default `invoke_reads`.
struct Port {
    actions: Vec<&'static str>,
    panics: Option<&'static str>,
    concurrent: bool,
    handed: Mutex<Vec<String>>,
}

impl Port {
    fn new(actions: &[&'static str]) -> Self {
        Self {
            actions: actions.to_vec(),
            panics: None,
            concurrent: true,
            handed: Mutex::new(Vec::new()),
        }
    }

    fn handed(&self) -> Vec<String> {
        let mut handed = lock(&self.handed).clone();
        handed.sort();
        handed
    }
}

impl EffectPort for Port {
    fn performs(&self, action: &str) -> bool {
        self.actions.contains(&action)
    }

    fn reads(&self, action: &str) -> bool {
        self.actions.contains(&action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.clone();
        lock(&self.handed).push(action.clone());
        if self.panics == Some(action.as_str()) {
            panic!("{action} panicked inside the port");
        }
        Ok(performed(&action))
    }

    fn invoke_reads(
        &self,
        commission: &Commission<commission_state::Assigned>,
        requests: &[AdmittedRequest],
    ) -> Option<Vec<Result<EffectOutcome, EffectError>>> {
        self.concurrent
            .then(|| invoke_concurrently(self, commission, requests))
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x700 + issued))
    }))
}

fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Port,
    case: &CaseId,
    mut context: Context,
) -> Result<LoopEnd, LoopError> {
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut store(),
        &mut context,
    )
}

/// One read of a fan-out answers `Stale`: the case moved to revision 2 under somebody else before
/// any read was invoked. The other read is performed, as the ADR says. The runtime must not then
/// adopt revision 2 as a revision "its own effect moved the case to" (module docs, lines 5-6): a
/// read moves nothing, and the stale answer proves the move was not the Run's. A single proposal
/// that goes stale on the same script ends `CaseMovedOn` without a second executor call, and so
/// does the fan-out: the executor is not run again on the moved case.
#[test]
fn a_move_a_stale_read_reveals_is_not_adopted_as_the_runs_own() {
    let sources = ["logs.read", "metrics.read"];
    let open = |revision| listing(revision, sources.iter().map(|a| admissible(a)).collect());

    // Control: a single proposal that goes stale ends the run `CaseMovedOn` (items 2 and 8).
    let case = CaseId("case-adv-single-stale".to_owned());
    let governor = FakeGovernor::new();
    // Start, completion, frontier at 1; the revalidation's revision is 2, and stays 2.
    governor.script(case.clone(), [open(1), open(1), open(1), open(2)]);
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedAction(proposed("metrics.read")),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let effects = Port::new(&sources);
    let single = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    )
    .expect("control: the loop ends");
    let moved = RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
        bound_case_revision: 1,
        current_case_revision: 2,
    });
    assert_eq!(single.outcome, moved, "control: a stale single proposal");
    assert_eq!(executor.calls().len(), 1, "control: one executor call");

    // The fan-out: the first read is admitted at 1, the second read's revalidation finds 2.
    let case = CaseId("case-adv-fan-out-stale".to_owned());
    let governor = FakeGovernor::new();
    // Start, completion, frontier, first read's revision and frontier at 1; then 2 for good.
    governor.script(
        case.clone(),
        [open(1), open(1), open(1), open(1), open(1), open(2)],
    );
    let executor = ScriptedExecutor::new([
        reads(&sources),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let effects = Port::new(&sources);
    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    )
    .expect("the loop ends");

    assert_eq!(
        effects.handed(),
        vec!["logs.read"],
        "the stale read is not invoked"
    );
    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "{:?}",
        end.requests
    );
    assert_eq!(
        (end.outcome, executor.calls().len()),
        (moved, 1),
        "a move a stale read revealed was adopted as the Run's own: the executor ran again on \
         the moved case"
    );
}

/// A read that panics inside the port fails the loop as an effect error once the others joined;
/// the other read was handed once and is observed.
#[test]
fn a_panicking_read_fails_the_loop_after_the_others_joined() {
    let sources = ["logs.read", "metrics.read"];
    let case = CaseId("case-adv-panic".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(1, sources.iter().map(|a| admissible(a)).collect())],
    );
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let mut effects = Port::new(&sources);
    effects.panics = Some("metrics.read");

    let error = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    )
    .expect_err("a panicking read fails the loop");

    assert_eq!(
        error.failure,
        LoopFailure::Effect(EffectError::new("the read of `metrics.read` panicked"))
    );
    assert_eq!(effects.handed(), vec!["logs.read", "metrics.read"]);
    assert!(matches!(
        error.suspension_reason.as_deref(),
        Some(SuspensionReason::ExternalAvailability(_))
    ));
}

/// A read admissible on the handed frontier that needs authority by its revalidation is recorded
/// and never invoked, and no authority is asked: a fan-out never widens authority.
#[test]
fn a_read_needing_authority_by_revalidation_is_not_invoked_nor_asked() {
    let sources = ["logs.read", "audit.read"];
    let case = CaseId("case-adv-needs-authority".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(1, sources.iter().map(|a| admissible(a)).collect());
    let regated = listing(
        1,
        vec![admissible("logs.read"), gated("audit.read", "audit.grant")],
    );
    governor.script(
        case.clone(),
        [
            open.clone(),
            open.clone(),
            open.clone(),
            open.clone(),
            open.clone(),
            open.clone(),
            regated,
            open.complete("collected"),
        ],
    );
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let effects = Port::new(&sources);
    let authority = StaticAuthorityProvider::new();

    let end = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::default(),
    )
    .expect("the loop ends");

    assert_eq!(effects.handed(), vec!["logs.read"]);
    assert!(authority.asked().is_empty(), "authority asked in a fan-out");
    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::NeedsAuthority { .. }
        ),
        "{:?}",
        end.requests
    );
    assert_eq!(end.admitted.len(), 1);
}

/// A port that reads the actions but cannot invoke concurrently fails the loop, invoking nothing.
#[test]
fn a_port_reading_without_fan_out_invokes_nothing() {
    let sources = ["logs.read", "metrics.read"];
    let case = CaseId("case-adv-no-invoke-reads".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(1, sources.iter().map(|a| admissible(a)).collect())],
    );
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let mut effects = Port::new(&sources);
    effects.concurrent = false;

    let error = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    )
    .expect_err("the loop fails");
    assert!(
        matches!(error.failure, LoopFailure::Effect(_)),
        "{:?}",
        error.failure
    );
    assert!(effects.handed().is_empty());
}

/// With a step budget of one, a fan-out is the one step; a case still open is then suspended for
/// its budget, each read performed once.
#[test]
fn a_fan_out_is_one_step_of_a_budget_of_one() {
    let sources = ["a.read", "b.read", "c.read"];
    let case = CaseId("case-adv-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(1, sources.iter().map(|a| admissible(a)).collect())],
    );
    let executor = ScriptedExecutor::new([reads(&sources), reads(&sources)]);
    let effects = Port::new(&sources);

    let end = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context {
            budget: Some(1),
            ..Context::default()
        },
    )
    .expect("the loop ends");
    assert_eq!(end.steps, 1);
    assert_eq!(executor.calls().len(), 1);
    assert_eq!(effects.handed(), vec!["a.read", "b.read", "c.read"]);
    assert!(
        matches!(
            end.outcome,
            RunOutcome::Suspended(ref s) if matches!(s.reason, SuspensionReason::Budget(_))
        ),
        "{:?}",
        end.outcome
    );
}

// ---------------------------------------------------------------------------------------------
// Admission boundaries and the `ConnectorInvoker::concurrent` path.

fn run_over<F: EffectPort>(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &F,
    case: &CaseId,
) -> Result<LoopEnd, LoopError> {
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut store(),
        &mut Context::default(),
    )
}

/// A fan-out that lists the same read (action and arguments) twice. The story and ADR promise
/// "each [read invoked] once", and the specification refuses such a fan-out whole: one step never
/// hands the port the same read twice, and here hands it nothing.
#[test]
fn a_fan_out_listing_one_read_twice_does_not_invoke_it_twice() {
    let sources = ["logs.read", "metrics.read"];
    let case = CaseId("case-adv-duplicate".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(1, sources.iter().map(|a| admissible(a)).collect())],
    );
    let executor = ScriptedExecutor::new([
        reads(&["logs.read", "logs.read"]),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ]);
    let effects = Port::new(&sources);

    let _ = run_over(&governor, &executor, &effects, &case);

    let handed = effects.handed();
    assert!(
        handed.iter().filter(|a| *a == "logs.read").count() <= 1,
        "one fan-out handed the port the same read twice: {handed:?}"
    );
    assert!(
        handed.is_empty(),
        "a fan-out listing one read twice is refused whole: {handed:?}"
    );
}

/// A frontier entry that is `Admissible` but names a capability needs authority: the fan-out is
/// refused whole and nothing is invoked.
#[test]
fn an_admissible_entry_naming_a_capability_refuses_the_fan_out() {
    let case = CaseId("case-adv-capability".to_owned());
    let governor = FakeGovernor::new();
    let mut named = admissible("audit.read");
    named.capability = Some("audit.grant".to_owned());
    governor.script(
        case.clone(),
        [listing(1, vec![admissible("logs.read"), named])],
    );
    let executor = ScriptedExecutor::new([
        reads(&["logs.read", "audit.read"]),
        reads(&["logs.read", "audit.read"]),
        reads(&["logs.read", "audit.read"]),
    ]);
    let effects = Port::new(&["logs.read", "audit.read"]);

    let end = run_over(&governor, &executor, &effects, &case);

    assert!(
        effects.handed().is_empty(),
        "invoked: {:?}",
        effects.handed()
    );
    if let Ok(end) = end {
        assert!(end.requests.is_empty(), "{:?}", end.requests);
    }
}

/// An action the frontier lists twice, once `Admissible` and once `ApprovalRequired`, needs
/// authority: the fan-out is refused whole.
#[test]
fn a_read_listed_twice_once_gated_refuses_the_fan_out() {
    let case = CaseId("case-adv-twice-listed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            1,
            vec![
                admissible("logs.read"),
                admissible("audit.read"),
                gated("audit.read", "audit.grant"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([
        reads(&["logs.read", "audit.read"]),
        reads(&["logs.read", "audit.read"]),
    ]);
    let effects = Port::new(&["logs.read", "audit.read"]);

    let _ = run_over(&governor, &executor, &effects, &case);

    assert!(
        effects.handed().is_empty(),
        "invoked: {:?}",
        effects.handed()
    );
}

/// A `Sync` invoker that opts in through `concurrent`, every call waiting until `expected` calls
/// are under way together (or reporting that it ran alone after two seconds).
#[derive(Debug)]
struct MeetingInvoker {
    expected: usize,
    arrived: Mutex<usize>,
    all: Condvar,
    calls: Mutex<Vec<String>>,
}

impl MeetingInvoker {
    fn new(expected: usize) -> Self {
        Self {
            expected,
            arrived: Mutex::new(0),
            all: Condvar::new(),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl ConnectorInvoker for MeetingInvoker {
    fn invoke(
        &self,
        binding: &ActionBindingData,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        lock(&self.calls).push(binding.operation_id.0.clone());
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut arrived = lock(&self.arrived);
        *arrived += 1;
        self.all.notify_all();
        while *arrived < self.expected {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(EffectError::new(format!(
                    "{} ran alone",
                    request.data().action
                )));
            }
            arrived = self
                .all
                .wait_timeout(arrived, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        Ok(performed(&request.data().action))
    }

    fn concurrent(&self) -> Option<&(dyn ConnectorInvoker + Sync)> {
        Some(self)
    }
}

fn bound(
    commission: &Commission<commission_state::Assigned>,
    action: &str,
    effect: ConnectorOperationEffect,
) -> ActionBinding<action_binding_state::Declared> {
    let id = commission.data().commission_id.clone();
    ActionBinding::new(ActionBindingData {
        binding: ActionBindingKey {
            commission_id: id.clone(),
            action: action.to_owned(),
        },
        commission_id: id,
        instance_id: ConnectorInstanceId("adv-instance".to_owned()),
        operation_id: ConnectorOperationId(format!("op-{action}")),
        effect,
    })
}

/// `ConnectorEffects` over a reference to an opting-in invoker reads its `Read` bindings and
/// invokes three of them concurrently, each once; over an invoker that does not opt in
/// (`RecordingInvoker`) it reads nothing and the fan-out invokes nothing.
#[test]
fn connector_effects_fans_out_only_through_an_opting_in_invoker() {
    let sources = ["a.read", "b.read", "c.read"];
    let case = CaseId("case-adv-connector".to_owned());
    let held = commission(&case);
    let bindings = || {
        sources
            .iter()
            .map(|a| bound(&held, a, ConnectorOperationEffect::Read))
            .collect::<Vec<_>>()
    };

    let invoker = MeetingInvoker::new(sources.len());
    let effects = ConnectorEffects::new(&held, bindings(), &invoker).expect("bindings");
    let governor = FakeGovernor::new();
    let open = listing(1, sources.iter().map(|a| admissible(a)).collect());
    governor.script(
        case.clone(),
        std::iter::repeat_n(open.clone(), 9).chain([open.clone().complete("collected")]),
    );
    let executor = ScriptedExecutor::new([reads(&sources)]);
    let end = run_over(&governor, &executor, &effects, &case).expect("the loop ends");
    let mut calls = lock(&invoker.calls).clone();
    calls.sort();
    assert_eq!(calls, vec!["op-a.read", "op-b.read", "op-c.read"]);
    assert_eq!(
        end.effects,
        sources.iter().map(|a| performed(a)).collect::<Vec<_>>()
    );

    let recording = RecordingInvoker::new();
    let effects = ConnectorEffects::new(&held, bindings(), &recording).expect("bindings");
    assert!(sources.iter().all(|a| !effects.reads(a)));
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [open]);
    let executor = ScriptedExecutor::new([reads(&sources), reads(&sources)]);
    let _ = run_over(&governor, &executor, &effects, &case);
    assert!(recording.calls().is_empty(), "{:?}", recording.calls());
}
