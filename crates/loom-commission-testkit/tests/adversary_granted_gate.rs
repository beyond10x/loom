//! Adversary cases for `story:granted-gate-ends-run` (wave 2026-10-12-w1, unit U2): the approval
//! gate after a step performed under an authority grant (`runtime.rs` module doc, items 0 and 5;
//! `docs/commission/design/approval-gate.md`).
//!
//! The first case is red against the tree it was written for. The others hold rules of the design
//! note that the unit's own suite does not drive (a grant with a refused effect; a granted step
//! followed by an ungated one; a grant for one action beside an action only a person approves;
//! the idle bound over repeated granted steps), and are green.

use std::collections::VecDeque;
use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, CaseId, Commission, CommissionData, CommissionId, EffectOutcome,
    EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction,
    FrontierAction, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
    RunOutcomeAwaitingApproval, RunOutcomeCompleted, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

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

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
    budget: Option<usize>,
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
        Timestamp("2026-10-12T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        self.budget
    }
}

/// Performs every action; answers each invocation with the next scripted answer, `Performed`
/// once the script is used up; records every request it was handed.
#[derive(Default)]
struct Effects {
    answers: Mutex<VecDeque<EffectOutcome>>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn answering(answers: Vec<EffectOutcome>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
            ..Self::default()
        }
    }

    fn invoked(&self) -> Vec<String> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|request| request.action.clone())
            .collect()
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
        let answer = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| {
                EffectOutcome::Performed(EffectOutcomePerformed {
                    report: Value::Text(format!("done {}", data.action)),
                    attempt: None,
                    audit: None,
                })
            });
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(answer)
    }
}

fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    authority: &StaticAuthorityProvider,
    effects: &Effects,
    case: &CaseId,
    budget: Option<usize>,
) -> LoopEnd {
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(case),
        &mut runs,
        &mut Context {
            budget,
            ..Context::default()
        },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// A granted step that completes the case on the Run's last budgeted step ends the Run
/// `Completed`, not suspended for its budget.
///
/// The frontier lists `read` and `propose`, both needing a capability the provider allows; the
/// executor proposes `read`, then `propose`, and the governor holds the case complete once
/// `propose` is performed. With a budget of 2, `propose` is performed on the last step. Before
/// this change the step left the gate pending, so the next iteration loaded the case (item 0)
/// and found it complete. Now a granted step leaves no gate pending and the Run is suspended for
/// its budget without loading the case: the completed case is reported as a Run out of budget.
/// The plugin turn reaches exactly this (`TURN_STEP_BUDGET` 8, a proposal on the eighth step),
/// see `crates/loom-plugin/tests/adversary_granted_gate.rs`.
#[test]
fn a_granted_step_that_completes_the_case_at_the_budget_ends_completed() {
    let open = || {
        listing(
            4,
            vec![
                gated("read", "datasource.read"),
                gated("propose", "reply.propose"),
            ],
        )
    };
    let case = CaseId("case-granted-completes-at-budget".to_owned());
    let governor = FakeGovernor::new();
    // Two iterations of three calls and two revalidations of two: ten open answers, then the case
    // as `propose` left it.
    governor.script(
        case.clone(),
        repeat_n(open(), 10).chain([open().complete("answered")]),
    );
    let executor = ScriptedExecutor::new([proposal("read"), proposal("propose")]);
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new()
        .answer("datasource.read", allow())
        .answer("reply.propose", allow());
    let end = run(&governor, &executor, &authority, &effects, &case, Some(2));
    assert_eq!(
        effects.invoked(),
        ["read", "propose"],
        "precondition: both granted effects performed"
    );
    assert_eq!(end.steps, 2, "precondition: the budget is used up");
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "answered".to_owned()
        }),
        "the case the last granted step completed"
    );
}

/// An allowed request whose effect the port refused reads the gate as before (design note: "one
/// whose request was refused ... or not performed"): the next iteration ends `AwaitingApproval`
/// and the executor is not run again. Kills the mutant that drops `effected &&` from `granted`.
#[test]
fn an_allowed_but_refused_step_reads_the_gate() {
    let case = CaseId("case-allowed-refused".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("inspect"),
                gated("read", "datasource.read"),
                gated("propose", "reply.propose"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("read")]);
    let effects = Effects::answering(vec![EffectOutcome::Refused(EffectOutcomeRefused {
        reason: "no such source".to_owned(),
    })]);
    let authority = StaticAuthorityProvider::new().answer("datasource.read", allow());
    let end = run(&governor, &executor, &authority, &effects, &case, Some(10));
    assert_eq!(effects.invoked(), ["read"], "the allowed read was invoked");
    assert_eq!(executor.calls().len(), 1, "the executor ran once");
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["read".to_owned(), "propose".to_owned()],
        })
    );
}

/// A granted step followed by an ungated step that leaves the gate unchanged: the gate is read
/// after the second step, and the run ends `AwaitingApproval`, naming the granted action too.
#[test]
fn an_ungated_step_after_a_granted_one_reads_the_gate() {
    let case = CaseId("case-granted-then-ungated".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("inspect"),
                gated("read", "datasource.read"),
                gated("merge", "repository.merge"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("read"), proposal("inspect")]);
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new().answer("datasource.read", allow());
    let end = run(&governor, &executor, &authority, &effects, &case, Some(10));
    assert_eq!(effects.invoked(), ["read", "inspect"]);
    assert_eq!(executor.calls().len(), 2);
    assert_eq!(end.steps, 2);
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["read".to_owned(), "merge".to_owned()],
        })
    );
}

/// A grant for one action beside an action only a person approves (`ApprovalRequired`, no
/// capability): the design note lets the granted step pass the whole gate, so the executor runs
/// again on the iteration after the grant; the gate forms again after the next step, which
/// proposes nothing, and holds `merge`.
#[test]
fn a_grant_for_one_action_passes_the_gate_of_another_for_one_step() {
    let case = CaseId("case-grant-beside-human".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                admissible("inspect"),
                gated("read", "datasource.read"),
                action("merge", ActionStatus::ApprovalRequired, None),
            ],
        )],
    );
    let executor = ScriptedExecutor::new([proposal("read"), idle()]);
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new().answer("datasource.read", allow());
    let end = run(&governor, &executor, &authority, &effects, &case, Some(10));
    assert_eq!(
        executor.calls().len(),
        2,
        "the executor ran after the grant"
    );
    assert_eq!(
        end.outcome,
        RunOutcome::AwaitingApproval(RunOutcomeAwaitingApproval {
            actions: vec!["read".to_owned(), "merge".to_owned()],
        })
    );
}

/// Without a step budget the idle bound ends a Run that only repeats one granted step (design
/// note), with no admissible action; the same granted effect is performed three times on an
/// unchanged frontier, where before this change it was performed once and the Run ended
/// `AwaitingApproval`.
#[test]
fn the_idle_bound_ends_a_run_that_repeats_a_granted_step() {
    let case = CaseId("case-granted-repeat".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [listing(
            4,
            vec![
                gated("read", "datasource.read"),
                gated("propose", "reply.propose"),
            ],
        )],
    );
    let executor = ScriptedExecutor::new(repeat_n(proposal("read"), 3));
    let effects = Effects::default();
    let authority = StaticAuthorityProvider::new().answer("datasource.read", allow());
    let end = run(&governor, &executor, &authority, &effects, &case, None);
    assert_eq!(effects.invoked(), ["read", "read", "read"]);
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
}
