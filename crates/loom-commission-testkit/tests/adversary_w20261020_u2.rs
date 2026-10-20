//! Adversary cases for `story:authority-sees-the-request` (wave 2026-10-20-w1, U2).
//!
//! The unit tells an authority provider the action request it decides, bound to the case revision
//! the runtime revalidated it at, so that "a provider can allow one request at one case revision
//! and refuse the same action at the next" (`crates/loom-commission/src/ports/authority.rs`, the
//! `CHANGELOG.md` entry, the `AGENTS.md` row "a grant bound to one revision allows no other").
//!
//! These cases hold the runtime to that binding at the moment it matters, the invoke: a request
//! the provider allowed at revision 7 is not performed once the case has moved to 8 while the
//! provider was deciding. Somebody else moving the case between two port calls is a state the
//! runtime itself models ("after a refusal, a move is somebody else's"), and the governor's file
//! stores take their lock per operation, "in one process or in two"
//! (`crates/loom-governor/src/locked_dir.rs`), so a second process over the same state directory
//! can write the case while a provider decides. The provider below stands in for that writer: it
//! moves the case on the fake governor from inside `decide`, the one port call between the
//! runtime's revalidation and its invoke at the run loop.

use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeNeedsAuthority, SuspensionState, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, resume_after_grant, run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};

const MERGE: &str = "merge";
const CAPABILITY: &str = "repository.merge";
const WITHHELD: &str = "approve the merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xb1)),
        agent_revision_id: AgentRevisionId(uuid(0xb2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary".to_owned()),
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

fn propose_merge() -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: MERGE.to_owned(),
        arguments: ProposedActionArguments(
            json::parse(r#"{"strategy":"squash"}"#)
                .unwrap_or_else(|error| panic!("arguments fixture is not JSON: {error:?}")),
        ),
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
        Timestamp("2026-10-20T10:00:00Z".to_owned())
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

/// A grant bound to one case revision, as `authority_request.rs`'s `RevisionBoundGrant` is: it
/// allows [`MERGE`] under [`CAPABILITY`] only for a request made at `revision`. On its first
/// question, while it decides, another writer moves the case: the fake governor is scripted to
/// `moved` from then on.
struct GrantWhileTheCaseMoves<'a> {
    revision: i64,
    governor: &'a FakeGovernor,
    case: CaseId,
    moved: Answer,
    asked: Mutex<Vec<ActionRequestData>>,
}

impl<'a> GrantWhileTheCaseMoves<'a> {
    fn new(revision: i64, governor: &'a FakeGovernor, case: &CaseId, moved: Answer) -> Self {
        Self {
            revision,
            governor,
            case: case.clone(),
            moved,
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

impl AuthorityProvider for GrantWhileTheCaseMoves<'_> {
    fn decide(
        &self,
        _commission: &CommissionData,
        request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let first = {
            let mut asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
            asked.push(request.clone());
            asked.len() == 1
        };
        if first {
            // Somebody else writes the case while the provider decides.
            self.governor
                .script(self.case.clone(), [self.moved.clone()]);
        }
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

/// Runs the commission on `case` with a suspension port, the merge proposed once at revision 7 and
/// withheld, so the run ends holding a Suspension for the request at 7.
fn suspend_at_seven(
    governor: &FakeGovernor,
    effects: &Effects,
    suspensions: &SuspensionStore,
    case: &CaseId,
) -> LoopEnd {
    governor.script(case.clone(), [gated_merge(7)]);
    let withholding = StaticAuthorityProvider::new().answer(CAPABILITY, withheld());
    let end = run_with_suspension(
        governor,
        &ScriptedExecutor::new([propose_merge()]),
        &withholding,
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
        "precondition: the merge is withheld at 7"
    );
    end
}

/// The run loop's check: the provider allows the merge at 7, the request the runtime revalidated
/// at 7. While it decides, the case moves to 8 under somebody else. The request bound to 7 is
/// stale by the time the runtime would invoke it, so it is not performed: a grant at one revision
/// does not perform at the next.
#[test]
fn adversary_the_run_loop_does_not_perform_a_request_whose_case_moved_while_the_provider_decided() {
    let case = CaseId("case-adversary-moved-while-deciding-loop".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [gated_merge(7)]);
    let grant = GrantWhileTheCaseMoves::new(
        7,
        &governor,
        &case,
        gated_merge(8).complete("merged by somebody else"),
    );
    let effects = Effects::default();

    let end = run_until_blocked(
        &governor,
        &ScriptedExecutor::new([propose_merge(), propose_merge()]),
        &grant,
        &effects,
        &commission(&case),
        &mut runs(0x100),
        &mut Context::starting_at(0x300),
    );

    let asked = grant.asked();
    assert_eq!(
        asked.first().map(|request| request.expected_case_revision),
        Some(7),
        "precondition: the provider is asked about the request at 7, got {asked:?}"
    );
    let performed = effects.invoked();
    assert!(
        performed.is_empty(),
        "the case moved to 8 while the provider decided; the request it allowed at 7 is stale and \
         must not be performed, but the effect port was handed {performed:?} (the run ended {end:?})"
    );
}

/// The resumed request's check: the Suspension holds the merge at 7, the case is at 7 when the
/// resume reads it, and the provider allows the request at 7. While it decides, the case moves to
/// 8 under somebody else. The resume performs nothing and leaves the Suspension pending.
#[test]
fn adversary_a_resume_does_not_perform_a_request_whose_case_moved_while_the_provider_decided() {
    let case = CaseId("case-adversary-moved-while-deciding-resume".to_owned());
    let governor = FakeGovernor::new();
    let effects = Effects::default();
    let suspensions = SuspensionStore::new();
    suspend_at_seven(&governor, &effects, &suspensions, &case);

    governor.script(case.clone(), [gated_merge(7)]);
    let grant = GrantWhileTheCaseMoves::new(
        7,
        &governor,
        &case,
        gated_merge(8).complete("merged by somebody else"),
    );
    let resumed = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &grant,
        &effects,
        &commission(&case),
        &mut runs(0x200),
        &suspensions,
        &mut Context::starting_at(0x500),
    );

    let asked = grant.asked();
    assert_eq!(
        asked.first().map(|request| request.expected_case_revision),
        Some(7),
        "precondition: the provider is asked about the held request at 7, got {asked:?}"
    );
    let performed = effects.invoked();
    assert!(
        performed.is_empty(),
        "the case moved to 8 while the provider decided; the held request it allowed at 7 is stale \
         and must not be performed, but the effect port was handed {performed:?} (the resume \
         answered {resumed:?})"
    );
    let commission_id = commission(&case).data().commission_id.clone();
    let stored = SuspensionPort::get(&suspensions, &commission_id)
        .unwrap_or_else(|error| panic!("the store reads: {error:?}"));
    assert_eq!(
        stored.map(|stored| stored.state),
        Some(SuspensionState::Pending),
        "a resume that performed nothing leaves the Suspension pending"
    );
}
