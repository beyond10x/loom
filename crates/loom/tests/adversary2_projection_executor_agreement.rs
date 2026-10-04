//! Adversary pass 2 on `story:frontier-projection`: the catalogue and Loom's executor agree.
//!
//! The model is shown the catalogue; Loom's executor proposes what its selector names only if
//! Commission does not refuse it. The two must agree on every action: an action the catalogue
//! lists is one the executor proposes when it is selected, and an action the catalogue leaves out
//! is one the executor never proposes. Checked over every frontier of up to three entries for one
//! action, each one of five kinds, beside an admissible `inspect`, and for an unlisted action.
//! Exhaustive, so deterministic. The oracle is the executor's outcome, not `admit`.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::model::primitives::Uuid as LoomUuid;
use b10x_loom::model::run::{CatalogueId, TurnId};
use b10x_loom::projection::project;
use b10x_loom::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

const CASE: &str = "CHG-1842";
const MERGE: &str = "repository.merge";
const INSPECT: &str = "repository.inspect";
const UNLISTED: &str = "repository.unlisted";

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec![format!("{action} {status:?} {capability:?}")],
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-0000000000f4".to_owned())),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 5,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

struct Names(String);

impl ActionSelector for Names {
    fn select(
        &self,
        _frontier: &Frontier<frontier_state::Issued>,
        _prompt: &str,
    ) -> Result<String, SelectorError> {
        Ok(self.0.clone())
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn proposes(frontier: &Frontier<frontier_state::Issued>, action: &str) -> bool {
    match Loom::new(Names(action.to_owned()), EmptyObjectArguments, "go")
        .run(&commission(), frontier)
    {
        ExecutorOutcome::ProposedAction(proposal) => {
            assert_eq!(
                proposal.action, action,
                "the executor proposed another action"
            );
            true
        }
        _ => false,
    }
}

#[test]
fn adversary2_projection_catalogue_is_what_the_executor_proposes() {
    let kinds: [(ActionStatus, Option<&str>); 5] = [
        (ActionStatus::Admissible, None),
        (ActionStatus::Blocked, None),
        (ActionStatus::ApprovalRequired, Some("cap.a")),
        (ActionStatus::ApprovalRequired, Some("cap.b")),
        (ActionStatus::ApprovalRequired, None),
    ];
    let mut picks: Vec<Vec<usize>> = vec![Vec::new()];
    for a in 0..kinds.len() {
        picks.push(vec![a]);
        for b in 0..kinds.len() {
            picks.push(vec![a, b]);
            for c in 0..kinds.len() {
                picks.push(vec![a, b, c]);
            }
        }
    }

    let mut wrong = Vec::new();
    let mut shown_merge = 0;
    for pick in &picks {
        // Merge's entries are spread around inspect so the first position varies too.
        let mut actions = Vec::new();
        for (index, &kind) in pick.iter().enumerate() {
            if index == 1 {
                actions.push(listed(INSPECT, ActionStatus::Admissible, None));
            }
            actions.push(listed(MERGE, kinds[kind].0, kinds[kind].1));
        }
        if pick.len() < 2 {
            actions.push(listed(INSPECT, ActionStatus::Admissible, None));
        }
        let frontier = frontier(actions);
        let catalogue = project(
            &frontier,
            CatalogueId(LoomUuid("00000000-0000-4000-8000-0000000000c4".to_owned())),
            TurnId(LoomUuid("00000000-0000-4000-8000-0000000000a4".to_owned())),
        )
        .into_data();
        for action in [MERGE, INSPECT, UNLISTED] {
            let shown = catalogue
                .entries
                .iter()
                .filter(|entry| entry.action == action)
                .count();
            let runs = proposes(&frontier, action);
            if shown > 1 || (shown == 1) != runs {
                wrong.push(format!(
                    "{action} on {:?}: shown {shown} time(s), executor proposes it: {runs}",
                    pick.iter().map(|&kind| kinds[kind]).collect::<Vec<_>>()
                ));
            }
            if action == MERGE && shown == 1 {
                shown_merge += 1;
            }
        }
    }
    assert!(
        shown_merge > 0 && shown_merge < picks.len(),
        "the frontiers must show merge on some and hide it on others: shown on {shown_merge} of {}",
        picks.len()
    );
    assert!(
        wrong.is_empty(),
        "{} disagreements between the catalogue and the executor; first five:\n{}",
        wrong.len(),
        wrong.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}
