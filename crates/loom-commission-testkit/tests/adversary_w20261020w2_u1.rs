//! Adversary cases for `story:refused-effect-ends-run` (wave 2026-10-20-w2, U1): a refusal the
//! effect port marks final (`EffectOutcomeRefused.ends_run`) against the outcomes it competes with.
//!
//! `docs/commission/contracts/commission-executor.md` promises an executor that a stale read in a
//! fan-out is judged as a move: "A read that is stale shows that somebody else moved the case: once
//! the others are recorded, the runtime judges the move as after a stale proposal". The runtime's
//! own item 8a says the same ("When a read was stale, somebody else moved the case: once the
//! admitted reads were recorded, the iteration is as a stale proposal, so the next one loads the
//! case and judges the move (2)"), and item 2 ends a run whose case the governor holds complete as
//! completed, as rule 1 of `outcome::derive` does "whatever the executor returned".

use std::collections::BTreeMap;
use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomeRefused, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeProposedReads, FrontierAction, ObservationId,
    PrincipalId, ProposedActionArguments, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RunOutcomeCompleted, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{
    AdmittedRequest, EffectError, EffectPort, invoke_concurrently,
};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const NOW: &str = "2026-10-20T12:00:00Z";
const GUARD: &str = "the step guard does not admit this read";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary".to_owned()),
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

fn proposed(name: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Text(format!("{name} arguments"))),
    }
}

fn reads(names: &[&str]) -> ExecutorOutcome {
    ExecutorOutcome::ProposedReads(ExecutorOutcomeProposedReads {
        reads: names.iter().map(|name| proposed(name)).collect(),
    })
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

#[derive(Debug)]
struct Context {
    requests: u64,
    observations: u64,
}

impl Context {
    fn new() -> Self {
        Self {
            requests: 0x300,
            observations: 0x400,
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
        None
    }
}

/// A port that performs every action, reads the actions it was told to, answers each action with
/// the answer it was given for it (`Refused`, ordinary, for any other), and records what it was
/// handed.
#[derive(Default)]
struct Effects {
    reads: Vec<&'static str>,
    answers: BTreeMap<&'static str, Result<EffectOutcome, EffectError>>,
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn answering(action: &'static str, answer: EffectOutcome) -> Self {
        Self {
            answers: BTreeMap::from([(action, Ok(answer))]),
            ..Self::default()
        }
    }

    fn reading(self, reads: Vec<&'static str>) -> Self {
        Self { reads, ..self }
    }

    fn invoked(&self) -> Vec<String> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|data| data.action.clone())
            .collect()
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
            .unwrap_or_else(|| Ok(refused("not this one", false)));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        answer
    }
}

fn store() -> Generated<RunStore> {
    let mut issued = 0;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }))
}

fn run(
    governor: &FakeGovernor,
    executor: &ScriptedExecutor,
    effects: &Effects,
    case: &CaseId,
) -> LoopEnd {
    run_until_blocked(
        governor,
        executor,
        &StaticAuthorityProvider::new(),
        effects,
        &commission(case),
        &mut store(),
        &mut Context::new(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// A fan-out of two reads: `search` is revalidated and admitted at revision 4, then somebody else
/// moves the case to revision 5, where the governor holds it complete, so `fetch` is recorded stale
/// and never invoked. The port refuses `search` as final.
///
/// The stale read tells the runtime the case moved; the executor contract and item 8a say the move
/// is then judged as after a stale proposal, and a case the governor holds complete ends the run
/// completed (item 2, `derive` rule 1). On 0.27.0 an ordinary refusal of `search` ends this run
/// `Completed`. The unit ends it `EffectRefused` and never loads the case it knows moved.
#[test]
fn adversary_a_stale_read_beside_a_final_refusal_is_judged_as_the_move() {
    let case = CaseId("case-adv-stale-beside-final".to_owned());
    let governor = FakeGovernor::new();
    let open = listing(4, vec![admissible("search"), admissible("fetch")]);
    // The start, the completion, the frontier and `search`'s revalidation (revision and frontier)
    // answer at 4; `fetch`'s revalidation finds the case at 5, complete, and so does every call after.
    governor.script(
        case.clone(),
        repeat_n(open, 5).chain([listing(5, Vec::new()).complete("closed")]),
    );
    let executor = ScriptedExecutor::new([reads(&["search", "fetch"]), idle(), idle()]);
    let effects =
        Effects::answering("search", refused(GUARD, true)).reading(vec!["search", "fetch"]);

    let end = run(&governor, &executor, &effects, &case);

    assert_eq!(
        end.requests[0].outcome,
        RevalidateActionRequestOutcome::Admitted,
        "precondition: `search` is admitted"
    );
    assert!(
        matches!(
            end.requests[1].outcome,
            RevalidateActionRequestOutcome::Stale { .. }
        ),
        "precondition: `fetch` is stale: {:?}",
        end.requests[1].outcome
    );
    assert_eq!(
        effects.invoked(),
        ["search"],
        "precondition: only `search` is handed to the port"
    );
    assert_eq!(executor.calls().len(), 1, "the executor is not run again");
    assert_eq!(
        end.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "closed".to_owned(),
        }),
        "a stale read shows the case moved, and the move is judged as after a stale proposal \
         (docs/commission/contracts/commission-executor.md; runtime items 8a and 2): the governor \
         holds the case complete"
    );
}
