//! Acceptance for `story:argument-generator`: arguments are generated for the selected action only
//! (Atlas ADR 0073, step 1; `docs/contracts/loom-action-selection.md`).
//!
//! The generator is handed the argument context and the one catalogue entry the selection names,
//! never the rest of the catalogue, and returns a JSON value. That value reaches the
//! `ProposedAction` as Commission's `ProposedActionArguments`. Before it is generated, Loom records
//! the synthesized `loom.run.ArgumentRequest` (`loom.run.RequestArguments`, outcome `requested`)
//! against the selection it serves.
//!
//! The frontier is Commission's generated `Frontier`, projected with `projection::project` exactly
//! as `Loom::run` projects it, so the test knows the three catalogue entries and their order. The
//! selector picks the second entry by position, so neither the first nor the last entry is the
//! selected one.

use std::cell::RefCell;
use std::rc::Rc;

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    ProposedActionArguments, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::arguments::ArgumentContext;
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{
    ArgumentRequestState, CatalogueEntry, CatalogueId, SelectionStrategy, TurnId,
};
use b10x_loom::projection::project;
use b10x_loom::selection::{Choice, SelectionContext};
use b10x_loom::{ActionSelector, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CHG-1842";

const REVISION: i64 = 3;

/// Names no catalogue action, so an action id in what the generator is handed did not come from
/// the prompt.
const PROMPT: &str = "find out why the build is red";

/// The first catalogue entry.
const INSPECT: &str = "repository.inspect";

/// The second catalogue entry: the one the selector picks.
const TESTS_RUN: &str = "tests.run";

/// The third catalogue entry, approval-gated.
const MERGE: &str = "repository.merge";

fn commission_uuid(n: u32) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
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
            listed(INSPECT, ActionStatus::Admissible, None),
            listed(TESTS_RUN, ActionStatus::Admissible, None),
            listed(
                MERGE,
                ActionStatus::ApprovalRequired,
                Some("repository.write"),
            ),
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

/// A selector that picks the second candidate it is handed, and records every candidate set.
#[derive(Default)]
struct PicksSecond {
    handed: Rc<RefCell<Vec<Vec<CatalogueEntry>>>>,
}

impl ActionSelector for PicksSecond {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.handed.borrow_mut().push(candidates.to_vec());
        let second = candidates.get(1).ok_or(SelectorError::NothingAdmissible)?;
        Ok(Choice {
            action: second.action.clone(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// What the generator was handed on one call.
#[derive(Debug)]
struct Handed {
    context: ArgumentContext,
    entry: CatalogueEntry,
}

/// A generator that records everything it is handed and returns one fixed JSON value.
struct Recording {
    handed: Rc<RefCell<Vec<Handed>>>,
    returns: Value,
}

impl ArgumentGenerator for Recording {
    fn generate(&self, context: &ArgumentContext, entry: &CatalogueEntry) -> Result<Value, String> {
        self.handed.borrow_mut().push(Handed {
            context: context.clone(),
            entry: entry.clone(),
        });
        Ok(self.returns.clone())
    }
}

/// Arguments no default could produce: not the empty object.
fn generated_arguments() -> Value {
    Value::Object(vec![
        ("suite".to_owned(), Value::Text("crates/loom".to_owned())),
        ("jobs".to_owned(), Value::Number("4".to_owned())),
    ])
}

#[test]
fn arguments_for_selected_action_only() {
    let frontier = frontier();
    let catalogue = project(&frontier, CatalogueId(uuid(0xc00)), TurnId(uuid(0x700)));
    let entries = catalogue.data().entries.clone();
    let actions: Vec<&str> = entries.iter().map(|entry| entry.action.as_str()).collect();
    assert_eq!(
        actions,
        [INSPECT, TESTS_RUN, MERGE],
        "the catalogue lists three entries, in frontier order"
    );
    let selected = entries[1].clone();
    let others = [&entries[0], &entries[2]];

    let selector = PicksSecond::default();
    let selector_handed = Rc::clone(&selector.handed);
    let generator_handed = Rc::new(RefCell::new(Vec::new()));
    let generator = Recording {
        handed: Rc::clone(&generator_handed),
        returns: generated_arguments(),
    };
    let loom = Loom::new(selector, generator, PROMPT);

    let outcome = loom.run(&commission(), &frontier);

    assert_eq!(
        *selector_handed.borrow(),
        vec![entries.clone()],
        "the selector is handed the whole catalogue, once"
    );

    // 1. The generator was handed exactly one catalogue entry, the selected one.
    let handed = generator_handed.borrow();
    assert_eq!(handed.len(), 1, "the generator is called once: {handed:?}");
    assert_eq!(
        handed[0].entry, selected,
        "the generator is handed the selected entry"
    );
    assert_eq!(
        handed[0].context.prompt, PROMPT,
        "the generator is told the run's prompt"
    );

    // 2. Neither of the other two entries' action ids appears in anything the generator was handed.
    let rendered = format!("{handed:?}");
    for other in others {
        assert!(
            !rendered.contains(other.action.as_str()),
            "the generator was handed {other:?}'s action id: {rendered}"
        );
    }

    // 3. The value the generator returned is the proposed action's arguments.
    let ExecutorOutcome::ProposedAction(proposal) = &outcome else {
        panic!("Loom proposes the selected action: {outcome:?}");
    };
    assert_eq!(proposal.action, TESTS_RUN);
    assert_eq!(
        proposal.arguments,
        ProposedActionArguments(generated_arguments()),
        "the generated JSON value is the ProposedActionArguments"
    );

    // 4. One synthesized ArgumentRequest was recorded, against that selection.
    let selections = loom.selections();
    assert_eq!(
        selections.len(),
        1,
        "one selection is recorded: {selections:?}"
    );
    assert_eq!(selections[0].data.action, TESTS_RUN);
    let requests = loom.argument_requests();
    assert_eq!(
        requests.len(),
        1,
        "one argument request is recorded: {requests:?}"
    );
    assert_eq!(requests[0].state, ArgumentRequestState::Requested);
    assert_eq!(
        requests[0].data.selection_id, selections[0].data.selection_id,
        "the argument request serves the selection the arguments were generated for"
    );
}
