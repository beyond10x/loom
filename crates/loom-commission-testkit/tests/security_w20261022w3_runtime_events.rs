//! Security review of `story:runtime-events` (wave 2026-10-22-w3): conformance checks of the
//! `RuntimeEvent` declaration in `ess/commission/domains/responsibility.yaml` against the runtime.
//!
//! Each case drives the real loop and reads what a host's sink received: events only for steps
//! that took place, in loop order, `Stopped` last and once, on the paths the implementor's suite
//! does not drive (a pending request invoked again, a fan-out, a host stop, an effect failure, a
//! resume whose case moved after its take); a sink that panics never lets a performed request be
//! performed again; and no event carries the commission's authority context.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EffectOutcomePending, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, ExecutorStep, Frontier,
    FrontierAction, FrontierId, HoldSuspension, HoldSuspensionOutcome, HostStop, ObservationId,
    PrincipalId, ProposedActionArguments, ReleaseSuspension, ReleaseSuspensionOutcome,
    RevalidationAnswer, RunId, RunOutcome, RuntimeEvent, SuspensionReason, SuspensionSnapshot,
    SuspensionState, TakeSuspension, TakeSuspensionOutcome, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::ports::events::EventSink;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStore, SuspensionStoreError,
};
use b10x_loom_commission::runtime::{
    LoopContext, ResumeError, ResumeRefusal, resume_after_grant, run_until_blocked,
    run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const MERGE: &str = "merge";
const INSPECT: &str = "inspect";
const LIST: &str = "list";
const CAPABILITY: &str = "repository.merge";
const WITHHELD: &str = "approve the merge";
const SINK_PANIC: &str = "security: the host's sink panicked";
/// A value only the host's verified authority context holds.
const AUTHORITY_MARKER: &str = "verified-subject-w3-marker";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    let context = json::parse(&format!(
        r#"{{"verified":{{"subject":"{AUTHORITY_MARKER}"}}}}"#
    ))
    .unwrap_or_else(|error| panic!("authority context fixture: {error:?}"));
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x3e1)),
        agent_revision_id: AgentRevisionId(uuid(0x3e2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-security-w3".to_owned()),
        authority_context: AuthorityContext(context),
    })
}

fn action(name: &str, status: ActionStatus) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        capability: (status == ActionStatus::ApprovalRequired).then(|| CAPABILITY.to_owned()),
        status,
        reasons: Vec::new(),
    }
}

fn at(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn arguments(action: &str) -> ProposedActionArguments {
    ProposedActionArguments(
        json::parse(&format!(r#"{{"what":"{action}"}}"#))
            .unwrap_or_else(|error| panic!("arguments fixture: {error:?}")),
    )
}

fn proposed(action: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: arguments(action),
    }
}

fn propose(action: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(proposed(action))
}

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

fn withheld() -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: WITHHELD.to_owned(),
    })
}

fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(issued))
    })
}

/// A loop context with a step budget, an event sink and, when given one, a host stop it always
/// answers.
struct Context<S> {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
    host_stop: Option<HostStop>,
    sink: S,
}

impl<S> Context<S> {
    fn new(base: u64, budget: Option<usize>, sink: S) -> Self {
        Self {
            requests: base,
            observations: base + 0x100,
            budget,
            host_stop: None,
            sink,
        }
    }
}

impl<S: EventSink> LoopContext for Context<S> {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-22T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }

    fn host_stop(&mut self) -> Option<HostStop> {
        self.host_stop.clone()
    }

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.sink
    }
}

/// A sink that keeps every event and panics on the first event of the kind it names.
struct PanicsOn {
    kind: &'static str,
    heard: Vec<RuntimeEvent>,
}

impl EventSink for PanicsOn {
    fn emit(&mut self, event: &RuntimeEvent) {
        self.heard.push(event.clone());
        if kind(event) == self.kind {
            panic!("{SINK_PANIC}");
        }
    }
}

/// An effect port that performs every action, reads [`INSPECT`] and [`LIST`], answers `Pending`
/// to the first `pending` invocations, and fails every invocation when `fails`.
#[derive(Default)]
struct Effects {
    invoked: Mutex<Vec<ActionRequestData>>,
    pending: Mutex<usize>,
    fails: bool,
}

impl Effects {
    fn pending_first(times: usize) -> Self {
        Self {
            pending: Mutex::new(times),
            ..Self::default()
        }
    }

    fn failing() -> Self {
        Self {
            fails: true,
            ..Self::default()
        }
    }

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

    fn reads(&self, action: &str) -> bool {
        action == INSPECT || action == LIST
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
        if self.fails {
            return Err(EffectError::new("security: the effect port failed"));
        }
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if *pending > 0 {
            *pending -= 1;
            return Ok(EffectOutcome::Pending(EffectOutcomePending {
                report,
                ends_run: false,
            }));
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// An executor that answers its script in order and records each frontier it was handed.
struct HandedExecutor {
    script: Mutex<VecDeque<ExecutorOutcome>>,
    handed: Mutex<Vec<(FrontierId, i64)>>,
}

impl HandedExecutor {
    fn new(script: impl IntoIterator<Item = ExecutorOutcome>) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            handed: Mutex::new(Vec::new()),
        }
    }

    fn handed(&self) -> Vec<(FrontierId, i64)> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AgentExecutor for HandedExecutor {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((
                frontier.data().frontier_id.clone(),
                frontier.data().case_revision,
            ));
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| panic!("HandedExecutor: the script is used up"))
    }

    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        ExecutorStep {
            outcome: self.run(commission, frontier),
            usage: None,
            transcript_digest: None,
            summary: None,
        }
    }
}

/// A suspension port that moves the case to `moved` as a resume takes the Suspension.
struct MovesOnTake<'a> {
    inner: &'a SuspensionStore,
    governor: &'a FakeGovernor,
    case: CaseId,
    moved: Answer,
    took: Mutex<bool>,
}

impl SuspensionPort for MovesOnTake<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        SuspensionPort::hold(self.inner, input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        let first = !std::mem::replace(
            &mut *self.took.lock().unwrap_or_else(PoisonError::into_inner),
            true,
        );
        if first {
            self.governor
                .script(self.case.clone(), [self.moved.clone()]);
        }
        SuspensionPort::take(self.inner, input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        SuspensionPort::release(self.inner, input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        SuspensionPort::get(self.inner, commission)
    }
}

fn kind(event: &RuntimeEvent) -> &'static str {
    match event {
        RuntimeEvent::StepStarted(_) => "StepStarted",
        RuntimeEvent::ActionSelected(_) => "ActionSelected",
        RuntimeEvent::Proposed(_) => "Proposed",
        RuntimeEvent::AuthorityDecided(_) => "AuthorityDecided",
        RuntimeEvent::EffectInvoked(_) => "EffectInvoked",
        RuntimeEvent::SessionClosed(_) => "SessionClosed",
        RuntimeEvent::Stopped(_) => "Stopped",
    }
}

fn kinds(events: &[RuntimeEvent]) -> Vec<&'static str> {
    events.iter().map(kind).collect()
}

/// `Stopped` is the last event and is emitted exactly once.
fn assert_stopped_last_once(events: &[RuntimeEvent]) {
    let stops = events
        .iter()
        .filter(|event| matches!(event, RuntimeEvent::Stopped(_)))
        .count();
    assert_eq!(stops, 1, "Stopped exactly once: {events:#?}");
    assert!(
        matches!(events.last(), Some(RuntimeEvent::Stopped(_))),
        "Stopped last: {events:#?}"
    );
}

/// Suspends a run of `case` on the withheld merge at revision 7, holding its Suspension.
fn suspend_merge(
    governor: &FakeGovernor,
    effects: &Effects,
    suspensions: &SuspensionStore,
    case: &CaseId,
) {
    governor.script(
        case.clone(),
        [at(7, vec![action(MERGE, ActionStatus::ApprovalRequired)])],
    );
    let end = run_with_suspension(
        governor,
        &ScriptedExecutor::new([propose(MERGE)]),
        &StaticAuthorityProvider::new().answer(CAPABILITY, withheld()),
        effects,
        &commission(case),
        &mut runs(0x3100),
        suspensions,
        &mut Context::new(0x3200, None, Vec::<RuntimeEvent>::new()),
    )
    .unwrap_or_else(|error| panic!("precondition: the first run failed: {error:?}"));
    assert!(
        matches!(end.outcome, RunOutcome::NeedsAuthority(_)),
        "precondition: the merge is held: {:?}",
        end.outcome
    );
}

/// Every executor call's `StepStarted` names the frontier that call was handed, and the revision
/// it acts at: the check `every_step_of_a_run_is_emitted_in_order` weakened to "differs from the
/// first step's", which a stale or invented frontier id would pass.
#[test]
fn security_step_started_names_the_frontier_each_executor_call_was_handed() {
    let case = CaseId("case-security-w3-handed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(
            7,
            vec![
                action(MERGE, ActionStatus::ApprovalRequired),
                action(INSPECT, ActionStatus::Admissible),
            ],
        )],
    );
    let executor = HandedExecutor::new([propose(MERGE), propose(INSPECT)]);
    let effects = Effects::default();
    let mut context = Context::new(0x3300, Some(2), Vec::<RuntimeEvent>::new());
    run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new().answer(CAPABILITY, allow()),
        &effects,
        &commission(&case),
        &mut runs(0x3400),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    let started: Vec<(FrontierId, i64)> = context
        .sink
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::StepStarted(step) => step
                .frontier_id
                .clone()
                .map(|frontier| (frontier, step.case_revision)),
            _ => None,
        })
        .collect();
    assert_eq!(
        executor.handed().len(),
        2,
        "precondition: two executor calls"
    );
    assert_eq!(
        started,
        executor.handed(),
        "each StepStarted names the frontier and revision its executor call was handed"
    );
}

/// A request the port answered `Pending` is invoked again as its own step: `StepStarted` with no
/// frontier, then the request proposed again under its own id, then its second answer; the
/// executor is not called between, and `Stopped` comes last and once.
#[test]
fn security_a_pending_request_invoked_again_is_its_own_step() {
    let case = CaseId("case-security-w3-pending".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(7, vec![action(INSPECT, ActionStatus::Admissible)])],
    );
    let effects = Effects::pending_first(1);
    let mut context = Context::new(0x3500, Some(2), Vec::<RuntimeEvent>::new());
    run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(INSPECT)]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut runs(0x3600),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let events = context.sink;

    assert_eq!(
        kinds(&events),
        [
            "StepStarted",
            "ActionSelected",
            "Proposed",
            "EffectInvoked",
            "StepStarted",
            "Proposed",
            "EffectInvoked",
            "Stopped",
        ],
        "the pending request's second invocation is a step of its own: {events:#?}"
    );
    let RuntimeEvent::StepStarted(again) = &events[4] else {
        panic!("a step starts: {:?}", events[4]);
    };
    assert_eq!((again.step, again.frontier_id.as_ref()), (2, None));
    let (RuntimeEvent::Proposed(first), RuntimeEvent::Proposed(second)) = (&events[2], &events[5])
    else {
        panic!("two proposals: {events:#?}");
    };
    assert_eq!(first.action_request_id, second.action_request_id);
    assert_eq!(second.answer, RevalidationAnswer::Admitted);
    let RuntimeEvent::EffectInvoked(pending) = &events[3] else {
        panic!("the first answer: {:?}", events[3]);
    };
    assert!(matches!(pending.outcome, EffectOutcome::Pending(_)));
    let RuntimeEvent::EffectInvoked(performed) = &events[6] else {
        panic!("the second answer: {:?}", events[6]);
    };
    assert!(matches!(performed.outcome, EffectOutcome::Performed(_)));
    assert_eq!(effects.invoked().len(), 2);
}

/// Reads proposed together are one step: each read selected, proposed and invoked, in proposal
/// order, after one `StepStarted`.
#[test]
fn security_a_fan_out_is_one_step_emitting_each_read_in_proposal_order() {
    let case = CaseId("case-security-w3-fan-out".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(
            7,
            vec![
                action(INSPECT, ActionStatus::Admissible),
                action(LIST, ActionStatus::Admissible),
            ],
        )],
    );
    let effects = Effects::default();
    let mut context = Context::new(0x3700, Some(1), Vec::<RuntimeEvent>::new());
    run_until_blocked(
        &governor,
        &ScriptedExecutor::new([ExecutorOutcome::ProposedReads(
            ExecutorOutcomeProposedReads {
                reads: vec![proposed(LIST), proposed(INSPECT)],
            },
        )]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut runs(0x3800),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let events = context.sink;

    assert_eq!(
        kinds(&events),
        [
            "StepStarted",
            "ActionSelected",
            "ActionSelected",
            "Proposed",
            "Proposed",
            "EffectInvoked",
            "EffectInvoked",
            "Stopped",
        ],
        "{events:#?}"
    );
    let order: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::ActionSelected(selected) => Some(selected.action.as_str()),
            RuntimeEvent::Proposed(proposed) => Some(proposed.action.as_str()),
            RuntimeEvent::EffectInvoked(invoked) => Some(invoked.action.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(order, [LIST, INSPECT, LIST, INSPECT, LIST, INSPECT]);
}

/// A host stop before the first executor call ends the run with no step: the sink hears only
/// `Stopped`, carrying the host's stop.
#[test]
fn security_a_host_stop_before_any_step_emits_only_stopped() {
    let case = CaseId("case-security-w3-host-stop".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(7, vec![action(INSPECT, ActionStatus::Admissible)])],
    );
    let effects = Effects::default();
    let mut context = Context::new(0x3900, Some(2), Vec::<RuntimeEvent>::new());
    context.host_stop = Some(HostStop {
        name: "disk".to_owned(),
        detail: Value::Null,
    });
    let end = run_until_blocked(
        &governor,
        &HandedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut runs(0x3a00),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert!(matches!(
        &end.outcome,
        RunOutcome::Suspended(suspended) if matches!(suspended.reason, SuspensionReason::HostStop(_))
    ));
    assert_eq!(kinds(&context.sink), ["Stopped"], "{:#?}", context.sink);
}

/// A granted request the host stops right before its effect is decided and never invoked: the
/// sink hears `AuthorityDecided` allow, then `Stopped`, and no `EffectInvoked`.
#[test]
fn security_a_granted_request_the_host_stops_emits_no_effect() {
    let case = CaseId("case-security-w3-host-stop-effect".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(7, vec![action(MERGE, ActionStatus::ApprovalRequired)])],
    );
    let effects = Effects::default();
    let mut context = Context::new(0x3b00, Some(2), Vec::<RuntimeEvent>::new());
    context.host_stop = Some(HostStop {
        name: "disk".to_owned(),
        detail: Value::Null,
    });
    run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(MERGE)]),
        &StaticAuthorityProvider::new().answer(CAPABILITY, allow()),
        &effects,
        &commission(&case),
        &mut runs(0x3c00),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert!(
        effects.invoked().is_empty(),
        "precondition: nothing invoked"
    );
    assert_eq!(
        kinds(&context.sink),
        [
            "StepStarted",
            "ActionSelected",
            "Proposed",
            "AuthorityDecided",
            "Stopped"
        ],
        "{:#?}",
        context.sink
    );
}

/// An effect port that fails to answer stops the loop with a failure: `Stopped` last and once,
/// with no outcome and the failure named, and no `EffectInvoked` for an answer never given.
#[test]
fn security_an_effect_failure_stops_once_naming_it() {
    let case = CaseId("case-security-w3-effect-failure".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(7, vec![action(INSPECT, ActionStatus::Admissible)])],
    );
    let effects = Effects::failing();
    let mut context = Context::new(0x3d00, Some(2), Vec::<RuntimeEvent>::new());
    let failed = run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(INSPECT)]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut runs(0x3e00),
        &mut context,
    );
    assert!(failed.is_err(), "precondition: the loop failed");
    let events = context.sink;
    assert_stopped_last_once(&events);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::EffectInvoked(_))),
        "{events:#?}"
    );
    let Some(RuntimeEvent::Stopped(stopped)) = events.last() else {
        panic!("{events:#?}");
    };
    assert!(
        stopped.outcome.is_none() && stopped.failure.is_some(),
        "{stopped:?}"
    );
}

/// A resume whose case moved after it took the Suspension invokes nothing. Every `StepStarted`
/// that names no frontier announces the invocation of a request (the spec: "before each
/// invocation of a request that is no executor call"); when that invocation does not happen, the
/// sink must hear why before the next step: the request `Proposed` with its `Stale` answer, as a
/// pending request found stale is. Otherwise the host hears a step that invoked nothing, with no
/// reason.
#[test]
fn security_a_resumed_request_on_a_moved_case_announces_no_unexplained_step() {
    let case = CaseId("case-security-w3-resume-moved".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    suspend_merge(&governor, &effects, &suspensions, &case);
    governor.script(
        case.clone(),
        [at(7, vec![action(MERGE, ActionStatus::ApprovalRequired)])],
    );
    let port = MovesOnTake {
        inner: &suspensions,
        governor: &governor,
        case: case.clone(),
        moved: at(8, vec![action(INSPECT, ActionStatus::Admissible)]),
        took: Mutex::new(false),
    };
    let mut context = Context::new(0x3f00, Some(1), Vec::<RuntimeEvent>::new());
    let _ = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new().answer(CAPABILITY, allow()),
        &effects,
        &commission(&case),
        &mut runs(0x4000),
        &port,
        &mut context,
    );
    assert!(
        *port.took.lock().unwrap_or_else(PoisonError::into_inner),
        "precondition: the resume took the Suspension"
    );
    assert!(
        effects.invoked().is_empty(),
        "precondition: nothing invoked"
    );
    let events = context.sink;
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::EffectInvoked(_))),
        "{events:#?}"
    );
    for (at, event) in events.iter().enumerate() {
        let RuntimeEvent::StepStarted(step) = event else {
            continue;
        };
        if step.frontier_id.is_some() {
            continue;
        }
        let explained = events[at + 1..]
            .iter()
            .take_while(|later| {
                !matches!(
                    later,
                    RuntimeEvent::StepStarted(_) | RuntimeEvent::Stopped(_)
                )
            })
            .any(|later| {
                matches!(
                    later,
                    RuntimeEvent::EffectInvoked(_)
                        | RuntimeEvent::Proposed(_)
                        | RuntimeEvent::AuthorityDecided(_)
                )
            });
        assert!(
            explained,
            "StepStarted #{} names no frontier and is followed by no invocation and no reason: \
             {events:#?}",
            step.step
        );
    }
}

/// A sink that panics on a resumed request's `EffectInvoked`, after the port performed it, never
/// lets the request be performed again: the Suspension is left `Resuming` (in doubt), never
/// `Pending`, and a later resume is refused `InDoubt` and invokes nothing.
#[test]
fn security_a_sink_panicking_after_a_resumed_effect_leaves_it_in_doubt_never_pending() {
    let case = CaseId("case-security-w3-resume-panic".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    suspend_merge(&governor, &effects, &suspensions, &case);
    let granting = StaticAuthorityProvider::new().answer(CAPABILITY, allow());

    let mut panicking = Context::new(
        0x4100,
        Some(1),
        PanicsOn {
            kind: "EffectInvoked",
            heard: Vec::new(),
        },
    );
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        resume_after_grant(
            &governor,
            &ScriptedExecutor::new([]),
            &granting,
            &effects,
            &commission(&case),
            &mut runs(0x3100),
            &suspensions,
            &mut panicking,
        )
    }));
    assert!(unwound.is_err(), "precondition: the sink's panic unwound");
    assert_eq!(effects.invoked().len(), 1, "precondition: performed once");
    let held = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("{error:?}"))
        .unwrap_or_else(|| panic!("the Suspension is still stored"));
    assert_eq!(held.state, SuspensionState::Resuming);

    let again = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &granting,
        &effects,
        &commission(&case),
        &mut runs(0x3100),
        &suspensions,
        &mut Context::new(0x4200, Some(1), Vec::<RuntimeEvent>::new()),
    );
    assert!(
        matches!(
            again,
            Err(ResumeError::Refused(ResumeRefusal::InDoubt { .. }))
        ),
        "{again:?}"
    );
    assert_eq!(effects.invoked().len(), 1, "never performed twice");
}

/// A sink that panics on a resumed Run's `StepStarted` stops it before the invoke: nothing is
/// performed, then or by a later resume.
#[test]
fn security_a_sink_panicking_before_a_resumed_invoke_performs_nothing() {
    let case = CaseId("case-security-w3-resume-panic-early".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    suspend_merge(&governor, &effects, &suspensions, &case);
    let granting = StaticAuthorityProvider::new().answer(CAPABILITY, allow());

    let mut panicking = Context::new(
        0x4300,
        Some(1),
        PanicsOn {
            kind: "StepStarted",
            heard: Vec::new(),
        },
    );
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        resume_after_grant(
            &governor,
            &ScriptedExecutor::new([]),
            &granting,
            &effects,
            &commission(&case),
            &mut runs(0x3100),
            &suspensions,
            &mut panicking,
        )
    }));
    assert!(unwound.is_err(), "precondition: the sink's panic unwound");
    assert!(effects.invoked().is_empty());
    let _ = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &granting,
        &effects,
        &commission(&case),
        &mut runs(0x3100),
        &suspensions,
        &mut Context::new(0x4400, Some(1), Vec::<RuntimeEvent>::new()),
    );
    assert!(
        effects.invoked().len() <= 1,
        "performed at most once across both resumes"
    );
}

/// No event carries the commission's authority context: a host's verified subject never reaches
/// the sink through an event.
#[test]
fn security_no_event_carries_the_authority_context() {
    let case = CaseId("case-security-w3-authority-context".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [at(
            7,
            vec![
                action(MERGE, ActionStatus::ApprovalRequired),
                action(INSPECT, ActionStatus::Admissible),
            ],
        )],
    );
    let effects = Effects::default();
    let mut context = Context::new(0x4500, Some(2), Vec::<RuntimeEvent>::new());
    run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(MERGE), propose(INSPECT)]),
        &StaticAuthorityProvider::new().answer(CAPABILITY, allow()),
        &effects,
        &commission(&case),
        &mut runs(0x4600),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert!(!context.sink.is_empty());
    let printed = format!("{:?}", context.sink);
    assert!(
        !printed.contains(AUTHORITY_MARKER),
        "an event carries the authority context: {printed}"
    );
}
