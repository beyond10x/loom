//! Acceptance for `story:authority-sees-the-request`: `AuthorityProvider::decide` receives the
//! action request it decides, beside the commission and the capability, at both of the runtime's
//! authority checks (the run loop and a resumed request).
//!
//! The request is the one the runtime revalidated: bound to the revision of the frontier it was
//! made on, never to anything the executor wrote in its arguments (`AGENTS.md` § Rules: the model
//! never supplies a case revision). So a provider can grant one request at one revision and refuse
//! the same action at the next, and a provider that fails still grants nothing: authority fails
//! toward less authority.

use std::iter::repeat_n;
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionRequestStale, ActionStatus, AgentRevisionId,
    AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, HoldSuspension, HoldSuspensionOutcome,
    ObservationId, PrincipalId, ProposedActionArguments, ReleaseSuspension,
    ReleaseSuspensionOutcome, RevalidateActionRequestOutcome, RunId, RunOutcome,
    RunOutcomeCaseMovedOn, RunOutcomeCompleted, RunOutcomeNeedsAuthority, SuspensionSnapshot,
    SuspensionState, TakeSuspension, TakeSuspensionOutcome, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{
    AuthorityCheck, AuthorityProvider, AuthorityProviderError, check_authority,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStore, SuspensionStoreError,
};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, ResumeError, ResumeRefusal, Revalidated, resume_after_grant,
    run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

/// The action every case here proposes, and the capability its frontier entry names.
const MERGE: &str = "merge";
const CAPABILITY: &str = "repository.merge";
const WITHHELD: &str = "approve the merge";
const FAILURE: &str = "authority backend unreachable";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xa1)),
        agent_revision_id: AgentRevisionId(uuid(0xa2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-r".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case at `revision`, open, its frontier listing [`MERGE`] as needing [`CAPABILITY`].
fn gated_merge(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: MERGE.to_owned(),
            status: ActionStatus::ApprovalRequired,
            capability: Some(CAPABILITY.to_owned()),
            reasons: Vec::new(),
        }],
    )
}

/// The arguments the executor proposes the merge with. They name a revision and artifact
/// revisions of their own, which the request must not take: its revision is the frontier's.
fn arguments() -> ProposedActionArguments {
    ProposedActionArguments(
        json::parse(
            r#"{"strategy":"squash","expected_case_revision":99,"artifact_revisions":{"implementation":"r-forged"}}"#,
        )
        .unwrap_or_else(|error| panic!("arguments fixture is not JSON: {error:?}")),
    )
}

fn propose_merge() -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: MERGE.to_owned(),
        arguments: arguments(),
    })
}

fn allow() -> AuthorityVerdict {
    AuthorityVerdict::Allow(Unit(true))
}

fn withheld() -> AuthorityVerdict {
    AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
        request: WITHHELD.to_owned(),
    })
}

#[derive(Debug)]
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
        None
    }
}

fn runs(base: u64) -> RunStore {
    let mut issued = base;
    RunStore::new(move || {
        issued += 1;
        RunId(uuid(issued))
    })
}

/// Performs every action and records every request it was handed.
#[derive(Default)]
struct Effects {
    invoked: Mutex<Vec<ActionRequestData>>,
}

impl Effects {
    fn invoked(&self) -> Vec<ActionRequestData> {
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let data = request.data().clone();
        let report = Value::Text(format!("done {}", data.action));
        self.invoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(data);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// A grant bound to one case revision: it allows [`MERGE`] under [`CAPABILITY`] only for a request
/// made at `revision`, and answers any other request with approval required, naming the revision
/// it was asked about. It records every request it was asked about.
struct RevisionBoundGrant {
    revision: i64,
    asked: Mutex<Vec<ActionRequestData>>,
}

impl RevisionBoundGrant {
    fn at(revision: i64) -> Self {
        Self {
            revision,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<ActionRequestData> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AuthorityProvider for RevisionBoundGrant {
    fn decide(
        &self,
        _commission: &CommissionData,
        request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Ok(
            if request.action == MERGE
                && capability == CAPABILITY
                && request.expected_case_revision == self.revision
            {
                allow()
            } else {
                AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                    request: format!(
                        "approve `{}` at revision {}",
                        request.action, request.expected_case_revision
                    ),
                })
            },
        )
    }
}

/// The one question `provider` was asked: its capability and its action request.
fn asked_once(provider: &StaticAuthorityProvider) -> (String, ActionRequestData) {
    let asked = provider.asked();
    let requests = provider.requests();
    match (asked.as_slice(), requests.as_slice()) {
        ([query], [request]) => (query.capability.clone(), request.clone()),
        _ => panic!("the provider is asked once, got {asked:?} about {requests:?}"),
    }
}

/// Runs the commission on `case` with a suspension port, the merge proposed once at revision 7 and
/// withheld by `authority`, so the run ends holding a Suspension for the request.
fn suspend_at_seven(
    governor: &FakeGovernor,
    authority: &StaticAuthorityProvider,
    effects: &Effects,
    suspensions: &SuspensionStore,
    case: &CaseId,
) -> LoopEnd {
    governor.script(case.clone(), [gated_merge(7)]);
    let executor = ScriptedExecutor::new([propose_merge()]);
    let end = run_with_suspension(
        governor,
        &executor,
        authority,
        effects,
        &commission(case),
        &mut runs(0x100),
        suspensions,
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: WITHHELD.to_owned(),
        }),
        "precondition: the merge is withheld"
    );
    end
}

/// The provider records what it was asked. At the run loop's check, the request it receives is the
/// request the runtime revalidated: its action, its case revision (the frontier's 7, not the 99 the
/// arguments name) and its arguments as proposed. At the resumed request's check it receives the
/// request the Suspension holds, the same one, and the request it allowed is the request invoked.
#[test]
fn the_provider_sees_the_request_it_decides() {
    let case = CaseId("case-authority-request".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    let withholding = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());

    let end = suspend_at_seven(&governor, &withholding, &effects, &suspensions, &case);
    let [revalidated] = end.requests.as_slice() else {
        panic!("one request revalidated, got {:?}", end.requests);
    };
    let revalidated = revalidated.request.clone();
    let (capability, seen) = asked_once(&withholding);
    assert_eq!(
        seen, revalidated,
        "the provider is asked about the request the runtime revalidated"
    );
    assert_eq!(capability, CAPABILITY);
    assert_eq!(seen.action, MERGE);
    assert_eq!(
        seen.expected_case_revision, 7,
        "the request is bound to the frontier's revision, never to one the arguments name"
    );
    assert_eq!(seen.case_id, case);
    assert_eq!(seen.arguments, arguments());
    assert!(effects.invoked().is_empty(), "nothing is performed yet");

    // The resume, in a new process: the case is still at 7 and completes once the merge is done.
    governor.script(
        case.clone(),
        repeat_n(gated_merge(7), 3).chain([gated_merge(7).complete("merged")]),
    );
    let granting = StaticAuthorityProvider::new().answer(CAPABILITY, allow());
    let resumed = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &granting,
        &effects,
        &commission(&case),
        &mut runs(0x200),
        &suspensions,
        &mut Context::starting_at(0x500),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    let (capability, seen) = asked_once(&granting);
    assert_eq!(
        seen, revalidated,
        "the resumed request's provider is asked about the request the Suspension holds"
    );
    assert_eq!(capability, CAPABILITY);
    assert_eq!(
        effects.invoked(),
        vec![revalidated],
        "the request the provider allowed is the request invoked"
    );
    assert_eq!(
        resumed.outcome,
        RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "merged".to_owned(),
        })
    );
}

/// A provider that allows only revision 4 allows the request at 4, which is performed. The merge's
/// effect moves the case to 5; the same action proposed there is a request at 5, which the
/// provider answers with approval required, and nothing more is performed.
#[test]
fn a_grant_bound_to_one_revision_does_not_allow_the_next() {
    let case = CaseId("case-revision-bound-grant".to_owned());
    let governor = FakeGovernor::new();
    // The first iteration's three calls, its revalidation's two and the load after the allow at 4,
    // then the case at 5.
    governor.script(
        case.clone(),
        repeat_n(gated_merge(4), 6).chain([gated_merge(5)]),
    );
    let executor = ScriptedExecutor::new([propose_merge(), propose_merge()]);
    let grant = RevisionBoundGrant::at(4);
    let effects = Effects::default();

    let end = run_until_blocked(
        &governor,
        &executor,
        &grant,
        &effects,
        &commission(&case),
        &mut runs(0x100),
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    let asked = grant.asked();
    let revisions: Vec<i64> = asked
        .iter()
        .map(|request| request.expected_case_revision)
        .collect();
    assert_eq!(revisions, [4, 5], "the provider is asked at 4, then at 5");
    let revalidated: Vec<ActionRequestData> = end
        .requests
        .iter()
        .map(|revalidated| revalidated.request.clone())
        .collect();
    assert_eq!(
        asked, revalidated,
        "each time about the request revalidated"
    );
    assert_eq!(
        effects.invoked(),
        vec![asked[0].clone()],
        "only the request at 4 is performed"
    );
    assert_eq!(end.admitted, vec![asked[0].clone()]);
    assert_eq!(
        end.outcome,
        RunOutcome::NeedsAuthority(RunOutcomeNeedsAuthority {
            request: format!("approve `{MERGE}` at revision 5"),
        }),
        "the same action at 5 is answered approval required"
    );
}

/// A provider that fails grants nothing, at either check, though it was asked about the request:
/// the run loop admits and performs nothing, a resume is refused `NotGranted` with the refusal and
/// leaves the Suspension pending, and `check_authority` itself refuses.
#[test]
fn a_failing_provider_still_grants_nothing() {
    let failing = || StaticAuthorityProvider::new().fail(CAPABILITY, FAILURE);

    // The run loop.
    let case = CaseId("case-failing-provider-loop".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated_merge(7)]);
    let executor = ScriptedExecutor::new(
        std::iter::once(propose_merge())
            .chain(repeat_n(ExecutorOutcome::NoUsefulAction(Unit(true)), 3)),
    );
    let effects = Effects::default();
    let provider = failing();
    let end = run_until_blocked(
        &governor,
        &executor,
        &provider,
        &effects,
        &commission(&case),
        &mut runs(0x100),
        &mut Context::starting_at(0x300),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let (_, seen) = asked_once(&provider);
    assert_eq!(seen, end.requests[0].request);
    assert!(end.admitted.is_empty(), "nothing is admitted: {end:?}");
    assert!(effects.invoked().is_empty(), "nothing is performed");

    // A resumed request.
    let case = CaseId("case-failing-provider-resume".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    let withholding = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let end = suspend_at_seven(&governor, &withholding, &effects, &suspensions, &case);
    let held = end.requests[0].request.clone();
    let provider = failing();
    let resumed = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &provider,
        &effects,
        &commission(&case),
        &mut runs(0x200),
        &suspensions,
        &mut Context::starting_at(0x500),
    );
    match &resumed {
        Err(ResumeError::Refused(ResumeRefusal::NotGranted(AuthorityCheck::Refused(error)))) => {
            assert!(
                error.to_string().contains(FAILURE),
                "the refusal carries the provider's failure: {error}"
            );
        }
        other => panic!("a failing provider refuses the resume NotGranted, got {other:?}"),
    }
    let (_, seen) = asked_once(&provider);
    assert_eq!(seen, held);
    assert!(effects.invoked().is_empty(), "nothing is performed");
    let commission_id = commission(&case).data().commission_id.clone();
    let stored = SuspensionPort::get(&suspensions, &commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"))
        .unwrap_or_else(|| panic!("the Suspension is still stored"));
    assert_eq!(stored.state, SuspensionState::Pending);

    // The check itself.
    let check = check_authority(&failing(), &commission(&case), &held, CAPABILITY);
    assert!(
        matches!(check, AuthorityCheck::Refused(_)),
        "a provider failure is refused, got {check:?}"
    );
    assert!(!check.allows());
}

/// The suspension store, with somebody else moving the case once, as the first `take` lands: the
/// governor is scripted to `moved` from then on.
struct MovesOnTake<'a> {
    inner: &'a SuspensionStore,
    governor: &'a FakeGovernor,
    case: CaseId,
    moved: Answer,
    took: Mutex<bool>,
}

impl SuspensionPort for MovesOnTake<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        SuspensionPort::hold(self.inner, input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        let first = !std::mem::replace(
            &mut *self.took.lock().unwrap_or_else(PoisonError::into_inner),
            true,
        );
        if first {
            self.governor
                .script(self.case.clone(), [self.moved.clone()]);
        }
        SuspensionPort::take(self.inner, input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        SuspensionPort::release(self.inner, input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        SuspensionPort::get(self.inner, commission)
    }
}

/// A resume reads the case at 7 after the provider allows the held request; the case moves to 8
/// as the resume takes the Suspension. Read once more right before the invoke, the move is seen:
/// the request is recorded `Stale` naming 7 and 8, nothing is invoked, the Suspension the resume
/// took is released, since the request can never be performed, and the resumed loop judges the
/// move: the Run bound to 7 ends `CaseMovedOn` at 8.
#[test]
fn a_resume_that_sees_the_case_moved_after_its_take_releases_the_suspension_and_performs_nothing() {
    let case = CaseId("case-moved-after-take".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    let withholding = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let end = suspend_at_seven(&governor, &withholding, &effects, &suspensions, &case);
    let held = end.requests[0].request.clone();

    governor.script(case.clone(), [gated_merge(7)]);
    let moved = Answer::at(8).with_items(
        Vec::new(),
        Vec::new(),
        vec![
            FrontierAction {
                action: "inspect".to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            },
            FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            },
        ],
    );
    let port = MovesOnTake {
        inner: &suspensions,
        governor: &governor,
        case: case.clone(),
        moved,
        took: Mutex::new(false),
    };
    let granting = StaticAuthorityProvider::new().answer(CAPABILITY, allow());
    let resumed = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &granting,
        &effects,
        &commission(&case),
        &mut runs(0x200),
        &port,
        &mut Context::starting_at(0x500),
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));

    assert!(
        *port.took.lock().unwrap_or_else(PoisonError::into_inner),
        "precondition: the resume took the Suspension"
    );
    assert!(effects.invoked().is_empty(), "nothing is performed");
    assert_eq!(
        resumed.requests,
        vec![Revalidated {
            request: held,
            outcome: RevalidateActionRequestOutcome::Stale {
                error: ActionRequestStale {
                    expected_case_revision: 7,
                    current_case_revision: 8,
                },
            },
        }],
        "the held request is recorded stale"
    );
    assert!(resumed.admitted.is_empty(), "nothing is admitted");
    let commission_id = commission(&case).data().commission_id.clone();
    assert_eq!(
        SuspensionPort::get(&suspensions, &commission_id)
            .unwrap_or_else(|error| panic!("the store reads: {error:?}")),
        None,
        "the Suspension the resume took is released"
    );
    assert_eq!(
        resumed.outcome,
        RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
            bound_case_revision: 7,
            current_case_revision: 8,
        })
    );
}
