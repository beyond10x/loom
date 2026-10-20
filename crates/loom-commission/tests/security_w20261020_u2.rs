//! Security review of `story:authority-sees-the-request` (wave 2026-10-20-w1, U2): conformance
//! checks of the runtime's two authority call sites against `AGENTS.md` § Rules.
//!
//! * "Revalidate every selected action against case revision, frontier and authority before
//!   execution": the request an authority provider decided is bound to the case revision it was
//!   made at, and the provider can now grant it at that revision only. A case another writer moves
//!   on while the provider decides is no longer at that revision when the request is invoked, so
//!   the effect port must not be handed it, at the run loop's check or at a resumed request's.
//! * "When a selector, integration, verifier or authority provider fails, fail toward less
//!   authority": a provider that panics while deciding performs nothing at either call site, and
//!   a resume it panics in leaves the Suspension pending.
//! * Pass 2, against the revision the runtime reads again once the provider allows: what that
//!   read refuses leaves nothing taken, held or performed and is recorded stale, and a case that
//!   moves after that read, while a resume takes its Suspension, is still not invoked.
//!
//! Every port here is hand-written against Commission alone, so the same cases build on any
//! commit that has these ports.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::RunStorage;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionRequestStale, ActionStatus, AgentRevisionId,
    AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, CompletionDeterminationComplete,
    EffectOutcome, EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Frontier, FrontierAction, FrontierData, FrontierId, GovernorError, HoldSuspension,
    HoldSuspensionOutcome, Observation, ObservationId, PrincipalId, ProposedActionArguments,
    ReleaseSuspension, ReleaseSuspensionOutcome, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RunOutcomeNeedsAuthority, SuspensionSnapshot, SuspensionState, TakeSuspension,
    TakeSuspensionOutcome, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStore, SuspensionStoreError,
};
use b10x_loom_commission::runtime::{
    LoopContext, ResumeError, ResumeRefusal, resume_after_grant, run_until_blocked,
    run_with_suspension,
};

const MERGE: &str = "merge";
const CAPABILITY: &str = "repository.merge";
const WITHHELD: &str = "approve the merge";
const CASE: &str = "case-security-w20261020-u2";
const START: i64 = 7;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn case_id() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case_id(),
        principal: PrincipalId("principal-s".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// One case, held by a governor any writer can move on. Its frontier lists [`MERGE`] as needing
/// [`CAPABILITY`] at whatever revision the case is at.
struct Case {
    revision: Mutex<i64>,
    complete: Mutex<bool>,
    frontiers: Mutex<u64>,
}

impl Case {
    fn at(revision: i64) -> Self {
        Self {
            revision: Mutex::new(revision),
            complete: Mutex::new(false),
            frontiers: Mutex::new(0),
        }
    }

    fn revision(&self) -> i64 {
        *lock(&self.revision)
    }

    /// Another writer moves the case on by one revision.
    fn moved_on(&self) {
        *lock(&self.revision) += 1;
    }

    fn completed(&self) {
        *lock(&self.complete) = true;
    }
}

impl Governor for Case {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        if case.0 != CASE {
            return Err(GovernorError::UnknownCase);
        }
        Ok(self.revision())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        if case.0 != CASE {
            return Err(GovernorError::UnknownCase);
        }
        let issued = {
            let mut frontiers = lock(&self.frontiers);
            *frontiers += 1;
            *frontiers
        };
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf00 + issued)),
            case_id: case.clone(),
            case_revision: self.revision(),
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            }],
        }))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        if case.0 != CASE {
            return Err(GovernorError::UnknownCase);
        }
        Ok(if *lock(&self.complete) {
            CompletionDetermination::Complete(CompletionDeterminationComplete {
                outcome: "merged".to_owned(),
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

/// Proposes the merge once, then nothing useful.
struct ProposesMergeOnce {
    outcomes: Mutex<VecDeque<ExecutorOutcome>>,
}

impl ProposesMergeOnce {
    fn new() -> Self {
        Self {
            outcomes: Mutex::new(VecDeque::from([ExecutorOutcome::ProposedAction(
                ExecutorOutcomeProposedAction {
                    action: MERGE.to_owned(),
                    arguments: ProposedActionArguments(Value::Null),
                },
            )])),
        }
    }

    fn none() -> Self {
        Self {
            outcomes: Mutex::new(VecDeque::new()),
        }
    }
}

impl AgentExecutor for ProposesMergeOnce {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        lock(&self.outcomes)
            .pop_front()
            .unwrap_or(ExecutorOutcome::NoUsefulAction(Unit(true)))
    }
}

/// Performs the merge, which completes the case, and records each request it was handed beside
/// the revision the case was at when it was handed it.
struct Effects<'a> {
    case: &'a Case,
    invoked: Mutex<Vec<(ActionRequestData, i64)>>,
}

impl<'a> Effects<'a> {
    fn on(case: &'a Case) -> Self {
        Self {
            case,
            invoked: Mutex::new(Vec::new()),
        }
    }

    fn invoked(&self) -> Vec<(ActionRequestData, i64)> {
        lock(&self.invoked).clone()
    }

    /// Every request handed over while the case was at another revision than the request's.
    fn invoked_on_a_moved_case(&self) -> Vec<(i64, i64)> {
        self.invoked()
            .into_iter()
            .filter(|(request, at)| request.expected_case_revision != *at)
            .map(|(request, at)| (request.expected_case_revision, at))
            .collect()
    }
}

impl EffectPort for Effects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        lock(&self.invoked).push((data, self.case.revision()));
        self.case.completed();
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("merged".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// Allows the merge; while it decides, another writer moves the case on, as a provider that asks
/// a remote authority gives any concurrent writer time to.
struct MovesWhileDeciding<'a> {
    case: &'a Case,
    decided: Mutex<usize>,
}

impl<'a> MovesWhileDeciding<'a> {
    fn on(case: &'a Case) -> Self {
        Self {
            case,
            decided: Mutex::new(0),
        }
    }

    fn decided(&self) -> usize {
        *lock(&self.decided)
    }
}

impl AuthorityProvider for MovesWhileDeciding<'_> {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        *lock(&self.decided) += 1;
        self.case.moved_on();
        Ok(if capability == CAPABILITY {
            AuthorityVerdict::Allow(Unit(true))
        } else {
            withheld()
        })
    }
}

fn withheld() -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: WITHHELD.to_owned(),
    })
}

/// Withholds every capability.
struct Withholds;

impl AuthorityProvider for Withholds {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        _capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(withheld())
    }
}

/// Panics while it decides.
struct Panics;

impl AuthorityProvider for Panics {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        _capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        panic!("authority provider panicked while deciding")
    }
}

struct Context {
    requests: u64,
    observations: u64,
}

impl Context {
    fn starting_at(base: u64) -> Self {
        Self {
            requests: base,
            observations: base + 0x100,
        }
    }
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-20T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(4)
    }
}

fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(issued))
    })
}

/// Runs the merge at [`START`] with a provider that withholds it, so the run ends holding a
/// Suspension for the request, made at [`START`].
fn suspend_at_start(case: &Case, effects: &Effects<'_>, suspensions: &SuspensionStore) {
    let end = run_with_suspension(
        case,
        &ProposesMergeOnce::new(),
        &Withholds,
        effects,
        &commission(),
        &mut runs(0x100),
        suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("precondition: the withheld run ends: {error:?}"));
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: WITHHELD.to_owned(),
        }),
        "precondition: the merge is withheld"
    );
    assert!(
        effects.invoked().is_empty(),
        "precondition: nothing performed"
    );
}

/// The run loop's check (`runtime.rs`, the `NeedsAuthority` arm of item 8): the provider allows
/// the request made at revision 7, and while it decides the case moves on to 8. The request is
/// still bound to 7 and the provider was never asked about 8, so the effect port must not be
/// handed it: a selection made on state that moved is not permission.
#[test]
fn security_a_case_moved_while_the_provider_decides_is_not_invoked_in_the_run_loop() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let provider = MovesWhileDeciding::on(&case);

    let ended = run_until_blocked(
        &case,
        &ProposesMergeOnce::new(),
        &provider,
        &effects,
        &commission(),
        &mut runs(0x100),
        &mut Context::starting_at(0x300),
    );

    assert_eq!(
        provider.decided(),
        1,
        "precondition: the provider decided the request once; the run ended {ended:?}"
    );
    assert_eq!(
        case.revision(),
        START + 1,
        "precondition: the case moved on"
    );
    let moved = effects.invoked_on_a_moved_case();
    assert!(
        moved.is_empty(),
        "the effect port was handed a request bound to a revision the case had left while the \
         provider decided, as (request revision, case revision at invocation): {moved:?}; the run \
         ended {ended:?}"
    );
}

/// The resumed request's check (`resume_after_grant`): the Suspension holds the request made at
/// 7, the case is still at 7 when the resume revalidates it, and while the provider decides the
/// case moves on to 8. The resume must not perform the request on the moved case.
#[test]
fn security_a_case_moved_while_the_provider_decides_is_not_invoked_on_a_resume() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let suspensions = SuspensionStore::new();
    suspend_at_start(&case, &effects, &suspensions);

    let provider = MovesWhileDeciding::on(&case);
    let resumed = resume_after_grant(
        &case,
        &ProposesMergeOnce::none(),
        &provider,
        &effects,
        &commission(),
        &mut runs(0x200),
        &suspensions,
        &mut Context::starting_at(0x500),
    );

    assert_eq!(
        provider.decided(),
        1,
        "precondition: the provider decided the resumed request once; the resume answered \
         {resumed:?}"
    );
    assert_eq!(
        case.revision(),
        START + 1,
        "precondition: the case moved on"
    );
    let moved = effects.invoked_on_a_moved_case();
    assert!(
        moved.is_empty(),
        "the resume handed the effect port the suspended request after the case left its \
         revision while the provider decided, as (request revision, case revision at \
         invocation): {moved:?}; the resume answered {resumed:?}"
    );
}

/// A provider that panics at the run loop's check grants nothing: the panic leaves the loop and
/// the effect port was handed nothing.
#[test]
fn security_a_panicking_provider_performs_nothing_in_the_run_loop() {
    let case = Case::at(START);
    let effects = Effects::on(&case);

    let ran = catch_unwind(AssertUnwindSafe(|| {
        run_until_blocked(
            &case,
            &ProposesMergeOnce::new(),
            &Panics,
            &effects,
            &commission(),
            &mut runs(0x100),
            &mut Context::starting_at(0x300),
        )
    }));

    assert!(
        ran.is_err(),
        "the provider's panic leaves the loop, got {ran:?}"
    );
    assert!(
        effects.invoked().is_empty(),
        "a panicking provider performed {:?}",
        effects.invoked()
    );
}

/// A provider that panics at a resumed request's check grants nothing: nothing is performed and
/// the Suspension stays pending, so a later resume can still decide it.
#[test]
fn security_a_panicking_provider_performs_nothing_on_a_resume() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let suspensions = SuspensionStore::new();
    suspend_at_start(&case, &effects, &suspensions);

    let resumed = catch_unwind(AssertUnwindSafe(|| {
        resume_after_grant(
            &case,
            &ProposesMergeOnce::none(),
            &Panics,
            &effects,
            &commission(),
            &mut runs(0x200),
            &suspensions,
            &mut Context::starting_at(0x500),
        )
    }));

    assert!(
        resumed.is_err(),
        "the provider's panic leaves the resume, got {resumed:?}"
    );
    assert!(
        effects.invoked().is_empty(),
        "a panicking provider performed {:?}",
        effects.invoked()
    );
    let stored = SuspensionPort::get(&suspensions, &commission().data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("the Suspension is still stored"));
    assert_eq!(stored.state, SuspensionState::Pending);
}

/// Allows every capability and moves nothing.
struct Allows;

impl AuthorityProvider for Allows {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        _capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(AuthorityVerdict::Allow(Unit(true)))
    }
}

/// A suspension store during whose `TakeSuspension` another writer moves the case on, as one that
/// writes the Suspension to disk gives any concurrent writer time to. Every command is answered by
/// `inner`.
struct MovesOnTake<'a> {
    inner: &'a SuspensionStore,
    case: &'a Case,
}

impl SuspensionPort for MovesOnTake<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.inner.hold(input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        self.case.moved_on();
        self.inner.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.inner.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        SuspensionPort::get(self.inner, commission)
    }
}

/// Pass 2, a second move after the revision read once the provider allows: the resume reads the
/// case at 7 after the allow, then resumes the Run and takes the Suspension, and while it takes
/// it the case moves on to 8. The held request is bound to 7 and the case is at 8 when the
/// resumed loop hands it over, so the effect port must not be handed it.
#[test]
fn security_a_case_moved_while_the_resume_takes_its_suspension_is_not_invoked() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let suspensions = SuspensionStore::new();
    suspend_at_start(&case, &effects, &suspensions);

    let taking = MovesOnTake {
        inner: &suspensions,
        case: &case,
    };
    let resumed = resume_after_grant(
        &case,
        &ProposesMergeOnce::none(),
        &Allows,
        &effects,
        &commission(),
        &mut runs(0x200),
        &taking,
        &mut Context::starting_at(0x500),
    );

    assert_eq!(
        case.revision(),
        START + 1,
        "precondition: the case moved on while the Suspension was taken; the resume answered \
         {resumed:?}"
    );
    let moved = effects.invoked_on_a_moved_case();
    assert!(
        moved.is_empty(),
        "the resume handed the effect port the suspended request after the case left its \
         revision while the Suspension was taken, as (request revision, case revision at \
         invocation): {moved:?}; the resume answered {resumed:?}"
    );
}

/// Pass 2, what the run loop's read after the allow leaves: the request the provider allowed at
/// 7 on a case that moved to 8 while it decided is recorded `Stale`, naming both revisions, in
/// place of `NeedsAuthority`; it is not admitted, not performed, and with a suspension port
/// nothing is held for it.
#[test]
fn security_the_run_loop_records_a_request_allowed_on_a_moved_case_stale_and_holds_nothing() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let provider = MovesWhileDeciding::on(&case);
    let suspensions = SuspensionStore::new();

    let end = run_with_suspension(
        &case,
        &ProposesMergeOnce::new(),
        &provider,
        &effects,
        &commission(),
        &mut runs(0x100),
        &suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    assert_eq!(provider.decided(), 1, "precondition: one decision; {end:?}");
    let outcomes: Vec<RevalidateActionRequestOutcome> = end
        .requests
        .iter()
        .map(|revalidated| revalidated.outcome.clone())
        .collect();
    assert_eq!(
        outcomes,
        vec![RevalidateActionRequestOutcome::Stale {
            error: ActionRequestStale {
                expected_case_revision: START,
                current_case_revision: START + 1,
            },
        }],
        "the request allowed on a moved case is recorded stale: {end:?}"
    );
    assert!(end.admitted.is_empty(), "nothing is admitted: {end:?}");
    assert!(effects.invoked().is_empty(), "nothing is performed");
    let held = SuspensionPort::get(&suspensions, &commission().data().commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"));
    assert_eq!(held, None, "nothing is held for a stale request");
}

/// Pass 2, what the resume's read after the allow leaves: refused `CaseMoved` naming 7 and 8,
/// nothing performed, no Run put into the new process's run store, and the Suspension still
/// `Pending` for the same request.
#[test]
fn security_a_resume_refused_after_the_allow_takes_nothing_and_resumes_no_run() {
    let case = Case::at(START);
    let effects = Effects::on(&case);
    let suspensions = SuspensionStore::new();
    suspend_at_start(&case, &effects, &suspensions);
    let commission_id = commission().data().commission_id.clone();
    let before = SuspensionPort::get(&suspensions, &commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("precondition: a Suspension is held"));

    let provider = MovesWhileDeciding::on(&case);
    let mut fresh = runs(0x200);
    let resumed = resume_after_grant(
        &case,
        &ProposesMergeOnce::none(),
        &provider,
        &effects,
        &commission(),
        &mut fresh,
        &suspensions,
        &mut Context::starting_at(0x500),
    );

    match &resumed {
        Err(ResumeError::Refused(ResumeRefusal::CaseMoved {
            expected_case_revision,
            current_case_revision,
        })) => {
            assert_eq!(
                (*expected_case_revision, *current_case_revision),
                (START, START + 1)
            );
        }
        other => panic!("a move while the provider decided refuses the resume, got {other:?}"),
    }
    assert!(effects.invoked().is_empty(), "nothing is performed");
    assert!(
        RunStorage::list(&fresh).is_empty(),
        "no Run is put or resumed: {:?}",
        RunStorage::list(&fresh)
    );
    let after = SuspensionPort::get(&suspensions, &commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("the Suspension is still stored"));
    assert_eq!(after.state, SuspensionState::Pending, "nothing is taken");
    assert_eq!(after, before, "the Suspension is held as it was");
}
