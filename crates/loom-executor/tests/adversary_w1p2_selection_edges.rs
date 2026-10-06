//! Adversary pass 2, wave 2026-10-06-w1, `story:selection-revalidation`: edges of
//! `Loom::with_governor` that pass 1 (`adversary_w1_selection_boundaries.rs`) did not reach.
//!
//! - The stale-revision comparison at the ends of `i64`, and between neighbours that one `f64`
//!   cannot tell apart: the generated guard compares decimal renderings, and a lossy comparison
//!   would admit a selection made at another revision.
//! - `with_governor` called on a Loom that has already run: the record and the run numbering carry
//!   over, so a governed run cannot reuse an ungoverned run's selection or argument-request id.

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::model::run::{
    CatalogueEntry, RevalidateSelectionOutcome, SelectionAdmitted, SelectionStale, SelectionState,
    SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

const CASE: &str = "CASE-1";
const EDIT: &str = "repository.edit";

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn edit() -> FrontierAction {
    FrontierAction {
        action: EDIT.to_owned(),
        status: ActionStatus::Admissible,
        capability: None,
        reasons: Vec::new(),
    }
}

/// The frontier a run is handed: `revision`, listing `repository.edit` as admissible.
fn handed(revision: i64) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000f0".to_owned(),
        )),
        case_id: case(),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![edit()],
    })
}

/// The governor's current frontier: `revision`, listing `repository.edit` as admissible.
fn current(revision: i64) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), vec![edit()])
}

/// Picks `repository.edit`.
struct PicksEdit;

impl ActionSelector for PicksEdit {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .iter()
            .find(|entry| entry.action == EDIT)
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

const TWO_TO_53: i64 = 1 << 53;

/// A selection made at one revision and revalidated against a neighbouring one is refused stale,
/// naming both exactly, at the ends of `i64` and where `f64` rounds the two to one value. The same
/// revision on both sides is admitted.
#[test]
fn stale_revision_is_exact_at_the_far_ends_of_i64() {
    // (the handed frontier's revision, the governor's current one)
    let stale = [
        (TWO_TO_53, TWO_TO_53 + 1),
        (TWO_TO_53 + 1, TWO_TO_53),
        (i64::MAX - 1, i64::MAX),
        (i64::MAX, i64::MAX - 1),
        (i64::MIN, i64::MIN + 1),
        (i64::MIN + 1, i64::MIN),
        (i64::MIN, i64::MAX),
        (-1, 0),
        (0, -1),
    ];
    let same = [i64::MIN, -1, 0, TWO_TO_53 + 1, i64::MAX];

    let mut failures = Vec::new();
    for (selected_at, now) in stale {
        let governor = FakeGovernor::new();
        governor.script(case(), [current(now)]);
        let loom = Loom::new(PicksEdit, EmptyObjectArguments, "edit").with_governor(&governor);

        let outcome = loom.run(&commission(), &handed(selected_at));

        if outcome != ExecutorOutcome::NoUsefulAction(Unit(true)) {
            failures.push(format!(
                "selected at {selected_at}, current {now}: Loom returned {outcome:?}"
            ));
        }
        let selection_id = loom
            .selections()
            .last()
            .map(|held| held.data.selection_id.clone());
        let expected: Vec<RevalidateSelectionOutcome> = selection_id
            .into_iter()
            .map(|selection_id| RevalidateSelectionOutcome::StaleRevision {
                selection_stale: SelectionStale {
                    selection_id,
                    catalogue_revision: selected_at,
                    case_revision: now,
                },
            })
            .collect();
        if loom.revalidations() != expected || expected.is_empty() {
            failures.push(format!(
                "selected at {selected_at}, current {now}: revalidations {:?}, expected {expected:?}",
                loom.revalidations()
            ));
        }
    }
    for revision in same {
        let governor = FakeGovernor::new();
        governor.script(case(), [current(revision)]);
        let loom = Loom::new(PicksEdit, EmptyObjectArguments, "edit").with_governor(&governor);

        let outcome = loom.run(&commission(), &handed(revision));

        if !matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == EDIT)
        {
            failures.push(format!(
                "selected and current at {revision}: Loom returned {outcome:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An ungoverned run, then `with_governor` on the same Loom, then a governed run on the same
/// frontier: two selections and two argument requests with ids of their own, the first selection
/// never revalidated and still `Selected`, the second revalidated once and `Admitted`.
#[test]
fn with_governor_keeps_the_record_and_the_run_numbering() {
    let governor = FakeGovernor::new();
    governor.script(case(), [current(7)]);
    let loom = Loom::new(PicksEdit, EmptyObjectArguments, "edit");

    let first = loom.run(&commission(), &handed(7));
    let loom = loom.with_governor(&governor);
    let second = loom.run(&commission(), &handed(7));

    for (name, outcome) in [("ungoverned", &first), ("governed", &second)] {
        assert!(
            matches!(outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == EDIT),
            "{name} run: {outcome:?}"
        );
    }
    let selections = loom.selections();
    assert_eq!(selections.len(), 2, "{selections:#?}");
    assert_ne!(
        selections[0].data.selection_id, selections[1].data.selection_id,
        "{selections:#?}"
    );
    assert_eq!(
        [selections[0].state, selections[1].state],
        [SelectionState::Selected, SelectionState::Admitted],
        "{selections:#?}"
    );
    let requests = loom.argument_requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    assert_ne!(
        requests[0].data.argument_request_id, requests[1].data.argument_request_id,
        "{requests:#?}"
    );
    assert_eq!(
        loom.revalidations(),
        [RevalidateSelectionOutcome::Admitted {
            selection_admitted: SelectionAdmitted {
                selection_id: selections[1].data.selection_id.clone(),
            },
        }]
    );
    assert_eq!(governor.calls(), [GovernorCall::Frontier(case())]);
}
