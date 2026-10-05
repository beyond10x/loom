//! Adversary pass 1 on `story:argument-generator`.
//!
//! Drives `Loom::run` and its record (`Loom::selections`, `Loom::argument_requests`) against what
//! the unit wrote about itself: the generator is handed only the selected catalogue entry, the
//! `loom.run.ArgumentRequest` is recorded before the generator is called, and `Loom::selections`
//! lists "every selection this Loom has made, in the order it made them".

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeSuspended, Frontier, FrontierAction,
    FrontierData, FrontierId, PrincipalId, SuspensionReason, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::run::{
    ArgumentRequestSnapshot, ArgumentRequestState, CatalogueEntry, CatalogueEntryStatus,
    SelectionSnapshot, SelectionState, SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CASE-A";
const INSPECT: &str = "repository.inspect";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const WRITE: &str = "repository.write";

fn commission_uuid(n: u32) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn three_actions() -> Vec<FrontierAction> {
    vec![
        listed(INSPECT, ActionStatus::Admissible, None),
        listed(TESTS_RUN, ActionStatus::Admissible, None),
        listed(MERGE, ActionStatus::ApprovalRequired, Some(WRITE)),
    ]
}

fn frontier(id: u32, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(commission_uuid(id)),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
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

fn proposes(outcome: &ExecutorOutcome, action: &str) -> bool {
    matches!(outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == action)
}

/// A selector that names the next action of a script on each call, cycling.
struct Script {
    actions: Vec<&'static str>,
    calls: Cell<usize>,
}

impl Script {
    fn always(action: &'static str) -> Self {
        Self::cycling(vec![action])
    }

    fn cycling(actions: Vec<&'static str>) -> Self {
        Self {
            actions,
            calls: Cell::new(0),
        }
    }
}

impl ActionSelector for Script {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        Ok(Choice {
            action: self.actions[call % self.actions.len()].to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// A selector that always fails with one error.
struct Fails(SelectorError);

impl ActionSelector for Fails {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Err(self.0.clone())
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// A generator that keeps every entry it is handed.
#[derive(Default, Clone)]
struct Keeps(Rc<RefCell<Vec<CatalogueEntry>>>);

impl ArgumentGenerator for Keeps {
    fn generate(
        &self,
        _context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.borrow_mut().push(entry.clone());
        Ok(Value::Object(vec![(
            "for".to_owned(),
            Value::Text(entry.action.clone()),
        )]))
    }
}

/// A generator that fails.
struct Failing;

impl ArgumentGenerator for Failing {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        Err("model endpoint down".to_owned())
    }
}

/// A generator that panics on its first call and answers afterwards.
#[derive(Default)]
struct PanicsOnce(Cell<bool>);

impl ArgumentGenerator for PanicsOnce {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        assert!(self.0.replace(true), "generator crashed");
        Ok(Value::Object(Vec::new()))
    }
}

/// What the Loom's record held at the moment the generator was called.
#[derive(Debug, Clone)]
struct SeenAtGeneration {
    selections: Vec<SelectionSnapshot>,
    requests: Vec<ArgumentRequestSnapshot>,
}

/// A generator that reads the record of the Loom it belongs to while it is generating.
struct ReadsRecord {
    loom: Weak<Loom<Script, ReadsRecord>>,
    seen: Rc<RefCell<Vec<SeenAtGeneration>>>,
}

impl ArgumentGenerator for ReadsRecord {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        let loom = self.loom.upgrade().expect("the Loom outlives its run");
        self.seen.borrow_mut().push(SeenAtGeneration {
            selections: loom.selections(),
            requests: loom.argument_requests(),
        });
        Ok(Value::Object(Vec::new()))
    }
}

/// Ordering: `run()`'s doc and the contract say the request is recorded against the selection
/// *before* the generator is handed the entry. The acceptance test reads the record only after
/// `run` returns, so it cannot tell "before" from "after". Here the generator itself reads the
/// record while it runs (which also shows the record's lock is not held across the call).
#[test]
fn the_request_is_recorded_before_the_generator_is_called() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let loom = Rc::new_cyclic(|weak| {
        Loom::new(
            Script::always(TESTS_RUN),
            ReadsRecord {
                loom: weak.clone(),
                seen: Rc::clone(&seen),
            },
            "investigate",
        )
    });

    let outcome = loom.run(&commission(), &frontier(0xf01, three_actions()));

    assert!(proposes(&outcome, TESTS_RUN), "{outcome:?}");
    let after_selections = loom.selections();
    let after_requests = loom.argument_requests();
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1, "the generator is called once: {seen:?}");
    assert_eq!(
        seen[0].selections, after_selections,
        "the selection is recorded before the generator is called"
    );
    assert_eq!(
        seen[0].requests.len(),
        1,
        "the argument request is recorded before the generator is called: {:?}",
        seen[0].requests
    );
    assert_eq!(seen[0].requests, after_requests);
    assert_eq!(
        seen[0].requests[0].data.selection_id,
        after_selections[0].data.selection_id
    );
}

/// A generator that fails after the request is recorded: the run suspends with the generator's
/// message, and the request stays recorded in `Requested`, the only state `loom.run.ArgumentRequest`
/// declares (`ess/domains/run.yaml`).
#[test]
fn a_failing_generator_suspends_and_leaves_its_request_recorded() {
    let loom = Loom::new(Script::always(TESTS_RUN), Failing, "investigate");

    let outcome = loom.run(&commission(), &frontier(0xf02, three_actions()));

    assert_eq!(
        outcome,
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::ExternalAvailability(Value::Object(vec![(
                "error".to_owned(),
                Value::Text("model endpoint down".to_owned()),
            )])),
        })
    );
    let selections = loom.selections();
    let requests = loom.argument_requests();
    assert_eq!(selections.len(), 1, "{selections:?}");
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].state, ArgumentRequestState::Requested);
    assert_eq!(
        requests[0].data.selection_id,
        selections[0].data.selection_id
    );
}

/// Duplicate actions in the frontier: the catalogue lists each action once, so the generator is
/// handed exactly that one entry, with the status Commission decides, and nothing else.
#[test]
fn duplicate_frontier_entries_hand_the_generator_one_entry() {
    let actions = vec![
        listed(TESTS_RUN, ActionStatus::Admissible, None),
        listed(INSPECT, ActionStatus::Admissible, None),
        listed(TESTS_RUN, ActionStatus::Admissible, None),
        listed(INSPECT, ActionStatus::ApprovalRequired, Some(WRITE)),
    ];
    let kept = Keeps::default();
    let loom = Loom::new(Script::always(INSPECT), kept.clone(), "investigate");

    let outcome = loom.run(&commission(), &frontier(0xf03, actions));

    assert!(proposes(&outcome, INSPECT), "{outcome:?}");
    let handed = kept.0.borrow();
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert_eq!(handed[0].action, INSPECT);
    assert!(
        !format!("{handed:?}").contains(TESTS_RUN),
        "the generator saw another action: {handed:?}"
    );
    assert_eq!(loom.argument_requests().len(), 1);
}

/// An `ApprovalRequired` entry is handed as `ApprovalRequired`, and the action is proposed.
#[test]
fn an_approval_required_entry_is_handed_as_approval_required() {
    let kept = Keeps::default();
    let loom = Loom::new(Script::always(MERGE), kept.clone(), "investigate");

    let outcome = loom.run(&commission(), &frontier(0xf04, three_actions()));

    assert!(proposes(&outcome, MERGE), "{outcome:?}");
    assert_eq!(
        *kept.0.borrow(),
        vec![CatalogueEntry {
            action: MERGE.to_owned(),
            status: CatalogueEntryStatus::ApprovalRequired,
        }]
    );
    assert_eq!(loom.argument_requests().len(), 1);
}

/// A selector error: no generator call, no selection and no request recorded.
#[test]
fn a_selector_error_records_nothing_and_calls_no_generator() {
    for (error, expected) in [
        (
            SelectorError::Unavailable("selector down".to_owned()),
            ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: SuspensionReason::ExternalAvailability(Value::Object(vec![(
                    "error".to_owned(),
                    Value::Text("selector down".to_owned()),
                )])),
            }),
        ),
        (
            SelectorError::NothingAdmissible,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
        ),
    ] {
        let kept = Keeps::default();
        let loom = Loom::new(Fails(error.clone()), kept.clone(), "investigate");

        let outcome = loom.run(&commission(), &frontier(0xf05, three_actions()));

        assert_eq!(outcome, expected, "{error:?}");
        assert!(kept.0.borrow().is_empty(), "{error:?}: generator called");
        assert!(
            loom.selections().is_empty(),
            "{error:?}: selection recorded"
        );
        assert!(
            loom.argument_requests().is_empty(),
            "{error:?}: request recorded"
        );
    }
}

/// A selection outside the catalogue is refused before anything is recorded.
#[test]
fn a_selection_outside_the_catalogue_records_nothing() {
    let kept = Keeps::default();
    let loom = Loom::new(Script::always("repository.delete"), kept.clone(), "x");

    let outcome = loom.run(&commission(), &frontier(0xf06, three_actions()));

    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(kept.0.borrow().is_empty());
    assert!(loom.selections().is_empty());
    assert!(loom.argument_requests().is_empty());
}

/// Id reuse across frontiers: two runs on two frontiers keep two selections and two requests,
/// each request naming its own selection.
#[test]
fn two_frontiers_on_one_loom_keep_their_requests_apart() {
    let loom = Loom::new(
        Script::cycling(vec![INSPECT, TESTS_RUN]),
        Keeps::default(),
        "investigate",
    );

    let first = loom.run(&commission(), &frontier(0xf07, three_actions()));
    let second = loom.run(&commission(), &frontier(0xf08, three_actions()));

    assert!(proposes(&first, INSPECT), "{first:?}");
    assert!(proposes(&second, TESTS_RUN), "{second:?}");
    let selections = loom.selections();
    let requests = loom.argument_requests();
    assert_eq!(selections.len(), 2, "{selections:?}");
    assert_eq!(requests.len(), 2, "{requests:?}");
    for (selection, request) in selections.iter().zip(&requests) {
        assert_eq!(selection.state, SelectionState::Selected);
        assert_eq!(request.data.selection_id, selection.data.selection_id);
    }
    assert_ne!(
        selections[0].data.selection_id,
        selections[1].data.selection_id
    );
}

/// A second run on the same frontier is not refused: it proposes again.
#[test]
fn a_second_run_on_the_same_frontier_proposes_again() {
    let loom = Loom::new(Script::always(TESTS_RUN), Keeps::default(), "investigate");
    let frontier = frontier(0xf09, three_actions());

    let first = loom.run(&commission(), &frontier);
    let second = loom.run(&commission(), &frontier);

    assert!(proposes(&first, TESTS_RUN), "{first:?}");
    assert!(proposes(&second, TESTS_RUN), "{second:?}");
}

/// `Loom::selections` promises "every selection this Loom has made, in the order it made them".
/// A model-backed selector asked twice about one frontier (a retry after a `Suspended` run) may
/// choose differently. Both are selections Loom made, and both were proposed; the record keeps
/// one, because the selection id is the frontier id and a second `put` replaces the first.
#[test]
fn every_selection_made_on_one_frontier_is_listed() {
    let loom = Loom::new(
        Script::cycling(vec![INSPECT, TESTS_RUN]),
        Keeps::default(),
        "investigate",
    );
    let frontier = frontier(0xf0a, three_actions());

    let first = loom.run(&commission(), &frontier);
    let second = loom.run(&commission(), &frontier);

    assert!(proposes(&first, INSPECT), "{first:?}");
    assert!(proposes(&second, TESTS_RUN), "{second:?}");
    let made: Vec<String> = loom
        .selections()
        .iter()
        .map(|selection| selection.data.action.clone())
        .collect();
    assert_eq!(
        made,
        [INSPECT, TESTS_RUN],
        "every selection this Loom has made, in the order it made them"
    );
}

/// Concurrency: a generator that panics does not hold the record's lock, and the next run on the
/// same Loom records and proposes normally.
#[test]
fn a_panicking_generator_does_not_break_the_next_run() {
    let loom = Loom::new(
        Script::always(TESTS_RUN),
        PanicsOnce::default(),
        "investigate",
    );

    let crashed = catch_unwind(AssertUnwindSafe(|| {
        loom.run(&commission(), &frontier(0xf0b, three_actions()))
    }));
    assert!(crashed.is_err(), "the first run panics in the generator");

    let outcome = loom.run(&commission(), &frontier(0xf0c, three_actions()));

    assert!(proposes(&outcome, TESTS_RUN), "{outcome:?}");
    assert_eq!(loom.selections().len(), 2);
    assert_eq!(loom.argument_requests().len(), 2);
}
