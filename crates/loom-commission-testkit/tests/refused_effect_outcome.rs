//! Acceptance for `story:refused-effect-ends-run`: an effect port can refuse an admitted request
//! as final, and the run ends at once with `RunOutcome::EffectRefused`, naming the request and the
//! port's reason (`ess/commission/domains/responsibility.yaml`, `EffectOutcomeRefused.ends_run`,
//! `JudgeEffectRefusal`).
//!
//! A refusal not marked final is an ordinary one: the loop goes on, and the iteration counts toward
//! the idle bound exactly as before, so a host that never marks a refusal final sees no change.
//! The final refusal is recorded and observed as every effect answer is before the run ends, and
//! nothing is retried. It ends the run wherever the runtime hands the port a request: a proposal
//! (item 8 of `runtime`'s module documentation), a fan-out of reads once every read answered and
//! was recorded (8a), and the request a resume performs, whose Suspension is released first.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, FrontierAction, ObservationId,
    PrincipalId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RunOutcomeCaseMovedOn, RunOutcomeEffectRefused, RunOutcomeIdleBound, RunOutcomeNeedsAuthority,
    RunState, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, Revalidated, resume_after_grant, run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

/// The trusted time the loop's context gives.
const NOW: &str = "2026-10-20T11:00:00Z";

/// What the step guard of a host's edit effect says when it refuses a change for good.
const GUARD: &str = "the step guard does not admit this change";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xe1)),
        agent_revision_id: AgentRevisionId(uuid(0xe2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-refused".to_owned()),
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

fn refused(reason: &str, ends_run: bool) -> EffectOutcome {
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason: reason.to_owned(),
        ends_run,
    })
}

fn performed(action: &str) -> EffectOutcome {
    EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text(format!("done {action}")),
        attempt: None,
        audit: None,
        usage: None,
    })
}

/// The request the loop makes as its `n`-th, for `action`, at `revision`.
fn request(n: u64, run: &RunId, case: &CaseId, revision: i64, action: &str) -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(uuid(0x300 + n)),
        run_id: run.clone(),
        case_id: case.clone(),
        expected_case_revision: revision,
        action: action.to_owned(),
        arguments: proposed(action).arguments,
    }
}

fn ended_refused(n: u64, reason: &str) -> RunOutcome {
    RunOutcome::EffectRefused(RunOutcomeEffectRefused {
        action_request_id: ActionRequestId(uuid(0x300 + n)),
        reason: reason.to_owned(),
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

/// New ids from counters starting at `0x300` and `0x400`, the one trusted time [`NOW`], and the
/// step budget it was given.
#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
}

impl Context {
    fn new(budget: Option<usize>) -> Self {
        Self {
            requests: 0x300,
            observations: 0x400,
            budget,
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
        self.budget
    }
}

/// A fake effect port. It performs every action, reads the actions it was told to, answers each
/// invocation of an action with the answer it was given for that action (`Performed` with report
/// `done <action>` for any other), and records each request it was handed.
#[derive(Default)]
struct Effects {
    reads: Vec<&'static str>,
    answers: BTreeMap<&'static str, EffectOutcome>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn answering(action: &'static str, answer: EffectOutcome) -> Self {
        Self {
            answers: BTreeMap::from([(action, answer)]),
            ..Self::default()
        }
    }

    /// The same port, answering `action` with `answer` too.
    fn with(mut self, action: &'static str, answer: EffectOutcome) -> Self {
        self.answers.insert(action, answer);
        self
    }

    fn reading(self, reads: Vec<&'static str>) -> Self {
        Self { reads, ..self }
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
        self.reads.contains(&action)
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
        let answer = self
            .answers
            .get(data.action.as_str())
            .cloned()
            .unwrap_or_else(|| performed(&data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(answer)
    }
}

/// A run store whose new runs take ids from `0x100 + base + 1` on.
fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    })
}

fn store() -> Generated<RunStore> {
    Generated::new(runs(0))
}

/// Runs one loop on a fresh store with no authority; returns its end and the store.
fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    case: &CaseId,
    budget: Option<usize>,
) -> (LoopEnd, Generated<RunStore>) {
    let mut runs = store();
    let end = run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut runs,
        &mut Context::new(budget),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    (end, runs)
}

fn stored_state(runs: &Generated<RunStore>, run: &RunId) -> RunState {
    RunStorage::get(&runs.ports, run)
        .unwrap_or_else(|| panic!("the loop's Run {} is not stored", run.0.0))
        .state
}

/// The edit effect refuses the admitted request as final: the run ends at once with
/// `EffectRefused`, naming the request and the port's reason. The executor is called once, the
/// case is not loaded again after the effect, and the Run is not suspended. On 0.26.0 the
/// refusal was an idle iteration, and this run ended on its idle bound as `NoAdmissibleAction`.
#[test]
fn a_final_refusal_ends_the_run_naming_request_and_reason() {
    let case = CaseId("case-final-refusal".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("edit"), admissible("inspect")])],
    );
    let executor = ScriptedExecutor::new([proposal("edit"), idle(), idle()]);
    let effects = Effects::answering("edit", refused(GUARD, true));

    let (end, runs) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(
        end.outcome,
        ended_refused(1, GUARD),
        "the run ends on the final refusal, naming the request and the reason"
    );
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
    assert_eq!(
        effects.invoked(),
        vec![request(1, &end.run_id, &case, 4, "edit")],
        "the request is handed to the port once and never retried"
    );
    assert_eq!(
        governor.calls(),
        [iteration(&case), revalidation(&case)].concat(),
        "the case is not loaded again after the final refusal"
    );
    assert_eq!(end.steps, 0, "the iteration that ended the run is no step");
    assert_eq!(
        stored_state(&runs, &end.run_id),
        RunState::Running,
        "the Run is ended, not suspended"
    );
}

/// With a step budget the idle bound does not apply, and on 0.26.0 this run went on to its budget;
/// a final refusal still ends it at once.
#[test]
fn a_final_refusal_ends_a_budgeted_run_at_once() {
    let case = CaseId("case-final-refusal-budgeted".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("edit")])]);
    let executor = ScriptedExecutor::new([proposal("edit"), idle(), idle(), idle(), idle()]);
    let effects = Effects::answering("edit", refused(GUARD, true));

    let (end, _) = run(&governor, &executor, &effects, &case, Some(4));

    assert_eq!(end.outcome, ended_refused(1, GUARD));
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
    assert_eq!(effects.invoked().len(), 1, "nothing is retried");
}

/// A refusal not marked final is an ordinary one, as on 0.26.0: the loop goes on, a repeated
/// refused proposal is idle, and two idle iterations in a row end the run on the idle bound
/// (`IdleBound`, `story:idle-bound-outcome`). The reason does not reach the outcome.
#[test]
fn an_ordinary_refusal_still_counts_idle() {
    let case = CaseId("case-ordinary-refusal".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("edit")])]);
    let executor = ScriptedExecutor::new([proposal("edit"), proposal("edit"), proposal("edit")]);
    let effects = Effects::answering("edit", refused(GUARD, false));

    let (end, runs) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 }),
        "an ordinary refusal ends the run on the idle bound"
    );
    assert_eq!(
        executor.calls().len(),
        3,
        "the loop went on after each refusal"
    );
    assert_eq!(
        effects.invoked(),
        vec![
            request(1, &end.run_id, &case, 4, "edit"),
            request(2, &end.run_id, &case, 4, "edit"),
            request(3, &end.run_id, &case, 4, "edit"),
        ]
    );
    assert_eq!(end.effects, vec![refused(GUARD, false); 3]);
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

/// The final refusal is recorded and observed before the run ends, each once: the effect's
/// observation carries the outcome `Refused`, the request, its action and arguments and the
/// reason, and `LoopEnd::requests`, `LoopEnd::admitted` and `LoopEnd::effects` each hold the
/// request once. Nothing becomes evidence.
#[test]
fn the_refusal_is_observed_once_and_recorded() {
    let case = CaseId("case-final-refusal-observed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible("edit")])]);
    let executor = ScriptedExecutor::new([proposal("edit"), idle(), idle()]);
    let effects = Effects::answering("edit", refused(GUARD, true));

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, "edit");
    assert_eq!(
        end.requests,
        vec![Revalidated {
            request: made.clone(),
            outcome: RevalidateActionRequestOutcome::Admitted,
        }],
        "the request is recorded once, admitted"
    );
    assert_eq!(end.admitted, vec![made.clone()], "admitted once");
    assert_eq!(end.effects, vec![refused(GUARD, true)], "its refusal, once");

    let observations = governor.observations();
    let sources: Vec<&str> = observations.iter().map(|o| o.source.as_str()).collect();
    assert_eq!(
        sources,
        ["executor", "effect"],
        "one observation of the step and one of the effect"
    );
    let effect = &observations[1];
    assert_eq!(effect.subject, "case-final-refusal-observed@4");
    assert_eq!(effect.observed_at, Timestamp(NOW.to_owned()));
    let member = |key: &str| effect.payload.member(key).cloned();
    assert_eq!(member("outcome"), Some(Value::Text("Refused".to_owned())));
    assert_eq!(
        member("action_request"),
        Some(Value::Text(made.action_request_id.0.0.clone()))
    );
    assert_eq!(member("action"), Some(Value::Text("edit".to_owned())));
    assert_eq!(member("arguments"), Some(made.arguments.0.clone()));
    assert_eq!(member("reason"), Some(Value::Text(GUARD.to_owned())));
    assert!(
        governor.evidence().is_empty(),
        "an effect became evidence: {:?}",
        governor.evidence()
    );
}

/// A fan-out of two reads, the first refused as final and the second performed: no read is
/// cancelled, both are recorded and observed in the order proposed, and then the run ends with
/// `EffectRefused` naming the refused read's request.
#[test]
fn a_final_refusal_of_a_read_ends_the_run_once_every_read_is_recorded() {
    let case = CaseId("case-final-refusal-fan-out".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("search"), admissible("fetch")])],
    );
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
            reads: vec![proposed("search"), proposed("fetch")],
        }),
        idle(),
        idle(),
    ]);
    let effects =
        Effects::answering("search", refused(GUARD, true)).reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(end.outcome, ended_refused(1, GUARD));
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
    assert_eq!(
        end.admitted,
        vec![
            request(1, &end.run_id, &case, 4, "search"),
            request(2, &end.run_id, &case, 4, "fetch"),
        ]
    );
    assert_eq!(
        end.effects,
        vec![refused(GUARD, true), performed("fetch")],
        "both reads are recorded, in the order proposed"
    );
    let sources: Vec<String> = governor
        .observations()
        .into_iter()
        .map(|o| o.source)
        .collect();
    assert_eq!(sources, ["executor", "effect", "effect"]);
}

fn fan_out(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions.iter().map(|action| proposed(action)).collect(),
    })
}

/// Both reads of a fan-out refused as final: the run ends naming the first in the order proposed,
/// with its reason, and both refusals are recorded.
#[test]
fn two_final_reads_in_a_fan_out_name_the_first_in_proposal_order() {
    let case = CaseId("case-final-refusal-two-reads".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("search"), admissible("fetch")])],
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch"]), idle(), idle()]);
    let effects = Effects::answering("search", refused("first", true))
        .with("fetch", refused("second", true))
        .reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(end.outcome, ended_refused(1, "first"));
    assert_eq!(
        end.effects,
        vec![refused("first", true), refused("second", true)],
        "both refusals are recorded, in the order proposed"
    );
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
}

/// Only the second read of a fan-out is refused as final, the first performed: the run ends
/// naming the second read's request, not the fan-out's first.
#[test]
fn a_final_refusal_of_the_second_read_names_its_request() {
    let case = CaseId("case-final-refusal-second-read".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("search"), admissible("fetch")])],
    );
    let executor = ScriptedExecutor::new([fan_out(&["fetch", "search"]), idle(), idle()]);
    let effects =
        Effects::answering("search", refused(GUARD, true)).reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(end.outcome, ended_refused(2, GUARD));
    assert_eq!(
        end.effects,
        vec![performed("fetch"), refused(GUARD, true)],
        "both reads are recorded, in the order proposed"
    );
}

/// A fan-out whose first read is refused as final and whose second is stale, the case having
/// moved to revision 5 where it still admits an action: the stale read shows the move, which is
/// judged first, as after a stale proposal, so the run ends `CaseMovedOn` and not `EffectRefused`.
/// (`adversary_w20261020w2_u1.rs` holds the same fan-out on a case the governor holds complete.)
#[test]
fn a_stale_read_beside_a_final_refusal_ends_as_after_a_stale_proposal() {
    let case = CaseId("case-final-refusal-beside-stale".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(4, vec![admissible("search"), admissible("fetch")]);
    // The start, the completion, the frontier and `search`'s revalidation answer at 4; `fetch`'s
    // revalidation finds the case at 5, open, and so does every call after.
    governor.script(
        case.clone(),
        std::iter::repeat_n(open, 5).chain([listing(5, vec![admissible("search")])]),
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch"]), idle(), idle()]);
    let effects =
        Effects::answering("search", refused(GUARD, true)).reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "precondition: `fetch` is stale: {:?}",
        end.requests[1].outcome
    );
    assert_eq!(end.effects, vec![refused(GUARD, true)], "only `search` ran");
    assert_eq!(executor.calls().len(), 1, "the executor is not run again");
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 4,
            current_case_revision: 5,
        }),
        "the move is judged before the final refusal"
    );
}

/// The same fan-out under a step budget of one, the stale read's revision not borne out: the next
/// load finds the case at 4 again. The stale iteration used up the budget, and the refusal kept
/// across it still ends the run `EffectRefused`, before the budget suspends it: a final refusal is
/// never lost, and the read it refused is never handed to the port again.
/// (`adversary_w20261020w2_u1.rs` holds the same case without a budget.)
#[test]
fn a_final_refusal_kept_across_a_stale_read_ends_the_run_before_its_budget() {
    let case = CaseId("case-final-refusal-kept-at-budget".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(4, vec![admissible("search"), admissible("fetch")]);
    // `fetch`'s revalidation reads revision 5; every call after answers at 4 again.
    governor.script(
        case.clone(),
        std::iter::repeat_n(open.clone(), 5)
            .chain([listing(5, vec![admissible("search"), admissible("fetch")])])
            .chain([open]),
    );
    let executor = ScriptedExecutor::new([
        fan_out(&["search", "fetch"]),
        fan_out(&["search", "fetch"]),
        idle(),
    ]);
    let effects =
        Effects::answering("search", refused(GUARD, true)).reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, Some(1));

    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "precondition: `fetch` is stale: {:?}",
        end.requests[1].outcome
    );
    assert_eq!(end.outcome, ended_refused(1, GUARD));
    assert_eq!(executor.calls().len(), 1, "the executor is not run again");
    assert_eq!(
        effects.invoked(),
        vec![request(1, &end.run_id, &case, 4, "search")],
        "the refused read is handed to the port once"
    );
    assert_eq!(end.steps, 1, "the stale fan-out was a step");
}

const MERGE: &str = "merge";
const CAPABILITY: &str = "repository.merge";

/// A resume performs the held request, the port refuses it as final, and the resumed Run ends with
/// `EffectRefused` naming it: the request was answered, so its Suspension is released, and no
/// executor is run after it.
#[test]
fn a_final_refusal_of_a_resumed_request_ends_the_run_and_releases_its_suspension() {
    let case = CaseId("case-final-refusal-resumed".to_owned());
    let governor = FakeGovernor::new();
    let gated = listing(
        7,
        vec![FrontierAction {
            action: MERGE.to_owned(),
            status: ActionStatus::ApprovalRequired,
            capability: Some(CAPABILITY.to_owned()),
            reasons: Vec::new(),
        }],
    );
    governor.script(case.clone(), [gated]);
    let effects = Effects::answering(MERGE, refused(GUARD, true));
    let suspensions = SuspensionStore::new();
    let withheld = AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: "approve the merge".to_owned(),
    });
    let suspended = run_with_suspension(
        &governor,
        &ScriptedExecutor::new([proposal(MERGE)]),
        &StaticAuthorityProvider::new().answer(CAPABILITY, withheld),
        &effects,
        &commission(&case),
        &mut runs(0),
        &suspensions,
        &mut Context::new(None),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        suspended.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: "approve the merge".to_owned(),
        }),
        "precondition: the merge is withheld and held as a Suspension"
    );

    let executor = ScriptedExecutor::new([idle(), idle()]);
    let mut context = Context::new(None);
    context.requests = 0x380;
    context.observations = 0x480;
    let end = resume_after_grant(
        &governor,
        &executor,
        &StaticAuthorityProvider::new().answer(CAPABILITY, AuthorityVerdict::Allow(Unit(true))),
        &effects,
        &commission(&case),
        &mut runs(0x80),
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert_eq!(
        end.outcome,
        ended_refused(1, GUARD),
        "the resumed Run ends on the final refusal of the held request"
    );
    assert_eq!(
        effects.invoked(),
        vec![request(1, &suspended.run_id, &case, 7, MERGE)],
        "the held request is performed once"
    );
    assert_eq!(end.effects, vec![refused(GUARD, true)]);
    assert!(
        executor.calls().is_empty(),
        "no executor runs after the final refusal"
    );
    assert_eq!(
        end.steps, 1,
        "the resumed request is the resumed Run's step"
    );
    let held = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"));
    assert_eq!(held, None, "the answered request's Suspension is released");
}
