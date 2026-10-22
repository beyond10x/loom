//! Acceptance for `story:runtime-events`: a host passes an `EventSink` through
//! `LoopContext::events` and receives every `RuntimeEvent` of a run as it happens, in loop order,
//! `Stopped` last and exactly once. A sink sees no event for a step that did not happen; an event
//! is never evidence and never changes the loop; a sink that panics is the host's. `NoEvents`, the
//! default, receives nothing.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorStep, ExecutorUsage, Frontier, FrontierAction,
    ObservationId, PrincipalId, ProposedActionArguments, RevalidationAnswer, RunId, RunOutcome,
    RunOutcomeNeedsAuthority, RuntimeEvent, RuntimeEventActionSelected,
    RuntimeEventAuthorityDecided, RuntimeEventEffectInvoked, RuntimeEventProposed,
    RuntimeEventSessionClosed, RuntimeEventStepStarted, RuntimeEventStopped, Unit,
    commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::events::{EventSink, NoEvents};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::suspension::SuspensionStore;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, resume_after_grant, run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const MERGE: &str = "merge";
const INSPECT: &str = "inspect";
const CAPABILITY: &str = "repository.merge";
const WITHHELD: &str = "approve the merge";
const SINK_PANIC: &str = "the host's sink panicked";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-events".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case at revision 7, open, listing [`MERGE`] as needing [`CAPABILITY`] and [`INSPECT`] as
/// admissible.
fn both_actions() -> Answer {
    Answer::at(7).with_items(
        Vec::new(),
        Vec::new(),
        vec![
            FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            },
            FrontierAction {
                action: INSPECT.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            },
        ],
    )
}

fn arguments(action: &str) -> ProposedActionArguments {
    ProposedActionArguments(
        json::parse(&format!(r#"{{"what":"{action}"}}"#))
            .unwrap_or_else(|error| panic!("arguments fixture is not JSON: {error:?}")),
    )
}

fn propose(action: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: arguments(action),
    })
}

fn usage() -> ExecutorUsage {
    ExecutorUsage {
        input_tokens: 120,
        output_tokens: 30,
        duration_ms: 900,
        cost_micro_usd: None,
    }
}

/// An executor that is an agent session: each call returns the next scripted outcome with the
/// session's transcript digest, summary and usage.
struct SessionExecutor {
    script: Mutex<Vec<ExecutorOutcome>>,
}

impl SessionExecutor {
    fn new(script: impl IntoIterator<Item = ExecutorOutcome>) -> Self {
        let mut script: Vec<ExecutorOutcome> = script.into_iter().collect();
        script.reverse();
        Self {
            script: Mutex::new(script),
        }
    }

    fn next(&self) -> ExecutorOutcome {
        self.script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop()
            .unwrap_or_else(|| panic!("SessionExecutor: the script is used up"))
    }
}

impl AgentExecutor for SessionExecutor {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.next()
    }

    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let outcome = self.run(commission, frontier);
        let action = match &outcome {
            ExecutorOutcome::ProposedAction(proposed) => proposed.action.clone(),
            _ => "nothing".to_owned(),
        };
        ExecutorStep {
            outcome,
            usage: Some(usage()),
            transcript_digest: Some(format!("sha256:{action}")),
            summary: Some(format!("proposed {action}")),
        }
    }
}

/// A loop context with a step budget and, when given one, an event sink.
struct Context<S> {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
    sink: S,
}

impl<S> Context<S> {
    fn new(base: u64, budget: Option<usize>, sink: S) -> Self {
        Self {
            requests: base,
            observations: base + 0x100,
            budget,
            sink,
        }
    }
}

/// The default context: it implements no `events`, so the runtime's default, `NoEvents`, holds.
struct Quiet {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl LoopContext for Quiet {
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

    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.sink
    }
}

fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(issued))
    })
}

/// Performs every action and records every request it was handed.
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

/// A sink that panics on the first event it receives.
struct Panicking;

impl EventSink for Panicking {
    fn emit(&mut self, _event: &RuntimeEvent) {
        panic!("{SINK_PANIC}");
    }
}

/// The kind of each event, in order.
fn kinds(events: &[RuntimeEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            RuntimeEvent::StepStarted(_) => "StepStarted",
            RuntimeEvent::ActionSelected(_) => "ActionSelected",
            RuntimeEvent::Proposed(_) => "Proposed",
            RuntimeEvent::AuthorityDecided(_) => "AuthorityDecided",
            RuntimeEvent::EffectInvoked(_) => "EffectInvoked",
            RuntimeEvent::SessionClosed(_) => "SessionClosed",
            RuntimeEvent::Stopped(_) => "Stopped",
        })
        .collect()
}

/// Runs the two-step scenario: the merge, allowed and performed, then the inspection, performed,
/// with a step budget of two.
fn two_steps<C: LoopContext>(
    governor: &FakeGovernor,
    effects: &Effects,
    case: &CaseId,
    context: &mut C,
) -> LoopEnd {
    governor.script(case.clone(), [both_actions()]);
    let executor = SessionExecutor::new([propose(MERGE), propose(INSPECT)]);
    let authority =
        StaticAuthorityProvider::new().answer(CAPABILITY, AuthorityVerdict::Allow(Unit(true)));
    run_until_blocked(
        governor,
        &executor,
        &authority,
        effects,
        &commission(case),
        &mut runs(0x100),
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// A recorded run of two steps emits each event once, in loop order: each step starts, its session
/// closes, its action is selected, proposed and revalidated, the merge's authority decided, and its
/// effect invoked; `Stopped` comes last and exactly once, carrying the run's outcome. Every event
/// names the Run, and the requests and effects the events name are the ones the loop recorded.
#[test]
fn every_step_of_a_run_is_emitted_in_order() {
    let case = CaseId("case-events-in-order".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let mut context = Context::new(0x300, Some(2), Vec::<RuntimeEvent>::new());

    let end = two_steps(&governor, &effects, &case, &mut context);
    let events = context.sink;

    assert_eq!(
        kinds(&events),
        [
            "StepStarted",
            "SessionClosed",
            "ActionSelected",
            "Proposed",
            "AuthorityDecided",
            "EffectInvoked",
            "StepStarted",
            "SessionClosed",
            "ActionSelected",
            "Proposed",
            "EffectInvoked",
            "Stopped",
        ],
        "every event, in loop order: {events:#?}"
    );

    let [merge, inspect] = end.requests.as_slice() else {
        panic!("two requests revalidated, got {:?}", end.requests);
    };
    let run_id = end.run_id.clone();

    let RuntimeEvent::StepStarted(first) = &events[0] else {
        panic!("the run starts with a step: {:?}", events[0]);
    };
    assert_eq!(first.run_id, run_id);
    assert_eq!(first.step, 1);
    assert_eq!(first.case_revision, 7);
    assert!(
        first.frontier_id.is_some(),
        "an executor call is handed a frontier"
    );
    assert_eq!(
        events[1],
        RuntimeEvent::SessionClosed(RuntimeEventSessionClosed {
            run_id: run_id.clone(),
            step: 1,
            transcript_digest: Some(format!("sha256:{MERGE}")),
            summary: Some(format!("proposed {MERGE}")),
            usage: Some(usage()),
        })
    );
    assert_eq!(
        events[2],
        RuntimeEvent::ActionSelected(RuntimeEventActionSelected {
            run_id: run_id.clone(),
            action: MERGE.to_owned(),
            arguments: arguments(MERGE),
        })
    );
    assert_eq!(
        events[3],
        RuntimeEvent::Proposed(RuntimeEventProposed {
            run_id: run_id.clone(),
            action_request_id: merge.request.action_request_id.clone(),
            action: MERGE.to_owned(),
            arguments: arguments(MERGE),
            case_revision: 7,
            answer: RevalidationAnswer::NeedsAuthority,
        })
    );
    assert_eq!(
        events[4],
        RuntimeEvent::AuthorityDecided(RuntimeEventAuthorityDecided {
            run_id: run_id.clone(),
            action_request_id: merge.request.action_request_id.clone(),
            capability: CAPABILITY.to_owned(),
            verdict: Some(AuthorityVerdict::Allow(Unit(true))),
        })
    );
    assert_eq!(
        events[5],
        RuntimeEvent::EffectInvoked(RuntimeEventEffectInvoked {
            run_id: run_id.clone(),
            action_request_id: merge.request.action_request_id.clone(),
            action: MERGE.to_owned(),
            outcome: end.effects[0].clone(),
        })
    );
    let RuntimeEvent::StepStarted(second) = &events[6] else {
        panic!("the second step starts: {:?}", events[6]);
    };
    assert_eq!(
        (&second.run_id, second.step, second.case_revision),
        (&run_id, 2, 7)
    );
    assert!(
        second.frontier_id.is_some() && second.frontier_id != first.frontier_id,
        "each executor call is handed the frontier the loop read for it: {second:?}"
    );
    assert_eq!(
        events[9],
        RuntimeEvent::Proposed(RuntimeEventProposed {
            run_id: run_id.clone(),
            action_request_id: inspect.request.action_request_id.clone(),
            action: INSPECT.to_owned(),
            arguments: arguments(INSPECT),
            case_revision: 7,
            answer: RevalidationAnswer::Admitted,
        })
    );
    assert_eq!(
        events[10],
        RuntimeEvent::EffectInvoked(RuntimeEventEffectInvoked {
            run_id: run_id.clone(),
            action_request_id: inspect.request.action_request_id.clone(),
            action: INSPECT.to_owned(),
            outcome: end.effects[1].clone(),
        })
    );
    assert_eq!(
        events.last(),
        Some(&RuntimeEvent::Stopped(RuntimeEventStopped {
            run_id,
            outcome: Some(end.outcome.clone()),
            failure: None,
        })),
        "the loop's stop is the last event and carries its outcome"
    );
    let stops = events
        .iter()
        .filter(|event| matches!(event, RuntimeEvent::Stopped(_)))
        .count();
    assert_eq!(stops, 1, "Stopped is emitted exactly once");
}

/// A proposal the provider denies emits its `Proposed` and its `AuthorityDecided`, never an
/// `EffectInvoked`: nothing was invoked, and the sink hears of nothing that did not happen.
#[test]
fn a_sink_sees_no_event_for_a_step_that_did_not_happen() {
    let case = CaseId("case-events-refused".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [both_actions()]);
    let effects = Effects::default();
    let deny = AuthorityVerdict::Deny(AuthorityVerdictDeny {
        reason: "not this one".to_owned(),
    });
    let authority = StaticAuthorityProvider::new().answer(CAPABILITY, deny.clone());
    let mut context = Context::new(0x400, Some(1), Vec::<RuntimeEvent>::new());

    let end = run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(MERGE)]),
        &authority,
        &effects,
        &commission(&case),
        &mut runs(0x200),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let events = context.sink;

    assert!(
        effects.invoked().is_empty(),
        "the denied merge is not invoked"
    );
    assert_eq!(
        kinds(&events),
        [
            "StepStarted",
            "ActionSelected",
            "Proposed",
            "AuthorityDecided",
            "Stopped"
        ],
        "a refused proposal is proposed and decided, and nothing else happens: {events:#?}"
    );
    let [request] = end.requests.as_slice() else {
        panic!("one request revalidated, got {:?}", end.requests);
    };
    assert_eq!(
        events[3],
        RuntimeEvent::AuthorityDecided(RuntimeEventAuthorityDecided {
            run_id: end.run_id.clone(),
            action_request_id: request.request.action_request_id.clone(),
            capability: CAPABILITY.to_owned(),
            verdict: Some(deny),
        })
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::EffectInvoked(_))),
        "no effect was invoked, so none is emitted"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::SessionClosed(_))),
        "a call that was no session closes none"
    );
}

/// A provider that fails grants nothing, and its `AuthorityDecided` carries no verdict.
#[test]
fn a_failing_provider_is_decided_with_no_verdict() {
    let case = CaseId("case-events-provider-fails".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [both_actions()]);
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new().fail(CAPABILITY, "unreachable");
    let mut context = Context::new(0x480, Some(1), Vec::<RuntimeEvent>::new());

    run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose(MERGE)]),
        &authority,
        &effects,
        &commission(&case),
        &mut runs(0x280),
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let decided: Vec<&RuntimeEvent> = context
        .sink
        .iter()
        .filter(|event| matches!(event, RuntimeEvent::AuthorityDecided(_)))
        .collect();
    let [RuntimeEvent::AuthorityDecided(decided)] = decided.as_slice() else {
        panic!("one authority decided, got {decided:?}");
    };
    assert_eq!(decided.verdict, None, "a failed provider grants nothing");
    assert!(effects.invoked().is_empty());
}

/// The same run with and without a sink ends alike, makes the same requests and effects, and the
/// governor observes the same observations: an event is never evidence or an observation, and the
/// sink changes nothing. `NoEvents`, the default, receives nothing and answers that it does not.
#[test]
fn an_event_is_never_evidence_and_never_changes_the_loop() {
    let case = CaseId("case-events-change-nothing".to_owned());

    let quiet_governor = FakeGovernor::new();
    let quiet_effects = Effects::default();
    let mut quiet = Quiet {
        requests: 0x500,
        observations: 0x600,
        budget: Some(2),
    };
    let quiet_end = two_steps(&quiet_governor, &quiet_effects, &case, &mut quiet);

    let heard_governor = FakeGovernor::new();
    let heard_effects = Effects::default();
    let mut heard = Context::new(0x500, Some(2), Vec::<RuntimeEvent>::new());
    let heard_end = two_steps(&heard_governor, &heard_effects, &case, &mut heard);

    assert!(!heard.sink.is_empty(), "the sink heard the run");
    assert_eq!(
        heard_end, quiet_end,
        "the sink changes nothing the loop ends with"
    );
    assert_eq!(heard_effects.invoked(), quiet_effects.invoked());
    assert_eq!(
        heard_governor.observations(),
        quiet_governor.observations(),
        "no event is observed"
    );
    assert_eq!(
        heard_governor.evidence(),
        quiet_governor.evidence(),
        "no event is evidence"
    );
    assert_eq!(heard_governor.calls(), quiet_governor.calls());

    assert!(!NoEvents.receives(), "NoEvents wants no event built for it");
    let mut none = NoEvents;
    if let Some(event) = heard.sink.first() {
        none.emit(event);
    }
}

/// A sink that panics is the host's: the runtime catches nothing, the panic unwinds out of the
/// loop, and nothing the step would have invoked was invoked after it.
#[test]
fn a_panicking_sink_is_the_hosts() {
    let case = CaseId("case-events-panic".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let mut context = Context::new(0x700, Some(2), Panicking);

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        two_steps(&governor, &effects, &case, &mut context)
    }));
    let Err(payload) = unwound else {
        panic!("the sink's panic reaches the host");
    };
    let message = payload.downcast_ref::<String>().cloned().or_else(|| {
        payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
    });
    assert_eq!(message.as_deref(), Some(SINK_PANIC));
    assert!(
        effects.invoked().is_empty(),
        "the first event is emitted before any effect, and the panic stops the loop there"
    );
}

/// A resumed Run emits its resumed request as a step: `StepStarted` with no frontier,
/// `EffectInvoked`, and `Stopped` once. The resume's revalidation and authority check before its
/// Run resumed are its answer, not events, and a resume refused before its Run resumed emits
/// nothing.
#[test]
fn a_resumed_run_emits_its_request_and_stops_once() {
    let case = CaseId("case-events-resume".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    governor.script(case.clone(), [both_actions()]);
    let withholding = StaticAuthorityProvider::new().answer(
        CAPABILITY,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: WITHHELD.to_owned(),
        }),
    );
    let mut first = Context::new(0x800, None, Vec::<RuntimeEvent>::new());
    let suspended = run_with_suspension(
        &governor,
        &ScriptedExecutor::new([propose(MERGE)]),
        &withholding,
        &effects,
        &commission(&case),
        &mut runs(0x300),
        &suspensions,
        &mut first,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        suspended.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: WITHHELD.to_owned(),
        })
    );
    assert_eq!(
        kinds(&first.sink),
        [
            "StepStarted",
            "ActionSelected",
            "Proposed",
            "AuthorityDecided",
            "Stopped"
        ]
    );

    // Refused: the provider still withholds it, so the Run never resumes and nothing is emitted.
    let mut refused = Context::new(0x900, None, Vec::<RuntimeEvent>::new());
    let refusal = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &withholding,
        &effects,
        &commission(&case),
        &mut runs(0x300),
        &suspensions,
        &mut refused,
    );
    assert!(refusal.is_err(), "a withheld request is not resumed");
    assert!(refused.sink.is_empty(), "a refused resume emits nothing");

    // Granted: the request is performed as the resumed Run's first step; one step budget ends it.
    let granting =
        StaticAuthorityProvider::new().answer(CAPABILITY, AuthorityVerdict::Allow(Unit(true)));
    let mut resumed = Context::new(0xa00, Some(1), Vec::<RuntimeEvent>::new());
    let end = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &granting,
        &effects,
        &commission(&case),
        &mut runs(0x300),
        &suspensions,
        &mut resumed,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    let events = resumed.sink;
    assert_eq!(
        kinds(&events),
        ["StepStarted", "EffectInvoked", "Stopped"],
        "the resumed request is one step; the resume's own checks are no events: {events:#?}"
    );
    let [request] = end.requests.as_slice() else {
        panic!("one request, got {:?}", end.requests);
    };
    assert_eq!(
        events[0],
        RuntimeEvent::StepStarted(RuntimeEventStepStarted {
            run_id: end.run_id.clone(),
            step: 1,
            case_revision: 7,
            frontier_id: None,
        })
    );
    let RuntimeEvent::EffectInvoked(invoked) = &events[1] else {
        panic!("the resumed request is invoked: {:?}", events[1]);
    };
    assert_eq!(invoked.action_request_id, request.request.action_request_id);
    assert_eq!(invoked.action, MERGE);
    assert_eq!(
        events[2],
        RuntimeEvent::Stopped(RuntimeEventStopped {
            run_id: end.run_id.clone(),
            outcome: Some(end.outcome.clone()),
            failure: None,
        })
    );
}
