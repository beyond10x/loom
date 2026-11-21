// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 2, wave 2026-11-21-w1, unit 3 (`story:deterministic-default-selector`): the
//! `resolve_for_case` path `Loom::prepare` now takes for every selector. A host selector that
//! overrides `resolve` sees it called once; a `HybridSelector` whose fallback answers for another
//! case asks no port; a host wrapper keeps the case check when it forwards `resolve_for_case` and
//! answers for the wrapped selector's own case when it forwards only `select`. No model or network
//! call is made.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::primitives::Decimal;
use b10x_loom_executor::model::run::{ActionFacts, CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, Pick, Resolution, SelectionContext};
use b10x_loom_executor::{
    ActionFactsPort, ActionSelector, CaseFacts, DeterministicSelector, EmptyObjectArguments,
    FirstAdmissibleSelector, HybridSelector, Loom, SelectorError, StepHistory,
};

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &str) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-pass2".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier(case: &str, actions: &[&str]) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xcf)),
        case_id: CaseId(case.to_owned()),
        case_revision: 1,
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

fn facts(action: &str, open: &[&str]) -> ActionFacts {
    ActionFacts {
        action: action.to_owned(),
        effect: Some("read".to_owned()),
        may_produce: vec![format!("kind.{action}")],
        open_evidence: open.iter().map(|kind| (*kind).to_owned()).collect(),
    }
}

/// A facts port that records every case it is asked about. In CASE-A `a.done` is done and
/// `b.open` is open; in any other case both are open.
#[derive(Default)]
struct Asked {
    cases: Mutex<Vec<String>>,
}

impl ActionFactsPort for Asked {
    fn action_facts(&self, case: &CaseId) -> Result<CaseFacts, String> {
        self.cases.lock().unwrap().push(case.0.clone());
        let actions = if case.0 == "CASE-A" {
            vec![facts("a.done", &[]), facts("b.open", &["kind.b.open"])]
        } else {
            vec![
                facts("a.done", &["kind.a.done"]),
                facts("b.open", &["kind.b.open"]),
            ]
        };
        Ok(CaseFacts {
            case_revision: 1,
            actions,
        })
    }
}

/// A history that records every question it is asked; nothing performed, nothing due.
#[derive(Default)]
struct Quiet {
    asked: AtomicUsize,
}

impl StepHistory for Quiet {
    fn performed_at(&self, _: &str, _: i64) -> Result<bool, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }

    fn due(&self, _: &str) -> Result<bool, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(false)
    }
}

/// A host selector that implements `resolve` itself, with a side effect, and leaves
/// `resolve_for_case` to the trait's default.
#[derive(Default)]
struct HostResolve {
    resolved: AtomicUsize,
    selected: AtomicUsize,
}

impl ActionSelector for HostResolve {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.selected.fetch_add(1, Ordering::SeqCst);
        candidates
            .first()
            .map(|entry| Choice {
                action: entry.action.clone(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }

    fn resolve(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        self.resolved.fetch_add(1, Ordering::SeqCst);
        let last = candidates.last().ok_or(SelectorError::NothingAdmissible)?;
        Ok(Resolution {
            chosen: Pick {
                choice: Choice {
                    action: last.action.clone(),
                    confidence: None,
                },
                strategy: SelectionStrategy::Rule,
                latency_ms: 0,
                input_tokens: 7,
                output_tokens: 3,
            },
            overruled: None,
        })
    }
}

/// A host selector overriding `resolve` and not `resolve_for_case`: Loom's run calls its
/// `resolve` exactly once and never its `select`, and records the pick `resolve` made, tokens
/// included, as before the correction.
#[test]
fn adversary_p2_a_host_resolve_is_called_once_through_the_default_resolve_for_case() {
    let host = HostResolve::default();
    let loom = Loom::new(&host, EmptyObjectArguments, "Take the next useful step.");
    let outcome = loom.run(
        &commission("CASE-H"),
        &frontier("CASE-H", &["a.one", "z.two"]),
    );
    let ExecutorOutcome::ProposedAction(proposed) = outcome else {
        panic!("the run proposes the host's pick: {outcome:?}");
    };
    assert_eq!(
        proposed.action, "z.two",
        "the pick `resolve` made, not `select`'s"
    );
    assert_eq!(host.resolved.load(Ordering::SeqCst), 1);
    assert_eq!(host.selected.load(Ordering::SeqCst), 0);
    let records = loom.selection_records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        (records[0].data.input_tokens, records[0].data.output_tokens),
        (7, 3)
    );
}

impl ActionSelector for &HostResolve {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        (**self).select(context, candidates)
    }

    fn strategy(&self) -> SelectionStrategy {
        (**self).strategy()
    }

    fn resolve(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        (**self).resolve(context, candidates)
    }
}

/// A `HybridSelector` whose fallback is a `DeterministicSelector` built for CASE-A, run for
/// CASE-B: the fast selector's pick is not accepted (it has no confidence), the fallback refuses
/// the case, the run is suspended naming both cases, and neither of the deterministic selector's
/// ports is asked.
#[test]
fn adversary_p2_a_hybrid_whose_fallback_is_for_another_case_asks_no_port() {
    let asked = Asked::default();
    let history = Quiet::default();
    let hybrid = HybridSelector::new(
        FirstAdmissibleSelector,
        DeterministicSelector::new(CaseId("CASE-A".to_owned()), &asked, &history),
        &Decimal("0.9".to_owned()),
    )
    .unwrap();
    let loom = Loom::new(hybrid, EmptyObjectArguments, "Take the next useful step.");
    let outcome = loom.run(
        &commission("CASE-B"),
        &frontier("CASE-B", &["a.done", "b.open"]),
    );
    let ExecutorOutcome::Suspended(suspended) = &outcome else {
        panic!("the run is suspended: {outcome:?}");
    };
    let shown = format!("{suspended:?}");
    assert!(
        shown.contains("CASE-A") && shown.contains("CASE-B"),
        "{shown}"
    );
    assert!(asked.cases.lock().unwrap().is_empty());
    assert_eq!(history.asked.load(Ordering::SeqCst), 0);
    assert!(loom.selection_records().is_empty());
}

/// A host's own selector type that holds a `DeterministicSelector` and forwards `select` and
/// `strategy` to it, as `loom-intake-slice`'s `Recording` forwards to its `ModelSelector`
/// (`crates/loom-intake-slice/src/run.rs:1378`). A host cannot implement `ActionSelector` for a
/// reference to `DeterministicSelector`, so keeping access to it means writing such a wrapper.
struct Keeps<'s> {
    inner: &'s DeterministicSelector<&'s Asked, &'s Quiet>,
    picks: AtomicUsize,
}

impl ActionSelector for Keeps<'_> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.picks.fetch_add(1, Ordering::SeqCst);
        self.inner.select(context, candidates)
    }

    fn strategy(&self) -> SelectionStrategy {
        self.inner.strategy()
    }
}

/// The same wrapper forwarding `resolve_for_case` as well, as the `ActionSelector` trait docs
/// require of a selector that wraps another.
struct Forwards<'s> {
    inner: &'s DeterministicSelector<&'s Asked, &'s Quiet>,
}

impl ActionSelector for Forwards<'_> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.inner.select(context, candidates)
    }

    fn strategy(&self) -> SelectionStrategy {
        self.inner.strategy()
    }

    fn resolve_for_case(
        &self,
        case: &CaseId,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Resolution, SelectorError> {
        self.inner.resolve_for_case(case, context, candidates)
    }
}

/// The rule the `ActionSelector` trait docs state: a wrapper that forwards `resolve_for_case`
/// refuses a run of CASE-B for a selector built for CASE-A and asks no port; a wrapper that
/// forwards only `select` answers for the wrapped selector's own case, CASE-A.
#[test]
fn adversary_p2_a_wrapper_keeps_the_case_check_only_when_it_forwards_resolve_for_case() {
    let asked = Asked::default();
    let history = Quiet::default();
    let inner = DeterministicSelector::new(CaseId("CASE-A".to_owned()), &asked, &history);

    let forwards = Loom::new(
        Forwards { inner: &inner },
        EmptyObjectArguments,
        "Take the next useful step.",
    );
    let outcome = forwards.run(
        &commission("CASE-B"),
        &frontier("CASE-B", &["a.done", "b.open"]),
    );
    let ExecutorOutcome::Suspended(suspended) = &outcome else {
        panic!("a wrapper forwarding `resolve_for_case` refuses the other case: {outcome:?}");
    };
    let shown = format!("{suspended:?}");
    assert!(
        shown.contains("CASE-A") && shown.contains("CASE-B"),
        "{shown}"
    );
    assert!(asked.cases.lock().unwrap().is_empty());
    assert_eq!(history.asked.load(Ordering::SeqCst), 0);

    let wrapper = Keeps {
        inner: &inner,
        picks: AtomicUsize::new(0),
    };
    let loom = Loom::new(wrapper, EmptyObjectArguments, "Take the next useful step.");
    let outcome = loom.run(
        &commission("CASE-B"),
        &frontier("CASE-B", &["a.done", "b.open"]),
    );
    let asked_cases = asked.cases.lock().unwrap().clone();
    assert_eq!(
        asked_cases,
        vec!["CASE-A".to_owned()],
        "a selector that wraps another and forwards only `select` answers for the wrapped \
         selector's own case (`ActionSelector` trait docs): {outcome:?}"
    );
}
