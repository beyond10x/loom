//! Adversary pass 2 on `story:argument-generator`: the per-run selection and argument-request ids.
//!
//! `Loom::run` derives each run's selection id and argument-request id from the frontier's id and
//! the run's number (`run_id` in `crates/loom/src/lib.rs`). Its doc says the id is "a name-based
//! UUID (RFC 9562 version 8)", and that "two runs, two frontiers or two kinds get different ones";
//! the `Loom` doc says "two runs on one frontier therefore keep two selections and two requests
//! apart". The generated `Uuid` is "carried as its canonical textual rendering". These cases read
//! the ids only through the public record (`Loom::selections`, `Loom::argument_requests`).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom::selection::{Choice, SelectionContext};
use b10x_loom::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};

const CASE: &str = "CASE-A";
const INSPECT: &str = "repository.inspect";
const TESTS_RUN: &str = "tests.run";

fn commission_uuid(n: u32) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn two_actions() -> Vec<FrontierAction> {
    [INSPECT, TESTS_RUN]
        .into_iter()
        .map(|action| FrontierAction {
            action: action.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        })
        .collect()
}

fn frontier_named(id: &str) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(id.to_owned())),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: two_actions(),
    })
}

fn frontier(n: u32) -> Frontier<frontier_state::Issued> {
    frontier_named(&commission_uuid(n).0)
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

/// A selector that names the next action of a script on each call, cycling; `Sync`, so one Loom
/// can be shared between threads.
struct Script {
    actions: Vec<&'static str>,
    calls: AtomicUsize,
}

impl Script {
    fn cycling(actions: Vec<&'static str>) -> Self {
        Self {
            actions,
            calls: AtomicUsize::new(0),
        }
    }

    fn always(action: &'static str) -> Self {
        Self::cycling(vec![action])
    }
}

impl ActionSelector for Script {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(Choice {
            action: self.actions[call % self.actions.len()].to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// Every selection id and argument-request id the Loom has recorded, selections first.
fn recorded_ids<S, G>(loom: &Loom<S, G>) -> Vec<String> {
    let selections = loom.selections();
    let requests = loom.argument_requests();
    selections
        .iter()
        .map(|selection| selection.data.selection_id.0.0.clone())
        .chain(
            requests
                .iter()
                .map(|request| request.data.argument_request_id.0.0.clone()),
        )
        .collect()
}

/// Why `id` is not a canonical RFC 9562 version-8 UUID, or `None` when it is: 8-4-4-4-12
/// lowercase hex digits (§4), the version nibble `8` at octet 6 (§5.8) and the variant bits `10`
/// at octet 8 (§4.1), so the first digit of the fourth group is one of `8`, `9`, `a`, `b`.
fn not_canonical_version_8(id: &str) -> Option<String> {
    let bytes = id.as_bytes();
    if bytes.len() != 36 {
        return Some(format!("{id}: {} characters, not 36", bytes.len()));
    }
    for (position, byte) in bytes.iter().enumerate() {
        let hyphen = matches!(position, 8 | 13 | 18 | 23);
        if hyphen != (*byte == b'-') {
            return Some(format!("{id}: character {position} is {:?}", *byte as char));
        }
        if !hyphen && !matches!(byte, b'0'..=b'9' | b'a'..=b'f') {
            return Some(format!(
                "{id}: {:?} is not a lowercase hex digit",
                *byte as char
            ));
        }
    }
    if bytes[14] != b'8' {
        return Some(format!("{id}: version {:?}, not 8", bytes[14] as char));
    }
    if !matches!(bytes[19], b'8' | b'9' | b'a' | b'b') {
        return Some(format!("{id}: variant digit {:?}", bytes[19] as char));
    }
    None
}

/// Every id `Loom::run` records is a canonical version-8 UUID, over 64 frontiers and two runs
/// each (256 ids). The unit's own test checks one id, whose SHA-256 already carries `0x80` at
/// octet 6 and `0xa5` at octet 8, so it stays green when both bit writes are removed.
#[test]
fn every_recorded_id_is_a_canonical_version_8_uuid() {
    let loom = Loom::new(
        Script::cycling(vec![INSPECT, TESTS_RUN]),
        EmptyObjectArguments,
        "investigate",
    );
    for n in 0x100..0x140 {
        let frontier = frontier(n);
        for _ in 0..2 {
            let outcome = loom.run(&commission(), &frontier);
            assert!(
                matches!(outcome, ExecutorOutcome::ProposedAction(_)),
                "{outcome:?}"
            );
        }
    }

    let ids = recorded_ids(&loom);
    assert_eq!(ids.len(), 256, "128 selections and 128 requests");
    let wrong: Vec<String> = ids
        .iter()
        .filter_map(|id| not_canonical_version_8(id))
        .collect();
    assert!(
        wrong.is_empty(),
        "{} of {} ids are not canonical version-8 UUIDs: {wrong:#?}",
        wrong.len(),
        ids.len()
    );
}

/// The three parts of a run id are kept apart: frontier ids that contain the separator, or that
/// end in digits, give no two runs one id, and no id is a frontier's id. Frontier `f1` is run 0
/// and frontier `f` is run 10, so a derivation that drops the separators hashes both as
/// `selectionf10`.
#[test]
fn frontier_ids_built_from_the_separator_give_no_two_runs_one_id() {
    let loom = Loom::new(
        Script::always(TESTS_RUN),
        EmptyObjectArguments,
        "investigate",
    );
    let mut frontier_ids = vec!["f1".to_owned()];
    frontier_ids.extend((0..9).map(|n| commission_uuid(0x200 + n).0));
    frontier_ids.extend(
        [
            "f",
            "f\n0",
            "f\n0\n0",
            "",
            "\n",
            "selection",
            "argument-request",
            "argument-request\nf",
            "selection\nf\n1",
        ]
        .map(str::to_owned),
    );
    for id in &frontier_ids {
        let outcome = loom.run(&commission(), &frontier_named(id));
        assert!(proposes(&outcome, TESTS_RUN), "{id:?}: {outcome:?}");
    }

    let ids = recorded_ids(&loom);
    assert_eq!(ids.len(), 2 * frontier_ids.len(), "{ids:#?}");
    let distinct: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(distinct.len(), ids.len(), "two runs share an id: {ids:#?}");
    for id in &ids {
        assert!(!frontier_ids.contains(id), "{id} is a frontier's id");
    }
}

// Declined, wave 2026-10-04-w15 pass 2: two Looms on one frontier sharing ids is carried to
// story:interruption-recovery (kept at ga-wave-2026-10-04-w15/loom-argument-generator/scratch).

#[test]
fn concurrent_runs_on_one_loom_record_every_selection_and_request_once() {
    const THREADS: usize = 8;
    const RUNS: usize = 256;
    let loom = Loom::new(
        Script::cycling(vec![INSPECT, TESTS_RUN]),
        EmptyObjectArguments,
        "investigate",
    );
    let frontier = frontier(0x400);

    std::thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                for _ in 0..RUNS {
                    let outcome = loom.run(&commission(), &frontier);
                    assert!(
                        matches!(outcome, ExecutorOutcome::ProposedAction(_)),
                        "{outcome:?}"
                    );
                }
            });
        }
    });

    let selections = loom.selections();
    let requests = loom.argument_requests();
    assert_eq!(selections.len(), THREADS * RUNS, "selections recorded");
    assert_eq!(requests.len(), THREADS * RUNS, "argument requests recorded");
    let selection_ids: BTreeSet<_> = selections
        .iter()
        .map(|selection| selection.data.selection_id.0.0.clone())
        .collect();
    let request_ids: BTreeSet<_> = requests
        .iter()
        .map(|request| request.data.argument_request_id.0.0.clone())
        .collect();
    let served: BTreeSet<_> = requests
        .iter()
        .map(|request| request.data.selection_id.0.0.clone())
        .collect();
    assert_eq!(
        selection_ids.len(),
        THREADS * RUNS,
        "distinct selection ids"
    );
    assert_eq!(request_ids.len(), THREADS * RUNS, "distinct request ids");
    assert_eq!(
        served, selection_ids,
        "each request serves its own selection"
    );
}
