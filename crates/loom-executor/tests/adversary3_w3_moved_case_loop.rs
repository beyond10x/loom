// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 3, wave 2026-10-07-w3, `story:moved-case-outcome`: the correction of pass 2
//! (43de0b0), "`LoopExecutor` reports a move whenever its run left the handed revision".
//!
//! The rule `LoopExecutor::run` keys a move on (`harness/governed.rs`): the run ends with a
//! proposal, `CompletedLocalReasoning` or `NoUsefulAction`, and during it a selection was refused
//! `stale-revision` or a turn's catalogue was projected at another case revision than the handed
//! frontier's. Both triggers are things the loop happened to read. Neither fires when the case
//! moves while the model takes its last turn and the model answers in prose: no call reaches the
//! pipeline, so nothing is revalidated, and no further turn is taken, so no catalogue is projected
//! at the new revision. The run left the handed revision and `LoopExecutor` answers
//! `CompletedLocalReasoning`; Commission judges the run on the frontier the case left.
//!
//! The model's turn is the longest window of a run, and prose is the answer a model gives when it
//! decides not to call the one, approval-gated, tool it was offered.
//!
//! The scenario is the flipped suites' own (`adversary_w1_runtime_stale.rs`, and pass 2's
//! `adversary2_w3_moved_case_loop.rs`): the case at revision 7 lists one approval-gated action,
//! `repository.edit`, with `tests-pass` open; it moves to revision 8, complete, or open with
//! `review-approved`. The decided outcomes are `Completed` and evidence asked for
//! `review-approved`; each loop case first runs the pipeline Loom (`Loom::run`) on the case moving
//! while it works, as the reference that reaches them.
//!
//! The last case is the pipeline Loom's own form of the same window: its selector, asked while the
//! case moves, finds nothing admissible. Loom reads the governor only to revalidate a selection,
//! so a run with no selection is never reported as moved.

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
use b10x_loom_executor::harness::turn_loop::LoopConfig;
use b10x_loom_executor::harness::wire::{
    Item, ModelPort, StopReason, StreamSink, TurnOutcome, TurnRequest, WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueEntry, CommissionRunId, RevalidateSelectionOutcome, SelectionStrategy, SessionData,
    SessionId,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CASE-ADV3-W3-LOOP";
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
enum During {
    /// While the run loop's model takes its first turn.
    FirstTurn,
    /// While the pipeline Loom's selector is asked.
    Selection,
    /// While the pipeline Loom's generator writes arguments (the flipped suites' window).
    ArgumentGeneration,
}

/// The case at [`LEFT`] with `tests-pass` open and the gated edit listed, until it moves; then at
/// [`CURRENT`], complete with `done` when `completes`, its frontier holding `after` and
/// `after_actions`.
struct Case {
    moves: During,
    moved: AtomicBool,
    completes: bool,
    after: Vec<FrontierObligation>,
    after_actions: Vec<FrontierAction>,
}

impl Case {
    fn new(
        moves: During,
        completes: bool,
        after: Vec<FrontierObligation>,
        after_actions: Vec<FrontierAction>,
    ) -> Self {
        Self {
            moves,
            moved: AtomicBool::new(false),
            completes,
            after,
            after_actions,
        }
    }

    /// Complete at [`CURRENT`], whose frontier lists nothing, as a completed case's does.
    fn completing(moves: During) -> Self {
        Self::new(moves, true, Vec::new(), Vec::new())
    }

    /// Open at [`CURRENT`] with `review-approved` open and the gated edit still listed.
    fn moving_on(moves: During) -> Self {
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
        Ok(Frontier::new(FrontierData {
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

/// Picks `repository.edit` whenever it is a candidate (the reference pipeline Loom's selector).
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

/// A selector that, while it is asked, sees the case move ([`During::Selection`]) and then
/// finds nothing admissible among the candidates it was handed: a model-backed selector that
/// decides not to propose the one gated action.
struct FindsNothing<'c> {
    case: &'c Case,
    asked: AtomicUsize,
}

impl ActionSelector for FindsNothing<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        if self.case.moves == During::Selection {
            self.case.move_on();
        }
        Err(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

/// Generates the empty object; the case moves meanwhile when it [`During::ArgumentGeneration`].
struct Arguments<'c>(&'c Case);

impl ArgumentGenerator for Arguments<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        if self.0.moves == During::ArgumentGeneration {
            self.0.move_on();
        }
        Ok(CommissionValue::Object(Vec::new()))
    }
}

fn prose() -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant(
            "The only action needs approval; I will not propose it now.",
        )],
        usage: None,
    }
}

/// The run loop's model: its turns, in order; the case moves during the first when it
/// [`During::FirstTurn`]. Past the script it fails the turn.
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
        if self.asked == 1 && self.case.moves == During::FirstTurn {
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
        Timestamp("2026-10-08T12:00:00Z".to_owned())
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
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000e013".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000e0ac".to_owned())),
        wire: responses::WIRE.to_owned(),
    }
}

/// The pipeline Loom on a fresh `case` that moves while its generator writes arguments: the
/// reference the decision fixed.
fn pipeline(case: &Case) -> RunOutcome {
    run_loop(
        case,
        &Loom::new(PicksEdit, Arguments(case), "edit").with_governor(case),
    )
    .outcome
}

/// What a run recorded: its end, the revisions of the catalogues the Loom projected, and its
/// revalidations.
struct Ran {
    end: LoopEnd,
    offered_at: Vec<i64>,
    revalidations: Vec<RevalidateSelectionOutcome>,
}

/// `LoopExecutor` on `case`, its model answering `turns`.
fn looped(case: &Case, turns: Vec<TurnOutcome>) -> Ran {
    let loom = Loom::new(PicksEdit, Arguments(case), "edit").with_governor(case);
    let executor = LoopExecutor::new(
        &loom,
        Scripted::new(case, turns),
        LoopConfig::new("scripted-model", "INSTRUCTIONS")
            .with_retry_backoff(Duration::from_millis(1)),
        session(),
    );
    let end = run_loop(case, &executor);
    Ran {
        end,
        offered_at: loom
            .catalogues()
            .iter()
            .map(|catalogue| catalogue.data.case_revision)
            .collect(),
        revalidations: loom.revalidations(),
    }
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
    reference: Option<(&RunOutcome, &RunOutcome)>,
    preconditions: Vec<(bool, String)>,
    (who, ran): (&str, &Ran),
    expected: &RunOutcome,
    why: &str,
) {
    let mut failures = Vec::new();
    if let Some((measured, decided)) = reference
        && measured != decided
    {
        failures.push(format!(
            "reference, the pipeline Loom with the case moving while it writes arguments: \
             {measured:?}, expected {decided:?}"
        ));
    }
    for (held, what) in preconditions {
        if !held {
            failures.push(format!("precondition: {what}"));
        }
    }
    if ran.end.outcome != *expected {
        failures.push(format!(
            "{who}: {:?} after {} step(s), {} request(s), expected {expected:?}: {why}. \
             Catalogues projected at revisions {:?}; revalidations {:?}",
            ran.end.outcome,
            ran.end.steps,
            ran.end.requests.len(),
            ran.offered_at,
            ran.revalidations
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The case is completed while the model takes its only turn; the model answers in prose. The
/// loop projected one catalogue, at 7, before the turn, and revalidated nothing, so neither half
/// of the rule fires: `LoopExecutor` answers `CompletedLocalReasoning`, and Commission judges the
/// run on the frontier of 7. Expected: `Completed`, as the pipeline Loom ends a case completed
/// while it works.
#[test]
fn adversary3_w3_a_case_completed_during_the_models_only_turn_ends_completed_when_it_answers_in_prose()
 {
    let reference = pipeline(&Case::completing(During::ArgumentGeneration));
    let case = Case::completing(During::FirstTurn);
    let ran = looped(&case, vec![prose()]);
    verdict(
        Some((&reference, &completed())),
        vec![
            (
                case.has_moved(),
                "the case moved during the model's turn".to_owned(),
            ),
            (
                ran.offered_at == [LEFT],
                format!(
                    "the loop projected one catalogue, at {LEFT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        ("LoopExecutor", &ran),
        &completed(),
        "the case completed while the model took its only turn and the model answered in prose; \
         the run left the handed revision and LoopExecutor did not report it, so the run was \
         judged on the frontier the case left",
    );
}

/// The same window, the case moved on past `tests-pass` to `review-approved`; the model answers in
/// prose. Expected: evidence asked for `review-approved`, not for the superseded `tests-pass`, as
/// the pipeline Loom ends the same move.
#[test]
fn adversary3_w3_a_case_moved_on_during_the_models_only_turn_ends_on_its_current_obligation_when_it_answers_in_prose()
 {
    let reference = pipeline(&Case::moving_on(During::ArgumentGeneration));
    let case = Case::moving_on(During::FirstTurn);
    let ran = looped(&case, vec![prose()]);
    verdict(
        Some((&reference, &review_approved())),
        vec![
            (
                case.has_moved(),
                "the case moved during the model's turn".to_owned(),
            ),
            (
                ran.offered_at == [LEFT],
                format!(
                    "the loop projected one catalogue, at {LEFT}: {:?}",
                    ran.offered_at
                ),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        ("LoopExecutor", &ran),
        &review_approved(),
        "the case moved past tests-pass while the model took its only turn and the model \
         answered in prose; the run left the handed revision and LoopExecutor did not report it, \
         so evidence was asked for the obligation the case left",
    );
}

/// The pipeline Loom's form of the window: the case is completed while its selector is asked, and
/// the selector finds nothing admissible. Loom reads the governor only to revalidate a selection,
/// so it answers `NoUsefulAction`, and Commission judges the run on the frontier of 7. Expected:
/// `Completed`, the outcome the story names for "a case completed while the selector selects".
#[test]
fn adversary3_w3_a_case_completed_while_the_pipeline_selector_finds_nothing_ends_completed() {
    let case = Case::completing(During::Selection);
    let selector = FindsNothing {
        case: &case,
        asked: AtomicUsize::new(0),
    };
    let loom = Loom::new(selector, Arguments(&case), "edit").with_governor(&case);
    let end = run_loop(&case, &loom);
    let ran = Ran {
        end,
        offered_at: Vec::new(),
        revalidations: loom.revalidations(),
    };
    verdict(
        None,
        vec![
            (
                case.has_moved(),
                "the case moved while the selector was asked".to_owned(),
            ),
            (
                ran.revalidations.is_empty(),
                format!("nothing was revalidated: {:?}", ran.revalidations),
            ),
        ],
        ("the pipeline Loom", &ran),
        &completed(),
        "the case completed while the selector was asked and the selector found nothing \
         admissible; Loom answered NoUsefulAction without reading the case, so the run was \
         judged on the frontier the case left",
    );
}
