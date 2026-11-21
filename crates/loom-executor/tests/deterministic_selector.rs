// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:deterministic-default-selector`: a selector that calls no model, keeps
//! only admissible candidates that are useful, takes every action whose effect class is not
//! `write` before every `write` action and then orders by action id. What it knows beyond a
//! candidate's id and status comes from the host's two ports, `ActionFactsPort` (effect class and
//! open evidence) and `StepHistory` (what was performed, what is due), never from the candidates.
//!
//! The governor half (open evidence after a record arrives and after the subject's revision moves)
//! is `crates/loom-governor/tests/action_facts.rs`. No model or network call is made.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::run::{
    ActionFacts, CatalogueEntry, CatalogueEntryStatus, SelectionStrategy,
};
use b10x_loom_executor::selection::SelectionContext;
use b10x_loom_executor::{
    ActionFactsPort, ActionSelector, CaseFacts, DeterministicSelector, EmptyObjectArguments, Loom,
    SelectorError, StepHistory,
};

const CASE: &str = "CASE-DETERMINISTIC-1";
const REVISION: i64 = 3;

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: "Take the next useful step.".to_owned(),
    }
}

fn admissible(action: &str) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status: CatalogueEntryStatus::Admissible,
    }
}

fn approval_required(action: &str) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status: CatalogueEntryStatus::ApprovalRequired,
    }
}

fn facts(action: &str, effect: Option<&str>, may_produce: &[&str], open: &[&str]) -> ActionFacts {
    ActionFacts {
        action: action.to_owned(),
        effect: effect.map(str::to_owned),
        may_produce: may_produce.iter().map(|kind| (*kind).to_owned()).collect(),
        open_evidence: open.iter().map(|kind| (*kind).to_owned()).collect(),
    }
}

/// The host's facts of a case, changed by the test between steps; it counts the calls, and
/// answers only for [`CASE`].
struct Facts {
    revision: Mutex<i64>,
    actions: Mutex<Vec<ActionFacts>>,
}

impl Facts {
    fn new(actions: Vec<ActionFacts>) -> Self {
        Self {
            revision: Mutex::new(REVISION),
            actions: Mutex::new(actions),
        }
    }

    /// Records a counted record of every kind `action` may produce: its open evidence empties.
    fn produce(&self, action: &str) {
        for entry in self.actions.lock().unwrap().iter_mut() {
            if entry.action == action {
                entry.open_evidence.clear();
            }
        }
    }
}

impl ActionFactsPort for Facts {
    fn action_facts(&self, case: &CaseId) -> Result<CaseFacts, String> {
        if case.0 != CASE {
            return Err(format!("no facts for case {}", case.0));
        }
        Ok(CaseFacts {
            case_revision: *self.revision.lock().unwrap(),
            actions: self.actions.lock().unwrap().clone(),
        })
    }
}

/// A host's step history: the actions performed at each case revision, and the actions due.
#[derive(Default)]
struct History {
    performed: Mutex<BTreeMap<i64, BTreeSet<String>>>,
    due: Mutex<BTreeSet<String>>,
}

impl History {
    fn perform(&self, action: &str, case_revision: i64) {
        self.performed
            .lock()
            .unwrap()
            .entry(case_revision)
            .or_default()
            .insert(action.to_owned());
    }

    fn set_due(&self, action: &str, due: bool) {
        let mut set = self.due.lock().unwrap();
        if due {
            set.insert(action.to_owned());
        } else {
            set.remove(action);
        }
    }
}

impl StepHistory for History {
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String> {
        Ok(self
            .performed
            .lock()
            .unwrap()
            .get(&case_revision)
            .is_some_and(|actions| actions.contains(action)))
    }

    fn due(&self, action: &str) -> Result<bool, String> {
        Ok(self.due.lock().unwrap().contains(action))
    }
}

/// A history whose store cannot answer.
struct BrokenHistory;

impl StepHistory for BrokenHistory {
    fn performed_at(&self, _action: &str, _case_revision: i64) -> Result<bool, String> {
        Err("the step history is unavailable".to_owned())
    }

    fn due(&self, _action: &str) -> Result<bool, String> {
        Err("the step history is unavailable".to_owned())
    }
}

fn chosen(selector: &impl ActionSelector, candidates: &[CatalogueEntry]) -> String {
    selector
        .select(&context(), candidates)
        .expect("a useful candidate is selected")
        .action
}

#[test]
fn reads_by_identifier_then_writes_by_identifier_across_four_steps() {
    let facts = Facts::new(vec![
        facts("write.b", Some("write"), &["kind.wb"], &["kind.wb"]),
        facts("read.b", Some("read"), &["kind.rb"], &["kind.rb"]),
        facts("write.a", Some("write"), &["kind.wa"], &["kind.wa"]),
        facts("read.a", Some("read"), &["kind.ra"], &["kind.ra"]),
    ]);
    let history = History::default();
    let selector = DeterministicSelector::new(case(), &facts, &history);
    // The candidates are offered writes first and out of order: the order taken is the selector's.
    let candidates = [
        admissible("write.b"),
        admissible("write.a"),
        admissible("read.b"),
        admissible("read.a"),
    ];
    let mut taken = Vec::new();
    for _ in 0..4 {
        let action = chosen(&selector, &candidates);
        facts.produce(&action);
        taken.push(action);
    }
    assert_eq!(taken, ["read.a", "read.b", "write.a", "write.b"]);
    assert_eq!(
        selector.select(&context(), &candidates),
        Err(SelectorError::NothingAdmissible),
        "with every kind counted nothing is useful"
    );
}

#[test]
fn an_action_without_a_declared_effect_sorts_with_the_non_write_actions() {
    let facts = Facts::new(vec![
        facts("a.write", Some("write"), &["kind.w"], &["kind.w"]),
        facts("z.undeclared", None, &["kind.z"], &["kind.z"]),
    ]);
    let history = History::default();
    let selector = DeterministicSelector::new(case(), &facts, &history);
    let candidates = [admissible("a.write"), admissible("z.undeclared")];
    assert_eq!(chosen(&selector, &candidates), "z.undeclared");
}

#[test]
fn an_admissible_action_whose_kinds_are_all_counted_is_never_selected() {
    let facts = Facts::new(vec![
        facts("a.done", Some("read"), &["kind.a"], &[]),
        facts("b.open", Some("write"), &["kind.b", "kind.c"], &["kind.c"]),
    ]);
    let history = History::default();
    let selector = DeterministicSelector::new(case(), &facts, &history);
    let candidates = [admissible("a.done"), admissible("b.open")];
    assert_eq!(chosen(&selector, &candidates), "b.open");
    assert_eq!(
        selector.select(&context(), &[admissible("a.done")]),
        Err(SelectorError::NothingAdmissible)
    );
}

#[test]
fn an_action_that_produces_nothing_is_useful_until_performed_at_the_case_revision() {
    let facts = Facts::new(vec![facts("notify", Some("write"), &[], &[])]);
    let history = History::default();
    let selector = DeterministicSelector::new(case(), &facts, &history);
    let candidates = [admissible("notify")];
    assert_eq!(chosen(&selector, &candidates), "notify");
    history.perform("notify", REVISION - 1);
    assert_eq!(
        chosen(&selector, &candidates),
        "notify",
        "performed at an earlier case revision only"
    );
    history.perform("notify", REVISION);
    assert_eq!(
        selector.select(&context(), &candidates),
        Err(SelectorError::NothingAdmissible)
    );
    *facts.revision.lock().unwrap() = REVISION + 1;
    assert_eq!(
        chosen(&selector, &candidates),
        "notify",
        "useful again once the case revision moved"
    );
}

#[test]
fn an_action_with_no_open_evidence_is_useful_while_due() {
    let facts = Facts::new(vec![
        facts("fetch", Some("read"), &["kind.f"], &[]),
        facts("notify", Some("write"), &[], &[]),
    ]);
    let history = History::default();
    history.perform("notify", REVISION);
    let selector = DeterministicSelector::new(case(), &facts, &history);
    let candidates = [admissible("fetch"), admissible("notify")];
    assert_eq!(
        selector.select(&context(), &candidates),
        Err(SelectorError::NothingAdmissible)
    );
    history.set_due("fetch", true);
    assert_eq!(chosen(&selector, &candidates), "fetch");
    history.set_due("fetch", false);
    history.set_due("notify", true);
    assert_eq!(chosen(&selector, &candidates), "notify");
    history.set_due("notify", false);
    assert_eq!(
        selector.select(&context(), &candidates),
        Err(SelectorError::NothingAdmissible)
    );
}

#[test]
fn an_approval_required_candidate_is_never_selected() {
    let facts = Facts::new(vec![
        facts("a.publish", Some("read"), &["kind.p"], &["kind.p"]),
        facts("b.inspect", Some("write"), &["kind.i"], &["kind.i"]),
    ]);
    let history = History::default();
    history.set_due("a.publish", true);
    let selector = DeterministicSelector::new(case(), &facts, &history);
    assert_eq!(
        chosen(
            &selector,
            &[approval_required("a.publish"), admissible("b.inspect")]
        ),
        "b.inspect"
    );
    assert_eq!(
        selector.select(&context(), &[approval_required("a.publish")]),
        Err(SelectorError::NothingAdmissible)
    );
}

#[test]
fn an_action_the_facts_name_outside_the_candidates_is_never_returned() {
    let facts = Facts::new(vec![
        facts("a.elsewhere", Some("read"), &["kind.e"], &["kind.e"]),
        facts("z.offered", Some("write"), &["kind.o"], &["kind.o"]),
    ]);
    let history = History::default();
    history.set_due("a.elsewhere", true);
    let selector = DeterministicSelector::new(case(), &facts, &history);
    assert_eq!(chosen(&selector, &[admissible("z.offered")]), "z.offered");
    assert_eq!(
        selector.select(&context(), &[]),
        Err(SelectorError::NothingAdmissible)
    );
}

#[test]
fn a_candidate_the_facts_do_not_name_is_never_selected() {
    let facts = Facts::new(vec![facts(
        "b.known",
        Some("write"),
        &["kind.k"],
        &["kind.k"],
    )]);
    let history = History::default();
    history.set_due("a.unknown", true);
    let selector = DeterministicSelector::new(case(), &facts, &history);
    assert_eq!(
        chosen(&selector, &[admissible("a.unknown"), admissible("b.known")]),
        "b.known"
    );
}

#[test]
fn a_port_that_cannot_answer_makes_the_selector_unavailable() {
    let facts = Facts::new(vec![facts("notify", Some("write"), &[], &[])]);
    let selector = DeterministicSelector::new(case(), &facts, BrokenHistory);
    assert!(matches!(
        selector.select(&context(), &[admissible("notify")]),
        Err(SelectorError::Unavailable(_))
    ));
    let history = History::default();
    let other = DeterministicSelector::new(CaseId("CASE-OTHER".to_owned()), &facts, &history);
    assert!(matches!(
        other.select(&context(), &[admissible("notify")]),
        Err(SelectorError::Unavailable(_))
    ));
}

#[test]
fn the_same_inputs_give_the_same_choice_on_a_hundred_calls() {
    let facts = Facts::new(vec![
        facts("w.one", Some("write"), &["kind.1"], &["kind.1"]),
        facts("r.two", Some("read"), &["kind.2"], &["kind.2"]),
        facts("r.one", None, &[], &[]),
        facts("x.due", Some("write"), &["kind.3"], &[]),
    ]);
    let history = History::default();
    history.set_due("x.due", true);
    let selector = DeterministicSelector::new(case(), &facts, &history);
    let candidates = [
        admissible("w.one"),
        admissible("x.due"),
        approval_required("a.held"),
        admissible("r.two"),
        admissible("r.one"),
    ];
    let first = selector.select(&context(), &candidates);
    for _ in 0..100 {
        assert_eq!(selector.select(&context(), &candidates), first);
    }
    assert_eq!(first.map(|choice| choice.action).as_deref(), Ok("r.one"));
    assert_eq!(selector.strategy(), SelectionStrategy::Deterministic);
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xd1)),
        agent_revision_id: AgentRevisionId(uuid(0xd2)),
        case_id: case(),
        principal: PrincipalId("principal-deterministic".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier(actions: &[&str]) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case(),
        case_revision: REVISION,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: actions
            .iter()
            .map(|action| FrontierAction {
                action: (*action).to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            })
            .collect(),
    })
}

#[test]
fn the_selection_record_names_deterministic_no_tokens_and_no_confidence() {
    let facts = Facts::new(vec![
        facts("logs.search", Some("read"), &["kind.logs"], &["kind.logs"]),
        facts(
            "metrics.inspect",
            Some("read"),
            &["kind.metrics"],
            &["kind.metrics"],
        ),
    ]);
    let history = History::default();
    let loom = Loom::new(
        DeterministicSelector::new(case(), &facts, &history),
        EmptyObjectArguments,
        "Find out why checkout latency doubled.",
    );
    let outcome = loom.run(
        &commission(),
        &frontier(&["metrics.inspect", "logs.search"]),
    );
    let ExecutorOutcome::ProposedAction(proposed) = outcome else {
        panic!("the run proposes its selection: {outcome:?}");
    };
    assert_eq!(proposed.action, "logs.search");
    let selections = loom.selections();
    assert_eq!(selections.len(), 1);
    assert_eq!(
        selections[0].data.strategy,
        SelectionStrategy::Deterministic
    );
    assert_eq!(selections[0].data.confidence, None);
    let records = loom.selection_records();
    assert_eq!(records.len(), 1);
    let record = &records[0].data;
    assert_eq!(record.strategy, SelectionStrategy::Deterministic);
    assert_eq!(record.chosen_action, "logs.search");
    assert_eq!(record.confidence, None);
    assert_eq!(record.fell_back_to, None);
    assert_eq!((record.input_tokens, record.output_tokens), (0, 0));
    assert_eq!(record.candidate_count, 2);
}
