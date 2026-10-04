//! Acceptance for `story:action-selector`: a selector is handed the selection context and the
//! catalogue projected from the current frontier, and nothing else; it returns one action id and an
//! optional confidence (`docs/contracts/loom-action-selection.md`, Atlas ADR 0073). Loom refuses an
//! id the catalogue does not list, whatever the confidence, and names it in the refusal; nothing is
//! generated for a refused selection. What Loom returns for an accepted one is the synthesized
//! `loom.run.Selection`, its confidence `Optional<Decimal>`.
//!
//! The frontier is Commission's generated `Frontier`, built here and projected with
//! `projection::project` exactly as `Loom::run` projects it. It lists an approval-gated action
//! first, so the first catalogue entry is not the first admissible one, and a blocked action, which
//! the frontier lists and the catalogue does not.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    ProposedActionArguments, Unit, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::model::primitives::{Decimal, Uuid};
use b10x_loom::model::run::{
    ActionCatalogue, ActionNotInCatalogue, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    Selection, SelectionData, SelectionId, SelectionState, SelectionStrategy, TurnId,
    action_catalogue_state, selection_state,
};
use b10x_loom::projection::project;
use b10x_loom::selection::{Choice, SelectionContext, SelectionRefusal};
use b10x_loom::{ActionSelector, ArgumentGenerator, FirstAdmissibleSelector, Loom, SelectorError};

const CASE: &str = "CHG-1842";

const REVISION: i64 = 2;

const PROMPT: &str = "investigate";

/// Approval-gated, listed first.
const MERGE: &str = "repository.merge";

/// The first admissible entry.
const INSPECT: &str = "repository.inspect";

const TESTS_RUN: &str = "tests.run";

/// Listed by the frontier as blocked, so not in the catalogue.
const BLOCKED: &str = "release.publish";

/// Listed nowhere.
const UNLISTED: &str = "forbidden.action";

/// The ids a selector may name that the catalogue does not list.
const ABSENT: [&str; 2] = [BLOCKED, UNLISTED];

fn commission_uuid(n: u32) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn listed(
    action: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
    }
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(commission_uuid(0xf00)),
        case_id: CaseId(CASE.to_owned()),
        case_revision: REVISION,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![
            listed(
                MERGE,
                ActionStatus::ApprovalRequired,
                Some("repository.write"),
                &[],
            ),
            listed(INSPECT, ActionStatus::Admissible, None, &[]),
            listed(TESTS_RUN, ActionStatus::Admissible, None, &[]),
            listed(BLOCKED, ActionStatus::Blocked, None, &["awaiting review"]),
        ],
    })
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(commission_uuid(1)),
        agent_revision_id: AgentRevisionId(commission_uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    project(&frontier(), CatalogueId(uuid(0xc00)), TurnId(uuid(0x700)))
}

fn decimal(text: &str) -> Decimal {
    Decimal(text.to_owned())
}

/// A selector that names one action at one confidence, and records every candidate set it is
/// handed.
struct Names {
    action: &'static str,
    confidence: Option<Decimal>,
    handed: Rc<RefCell<Vec<Vec<CatalogueEntry>>>>,
}

impl Names {
    fn new(action: &'static str, confidence: Option<&str>) -> Self {
        Self {
            action,
            confidence: confidence.map(decimal),
            handed: Rc::default(),
        }
    }
}

impl ActionSelector for Names {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.handed.borrow_mut().push(candidates.to_vec());
        Ok(Choice {
            action: self.action.to_owned(),
            confidence: self.confidence.clone(),
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// An argument generator that counts its calls.
#[derive(Default)]
struct Counting(Rc<Cell<usize>>);

impl ArgumentGenerator for Counting {
    fn generate(
        &self,
        _action: &FrontierAction,
        _prompt: &str,
    ) -> Result<ProposedActionArguments, String> {
        self.0.set(self.0.get() + 1);
        Ok(ProposedActionArguments(Value::Object(Vec::new())))
    }
}

#[test]
fn selector_cannot_leave_catalogue() {
    let catalogue = catalogue();
    let entries = catalogue.data().entries.clone();
    for absent in ABSENT {
        assert!(
            entries.iter().all(|entry| entry.action != absent),
            "the fixture's catalogue lists {absent}: {entries:?}"
        );
    }
    assert_eq!(
        entries.first().map(|entry| entry.status),
        Some(CatalogueEntryStatus::ApprovalRequired),
        "the first catalogue entry is approval-gated, so first entry and first admissible differ"
    );

    // 1 and 2. An id the catalogue does not list, at confidence 1.0, is refused and named; the
    // argument generator is never called for it, through selection or through the executor.
    for absent in ABSENT {
        let selector = Names::new(absent, Some("1.0"));
        let handed = Rc::clone(&selector.handed);
        let generated = Rc::new(Cell::new(0));
        let loom = Loom::new(selector, Counting(Rc::clone(&generated)), PROMPT);

        let refused = loom.select(&catalogue, SelectionId(uuid(0x5e1)));
        assert_eq!(
            refused.map(|selection| selection.into_data()),
            Err(SelectionRefusal::NotInCatalogue(ActionNotInCatalogue {
                action: absent.to_owned(),
            })),
            "{absent} at confidence 1.0"
        );
        assert_eq!(
            handed.borrow().as_slice(),
            std::slice::from_ref(&entries),
            "the selector is handed the projected catalogue's entries and nothing else"
        );
        assert_eq!(
            generated.get(),
            0,
            "arguments generated for refused {absent}"
        );

        let outcome = loom.run(&commission(), &frontier());
        assert_eq!(
            outcome,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            "the executor proposes {absent}"
        );
        assert_eq!(
            handed.borrow().last(),
            Some(&entries),
            "Loom::run hands the selector the catalogue projected from its frontier"
        );
        assert_eq!(
            generated.get(),
            0,
            "arguments generated for refused {absent}"
        );
    }

    // The counter can move: an id the catalogue lists is proposed and its arguments generated.
    let generated = Rc::new(Cell::new(0));
    let listed_choice = Loom::new(
        Names::new(TESTS_RUN, Some("1.0")),
        Counting(Rc::clone(&generated)),
        PROMPT,
    );
    let outcome = listed_choice.run(&commission(), &frontier());
    assert!(
        matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == TESTS_RUN),
        "{outcome:?}"
    );
    assert_eq!(
        generated.get(),
        1,
        "arguments generated once for {TESTS_RUN}"
    );

    // 3. FirstAdmissibleSelector returns the first admissible entry, not the first entry.
    let context = SelectionContext {
        prompt: PROMPT.to_owned(),
    };
    assert_eq!(
        FirstAdmissibleSelector.select(&context, &entries),
        Ok(Choice {
            action: INSPECT.to_owned(),
            confidence: None,
        })
    );

    // 4. What Loom returns is the synthesized `loom.run.Selection`, in `Selected`, over this
    // catalogue at its case revision; its confidence is `Optional<Decimal>`.
    let first = Loom::new(FirstAdmissibleSelector, Counting::default(), PROMPT);
    let selection: Selection<selection_state::Selected> = first
        .select(&catalogue, SelectionId(uuid(0x5e2)))
        .expect("the first admissible entry is selected");
    assert_eq!(selection.state(), SelectionState::Selected);
    assert_eq!(
        selection.into_data(),
        SelectionData {
            selection_id: SelectionId(uuid(0x5e2)),
            catalogue_id: catalogue.data().catalogue_id.clone(),
            action: INSPECT.to_owned(),
            confidence: None,
            strategy: SelectionStrategy::Rule,
            case_revision: REVISION,
        }
    );

    let confident = Loom::new(Names::new(MERGE, Some("0.75")), Counting::default(), PROMPT);
    let selection = confident
        .select(&catalogue, SelectionId(uuid(0x5e3)))
        .expect("an approval-gated entry the catalogue lists is selected");
    let confidence: &Option<Decimal> = &selection.data().confidence;
    assert_eq!(confidence, &Some(decimal("0.75")));
    assert_eq!(selection.data().action, MERGE);
    assert_eq!(selection.data().strategy, SelectionStrategy::ReasoningModel);
    assert_eq!(selection.data().case_revision, REVISION);
}
