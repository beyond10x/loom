//! Adversary pass 1, wave 2026-10-06-w1, `story:selection-revalidation`: a Loom given a governor
//! (`Loom::with_governor`), run as the executor of Commission's `run_until_blocked`, as the SDK
//! composes them (`loom_sdk` re-exports `Loom` and `run_until_blocked` side by side).
//!
//! The case moves while Loom generates arguments (a model call, during which somebody else acts on
//! the case). Commission's runtime documents what follows a proposal made at a revision the case has
//! left (`crates/loom-commission/src/runtime.rs`, module docs, item 8): "Stale: the iteration is a
//! step, and the next iteration loads the case, which has moved: it ends the run completed if the
//! case is complete, else with no admissible action."
//!
//! A Loom without a governor proposes, Commission finds the request stale, and the run ends as
//! documented. A Loom with one refuses the selection itself and returns `NoUsefulAction`; the
//! runtime then derives the run's outcome from the frontier of the revision the case has left
//! (`outcome::derive`, rule 6) instead of loading the case again. Each case below runs both Looms
//! on the same scenario: the ungoverned half asserts the outcome Commission documents, and the
//! governed half asserts today's outcome.
//!
//! No `ExecutorOutcome` or `RunOutcome` can yet say that the case moved during the step, so the
//! runtime has nothing but the left frontier to judge the governed run on. The governed half
//! asserts that outcome until the executor port can report a moved case to Commission
//! (`decision-blocker:run-stale-outcome`), a change to Commission's specification; then each
//! governed assertion becomes the documented outcome its failure message names.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
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
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom, SelectorError};

const CASE: &str = "CASE-1";
const EDIT: &str = "repository.edit";
const WRITE: &str = "repository.write";
const REVISION: i64 = 7;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case at revision 7, listing one approval-gated action, until `moved`; then at revision 8,
/// complete with `done` when `completes`, open otherwise. Before the move, its frontier carries
/// `obligations`; after it, none.
struct Case {
    moved: AtomicBool,
    completes: bool,
    obligations: Vec<FrontierObligation>,
}

impl Case {
    fn new(completes: bool, obligations: Vec<FrontierObligation>) -> Self {
        Self {
            moved: AtomicBool::new(false),
            completes,
            obligations,
        }
    }

    fn has_moved(&self) -> bool {
        self.moved.load(Ordering::SeqCst)
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
                Vec::new()
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

/// Picks `repository.edit` whenever it is a candidate.
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

/// Generates the empty object, and the case moves meanwhile.
struct CaseMovesMeanwhile<'c>(&'c Case);

impl ArgumentGenerator for CaseMovesMeanwhile<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.0.moved.store(true, Ordering::SeqCst);
        Ok(Value::Object(Vec::new()))
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
            report: Value::Null,
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
        Timestamp("2026-10-06T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The run outcome of one loop over `case`, with `executor`.
fn run_loop<E: AgentExecutor>(case: &Case, executor: &E) -> RunOutcome {
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let end = run_until_blocked(
        case,
        executor,
        &StaticAuthorityProvider::new(),
        &Effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    end.outcome
}

/// Both Looms on the same scenario: (without a governor, with one).
fn both(completes: bool, obligations: Vec<FrontierObligation>) -> (RunOutcome, RunOutcome) {
    let plain = Case::new(completes, obligations.clone());
    let ungoverned = run_loop(
        &plain,
        &Loom::new(PicksEdit, CaseMovesMeanwhile(&plain), "edit"),
    );

    let governed_case = Case::new(completes, obligations);
    let governed = run_loop(
        &governed_case,
        &Loom::new(PicksEdit, CaseMovesMeanwhile(&governed_case), "edit")
            .with_governor(&governed_case),
    );
    (ungoverned, governed)
}

/// Somebody completes the case while Loom generates arguments. The runtime documents the run as
/// ending completed; with a governed Loom it ends with no admissible action instead, which the
/// governed half asserts until the executor port can report a moved case to Commission
/// (`decision-blocker:run-stale-outcome`).
#[test]
fn a_case_completed_while_arguments_are_generated_is_judged_on_the_left_frontier_when_governed() {
    let (ungoverned, governed) = both(true, Vec::new());
    let completed = RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "done".to_owned(),
    });

    let mut failures = Vec::new();
    if ungoverned != completed {
        failures.push(format!(
            "without a governor: {ungoverned:?}, expected {completed:?}"
        ));
    }
    let today = RunOutcome::NoAdmissibleAction(Unit(true));
    if governed != today {
        failures.push(format!(
            "with a governor: {governed:?}, expected {today:?}, the outcome reached until the \
             executor port can report a moved case to Commission \
             (decision-blocker:run-stale-outcome); then this assertion becomes {completed:?}"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Somebody moves the case while Loom generates arguments; the revision it left had an open
/// obligation. The runtime documents the run as ending with no admissible action, since the case
/// moved; with a governed Loom it ends asking for evidence for the left revision's obligation,
/// which the governed half asserts until the executor port can report a moved case to Commission
/// (`decision-blocker:run-stale-outcome`).
#[test]
fn a_case_moved_while_arguments_are_generated_is_judged_on_the_left_frontier_when_governed() {
    let (ungoverned, governed) = both(
        false,
        vec![FrontierObligation {
            obligation: "tests-pass".to_owned(),
            open: true,
        }],
    );
    let moved = RunOutcome::NoAdmissibleAction(Unit(true));

    let mut failures = Vec::new();
    if ungoverned != moved {
        failures.push(format!(
            "without a governor: {ungoverned:?}, expected {moved:?}"
        ));
    }
    let today = RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
        requirements: vec!["tests-pass".to_owned()],
    });
    if governed != today {
        failures.push(format!(
            "with a governor: {governed:?}, expected {today:?}, the outcome reached until the \
             executor port can report a moved case to Commission \
             (decision-blocker:run-stale-outcome); then this assertion becomes {moved:?}"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
