//! `story:control-plane-storage`: a durable run store reports a failed write as the declared
//! `storage-failed` outcome of `StartRun` and `SuspendRun`
//! (`ess/commission/domains/responsibility.yaml`), and [`run_until_blocked`] reports it as
//! [`LoopFailure::RunStorage`], never as an unmet obligation.
//!
//! * `failed_durable_run_start_invokes_no_effect`: `StartRun` answers `storage-failed`, and the
//!   loop ends with no Run named before any executor, authority or effect call.
//! * `durable_run_suspend_failure_is_reported_not_obligation`: after a governor failure,
//!   `SuspendRun` answers `storage-failed`, and the error carries it as the suspension's failure,
//!   not as `NotSuspended` and not as an obligation.

use std::cell::Cell;

use b10x_loom_commission::model::behaviour::RunStorage;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::obligation::UnmetObligation;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::obligations::{
    StartRunBehavior, SuspendRunBehavior,
};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, GovernorError, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunState, RunStorageFailed, StartRun, StartRunOutcome,
    SuspendRun, SuspendRunOutcome, SuspensionReason, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, LoopFailure, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x71)),
        agent_revision_id: AgentRevisionId(uuid(0x72)),
        case_id: case.clone(),
        principal: PrincipalId("principal-durable".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn inspect_only(revision: i64) -> Answer {
    Answer::at(revision).with_items(
        Vec::new(),
        Vec::new(),
        vec![FrontierAction {
            action: "inspect".to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    )
}

fn proposal(name: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: name.to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    })
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x800 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x900 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-08T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// An effect port that performs every action, counting each `performs` and `invoke` call.
#[derive(Default)]
struct CountingEffects {
    asked: Cell<usize>,
    invoked: Cell<usize>,
}

impl EffectPort for CountingEffects {
    fn performs(&self, _action: &str) -> bool {
        self.asked.set(self.asked.get() + 1);
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.set(self.invoked.get() + 1);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Null,
            attempt: None,
            audit: None,
        }))
    }
}

/// A durable run store as a host writes one: it implements the `StartRun` and `SuspendRun`
/// obligations over its own storage (here memory) and answers `storage-failed` for the write it
/// is told cannot be made.
struct DurableRuns {
    store: RunStore,
    fail_start: bool,
    fail_suspend: bool,
    starts: usize,
    suspends: Vec<SuspendRun>,
}

impl DurableRuns {
    fn new(fail_start: bool, fail_suspend: bool) -> Self {
        let mut issued = 0u64;
        Self {
            store: RunStore::new(move || {
                issued += 1;
                RunId(uuid(0xa00 + issued))
            }),
            fail_start,
            fail_suspend,
            starts: 0,
            suspends: Vec::new(),
        }
    }
}

impl StartRunBehavior for DurableRuns {
    fn start_run(&mut self, input: StartRun) -> Result<StartRunOutcome, UnmetObligation> {
        self.starts += 1;
        if self.fail_start {
            return Ok(StartRunOutcome::StorageFailed {
                error: RunStorageFailed {
                    reason: "run table unreachable".to_owned(),
                },
            });
        }
        self.store.start_run(input)
    }
}

impl SuspendRunBehavior for DurableRuns {
    fn suspend_run(&mut self, input: SuspendRun) -> Result<SuspendRunOutcome, UnmetObligation> {
        self.suspends.push(input.clone());
        if self.fail_suspend {
            return Ok(SuspendRunOutcome::StorageFailed {
                error: RunStorageFailed {
                    reason: "suspension write timed out".to_owned(),
                },
            });
        }
        self.store.suspend_run(input)
    }
}

#[test]
fn failed_durable_run_start_invokes_no_effect() {
    let case = CaseId("case-durable-start-fails".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [inspect_only(4)]);
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let authority = StaticAuthorityProvider::new();
    let effects = CountingEffects::default();
    let mut runs = DurableRuns::new(true, false);

    let error = run_until_blocked(
        &governor,
        &executor,
        &authority,
        &effects,
        &commission(&case),
        &mut runs,
        &mut Context::default(),
    )
    .err()
    .unwrap_or_else(|| panic!("fixture: the run store cannot hold the new run"));

    assert_eq!(error.run_id, None, "no Run was started: {error:?}");
    assert_eq!(
        error.failure,
        LoopFailure::RunStorage(RunStorageFailed {
            reason: "run table unreachable".to_owned(),
        }),
        "a storage failure is reported as one"
    );
    assert!(
        !matches!(error.failure, LoopFailure::Obligation(_)),
        "no unmet obligation: {error:?}"
    );
    assert_eq!(error.suspension, None, "nothing to suspend: {error:?}");
    assert_eq!(
        error.suspension_reason, None,
        "nothing to suspend: {error:?}"
    );
    assert_eq!(runs.starts, 1, "StartRun asked once");
    assert!(
        runs.suspends.is_empty(),
        "no SuspendRun: {:?}",
        runs.suspends
    );
    assert!(
        RunStorage::list(&runs.store).is_empty(),
        "the store holds no run"
    );
    assert!(
        executor.calls().is_empty(),
        "executor called: {:?}",
        executor.calls()
    );
    assert!(
        authority.asked().is_empty(),
        "authority asked: {:?}",
        authority.asked()
    );
    assert_eq!(effects.asked.get(), 0, "effect port asked what it performs");
    assert_eq!(effects.invoked.get(), 0, "effect port invoked");
    assert_eq!(
        governor.calls(),
        vec![GovernorCall::CurrentRevision(case.clone())],
        "only the revision the Run would start at is read"
    );
    assert!(
        governor.observations().is_empty(),
        "no observation delivered: {:?}",
        governor.observations()
    );
    let shown = error.to_string();
    assert!(
        shown.contains("no run started") && shown.contains("run table unreachable"),
        "{shown}"
    );
}

#[test]
fn durable_run_suspend_failure_is_reported_not_obligation() {
    let case = CaseId("case-durable-suspend-fails".to_owned());
    let governor = FakeGovernor::new();
    // calls 1-3: start load, completion, frontier at 4; call 4 (revalidation): unavailable.
    governor.script(
        case.clone(),
        std::iter::repeat_n(inspect_only(4), 3).chain([Answer::unavailable()]),
    );
    let executor = ScriptedExecutor::new([proposal("inspect")]);
    let mut runs = DurableRuns::new(false, true);

    let error = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &CountingEffects::default(),
        &commission(&case),
        &mut runs,
        &mut Context::default(),
    )
    .err()
    .unwrap_or_else(|| panic!("fixture: the governor fails revalidation"));

    let held = RunStorage::list(&runs.store);
    assert_eq!(held.len(), 1, "fixture: the loop starts one Run: {held:?}");
    let run_id = held[0].data.run_id.clone();
    assert_eq!(error.run_id, Some(run_id.clone()));
    assert_eq!(
        error.failure,
        LoopFailure::Governor(GovernorError::GovernorUnavailable)
    );
    assert_eq!(runs.suspends.len(), 1, "SuspendRun asked once");
    assert_eq!(runs.suspends[0].run_id, run_id);
    assert!(
        matches!(
            runs.suspends[0].reason,
            SuspensionReason::ExternalAvailability(_)
        ),
        "suspended for availability: {:?}",
        runs.suspends[0].reason
    );
    assert_eq!(
        error.suspension.as_deref(),
        Some(&LoopFailure::RunStorage(RunStorageFailed {
            reason: "suspension write timed out".to_owned(),
        })),
        "the suspension's storage failure, not NotSuspended and not an obligation"
    );
    assert!(
        matches!(
            error.suspension_reason.as_deref(),
            Some(SuspensionReason::ExternalAvailability(_))
        ),
        "suspension reason: {:?}",
        error.suspension_reason
    );
    assert_eq!(
        held[0].state,
        RunState::Running,
        "the store could not record the suspension"
    );
    let shown = error.to_string();
    assert!(
        shown.contains(&run_id.0.0)
            && shown.contains("GovernorUnavailable")
            && shown.contains("suspension write timed out"),
        "{shown}"
    );
}
