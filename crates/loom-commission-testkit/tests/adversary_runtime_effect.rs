//! Adversary pass 1 on `story:runtime-merge`: Commission's runtime invokes admitted effects.
//!
//! Each case states the claim it drives the runtime against (`crates/loom-commission/src/runtime.rs`,
//! `crates/loom-commission/src/ports/effect.rs`) and fails when the runtime breaks it.

use std::collections::VecDeque;
use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, CompletionDetermination, EffectOutcome,
    EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, Frontier, FrontierAction, FrontierData, FrontierId, GovernorError,
    Observation, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
    SuspensionReason, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
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
        principal: PrincipalId("principal-a".to_owned()),
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

fn listing(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn proposal(name: &str, arguments: Value) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(arguments),
    })
}

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
        Timestamp("2026-10-05T09:00:00Z".to_owned())
    }

    /// No budget: the module promises the idle bound ends the loop instead.
    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

/// An effect port performing exactly `performs` (every action when `None`), answering scripted
/// outcomes and then `Performed`, recording every request it is handed.
struct Effects {
    performs: Option<Vec<&'static str>>,
    answers: Mutex<VecDeque<EffectOutcome>>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn new(performs: Option<Vec<&'static str>>, answers: Vec<EffectOutcome>) -> Self {
        Self {
            performs,
            answers: Mutex::new(answers.into()),
            invoked: Mutex::new(Vec::new()),
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
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.data().clone());
        Ok(self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or(EffectOutcome::Performed(EffectOutcomePerformed {
                report: Value::Null,
                attempt: None,
            })))
    }
}

/// `ports/effect.rs`: `performs` says "whether this port performs `action` at all". A port that
/// declares it does not perform `deploy` is nevertheless handed an admitted `deploy` request: the
/// runtime checks `performs` only to decide whether to run the executor at all, never before
/// `invoke`. The port is the only line left between an admitted action and its effect.
#[test]
fn adversary_an_action_the_port_does_not_perform_is_never_invoked() {
    let case = CaseId("case-not-performed".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(5, vec![admissible("edit"), admissible("deploy")]);
    governor.script(
        case.clone(),
        repeat_n(open.clone(), 5).chain([open.complete("W")]),
    );
    let executor = ScriptedExecutor::new([proposal("deploy", Value::Null)]);
    let effects = Effects::new(Some(vec!["edit"]), Vec::new());

    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut store(),
        &mut Context::default(),
    );

    let handed: Vec<String> = effects.invoked().into_iter().map(|r| r.action).collect();
    assert!(
        !handed.iter().any(|action| action == "deploy"),
        "the port said it does not perform `deploy` and was handed it anyway: {handed:?}; \
         loop result {result:?}"
    );
}

/// `runtime.rs` module docs: "A Run holds the case at that revision, or at a revision its own
/// effect moved the case to". The effect here is `Refused`: it changed nothing (`ports/effect.rs`:
/// "that it refused and changed nothing"). The case then moves 5 -> 9 by somebody else. That move
/// is not the Run's own, so the run ends with no admissible action and nothing more is invoked.
/// The runtime instead adopts revision 9 because the previous iteration "invoked an effect", and
/// hands the port a second request at 9.
#[test]
fn adversary_a_refused_effect_does_not_adopt_a_foreign_move() {
    let case = CaseId("case-foreign-move".to_owned());
    let governor = FakeGovernor::new();
    let at = |revision| listing(revision, vec![admissible("inspect")]);
    governor.script(
        case.clone(),
        repeat_n(at(5), 5)
            .chain(repeat_n(at(9), 5))
            .chain([at(9).complete("V")]),
    );
    let executor = ScriptedExecutor::new([
        proposal("inspect", Value::Null),
        proposal("inspect", Value::Null),
    ]);
    let refused = EffectOutcome::Refused(EffectOutcomeRefused {
        reason: "no such path".to_owned(),
    });
    let effects = Effects::new(None, vec![refused]);

    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut store(),
        &mut Context::default(),
    );

    let revisions: Vec<i64> = effects
        .invoked()
        .iter()
        .map(|r| r.expected_case_revision)
        .collect();
    assert_eq!(
        revisions,
        [5],
        "only the request at the Run's revision may take effect; result {result:?}"
    );
    let end = result.unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        end.outcome,
        RunOutcome::NoAdmissibleAction(Unit(true)),
        "a move no effect of this Run made ends it"
    );
}

/// A governor whose case revision only the effect port moves, with the same frontier actions at
/// every revision. Never complete.
struct Moving {
    case: CaseId,
    revision: Mutex<i64>,
}

impl Moving {
    fn revision(&self) -> i64 {
        *self.revision.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Governor for Moving {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision())
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision();
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0x900 + u64::try_from(revision).unwrap_or(0))),
            case_id: self.case.clone(),
            case_revision: revision,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![admissible("edit")],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Moving {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// An effect port that performs every action and moves the case one revision on.
struct Bumps<'g>(&'g Moving);

impl EffectPort for Bumps<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        *self
            .0
            .revision
            .lock()
            .unwrap_or_else(PoisonError::into_inner) += 1;
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
        }))
    }
}

/// The cap after which the test's executor suspends, so a loop that never ends still returns.
const CAP: usize = 25;

/// An executor that proposes the same `edit`, with the same arguments, on every frontier, and
/// suspends for `adversary-cap` on its `CAP`-th call.
struct SameEdit {
    calls: Mutex<usize>,
}

impl AgentExecutor for SameEdit {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let mut calls = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
        *calls += 1;
        if *calls >= CAP {
            return ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: SuspensionReason::ExternalAvailability(Value::Text(
                    "adversary-cap".to_owned(),
                )),
            });
        }
        proposal("edit", Value::Text("same".to_owned()))
    }
}

/// `LoopContext::step_budget`: "`None` for no budget, where the idle bound ends the loop instead".
/// Before this story no effect ran, so an identical proposal was idle and the loop ended after two.
/// Now each identical `edit` takes effect and moves the case, the moved revision makes it a "new"
/// request, the idle count never starts, and with no budget the loop runs until something outside
/// it stops it: here the test's executor, at its 25th call.
#[test]
fn adversary_without_a_budget_an_effect_that_moves_the_case_still_ends() {
    let case = CaseId("case-unbounded".to_owned());
    let governor = Moving {
        case: case.clone(),
        revision: Mutex::new(1),
    };
    let effects = Bumps(&governor);
    let executor = SameEdit {
        calls: Mutex::new(0),
    };

    let result = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(&case),
        &mut store(),
        &mut Context::default(),
    );

    let calls = *executor
        .calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let end = result.unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert!(
        calls < CAP,
        "the loop without a budget ran the executor {calls} times on an unchanged list of actions \
         (the same `edit` every time, {} effects) and ended only at the test's cap: {:?}",
        end.effects.len(),
        end.outcome
    );
}
