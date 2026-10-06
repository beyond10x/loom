//! Acceptance for `story:selection-revalidation`: before Loom returns a `ProposedAction`, it asks
//! the governor for the case's current frontier and revalidates the selection against it
//! (`loom.run.RevalidateSelection`). The revision and the action ids come from the governor, never
//! from the model or from the frontier the run was handed.
//!
//! Every run here is handed the same frontier, at revision 7 and listing `repository.read` and
//! `repository.edit`; the selector always picks `repository.edit`. What the fake governor answers
//! at revalidation is what each case varies. The selector notes how many calls the governor had
//! received when it selected, so "after the selection" is observed, not assumed.

use std::cell::Cell;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, Frontier, FrontierAction, FrontierData, FrontierId, GovernorError,
    PrincipalId, ProposedActionArguments, SuspensionReason, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::model::run::{
    CatalogueEntry, RevalidateSelectionOutcome, SelectionAdmitted, SelectionId,
    SelectionNotInFrontier, SelectionStale, SelectionState, SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

const CASE: &str = "CASE-1";
const READ: &str = "repository.read";
const EDIT: &str = "repository.edit";
/// The revision of the frontier every run is handed, and so of the catalogue it selects from.
const REVISION: i64 = 7;

/// Picks `repository.edit` whenever it is a candidate, and notes how many calls `governor` had
/// received at that moment.
struct PicksEdit<'g> {
    governor: &'g dyn Calls,
    calls_at_selection: Cell<Option<usize>>,
}

/// How many `Governor` calls a governor has received.
trait Calls {
    fn count(&self) -> usize;
}

impl Calls for FakeGovernor {
    fn count(&self) -> usize {
        self.calls().len()
    }
}

impl<'g> PicksEdit<'g> {
    fn new(governor: &'g dyn Calls) -> Self {
        Self {
            governor,
            calls_at_selection: Cell::new(None),
        }
    }
}

impl ActionSelector for PicksEdit<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.calls_at_selection.set(Some(self.governor.count()));
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

fn listed(action: &str, status: ActionStatus) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: None,
        reasons: Vec::new(),
    }
}

/// The frontier every run is handed: revision 7, `repository.read` and `repository.edit` both
/// admissible. Its id is one the fake governor never issues.
fn handed() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000f0".to_owned(),
        )),
        case_id: case(),
        case_revision: REVISION,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![
            listed(READ, ActionStatus::Admissible),
            listed(EDIT, ActionStatus::Admissible),
        ],
    })
}

/// The governor's answer at revalidation: the case at `revision`, its frontier listing `actions`.
fn current(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

/// What one run with a governor scripted to `answer` produced.
struct Observed {
    outcome: ExecutorOutcome,
    selection: Option<(SelectionId, SelectionState)>,
    revalidations: Vec<RevalidateSelectionOutcome>,
    calls_at_selection: Option<usize>,
    calls: Vec<GovernorCall>,
}

fn run_with(answer: Answer) -> Observed {
    let governor = FakeGovernor::new();
    governor.script(case(), [answer]);
    let selector = PicksEdit::new(&governor);
    let loom =
        Loom::new(&selector, EmptyObjectArguments, "edit the change").with_governor(&governor);

    let outcome = loom.run(&commission(), &handed());

    let selection = loom
        .selections()
        .last()
        .map(|held| (held.data.selection_id.clone(), held.state));
    Observed {
        outcome,
        selection,
        revalidations: loom.revalidations(),
        calls_at_selection: selector.calls_at_selection.get(),
        calls: governor.calls(),
    }
}

impl ActionSelector for &PicksEdit<'_> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        (*self).select(context, candidates)
    }

    fn strategy(&self) -> SelectionStrategy {
        (*self).strategy()
    }
}

/// Acceptance 4 for one case: no governor call before the selection, and exactly one frontier call
/// for the commission's case between the selection and the return.
fn asked_once_after_selection(name: &str, observed: &Observed, failures: &mut Vec<String>) {
    if observed.calls_at_selection != Some(0) {
        failures.push(format!(
            "{name}: the governor had received {:?} calls when the selector picked; expected 0",
            observed.calls_at_selection
        ));
    }
    if observed.calls != [GovernorCall::Frontier(case())] {
        failures.push(format!(
            "{name}: the governor received {:?}; expected exactly one frontier call for {CASE}",
            observed.calls
        ));
    }
}

/// `story:selection-revalidation` § Acceptance, items 1–4.
#[test]
fn revalidation_refuses_before_proposing() {
    let mut failures = Vec::new();

    // 1. The selected action is absent from the governor's current frontier: refused, naming the
    //    action and the reason, and nothing is proposed.
    let absent = run_with(current(
        REVISION,
        vec![listed(READ, ActionStatus::Admissible)],
    ));
    asked_once_after_selection("not in frontier", &absent, &mut failures);
    match &absent.selection {
        Some((selection_id, state)) => {
            let expected = vec![RevalidateSelectionOutcome::NotInFrontier {
                selection_not_in_frontier: SelectionNotInFrontier {
                    selection_id: selection_id.clone(),
                    action: EDIT.to_owned(),
                },
            }];
            if absent.revalidations != expected {
                failures.push(format!(
                    "not in frontier: revalidations {:?}, expected {expected:?}",
                    absent.revalidations
                ));
            }
            if *state != SelectionState::Refused {
                failures.push(format!(
                    "not in frontier: the selection is {state:?}, not Refused"
                ));
            }
        }
        None => failures.push("not in frontier: no selection was recorded".to_owned()),
    }
    if absent.outcome != ExecutorOutcome::NoUsefulAction(Unit(true)) {
        failures.push(format!(
            "not in frontier: Loom returned {:?}, expected NoUsefulAction",
            absent.outcome
        ));
    }

    // 2. The governor's case is at revision n + 1 while the selection was made at n: refused,
    //    naming both revisions, and nothing is proposed.
    let stale = run_with(current(
        REVISION + 1,
        vec![
            listed(READ, ActionStatus::Admissible),
            listed(EDIT, ActionStatus::Admissible),
        ],
    ));
    asked_once_after_selection("stale revision", &stale, &mut failures);
    match &stale.selection {
        Some((selection_id, state)) => {
            let expected = vec![RevalidateSelectionOutcome::StaleRevision {
                selection_stale: SelectionStale {
                    selection_id: selection_id.clone(),
                    catalogue_revision: REVISION,
                    case_revision: REVISION + 1,
                },
            }];
            if stale.revalidations != expected {
                failures.push(format!(
                    "stale revision: revalidations {:?}, expected {expected:?}",
                    stale.revalidations
                ));
            }
            if *state != SelectionState::Refused {
                failures.push(format!(
                    "stale revision: the selection is {state:?}, not Refused"
                ));
            }
        }
        None => failures.push("stale revision: no selection was recorded".to_owned()),
    }
    if stale.outcome != ExecutorOutcome::NoUsefulAction(Unit(true)) {
        failures.push(format!(
            "stale revision: Loom returned {:?}, expected NoUsefulAction",
            stale.outcome
        ));
    }

    // 3. The selection passes both checks: it is proposed.
    let passes = run_with(current(
        REVISION,
        vec![
            listed(READ, ActionStatus::Admissible),
            listed(EDIT, ActionStatus::Admissible),
        ],
    ));
    asked_once_after_selection("admitted", &passes, &mut failures);
    match &passes.selection {
        Some((selection_id, state)) => {
            let expected = vec![RevalidateSelectionOutcome::Admitted {
                selection_admitted: SelectionAdmitted {
                    selection_id: selection_id.clone(),
                },
            }];
            if passes.revalidations != expected {
                failures.push(format!(
                    "admitted: revalidations {:?}, expected {expected:?}",
                    passes.revalidations
                ));
            }
            if *state != SelectionState::Admitted {
                failures.push(format!(
                    "admitted: the selection is {state:?}, not Admitted"
                ));
            }
        }
        None => failures.push("admitted: no selection was recorded".to_owned()),
    }
    let proposed = ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: EDIT.to_owned(),
        arguments: ProposedActionArguments(Value::Object(Vec::new())),
    });
    if passes.outcome != proposed {
        failures.push(format!(
            "admitted: Loom returned {:?}, expected {proposed:?}",
            passes.outcome
        ));
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The current frontier still lists the selected action, but blocks it: the catalogue projected
/// from that frontier does not list it, so it is not in the frontier the selection is revalidated
/// against.
#[test]
fn an_action_the_current_frontier_blocks_is_not_in_frontier() {
    let observed = run_with(current(
        REVISION,
        vec![
            listed(READ, ActionStatus::Admissible),
            listed(EDIT, ActionStatus::Blocked),
        ],
    ));
    assert_eq!(
        observed.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true))
    );
    assert!(
        matches!(
            observed.revalidations.as_slice(),
            [RevalidateSelectionOutcome::NotInFrontier { selection_not_in_frontier }]
                if selection_not_in_frontier.action == EDIT
        ),
        "{:?}",
        observed.revalidations
    );
}

/// A governor that cannot answer: nothing is proposed, and the run is suspended for its
/// availability with the governor's error, as a selector outage is.
#[test]
fn a_governor_error_is_an_outage_and_proposes_nothing() {
    let observed = run_with(Answer::unavailable());
    let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: SuspensionReason::ExternalAvailability(detail),
    }) = &observed.outcome
    else {
        panic!("not an outage: {:?}", observed.outcome);
    };
    let Some(Value::Text(error)) = detail.member("error") else {
        panic!("the outage carries no error: {detail:?}");
    };
    assert!(error.contains("GovernorUnavailable"), "{error}");
    assert!(
        observed.revalidations.is_empty(),
        "{:?}",
        observed.revalidations
    );
    assert_eq!(observed.calls, [GovernorCall::Frontier(case())]);
}

/// A governor that issues, as the current frontier, one for another case than the one it was
/// asked about.
struct OtherCase;

impl Calls for OtherCase {
    fn count(&self) -> usize {
        0
    }
}

impl Governor for OtherCase {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(REVISION)
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(CommissionUuid(
                "00000000-0000-4000-8000-0000000000f1".to_owned(),
            )),
            case_id: CaseId("CASE-2".to_owned()),
            case_revision: REVISION,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![listed(EDIT, ActionStatus::Admissible)],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

/// The governor answers with another case's frontier: the selection is not revalidated against it,
/// nothing is proposed, and the run is suspended naming both cases.
#[test]
fn a_frontier_for_another_case_from_the_governor_proposes_nothing() {
    let governor = OtherCase;
    let selector = PicksEdit::new(&governor);
    let loom =
        Loom::new(&selector, EmptyObjectArguments, "edit the change").with_governor(&governor);

    let outcome = loom.run(&commission(), &handed());

    let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: SuspensionReason::ExternalAvailability(detail),
    }) = &outcome
    else {
        panic!("not an outage: {outcome:?}");
    };
    let Some(Value::Text(error)) = detail.member("error") else {
        panic!("the outage carries no error: {detail:?}");
    };
    assert!(error.contains("CASE-2") && error.contains(CASE), "{error}");
    assert!(
        loom.revalidations().is_empty(),
        "{:?}",
        loom.revalidations()
    );
}

/// Without a governor, Loom proposes what it selected, as it did before revalidation, and records
/// no revalidation.
#[test]
fn a_loom_without_a_governor_proposes_unrevalidated() {
    let governor = FakeGovernor::new();
    let selector = PicksEdit::new(&governor);
    let loom = Loom::new(&selector, EmptyObjectArguments, "edit the change");

    let outcome = loom.run(&commission(), &handed());

    assert!(
        matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == EDIT),
        "{outcome:?}"
    );
    assert!(
        loom.revalidations().is_empty(),
        "{:?}",
        loom.revalidations()
    );
    assert!(governor.calls().is_empty(), "{:?}", governor.calls());
}
