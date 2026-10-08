// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 2, wave 2026-10-07-w3, `story:moved-case-outcome`: the correction of pass 1's F2
//! (a5b2e2e), `LoopExecutor` returning `CaseMoved` after a `stale-revision` refusal.
//!
//! The correction keys on the refusal: `LoopExecutor::run` (`harness/governed.rs`) answers
//! `CaseMoved` only when the run ends with `CompletedLocalReasoning` or `NoUsefulAction` and the
//! pipeline refused one of its selections `stale-revision`. But the run loop does not select from
//! the frontier it was handed: every turn's catalogue is projected from the governor's current
//! frontier (module docs of `harness::governed`, "The tool list"). So a turn can be offered the
//! catalogue of a revision the Run does not hold without any refusal, and a call from it is
//! admitted at that revision and proposed to a Run bound to the one it left.
//!
//! The scenario is the flipped suites' own (`adversary_w1_runtime_stale.rs`): the case at revision 7
//! lists one approval-gated action, `repository.edit`, with `tests-pass` open; it moves to revision
//! 8, complete, or open with `review-approved`. The decided outcomes are `Completed` and evidence
//! asked for `review-approved`; every case first runs the pipeline Loom (`Loom::run`) on the same
//! scenario as the reference that reaches them.
//!
//! Two windows are attacked:
//!
//! - the case moves after Commission read its frontier and before the loop's first turn (a run that
//!   starts already stale), and the model answers in prose, or calls the action from the catalogue
//!   it is offered;
//! - the case moves during the model's first turn, whose call is refused `stale-revision`, and the
//!   model, told to "choose from the tools of the next turn", calls the action again from that
//!   turn's catalogue.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::harness::governed::LoopExecutor;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{Budget, LoopConfig};
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

const CASE: &str = "CASE-ADV2-W3-LOOP";
const EDIT: &str = "repository.edit";
const WRITE: &str = "repository.write";
/// The revision the Run starts at and the frontier the executor is handed.
const LEFT: i64 = 7;
/// The revision the case moves to.
const CURRENT: i64 = 8;

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

fn gated_edit() -> FrontierAction {
    FrontierAction {
        action: EDIT.to_owned(),
        status: ActionStatus::ApprovalRequired,
        capability: Some(WRITE.to_owned()),
        reasons: Vec::new(),
    }
}

/// When somebody else moves the case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moves {
    /// Right after the governor answered its first frontier read: Commission's read of the
    /// iteration, before Loom reads the case.
    AfterFirstFrontierRead,
    /// While the model takes its first turn.
    DuringFirstTurn,
    /// While the pipeline Loom's generator writes arguments (the flipped suites' window).
    DuringArguments,
}

/// The case at [`LEFT`] with `tests-pass` open and the gated edit listed, until it moves; then at
/// [`CURRENT`], complete with `done` when `completes`, its frontier holding `after` and
/// `after_actions`.
struct Case {
    moves: Moves,
    moved: AtomicBool,
    frontier_reads: AtomicUsize,
    completes: bool,
    after: Vec<FrontierObligation>,
    after_actions: Vec<FrontierAction>,
}

impl Case {
    fn new(
        moves: Moves,
        completes: bool,
        after: Vec<FrontierObligation>,
        after_actions: Vec<FrontierAction>,
    ) -> Self {
        Self {
            moves,
            moved: AtomicBool::new(false),
            frontier_reads: AtomicUsize::new(0),
            completes,
            after,
            after_actions,
        }
    }

    /// Complete at [`CURRENT`], whose frontier lists nothing, as a completed case's does.
    fn completing(moves: Moves) -> Self {
        Self::new(moves, true, Vec::new(), Vec::new())
    }

    /// Open at [`CURRENT`] with `review-approved` open and the gated edit still listed.
    fn moving_on(moves: Moves) -> Self {
        Self::new(
            moves,
            false,
            vec![open("review-approved")],
            vec![gated_edit()],
        )
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
        Ok(if self.has_moved() { CURRENT } else { LEFT })
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let moved = self.has_moved();
        let frontier = Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(if moved { 0xf8 } else { 0xf7 })),
            case_id: case.clone(),
            case_revision: if moved { CURRENT } else { LEFT },
            claims: Vec::new(),
            obligations: if moved {
                self.after.clone()
            } else {
                vec![open("tests-pass")]
            },
            actions: if moved {
                self.after_actions.clone()
            } else {
                vec![gated_edit()]
            },
        });
        let reads = self.frontier_reads.fetch_add(1, Ordering::SeqCst) + 1;
        if self.moves == Moves::AfterFirstFrontierRead && reads == 1 {
            self.move_on();
        }
        Ok(frontier)
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

/// Generates the empty object; the case moves meanwhile when it [`Moves::DuringArguments`].
struct Arguments<'c>(&'c Case);

impl ArgumentGenerator for Arguments<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        if self.0.moves == Moves::DuringArguments {
            self.0.move_on();
        }
        Ok(CommissionValue::Object(Vec::new()))
    }
}

fn calls_edit(id: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(ToolCall {
            call_id: CallId::new(id).expect("valid"),
            name: ToolName::new("repository_edit").expect("valid"),
            arguments: json!({}),
        })],
        usage: None,
    }
}

fn prose() -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant("NOTHING-MORE")],
        usage: None,
    }
}

/// The run loop's model: its turns, in order; the case moves during the first when it
/// [`Moves::DuringFirstTurn`]. Past the script it fails the turn.
struct Scripted<'c> {
    case: &'c Case,
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    asked: usize,
}

impl<'c> Scripted<'c> {
    fn new(case: &'c Case, turns: impl IntoIterator<Item = TurnOutcome>) -> Self {
        Self {
            case,
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: turns.into_iter().collect(),
            asked: 0,
        }
    }
}

impl ModelPort for Scripted<'_> {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked += 1;
        if self.asked == 1 && self.case.moves == Moves::DuringFirstTurn {
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

fn run_loop<E: AgentExecutor>(case: &Case, executor: &E) -> LoopEnd {
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
}

fn session() -> SessionData {
    SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000e012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000e0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
    }
}

/// What the run loop's Loom recorded: the revisions of the catalogues it offered, and its
/// revalidations.
struct Loop {
    end: LoopEnd,
    offered_at: Vec<i64>,
    revalidations: Vec<RevalidateSelectionOutcome>,
}

/// The pipeline Loom on a fresh `case`: the reference the decision fixed.
fn pipeline(case: &Case) -> RunOutcome {
    run_loop(
        case,
        &Loom::new(PicksEdit, Arguments(case), "edit").with_governor(case),
    )
    .outcome
}

/// `LoopExecutor` on `case`, its model answering `turns`.
fn looped(case: &Case, turns: Vec<TurnOutcome>) -> Loop {
    let loom = Loom::new(PicksEdit, Arguments(case), "edit").with_governor(case);
    let executor = LoopExecutor::new(
        &loom,
        Scripted::new(case, turns),
        LoopConfig::new("scripted-model", "INSTRUCTIONS")
            .with_retry_backoff(Duration::from_millis(1)),
        session(),
    );
    let end = run_loop(case, &executor);
    Loop {
        end,
        offered_at: loom
            .catalogues()
            .iter()
            .map(|catalogue| catalogue.data.case_revision)
            .collect(),
        revalidations: loom.revalidations(),
    }
}

fn stale_refusals(revalidations: &[RevalidateSelectionOutcome]) -> usize {
    revalidations
        .iter()
        .filter(|outcome| matches!(outcome, RevalidateSelectionOutcome::StaleRevision { .. }))
        .count()
}

fn admissions(revalidations: &[RevalidateSelectionOutcome]) -> usize {
    revalidations
        .iter()
        .filter(|outcome| matches!(outcome, RevalidateSelectionOutcome::Admitted { .. }))
        .count()
}

fn completed() -> RunOutcome {
    RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "done".to_owned(),
    })
}

fn review_approved() -> RunOutcome {
    RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
        requirements: vec!["review-approved".to_owned()],
    })
}

/// Collects a reference check, the preconditions and the measured outcome into one verdict.
fn verdict(
    reference: (&RunOutcome, &RunOutcome),
    preconditions: Vec<(bool, String)>,
    ran: &Loop,
    expected: &RunOutcome,
    why: &str,
) {
    let mut failures = Vec::new();
    if reference.0 != reference.1 {
        failures.push(format!(
            "reference, the pipeline Loom: {:?}, expected {:?}",
            reference.0, reference.1
        ));
    }
    for (held, what) in preconditions {
        if !held {
            failures.push(format!("precondition: {what}"));
        }
    }
    if ran.end.outcome != *expected {
        failures.push(format!(
            "LoopExecutor: {:?} after {} step(s), {} request(s), expected {expected:?}: {why}. \
             Catalogues offered at revisions {:?}; revalidations {:?}",
            ran.end.outcome,
            ran.end.steps,
            ran.end.requests.len(),
            ran.offered_at,
            ran.revalidations
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The case is completed after Commission read its frontier at 7 and before the loop's first
/// turn: the loop offers the catalogue of revision 8, which lists nothing, and the model answers
/// in prose. Nothing was refused, so `LoopExecutor` answers `CompletedLocalReasoning` and
/// Commission judges the run on the frontier of 7. Expected: `Completed`, as the pipeline Loom
/// ends the same scenario.
#[test]
fn adversary2_w3_a_case_completed_before_the_first_turn_ends_completed_when_the_model_answers_in_prose()
 {
    let reference = pipeline(&Case::completing(Moves::AfterFirstFrontierRead));
    let case = Case::completing(Moves::AfterFirstFrontierRead);
    let ran = looped(&case, vec![prose()]);
    verdict(
        (&reference, &completed()),
        vec![
            (
                ran.offered_at == [CURRENT],
                format!(
                    "the loop offered one catalogue, at {CURRENT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        &ran,
        &completed(),
        "the loop offered the catalogue of the revision the case moved to, and the run was \
         judged on the frontier it left",
    );
}

/// The same window, the case moved on past `tests-pass` to `review-approved`; the model answers in
/// prose. Expected: evidence asked for `review-approved`, as the pipeline Loom ends it.
#[test]
fn adversary2_w3_a_case_moved_before_the_first_turn_ends_on_its_current_obligation_when_the_model_answers_in_prose()
 {
    let reference = pipeline(&Case::moving_on(Moves::AfterFirstFrontierRead));
    let case = Case::moving_on(Moves::AfterFirstFrontierRead);
    let ran = looped(&case, vec![prose()]);
    verdict(
        (&reference, &review_approved()),
        vec![
            (
                ran.offered_at == [CURRENT],
                format!(
                    "the loop offered one catalogue, at {CURRENT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        &ran,
        &review_approved(),
        "the loop offered the catalogue of the revision the case moved to, and the run was \
         judged on the frontier it left",
    );
}

/// The same window; the model calls `repository_edit` from the catalogue of revision 8 it was
/// offered. Loom admits the selection at 8 and proposes it, Commission binds the request to the
/// Run's revision 7 and finds it stale, and the next iteration ends the run with no admissible
/// action. Expected: evidence asked for `review-approved`, as the pipeline Loom ends it.
#[test]
fn adversary2_w3_a_case_moved_before_the_first_turn_ends_on_its_current_obligation_when_the_model_calls()
 {
    let reference = pipeline(&Case::moving_on(Moves::AfterFirstFrontierRead));
    let case = Case::moving_on(Moves::AfterFirstFrontierRead);
    let ran = looped(&case, vec![calls_edit("call_edit_1")]);
    verdict(
        (&reference, &review_approved()),
        vec![
            (
                ran.offered_at == [CURRENT],
                format!(
                    "the loop offered one catalogue, at {CURRENT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                stale_refusals(&ran.revalidations) == 0 && admissions(&ran.revalidations) == 1,
                format!(
                    "one selection, admitted, none refused stale-revision: {:?}",
                    ran.revalidations
                ),
            ),
        ],
        &ran,
        &review_approved(),
        "Loom proposed a selection admitted at the revision the case moved to, to a Run that \
         holds the case at the revision it left",
    );
}

/// The case moves during the model's first turn, whose call is refused `stale-revision` and denied
/// ("choose from the tools of the next turn"). The model does that: it calls `repository_edit`
/// again from the next turn's catalogue, of revision 8. Loom admits and proposes it; Commission
/// finds the request stale, and the run ends with no admissible action. Only a run that ends
/// without a proposal is reported as `CaseMoved`. Expected: evidence asked for `review-approved`,
/// the outcome the pipeline Loom reaches on the same move, and the one `LoopExecutor` reaches when
/// the model answers in prose instead.
#[test]
fn adversary2_w3_a_stale_refusal_then_a_call_from_the_next_catalogue_ends_on_the_current_obligation()
 {
    let reference = pipeline(&Case::moving_on(Moves::DuringArguments));
    let case = Case::moving_on(Moves::DuringFirstTurn);
    let ran = looped(
        &case,
        vec![calls_edit("call_edit_1"), calls_edit("call_edit_2")],
    );
    verdict(
        (&reference, &review_approved()),
        vec![
            (
                ran.offered_at == [LEFT, CURRENT],
                format!(
                    "the loop offered catalogues at {LEFT}, then {CURRENT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                stale_refusals(&ran.revalidations) == 1 && admissions(&ran.revalidations) == 1,
                format!(
                    "one selection refused stale-revision, then one admitted: {:?}",
                    ran.revalidations
                ),
            ),
        ],
        &ran,
        &review_approved(),
        "after the stale-revision refusal the model called again from the catalogue of the \
         revision the case moved to; Loom proposed it to a Run that holds the case at the \
         revision it left",
    );
}

/// [`looped`], the loop bounded to one model turn: a run whose budget binds after that turn ends
/// with `NoUsefulAction`, and no further catalogue is projected.
fn looped_for_one_turn(case: &Case, turns: Vec<TurnOutcome>) -> Loop {
    let loom = Loom::new(PicksEdit, Arguments(case), "edit").with_governor(case);
    let executor = LoopExecutor::new(
        &loom,
        Scripted::new(case, turns),
        LoopConfig::new("scripted-model", "INSTRUCTIONS")
            .with_retry_backoff(Duration::from_millis(1))
            .with_budget(Budget {
                max_turns: Some(1),
                ..Budget::default()
            }),
        session(),
    );
    let end = run_loop(case, &executor);
    Loop {
        end,
        offered_at: loom
            .catalogues()
            .iter()
            .map(|catalogue| catalogue.data.case_revision)
            .collect(),
        revalidations: loom.revalidations(),
    }
}

/// Added in correction round 2 (`brief-moved-case-outcome-round-3.md`), with the next case: the
/// two halves of the rule `LoopExecutor::run` keys a move on. The case moves during the model's
/// only turn, whose call is refused `stale-revision`; the budget binds before the next turn, so
/// every catalogue the loop offered is at the revision the run was handed, and only the refusal
/// says the case moved. Expected: evidence asked for `review-approved`.
#[test]
fn a_stale_refusal_then_a_budget_stop_before_the_next_catalogue_ends_on_the_current_obligation() {
    let reference = pipeline(&Case::moving_on(Moves::DuringArguments));
    let case = Case::moving_on(Moves::DuringFirstTurn);
    let ran = looped_for_one_turn(&case, vec![calls_edit("call_edit_1")]);
    verdict(
        (&reference, &review_approved()),
        vec![
            (
                ran.offered_at == [LEFT],
                format!(
                    "the loop offered one catalogue, at {LEFT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                stale_refusals(&ran.revalidations) == 1 && admissions(&ran.revalidations) == 0,
                format!(
                    "one selection refused stale-revision, none admitted: {:?}",
                    ran.revalidations
                ),
            ),
        ],
        &ran,
        &review_approved(),
        "the run's only selection was refused stale-revision and the budget ended it before \
         another catalogue was offered; the run was judged on the frontier the case left",
    );
}

/// The case is completed before the loop's first turn, which is offered the empty catalogue of
/// revision 8; the model calls `repository_edit`, which that catalogue does not publish, and the
/// budget binds before the next turn. Nothing reaches the pipeline, so only the catalogue's
/// revision says the case moved, and the run ends `NoUsefulAction`, not in prose. Expected:
/// `Completed`.
#[test]
fn a_case_completed_before_the_first_turn_ends_completed_when_the_budget_stops_the_run() {
    let reference = pipeline(&Case::completing(Moves::AfterFirstFrontierRead));
    let case = Case::completing(Moves::AfterFirstFrontierRead);
    let ran = looped_for_one_turn(&case, vec![calls_edit("call_edit_1")]);
    verdict(
        (&reference, &completed()),
        vec![
            (
                ran.offered_at == [CURRENT],
                format!(
                    "the loop offered one catalogue, at {CURRENT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        &ran,
        &completed(),
        "the loop offered the catalogue of the revision the case moved to, the budget ended the \
         run with no proposal, and the run was judged on the frontier it left",
    );
}
