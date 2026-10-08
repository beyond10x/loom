//! Acceptance for `story:runtime-merge` (Atlas ADR 0082): Commission's runtime invokes the effect,
//! and the slice's stop reasons and its approval-gate rule are the runtime's.
//!
//! `runtime::run_until_blocked` takes an [`EffectPort`]. Each iteration of the loop, in order:
//!
//! 0. the step budget ([`LoopContext::step_budget`]): when the steps taken reach it, the Run is
//!    suspended through `SuspendRun` with `SuspensionReason::Budget({"max_steps": <budget>})` and
//!    the run ends suspended with that reason, before the case is loaded again. A step is an
//!    executor call whose iteration did not end the run (`LoopEnd::steps`);
//! 1. the case is loaded. The Run continues at a revision its own effect moved the case to: the
//!    load after an iteration that invoked an effect sets the revision the loop holds the case at;
//!    any other move ends the run with no admissible action, as before;
//! 2. the completion, as before;
//! 3. the frontier, as before. It is kept as `LoopEnd::last_frontier`;
//! 4. a frontier that lists no action the effect port performs ([`EffectPort::performs`]) ends the
//!    run with `NoPerformableAction`, and the executor is not run;
//! 5. the approval gate: when the previous iteration was a step on a frontier listing actions that
//!    need approval, and this frontier lists the same actions with the same status and reasons,
//!    the run ends `AwaitingApproval`, carrying those actions in frontier order, and the executor
//!    is not run. An action an authority verdict in this Run denied is not one of them;
//! 6. the executor and its observation, as before;
//! 7. a proposal is revalidated as before. An admitted request (or one the provider allowed) is
//!    handed to [`EffectPort::invoke`] as an [`AdmittedRequest`], once, after its revalidation.
//!    The [`EffectOutcome`] is recorded in `LoopEnd::effects`, aligned with `LoopEnd::admitted`,
//!    and delivered to the observation port as one observation: id and time from the context,
//!    source `effect`, subject `<case>@<request revision>`, payload `outcome` (`Performed` or
//!    `Refused`), `action_request`, `action`, `arguments`, and `report` or `reason`. It is never
//!    evidence. The loop then goes on. An [`EffectError`] suspends the Run with
//!    `ExternalAvailability` and returns a `LoopError` with `LoopFailure::Effect`, naming the Run;
//! 8. the outcome is derived as before. Without a step budget, two idle iterations in a row end
//!    the run with no admissible action, as before; with one, the budget bounds the loop.
//!
//! The slice's stop reasons, in Commission's terms (each with a named test below):
//!
//! | Slice | Commission |
//! | --- | --- |
//! | `StepBudget` | `Suspended` with `SuspensionReason::Budget` |
//! | `NothingAdmissible` | `NoAdmissibleAction`, or `NeedsExternalEvidence` when obligations are open |
//! | `NoLocalExecutor` | `NoPerformableAction` |
//! | `ApprovalRequired` (proposal) | `NeedsAuthority` |
//! | `ApprovalRequired` (gate, `7dc84ef`) | `AwaitingApproval` |
//! | `Refused` | none: the router refuses before a case or Run exists (intake's own test) |

use std::collections::VecDeque;
use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction, FrontierId,
    FrontierObligation, ObservationId, PrincipalId, ProposedActionArguments,
    RevalidateActionRequestOutcome, RunId, RunOutcome, RunOutcomeAwaitingApproval,
    RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeNeedsAuthority,
    RunOutcomeNeedsExternalEvidence, RunOutcomeSuspended, RunState, SuspensionReason, Unit,
    commission_state, frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, UNBUDGETED_STEP_LIMIT, run_until_blocked,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-05T09:00:00Z";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-m".to_owned()),
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

fn gated(name: &str, capability: &str) -> FrontierAction {
    action(name, ActionStatus::ApprovalRequired, Some(capability))
}

/// The case open at `revision`, its frontier listing exactly `actions`.
fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn parse(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|error| panic!("fixture is not JSON: {error:?}"))
}

fn proposal_with(name: &str, arguments: Value) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(arguments),
    })
}

fn proposal(name: &str) -> ExecutorOutcome {
    proposal_with(name, Value::Null)
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

/// The request the loop makes as its `n`-th, for `action` with `arguments`, at `revision`.
fn request(
    n: u64,
    run: &RunId,
    case: &CaseId,
    revision: i64,
    action: &str,
    arguments: Value,
) -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(uuid(0x300 + n)),
        run_id: run.clone(),
        case_id: case.clone(),
        expected_case_revision: revision,
        action: action.to_owned(),
        arguments: ProposedActionArguments(arguments),
    }
}

fn performed(report: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text(report.to_owned()),
        attempt: None,
        audit: None,
    })
}

fn budget(steps: usize) -> SuspensionReason {
    SuspensionReason::Budget(Value::Object(vec![(
        "max_steps".to_owned(),
        Value::Number(steps.to_string()),
    )]))
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

/// The governor calls of an iteration that finds the case complete.
fn completed(case: &CaseId) -> Vec<GovernorCall> {
    vec![
        GovernorCall::CurrentRevision(case.clone()),
        GovernorCall::Completion(case.clone()),
    ]
}

/// New ids from counters, the one trusted time [`NOW`], and the step budget it was given.
#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl Context {
    fn with_budget(budget: usize) -> Self {
        Self {
            budget: Some(budget),
            ..Self::default()
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
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// The scripted fake executor, noting the governor log's length at each of its calls.
struct Sequenced<'g> {
    governor: &'g FakeGovernor,
    script: ScriptedExecutor,
    at: Mutex<Vec<usize>>,
}

impl<'g> Sequenced<'g> {
    fn new(governor: &'g FakeGovernor, script: impl IntoIterator<Item = ExecutorOutcome>) -> Self {
        Self {
            governor,
            script: ScriptedExecutor::new(script),
            at: Mutex::new(Vec::new()),
        }
    }

    fn at(&self) -> Vec<usize> {
        self.at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AgentExecutor for Sequenced<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(self.governor.calls().len());
        self.script.run(commission, frontier)
    }
}

/// A fake effect port. It performs the actions it was told to (every action unless told), answers
/// each invocation with the next scripted answer (`Performed` with report `done <action>` once the
/// script is used up), and records each request it was handed with the governor log's length at
/// the call.
struct Effects<'g> {
    governor: &'g FakeGovernor,
    performs: Option<Vec<&'static str>>,
    answers: Mutex<VecDeque<Result<EffectOutcome, EffectError>>>,
    invoked: Mutex<Vec<(ActionRequestData, usize)>>,
}

impl<'g> Effects<'g> {
    fn new(governor: &'g FakeGovernor) -> Self {
        Self {
            governor,
            performs: None,
            answers: Mutex::new(VecDeque::new()),
            invoked: Mutex::new(Vec::new()),
        }
    }

    fn performing(self, actions: Vec<&'static str>) -> Self {
        Self {
            performs: Some(actions),
            ..self
        }
    }

    fn answering(self, answers: Vec<Result<EffectOutcome, EffectError>>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
            ..self
        }
    }

    fn invoked(&self) -> Vec<ActionRequestData> {
        self.lock_invoked().iter().map(|(r, _)| r.clone()).collect()
    }

    fn at(&self) -> Vec<usize> {
        self.lock_invoked().iter().map(|(_, at)| *at).collect()
    }

    fn lock_invoked(&self) -> std::sync::MutexGuard<'_, Vec<(ActionRequestData, usize)>> {
        self.invoked.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl EffectPort for Effects<'_> {
    fn performs(&self, action: &str) -> bool {
        self.performs
            .as_ref()
            .is_none_or(|actions| actions.contains(&action))
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        let answer = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| Ok(performed(&format!("done {}", data.action))));
        self.lock_invoked()
            .push((data, self.governor.calls().len()));
        answer
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

/// Runs one loop on a fresh store; returns the loop's result and the store.
fn run<E: AgentExecutor>(
    governor: &FakeGovernor,
    executor: &E,
    authority: &StaticAuthorityProvider,
    effects: &Effects<'_>,
    case: &CaseId,
    mut context: Context,
) -> (Result<LoopEnd, LoopError>, Generated<RunStore>) {
    let mut runs = store();
    let result = run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut runs,
        &mut context,
    );
    (result, runs)
}

fn ended(name: &str, result: Result<LoopEnd, LoopError>) -> LoopEnd {
    result.unwrap_or_else(|error| panic!("{name}: the loop failed: {error:?}"))
}

fn stored(runs: &Generated<RunStore>, run: &RunId) -> (i64, RunState) {
    let held = RunStorage::get(&runs.ports, run)
        .unwrap_or_else(|| panic!("the loop's Run {} is not stored", run.0.0));
    (held.data.case_revision, held.state)
}

// ---------------------------------------------------------------------------------------------
// The effect step.

/// Two admitted requests, each handed to the effect port right after its revalidation, and the
/// loop going on after each: the executor runs again after the first effect, and the run ends
/// only when the governor holds the case complete.
#[test]
fn an_admitted_request_is_invoked_and_the_loop_goes_on() {
    let name = "invoked, then on";
    let case = CaseId("case-effect".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(5, vec![admissible("inspect"), admissible("edit")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 11).chain([open.complete("X")]),
    );
    let paths = parse(r#"{"paths": ["a.txt"]}"#);
    let files = parse(r#"{"files": [{"path": "a.txt", "contents": "b"}]}"#);
    let executor = Sequenced::new(
        &governor,
        [
            proposal_with("inspect", paths.clone()),
            proposal_with("edit", files.clone()),
        ],
    );
    let effects = Effects::new(&governor);
    let authority = StaticAuthorityProvider::new();

    let (result, _) = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::default(),
    );
    let end = ended(name, result);

    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            completed(&case),
        ]
        .concat(),
        "{name}: the effect step adds no governor read"
    );
    assert_eq!(executor.at(), [3, 8], "{name}: the executor runs again");
    assert_eq!(
        effects.at(),
        [5, 10],
        "{name}: each invocation comes right after its revalidation"
    );
    let inspect = request(1, &end.run_id, &case, 5, "inspect", paths.clone());
    let edit = request(2, &end.run_id, &case, 5, "edit", files);
    assert_eq!(end.admitted, vec![inspect.clone(), edit.clone()], "{name}");
    assert_eq!(
        effects.invoked(),
        end.admitted,
        "{name}: every admitted request is invoked once, in order"
    );
    assert_eq!(
        end.effects,
        vec![performed("done inspect"), performed("done edit")],
        "{name}: the effect outcomes, aligned with the admitted requests"
    );
    assert_eq!(end.steps, 2, "{name}: steps");
    assert_eq!(
        end.last_frontier.as_ref().map(|f| f.frontier_id.clone()),
        Some(FrontierId(uuid(3))),
        "{name}: the last frontier an iteration read"
    );

    let observations = governor.observations();
    let sources: Vec<&str> = observations.iter().map(|o| o.source.as_str()).collect();
    assert_eq!(
        sources,
        ["executor", "effect", "executor", "effect"],
        "{name}: one observation per step and one per effect, in order"
    );
    let effect = &observations[1];
    assert_eq!(effect.observation_id, ObservationId(uuid(0x402)), "{name}");
    assert_eq!(effect.observed_at, Timestamp(NOW.to_owned()), "{name}");
    assert_eq!(effect.subject, "case-effect@5", "{name}: subject");
    let member = |key: &str| effect.payload.member(key).cloned();
    assert_eq!(member("outcome"), Some(Value::Text("Performed".to_owned())));
    assert_eq!(
        member("action_request"),
        Some(Value::Text(inspect.action_request_id.0.0.clone()))
    );
    assert_eq!(member("action"), Some(Value::Text("inspect".to_owned())));
    assert_eq!(member("arguments"), Some(paths));
    assert_eq!(
        member("report"),
        Some(Value::Text("done inspect".to_owned()))
    );
    assert!(
        governor.evidence().is_empty(),
        "{name}: an effect became evidence: {:?}",
        governor.evidence()
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "X".to_owned()
        }),
        "{name}: outcome"
    );
}

/// A refused effect is recorded and observed with its reason, and the loop goes on.
#[test]
fn a_refused_effect_is_observed_and_the_loop_goes_on() {
    let name = "refused effect";
    let case = CaseId("case-refused-effect".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(2, vec![admissible("inspect")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 6).chain([open.complete("Y")]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let refused = EffectOutcome::Refused(EffectOutcomeRefused {
        reason: "no such path".to_owned(),
    });
    let effects = Effects::new(&governor).answering(vec![Ok(refused.clone())]);

    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended(name, result);

    assert_eq!(end.effects, vec![refused], "{name}: effects");
    let observations = governor.observations();
    assert_eq!(observations.len(), 2, "{name}: {observations:?}");
    assert_eq!(observations[1].source, "effect");
    assert_eq!(
        observations[1].payload.member("outcome"),
        Some(&Value::Text("Refused".to_owned()))
    );
    assert_eq!(
        observations[1].payload.member("reason"),
        Some(&Value::Text("no such path".to_owned()))
    );
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case), completed(&case)].concat(),
        "{name}: the loop went on to the next load"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Y".to_owned()
        })
    );
}

/// The Run continues at the revision its own effect moved the case to: the edit at 5 moves the
/// case to 6, the next request is made and admitted at 6, and the Run keeps its starting revision.
/// Control: the same move with no effect before it ends the run with no admissible action.
#[test]
fn the_run_goes_on_at_the_revision_its_effect_moved_the_case_to() {
    let name = "moved by its effect";
    let case = CaseId("case-moved-by-effect".to_owned());
    let governor = FakeGovernor::new();
    let at = |revision| listing(revision, vec![admissible("edit"), admissible("inspect")]);
    governor.script(
        case.clone(),
        repeat_n(at(5), 5)
            .chain(repeat_n(at(6), 6))
            .chain([at(6).complete("Z")]),
    );
    let executor = ScriptedExecutor::new([proposal("edit"), proposal("inspect")]);
    let effects = Effects::new(&governor);

    let (result, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended(name, result);

    assert_eq!(
        end.admitted,
        vec![
            request(1, &end.run_id, &case, 5, "edit", Value::Null),
            request(2, &end.run_id, &case, 6, "inspect", Value::Null),
        ],
        "{name}: the second request is made at the revision the effect left"
    );
    assert_eq!(effects.invoked(), end.admitted, "{name}: both invoked");
    assert_eq!(
        stored(&runs, &end.run_id),
        (5, RunState::Running),
        "{name}: the Run keeps the revision it started against"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "Z".to_owned()
        })
    );

    let name = "moved by someone else";
    let case = CaseId("case-moved-by-other".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), repeat_n(at(5), 5).chain([at(6)]));
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended(name, result);
    assert!(effects.invoked().is_empty(), "{name}: nothing admitted");
    // The frontier of 6 admits an action: the run ends `CaseMovedOn`
    // (story:moved-run-named-outcome).
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 5,
            current_case_revision: 6,
        }),
        "{name}: a move no effect of the run made ends it"
    );
}

/// An effect port that fails suspends the Run with `ExternalAvailability`, and the error names
/// the Run and carries the port's failure. No effect observation is delivered.
#[test]
fn an_effect_failure_suspends_the_run_and_names_it() {
    let case = CaseId("case-effect-fails".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let effects = Effects::new(&governor).answering(vec![Err(EffectError::new("disk full"))]);

    let (result, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let error = match result {
        Ok(end) => panic!("the effect port fails, got {end:?}"),
        Err(error) => error,
    };

    let held = RunStorage::list(&runs.ports);
    assert_eq!(held.len(), 1, "one Run");
    let run_id = held[0].data.run_id.clone();
    assert_eq!(error.run_id, Some(run_id.clone()));
    assert_eq!(
        error.failure,
        LoopFailure::Effect(EffectError::new("disk full"))
    );
    assert_eq!(error.suspension, None);
    assert!(
        matches!(
            error.suspension_reason.as_deref(),
            Some(SuspensionReason::ExternalAvailability(_))
        ),
        "{:?}",
        error.suspension_reason
    );
    assert_eq!(held[0].state, RunState::Suspended, "the Run is suspended");
    let shown = error.to_string();
    assert!(
        shown.contains(&run_id.0.0) && shown.contains("disk full"),
        "{shown}"
    );
    let sources: Vec<String> = governor
        .observations()
        .into_iter()
        .map(|o| o.source)
        .collect();
    assert_eq!(sources, ["executor"], "no effect observation");
}

/// Only an admitted request reaches the effect port: an unlisted proposal, a stale one and one the
/// provider did not allow are never invoked. Control: the provider's allow is invoked once.
#[test]
fn a_request_not_admitted_is_never_invoked() {
    let authority = StaticAuthorityProvider::new;

    // Not admitted: unlisted, then idle; the idle bound ends the run.
    let case = CaseId("case-not-admitted".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(3, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([proposal("deploy"), idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &authority(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("not admitted", result);
    assert!(effects.invoked().is_empty(), "not admitted, yet invoked");
    assert!(end.effects.is_empty());
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));

    // Stale: proposed at 7, the case at 8 by revalidation.
    let case = CaseId("case-stale".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        repeat_n(listing(7, vec![admissible("inspect")]), 3)
            .chain([listing(8, vec![admissible("inspect")])]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &authority(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("stale", result);
    assert!(effects.invoked().is_empty(), "stale, yet invoked");
    // The frontier of 8 admits `inspect`: the run ends `CaseMovedOn`
    // (story:moved-run-named-outcome).
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 7,
            current_case_revision: 8,
        })
    );

    // Allowed by the provider: invoked once.
    let case = CaseId("case-allowed".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(4, vec![gated("deploy", "prod.deploy")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 6).chain([open.complete("D")]),
    );
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &authority().answer("prod.deploy", AuthorityVerdict::Allow(Unit(true))),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("allowed", result);
    assert_eq!(effects.invoked(), end.admitted, "allowed: invoked");
    assert_eq!(effects.invoked().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// The slice's stop reasons, in Commission's terms.

/// `StepBudget`: the steps taken reach the budget, and the Run is suspended for `Budget` before
/// the case is loaded again. A budget of 0 reads no frontier and never runs the executor.
#[test]
fn the_step_budget_suspends_the_run_for_budget() {
    let name = "budget 1";
    let case = CaseId("case-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(3, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let effects = Effects::new(&governor);
    let (result, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(1),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case)].concat(),
        "{name}: no load after the budget is used up"
    );
    assert_eq!(
        effects.invoked().len(),
        1,
        "{name}: the one step was performed"
    );
    assert_eq!(end.steps, 1, "{name}: steps");
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended { reason: budget(1) }),
        "{name}: outcome"
    );
    assert_eq!(stored(&runs, &end.run_id).1, RunState::Suspended, "{name}");

    let name = "budget 0";
    let case = CaseId("case-budget-zero".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(3, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::new(&governor);
    let (result, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(0),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [GovernorCall::CurrentRevision(case.clone())],
        "{name}: only the load that starts the Run"
    );
    assert_eq!(end.steps, 0, "{name}");
    assert_eq!(end.last_frontier, None, "{name}: no frontier read");
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended { reason: budget(0) }),
        "{name}: outcome"
    );
    assert_eq!(stored(&runs, &end.run_id).1, RunState::Suspended, "{name}");
}

/// With a step budget, idle steps run to the budget: the budget bounds the loop, not the idle
/// bound. Control: without one, two idle iterations end the run with no admissible action.
#[test]
fn with_a_step_budget_idle_steps_run_to_the_budget() {
    let case = CaseId("case-idle-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(6, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([idle(), idle(), idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(3),
    );
    let end = ended("idle under a budget", result);
    assert_eq!(executor.calls().len(), 3, "the executor ran to the budget");
    assert_eq!(end.steps, 3);
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended { reason: budget(3) })
    );

    let case = CaseId("case-idle-unbounded".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(6, vec![admissible("inspect")])]);
    let executor = ScriptedExecutor::new([idle(), idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("idle without a budget", result);
    assert_eq!(executor.calls().len(), 2);
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}

/// `NothingAdmissible`: the executor has nothing to propose and the frontier admits no action. The
/// run ends from the frontier: no admissible action, or the open obligations it needs evidence for.
#[test]
fn nothing_admissible_ends_from_the_frontier() {
    let blocked = FrontierAction {
        reasons: vec!["recorded: blocked".to_owned()],
        ..action("inspect", ActionStatus::Blocked, None)
    };

    let case = CaseId("case-nothing".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(2, vec![blocked.clone()])]);
    let executor = ScriptedExecutor::new([idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended("nothing admissible", result);
    assert_eq!(end.steps, 0);
    assert!(effects.invoked().is_empty());
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));

    let case = CaseId("case-nothing-open".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(2).with_items(
            Vec::new(),
            vec![FrontierObligation {
                obligation: "tests.pass".to_owned(),
                open: true,
            }],
            vec![blocked],
        )],
    );
    let executor = ScriptedExecutor::new([idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended("nothing admissible, an obligation open", result);
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
            requirements: vec!["tests.pass".to_owned()],
        })
    );
}

/// `NoLocalExecutor`: a frontier that lists no action the effect port performs ends the run with
/// `NoPerformableAction` after its one frontier, and the executor is never run. Control: a frontier
/// that lists one such action, blocked or not, runs the executor.
#[test]
fn a_frontier_with_nothing_performable_never_runs_the_executor() {
    let name = "nothing performable";
    let case = CaseId("case-incident".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            1,
            vec![admissible("triage"), gated("escalate", "incident.escalate")],
        )],
    );
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::new(&governor).performing(vec!["inspect", "edit"]);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended(name, result);
    assert_eq!(governor.calls(), iteration(&case), "{name}: one frontier");
    assert!(executor.calls().is_empty(), "{name}: the executor ran");
    assert!(end.requests.is_empty(), "{name}");
    assert_eq!(end.steps, 0, "{name}");
    assert_eq!(
        end.last_frontier.as_ref().map(|f| f.frontier_id.clone()),
        Some(FrontierId(uuid(1))),
        "{name}: the frontier it stopped on"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::NoPerformableAction(Unit(true)),
        "{name}: outcome"
    );

    let name = "one performable action, blocked";
    let case = CaseId("case-performable".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            1,
            vec![
                admissible("triage"),
                action("inspect", ActionStatus::Blocked, None),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([idle(), idle()]);
    let effects = Effects::new(&governor).performing(vec!["inspect", "edit"]);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended(name, result);
    // `triage` is never offered (`story:effect-invocation`): the frontier the executor is handed
    // admits nothing, so the outcome derived from it ends the run after the one call.
    assert_eq!(executor.calls().len(), 1, "{name}: the executor ran");
    assert_ne!(end.outcome, RunOutcome::NoPerformableAction(Unit(true)));
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "{name}: outcome"
    );
}

/// `ApprovalRequired`, when the executor proposes an action needing approval: the provider is
/// asked, answers approval required, and the run ends `NeedsAuthority` with nothing invoked and the
/// proposal not counted as a step.
#[test]
fn a_proposal_needing_approval_ends_needs_authority() {
    let case = CaseId("case-merge".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![admissible("inspect"), gated("merge", "repository.merge")],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("merge")]);
    let effects = Effects::new(&governor);
    let authority = StaticAuthorityProvider::new().answer(
        "repository.merge",
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "approve repository.merge".to_owned(),
        }),
    );
    let (result, _) = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended("merge proposed", result);
    assert_eq!(authority.asked().len(), 1);
    assert!(effects.invoked().is_empty(), "invoked without authority");
    assert_eq!(end.steps, 0);
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: "approve repository.merge".to_owned(),
        })
    );
}

// ---------------------------------------------------------------------------------------------
// The approval gate (`7dc84ef`).

/// A performed step on a frontier listing two actions that need approval leaves it unchanged: the
/// next iteration reads it and ends `AwaitingApproval`, naming both in frontier order, before the
/// executor runs again. The first iteration on that frontier still runs the executor.
#[test]
fn a_step_that_leaves_the_approval_gate_unchanged_ends_awaiting_approval() {
    let name = "performed step at the gate";
    let case = CaseId("case-gate".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("inspect"),
                gated("deploy", "prod.deploy"),
                gated("merge", "repository.merge"),
            ],
        )],
    );
    let executor = Sequenced::new(&governor, [proposal("inspect")]);
    let effects = Effects::new(&governor);
    let authority = StaticAuthorityProvider::new();
    let (result, _) = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case), iteration(&case)].concat(),
        "{name}: governor calls"
    );
    assert_eq!(executor.at(), [3], "{name}: the executor ran once");
    assert_eq!(effects.invoked().len(), 1, "{name}: the step was performed");
    assert!(authority.asked().is_empty(), "{name}: provider asked");
    assert_eq!(end.steps, 1, "{name}");
    assert_eq!(
        end.last_frontier.as_ref().map(|f| f.frontier_id.clone()),
        Some(FrontierId(uuid(3))),
        "{name}: the frontier the gate stopped on"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["deploy".to_owned(), "merge".to_owned()],
        }),
        "{name}: outcome"
    );

    let name = "idle step at the gate";
    let case = CaseId("case-gate-idle".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![admissible("inspect"), gated("merge", "repository.merge")],
        )],
    );
    let executor = ScriptedExecutor::new([idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [iteration(&case), iteration(&case)].concat(),
        "{name}"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["merge".to_owned()],
        }),
        "{name}: outcome"
    );
}

/// A step at the gate that changes the frontier (here the reasons of the action needing approval)
/// does not stop the run; the next step that leaves it unchanged does.
#[test]
fn a_step_that_moves_the_frontier_at_the_gate_goes_on() {
    let case = CaseId("case-gate-moves".to_owned());
    let governor = FakeGovernor::new();
    let before = listing(
        4,
        vec![admissible("inspect"), gated("merge", "repository.merge")],
    );
    let after = listing(
        4,
        vec![
            admissible("inspect"),
            FrontierAction {
                reasons: vec!["evidence is stale".to_owned()],
                ..gated("merge", "repository.merge")
            },
        ],
    );
    governor.script(case.clone(), repeat_n(before, 5).chain([after]));
    let executor = ScriptedExecutor::new([proposal("inspect"), idle()]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(10),
    );
    let end = ended("moved at the gate", result);
    assert_eq!(executor.calls().len(), 2, "the run went on after the move");
    assert_eq!(end.steps, 2);
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["merge".to_owned()],
        })
    );
}

/// An action an authority verdict in this Run denied is not awaited: with it the only action
/// needing approval, the gate does not stop the run, which the idle bound then ends; beside
/// another action needing approval, only that other one is awaited.
#[test]
fn a_denied_action_is_not_awaited() {
    let deny = || {
        StaticAuthorityProvider::new().answer(
            "prod.deploy",
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: "no".to_owned(),
            }),
        )
    };

    let case = CaseId("case-denied".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![admissible("inspect"), gated("deploy", "prod.deploy")],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("deploy"), proposal("deploy")]);
    let effects = Effects::new(&governor);
    let authority = deny();
    let (result, _) = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("denied, alone", result);
    assert_eq!(authority.asked().len(), 2, "the executor ran again");
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));

    let case = CaseId("case-denied-beside".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("inspect"),
                gated("deploy", "prod.deploy"),
                gated("merge", "repository.merge"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let effects = Effects::new(&governor);
    let authority = deny();
    let (result, _) = run(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("denied, beside another", result);
    assert_eq!(authority.asked().len(), 1);
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["merge".to_owned()],
        })
    );
}

// ---------------------------------------------------------------------------------------------
// Adversary pass 1 corrections.

/// The approval gate is checked before the step budget: the last budgeted step leaving the gate
/// unchanged ends `AwaitingApproval`, after the case and frontier are read once more. Control: when
/// that step moved the frontier, the run is suspended for its budget, after the same reads.
#[test]
fn the_approval_gate_is_checked_before_the_step_budget() {
    let gate = || {
        listing(
            4,
            vec![admissible("inspect"), gated("merge", "repository.merge")],
        )
    };

    let name = "gate at the budget";
    let case = CaseId("case-gate-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gate()]);
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(1),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case), iteration(&case)].concat(),
        "{name}: the gate's frontier is read after the last step"
    );
    assert_eq!(end.steps, 1, "{name}");
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["merge".to_owned()],
        }),
        "{name}: outcome"
    );

    let name = "gate moved at the budget";
    let case = CaseId("case-gate-budget-moved".to_owned());
    let governor = FakeGovernor::new();
    let moved = listing(
        4,
        vec![
            admissible("inspect"),
            FrontierAction {
                reasons: vec!["evidence is stale".to_owned()],
                ..gated("merge", "repository.merge")
            },
        ],
    );
    governor.script(case.clone(), repeat_n(gate(), 5).chain([moved]));
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let effects = Effects::new(&governor);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::with_budget(1),
    );
    let end = ended(name, result);
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case), iteration(&case)].concat(),
        "{name}"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended { reason: budget(1) }),
        "{name}: outcome"
    );
}

/// A request admitted for an action the effect port does not perform ends the run with
/// `NoPerformableAction`: it stays in `requests` with its revalidation, is not admitted, and the
/// port is never handed it.
#[test]
fn an_admitted_action_the_port_does_not_perform_ends_no_performable_action() {
    let case = CaseId("case-unperformed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(5, vec![admissible("edit"), admissible("deploy")])],
    );
    let executor = ScriptedExecutor::new([proposal("deploy")]);
    let effects = Effects::new(&governor).performing(vec!["edit"]);
    let (result, _) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("unperformed", result);
    assert!(effects.invoked().is_empty(), "the port was handed it");
    assert_eq!(end.requests.len(), 1);
    assert_eq!(
        end.requests[0].outcome,
        RevalidateActionRequestOutcome::Admitted
    );
    assert!(end.admitted.is_empty(), "{:?}", end.admitted);
    assert!(end.effects.is_empty());
    assert_eq!(end.steps, 0);
    assert_eq!(end.outcome, RunOutcome::NoPerformableAction(Unit(true)));
}

/// An executor proposing `edit` with new arguments on every call: each admission is progress.
#[derive(Default)]
struct NewEdits {
    calls: Mutex<usize>,
}

impl AgentExecutor for NewEdits {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        *calls += 1;
        assert!(*calls <= UNBUDGETED_STEP_LIMIT, "past the limit: {calls}");
        proposal_with("edit", Value::Number(calls.to_string()))
    }
}

/// Without a step budget a loop that keeps making progress still ends: at
/// `UNBUDGETED_STEP_LIMIT` steps the Run is suspended for `Budget` with that limit.
#[test]
fn without_a_budget_the_default_limit_ends_a_loop_that_keeps_progressing() {
    let case = CaseId("case-progressing".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(2, vec![admissible("edit")])]);
    let executor = NewEdits::default();
    let effects = Effects::new(&governor);
    let (result, runs) = run(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        Context::default(),
    );
    let end = ended("progressing", result);
    let calls = *executor
        .calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert_eq!(calls, UNBUDGETED_STEP_LIMIT);
    assert_eq!(end.steps, UNBUDGETED_STEP_LIMIT);
    assert_eq!(effects.invoked().len(), UNBUDGETED_STEP_LIMIT);
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: budget(UNBUDGETED_STEP_LIMIT),
        })
    );
    assert_eq!(stored(&runs, &end.run_id).1, RunState::Suspended);
}
