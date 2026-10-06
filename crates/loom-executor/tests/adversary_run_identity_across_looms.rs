// SPDX-License-Identifier: Apache-2.0

//! The case `story:argument-generator` carried to `story:interruption-recovery` (wave
//! 2026-10-04-w15): selection and argument-request ids were derived from the frontier id and a run
//! counter each `Loom` starts at 0, so a second `Loom` (one built to recover a run, or one in
//! another process) gave its first selection on a frontier the id an earlier `Loom` had already
//! given to a different selection on that frontier. Recovery must not reuse an id an earlier `Loom`
//! gave.

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::run::CatalogueEntry;
use b10x_loom_executor::model::run::SelectionStrategy;
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

/// Selects the entry at `at` of the catalogue it is handed.
struct Nth(usize);

impl ActionSelector for Nth {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .get(self.0)
            .map(|entry| Choice {
                action: entry.action.clone(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
        case_id: CaseId("CASE-1".into()),
        principal: PrincipalId("principal-a".into()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    let admissible = |action: &str| FrontierAction {
        action: action.into(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    };
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-000000000003".into())),
        case_id: CaseId("CASE-1".into()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![admissible("repository.inspect"), admissible("tests.run")],
    })
}

/// Two Looms built as a host builds them, each running once on one frontier and selecting a
/// different action: two different selections, so two different selection ids and two different
/// argument-request ids.
#[test]
fn two_looms_give_their_different_selections_on_one_frontier_different_ids() {
    let earlier = Loom::new(Nth(0), EmptyObjectArguments, "investigate");
    let recovered = Loom::new(Nth(1), EmptyObjectArguments, "investigate");
    for loom in [&earlier, &recovered] {
        let outcome = loom.run(&commission(), &frontier());
        assert!(
            matches!(outcome, ExecutorOutcome::ProposedAction(_)),
            "{outcome:?}"
        );
    }

    let (first, second) = (earlier.selections(), recovered.selections());
    assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
    assert_ne!(
        first[0].data.action, second[0].data.action,
        "precondition: the two selections differ"
    );
    assert_ne!(
        first[0].data.selection_id, second[0].data.selection_id,
        "two Looms gave their different selections one id"
    );
    let (first, second) = (earlier.argument_requests(), recovered.argument_requests());
    assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
    assert_ne!(
        first[0].data.argument_request_id, second[0].data.argument_request_id,
        "two Looms gave the argument requests of their different selections one id"
    );
}
