//! Adversary pass 2 on `story:action-selector`: `Loom::run` over the projected catalogue against
//! Commission's admission ladder and run derivation (commission `174bf07`), the ESS `SelectAction`
//! payload, and the hand-written pages that describe the selector seam.
//!
//! Pass 1 pinned membership and the selector's errors. These cases ask whether the catalogue the
//! selector is shown, the entry Loom hands its argument generator and the decision Commission takes
//! on the proposal are one answer for every frontier that lists one action up to three times.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, Admission, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::{Derived, derive};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    SelectionId, SelectionStrategy, TurnId, action_catalogue_state,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, FirstAdmissibleSelector, Loom,
    SelectorError,
};

const CASE: &str = "CHG-1842";
const MERGE: &str = "repository.merge";
const INSPECT: &str = "repository.inspect";

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(revision: i64, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000002f0".to_owned(),
        )),
        case_id: CaseId(CASE.to_owned()),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn listed(
    action: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reason: &str,
) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec![reason.to_owned()],
    }
}

/// A selector that names one action and records what it is handed.
struct Names(String);

impl ActionSelector for Names {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Ok(Choice {
            action: self.0.clone(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// An argument generator that keeps every entry it is handed.
///
/// `story:argument-generator`: the generator is handed the selected catalogue entry, no longer a
/// frontier entry, so what it keeps is a `CatalogueEntry`.
#[derive(Default, Clone)]
struct Keeps(Rc<RefCell<Vec<CatalogueEntry>>>);

impl ArgumentGenerator for Keeps {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.borrow_mut().push(entry.clone());
        Ok(Value::Object(Vec::new()))
    }
}

/// Every frontier listing `MERGE` zero to three times, each entry one of six kinds with a distinct
/// reason, with an admissible `INSPECT` placed before, between or after them.
fn frontiers() -> Vec<(String, Vec<FrontierAction>)> {
    let kinds: [(ActionStatus, Option<&str>); 6] = [
        (ActionStatus::Admissible, None),
        (ActionStatus::Blocked, None),
        (ActionStatus::ApprovalRequired, Some("cap.a")),
        (ActionStatus::ApprovalRequired, Some("cap.b")),
        (ActionStatus::ApprovalRequired, None),
        (ActionStatus::ApprovalRequired, Some(" ")),
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
    let mut out = Vec::new();
    for pick in &picks {
        for inspect_at in 0..=pick.len() {
            let mut actions = Vec::new();
            for (index, &kind) in pick.iter().enumerate() {
                if index == inspect_at {
                    actions.push(listed(INSPECT, ActionStatus::Admissible, None, "inspect"));
                }
                actions.push(listed(
                    MERGE,
                    kinds[kind].0,
                    kinds[kind].1,
                    &format!("merge #{index} {:?}", kinds[kind]),
                ));
            }
            if inspect_at == pick.len() {
                actions.push(listed(INSPECT, ActionStatus::Admissible, None, "inspect"));
            }
            let shape = format!(
                "{:?} with inspect at {inspect_at}",
                pick.iter().map(|&kind| kinds[kind]).collect::<Vec<_>>()
            );
            out.push((shape, actions));
        }
    }
    out
}

fn catalogue_of(
    frontier: &Frontier<frontier_state::Issued>,
) -> ActionCatalogue<action_catalogue_state::Projected> {
    project(
        frontier,
        CatalogueId(Uuid("00000000-0000-4000-8000-0000000002c0".to_owned())),
        TurnId(Uuid("00000000-0000-4000-8000-0000000002a0".to_owned())),
    )
}

/// For every catalogue entry the selector may name, the entry Loom hands its argument generator
/// is one of that action's frontier entries, with the status the catalogue showed the selector
/// (`Admissible` for `Admissible`, `ApprovalRequired` naming the capability Commission will ask
/// for, for `ApprovalRequired`), and Commission's run derivation then takes the branch that status
/// implies: continue for an admissible proposal, the authority path for an approval-gated one.
///
/// `story:argument-generator`: the generator is handed the selected catalogue entry itself, not a
/// frontier entry, and a catalogue entry carries no capability. So "one of that action's frontier
/// entries" is now "the catalogue entry the selector named", and the capability Commission will
/// ask for is checked against the frontier entry that backs the proposal, not against the
/// generator's input.
#[test]
fn the_generator_sees_the_entry_the_catalogue_showed_and_commission_decides() {
    let mut wrong = Vec::new();
    let mut approval_entries = 0;
    for (shape, actions) in frontiers() {
        let frontier = frontier(7, actions);
        let catalogue = catalogue_of(&frontier);
        for entry in &catalogue.data().entries {
            let kept = Keeps::default();
            let outcome = Loom::new(Names(entry.action.clone()), kept.clone(), "go")
                .run(&commission(), &frontier);
            let ExecutorOutcome::ProposedAction(proposal) = &outcome else {
                wrong.push(format!(
                    "{shape}: catalogue lists {entry:?}, Loom: {outcome:?}"
                ));
                continue;
            };
            if proposal.action != entry.action {
                wrong.push(format!(
                    "{shape}: proposed {} for {entry:?}",
                    proposal.action
                ));
            }
            let handed = kept.0.borrow().clone();
            let [given] = handed.as_slice() else {
                wrong.push(format!(
                    "{shape}: generator handed {handed:?} for {entry:?}"
                ));
                continue;
            };
            if !catalogue.data().entries.contains(given) || given != entry {
                wrong.push(format!("{shape}: generator handed {given:?} for {entry:?}"));
            }
            match (entry.status, admit(&frontier, &entry.action)) {
                (CatalogueEntryStatus::Admissible, Admission::Admissible(_)) => {
                    if given.status != CatalogueEntryStatus::Admissible {
                        wrong.push(format!(
                            "{shape}: catalogue shows {entry:?}, generator handed {given:?}"
                        ));
                    }
                    let derived = derive(
                        &CompletionDetermination::Open(Unit(true)),
                        &frontier,
                        &outcome,
                        None,
                    );
                    if derived != Derived::Continue {
                        wrong.push(format!("{shape}: {entry:?} derives {derived:?}"));
                    }
                }
                (CatalogueEntryStatus::ApprovalRequired, Admission::NeedsAuthority(needs)) => {
                    approval_entries += 1;
                    let backed = frontier.data().actions.iter().any(|listed| {
                        listed.action == entry.action
                            && listed.status == ActionStatus::ApprovalRequired
                            && listed.capability.as_deref() == Some(needs.capability.as_str())
                    });
                    if given.status != CatalogueEntryStatus::ApprovalRequired || !backed {
                        wrong.push(format!(
                            "{shape}: catalogue shows {entry:?} needing {:?}, generator handed \
                             {given:?}",
                            needs.capability
                        ));
                    }
                }
                (status, admission) => wrong.push(format!(
                    "{shape}: catalogue shows {status:?}, Commission says {admission:?}"
                )),
            }
        }
    }
    assert!(
        approval_entries > 0,
        "the frontiers must include approval-gated catalogue entries"
    );
    assert!(
        wrong.is_empty(),
        "{} disagreements; first five:\n{}",
        wrong.len(),
        wrong.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// `FirstAdmissibleSelector` through `Loom::run`, on the same frontiers: it proposes the first
/// `Admissible` catalogue entry and hands the generator an `Admissible` frontier entry for it, or,
/// when the catalogue has no `Admissible` entry, answers `NoUsefulAction`. The answer does not
/// depend on where the frontier lists a duplicated action's entries relative to each other.
///
/// `story:argument-generator`: the entry the generator is handed is the `Admissible` catalogue
/// entry the selector named, no longer a frontier entry.
#[test]
fn the_first_admissible_selector_follows_the_catalogue_not_the_raw_frontier_status() {
    let mut wrong = Vec::new();
    for (shape, actions) in frontiers() {
        let frontier = frontier(7, actions.clone());
        let catalogue = catalogue_of(&frontier);
        let expected = catalogue
            .data()
            .entries
            .iter()
            .find(|entry| entry.status == CatalogueEntryStatus::Admissible)
            .map(|entry| entry.action.clone());
        let kept = Keeps::default();
        let outcome =
            Loom::new(FirstAdmissibleSelector, kept.clone(), "go").run(&commission(), &frontier);
        match (&expected, &outcome) {
            (Some(action), ExecutorOutcome::ProposedAction(proposal))
                if &proposal.action == action =>
            {
                let handed = kept.0.borrow();
                if handed.len() != 1 || handed[0].status != CatalogueEntryStatus::Admissible {
                    wrong.push(format!("{shape}: generator handed {handed:?}"));
                }
            }
            (None, ExecutorOutcome::NoUsefulAction(Unit(true))) => {}
            _ => wrong.push(format!("{shape}: expected {expected:?}, Loom: {outcome:?}")),
        }

        // The same entries of MERGE in reverse order: the same proposal.
        let mut reversed: Vec<FrontierAction> = actions.clone();
        let merge_positions: Vec<usize> = reversed
            .iter()
            .enumerate()
            .filter(|(_, listed)| listed.action == MERGE)
            .map(|(index, _)| index)
            .collect();
        let merges: Vec<FrontierAction> = merge_positions
            .iter()
            .rev()
            .map(|&index| actions[index].clone())
            .collect();
        for (slot, entry) in merge_positions.iter().zip(merges) {
            reversed[*slot] = entry;
        }
        let other = Loom::new(FirstAdmissibleSelector, Keeps::default(), "go")
            .run(&commission(), &frontier_with(7, reversed));
        if other != outcome {
            wrong.push(format!(
                "{shape}: reversing merge's entries changes {outcome:?} to {other:?}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} disagreements; first five:\n{}",
        wrong.len(),
        wrong.iter().take(5).cloned().collect::<Vec<_>>().join("\n")
    );
}

fn frontier_with(revision: i64, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    frontier(revision, actions)
}

/// ESS `loom.run.SelectAction` refuses `revision-mismatch` when the case revision a selection
/// claims is not the catalogue's. Loom builds the selection itself, so the revision it records is
/// the catalogue's at every boundary, for projected and hand-built catalogues alike, and the
/// catalogue id is the catalogue's.
#[test]
fn a_selection_always_carries_its_catalogues_revision_and_id() {
    for revision in [i64::MIN, -1, 0, 1, 7, i64::MAX] {
        let frontier = frontier(
            revision,
            vec![listed(INSPECT, ActionStatus::Admissible, None, "inspect")],
        );
        let projected = catalogue_of(&frontier);
        let hand_built = ActionCatalogue::new(ActionCatalogueData {
            catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-0000000002c1".to_owned())),
            turn_id: TurnId(Uuid("00000000-0000-4000-8000-0000000002a1".to_owned())),
            frontier: "another frontier".to_owned(),
            case_revision: revision.wrapping_add(1),
            entries: vec![CatalogueEntry {
                action: INSPECT.to_owned(),
                status: CatalogueEntryStatus::Admissible,
            }],
        });
        for catalogue in [&projected, &hand_built] {
            let selection = Loom::new(Names(INSPECT.to_owned()), Keeps::default(), "go")
                .select(
                    catalogue,
                    SelectionId(Uuid("00000000-0000-4000-8000-0000000002e0".to_owned())),
                )
                .expect("an entry the catalogue lists is selected")
                .into_data();
            assert_eq!(
                (selection.case_revision, &selection.catalogue_id),
                (
                    catalogue.data().case_revision,
                    &catalogue.data().catalogue_id
                ),
                "revision {revision}"
            );
        }
    }
}

fn page(relative: &str) -> (PathBuf, String) {
    let root = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    )
    .join("../..");
    let path = root.join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (path, text)
}

/// The hand-written site pages say what a selector is handed. Since `story:action-selector` it is
/// the selection context and the projected catalogue's entries (`selection::ActionSelector`), so a
/// page that still says a selector sees the frontier tells a selector author the wrong input.
#[test]
fn site_pages_do_not_say_a_selector_sees_the_frontier() {
    // The code: a selector is handed catalogue entries, never frontier entries. A frontier with a
    // blocked action shows the selector fewer candidates than the frontier lists.
    let frontier = frontier(
        1,
        vec![
            listed(MERGE, ActionStatus::Blocked, None, "awaiting review"),
            listed(INSPECT, ActionStatus::Admissible, None, "inspect"),
        ],
    );
    let catalogue = catalogue_of(&frontier);
    assert_eq!(
        catalogue.data().entries,
        vec![CatalogueEntry {
            action: INSPECT.to_owned(),
            status: CatalogueEntryStatus::Admissible,
        }]
    );

    let stale = "a selector sees the frontier";
    let mut wrong = Vec::new();
    for relative in [
        "website/docs/status.mdx",
        "website/docs/concepts/action-selection.md",
    ] {
        let (path, text) = page(relative);
        for (number, line) in text.lines().enumerate() {
            if line.contains(stale) {
                wrong.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "pages still describe the bootstrap selector seam:\n{}",
        wrong.join("\n")
    );
}

/// The status page lists, as shipped, that an approval-gated action suspends the run and Loom
/// proposes nothing. Loom proposes it and Commission asks for the authority (Atlas ADR 0082).
#[test]
fn the_status_page_does_not_say_approval_gated_actions_suspend() {
    let frontier = frontier(
        1,
        vec![listed(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
            "needs review",
        )],
    );
    let outcome =
        Loom::new(Names(MERGE.to_owned()), Keeps::default(), "go").run(&commission(), &frontier);
    assert!(
        matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == MERGE),
        "{outcome:?}"
    );

    let (path, text) = page("website/docs/status.mdx");
    let claims: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("Approval-gated actions suspend the run"))
        .map(|(number, line)| format!("{}:{}: {}", path.display(), number + 1, line.trim()))
        .collect();
    assert!(
        claims.is_empty(),
        "Loom answered {outcome:?}, and the status page says:\n{}",
        claims.join("\n")
    );
}
