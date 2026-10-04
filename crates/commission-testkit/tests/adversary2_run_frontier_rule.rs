//! Adversary pass 2 on `story:run-outcomes`: rule 6 of `derive` counts an action as admitted only
//! when admission admits it.
//!
//! A frontier may list one action more than once, and admission then takes the least-authority
//! entry (`crates/commission/src/admission.rs`). An action listed both `Admissible` and `Blocked`
//! is refused, and one listed `Admissible` and `ApprovalRequired` needs authority: neither is
//! admitted, so with nothing else on the frontier the obligations decide. A rule 6 that reads an
//! entry's status instead of asking admission continues the run on an action the executor may not
//! take — the loop the amended acceptance row 1 exists to prevent. No existing case lists one
//! action twice, so that mutant passes the suite.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, FrontierAction, FrontierObligation, PrincipalId, RunOutcome,
    RunOutcomeNeedsExternalEvidence, Unit,
};
use b10x_commission::outcome::{Derived, derive};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn entry(status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: "deploy".to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec!["release frozen".to_owned()],
    }
}

fn derived(actions: Vec<FrontierAction>, executor: ExecutorOutcome) -> Derived {
    let case = CaseId("case-adversary2-frontier-rule".to_owned());
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
            actions,
        )],
    );
    let frontier = governor.frontier(&case).expect("frontier");
    let determination = governor.completion(&case).expect("completion");
    let outcome = ScriptedExecutor::new([executor]).run(&commission, &frontier);
    derive(&determination, &frontier, &outcome, None)
}

#[test]
fn adversary2_run_an_action_listed_admissible_and_refused_is_not_admitted() {
    let evidence = Derived::Ended(RunOutcome::NeedsExternalEvidence(
        RunOutcomeNeedsExternalEvidence {
            requirements: vec!["tests pass".to_owned()],
        },
    ));
    for executor in [
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
    ] {
        assert_eq!(
            derived(
                vec![
                    entry(ActionStatus::Admissible, None),
                    entry(ActionStatus::Blocked, None),
                ],
                executor.clone(),
            ),
            evidence,
            "{executor:?}: `deploy` is listed Admissible and Blocked, so admission refuses it and \
             the frontier admits nothing"
        );
        assert_eq!(
            derived(
                vec![
                    entry(ActionStatus::Admissible, None),
                    entry(ActionStatus::ApprovalRequired, Some("prod.deploy")),
                ],
                executor.clone(),
            ),
            evidence,
            "{executor:?}: `deploy` is listed Admissible and ApprovalRequired, so it needs \
             authority and is not admitted without it"
        );
    }
}
