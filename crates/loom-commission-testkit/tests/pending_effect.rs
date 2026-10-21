//! Acceptance for `story:pending-effect`: an effect port can answer that the work an admitted
//! request started has not finished (`EffectOutcome::Pending`), and the runtime invokes the same
//! request again at the next iteration, within the run's step and run budgets, without counting
//! the iteration idle (`ess/commission/domains/responsibility.yaml`, `EffectOutcomePending`).
//!
//! Each invocation after the first is revalidated as the first was: a request whose case moved is
//! not invoked again. Each answer is one trial on the run budget and one observation, never
//! evidence. A budget that ends the run while a request is pending ends it `Waiting`, naming the
//! request and the budget's reason, where it would otherwise be `Suspended` (`JudgeBudgetStop`).
//!
//! `story:pending-answer-ends-run`: a pending answer with `ends_run` is not invoked again. Once it
//! was recorded, charged and observed, and the run budget's spending limits checked, the run ends
//! `Waiting`, naming the request (for a fan-out, every read answered pending, in proposal order),
//! the Run suspended for `SuspensionReason::Time` carrying the port's report.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::{Generated, RunStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionBudget, ActionCharge, ActionNeedsAuthority, ActionRequestData, ActionRequestId,
    ActionRequestStale, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, AuthorityVerdictDeny, BudgetLimit, CaseId, ChargedUsage,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePending,
    EffectOutcomePerformed, EffectUsage, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RevalidateActionRequestOutcome, RunBudget, RunBudgetBreach, RunId,
    RunOutcome, RunOutcomeAwaitingApproval, RunOutcomeCaseMovedOn, RunOutcomeIdleBound,
    RunOutcomeNeedsAuthority, RunOutcomeSuspended, RunOutcomeWaiting, RunState, SuspensionReason,
    SuspensionState, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
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
const NOW: &str = "2026-10-20T14:00:00Z";

/// The action of a validator that waits on work outside the run, such as a CI job.
const CHECK: &str = "ci.check";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xf1)),
        agent_revision_id: AgentRevisionId(uuid(0xf2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-pending".to_owned()),
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

fn fan_out(actions: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: actions.iter().map(|action| proposed(action)).collect(),
    })
}

fn idle() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn pending(seen: &str) -> EffectOutcome {
    EffectOutcome::Pending(EffectOutcomePending {
        report: Value::Text(seen.to_owned()),
        ends_run: false,
    })
}

/// A pending answer with `ends_run`: the port asks not to be invoked again in this run.
fn ending(seen: &str) -> EffectOutcome {
    EffectOutcome::Pending(EffectOutcomePending {
        report: Value::Text(seen.to_owned()),
        ends_run: true,
    })
}

/// The reason a run a pending answer with `ends_run` ended is suspended for: the port's report.
fn time(seen: &str) -> SuspensionReason {
    SuspensionReason::Time(Value::Text(seen.to_owned()))
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

/// The governor calls an iteration makes before it runs the executor or invokes a pending
/// request again.
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

/// New ids from counters starting at `0x300` and `0x400`, the one trusted time [`NOW`], the step
/// budget and the run budget it was given.
#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
    run_budget: Option<RunBudget>,
}

impl Context {
    fn new(budget: Option<usize>) -> Self {
        Self {
            requests: 0x300,
            observations: 0x400,
            budget,
            run_budget: None,
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

    fn run_budget(&self) -> Option<RunBudget> {
        self.run_budget.clone()
    }
}

/// A fake effect port. It performs every action and reads the actions it was told to. Each
/// invocation of an action takes the next answer scripted for it, the last one answering again
/// once the script has run out (`Performed` with report `done <action>` for an action with none),
/// and it records each request it was handed, in order.
#[derive(Default)]
struct Effects {
    reads: Vec<&'static str>,
    answers: Mutex<BTreeMap<&'static str, VecDeque<EffectOutcome>>>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn answering(action: &'static str, answers: Vec<EffectOutcome>) -> Self {
        Self {
            answers: Mutex::new(BTreeMap::from([(action, VecDeque::from(answers))])),
            ..Self::default()
        }
    }

    /// The same port, answering `action` with `answers` too.
    fn with(self, action: &'static str, answers: Vec<EffectOutcome>) -> Self {
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(action, VecDeque::from(answers));
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
        let answer = {
            let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
            match answers.get_mut(data.action.as_str()) {
                Some(script) if script.len() > 1 => script.pop_front(),
                Some(script) => script.front().cloned(),
                None => None,
            }
        }
        .unwrap_or_else(|| performed(&data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(answer)
    }
}

/// A run store whose new runs take ids from `0x101` on.
fn store() -> Generated<RunStore> {
    let mut issued = 0;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

/// Runs one loop on a fresh store with `authority` and `context`; returns its end and the store.
fn run_with(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &dyn AuthorityProvider,
    effects: &Effects,
    case: &CaseId,
    context: &mut Context,
) -> (LoopEnd, Generated<RunStore>) {
    let mut runs = store();
    let end = run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut runs,
        context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    (end, runs)
}

/// Runs one loop on a fresh store with no authority and the step budget `budget`.
fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    case: &CaseId,
    budget: Option<usize>,
) -> (LoopEnd, Generated<RunStore>) {
    run_with(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        case,
        &mut Context::new(budget),
    )
}

fn stored_state(runs: &Generated<RunStore>, run: &RunId) -> RunState {
    RunStorage::get(&runs.ports, run)
        .unwrap_or_else(|| panic!("the loop's Run {} is not stored", run.0.0))
        .state
}

/// A validator answers pending twice and then `Performed`: the runtime invokes the same request
/// three times, each time revalidated, with no executor call in between, and the run goes on: the
/// executor runs again after the third invocation. The idle iteration before the proposal is not
/// joined by the pending answers: had one counted idle, two in a row would have ended the run on
/// the idle bound before the request was invoked again. On 0.28.0 there was no pending answer.
#[test]
fn a_pending_answer_is_invoked_again_and_not_counted_idle() {
    let case = CaseId("case-pending-invoked-again".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([idle(), proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(
        CHECK,
        vec![
            pending("job queued"),
            pending("job running"),
            performed(CHECK),
        ],
    );

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(
        effects.invoked(),
        vec![made.clone(), made.clone(), made.clone()],
        "the same request, under its own id and at its own revision, is invoked three times"
    );
    assert_eq!(
        end.effects,
        vec![
            pending("job queued"),
            pending("job running"),
            performed(CHECK)
        ],
        "every answer is recorded, in order"
    );
    assert_eq!(
        executor.calls().len(),
        4,
        "no executor runs while the request is pending, and the run goes on after it"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 }),
        "the run ends on two idle iterations after the performed effect, not before"
    );
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            iteration(&case),
        ]
        .concat(),
        "each invocation of the pending request is revalidated before it"
    );
    assert_eq!(
        end.steps, 5,
        "the idle step, the proposal and the two invocations again are steps, and so is the \
         first idle step after them"
    );
    assert_eq!(
        end.requests,
        vec![
            Revalidated {
                request: made.clone(),
                outcome: RevalidateActionRequestOutcome::Admitted,
            };
            3
        ],
        "each revalidation is recorded"
    );
}

/// The idle count stands still while the request is pending: an idle iteration (count one), then
/// a proposal repeating an admitted request, answered pending twice and then `Performed`. The
/// repeated admission is idle, so once it is performed the count reaches two and the run ends
/// `IdleBound` at once. Had a pending answer counted idle, the run would have ended at the first;
/// had it started the count again, one more idle iteration would have been needed.
#[test]
fn a_pending_answer_leaves_the_idle_count_where_it_was() {
    let case = CaseId("case-pending-idle-count".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor =
        ScriptedExecutor::new([proposal(CHECK), idle(), proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(
        CHECK,
        vec![
            performed(CHECK),
            pending("job queued"),
            pending("job running"),
            performed(CHECK),
        ],
    );

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
    assert_eq!(
        effects.invoked(),
        vec![
            request(1, &end.run_id, &case, 4, CHECK),
            request(2, &end.run_id, &case, 4, CHECK),
            request(2, &end.run_id, &case, 4, CHECK),
            request(2, &end.run_id, &case, 4, CHECK),
        ],
        "the repeated request is invoked until it is performed"
    );
    assert_eq!(
        executor.calls().len(),
        3,
        "the run ends on the idle bound right after the repeated request is performed"
    );
}

/// The request answered pending is invoked again on the case at revision 4, revalidated first, and
/// answered pending again; then the case moves to revision 5, where it still admits an action. The
/// next iteration loads it, finds the move and ends `CaseMovedOn`, and the request bound to
/// revision 4 is not invoked a third time.
#[test]
fn each_reinvocation_is_revalidated() {
    let case = CaseId("case-pending-moved".to_owned());
    let governor = FakeGovernor::new();
    // The first iteration (the start, the completion, the frontier and the request's
    // revalidation) and the second (the load, the completion, the frontier and the revalidation
    // before the request is invoked again) answer at 4; every call after answers at 5.
    governor.script(
        case.clone(),
        std::iter::repeat_n(listing(4, vec![admissible(CHECK)]), 10)
            .chain([listing(5, vec![admissible(CHECK)])]),
    );
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![pending("job running")]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(
        effects.invoked(),
        vec![made.clone(), made.clone()],
        "invoked again at its revision, and not on the case that moved"
    );
    assert_eq!(
        end.requests,
        vec![
            Revalidated {
                request: made,
                outcome: RevalidateActionRequestOutcome::Admitted,
            };
            2
        ],
        "the invocation again is revalidated as the first was"
    );
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
        ]
        .concat()
    );
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 4,
            current_case_revision: 5,
        })
    );
    assert_eq!(executor.calls().len(), 1, "the executor is not run again");
}

/// The move shows only at the revalidation before the request is invoked again: the iteration
/// loads the case at 4, and the revalidation reads 5. The request is recorded stale and not
/// invoked, and the next iteration judges the move, as after a stale proposal.
#[test]
fn a_pending_request_found_stale_at_its_revalidation_is_not_invoked_again() {
    let case = CaseId("case-pending-stale".to_owned());
    let governor = FakeGovernor::new();
    // The first iteration and its revalidation (five calls), then the second iteration's load,
    // completion and frontier (three more), answer at 4; the revalidation after them reads 5.
    governor.script(
        case.clone(),
        std::iter::repeat_n(listing(4, vec![admissible(CHECK)]), 8)
            .chain([listing(5, vec![admissible(CHECK)])]),
    );
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![pending("job running")]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(effects.invoked(), vec![made.clone()], "invoked once");
    assert_eq!(
        end.requests,
        vec![
            Revalidated {
                request: made.clone(),
                outcome: RevalidateActionRequestOutcome::Admitted,
            },
            Revalidated {
                request: made,
                outcome: RevalidateActionRequestOutcome::Stale {
                    error: ActionRequestStale {
                        expected_case_revision: 4,
                        current_case_revision: 5,
                    },
                },
            },
        ]
    );
    assert_eq!(
        end.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 4,
            current_case_revision: 5,
        })
    );
    assert_eq!(executor.calls().len(), 1, "the executor is not run again");
}

const MERGE: &str = "merge";
const CAPABILITY: &str = "repository.merge";

/// An authority provider allowing `CAPABILITY` the first `allow` times it is asked and answering
/// `after` from then on, recording each request it was asked about.
struct Allowing {
    allow: usize,
    after: AuthorityVerdict,
    asked: Mutex<Vec<ActionRequestData>>,
}

impl Allowing {
    /// Allows the first `allow` times, then denies: the grant was withdrawn.
    fn then_denying(allow: usize) -> Self {
        Self::then(
            allow,
            AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: "the grant was withdrawn".to_owned(),
            }),
        )
    }

    fn then(allow: usize, after: AuthorityVerdict) -> Self {
        Self {
            allow,
            after,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<ActionRequestData> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AuthorityProvider for Allowing {
    fn decide(
        &self,
        _commission: &CommissionData,
        request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let mut asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        asked.push(request.clone());
        if capability != CAPABILITY {
            return Err(AuthorityProviderError::new("unknown capability"));
        }
        Ok(if asked.len() <= self.allow {
            AuthorityVerdict::Allow(Unit(true))
        } else {
            self.after.clone()
        })
    }
}

/// A request that needs authority is asked about again before each invocation, as before the
/// first, and the case's revision is read again after each allow. Once the provider no longer
/// allows it, it is not invoked again: the iteration goes on to the executor.
#[test]
fn a_pending_request_needing_authority_is_asked_about_again() {
    let case = CaseId("case-pending-authority".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            7,
            vec![
                FrontierAction {
                    action: MERGE.to_owned(),
                    status: ActionStatus::ApprovalRequired,
                    capability: Some(CAPABILITY.to_owned()),
                    reasons: Vec::new(),
                },
                admissible(CHECK),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal(MERGE), idle(), idle(), idle()]);
    let effects = Effects::answering(MERGE, vec![pending("merge queued")]);
    let authority = Allowing::then_denying(2);

    let (end, _) = run_with(
        &governor,
        &executor,
        &authority,
        &effects,
        &case,
        &mut Context::new(None),
    );

    let made = request(1, &end.run_id, &case, 7, MERGE);
    assert_eq!(
        authority.asked(),
        vec![made.clone(); 3],
        "the provider is asked about the same request before each invocation"
    );
    assert_eq!(
        effects.invoked(),
        vec![made.clone(), made.clone()],
        "invoked while allowed, and not once denied"
    );
    let reread = vec![GovernorCall::CurrentRevision(case.clone())];
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            reread.clone(),
            iteration(&case),
            revalidation(&case),
            reread,
            iteration(&case),
            revalidation(&case),
            iteration(&case),
        ]
        .concat(),
        "the revision is read again after each allow, and the iteration that finds the request \
         denied goes on to the executor"
    );
    assert_eq!(
        end.requests[2],
        Revalidated {
            request: made,
            outcome: RevalidateActionRequestOutcome::NeedsAuthority {
                error: ActionNeedsAuthority {
                    action: MERGE.to_owned(),
                    capability: CAPABILITY.to_owned(),
                },
            },
        },
        "the denied request is recorded with its revalidation"
    );
    assert_eq!(
        executor.calls().len(),
        3,
        "the executor runs in the iteration that found the request denied, and after it"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
}

/// A step budget of two: the proposal answered pending and one invocation again, answered pending
/// too, use it up, and the run ends `Waiting`, naming the request and the step budget's reason,
/// where it would otherwise be suspended for its budget. The Run is suspended for that reason.
#[test]
fn a_budget_reached_while_pending_ends_the_run_waiting() {
    let case = CaseId("case-pending-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![pending("job running")]);

    let (end, runs) = run(&governor, &executor, &effects, &case, Some(2));

    let budget = SuspensionReason::Budget(Value::Object(vec![(
        "max_steps".to_owned(),
        Value::Number("2".to_owned()),
    )]));
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: budget,
        }),
        "the outcome names the pending request and the budget"
    );
    assert_eq!(effects.invoked().len(), 2, "invoked within the budget only");
    assert_eq!(end.steps, 2);
    assert_eq!(
        stored_state(&runs, &end.run_id),
        RunState::Suspended,
        "the Run is suspended for its budget, as at any budget stop"
    );
}

/// A trial limit of two on the action: the request is answered pending twice, and the run ends
/// `Waiting` before a third trial is handed to the port, naming the request and the trial limit.
/// The same run whose request was performed on its second trial stops on its next request as
/// before: `Suspended`, since nothing is pending.
#[test]
fn a_trial_limit_reached_while_pending_ends_the_run_waiting() {
    let case = CaseId("case-pending-trials".to_owned());
    let limits = RunBudget {
        max_steps: None,
        max_tokens_in: None,
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
        max_tokens: None,
        deadline: None,
        per_action: vec![ActionBudget {
            action: CHECK.to_owned(),
            max_trials: Some(2),
            max_tokens_in: None,
            max_tokens_out: None,
            max_duration_ms: None,
            max_cost_micro_usd: None,
        }],
    };
    let breach = SuspensionReason::RunBudget(RunBudgetBreach {
        action: Some(CHECK.to_owned()),
        limit: BudgetLimit::MaxTrials,
        max: 2,
        charged: Some(2),
    });
    let ran = |answers: Vec<EffectOutcome>, script: Vec<ExecutorOutcome>| {
        let governor = FakeGovernor::new();
        governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
        let effects = Effects::answering(CHECK, answers);
        let mut context = Context::new(None);
        context.run_budget = Some(limits.clone());
        let (end, _) = run_with(
            &governor,
            &ScriptedExecutor::new(script),
            &StaticAuthorityProvider::new(),
            &effects,
            &case,
            &mut context,
        );
        (end, effects.invoked().len())
    };

    let (waiting, invoked) = ran(vec![pending("job running")], vec![proposal(CHECK), idle()]);
    assert_eq!(
        waiting.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: breach.clone(),
        })
    );
    assert_eq!(invoked, 2, "no third trial is handed to the port");

    let other = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: CHECK.to_owned(),
        arguments: ProposedActionArguments(Value::Text("another commit".to_owned())),
    });
    let (suspended, invoked) = ran(
        vec![pending("job running"), performed(CHECK)],
        vec![proposal(CHECK), other, idle()],
    );
    assert_eq!(
        suspended.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended { reason: breach }),
        "with nothing pending, a budget stop is suspended as before"
    );
    assert_eq!(invoked, 2);
}

/// Every answer, pending or not, is one trial charged to the run and to the request's action, and
/// one observation of the effect: its outcome, the request, its action and arguments and the
/// port's report. None becomes evidence.
#[test]
fn each_answer_is_charged_and_observed() {
    let case = CaseId("case-pending-charged".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(
        CHECK,
        vec![
            pending("job queued"),
            pending("job running"),
            performed(CHECK),
        ],
    );

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let charged = ChargedUsage {
        trials: 3,
        input_tokens: 0,
        output_tokens: 0,
        duration_ms: 0,
        cost_micro_usd: Some(0),
    };
    assert_eq!(end.charges.run, charged, "three trials on the run");
    assert_eq!(
        end.charges.actions,
        vec![ActionCharge {
            action: CHECK.to_owned(),
            charged,
        }],
        "three trials on the action"
    );

    let made = request(1, &end.run_id, &case, 4, CHECK);
    let effects: Vec<_> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == "effect")
        .collect();
    assert_eq!(effects.len(), 3, "one observation per answer");
    for (observation, (outcome, report)) in effects.iter().zip([
        ("Pending", "job queued".to_owned()),
        ("Pending", "job running".to_owned()),
        ("Performed", format!("done {CHECK}")),
    ]) {
        let member = |key: &str| observation.payload.member(key).cloned();
        assert_eq!(observation.subject, "case-pending-charged@4");
        assert_eq!(observation.observed_at, Timestamp(NOW.to_owned()));
        assert_eq!(member("outcome"), Some(Value::Text(outcome.to_owned())));
        assert_eq!(
            member("action_request"),
            Some(Value::Text(made.action_request_id.0.0.clone()))
        );
        assert_eq!(member("action"), Some(Value::Text(CHECK.to_owned())));
        assert_eq!(member("arguments"), Some(made.arguments.0.clone()));
        assert_eq!(member("report"), Some(Value::Text(report)));
    }
    assert!(
        governor.evidence().is_empty(),
        "an effect became evidence: {:?}",
        governor.evidence()
    );
}

/// A fan-out of two reads, one answered pending and one performed: the pending read alone is
/// invoked again at the next iteration, through the port's fan-out, until it is performed, and the
/// run goes on. The performed read is not invoked again.
#[test]
fn a_pending_read_of_a_fan_out_is_invoked_again_alone() {
    let case = CaseId("case-pending-read".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("search"), admissible("fetch")])],
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch"]), idle(), idle()]);
    let effects = Effects::answering(
        "search",
        vec![pending("index building"), performed("search")],
    )
    .with("fetch", vec![performed("fetch")])
    .reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let search = request(1, &end.run_id, &case, 4, "search");
    let fetch = request(2, &end.run_id, &case, 4, "fetch");
    assert_eq!(
        end.admitted,
        vec![search.clone(), fetch, search],
        "the pending read is handed to the port again, and the performed one is not"
    );
    assert_eq!(
        end.effects,
        vec![
            pending("index building"),
            performed("fetch"),
            performed("search")
        ]
    );
    assert_eq!(executor.calls().len(), 3, "the run goes on");
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
}

/// A step budget of one on the same fan-out, its read answered pending: the run ends `Waiting`,
/// naming the pending read alone.
#[test]
fn a_budget_reached_while_a_read_is_pending_names_that_read() {
    let case = CaseId("case-pending-read-budget".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("search"), admissible("fetch")])],
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch"]), idle()]);
    let effects = Effects::answering("search", vec![pending("index building")])
        .reading(vec!["search", "fetch"]);

    let (end, _) = run(&governor, &executor, &effects, &case, Some(1));

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: SuspensionReason::Budget(Value::Object(vec![(
                "max_steps".to_owned(),
                Value::Number("1".to_owned()),
            )])),
        })
    );
}

/// An action the frontier lists beside the pending one, which the executor may still take.
const INSPECT: &str = "repository.inspect";

/// A run store whose new runs take ids from `0x100 + base + 1` on, for a loop that suspends or
/// resumes a Run.
fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    })
}

/// A resumed request answered pending: the resume releases the Suspension it took, since the port
/// answered, and the loop's first iteration invokes the request again, revalidated and asked about
/// again, until it is performed; only then does the executor run. Nothing is taken from the
/// released Suspension.
#[test]
fn a_resumed_request_answered_pending_is_released_and_invoked_again() {
    let case = CaseId("case-pending-resumed".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            7,
            vec![
                FrontierAction {
                    action: MERGE.to_owned(),
                    status: ActionStatus::ApprovalRequired,
                    capability: Some(CAPABILITY.to_owned()),
                    reasons: Vec::new(),
                },
                admissible(CHECK),
            ],
        )],
    );
    let effects = Effects::answering(MERGE, vec![pending("merge queued"), performed(MERGE)]);
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
    assert!(
        effects.invoked().is_empty(),
        "precondition: nothing invoked"
    );

    let executor = ScriptedExecutor::new([idle(), idle()]);
    let authority =
        StaticAuthorityProvider::new().answer(CAPABILITY, AuthorityVerdict::Allow(Unit(true)));
    let mut context = Context::new(None);
    context.requests = 0x380;
    context.observations = 0x480;
    let end = resume_after_grant(
        &governor,
        &executor,
        &authority,
        &effects,
        &commission(&case),
        &mut runs(0x80),
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    let held = request(1, &suspended.run_id, &case, 7, MERGE);
    assert_eq!(
        effects.invoked(),
        vec![held.clone(), held.clone()],
        "invoked by the resume, and again by the loop, under its own id"
    );
    assert_eq!(
        authority.requests(),
        vec![held.clone(), held],
        "the provider is asked at the resume and again before the invocation again"
    );
    assert_eq!(end.effects, vec![pending("merge queued"), performed(MERGE)]);
    let stored = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"));
    assert_eq!(
        stored, None,
        "the Suspension is released once the port answered, pending or not"
    );
    assert_eq!(
        executor.calls().len(),
        1,
        "the executor runs only once the request was performed"
    );
    // This fake's frontier keeps listing the merge as needing approval, so the executor's idle
    // step leaves it unchanged and the approval gate ends the run, as it would after a resumed
    // request performed at once.
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec![MERGE.to_owned()],
        })
    );
}

/// The frontier stops listing the pending request's action, the case still at its revision: the
/// revalidation before the invocation again refuses it, so it is recorded with that answer, not
/// invoked again and no longer pending, and the same iteration goes on to the executor, which is
/// handed the frontier as it is now.
#[test]
fn a_pending_request_the_frontier_no_longer_admits_is_dropped_and_the_executor_runs() {
    let case = CaseId("case-pending-not-admitted".to_owned());
    let governor = FakeGovernor::new();
    // The first iteration and the request's revalidation (five calls) list `ci.check`; every call
    // after lists only `repository.inspect`, at the same revision.
    governor.script(
        case.clone(),
        std::iter::repeat_n(listing(4, vec![admissible(CHECK), admissible(INSPECT)]), 5)
            .chain([listing(4, vec![admissible(INSPECT)])]),
    );
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![pending("job running")]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(effects.invoked(), vec![made.clone()], "not invoked again");
    assert_eq!(end.requests.len(), 2, "{:?}", end.requests);
    assert!(
        matches!(
            &end.requests[1],
            Revalidated {
                request,
                outcome: RevalidateActionRequestOutcome::NotAdmitted { .. },
            } if *request == made
        ),
        "the refused revalidation is recorded: {:?}",
        end.requests[1]
    );
    assert_eq!(
        governor.calls(),
        [
            iteration(&case),
            revalidation(&case),
            iteration(&case),
            revalidation(&case),
            iteration(&case),
        ]
        .concat()
    );
    assert_eq!(
        executor.calls().len(),
        3,
        "the iteration that dropped the request runs the executor, and so does the next"
    );
    let handed: Vec<String> = executor.frontiers()[1]
        .actions
        .iter()
        .map(|listed| listed.action.clone())
        .collect();
    assert_eq!(
        handed,
        [INSPECT],
        "the executor is handed the frontier as it is now"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
    assert_eq!(end.steps, 2);
}

/// The frontier of the approval tests: the merge needs approval of `CAPABILITY`, and `ci.check`
/// is admissible beside it.
fn gated_listing(revision: i64) -> Answer {
    listing(
        revision,
        vec![
            FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            },
            admissible(CHECK),
        ],
    )
}

/// The provider allows the pending merge at its first invocation and asks for approval when it is
/// asked again before the next. The run ends `NeedsAuthority`, naming the provider's request, as
/// at a first `ApprovalRequired`; under `run_with_suspension` the Run is suspended and the
/// commission's Suspension holds the pending request, at its revision, for a resume to perform
/// once approved. The request is never dropped for the executor to run; without a suspension port
/// the run ends `NeedsAuthority` all the same.
#[test]
fn approval_required_when_a_pending_request_is_asked_again_ends_the_run_needing_authority() {
    const AGAIN: &str = "approve the merge again";
    let withheld = || {
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: AGAIN.to_owned(),
        })
    };
    let needs = RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
        request: AGAIN.to_owned(),
    });
    let case = CaseId("case-pending-approval-again".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated_listing(7)]);
    let effects = Effects::answering(MERGE, vec![pending("merge queued")]);
    let authority = Allowing::then(1, withheld());
    let executor = ScriptedExecutor::new([proposal(MERGE), idle(), idle()]);
    let suspensions = SuspensionStore::new();
    let mut store = runs(0);

    let end = run_with_suspension(
        &governor,
        &executor,
        &authority,
        &effects,
        &commission(&case),
        &mut store,
        &suspensions,
        &mut Context::new(None),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    let made = request(1, &end.run_id, &case, 7, MERGE);
    assert_eq!(end.outcome, needs, "the run ends needing authority");
    assert_eq!(
        effects.invoked(),
        vec![made.clone()],
        "invoked once, while allowed"
    );
    assert_eq!(
        authority.asked(),
        vec![made.clone(); 2],
        "asked before the first invocation and again before the next"
    );
    assert_eq!(
        executor.calls().len(),
        1,
        "no executor runs once the request is withheld"
    );
    let held = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("no Suspension holds the withheld request"));
    assert_eq!(held.state, SuspensionState::Pending);
    assert_eq!(held.data.action_request_id, made.action_request_id);
    assert_eq!(held.data.expected_case_revision, 7);
    assert_eq!(
        held.data.reason,
        SuspensionReason::Authority(Value::Object(vec![
            ("capability".to_owned(), Value::Text(CAPABILITY.to_owned())),
            ("request".to_owned(), Value::Text(AGAIN.to_owned())),
        ]))
    );
    let run = RunStorage::get(&store, &end.run_id)
        .unwrap_or_else(|| panic!("the Run {} is not stored", end.run_id.0.0));
    assert_eq!(
        run.state,
        RunState::Suspended,
        "the Run is suspended for it"
    );

    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated_listing(7)]);
    let (plain, _) = run_with(
        &governor,
        &ScriptedExecutor::new([proposal(MERGE), idle(), idle()]),
        &Allowing::then(1, withheld()),
        &Effects::answering(MERGE, vec![pending("merge queued")]),
        &case,
        &mut Context::new(None),
    );
    assert_eq!(plain.outcome, needs, "without a suspension port too");
}

/// Three reads proposed together, the first and the third answered pending, the second performed,
/// under a step budget of one: the run ends `Waiting`, naming both pending reads in the order they
/// were proposed, and not the performed one.
#[test]
fn a_budget_reached_while_two_reads_are_pending_names_both_in_order() {
    let case = CaseId("case-pending-two-reads".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("search"),
                admissible("fetch"),
                admissible("scan"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch", "scan"]), idle()]);
    let effects = Effects::answering("search", vec![pending("index building")])
        .with("scan", vec![pending("scan queued")])
        .reading(vec!["search", "fetch", "scan"]);

    let (end, _) = run(&governor, &executor, &effects, &case, Some(1));

    assert_eq!(
        end.effects,
        vec![
            pending("index building"),
            performed("fetch"),
            pending("scan queued")
        ],
        "precondition: the first and the third read are pending"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301)), ActionRequestId(uuid(0x303))],
            reason: SuspensionReason::Budget(Value::Object(vec![(
                "max_steps".to_owned(),
                Value::Number("1".to_owned()),
            )])),
        })
    );
}

/// The port answers the proposed request pending with `ends_run`: it is invoked once and never
/// again, the run ends `Waiting`, naming it, for the time the port's report names, the executor is
/// called once, and the Run is suspended for that reason. On 0.28.0 a pending answer could not end
/// the run: the request was invoked again at the next iteration.
#[test]
fn a_pending_answer_that_ends_the_run_is_not_invoked_again() {
    let case = CaseId("case-pending-ends-run".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![ending("job queued"), performed(CHECK)]);

    let (end, runs) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(effects.invoked(), vec![made], "the port is invoked once");
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: time("job queued"),
        })
    );
    assert_eq!(executor.calls().len(), 1, "the executor is called once");
    assert_eq!(
        stored_state(&runs, &end.run_id),
        RunState::Suspended,
        "the Run is suspended for the time the port named"
    );
}

/// The answer that ends the run is recorded in the loop's effects, charged one trial to the run
/// and to its action, and delivered as one observation, never as evidence.
#[test]
fn a_pending_answer_that_ends_the_run_is_recorded_charged_and_observed_once() {
    let case = CaseId("case-pending-ends-run-charged".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![ending("job queued")]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(end.effects, vec![ending("job queued")], "recorded once");
    let charged = ChargedUsage {
        trials: 1,
        input_tokens: 0,
        output_tokens: 0,
        duration_ms: 0,
        cost_micro_usd: Some(0),
    };
    assert_eq!(end.charges.run, charged, "one trial on the run");
    assert_eq!(
        end.charges.actions,
        vec![ActionCharge {
            action: CHECK.to_owned(),
            charged,
        }],
        "one trial on the action"
    );
    let observed: Vec<_> = governor
        .observations()
        .into_iter()
        .filter(|observation| observation.source == "effect")
        .collect();
    assert_eq!(observed.len(), 1, "observed once");
    let member = |key: &str| observed[0].payload.member(key).cloned();
    assert_eq!(member("outcome"), Some(Value::Text("Pending".to_owned())));
    assert_eq!(member("report"), Some(Value::Text("job queued".to_owned())));
    assert!(
        governor.evidence().is_empty(),
        "an effect became evidence: {:?}",
        governor.evidence()
    );
}

/// `ends_run: false` is the pending answer as before: the request is invoked again at the next
/// iteration, with no executor call between, until the port answers otherwise, and the run goes
/// on.
#[test]
fn a_pending_answer_with_ends_run_false_is_invoked_again() {
    let case = CaseId("case-pending-ends-run-false".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(CHECK, vec![pending("job queued"), performed(CHECK)]);

    let (end, _) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(effects.invoked(), vec![made.clone(), made]);
    assert_eq!(end.effects, vec![pending("job queued"), performed(CHECK)]);
    assert_eq!(executor.calls().len(), 3, "the run goes on once performed");
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 })
    );
}

/// A request answered pending, invoked again and answered pending with `ends_run`: it is not
/// invoked a third time, and the run ends `Waiting` for the time the second answer names.
#[test]
fn a_pending_request_invoked_again_that_ends_the_run_is_not_invoked_a_third_time() {
    let case = CaseId("case-pending-again-ends-run".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [listing(4, vec![admissible(CHECK)])]);
    let executor = ScriptedExecutor::new([proposal(CHECK), idle(), idle()]);
    let effects = Effects::answering(
        CHECK,
        vec![
            pending("job queued"),
            ending("job running"),
            performed(CHECK),
        ],
    );

    let (end, runs) = run(&governor, &executor, &effects, &case, None);

    let made = request(1, &end.run_id, &case, 4, CHECK);
    assert_eq!(effects.invoked(), vec![made.clone(), made], "invoked twice");
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x301))],
            reason: time("job running"),
        })
    );
    assert_eq!(executor.calls().len(), 1);
    assert_eq!(stored_state(&runs, &end.run_id), RunState::Suspended);
}

/// Four reads proposed together: `search` answered pending, `fetch` performed, and `scan` and
/// `crawl` pending with `ends_run`. Once every read joined, the run ends `Waiting`, naming every
/// read answered pending, with or without `ends_run`, in proposal order, for the time the report of
/// `scan`, the first that asked to end the run, names. No read is invoked again.
#[test]
fn a_fan_out_whose_read_ends_the_run_waits_on_every_pending_read() {
    let case = CaseId("case-pending-read-ends-run".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("search"),
                admissible("fetch"),
                admissible("scan"),
                admissible("crawl"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([fan_out(&["search", "fetch", "scan", "crawl"]), idle()]);
    let effects = Effects::answering("search", vec![pending("index building")])
        .with("scan", vec![ending("scan queued")])
        .with("crawl", vec![ending("crawl queued")])
        .reading(vec!["search", "fetch", "scan", "crawl"]);

    let (end, runs) = run(&governor, &executor, &effects, &case, None);

    assert_eq!(
        end.effects,
        vec![
            pending("index building"),
            performed("fetch"),
            ending("scan queued"),
            ending("crawl queued"),
        ],
        "every read joined and was recorded"
    );
    assert_eq!(effects.invoked().len(), 4, "each read is invoked once");
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![
                ActionRequestId(uuid(0x301)),
                ActionRequestId(uuid(0x303)),
                ActionRequestId(uuid(0x304)),
            ],
            reason: time("scan queued"),
        })
    );
    assert_eq!(executor.calls().len(), 1, "no executor runs after it");
    assert_eq!(stored_state(&runs, &end.run_id), RunState::Suspended);
}

/// The run budget's spending limits are checked first: a fan-out whose performed read passed the
/// run's input-token limit, beside a read answered pending with `ends_run`, ends `Waiting` for the
/// breach, naming the pending read, as at any budget stop.
#[test]
fn a_breach_beside_a_pending_answer_that_ends_the_run_ends_it_first() {
    let case = CaseId("case-pending-ends-run-breach".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(4, vec![admissible("fetch"), admissible("scan")])],
    );
    let executor = ScriptedExecutor::new([fan_out(&["fetch", "scan"]), idle()]);
    let spent = EffectOutcome::Performed(EffectOutcomePerformed {
        report: Value::Text("done fetch".to_owned()),
        attempt: None,
        audit: None,
        usage: Some(EffectUsage {
            input_tokens: 150,
            output_tokens: 0,
            duration_ms: 0,
            cost_micro_usd: None,
        }),
    });
    let effects = Effects::answering("fetch", vec![spent])
        .with("scan", vec![ending("scan queued")])
        .reading(vec!["fetch", "scan"]);
    let mut context = Context::new(None);
    context.run_budget = Some(RunBudget {
        max_steps: None,
        max_tokens_in: Some(100),
        max_tokens_out: None,
        max_duration_ms: None,
        max_cost_micro_usd: None,
        max_tokens: None,
        deadline: None,
        per_action: Vec::new(),
    });

    let (end, _) = run_with(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &case,
        &mut context,
    );

    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![ActionRequestId(uuid(0x302))],
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxTokensIn,
                max: 100,
                charged: Some(150),
            }),
        })
    );
    assert_eq!(effects.invoked().len(), 2);
}

/// A resumed request answered pending with `ends_run`: the resume releases the Suspension it took
/// first, the request is not invoked again, no executor runs, and the run ends `Waiting`, naming
/// the request, for the time the port's report names, the Run suspended for it.
#[test]
fn a_resumed_request_whose_pending_answer_ends_the_run_is_released_and_waits() {
    let case = CaseId("case-pending-resumed-ends-run".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated_listing(7)]);
    let effects = Effects::answering(MERGE, vec![ending("merge queued"), performed(MERGE)]);
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
    assert!(
        matches!(suspended.outcome, RunOutcome::NeedsAuthority(_)),
        "precondition: the merge is withheld and held as a Suspension"
    );

    let executor = ScriptedExecutor::new([idle(), idle()]);
    let authority =
        StaticAuthorityProvider::new().answer(CAPABILITY, AuthorityVerdict::Allow(Unit(true)));
    let mut context = Context::new(None);
    context.requests = 0x380;
    context.observations = 0x480;
    let mut store = runs(0x80);
    let end = resume_after_grant(
        &governor,
        &executor,
        &authority,
        &effects,
        &commission(&case),
        &mut store,
        &suspensions,
        &mut context,
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    let held = request(1, &suspended.run_id, &case, 7, MERGE);
    assert_eq!(
        effects.invoked(),
        vec![held.clone()],
        "invoked by the resume and not again"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::Waiting(RunOutcomeWaiting {
            action_request_ids: vec![held.action_request_id],
            reason: time("merge queued"),
        })
    );
    let stored = SuspensionPort::get(&suspensions, &commission(&case).data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"));
    assert_eq!(stored, None, "the Suspension is released");
    assert!(executor.calls().is_empty(), "no executor runs");
    let run = RunStorage::get(&store, &end.run_id)
        .unwrap_or_else(|| panic!("the Run {} is not stored", end.run_id.0.0));
    assert_eq!(run.state, RunState::Suspended);
}
