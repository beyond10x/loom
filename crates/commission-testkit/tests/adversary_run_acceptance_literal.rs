//! Adversary pass 1 on `story:run-outcomes`, acceptance row 1 read literally.
//!
//! The story's acceptance says: "`CompletedLocalReasoning` on a case the governor reports open
//! derives continue, not completed." It names no frontier. The acceptance test's row for it uses
//! a frontier with an admissible action, where rule 6 of `derive` (the frontier decides) also
//! answers continue, so that row cannot tell "the executor's local completion continues" from "the
//! frontier continues". On a frontier that admits nothing, `derive` ends the run instead.
//!
//! Rows 2 and 4 (Suspended, NeedsHumanJudgment) hold whatever the frontier is; row 1's second half
//! is the one executor-stated row that does not. Either the derivation or the acceptance wording
//! has to change. The coordinator amended row 1 (wave 2026-10-04-w5): on a frontier that admits
//! nothing the frontier rule decides, and this case asserts that.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, FrontierObligation, PrincipalId, RunOutcome, RunOutcomeNeedsExternalEvidence,
    Unit,
};
use b10x_commission::outcome::{Derived, derive};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

#[test]
fn adversary_run_local_reasoning_on_an_empty_frontier_falls_to_the_frontier_rule() {
    let case = CaseId("case-literal".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(4).with_items(
            Vec::new(),
            vec![FrontierObligation {
                obligation: "tests pass".to_owned(),
                open: true,
            }],
            Vec::new(),
        )],
    );
    let frontier = governor.frontier(&case).expect("frontier");
    let determination = governor.completion(&case).expect("completion");
    let executor = ScriptedExecutor::new([ExecutorOutcome::CompletedLocalReasoning(Unit(true))]);
    let outcome = executor.run(&commission, &frontier);
    assert_eq!(
        derive(&determination, &frontier, &outcome, None),
        Derived::Ended(RunOutcome::NeedsExternalEvidence(
            RunOutcomeNeedsExternalEvidence {
                requirements: vec!["tests pass".to_owned()],
            }
        )),
        "acceptance row 1 as amended: CompletedLocalReasoning on an open case continues only when \
         the frontier admits an action; this frontier admits none and has the open obligation \
         `tests pass`, so the frontier rule ends the run needing that evidence, not completed and \
         not continue"
    );
}
