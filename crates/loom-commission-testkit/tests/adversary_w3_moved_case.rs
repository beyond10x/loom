//! Adversary pass 1, wave 2026-10-07-w3, `story:moved-case-outcome`: an `ExecutorOutcome::CaseMoved`
//! the governor does not bear out.
//!
//! What the unit wrote about the variant:
//!
//! - `ess/commission/domains/responsibility.yaml`, `ExecutorOutcomeCaseMoved`: "The runtime reads
//!   the current revision and frontier from the governor, never from this outcome, and judges the
//!   run on them."
//! - `docs/commission/contracts/commission-executor.md`: "The runtime then loads the case again and
//!   judges the run on the frontier current then; it never takes the revision from the executor."
//! - `crates/loom-commission/src/runtime.rs`, module docs item 9: "where it would let the run go on,
//!   the run ends with no admissible action: the Run holds the case at the revision it left. The
//!   reported revision is observed (6), never trusted."
//!
//! Below, the governor holds the case at the Run's own revision throughout, and its frontier at
//! that revision admits an action the effect port performs. The executor reports `CaseMoved`
//! anyway, once naming that same revision and once a later one. Judged on the governor's revision
//! and frontier, as the three texts say, nothing moved and the frontier admits an action: the run
//! goes on (items 1 to 3 and 6), and the executor is run again. The second executor call answers
//! `NeedsHumanJudgment`, so a run that went on ends there.

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeCaseMoved, ExecutorOutcomeNeedsHumanJudgment, FrontierAction,
    FrontierObligation, HumanDecisionRequest, ObservationId, PrincipalId, RunId, RunOutcome,
    RunOutcomeNeedsHumanJudgment, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const CASE: &str = "CASE-ADV-W3";
/// The revision the governor holds the case at, before, during and after the executor's call.
const HELD: i64 = 7;
/// The only action listed: admissible, needing no authority, and performed by the port.
const TEST: &str = "tests.run";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case(),
        principal: PrincipalId("principal-adv".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case open at [`HELD`] on every call: `tests-pass` open and `tests.run` admissible.
fn unmoved() -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [Answer::at(HELD).with_items(
            Vec::new(),
            vec![FrontierObligation {
                obligation: "tests-pass".to_owned(),
                open: true,
            }],
            vec![FrontierAction {
                action: TEST.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )],
    );
    governor
}

fn human_request() -> HumanDecisionRequest {
    HumanDecisionRequest(Value::Object(vec![(
        "question".to_owned(),
        Value::Text("which suite?".to_owned()),
    )]))
}

/// Performs `tests.run`; never invoked here, since nothing is proposed.
struct Effects;

impl EffectPort for Effects {
    fn performs(&self, action: &str) -> bool {
        action == TEST
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
        }))
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-07T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The executor reports `CaseMoved` naming `reported` while the governor holds the case at
/// [`HELD`], whose frontier admits `tests.run`. Judged on the governor's revision and frontier, the
/// Run's case has not moved and the run goes on: the executor is run a second time and its
/// `NeedsHumanJudgment` ends the run. Measured: the run ends `NoAdmissibleAction` after one
/// executor call, on a frontier at the Run's own revision that admits `tests.run`.
#[test]
fn adversary_w3_a_move_the_governor_does_not_hold_does_not_end_the_run() {
    let mut failures = Vec::new();
    for reported in [HELD, HELD + 2] {
        let governor = unmoved();
        let executor = ScriptedExecutor::new([
            ExecutorOutcome::CaseMoved(ExecutorOutcomeCaseMoved {
                expected_case_revision: reported,
            }),
            ExecutorOutcome::NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment {
                request: human_request(),
            }),
        ]);
        let mut runs = Generated::new(RunStore::new(|| RunId(uuid(0x500))));
        let end = run_until_blocked(
            &governor,
            &executor,
            &StaticAuthorityProvider::new(),
            &Effects,
            &commission(),
            &mut runs,
            &mut Context::default(),
        )
        .unwrap_or_else(|error| panic!("the loop failed: {error}"));

        let expected = RunOutcome::NeedsHumanJudgment(RunOutcomeNeedsHumanJudgment {
            request: human_request(),
        });
        let calls = executor.calls().len();
        if end.outcome != expected || calls != 2 {
            failures.push(format!(
                "CaseMoved naming revision {reported}, the governor holding the case at {HELD} \
                 throughout with `{TEST}` admissible: the run ended {:?} after {calls} executor \
                 call(s), expected {expected:?} after 2. The executor's claim of a move was \
                 trusted over the governor's revision (runtime.rs item 9; the ESS comment says the \
                 runtime judges on the governor's revision and frontier, never on this outcome)",
                end.outcome
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
