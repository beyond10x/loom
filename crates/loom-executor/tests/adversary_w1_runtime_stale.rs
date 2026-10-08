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
//! documented. A Loom with one refuses the selection itself (`stale-revision`) and reports the move
//! as `ExecutorOutcome::CaseMoved` (`story:moved-case-outcome`, the decision of 2026-10-07 on
//! `decision-blocker:run-stale-outcome`, option C). The runtime then loads the case once more and
//! judges the run on the frontier current then (module docs, item 9). Until that story the
//! governed Loom returned `NoUsefulAction`, and the run was judged on the frontier the case had
//! left: no admissible action for a case that completed, and evidence asked for the superseded
//! obligation of one that moved.
//!
//! Each case below runs both Looms on the same scenario: the ungoverned half asserts the outcome
//! Commission documents for a stale proposal, and the governed half the outcome of a reported move.

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
/// `obligations`; after it, `after`.
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
fn both(
    completes: bool,
    obligations: Vec<FrontierObligation>,
    after: Vec<FrontierObligation>,
) -> (RunOutcome, RunOutcome) {
    let plain = Case::new(completes, obligations.clone(), after.clone());
    let ungoverned = run_loop(
        &plain,
        &Loom::new(PicksEdit, CaseMovesMeanwhile(&plain), "edit"),
    );

    let governed_case = Case::new(completes, obligations, after);
    let governed = run_loop(
        &governed_case,
        &Loom::new(PicksEdit, CaseMovesMeanwhile(&governed_case), "edit")
            .with_governor(&governed_case),
    );
    (ungoverned, governed)
}

fn open(obligation: &str) -> FrontierObligation {
    FrontierObligation {
        obligation: obligation.to_owned(),
        open: true,
    }
}

/// Somebody completes the case while Loom generates arguments. The runtime documents the run as
/// ending completed, and both Looms end it so: the governed one reports the move, and the runtime's
/// reload finds the case complete.
#[test]
fn a_case_completed_while_arguments_are_generated_ends_completed_when_governed() {
    let (ungoverned, governed) = both(true, Vec::new(), Vec::new());
    let completed = RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "done".to_owned(),
    });

    let mut failures = Vec::new();
    if ungoverned != completed {
        failures.push(format!(
            "without a governor: {ungoverned:?}, expected {completed:?}"
        ));
    }
    if governed != completed {
        failures.push(format!(
            "with a governor: {governed:?}, expected {completed:?}: Loom reports the move \
             (CaseMoved) and the runtime judges the run on the case as it is now"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Somebody moves the case while Loom generates arguments, past the open `tests-pass` of the
/// revision it left, to a revision whose frontier holds `review-approved` open. Without a governor
/// the request is stale and the run ends with no admissible action, as the runtime documents for a
/// stale proposal. With one, Loom reports the move and the run ends on the obligation the current
/// frontier holds, `review-approved`, not on the superseded `tests-pass`.
#[test]
fn a_case_moved_while_arguments_are_generated_ends_on_its_current_obligation_when_governed() {
    let (ungoverned, governed) = both(
        false,
        vec![open("tests-pass")],
        vec![open("review-approved")],
    );
    let stale = RunOutcome::NoAdmissibleAction(Unit(true));

    let mut failures = Vec::new();
    if ungoverned != stale {
        failures.push(format!(
            "without a governor: {ungoverned:?}, expected {stale:?}"
        ));
    }
    let current = RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
        requirements: vec!["review-approved".to_owned()],
    });
    if governed != current {
        failures.push(format!(
            "with a governor: {governed:?}, expected {current:?}: Loom reports the move \
             (CaseMoved) and the runtime judges the run on the obligation the current frontier \
             holds, not on the left revision's tests-pass"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
