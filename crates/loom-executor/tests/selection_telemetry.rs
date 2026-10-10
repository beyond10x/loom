// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:selection-telemetry`, as decided on
//! `decision-blocker:selection-telemetry-record`: one `loom.run.SelectionRecord` per
//! `loom.run.Selection`, and a per-session count of refusals at the execution boundary
//! (`loom.run.Session.boundary_refusals`). Metaharness reads both; neither is evidence (Atlas ADR
//! 0074).
//!
//! 1. Three runs of Loom as Commission's `AgentExecutor`, through a scripted selector, make three
//!    selections and write three records whose chosen actions, strategies and candidate counts
//!    equal the selections'.
//! 2. A confidence fallback records two selections (`story:fallback-selection-recording`): the
//!    fast one, `Overruled`, and the stronger selector's that replaced it. `fell_back_to` is on the
//!    record of the overruled fast selection and names the strategy of the selector that replaced
//!    it, `ReasoningModel`; the replacement's record has none.
//! 3. In a governed run (`Loom::run_loop`), the model calls an action the case's frontier stops
//!    listing during the call. Revalidation refuses the selection `not-in-frontier`; the session's
//!    `boundary_refusals` goes from 0 to 1, and the refusal adds no record: the one record is the
//!    refused selection's, written when it was made.
//!
//! The specification half of the acceptance (`ess_gate` passes) is `task ess-gate`
//! (`tests/ess_gate.rs`). No model or network call is made.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, Frontier, FrontierAction,
    FrontierData, FrontierId, GovernorError, PrincipalId, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_executor::harness::governed::LoopPorts;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{LoopConfig, VecLoopSink};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    CatalogueEntry, CommissionRunId, RevalidateSelectionOutcome, SelectionRecordSnapshot,
    SelectionSnapshot, SelectionState, SelectionStrategy, SessionData, SessionId,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, EmptyObjectArguments, HybridSelector, Loom, SelectorError,
};
use serde_json::json;

const CASE: &str = "CASE-TELEMETRY-1";
const REVISION: i64 = 7;
const PROMPT: &str = "The checkout latency doubled after the last deploy; find out why.";

const METRICS: &str = "metrics.inspect";
const LOGS: &str = "logs.search";
const RELEASE: &str = "release.inspect";
/// The frontier the case lists, in order.
const FRONTIER: [&str; 3] = [METRICS, LOGS, RELEASE];

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x7e1)),
        agent_revision_id: AgentRevisionId(uuid(0x7e2)),
        case_id: case(),
        principal: PrincipalId("principal-sre".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

/// The case at [`REVISION`], listing `actions` until `dropped`; then, at the same revision, every
/// one of them but [`LOGS`].
struct Case {
    dropped: AtomicBool,
}

impl Case {
    fn new() -> Self {
        Self {
            dropped: AtomicBool::new(false),
        }
    }

    fn drop_logs(&self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(REVISION)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let dropped = self.dropped.load(Ordering::SeqCst);
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(if dropped { 0xf2 } else { 0xf1 })),
            case_id: case.clone(),
            case_revision: REVISION,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: FRONTIER
                .iter()
                .filter(|action| !(dropped && **action == LOGS))
                .map(|action| FrontierAction {
                    action: (*action).to_owned(),
                    status: ActionStatus::Admissible,
                    capability: None,
                    reasons: Vec::new(),
                })
                .collect(),
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

/// A selector that answers with the next of its scripted choices, under one strategy.
struct Scripted {
    choices: Vec<Choice>,
    strategy: SelectionStrategy,
    asked: AtomicUsize,
}

impl Scripted {
    fn new(strategy: SelectionStrategy, choices: &[(&str, Option<&str>)]) -> Self {
        Self {
            choices: choices
                .iter()
                .map(|(action, confidence)| Choice {
                    action: (*action).to_owned(),
                    confidence: confidence.map(|text| Decimal(text.to_owned())),
                })
                .collect(),
            strategy,
            asked: AtomicUsize::new(0),
        }
    }
}

impl ActionSelector for &Scripted {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let at = self.asked.fetch_add(1, Ordering::SeqCst);
        self.choices
            .get(at)
            .cloned()
            .ok_or_else(|| SelectorError::Unavailable("the script has no further choice".into()))
    }

    fn strategy(&self) -> SelectionStrategy {
        self.strategy
    }
}

/// The record of `selection`, which must be the one record naming it.
fn record_of<'r>(
    records: &'r [SelectionRecordSnapshot],
    selection: &SelectionSnapshot,
) -> &'r SelectionRecordSnapshot {
    let named: Vec<_> = records
        .iter()
        .filter(|record| record.data.selection_id == selection.data.selection_id)
        .collect();
    assert_eq!(
        named.len(),
        1,
        "one record per selection, for {:?}: {records:?}",
        selection.data.selection_id
    );
    named[0]
}

/// (1) Three selections through the scripted selector write three records equal to them.
fn three_selections_write_three_records() {
    let governor = Case::new();
    let selector = Scripted::new(
        SelectionStrategy::FastTyped,
        &[
            (METRICS, Some("0.97")),
            (LOGS, Some("0.5")),
            (RELEASE, None),
        ],
    );
    let loom = Loom::new(&selector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    for _ in 0..3 {
        let frontier = governor.frontier(&case()).expect("the case has a frontier");
        let outcome = loom.run(&commission(), &frontier);
        assert!(
            matches!(outcome, ExecutorOutcome::ProposedAction(_)),
            "(1) each run proposes its selection: {outcome:?}"
        );
    }

    let selections = loom.selections();
    let records = loom.selection_records();
    assert_eq!(selections.len(), 3, "(1) three selections: {selections:?}");
    assert_eq!(records.len(), 3, "(1) three records: {records:?}");
    for (selection, expected) in selections.iter().zip([METRICS, LOGS, RELEASE]) {
        let record = record_of(&records, selection);
        assert_eq!(selection.data.action, expected, "(1) the selection");
        assert_eq!(
            record.data.chosen_action, selection.data.action,
            "(1) the chosen action"
        );
        assert_eq!(record.data.strategy, selection.data.strategy, "(1) strategy");
        assert_eq!(
            record.data.strategy,
            SelectionStrategy::FastTyped,
            "(1) the scripted selector's strategy"
        );
        assert_eq!(
            record.data.candidate_count,
            FRONTIER.len() as i64,
            "(1) the candidate count is the catalogue's"
        );
        assert_eq!(
            record.data.confidence, selection.data.confidence,
            "(1) the confidence"
        );
        assert_eq!(record.data.fell_back_to, None, "(1) nothing fell back");
        assert!(record.data.latency_ms >= 0, "(1) a latency: {record:?}");
        assert_eq!(
            (record.data.input_tokens, record.data.output_tokens),
            (0, 0),
            "(1) a scripted selector reports no tokens"
        );
    }
}

/// (2) A below-threshold fast selection is recorded as falling back to `ReasoningModel`.
fn a_fallback_is_recorded_on_the_overruled_selection() {
    let governor = Case::new();
    let fast = Scripted::new(SelectionStrategy::FastTyped, &[(METRICS, Some("0.42"))]);
    let stronger = Scripted::new(SelectionStrategy::ReasoningModel, &[(LOGS, None)]);
    let selector = HybridSelector::new(&fast, &stronger, &Decimal("0.9".to_owned()))
        .expect("the supplied threshold is a decimal in [0, 1]");
    let loom = Loom::new(selector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let frontier = governor.frontier(&case()).expect("the case has a frontier");
    let outcome = loom.run(&commission(), &frontier);
    assert!(
        matches!(outcome, ExecutorOutcome::ProposedAction(_)),
        "(2) the replacement is proposed: {outcome:?}"
    );

    let selections = loom.selections();
    let records = loom.selection_records();
    assert_eq!(selections.len(), 2, "(2) two selections: {selections:?}");
    assert_eq!(records.len(), 2, "(2) two records: {records:?}");
    let (overruled, replacement) = (&selections[0], &selections[1]);
    assert_eq!(overruled.state, SelectionState::Overruled, "(2) overruled");
    assert_eq!(
        overruled.data.replaced_by,
        Some(replacement.data.selection_id.clone()),
        "(2) the fast selection names its replacement"
    );

    let fast_record = record_of(&records, overruled);
    assert_eq!(fast_record.data.chosen_action, METRICS, "(2) the fast pick");
    assert_eq!(fast_record.data.strategy, SelectionStrategy::FastTyped);
    assert_eq!(
        fast_record.data.confidence,
        Some(Decimal("0.42".to_owned())),
        "(2) the fast confidence"
    );
    assert_eq!(
        fast_record.data.fell_back_to,
        Some(SelectionStrategy::ReasoningModel),
        "(2) the overruled fast selection fell back to the reasoning model"
    );

    let replacement_record = record_of(&records, replacement);
    assert_eq!(replacement_record.data.chosen_action, LOGS);
    assert_eq!(
        replacement_record.data.strategy,
        SelectionStrategy::ReasoningModel
    );
    assert_eq!(
        replacement_record.data.fell_back_to, None,
        "(2) the replacement fell back to nothing"
    );
    assert_eq!(
        (
            fast_record.data.candidate_count,
            replacement_record.data.candidate_count
        ),
        (FRONTIER.len() as i64, FRONTIER.len() as i64),
        "(2) both were offered the run's one catalogue"
    );
}

/// The governed run's model: during its first turn the case stops listing [`LOGS`], and the turn
/// calls `logs_search` from the catalogue it was offered; its second turn answers in prose.
struct DropsDuringTheCall<'c> {
    case: &'c Case,
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
}

impl<'c> DropsDuringTheCall<'c> {
    fn new(case: &'c Case) -> Self {
        Self {
            case,
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: VecDeque::from([
                TurnOutcome {
                    stop_reason: StopReason::ToolCalls,
                    items: vec![Item::ToolCall(ToolCall {
                        call_id: CallId::new("call_logs").expect("valid"),
                        name: ToolName::new("logs_search").expect("valid"),
                        arguments: json!({}),
                    })],
                    usage: None,
                },
                TurnOutcome {
                    stop_reason: StopReason::EndTurn,
                    items: vec![Item::assistant("NOTHING-MORE")],
                    usage: None,
                },
            ]),
        }
    }
}

impl ModelPort for DropsDuringTheCall<'_> {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.case.drop_logs();
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

/// (3) An out-of-frontier proposal refused at the boundary counts one refusal on the session and
/// adds no record.
fn a_boundary_refusal_is_counted_on_the_session() {
    let governor = Case::new();
    let unused = Scripted::new(SelectionStrategy::Rule, &[]);
    let loom = Loom::new(&unused, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000e012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000e0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    };
    let frontier = governor.frontier(&case()).expect("the case has a frontier");
    let mut model = DropsDuringTheCall::new(&governor);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut model,
            config: LoopConfig::new("scripted-model", "INSTRUCTIONS")
                .with_retry_backoff(Duration::from_millis(1)),
            sink: &mut sink,
        },
        &commission(),
        &frontier,
    );
    assert_eq!(
        run.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        "(3) the refused call is denied to the model, which answers in prose"
    );

    let revalidations = loom.revalidations();
    assert!(
        matches!(
            revalidations.as_slice(),
            [RevalidateSelectionOutcome::NotInFrontier { .. }]
        ),
        "(3) precondition: the selection was refused not-in-frontier: {revalidations:?}"
    );
    let sessions = loom.sessions();
    assert_eq!(sessions.len(), 1, "(3) one session: {sessions:?}");
    assert_eq!(
        sessions[0].data.boundary_refusals, 1,
        "(3) the refusal raised the session's count from 0 by one"
    );

    let selections = loom.selections();
    let records = loom.selection_records();
    assert_eq!(selections.len(), 1, "(3) one selection: {selections:?}");
    assert_eq!(selections[0].state, SelectionState::Refused);
    assert_eq!(
        records.len(),
        1,
        "(3) the refusal adds no record; the one record is the selection's: {records:?}"
    );
    let record = record_of(&records, &selections[0]);
    assert_eq!(record.data.chosen_action, LOGS);
    assert_eq!(record.data.strategy, SelectionStrategy::ReasoningModel);
}

#[test]
fn selection_telemetry_is_recorded() {
    three_selections_write_three_records();
    a_fallback_is_recorded_on_the_overruled_selection();
    a_boundary_refusal_is_counted_on_the_session();
}
