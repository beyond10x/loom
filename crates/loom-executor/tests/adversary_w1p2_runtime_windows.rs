//! Adversary pass 2, wave 2026-10-06-w1, `story:selection-revalidation`: a Loom given a governor
//! (`Loom::with_governor`) as the executor of Commission's `run_until_blocked`, outside the window
//! pass 1 measured (`adversary_w1_runtime_stale.rs`, the case moving while arguments are generated).
//!
//! `CHANGELOG.md` (Unreleased) and the site's status row (`website/data/status.json`) state the
//! limitation for the whole window between Commission reading the frontier and Loom's
//! revalidation: while the selector selects or while arguments are generated. The selection comes
//! first and is a model call as well (the slice's `ModelSelector`). The first two cases move the
//! case while the selector selects, and check that the arguments were generated after the move, so
//! the move falls in the selector's half of that window. Their governed half asserts the outcome
//! reached today, as `adversary_w1_runtime_stale.rs` does while
//! `decision-blocker:run-stale-outcome` is open.
//!
//! The last two cases are the converse: when nobody else moves the case, a governed Loom ends a
//! run of several steps, whose effects move the case, exactly as an ungoverned one does.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, CompletionDeterminationComplete, EffectOutcome,
    EffectOutcomePerformed, Frontier, FrontierAction, FrontierData, FrontierId, FrontierObligation,
    GovernorError, Observation, ObservationId, PrincipalId, RunId, RunOutcome, RunOutcomeCompleted,
    RunOutcomeNeedsAuthority, RunOutcomeNeedsExternalEvidence, Unit, commission_state,
    frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::AuthorityProvider;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, ArgumentContext, ArgumentGenerator, EmptyObjectArguments, Loom, SelectorError,
};

const CASE: &str = "CASE-1";
const EDIT: &str = "repository.edit";
const WRITE: &str = "repository.write";
const TEST: &str = "tests.run";
const MERGE: &str = "repository.merge";
const MERGE_CAPABILITY: &str = "repository.merge";
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

fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: action.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn obligation(open: bool) -> FrontierObligation {
    FrontierObligation {
        obligation: "tests-pass".to_owned(),
        open,
    }
}

// ----------------------------------------------------------------------------------------------
// The case moves while the selector selects.
// ----------------------------------------------------------------------------------------------

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
            actions: vec![listed(
                EDIT,
                ActionStatus::ApprovalRequired,
                Some("repository.write"),
            )],
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

/// Picks `repository.edit`, and the case moves while it does (somebody else acts on the case during
/// the selector's model call).
struct MovesWhileSelecting<'c>(&'c Case);

impl ActionSelector for MovesWhileSelecting<'_> {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.0.moved.store(true, Ordering::SeqCst);
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

/// Generates the empty object and notes, per call, whether the case had already moved when
/// generation began: `true` puts the move before argument generation.
struct NotesTheMove<'c> {
    case: &'c Case,
    moved_at_start: Mutex<Vec<bool>>,
}

impl<'c> NotesTheMove<'c> {
    fn new(case: &'c Case) -> Self {
        Self {
            case,
            moved_at_start: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<bool> {
        self.moved_at_start
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ArgumentGenerator for NotesTheMove<'_> {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        self.moved_at_start
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.case.has_moved());
        Ok(Value::Object(Vec::new()))
    }
}

/// Performs every action. Whether it moves a case is the governor's to say.
struct Effects<'l>(Option<&'l Ledger>);

impl EffectPort for Effects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if let Some(ledger) = self.0 {
            ledger.revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
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

/// One loop over `governor`, with `executor`, `authority` and `effects`.
fn run_loop<G, E, A>(governor: &G, executor: &E, authority: &A, effects: &Effects<'_>) -> LoopEnd
where
    G: Governor + ObservationPort,
    E: AgentExecutor,
    A: AuthorityProvider,
{
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    run_until_blocked(
        governor,
        executor,
        authority,
        effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// What one loop over a case that moves while the selector selects produced.
struct Moved {
    outcome: RunOutcome,
    /// Per argument generation: whether the case had already moved when it began.
    moved_at_generation: Vec<bool>,
}

/// The same scenario without a governor and with one.
fn moved_while_selecting(completes: bool, obligations: Vec<FrontierObligation>) -> (Moved, Moved) {
    let authority = StaticAuthorityProvider::new();

    let plain = Case::new(completes, obligations.clone());
    let plain_arguments = NotesTheMove::new(&plain);
    let ungoverned = run_loop(
        &plain,
        &Loom::new(MovesWhileSelecting(&plain), &plain_arguments, "edit"),
        &authority,
        &Effects(None),
    );

    let governed_case = Case::new(completes, obligations);
    let governed_arguments = NotesTheMove::new(&governed_case);
    let governed = run_loop(
        &governed_case,
        &Loom::new(
            MovesWhileSelecting(&governed_case),
            &governed_arguments,
            "edit",
        )
        .with_governor(&governed_case),
        &authority,
        &Effects(None),
    );
    (
        Moved {
            outcome: ungoverned.outcome,
            moved_at_generation: plain_arguments.seen(),
        },
        Moved {
            outcome: governed.outcome,
            moved_at_generation: governed_arguments.seen(),
        },
    )
}

impl ArgumentGenerator for &NotesTheMove<'_> {
    fn generate(&self, context: &ArgumentContext, entry: &CatalogueEntry) -> Result<Value, String> {
        (*self).generate(context, entry)
    }
}

/// Somebody completes the case while the selector selects; the arguments are generated after the
/// move. The runtime documents the run as ending completed, and the ungoverned run does. The
/// governed run ends with no admissible action: judged on the frontier the case left, in the
/// selector's half of the window the CHANGELOG and the status row name.
#[test]
fn a_case_completed_while_the_selector_selects_is_judged_on_the_left_frontier() {
    let (ungoverned, governed) = moved_while_selecting(true, Vec::new());
    let completed = RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "done".to_owned(),
    });
    let today = RunOutcome::NoAdmissibleAction(Unit(true));

    let mut failures = Vec::new();
    if governed.moved_at_generation != [true] {
        failures.push(format!(
            "with a governor, argument generation saw the case moved: {:?}, expected [true] \
             (the move happened while the selector selected, before the arguments)",
            governed.moved_at_generation
        ));
    }
    if ungoverned.outcome != completed {
        failures.push(format!(
            "without a governor: {:?}, expected {completed:?}",
            ungoverned.outcome
        ));
    }
    if governed.outcome != today {
        failures.push(format!(
            "with a governor: {:?}, expected {today:?}, the outcome reached while \
             decision-blocker:run-stale-outcome is open, for a move in the selector's half of \
             the window CHANGELOG.md and website/data/status.json name",
            governed.outcome
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Somebody moves the case while the selector selects; the revision it left had an open
/// obligation. The runtime documents the run as ending with no admissible action, and the
/// ungoverned run does. The governed run ends asking for evidence for the left revision's
/// obligation: judged on the frontier the case left, in the selector's half of the window the
/// CHANGELOG and the status row name.
#[test]
fn a_case_moved_while_the_selector_selects_is_judged_on_the_left_frontier() {
    let (ungoverned, governed) = moved_while_selecting(false, vec![obligation(true)]);
    let moved = RunOutcome::NoAdmissibleAction(Unit(true));
    let today = RunOutcome::NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence {
        requirements: vec!["tests-pass".to_owned()],
    });

    let mut failures = Vec::new();
    if governed.moved_at_generation != [true] {
        failures.push(format!(
            "with a governor, argument generation saw the case moved: {:?}, expected [true]",
            governed.moved_at_generation
        ));
    }
    if ungoverned.outcome != moved {
        failures.push(format!(
            "without a governor: {:?}, expected {moved:?}",
            ungoverned.outcome
        ));
    }
    if governed.outcome != today {
        failures.push(format!(
            "with a governor: {:?}, expected {today:?}, the outcome reached while \
             decision-blocker:run-stale-outcome is open, for a move in the selector's half of \
             the window CHANGELOG.md and website/data/status.json name",
            governed.outcome
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ----------------------------------------------------------------------------------------------
// Nobody else moves the case: a governor changes nothing.
// ----------------------------------------------------------------------------------------------

/// A case only its own effects move: each performed effect moves it one revision. At revision 1
/// its frontier admits `repository.write`, at 2 `tests.run`, both with `tests-pass` open; from 3
/// on it lists `repository.merge`, approval-gated, with `tests-pass` discharged. It is complete
/// from `complete_at` on, when that is set.
struct Ledger {
    revision: AtomicI64,
    complete_at: Option<i64>,
}

impl Ledger {
    fn new(complete_at: Option<i64>) -> Self {
        Self {
            revision: AtomicI64::new(1),
            complete_at,
        }
    }

    fn at(&self) -> i64 {
        self.revision.load(Ordering::SeqCst)
    }
}

impl Governor for Ledger {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.at())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.at();
        let (actions, open) = match revision {
            1 => (vec![listed(WRITE, ActionStatus::Admissible, None)], true),
            2 => (vec![listed(TEST, ActionStatus::Admissible, None)], true),
            _ => (
                vec![listed(
                    MERGE,
                    ActionStatus::ApprovalRequired,
                    Some(MERGE_CAPABILITY),
                )],
                false,
            ),
        };
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf00 + revision.unsigned_abs())),
            case_id: case.clone(),
            case_revision: revision,
            claims: Vec::new(),
            obligations: vec![obligation(open)],
            actions,
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(match self.complete_at {
            Some(at) if self.at() >= at => {
                CompletionDetermination::Complete(CompletionDeterminationComplete {
                    outcome: "merged".to_owned(),
                })
            }
            _ => CompletionDetermination::Open(Unit(true)),
        })
    }
}

impl ObservationPort for Ledger {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Picks the first candidate.
struct FirstListed;

impl ActionSelector for FirstListed {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
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
}

/// The merge capability needs approval.
fn merge_needs_approval() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(
        MERGE_CAPABILITY,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "approve the merge".to_owned(),
        }),
    )
}

/// One loop over a fresh ledger, without a governor and with one.
fn both_ledgers(complete_at: Option<i64>) -> (LoopEnd, LoopEnd) {
    let plain = Ledger::new(complete_at);
    let ungoverned = run_loop(
        &plain,
        &Loom::new(FirstListed, EmptyObjectArguments, "ship the change"),
        &merge_needs_approval(),
        &Effects(Some(&plain)),
    );

    let governed_ledger = Ledger::new(complete_at);
    let governed = run_loop(
        &governed_ledger,
        &Loom::new(FirstListed, EmptyObjectArguments, "ship the change")
            .with_governor(&governed_ledger),
        &merge_needs_approval(),
        &Effects(Some(&governed_ledger)),
    );
    (ungoverned, governed)
}

fn admitted(end: &LoopEnd) -> Vec<&str> {
    end.admitted
        .iter()
        .map(|request| request.action.as_str())
        .collect()
}

/// Two effects move the case to revision 3, where the governor holds it complete: both Looms end
/// the run completed, after the same two admitted requests, with the same loop record.
#[test]
fn without_an_outside_move_a_governed_run_completes_as_an_ungoverned_one() {
    let (ungoverned, governed) = both_ledgers(Some(3));
    let completed = RunOutcome::Completed(RunOutcomeCompleted {
        outcome: "merged".to_owned(),
    });

    assert_eq!(ungoverned.outcome, completed, "{ungoverned:#?}");
    assert_eq!(admitted(&ungoverned), [WRITE, TEST], "{ungoverned:#?}");
    assert_eq!(governed, ungoverned);
}

/// Two effects move the case to revision 3, where the merge needs approval the provider does not
/// give: both Looms end the run needing authority for it, with the same loop record.
#[test]
fn without_an_outside_move_a_governed_run_stops_at_the_approval_as_an_ungoverned_one() {
    let (ungoverned, governed) = both_ledgers(None);
    let needs = RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
        request: "approve the merge".to_owned(),
    });

    assert_eq!(ungoverned.outcome, needs, "{ungoverned:#?}");
    assert_eq!(admitted(&ungoverned), [WRITE, TEST], "{ungoverned:#?}");
    assert_eq!(governed, ungoverned);
}
