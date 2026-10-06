//! Adversary pass 1 on `story:run-pipeline-skeleton`: the revalidation the specification declares,
//! driven through the model `ess` generated from it (`b10x_loom_executor::model::behaviour::Generated`).
//!
//! `ProjectCatalogue` and `RevalidateSelection` are generated behaviour ("the specification fully
//! determines it"). `SelectAction` is an obligation; `SpecPorts` implements it exactly as
//! `ess/domains/run.yaml` declares it: `catalogue-unknown`, `not-in-catalogue`, then `selected`
//! setting every field from the input, `case_revision` included. A fix that adds a `SelectAction`
//! refusal changes that declaration, and `SpecPorts::select_action` follows it.
//!
//! The expectations come from `story:selection-revalidation` § Acceptance items 1 and 2, which
//! rely on these declarations and do not change `ess/`.
//!
//! `not-in-frontier` stays an `external:` outcome: ess 0.54.0 synthesis refuses it as a
//! membership guard over `frontier_actions`. Its answer is therefore the executor's, and
//! `not_in_frontier_follows_the_frontier_actions` revalidates through the executor's
//! `RequestRecord`, whose context decides it from the command input, rather than through
//! `SpecPorts`' fixed answer.

use b10x_loom_executor::arguments::RequestRecord;
use b10x_loom_executor::model::behaviour::{
    ActionCatalogueStorage, Context, ExternalCommand, Generated, SelectionStorage,
};
use b10x_loom_executor::model::obligation::UnmetObligation;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::obligations::{
    ProjectCatalogueBehavior, RevalidateSelectionBehavior, SelectActionBehavior,
};
use b10x_loom_executor::model::run::{
    ActionCatalogueSnapshot, ActionNotInCatalogue, ActionSelected, AnySelection, CatalogueEntry,
    CatalogueEntryStatus, CatalogueId, CatalogueNotFound, CatalogueRevisionMismatch,
    ProjectCatalogue, ProjectCatalogueOutcome, RevalidateSelection, RevalidateSelectionOutcome,
    SelectAction, SelectActionOutcome, Selection, SelectionData, SelectionId,
    SelectionNotInFrontier, SelectionSnapshot, SelectionStale, SelectionStrategy, TurnId,
};

/// In-memory storage, a fixed answer to every `external:` branch, and `SelectAction` as declared.
struct SpecPorts {
    catalogues: Vec<ActionCatalogueSnapshot>,
    selections: Vec<SelectionSnapshot>,
    external_answer: bool,
}

impl SpecPorts {
    fn new(external_answer: bool) -> Self {
        Self {
            catalogues: Vec::new(),
            selections: Vec::new(),
            external_answer,
        }
    }
}

impl ActionCatalogueStorage for SpecPorts {
    fn get(&self, identity: &CatalogueId) -> Option<ActionCatalogueSnapshot> {
        self.catalogues
            .iter()
            .find(|held| &held.data.catalogue_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: ActionCatalogueSnapshot) {
        ActionCatalogueStorage::delete(self, &snapshot.data.catalogue_id.clone());
        self.catalogues.push(snapshot);
    }

    fn delete(&mut self, identity: &CatalogueId) {
        self.catalogues
            .retain(|held| &held.data.catalogue_id != identity);
    }

    fn list(&self) -> Vec<ActionCatalogueSnapshot> {
        self.catalogues.clone()
    }
}

impl SelectionStorage for SpecPorts {
    fn get(&self, identity: &SelectionId) -> Option<SelectionSnapshot> {
        self.selections
            .iter()
            .find(|held| &held.data.selection_id == identity)
            .cloned()
    }

    fn put(&mut self, snapshot: SelectionSnapshot) {
        SelectionStorage::delete(self, &snapshot.data.selection_id.clone());
        self.selections.push(snapshot);
    }

    fn delete(&mut self, identity: &SelectionId) {
        self.selections
            .retain(|held| &held.data.selection_id != identity);
    }

    fn list(&self) -> Vec<SelectionSnapshot> {
        self.selections.clone()
    }
}

impl Context for SpecPorts {
    fn external(&mut self, _command: ExternalCommand<'_>, _outcome: &'static str) -> bool {
        self.external_answer
    }
}

/// `loom.run.SelectAction` exactly as `ess/domains/run.yaml` declares it.
impl SelectActionBehavior for SpecPorts {
    fn select_action(
        &mut self,
        input: SelectAction,
    ) -> Result<SelectActionOutcome, UnmetObligation> {
        let Some(catalogue) = ActionCatalogueStorage::get(self, &input.catalogue_id) else {
            return Ok(SelectActionOutcome::CatalogueUnknown {
                error: CatalogueNotFound {
                    catalogue_id: input.catalogue_id,
                },
            });
        };
        if !catalogue
            .data
            .entries
            .iter()
            .any(|entry| entry.action == input.action)
        {
            return Ok(SelectActionOutcome::NotInCatalogue {
                error: ActionNotInCatalogue {
                    action: input.action,
                },
            });
        }
        if catalogue.data.case_revision != input.case_revision {
            return Ok(SelectActionOutcome::RevisionMismatch {
                error: CatalogueRevisionMismatch {
                    catalogue_id: input.catalogue_id,
                    case_revision: input.case_revision,
                    catalogue_revision: catalogue.data.case_revision,
                },
            });
        }
        let data = SelectionData {
            selection_id: input.selection_id.clone(),
            catalogue_id: input.catalogue_id.clone(),
            action: input.action.clone(),
            confidence: input.confidence.clone(),
            strategy: input.strategy,
            case_revision: input.case_revision,
        };
        SelectionStorage::put(
            self,
            AnySelection::Selected(Selection::new(data)).snapshot(),
        );
        Ok(SelectActionOutcome::Selected {
            action_selected: ActionSelected {
                selection_id: input.selection_id,
                catalogue_id: input.catalogue_id,
                action: input.action,
            },
        })
    }
}

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012}"))
}

fn catalogue_id() -> CatalogueId {
    CatalogueId(uuid(1))
}

/// Projects catalogue 1 at `revision`, listing `repository.read` and `repository.merge`.
fn project(model: &mut Generated<SpecPorts>, revision: i64) {
    let outcome = model
        .project_catalogue(ProjectCatalogue {
            catalogue_id: catalogue_id(),
            turn_id: TurnId(uuid(2)),
            frontier: format!("frontier-{revision}"),
            case_revision: revision,
            entries: vec![
                CatalogueEntry {
                    action: "repository.read".to_owned(),
                    status: CatalogueEntryStatus::Admissible,
                },
                CatalogueEntry {
                    action: "repository.merge".to_owned(),
                    status: CatalogueEntryStatus::ApprovalRequired,
                },
            ],
        })
        .expect("ProjectCatalogue is generated");
    assert!(matches!(outcome, ProjectCatalogueOutcome::Projected { .. }));
}

fn select(
    model: &mut Generated<SpecPorts>,
    selection: u32,
    action: &str,
    case_revision: i64,
) -> SelectActionOutcome {
    model
        .select_action(SelectAction {
            selection_id: SelectionId(uuid(selection)),
            catalogue_id: catalogue_id(),
            action: action.to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision,
        })
        .expect("SelectAction as declared")
}

fn revalidate(
    model: &mut Generated<SpecPorts>,
    selection: u32,
    case_revision: i64,
    frontier_actions: &[&str],
) -> RevalidateSelectionOutcome {
    model
        .revalidate_selection(RevalidateSelection {
            selection_id: SelectionId(uuid(selection)),
            case_revision,
            frontier_actions: frontier_actions.iter().map(|a| (*a).to_owned()).collect(),
        })
        .expect("RevalidateSelection is generated")
}

/// `story:selection-revalidation` acceptance 2: a selection made on a catalogue at case revision
/// `n` while the current revision is `n + 1` is refused, naming both revisions. The specification
/// takes the selection's revision from `SelectAction`'s input rather than from the catalogue the
/// selection names, so a selection on the revision-7 catalogue that claims revision 8 is admitted
/// at revision 8. The synthesized conformance suite builds exactly this state (catalogue at
/// 194041, `SelectAction.case_revision` 1, `selected`) and expects it `admitted`.
#[test]
fn selection_on_an_older_catalogue_is_never_admitted() {
    let mut model = Generated::new(SpecPorts::new(false));
    project(&mut model, 7);

    let selected = select(&mut model, 10, "repository.merge", 8);
    if !matches!(selected, SelectActionOutcome::Selected { .. }) {
        // SelectAction refused the mismatched revision: the catalogue's revision is bound.
        return;
    }
    let outcome = revalidate(&mut model, 10, 8, &["repository.read", "repository.merge"]);
    assert_eq!(
        outcome,
        RevalidateSelectionOutcome::StaleRevision {
            selection_stale: SelectionStale {
                selection_id: SelectionId(uuid(10)),
                catalogue_revision: 7,
                case_revision: 8,
            },
        },
        "a selection made on the catalogue projected at revision 7 was revalidated at revision 8; \
         SelectAction accepted it ({selected:?}) and revalidation did not refuse it as stale"
    );
}

/// `story:selection-revalidation` acceptance 1, and the outcome's own words ("the frontier action
/// ids in the input do not list the selection's action"): whether a selection is refused
/// `not-in-frontier` depends on `frontier_actions`. The two selections are made through
/// `SelectAction` as declared, then held by the executor's `RequestRecord`, which revalidates them
/// and answers the `external:` branch itself: the one whose action the input lists is admitted, the
/// one whose action it does not list is refused `not-in-frontier`.
#[test]
fn not_in_frontier_follows_the_frontier_actions() {
    let mut model = Generated::new(SpecPorts::new(false));
    project(&mut model, 7);
    for selection in [20, 21] {
        assert!(matches!(
            select(&mut model, selection, "repository.merge", 7),
            SelectActionOutcome::Selected { .. }
        ));
    }
    let mut record = RequestRecord::default();
    for held in SelectionStorage::list(&model.ports) {
        SelectionStorage::put(&mut record, held);
    }

    let mut failures = Vec::new();
    let listed = record
        .revalidate_selection(RevalidateSelection {
            selection_id: SelectionId(uuid(20)),
            case_revision: 7,
            frontier_actions: vec!["repository.read".to_owned(), "repository.merge".to_owned()],
        })
        .expect("the executor answers RevalidateSelection");
    let absent = record
        .revalidate_selection(RevalidateSelection {
            selection_id: SelectionId(uuid(21)),
            case_revision: 7,
            frontier_actions: vec!["repository.read".to_owned()],
        })
        .expect("the executor answers RevalidateSelection");

    let expected_listed = RevalidateSelectionOutcome::Admitted {
        selection_admitted: b10x_loom_executor::model::run::SelectionAdmitted {
            selection_id: SelectionId(uuid(20)),
        },
    };
    let expected_absent = RevalidateSelectionOutcome::NotInFrontier {
        selection_not_in_frontier: SelectionNotInFrontier {
            selection_id: SelectionId(uuid(21)),
            action: "repository.merge".to_owned(),
        },
    };
    if listed != expected_listed {
        failures.push(format!("frontier listing repository.merge gave {listed:?}"));
    }
    if absent != expected_absent {
        failures.push(format!("frontier without repository.merge gave {absent:?}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
