//! Adversary pass 1 on `story:run-outcomes`: derivation rows the acceptance table does not script,
//! each one a branch of `derive` that a mutant can flip while `run_outcome_derivation` stays green,
//! and the run store's keying and id guard, which the single-run acceptance cannot observe.
//!
//! Every row goes through the fakes the same way the acceptance does: the governor answers the
//! frontier and determination, the executor runs on that frontier, and the provider is asked only
//! when the proposal needs authority.

use b10x_commission::admission::admit;
use b10x_commission::model::behaviour::{Generated, RunStorage};
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::obligations::{
    ResumeRunBehavior, RunStatesQuery, StartRunBehavior, SuspendRunBehavior,
};
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended, FrontierAction, FrontierObligation,
    PrincipalId, ProposedActionArguments, ResumeRun, ResumeRunOutcome, RunId, RunOutcome,
    RunOutcomeCompleted, RunOutcomeNeedsExternalEvidence, RunState, StartRun, StartRunOutcome,
    SuspendRun, SuspendRunOutcome, SuspensionReason, Unit, commission_state,
};
use b10x_commission::outcome::{CapabilityVerdict, Derived, RunStore, derive};
use b10x_commission::ports::authority::{AuthorityCheck, check_authority};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

const CASE: &str = "case-adversary-run";

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

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn open(name: &str) -> FrontierObligation {
    FrontierObligation {
        obligation: name.to_owned(),
        open: true,
    }
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

/// A frontier whose only action is `deploy`, which needs the capability `prod.deploy`.
fn approval_only(obligations: Vec<FrontierObligation>) -> Answer {
    Answer::at(4).with_items(
        Vec::new(),
        obligations,
        vec![action(
            "deploy",
            ActionStatus::ApprovalRequired,
            Some("prod.deploy"),
        )],
    )
}

/// One scripted derivation through the fakes, as the acceptance runs it, except that a provider
/// failure is passed on as no verdict instead of failing the test.
fn derived(
    governor: Answer,
    executor: ExecutorOutcome,
    provider: &StaticAuthorityProvider,
) -> Derived {
    let commission = commission();
    let case = CaseId(CASE.to_owned());
    let fake = FakeGovernor::new();
    fake.script(case.clone(), [governor]);
    let frontier = fake.frontier(&case).expect("frontier");
    let determination = fake.completion(&case).expect("completion");
    let executor = ScriptedExecutor::new([executor]);
    let outcome = executor.run(&commission, &frontier);
    let verdict = match &outcome {
        ExecutorOutcome::ProposedAction(proposed) => match admit(&frontier, &proposed.action) {
            Admission::NeedsAuthority(needs) => {
                match check_authority(provider, &commission, &needs.capability) {
                    AuthorityCheck::Decided(verdict) => Some((needs.capability, verdict)),
                    AuthorityCheck::Refused(_) => None,
                }
            }
            _ => None,
        },
        _ => None,
    };
    derive(
        &determination,
        &frontier,
        &outcome,
        verdict
            .as_ref()
            .map(|(capability, verdict)| CapabilityVerdict {
                capability,
                verdict,
            }),
    )
}

fn no_admissible_action() -> Derived {
    Derived::Ended(RunOutcome::NoAdmissibleAction(Unit(true)))
}

/// A denied proposal is not acted on: with nothing else admissible and no open obligation the run
/// ends with no admissible action. A derivation that continued here would let a denied action
/// through. Kills the mutant that answers every non-`ApprovalRequired` verdict with continue.
#[test]
fn adversary_run_deny_on_the_proposed_action_does_not_continue() {
    let provider = StaticAuthorityProvider::new().answer(
        "prod.deploy",
        AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: "not now".to_owned(),
        }),
    );
    assert_eq!(
        derived(approval_only(Vec::new()), proposal("deploy"), &provider),
        no_admissible_action()
    );
    assert_eq!(provider.asked().len(), 1, "the provider was asked once");
}

/// A provider that fails to decide gives no verdict, and no verdict is not an allow.
#[test]
fn adversary_run_provider_failure_on_the_proposed_action_does_not_continue() {
    let provider = StaticAuthorityProvider::new().fail("prod.deploy", "provider down");
    assert_eq!(
        derived(approval_only(Vec::new()), proposal("deploy"), &provider),
        no_admissible_action()
    );
    assert_eq!(provider.asked().len(), 1, "the provider was asked once");
}

/// The provider's explicit allow continues the run, though nothing on the frontier is admissible
/// without authority. Kills the mutant that drops the `Allow` arm.
#[test]
fn adversary_run_allow_on_the_proposed_action_continues() {
    let provider =
        StaticAuthorityProvider::new().answer("prod.deploy", AuthorityVerdict::Allow(Unit(true)));
    assert_eq!(
        derived(approval_only(Vec::new()), proposal("deploy"), &provider),
        Derived::Continue
    );
}

/// An action that needs authority is not admissible without it (`derive`'s own doc): a frontier
/// holding only such an action admits nothing, so the obligations decide. Kills the mutant that
/// counts a `NeedsAuthority` action as admissible in rule 6.
#[test]
fn adversary_run_an_approval_required_action_is_not_counted_as_admissible() {
    let provider = StaticAuthorityProvider::new();
    assert_eq!(
        derived(
            approval_only(Vec::new()),
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            &provider
        ),
        no_admissible_action()
    );
    assert_eq!(
        derived(
            approval_only(vec![open("sign-off")]),
            ExecutorOutcome::NoUsefulAction(Unit(true)),
            &provider
        ),
        Derived::Ended(RunOutcome::NeedsExternalEvidence(
            RunOutcomeNeedsExternalEvidence {
                requirements: vec!["sign-off".to_owned()],
            }
        ))
    );
    assert!(
        provider.asked().is_empty(),
        "nobody proposed, so nobody asks"
    );
}

/// A proposal the frontier refuses (not listed) is not acted on, and the provider is not asked.
/// Kills the mutant that continues on a refused proposal.
#[test]
fn adversary_run_a_refused_proposal_does_not_continue() {
    let provider = StaticAuthorityProvider::new();
    let governor = Answer::at(4).with_items(
        Vec::new(),
        vec![open("tests pass")],
        vec![action("deploy", ActionStatus::Blocked, None)],
    );
    assert_eq!(
        derived(governor, proposal("delete-everything"), &provider),
        Derived::Ended(RunOutcome::NeedsExternalEvidence(
            RunOutcomeNeedsExternalEvidence {
                requirements: vec!["tests pass".to_owned()],
            }
        ))
    );
    assert!(
        provider.asked().is_empty(),
        "a refused proposal asks nobody"
    );
}

/// Rule 1 of `derive`: the governor's completion wins whatever the executor returned, a
/// `Suspended` included. Kills the mutant that reads the executor before the governor.
#[test]
fn adversary_run_governor_completion_wins_over_an_executor_suspension() {
    let provider = StaticAuthorityProvider::new();
    assert_eq!(
        derived(
            Answer::at(4).complete("X"),
            ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                reason: SuspensionReason::Evidence(vec!["log".to_owned()]),
            }),
            &provider
        ),
        Derived::Ended(RunOutcome::Completed(RunOutcomeCompleted {
            outcome: "X".to_owned(),
        }))
    );
}

fn counter_store() -> Generated<RunStore> {
    let mut issued = 0u64;
    Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x200 + issued))
    }))
}

fn start(runs: &mut Generated<RunStore>, revision: i64) -> RunId {
    let StartRunOutcome::Started { run_started } = runs
        .start_run(StartRun {
            commission_id: CommissionId(uuid(1)),
            case_revision: revision,
        })
        .expect("start");
    run_started.run_id
}

/// Two runs are two runs: suspending one leaves the other running, and resume finds the one it
/// names. Kills a store that keeps every run under one key, which the one-run acceptance cannot see.
#[test]
fn adversary_run_two_runs_are_kept_apart() {
    let mut runs = counter_store();
    let first = start(&mut runs, 3);
    let second = start(&mut runs, 5);
    assert_ne!(first, second);

    assert!(matches!(
        runs.suspend_run(SuspendRun {
            run_id: first.clone(),
            reason: SuspensionReason::Evidence(Vec::new()),
        }),
        Ok(SuspendRunOutcome::Suspended { .. })
    ));
    let mut rows = runs.run_states().expect("RunStates");
    rows.sort_by_key(|row| row.case_revision);
    let seen: Vec<(RunId, i64, RunState)> = rows
        .into_iter()
        .map(|row| (row.run_id, row.case_revision, row.state))
        .collect();
    assert_eq!(
        seen,
        vec![
            (first.clone(), 3, RunState::Suspended),
            (second.clone(), 5, RunState::Running),
        ]
    );
    assert!(matches!(
        runs.resume_run(ResumeRun { run_id: second }),
        Ok(ResumeRunOutcome::WrongState { .. })
    ));
    assert!(matches!(
        runs.resume_run(ResumeRun { run_id: first }),
        Ok(ResumeRunOutcome::Resumed { .. })
    ));
}

/// Suspend and resume of a run nobody started create nothing.
#[test]
fn adversary_run_unknown_run_is_not_created() {
    let mut runs = counter_store();
    assert_eq!(
        runs.resume_run(ResumeRun {
            run_id: RunId(uuid(0xdead)),
        }),
        Ok(ResumeRunOutcome::WrongStateUnknownInstance)
    );
    assert_eq!(
        runs.suspend_run(SuspendRun {
            run_id: RunId(uuid(0xdead)),
            reason: SuspensionReason::Evidence(Vec::new()),
        }),
        Ok(SuspendRunOutcome::WrongStateUnknownInstance)
    );
    assert_eq!(runs.run_states().expect("RunStates"), Vec::new());
}

/// An id source that repeats a stored run's id must not replace that run: the suspended run stays
/// suspended at its revision. The store documents a panic for this; the test holds the effect, not
/// the mechanism. Kills the mutant that drops the guard, after which start replaces the run.
#[test]
fn adversary_run_a_repeated_run_id_does_not_replace_the_stored_run() {
    let fixed = Arc::new(Mutex::new(RunId(uuid(0x300))));
    let source = Arc::clone(&fixed);
    let mut runs = Generated::new(RunStore::new(move || source.lock().expect("id").clone()));
    let run_id = start(&mut runs, 7);
    assert!(matches!(
        runs.suspend_run(SuspendRun {
            run_id: run_id.clone(),
            reason: SuspensionReason::Evidence(Vec::new()),
        }),
        Ok(SuspendRunOutcome::Suspended { .. })
    ));

    let replaced = catch_unwind(AssertUnwindSafe(|| {
        runs.start_run(StartRun {
            commission_id: CommissionId(uuid(9)),
            case_revision: 99,
        })
    }));
    assert!(
        !matches!(replaced, Ok(Ok(StartRunOutcome::Started { .. }))),
        "a second run started under a stored run's id"
    );
    let held = RunStorage::get(&runs.ports, &run_id).expect("the run is still stored");
    assert_eq!(
        (held.state, held.data.case_revision, held.data.commission_id),
        (RunState::Suspended, 7, CommissionId(uuid(1)))
    );
}
