// SPDX-License-Identifier: Apache-2.0

//! A governed run that a model-loop budget stops reports that budget as its stop reason: the
//! loop's `LoopStop` reaches the executor's outcome as a `LoopBudget` suspension carrying it, never
//! `NoUsefulAction`.
//!
//! The model is scripted in this process: its one turn calls a catalogue action and reports 150
//! input tokens against a ceiling of 100, so the loop stops before the call is put to the pipeline.
//! A workflow's judgement step no longer runs this loop (`story:judgement-step-any-executor`): its
//! chooser's input-token budget suspends the run with the same `LoopBudget` stop, in
//! `tests/workflow_executor.rs`
//! (`a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it`).

use std::collections::VecDeque;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeSuspended, Frontier, FrontierAction,
    FrontierData, FrontierId, PrincipalId, SuspensionReason, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::harness::governed::{LoopExecutor, LoopPorts};
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    Budget, LoopConfig, LoopStop, LoopStopMaxInputTokens, NullLoopSink,
};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    Usage, WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, SessionData, SessionId};
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::json;

const CASE: &str = "CASE-BUDGET";
const MODEL: &str = "scripted-model";
const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf01)),
        case_id: CaseId(CASE.to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

fn session() -> SessionData {
    SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000b012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000b0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    }
}

/// A run whose input tokens may not reach 100.
fn config() -> LoopConfig {
    LoopConfig::new(MODEL, "INSTRUCTIONS").with_budget(Budget {
        max_input_tokens: Some(100),
        ..Budget::default()
    })
}

/// A model answering its scripted turns in order, counting the turns it was asked.
struct Scripted {
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    asked: usize,
}

impl Scripted {
    fn new() -> Self {
        Self {
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: vec![over_budget()].into(),
            asked: 0,
        }
    }
}

impl ModelPort for Scripted {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked += 1;
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

/// A turn that calls the edit and reports 150 input tokens.
fn over_budget() -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(ToolCall {
            call_id: CallId::new("call_edit").expect("valid"),
            name: ToolName::new("implementation_edit").expect("valid"),
            arguments: json!({"path": "src/lib.rs"}),
        })],
        usage: Some(Usage {
            model: MODEL.to_owned(),
            input_tokens: 150,
            output_tokens: 10,
            cached_input_tokens: 0,
            cache_creation_input_tokens: None,
        }),
    }
}

/// The suspension the acceptance names: the loop's `max-input-tokens` stop, limit 100, reported
/// 150, as `loom.run.LoopStop`'s wire writes it.
fn budget_suspension() -> ExecutorOutcome {
    ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: SuspensionReason::LoopBudget(CommissionValue::Object(vec![
            (
                "kind".to_owned(),
                CommissionValue::Text("max-input-tokens".to_owned()),
            ),
            (
                "limit".to_owned(),
                CommissionValue::Number("100".to_owned()),
            ),
            (
                "reported".to_owned(),
                CommissionValue::Number("150".to_owned()),
            ),
        ])),
    })
}

#[test]
fn a_governed_run_stopped_by_its_input_token_budget_reports_the_budget() {
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "change the code",
    );
    let mut model = Scripted::new();

    let run = loom.run_loop(
        &session(),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut NullLoopSink,
        },
        &commission(),
        &frontier(),
    );

    let stopped = run
        .run
        .expect("the loop ran")
        .expect("the loop stopped without an error");
    assert_eq!(
        stopped.stop,
        LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
            limit: 100,
            reported: 150,
        }),
        "the loop itself stopped on the budget"
    );
    assert_eq!(
        run.outcome,
        budget_suspension(),
        "the budget stop reaches the executor's outcome, not NoUsefulAction"
    );
    assert_eq!(model.asked, 1);
    assert_eq!(
        loom.selections(),
        [],
        "the over-budget call was never selected"
    );
}

#[test]
fn a_loop_executor_stopped_by_its_input_token_budget_reports_the_budget() {
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "change the code",
    );
    let executor = LoopExecutor::new(&loom, Scripted::new(), config(), session());

    assert_eq!(
        executor.run(&commission(), &frontier()),
        budget_suspension(),
        "Commission's executor port returns the budget stop as the run's suspension"
    );
}
