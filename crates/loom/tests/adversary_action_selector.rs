//! Adversary pass 1 on `story:action-selector`.
//!
//! The acceptance test refuses two absent ids, `release.publish` and `forbidden.action`, neither of
//! which is a prefix, substring or case variant of a catalogue entry, and it never looks at the
//! selection context a selector is handed. These cases close those gaps: every id that is not
//! byte-for-byte a catalogue entry is refused and named exactly as the selector returned it, the
//! selector is told the run's prompt, and a selector's error reaches the caller of `Loom::select`
//! unchanged with no arguments generated.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{
    ActionCatalogue, ActionNotInCatalogue, CatalogueEntry, CatalogueId, SelectionId,
    SelectionStrategy, TurnId, action_catalogue_state,
};
use b10x_loom::projection::project;
use b10x_loom::selection::{Choice, SelectionContext, SelectionRefusal};
use b10x_loom::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CASE-A";

const PROMPT: &str = "triage the failing build";

const INSPECT: &str = "repository.inspect";

const TESTS_RUN: &str = "tests.run";

/// Listed by the frontier as blocked, and differing from `INSPECT` only in case.
const INSPECT_UPPER_BLOCKED: &str = "Repository.Inspect";

fn commission_uuid(n: u32) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn uuid(n: u32) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn listed(action: &str, status: ActionStatus) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: None,
        reasons: Vec::new(),
    }
}

/// `INSPECT` listed twice, `TESTS_RUN` once, and a blocked case variant of `INSPECT`.
fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(commission_uuid(0xf01)),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 5,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![
            listed(INSPECT_UPPER_BLOCKED, ActionStatus::Blocked),
            listed(INSPECT, ActionStatus::Admissible),
            listed(INSPECT, ActionStatus::Admissible),
            listed(TESTS_RUN, ActionStatus::Admissible),
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
    project(&frontier(), CatalogueId(uuid(0xc01)), TurnId(uuid(0x701)))
}

/// A selector that answers one fixed result and records every context it is handed.
struct Answers {
    answer: Result<String, SelectorError>,
    contexts: Rc<RefCell<Vec<SelectionContext>>>,
    calls: Rc<Cell<usize>>,
}

impl Answers {
    fn new(answer: Result<&str, SelectorError>) -> Self {
        Self {
            answer: answer.map(str::to_owned),
            contexts: Rc::default(),
            calls: Rc::default(),
        }
    }
}

impl ActionSelector for Answers {
    fn select(
        &self,
        context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.calls.set(self.calls.get() + 1);
        self.contexts.borrow_mut().push(context.clone());
        self.answer.clone().map(|action| Choice {
            action,
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// An argument generator that counts its calls.
struct Counting(Rc<Cell<usize>>);

impl ArgumentGenerator for Counting {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.set(self.0.get() + 1);
        Ok(Value::Object(Vec::new()))
    }
}

/// Every id that is not byte-for-byte one of the catalogue's entries: the empty id, padding, case
/// variants (one of which the frontier lists as blocked), a prefix, a suffix, a substring, a
/// superstring and a homoglyph. Each is refused by `Loom::select` and named exactly as returned,
/// and `Loom::run` proposes nothing and generates nothing for it.
#[test]
fn every_id_not_exactly_a_catalogue_entry_is_refused_and_named_verbatim() {
    let catalogue = catalogue();
    let actions: Vec<&str> = catalogue
        .data()
        .entries
        .iter()
        .map(|entry| entry.action.as_str())
        .collect();
    assert_eq!(
        actions,
        [INSPECT, TESTS_RUN],
        "the catalogue lists each admitted action once, and not the blocked case variant"
    );

    let absent = [
        "",
        " ",
        " repository.inspect",
        "repository.inspect ",
        "repository.inspect\n",
        INSPECT_UPPER_BLOCKED,
        "REPOSITORY.INSPECT",
        "Tests.Run",
        "repository",
        "repository.",
        "inspect",
        ".inspect",
        "tests",
        "repository.inspect.all",
        "tests.run;repository.inspect",
        // Cyrillic 'е' (U+0435) in place of the Latin 'e' of `repository`.
        "r\u{0435}pository.inspect",
    ];
    for id in absent {
        let generated = Rc::new(Cell::new(0));
        let loom = Loom::new(
            Answers::new(Ok(id)),
            Counting(Rc::clone(&generated)),
            PROMPT,
        );
        let refused = loom
            .select(&catalogue, SelectionId(uuid(0x5e1)))
            .map(|selection| selection.into_data());
        assert_eq!(
            refused,
            Err(SelectionRefusal::NotInCatalogue(ActionNotInCatalogue {
                action: id.to_owned(),
            })),
            "{id:?} is not a catalogue entry"
        );

        let outcome = loom.run(&commission(), &frontier());
        assert_eq!(
            outcome,
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            "the executor proposes {id:?}"
        );
        assert_eq!(generated.get(), 0, "arguments generated for refused {id:?}");
    }

    // The same selector naming a listed id is accepted, so the refusals above are about the id.
    for id in [INSPECT, TESTS_RUN] {
        let generated = Rc::new(Cell::new(0));
        let loom = Loom::new(
            Answers::new(Ok(id)),
            Counting(Rc::clone(&generated)),
            PROMPT,
        );
        let selection = loom
            .select(&catalogue, SelectionId(uuid(0x5e2)))
            .unwrap_or_else(|refusal| panic!("{id} is a catalogue entry: {refusal:?}"));
        assert_eq!(selection.data().action, id);
        assert!(
            matches!(loom.run(&commission(), &frontier()), ExecutorOutcome::ProposedAction(p) if p.action == id)
        );
        assert_eq!(generated.get(), 1, "arguments generated once for {id}");
    }
}

/// The selection context a selector is handed carries the run's prompt, through `Loom::select`
/// and through `Loom::run`. A selector told nothing cannot choose for the instruction it serves.
#[test]
fn the_selector_is_told_the_prompt() {
    let selector = Answers::new(Ok(INSPECT));
    let contexts = Rc::clone(&selector.contexts);
    let loom = Loom::new(selector, Counting(Rc::default()), PROMPT);

    loom.select(&catalogue(), SelectionId(uuid(0x5e3)))
        .expect("a catalogue entry is selected");
    loom.run(&commission(), &frontier());

    let expected = SelectionContext {
        prompt: PROMPT.to_owned(),
    };
    assert_eq!(
        contexts.borrow().as_slice(),
        [expected.clone(), expected],
        "Loom::select, then Loom::run"
    );
}

/// A selector's error reaches the caller of `Loom::select` as it was returned, after one call,
/// with no arguments generated; `Loom::run` answers `Unavailable` with an outage and
/// `NothingAdmissible` with `NoUsefulAction`, still generating nothing.
#[test]
fn a_selector_error_propagates_unchanged_and_generates_nothing() {
    let catalogue = catalogue();
    for error in [
        SelectorError::Unavailable("model endpoint unreachable".to_owned()),
        SelectorError::Unavailable(String::new()),
        SelectorError::NothingAdmissible,
    ] {
        let selector = Answers::new(Err(error.clone()));
        let calls = Rc::clone(&selector.calls);
        let generated = Rc::new(Cell::new(0));
        let loom = Loom::new(selector, Counting(Rc::clone(&generated)), PROMPT);

        let refused = loom
            .select(&catalogue, SelectionId(uuid(0x5e4)))
            .map(|selection| selection.into_data());
        assert_eq!(
            refused,
            Err(SelectionRefusal::Selector(error.clone())),
            "Loom::select over {error:?}"
        );
        assert_eq!(calls.get(), 1, "the selector is asked once per selection");

        let outcome = loom.run(&commission(), &frontier());
        match &error {
            SelectorError::NothingAdmissible => assert_eq!(
                outcome,
                ExecutorOutcome::NoUsefulAction(Unit(true)),
                "Loom::run over {error:?}"
            ),
            SelectorError::Unavailable(_) => assert!(
                matches!(outcome, ExecutorOutcome::Suspended(_)),
                "Loom::run over {error:?}: {outcome:?}"
            ),
        }
        assert_eq!(calls.get(), 2, "Loom::run asks the selector once");
        assert_eq!(generated.get(), 0, "arguments generated after {error:?}");
    }
}
