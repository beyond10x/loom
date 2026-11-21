// SPDX-License-Identifier: Apache-2.0

//! Adversary, wave 2026-11-21-w1, unit 3 (`story:deterministic-default-selector`), the executor
//! half: the `DeterministicSelector` against ports that answer badly, and against a run whose case
//! is not the one the selector was built for. No model or network call is made.

use std::sync::Mutex;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::run::{ActionFacts, CatalogueEntry, CatalogueEntryStatus};
use b10x_loom_executor::selection::SelectionContext;
use b10x_loom_executor::{
    ActionFactsPort, ActionSelector, CaseFacts, DeterministicSelector, EmptyObjectArguments, Loom,
    SelectorError, StepHistory,
};

fn context() -> SelectionContext {
    SelectionContext {
        prompt: "Take the next useful step.".to_owned(),
    }
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
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

/// A fixed seed pseudo-random sequence (xorshift64*), so every run draws the same cases.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Facts the test sets per draw, answered for any case.
struct AnyFacts(Mutex<CaseFacts>);

impl ActionFactsPort for AnyFacts {
    fn action_facts(&self, _case: &CaseId) -> Result<CaseFacts, String> {
        Ok(self.0.lock().unwrap().clone())
    }
}

/// A history that answers differently on every call, from its own sequence.
struct ErraticHistory(Mutex<Draw>);

impl StepHistory for ErraticHistory {
    fn performed_at(&self, _action: &str, _case_revision: i64) -> Result<bool, String> {
        Ok(self.0.lock().unwrap().below(2) == 0)
    }

    fn due(&self, _action: &str) -> Result<bool, String> {
        Ok(self.0.lock().unwrap().below(3) == 0)
    }
}

const NAMES: [&str; 6] = ["a.read", "B.write", "c", "a.read2", "zz", "\u{e9}t\u{e9}"];

/// Safety, whatever the ports answer: facts naming unknown actions, duplicates, open evidence
/// outside `may_produce`, an empty list, and a history that changes its answer on every call.
/// Every pick is an `Admissible` candidate; a pick is never an `ApprovalRequired` one or a name
/// only the facts carry.
#[test]
fn adversary_whatever_the_ports_answer_the_pick_is_an_admissible_candidate() {
    let mut draw = Draw(0x5eed_2026_1121_0003);
    let port = AnyFacts(Mutex::new(CaseFacts {
        case_revision: 0,
        actions: Vec::new(),
    }));
    let history = ErraticHistory(Mutex::new(Draw(0x0bad_cafe)));
    let selector = DeterministicSelector::new(CaseId("CASE-ADVERSARY".to_owned()), &port, &history);
    for _ in 0..2000 {
        let candidates: Vec<CatalogueEntry> = (0..draw.below(5))
            .map(|_| {
                let name = NAMES[draw.below(NAMES.len() as u64) as usize];
                let status = if draw.below(3) == 0 {
                    CatalogueEntryStatus::ApprovalRequired
                } else {
                    CatalogueEntryStatus::Admissible
                };
                entry(name, status)
            })
            .collect();
        let named: Vec<ActionFacts> = (0..draw.below(8))
            .map(|_| {
                let name = NAMES[draw.below(NAMES.len() as u64) as usize];
                let effect =
                    [None, Some("write"), Some("read"), Some("Write")][draw.below(4) as usize];
                let may: &[&str] = [&[][..], &["k"][..], &["k", "j"][..]][draw.below(3) as usize];
                let open: &[&str] = [&[][..], &["k"][..], &["x"][..]][draw.below(3) as usize];
                facts(name, effect, may, open)
            })
            .collect();
        *port.0.lock().unwrap() = CaseFacts {
            case_revision: draw.below(4) as i64,
            actions: named,
        };
        match selector.select(&context(), &candidates) {
            Ok(choice) => assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.action == choice.action
                        && candidate.status == CatalogueEntryStatus::Admissible),
                "{choice:?} is not an admissible candidate of {candidates:?}"
            ),
            Err(error) => assert_eq!(error, SelectorError::NothingAdmissible),
        }
    }
}

/// An action listed twice in the facts is read at its first entry, whichever entry is useful,
/// and identifier order is byte order: `B.write` (0x42) sorts before `a.read` (0x61) when both are
/// writes.
#[test]
fn adversary_duplicate_facts_read_the_first_entry_and_ids_order_by_bytes() {
    let port = AnyFacts(Mutex::new(CaseFacts {
        case_revision: 1,
        actions: vec![
            facts("a.read", Some("write"), &["k"], &["k"]),
            facts("B.write", Some("write"), &["k"], &["k"]),
            facts("c", Some("read"), &["k"], &[]),
            facts("c", Some("read"), &["k"], &["k"]),
        ],
    }));
    let selector = DeterministicSelector::new(CaseId("CASE-ADVERSARY".to_owned()), &port, Never);
    let candidates = [
        entry("c", CatalogueEntryStatus::Admissible),
        entry("a.read", CatalogueEntryStatus::Admissible),
        entry("B.write", CatalogueEntryStatus::Admissible),
    ];
    assert_eq!(
        selector.select(&context(), &candidates).map(|c| c.action),
        Ok("B.write".to_owned())
    );
}

/// A facts port that remembers which cases it was asked about and answers for each.
struct Asked {
    cases: Mutex<Vec<String>>,
}

impl ActionFactsPort for Asked {
    fn action_facts(&self, case: &CaseId) -> Result<CaseFacts, String> {
        self.cases.lock().unwrap().push(case.0.clone());
        // In CASE-A `a.done` is done and `b.open` is open; any other case has not started.
        let actions = if case.0 == "CASE-A" {
            vec![
                facts("a.done", Some("read"), &["kind.a"], &[]),
                facts("b.open", Some("read"), &["kind.b"], &["kind.b"]),
            ]
        } else {
            vec![
                facts("a.done", Some("read"), &["kind.a"], &["kind.a"]),
                facts("b.open", Some("read"), &["kind.b"], &["kind.b"]),
            ]
        };
        Ok(CaseFacts {
            case_revision: 1,
            actions,
        })
    }
}

struct Never;

impl StepHistory for Never {
    fn performed_at(&self, _: &str, _: i64) -> Result<bool, String> {
        Ok(false)
    }
    fn due(&self, _: &str) -> Result<bool, String> {
        Ok(false)
    }
}

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// A run of a commission for CASE-B, by a Loom whose deterministic selector was built for
/// CASE-A: the selector reads CASE-A's facts and picks for CASE-B from them. Nothing in the
/// selection path compares the case the facts are for with the frontier's case, and
/// `CaseFacts` names no case to compare. The run should either ask the facts of CASE-B or refuse;
/// it proposes `b.open`, skipping `a.done`, which CASE-B still needs.
#[test]
fn adversary_a_run_for_another_case_does_not_pick_from_the_selectors_case() {
    let asked = Asked {
        cases: Mutex::new(Vec::new()),
    };
    let loom = Loom::new(
        DeterministicSelector::new(CaseId("CASE-A".to_owned()), &asked, Never),
        EmptyObjectArguments,
        "Take the next useful step.",
    );
    let case_b = CaseId("CASE-B".to_owned());
    let commission: Commission<commission_state::Assigned> = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xb1)),
        agent_revision_id: AgentRevisionId(uuid(0xb2)),
        case_id: case_b.clone(),
        principal: PrincipalId("principal-b".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    });
    let frontier: Frontier<frontier_state::Issued> = Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xbf)),
        case_id: case_b,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: ["a.done", "b.open"]
            .iter()
            .map(|action| FrontierAction {
                action: (*action).to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            })
            .collect(),
    });
    let outcome = loom.run(&commission, &frontier);
    let asked_cases = asked.cases.lock().unwrap().clone();
    let proposed = match &outcome {
        ExecutorOutcome::ProposedAction(proposed) => Some(proposed.action.clone()),
        _ => None,
    };
    assert!(
        asked_cases.iter().all(|case| case == "CASE-B"),
        "the facts were read for {asked_cases:?} while the run is for CASE-B; it proposed {proposed:?}"
    );
}
