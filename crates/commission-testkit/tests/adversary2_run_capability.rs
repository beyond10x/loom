//! Adversary pass 2 on `story:run-outcomes`: a verdict binds to its capability byte for byte.
//!
//! `authority_verdict_binds_to_its_capability` uses two capabilities that share nothing
//! (`prod.deploy`, `logs.read`), so a comparison that folds case or trims whitespace passes it. An
//! authority verdict is a grant; a grant for `PROD.DEPLOY` or `prod.deploy ` is not a grant for
//! `prod.deploy`, and the frontier names its capability exactly (admission keeps a padded name as
//! it is). Each case below is a capability the frontier did not name.

use b10x_commission::model::json::Value;
use b10x_commission::model::responsibility::{
    ActionStatus, AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ProposedActionArguments, RunOutcome,
    RunOutcomeNeedsAuthority, Unit,
};
use b10x_commission::outcome::{CapabilityVerdict, Derived, derive};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};

fn derive_for(frontier_capability: &str, asked: &str, verdict: &AuthorityVerdict) -> Derived {
    let case = CaseId("case-adversary2-capability".to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Answer::at(4).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: "deploy".to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(frontier_capability.to_owned()),
                reasons: Vec::new(),
            }],
        )],
    );
    let frontier = governor.frontier(&case).expect("frontier");
    let determination = governor.completion(&case).expect("completion");
    let outcome = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: "deploy".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    });
    derive(
        &determination,
        &frontier,
        &outcome,
        Some(CapabilityVerdict {
            capability: asked,
            verdict,
        }),
    )
}

#[test]
fn adversary2_run_a_verdict_for_a_case_or_whitespace_variant_is_no_verdict() {
    let allow = AuthorityVerdict::Allow(Unit(true));
    let approval = AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: "Q".to_owned(),
    });
    let nothing = Derived::Ended(RunOutcome::NoAdmissibleAction(Unit(true)));

    for asked in [
        "PROD.DEPLOY",
        "Prod.Deploy",
        "prod.deploy ",
        " prod.deploy",
        "prod.deploy\n",
    ] {
        assert_eq!(
            derive_for("prod.deploy", asked, &allow),
            nothing,
            "an allow for {asked:?} let an action needing \"prod.deploy\" continue"
        );
        assert_eq!(
            derive_for("prod.deploy", asked, &approval),
            nothing,
            "approval required for {asked:?} was reported for \"prod.deploy\""
        );
    }

    // The frontier's own spelling, padded, is the capability: a verdict for exactly it counts, and
    // one for the trimmed name does not.
    assert_eq!(
        derive_for(" prod.deploy", " prod.deploy", &allow),
        Derived::Continue,
        "an allow for the frontier's exact capability"
    );
    assert_eq!(
        derive_for(" prod.deploy", " prod.deploy", &approval),
        Derived::Ended(RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: "Q".to_owned(),
        })),
        "approval required for the frontier's exact capability"
    );
    assert_eq!(
        derive_for(" prod.deploy", "prod.deploy", &allow),
        nothing,
        "an allow for the trimmed name let the padded capability continue"
    );
}
