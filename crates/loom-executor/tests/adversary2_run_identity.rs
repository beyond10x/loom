//! Adversary pass 2 on `story:run-pipeline-skeleton`: a creating command sent again for an
//! identity a record already carries.
//!
//! `ProjectCatalogue` and `RevalidateSelection` are the generated behaviour
//! (`b10x_loom_executor::model::behaviour::Generated`). `SelectAction` is an obligation; `SpecPorts`
//! implements it as its contract in `generated/rust/loom/PLAN.md` states it: `catalogue-unknown`,
//! `not-in-catalogue`, `revision-mismatch`, then `selected` "otherwise, creates
//! `loom.run.Selection`". The specification declares no `existing_instance:` branch on any of its
//! creating commands, so neither the generated behaviour nor a conforming implementation of the
//! contract can refuse an identity that is already stored: each replaces it.
//!
//! The expectations are the specification's own words: `ess/domains/run.yaml` says a selection's
//! `case_revision` is "held equal to that catalogue's by SelectAction", that a selection names "one
//! action from that catalogue", and that a selection is "revalidated once", with `Admitted` and
//! `Refused` terminal.

use b10x_loom_executor::model::behaviour::{
    ActionCatalogueStorage, Context, Generated, SelectionStorage,
};
use b10x_loom_executor::model::obligation::UnmetObligation;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::obligations::{
    ProjectCatalogueBehavior, RevalidateSelectionBehavior, SelectActionBehavior, SelectionsQuery,
};
use b10x_loom_executor::model::run::{
    ActionCatalogueSnapshot, ActionNotInCatalogue, ActionSelected, AnySelection, CatalogueEntry,
    CatalogueEntryStatus, CatalogueId, CatalogueNotFound, CatalogueRevisionMismatch,
    ProjectCatalogue, RevalidateSelection, RevalidateSelectionOutcome, SelectAction,
    SelectActionOutcome, Selection, SelectionData, SelectionId, SelectionSnapshot, SelectionState,
    SelectionStrategy, TurnId,
};

/// In-memory storage, `false` for every `external:` branch, and `SelectAction` as its contract
/// states it.
#[derive(Default)]
struct SpecPorts {
    catalogues: Vec<ActionCatalogueSnapshot>,
    selections: Vec<SelectionSnapshot>,
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
    fn external(&mut self, _command: &'static str, _outcome: &'static str) -> bool {
        false
    }
}

/// `loom.run.SelectAction` as the contract in `PLAN.md` states it.
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

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

fn project(
    model: &mut Generated<SpecPorts>,
    catalogue: u32,
    turn: u32,
    revision: i64,
    entries: Vec<CatalogueEntry>,
) {
    model
        .project_catalogue(ProjectCatalogue {
            catalogue_id: CatalogueId(uuid(catalogue)),
            turn_id: TurnId(uuid(turn)),
            frontier: format!("frontier-{revision}"),
            case_revision: revision,
            entries,
        })
        .expect("ProjectCatalogue is generated");
}

fn select(
    model: &mut Generated<SpecPorts>,
    selection: u32,
    catalogue: u32,
    action: &str,
    case_revision: i64,
) -> SelectActionOutcome {
    model
        .select_action(SelectAction {
            selection_id: SelectionId(uuid(selection)),
            catalogue_id: CatalogueId(uuid(catalogue)),
            action: action.to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision,
        })
        .expect("SelectAction as its contract states it")
}

fn revalidate(
    model: &mut Generated<SpecPorts>,
    selection: u32,
    case_revision: i64,
) -> RevalidateSelectionOutcome {
    model
        .revalidate_selection(RevalidateSelection {
            selection_id: SelectionId(uuid(selection)),
            case_revision,
            frontier_actions: vec!["repository.read".to_owned(), "repository.merge".to_owned()],
        })
        .expect("RevalidateSelection is generated")
}

/// `run.yaml:141-142`: a selection's `case_revision` is "held equal to that catalogue's by
/// SelectAction", and `run.yaml:8-9`: "a selection names one action from that catalogue".
/// `ProjectCatalogue` sent a second time for catalogue 1 — another turn, revision 8, without the
/// selected action — must leave both true for the selection already made on it.
#[test]
fn projecting_a_stored_catalogue_id_again_keeps_its_selections_true() {
    let mut model = Generated::new(SpecPorts::default());
    project(
        &mut model,
        1,
        2,
        7,
        vec![
            entry("repository.read", CatalogueEntryStatus::Admissible),
            entry("repository.merge", CatalogueEntryStatus::ApprovalRequired),
        ],
    );
    assert!(matches!(
        select(&mut model, 10, 1, "repository.merge", 7),
        SelectActionOutcome::Selected { .. }
    ));

    project(
        &mut model,
        1,
        3,
        8,
        vec![entry("repository.read", CatalogueEntryStatus::Admissible)],
    );

    let mut failures = Vec::new();
    for selection in SelectionStorage::list(&model.ports) {
        let Some(catalogue) =
            ActionCatalogueStorage::get(&model.ports, &selection.data.catalogue_id)
        else {
            failures.push(format!(
                "{:?}: its catalogue is gone",
                selection.data.selection_id
            ));
            continue;
        };
        if catalogue.data.case_revision != selection.data.case_revision {
            failures.push(format!(
                "selection case_revision {} but its catalogue now holds {} (turn {:?})",
                selection.data.case_revision, catalogue.data.case_revision, catalogue.data.turn_id
            ));
        }
        if !catalogue
            .data
            .entries
            .iter()
            .any(|entry| entry.action == selection.data.action)
        {
            failures.push(format!(
                "selection names {} but its catalogue now lists {:?}",
                selection.data.action,
                catalogue
                    .data
                    .entries
                    .iter()
                    .map(|entry| entry.action.as_str())
                    .collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "the generated ProjectCatalogue replaced catalogue 1 under a selection made on it:\n{}",
        failures.join("\n")
    );
}

/// `run.yaml:151-155`: a selection is "revalidated once", and `Refused` is terminal. Selection 10
/// is refused as stale; `SelectAction` sent again for selection id 10 must not bring it back to
/// `Selected`, and a later revalidation must not admit it.
#[test]
#[ignore = "story:interruption-recovery; ESS-SYNTH-004"]
fn a_refused_selection_is_never_selected_or_admitted_again() {
    let mut model = Generated::new(SpecPorts::default());
    let entries = || {
        vec![
            entry("repository.read", CatalogueEntryStatus::Admissible),
            entry("repository.merge", CatalogueEntryStatus::ApprovalRequired),
        ]
    };
    project(&mut model, 1, 2, 7, entries());
    assert!(matches!(
        select(&mut model, 10, 1, "repository.merge", 7),
        SelectActionOutcome::Selected { .. }
    ));
    assert!(matches!(
        revalidate(&mut model, 10, 8),
        RevalidateSelectionOutcome::StaleRevision { .. }
    ));

    project(&mut model, 4, 5, 8, entries());
    let again = select(&mut model, 10, 4, "repository.merge", 8);
    let after = revalidate(&mut model, 10, 8);
    let rows = model.selections().expect("Selections is generated");
    let state = rows
        .iter()
        .find(|row| row.selection_id == SelectionId(uuid(10)))
        .map(|row| row.state);

    let mut failures = Vec::new();
    if matches!(again, SelectActionOutcome::Selected { .. }) {
        failures.push(format!(
            "SelectAction for the refused selection id 10 answered {again:?}"
        ));
    }
    if matches!(after, RevalidateSelectionOutcome::Admitted { .. }) {
        failures.push(format!(
            "the second revalidation of selection 10 answered {after:?}"
        ));
    }
    if state != Some(SelectionState::Refused) {
        failures.push(format!("selection 10 now rests in {state:?}, not Refused"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
