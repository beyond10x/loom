// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1, wave 2026-10-07-w3, `story:moved-case-outcome`: the decision of 2026-10-07 on
//! `decision-blocker:run-stale-outcome` (option C) through Loom's other `AgentExecutor`,
//! `harness::governed::LoopExecutor` (`Loom::run_loop`), on the scenario
//! `adversary_w1_runtime_stale.rs` flips for the pipeline Loom (`Loom::run`).
//!
//! The decision: "Loom reports its `stale-revision` refusal through that variant instead of
//! `NoUsefulAction`", and the flipped suites assert `Completed` for a case completed in the model
//! call window and the current obligation for a case moved past an open one.
//!
//! The scenario is the flipped suites' own: the case at revision 7 lists one approval-gated action,
//! `repository.edit`; it moves to revision 8, complete or open with `review-approved`, while the
//! model works. Here the model's first turn is that window: the case moves during it, and the model
//! calls `repository_edit` from the catalogue of revision 7. Loom's revalidation refuses the
//! selection `stale-revision` and the pipeline answers `CaseMoved`. `LoopExecutor` denies that to
//! the model (`governed.rs`, `Proposer::answer`), and the model then answers in prose, so Commission
//! receives `CompletedLocalReasoning` and judges the run on the frontier the case left. Each case
//! runs the pipeline Loom on the same scenario too, as the reference the decision fixed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, CompletionDeterminationComplete,
    EffectOutcome, EffectOutcomePerformed, Frontier, FrontierAction, FrontierData, FrontierId,
    FrontierObligation, GovernorError, Observation, ObservationId, PrincipalId, RunId, RunOutcome,
    RunOutcomeCompleted, RunOutcomeNeedsExternalEvidence, Unit, commission_state, frontier_state,
    observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::harness::governed::LoopExecutor;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::LoopConfig;
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueEntry, CommissionRunId, RevalidateSelectionOutcome, SelectionStrategy, SessionData,
    SessionId,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};
use serde_json::json;

const CASE: &str = "CASE-ADV-W3-LOOP";
const EDIT: &str = "repository.edit";
const WRITE: &str = "repository.write";
const REVISION: i64 = 7;

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

fn open(obligation: &str) -> FrontierObligation {
    FrontierObligation {
        obligation: obligation.to_owned(),
        open: true,
    }
}

/// The flipped suites' case: at revision 7, listing one approval-gated action, until `moved`; then
/// at revision 8, complete with `done` when `completes`, open otherwise. Before the move, its
/// frontier carries `obligations`; after it, `after`.
struct Case {
    moved: AtomicBool,
    completes: bool,
    obligations: Vec<FrontierObligation>,
    after: Vec<FrontierObligation>,
}

impl Case {
    fn new(
        completes: bool,
        obligations: Vec<FrontierObligation>,
        after: Vec<FrontierObligation>,
    ) -> Self {
        Self {
            moved: AtomicBool::new(false),
            completes,
            obligations,
            after,
        }
    }

    fn has_moved(&self) -> bool {
        self.moved.load(Ordering::SeqCst)
    }

    fn move_on(&self) {
        self.moved.store(true, Ordering::SeqCst);
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(if self.has_moved() {
            REVISION + 1
        } else {
            REVISION
        })
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let moved = self.has_moved();
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(if moved { 0xf8 } else { 0xf7 })),
            case_id: case.clone(),
            case_revision: if moved { REVISION + 1 } else { REVISION },
            claims: Vec::new(),
            obligations: if moved {
                self.after.clone()
            } else {
                self.obligations.clone()
            },
            actions: vec![FrontierAction {
                action: EDIT.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(WRITE.to_owned()),
                reasons: Vec::new(),
            }],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(if self.has_moved() && self.completes {
            CompletionDetermination::Complete(CompletionDeterminationComplete {
                outcome: "done".to_owned(),
            })
        } else {
            CompletionDetermination::Open(Unit(true))
        })
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Picks `repository.edit` whenever it is a candidate (the pipeline Loom's selector).
struct PicksEdit;

impl ActionSelector for PicksEdit {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .iter()
            .find(|entry| entry.action == EDIT)
            .map(|entry| Choice {
                action: entry.action.clone(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
}

/// Generates the empty object, and the case moves meanwhile (the pipeline Loom's window).
struct CaseMovesMeanwhile<'c>(&'c Case);

impl ArgumentGenerator for CaseMovesMeanwhile<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        self.0.move_on();
        Ok(CommissionValue::Object(Vec::new()))
    }
}

/// The model of the run loop: the case moves during its first turn, which calls
/// `repository_edit` from the catalogue it was offered; its second turn answers in prose. Past
/// that it fails the turn.
struct MovesDuringTheCall<'c> {
    case: &'c Case,
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    asked: usize,
}

impl<'c> MovesDuringTheCall<'c> {
    fn new(case: &'c Case) -> Self {
        Self {
            case,
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: VecDeque::from([
                TurnOutcome {
                    stop_reason: StopReason::ToolCalls,
                    items: vec![Item::ToolCall(ToolCall {
                        call_id: CallId::new("call_edit").expect("valid"),
                        name: ToolName::new("repository_edit").expect("valid"),
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
            asked: 0,
        }
    }
}

impl ModelPort for MovesDuringTheCall<'_> {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked += 1;
        if self.asked == 1 {
            self.case.move_on();
        }
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

/// Performs every action and changes nothing.
struct Effects;

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
        }))
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-07T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn run_loop<E: AgentExecutor>(case: &Case, executor: &E) -> RunOutcome {
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    run_until_blocked(
        case,
        executor,
        &StaticAuthorityProvider::new(),
        &Effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
    .outcome
}

fn session() -> SessionData {
    SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000d012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000d0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
    }
}

/// What one scenario ends with: through the pipeline Loom, and through `LoopExecutor`, with the
/// revalidations the loop's Loom recorded.
struct Ended {
    pipeline: RunOutcome,
    run_loop: RunOutcome,
    loop_revalidations: Vec<RevalidateSelectionOutcome>,
}

fn scenario(
    completes: bool,
    obligations: Vec<FrontierObligation>,
    after: Vec<FrontierObligation>,
) -> Ended {
    let piped = Case::new(completes, obligations.clone(), after.clone());
    let pipeline = run_loop(
        &piped,
        &Loom::new(PicksEdit, CaseMovesMeanwhile(&piped), "edit").with_governor(&piped),
    );

    let looped = Case::new(completes, obligations, after);
    let loom = Loom::new(PicksEdit, CaseMovesMeanwhile(&looped), "edit").with_governor(&looped);
    let executor = LoopExecutor::new(
        &loom,
        MovesDuringTheCall::new(&looped),
        LoopConfig::new("scripted-model", "INSTRUCTIONS")
            .with_retry_backoff(Duration::from_millis(1)),
        session(),
    );
    let run_loop = run_loop(&looped, &executor);
    Ended {
        pipeline,
        run_loop,
        loop_revalidations: loom.revalidations(),
    }
}

fn stale_revision_refused(revalidations: &[RevalidateSelectionOutcome]) -> bool {
    revalidations
        .iter()
        .any(|outcome| matches!(outcome, RevalidateSelectionOutcome::StaleRevision { .. }))
}

/// The case completes while the model works. The decided outcome, which the pipeline Loom reaches
/// (`adversary_w1_runtime_stale.rs`, `a_case_completed_while_arguments_are_generated_ends_completed_
/// when_governed`), is `Completed`. Measured through `LoopExecutor`: `NoAdmissibleAction`, judged on
/// the frontier the case left.
#[test]
fn adversary_w3_a_case_completed_during_the_model_call_ends_completed_through_the_run_loop() {
    let ended = scenario(true, Vec::new(), Vec::new());
    let completed = RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "done".to_owned(),
    });

    let mut failures = Vec::new();
    if ended.pipeline != completed {
        failures.push(format!(
            "reference, the pipeline Loom: {:?}, expected {completed:?}",
            ended.pipeline
        ));
    }
    if !stale_revision_refused(&ended.loop_revalidations) {
        failures.push(format!(
            "precondition: the loop's selection was not refused stale-revision: {:?}",
            ended.loop_revalidations
        ));
    }
    if ended.run_loop != completed {
        failures.push(format!(
            "LoopExecutor: {:?}, expected {completed:?}: Loom refused the selection \
             stale-revision, denied the CaseMoved to the model, and Commission judged the run on \
             the frontier the case left",
            ended.run_loop
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The case moves while the model works, past the open `tests-pass` of revision 7, to a revision
/// whose frontier holds `review-approved` open. The decided outcome, which the pipeline Loom reaches
/// (`adversary_w1_runtime_stale.rs`, `a_case_moved_while_arguments_are_generated_ends_on_its_
/// current_obligation_when_governed`), is evidence asked for `review-approved`. Measured through
/// `LoopExecutor`: evidence asked for the superseded `tests-pass`, the outcome the decision named
/// as the defect.
#[test]
fn adversary_w3_a_case_moved_during_the_model_call_ends_on_its_current_obligation_through_the_run_loop()
 {
    let ended = scenario(
        false,
        vec![open("tests-pass")],
        vec![open("review-approved")],
    );
    let current = RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
        requirements: vec!["review-approved".to_owned()],
    });

    let mut failures = Vec::new();
    if ended.pipeline != current {
        failures.push(format!(
            "reference, the pipeline Loom: {:?}, expected {current:?}",
            ended.pipeline
        ));
    }
    if !stale_revision_refused(&ended.loop_revalidations) {
        failures.push(format!(
            "precondition: the loop's selection was not refused stale-revision: {:?}",
            ended.loop_revalidations
        ));
    }
    if ended.run_loop != current {
        failures.push(format!(
            "LoopExecutor: {:?}, expected {current:?}: Loom refused the selection \
             stale-revision, denied the CaseMoved to the model, and Commission judged the run on \
             the frontier the case left",
            ended.run_loop
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
