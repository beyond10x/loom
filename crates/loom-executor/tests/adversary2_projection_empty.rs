//! Adversary pass 2 on `story:frontier-projection`: a frontier that projects to nothing.
//!
//! An empty frontier and a frontier whose every action Commission refuses both project to a
//! catalogue with no entries. That catalogue still names its frontier and revision, it is storable
//! through the generated `ProjectCatalogue` (which declares no refusal for an empty list), no
//! `SelectAction` can name an action in it, and Loom's executor proposes nothing on the same
//! frontier, whatever its selector names: the model is shown nothing and the executor runs nothing.

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::behaviour::{ActionCatalogueStorage, Generated};
use b10x_loom_executor::model::primitives::Uuid as LoomUuid;
use b10x_loom_executor::model::run::obligations::{CataloguesQuery, ProjectCatalogueBehavior};
use b10x_loom_executor::model::run::{
    ActionCatalogueSnapshot, ActionCatalogueState, CatalogueEntry, CatalogueId, CatalogueProjected,
    ProjectCatalogue, ProjectCatalogueOutcome, SelectionStrategy, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

const CASE: &str = "CHG-1842";
const FRONTIER_ID: &str = "00000000-0000-4000-8000-0000000000f3";

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec![format!("{action} {status:?}")],
    }
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid(FRONTIER_ID.to_owned())),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 11,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn all_refused() -> Vec<FrontierAction> {
    vec![
        listed("repository.merge", ActionStatus::Blocked, None),
        listed("repository.push", ActionStatus::ApprovalRequired, None),
        listed("repository.tag", ActionStatus::ApprovalRequired, Some("  ")),
        listed(
            "repository.delete",
            ActionStatus::ApprovalRequired,
            Some("a"),
        ),
        listed(
            "repository.delete",
            ActionStatus::ApprovalRequired,
            Some("b"),
        ),
        listed("repository.edit", ActionStatus::Admissible, None),
        listed("repository.edit", ActionStatus::Blocked, None),
    ]
}

fn catalogue_id() -> CatalogueId {
    CatalogueId(LoomUuid("00000000-0000-4000-8000-0000000000c3".to_owned()))
}

fn turn_id() -> TurnId {
    TurnId(LoomUuid("00000000-0000-4000-8000-0000000000a3".to_owned()))
}

/// In-memory catalogue storage: all the generated `ProjectCatalogue` needs.
#[derive(Default)]
struct Catalogues(Vec<ActionCatalogueSnapshot>);

impl ActionCatalogueStorage for Catalogues {
    fn get(&self, identity: &CatalogueId) -> Option<ActionCatalogueSnapshot> {
        self.0
            .iter()
            .find(|held| &held.data.catalogue_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: ActionCatalogueSnapshot) {
        let id = snapshot.data.catalogue_id.clone();
        ActionCatalogueStorage::delete(self, &id);
        self.0.push(snapshot);
    }

    fn delete(&mut self, identity: &CatalogueId) {
        self.0.retain(|held| &held.data.catalogue_id != identity);
    }

    fn list(&self) -> Vec<ActionCatalogueSnapshot> {
        self.0.clone()
    }
}

/// A selector that names one action, whatever it is handed.
///
/// `story:action-selector`: a selector is handed the catalogue projected from the frontier, not the
/// frontier; here that catalogue is empty, and every name is refused as absent from it.
struct Names(&'static str);

impl ActionSelector for Names {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Ok(Choice {
            action: self.0.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
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

#[test]
fn adversary2_projection_empty_frontier_is_an_empty_catalogue_of_that_frontier() {
    let data = project(&frontier(Vec::new()), catalogue_id(), turn_id()).into_data();
    assert_eq!(data.entries, Vec::new());
    assert_eq!(data.frontier, FRONTIER_ID);
    assert_eq!(data.case_revision, 11);
    assert_eq!(data.turn_id, turn_id());
}

#[test]
fn adversary2_projection_all_refused_frontier_is_an_empty_catalogue() {
    let data = project(&frontier(all_refused()), catalogue_id(), turn_id()).into_data();
    assert_eq!(data.entries, Vec::new(), "{data:?}");
    assert_eq!(data.case_revision, 11);
}

/// The empty catalogue goes through the generated `ProjectCatalogue` as a projected catalogue, and
/// a second projection of the same id is still refused.
#[test]
fn adversary2_projection_empty_catalogue_is_storable() {
    let data = project(&frontier(all_refused()), catalogue_id(), turn_id()).into_data();
    let command = ProjectCatalogue {
        catalogue_id: data.catalogue_id.clone(),
        turn_id: data.turn_id.clone(),
        frontier: data.frontier.clone(),
        case_revision: data.case_revision,
        entries: data.entries.clone(),
    };
    let mut model = Generated::new(Catalogues::default());
    assert_eq!(
        model.project_catalogue(command.clone()),
        Ok(ProjectCatalogueOutcome::Projected {
            catalogue_projected: CatalogueProjected {
                catalogue_id: catalogue_id(),
                turn_id: turn_id(),
                case_revision: 11,
            }
        })
    );
    let rows = model.catalogues().expect("Catalogues is generated");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, ActionCatalogueState::Projected);
    assert_eq!(model.ports.0[0].data.entries, Vec::new());
    assert!(matches!(
        model.project_catalogue(command),
        Ok(ProjectCatalogueOutcome::CatalogueExists { .. })
    ));
}

/// On a frontier that projects to nothing, Loom's executor proposes nothing either, whichever
/// listed action, or unlisted one, its selector names.
#[test]
fn adversary2_projection_empty_catalogue_agrees_with_executor() {
    for actions in [Vec::new(), all_refused()] {
        let frontier = frontier(actions);
        assert!(
            project(&frontier, catalogue_id(), turn_id())
                .into_data()
                .entries
                .is_empty()
        );
        for named in [
            "repository.merge",
            "repository.push",
            "repository.tag",
            "repository.delete",
            "repository.edit",
            "unlisted.action",
        ] {
            let outcome =
                Loom::new(Names(named), EmptyObjectArguments, "go").run(&commission(), &frontier);
            assert_eq!(
                outcome,
                ExecutorOutcome::NoUsefulAction(Unit(true)),
                "selector named {named} on {:?}",
                frontier.data().actions
            );
        }
    }
}
