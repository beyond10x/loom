//! The Rust ESS conformance target for `b10x-loom-commission`.
//!
//! [`CommissionTarget`] is the `ess_conformance::ConformanceTarget` the suite synthesized from
//! `ess/` runs against, and [`run_suite`] runs one suite against it and returns the
//! `ess-conformance-report/2` document a caller reads the verdict from.
//!
//! Every command is answered by `b10x-loom-commission`:
//!
//! * `StartRun`, `SuspendRun` and `ResumeRun` through the generated behaviours
//!   (`b10x_loom_commission::model::behaviour::Generated`) over `b10x_loom_commission::outcome::RunStore`,
//!   which answers all three itself, held in [`store`]: their `storage-failed`
//!   is the store failing the write a scenario forced;
//! * `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension` through the generated behaviours
//!   over `b10x_loom_commission::ports::suspension::SuspensionStore`, the suspension port over
//!   memory, held in [`store`]: their `storage-failed` is the store failing the write a scenario
//!   forced, and `HoldSuspension`'s `settled` is the host settling the hold a scenario forced;
//! * `RevalidateActionRequest` by `b10x_loom_commission::action_request::revalidate`, against the
//!   governor in [`governor`];
//! * `JudgeEffectRefusal` by `b10x_loom_commission::outcome::judge_refusal`, the judgement the
//!   runtime makes of every refusal an effect port answers;
//! * `JudgeIdleIteration` by `b10x_loom_commission::outcome::judge_idle`, the judgement the
//!   runtime makes of every iteration of a run with no step budget that made no progress;
//! * `JudgePerformableActions` by `b10x_loom_commission::outcome::judge_performable`, the
//!   judgement the runtime makes of a frontier before the executor runs and of an admitted
//!   request, when the effect port performs none of the actions the run needs;
//! * `JudgeBudgetStop` by `b10x_loom_commission::outcome::judge_budget_stop`, the judgement the
//!   runtime makes at every budget stop, `Waiting` while a request the effect port answered
//!   pending is still to be invoked again;
//! * the views `RunStates` and `SuspensionStates` by the generated queries over the same stores.
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the implementation returned, and each event is the one it returned.
//!
//! Each scenario starts from an empty store, an empty event log and no forced outcome, so no
//! observation of one scenario can satisfy another. Run ids and consistency tokens come from
//! counters, so two runs of one suite report the same thing.

pub mod codec;
pub mod governor;
pub mod store;

use std::cell::RefCell;

use b10x_loom_commission::action_request::revalidate;
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::obligations::{
    HoldSuspensionBehavior, ReleaseSuspensionBehavior, ResumeRunBehavior, RunStatesQuery,
    StartRunBehavior, SuspendRunBehavior, SuspensionStatesQuery, TakeSuspensionBehavior,
};
use b10x_loom_commission::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, CaseId, CommissionId, HoldSuspension,
    HoldSuspensionOutcome, JudgeBudgetStop, JudgeBudgetStopOutcome, JudgeEffectRefusal,
    JudgeEffectRefusalOutcome, JudgeIdleIteration, JudgeIdleIterationOutcome,
    JudgePerformableActions, JudgePerformableActionsOutcome, ProposedActionArguments,
    ReleaseSuspension, ReleaseSuspensionOutcome, ResumeRun, ResumeRunOutcome,
    RevalidateActionRequestOutcome, RunId, RunStateConflict, RunStorageFailed, StartRun,
    StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionRequestMismatch, SuspensionSettled,
    SuspensionStateConflict, SuspensionStorageFailed, TakeSuspension, TakeSuspensionOutcome,
};
use b10x_loom_commission::outcome::{
    RunStore, judge_budget_stop, judge_idle, judge_performable, judge_refusal,
};
use ess_conformance::scenario::{CommandRef, ErrorRef, EventRef, OutcomeRef};
use ess_conformance::target::{
    ConformanceTarget, DeclaredErrorValue, EventObservationRequest, ExternalOutcomeControl,
    ImplementationIdentity, ObservedEvent, RedeliveryRequest, ScenarioContext,
    SemanticCommandRequest, SemanticCommandResult, SemanticViewRequest, SemanticViewResult,
    TargetError, ViewRow,
};
use ess_conformance::{AdmittedSuite, CountReport, Runner};
use ess_primitives::consistency::ConsistencyToken;
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;

use crate::codec::Input;
use crate::governor::{Condition, ScenarioGovernor};
use crate::store::{ScenarioStore, ScenarioSuspensions, SuspensionUnwritable, Unwritable};

/// The name this implementation is reported under.
pub const IMPLEMENTATION: &str = "b10x-loom-commission";

const START_RUN: &str = "commission.responsibility.StartRun";
const SUSPEND_RUN: &str = "commission.responsibility.SuspendRun";
const RESUME_RUN: &str = "commission.responsibility.ResumeRun";
const REVALIDATE: &str = "commission.responsibility.RevalidateActionRequest";
const JUDGE_REFUSAL: &str = "commission.responsibility.JudgeEffectRefusal";
const EFFECT_REFUSED_FINAL: &str = "commission.responsibility.EffectRefusedFinal";
const JUDGE_IDLE: &str = "commission.responsibility.JudgeIdleIteration";
const IDLE_BOUND_REACHED: &str = "commission.responsibility.IdleBoundReached";
const JUDGE_PERFORMABLE: &str = "commission.responsibility.JudgePerformableActions";
const ACTIONS_UNPERFORMABLE: &str = "commission.responsibility.ActionsUnperformable";
const JUDGE_BUDGET_STOP: &str = "commission.responsibility.JudgeBudgetStop";
const REQUESTS_WAITING: &str = "commission.responsibility.RequestsWaiting";
const RUN_STARTED: &str = "commission.responsibility.RunStarted";
const RUN_SUSPENDED: &str = "commission.responsibility.RunSuspended";
const RUN_RESUMED: &str = "commission.responsibility.RunResumed";
const RUN_STATE_CONFLICT: &str = "commission.responsibility.RunStateConflict";
const ACTION_REQUEST_STALE: &str = "commission.responsibility.ActionRequestStale";
const ACTION_NOT_ADMITTED: &str = "commission.responsibility.ActionNotAdmitted";
const ACTION_NEEDS_AUTHORITY: &str = "commission.responsibility.ActionNeedsAuthority";
const RUN_STORAGE_FAILED: &str = "commission.responsibility.RunStorageFailed";
const STORAGE_FAILED: &str = "storage-failed";
const RUN_STATES: &str = "commission.responsibility.RunStates";
const HOLD_SUSPENSION: &str = "commission.responsibility.HoldSuspension";
const TAKE_SUSPENSION: &str = "commission.responsibility.TakeSuspension";
const RELEASE_SUSPENSION: &str = "commission.responsibility.ReleaseSuspension";
const SUSPENSION_HELD: &str = "commission.responsibility.SuspensionHeld";
const SUSPENSION_TAKEN: &str = "commission.responsibility.SuspensionTaken";
const SUSPENSION_RELEASED: &str = "commission.responsibility.SuspensionReleased";
const SUSPENSION_ALREADY_HELD: &str = "commission.responsibility.SuspensionAlreadyHeld";
const SUSPENSION_NOT_FOUND: &str = "commission.responsibility.SuspensionNotFound";
const SUSPENSION_STATE_CONFLICT: &str = "commission.responsibility.SuspensionStateConflict";
const SUSPENSION_REQUEST_MISMATCH: &str = "commission.responsibility.SuspensionRequestMismatch";
const SUSPENSION_STORAGE_FAILED: &str = "commission.responsibility.SuspensionStorageFailed";
const SUSPENSION_SETTLED: &str = "commission.responsibility.SuspensionSettled";
const SETTLED: &str = "settled";
const SUSPENSION_STATES: &str = "commission.responsibility.SuspensionStates";

/// One scenario's state.
struct Live {
    /// The runs, behind the generated behaviours.
    runs: Generated<ScenarioStore>,
    /// The Suspensions, behind the generated behaviours.
    suspensions: Generated<ScenarioSuspensions>,
    /// Every event published in this scenario, in order.
    log: Vec<ObservedEvent>,
    /// The external condition armed for the next revalidation.
    forced: Option<Condition>,
    /// The invocations performed, which consistency tokens are minted from.
    sequence: u64,
}

impl Live {
    fn new() -> Self {
        let mut next = 0_u64;
        let ids = move || {
            next += 1;
            RunId(Uuid(format!("00000000-0000-4000-9000-{next:012}")))
        };
        Self {
            runs: Generated::new(ScenarioStore::new(RunStore::new(ids))),
            suspensions: Generated::new(ScenarioSuspensions::new()),
            log: Vec::new(),
            forced: None,
            sequence: 0,
        }
    }

    fn token(&mut self) -> ConsistencyToken {
        self.sequence += 1;
        ConsistencyToken::new(format!("seq:{}", self.sequence))
            .expect("`seq:` and decimal digits is a well-formed token")
    }

    /// Publishes `event`, returning it for the command's own result.
    fn publish(&mut self, event: ObservedEvent, correlation: &CorrelationId) -> ObservedEvent {
        let sequence = u64::try_from(self.log.len()).expect("the log fits") + 1;
        let event = event.in_activity(correlation.clone()).at(sequence);
        self.log.push(event.clone());
        event
    }
}

/// The conformance target over `b10x-loom-commission`.
pub struct CommissionTarget {
    live: RefCell<Live>,
}

impl Default for CommissionTarget {
    fn default() -> Self {
        Self {
            live: RefCell::new(Live::new()),
        }
    }
}

impl std::fmt::Debug for CommissionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommissionTarget").finish_non_exhaustive()
    }
}

impl ConformanceTarget for CommissionTarget {
    fn identity(&self) -> Result<ImplementationIdentity, TargetError> {
        Ok(ImplementationIdentity::new(
            IMPLEMENTATION,
            env!("CARGO_PKG_VERSION"),
        ))
    }

    fn begin_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.live.borrow_mut() = Live::new();
        Ok(())
    }

    fn execute_command(
        &self,
        request: SemanticCommandRequest,
    ) -> Result<SemanticCommandResult, TargetError> {
        if request.caller.is_some() {
            return Err(TargetError::unsupported(
                format!("sending `{}` as a caller", request.command),
                "the specification declares no caller attribute, so this target holds no credential",
            ));
        }
        let mut live = self.live.borrow_mut();
        let command = request.command.to_string();
        let input = &request.input;
        let correlation = &request.correlation;
        let result = match command.as_str() {
            START_RUN => start_run(&mut live, input, correlation),
            SUSPEND_RUN => suspend_run(&mut live, input, correlation),
            RESUME_RUN => resume_run(&mut live, input, correlation),
            HOLD_SUSPENSION => hold_suspension(&mut live, input, correlation),
            TAKE_SUSPENSION => take_suspension(&mut live, input, correlation),
            RELEASE_SUSPENSION => release_suspension(&mut live, input, correlation),
            REVALIDATE => revalidate_request(&mut live, input)?,
            JUDGE_REFUSAL => judge_effect_refusal(input)?,
            JUDGE_IDLE => judge_idle_iteration(input)?,
            JUDGE_PERFORMABLE => judge_performable_actions(input)?,
            JUDGE_BUDGET_STOP => judge_budget_stop_command(input)?,
            other => {
                return Err(TargetError::unavailable(
                    format!("invoking `{other}`"),
                    "the specification declares no such command",
                ));
            }
        };
        Ok(match result {
            Some(result) => result.with_consistency(live.token()),
            None => SemanticCommandResult::undeclared(),
        })
    }

    fn query_view(&self, request: SemanticViewRequest) -> Result<SemanticViewResult, TargetError> {
        let view = request.view.to_string();
        if view == SUSPENSION_STATES {
            return suspension_states(&self.live.borrow(), &view);
        }
        if view != RUN_STATES {
            return Err(TargetError::unavailable(
                format!("reading `{view}`"),
                "the specification declares no such view",
            ));
        }
        let live = self.live.borrow();
        let rows = live.runs.run_states().map_err(|unmet| {
            TargetError::unavailable(
                format!("reading `{view}`"),
                format!("{} is owed: {}", unmet.capability, unmet.source),
            )
        })?;
        Ok(SemanticViewResult::of(rows.into_iter().map(|row| {
            ViewRow::from([
                ("run_id".to_owned(), Node::Text(row.run_id.0.0)),
                (
                    "commission_id".to_owned(),
                    Node::Text(row.commission_id.0.0),
                ),
                ("case_revision".to_owned(), codec::number(row.case_revision)),
                ("state".to_owned(), codec::run_state(row.state)),
            ])
        })))
    }

    fn observe_events(
        &self,
        request: EventObservationRequest,
    ) -> Result<Vec<ObservedEvent>, TargetError> {
        Ok(self
            .live
            .borrow()
            .log
            .iter()
            .filter(|published| published.event == request.event)
            .cloned()
            .collect())
    }

    fn configure_external_outcome(
        &self,
        request: ExternalOutcomeControl,
    ) -> Result<(), TargetError> {
        let forced = request.force.to_string();
        let undeclared = || {
            TargetError::unavailable(
                format!("forcing `{forced}`"),
                "the specification declares no such external outcome",
            )
        };
        let (command, outcome) = forced.split_once('/').ok_or_else(undeclared)?;
        let mut live = self.live.borrow_mut();
        match (command, outcome) {
            (REVALIDATE, outcome) => {
                live.forced = Some(Condition::forcing(outcome).ok_or_else(undeclared)?);
            }
            (START_RUN, STORAGE_FAILED) => live.runs.ports.fail_next(Unwritable::StartRun),
            (SUSPEND_RUN, STORAGE_FAILED) => live.runs.ports.fail_next(Unwritable::SuspendRun),
            (RESUME_RUN, STORAGE_FAILED) => live.runs.ports.fail_next(Unwritable::ResumeRun),
            (HOLD_SUSPENSION, STORAGE_FAILED) => {
                live.suspensions.ports.fail_next(SuspensionUnwritable::Hold)
            }
            (HOLD_SUSPENSION, SETTLED) => live.suspensions.ports.settle_next(),
            (TAKE_SUSPENSION, STORAGE_FAILED) => {
                live.suspensions.ports.fail_next(SuspensionUnwritable::Take)
            }
            (RELEASE_SUSPENSION, STORAGE_FAILED) => live
                .suspensions
                .ports
                .fail_next(SuspensionUnwritable::Release),
            _ => return Err(undeclared()),
        }
        Ok(())
    }

    fn redeliver_event(&self, request: RedeliveryRequest) -> Result<(), TargetError> {
        Err(TargetError::unsupported(
            format!("redelivering `{}`", request.event),
            "the specification declares no binding that reacts to an event",
        ))
    }

    fn end_scenario(&self, _scenario: &ScenarioContext) -> Result<(), TargetError> {
        *self.live.borrow_mut() = Live::new();
        Ok(())
    }
}

fn parse<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Display,
{
    text.parse()
        .unwrap_or_else(|error| panic!("`{text}` is a well-formed reference: {error}"))
}

fn took(command: &str, outcome: &str) -> SemanticCommandResult {
    SemanticCommandResult::took(OutcomeRef::new(
        parse::<CommandRef>(command),
        parse(outcome),
    ))
}

fn error(name: &str) -> DeclaredErrorValue {
    DeclaredErrorValue::new(parse::<ErrorRef>(name))
}

fn event(name: &str) -> ObservedEvent {
    ObservedEvent::new(parse::<EventRef>(name))
}

/// `wrong-state` with the run's actual state, or with no field for a run no record carries.
fn wrong_state(command: &str, conflict: Option<RunStateConflict>) -> SemanticCommandResult {
    let declared = error(RUN_STATE_CONFLICT);
    took(command, "wrong-state").with_error(match conflict {
        Some(conflict) => declared.with("state", codec::run_state(conflict.state)),
        None => declared,
    })
}

fn start_run(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = StartRun {
        commission_id: CommissionId(codec::uuid(input, "commission_id")?),
        case_revision: codec::integer(input, "case_revision")?,
    };
    match live.runs.start_run(command).ok()? {
        StartRunOutcome::Started { run_started } => {
            let published = live.publish(
                event(RUN_STARTED)
                    .with("run_id", Node::Text(run_started.run_id.0.0))
                    .with("commission_id", Node::Text(run_started.commission_id.0.0))
                    .with("case_revision", codec::number(run_started.case_revision)),
                correlation,
            );
            Some(took(START_RUN, "started").emitting(published))
        }
        StartRunOutcome::StorageFailed { error } => Some(storage_failed(START_RUN, error)),
    }
}

/// `storage-failed` with the store's reason.
fn storage_failed(command: &str, failed: RunStorageFailed) -> SemanticCommandResult {
    took(command, STORAGE_FAILED)
        .with_error(error(RUN_STORAGE_FAILED).with("reason", Node::Text(failed.reason)))
}

fn suspend_run(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = SuspendRun {
        run_id: RunId(codec::uuid(input, "run_id")?),
        reason: codec::suspension_reason(input.get("reason")?)?,
    };
    Some(match live.runs.suspend_run(command).ok()? {
        SuspendRunOutcome::Suspended { run_suspended } => {
            let published = live.publish(
                event(RUN_SUSPENDED)
                    .with("run_id", Node::Text(run_suspended.run_id.0.0))
                    .with(
                        "reason",
                        codec::from_suspension_reason(&run_suspended.reason)?,
                    ),
                correlation,
            );
            took(SUSPEND_RUN, "suspended").emitting(published)
        }
        SuspendRunOutcome::WrongState { error } => wrong_state(SUSPEND_RUN, Some(error)),
        SuspendRunOutcome::WrongStateUnknownInstance => wrong_state(SUSPEND_RUN, None),
        SuspendRunOutcome::StorageFailed { error } => storage_failed(SUSPEND_RUN, error),
    })
}

fn resume_run(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ResumeRun {
        run_id: RunId(codec::uuid(input, "run_id")?),
    };
    Some(match live.runs.resume_run(command).ok()? {
        ResumeRunOutcome::Resumed { run_resumed } => {
            let published = live.publish(
                event(RUN_RESUMED).with("run_id", Node::Text(run_resumed.run_id.0.0)),
                correlation,
            );
            took(RESUME_RUN, "resumed").emitting(published)
        }
        ResumeRunOutcome::WrongState { error } => wrong_state(RESUME_RUN, Some(error)),
        ResumeRunOutcome::WrongStateUnknownInstance => wrong_state(RESUME_RUN, None),
        ResumeRunOutcome::StorageFailed { error } => storage_failed(RESUME_RUN, error),
    })
}

/// `storage-failed` of a Suspension command, with the store's reason.
fn suspension_storage_failed(
    command: &str,
    failed: SuspensionStorageFailed,
) -> SemanticCommandResult {
    took(command, STORAGE_FAILED)
        .with_error(error(SUSPENSION_STORAGE_FAILED).with("reason", Node::Text(failed.reason)))
}

/// `HoldSuspension`'s `settled`: the host already settled the request, naming the Suspension, the
/// request and why; nothing was stored.
fn suspension_settled(settled: SuspensionSettled) -> SemanticCommandResult {
    took(HOLD_SUSPENSION, SETTLED).with_error(
        error(SUSPENSION_SETTLED)
            .with(
                "suspension",
                codec::from_suspension_key(&settled.suspension),
            )
            .with(
                "action_request_id",
                Node::Text(settled.action_request_id.0.0),
            )
            .with("reason", Node::Text(settled.reason)),
    )
}

/// A Suspension command's refusal naming the state the Suspension is in and its request.
fn suspension_conflict(
    command: &str,
    outcome: &str,
    conflict: SuspensionStateConflict,
) -> SemanticCommandResult {
    took(command, outcome).with_error(
        error(SUSPENSION_STATE_CONFLICT)
            .with("state", codec::suspension_state(conflict.state))
            .with(
                "action_request_id",
                Node::Text(conflict.action_request_id.0.0),
            ),
    )
}

/// A take's or a release's `other-request` refusal, naming the Suspension, the request it is held
/// for and the one the command named.
fn suspension_other_request(
    command: &str,
    mismatch: SuspensionRequestMismatch,
) -> SemanticCommandResult {
    took(command, "other-request").with_error(
        error(SUSPENSION_REQUEST_MISMATCH)
            .with(
                "suspension",
                codec::from_suspension_key(&mismatch.suspension),
            )
            .with(
                "held_action_request_id",
                Node::Text(mismatch.held_action_request_id.0.0),
            )
            .with(
                "action_request_id",
                Node::Text(mismatch.action_request_id.0.0),
            ),
    )
}

fn hold_suspension(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = HoldSuspension {
        suspension: codec::suspension_key(input.get("suspension")?)?,
        commission_id: CommissionId(codec::uuid(input, "commission_id")?),
        run_id: RunId(codec::uuid(input, "run_id")?),
        run_case_revision: codec::integer(input, "run_case_revision")?,
        action_request_id: ActionRequestId(codec::uuid(input, "action_request_id")?),
        case_id: CaseId(codec::text(input, "case_id")?),
        expected_case_revision: codec::integer(input, "expected_case_revision")?,
        action: codec::text(input, "action")?,
        arguments: ProposedActionArguments(codec::json(input, "arguments")?),
        reason: codec::suspension_reason(input.get("reason")?)?,
    };
    Some(match live.suspensions.hold_suspension(command).ok()? {
        HoldSuspensionOutcome::Settled { error: settled } => suspension_settled(settled),
        HoldSuspensionOutcome::Held { suspension_held } => {
            let published = live.publish(
                event(SUSPENSION_HELD)
                    .with(
                        "suspension",
                        codec::from_suspension_key(&suspension_held.suspension),
                    )
                    .with("run_id", Node::Text(suspension_held.run_id.0.0))
                    .with(
                        "action_request_id",
                        Node::Text(suspension_held.action_request_id.0.0),
                    ),
                correlation,
            );
            took(HOLD_SUSPENSION, "held").emitting(published)
        }
        HoldSuspensionOutcome::AlreadyHeld { error: held } => took(HOLD_SUSPENSION, "already-held")
            .with_error(
                error(SUSPENSION_ALREADY_HELD)
                    .with("suspension", codec::from_suspension_key(&held.suspension)),
            ),
        HoldSuspensionOutcome::StorageFailed { error } => {
            suspension_storage_failed(HOLD_SUSPENSION, error)
        }
    })
}

fn take_suspension(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = TakeSuspension {
        suspension: codec::suspension_key(input.get("suspension")?)?,
        action_request_id: ActionRequestId(codec::uuid(input, "action_request_id")?),
    };
    Some(match live.suspensions.take_suspension(command).ok()? {
        TakeSuspensionOutcome::OtherRequest { error } => {
            suspension_other_request(TAKE_SUSPENSION, error)
        }
        TakeSuspensionOutcome::Taken { suspension_taken } => {
            let published = live.publish(
                event(SUSPENSION_TAKEN)
                    .with(
                        "suspension",
                        codec::from_suspension_key(&suspension_taken.suspension),
                    )
                    .with(
                        "action_request_id",
                        Node::Text(suspension_taken.action_request_id.0.0),
                    ),
                correlation,
            );
            took(TAKE_SUSPENSION, "taken").emitting(published)
        }
        TakeSuspensionOutcome::NotFound { error: missing } => took(TAKE_SUSPENSION, "not-found")
            .with_error(error(SUSPENSION_NOT_FOUND).with(
                "suspension",
                codec::from_suspension_key(&missing.suspension),
            )),
        TakeSuspensionOutcome::WrongState { error } => {
            suspension_conflict(TAKE_SUSPENSION, "wrong-state", error)
        }
        TakeSuspensionOutcome::StorageFailed { error } => {
            suspension_storage_failed(TAKE_SUSPENSION, error)
        }
    })
}

fn release_suspension(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = ReleaseSuspension {
        suspension: codec::suspension_key(input.get("suspension")?)?,
        action_request_id: ActionRequestId(codec::uuid(input, "action_request_id")?),
    };
    Some(match live.suspensions.release_suspension(command).ok()? {
        ReleaseSuspensionOutcome::OtherRequest { error } => {
            suspension_other_request(RELEASE_SUSPENSION, error)
        }
        ReleaseSuspensionOutcome::Released {
            suspension_released,
        } => {
            let published = live.publish(
                event(SUSPENSION_RELEASED).with(
                    "suspension",
                    codec::from_suspension_key(&suspension_released.suspension),
                ),
                correlation,
            );
            took(RELEASE_SUSPENSION, "released").emitting(published)
        }
        ReleaseSuspensionOutcome::NotTaken { error } => {
            suspension_conflict(RELEASE_SUSPENSION, "not-taken", error)
        }
        ReleaseSuspensionOutcome::NotFound { error: missing } => {
            took(RELEASE_SUSPENSION, "not-found").with_error(error(SUSPENSION_NOT_FOUND).with(
                "suspension",
                codec::from_suspension_key(&missing.suspension),
            ))
        }
        ReleaseSuspensionOutcome::StorageFailed { error } => {
            suspension_storage_failed(RELEASE_SUSPENSION, error)
        }
    })
}

/// The view `SuspensionStates`, by the generated query over the suspension store.
fn suspension_states(live: &Live, view: &str) -> Result<SemanticViewResult, TargetError> {
    let rows = live.suspensions.suspension_states().map_err(|unmet| {
        TargetError::unavailable(
            format!("reading `{view}`"),
            format!("{} is owed: {}", unmet.capability, unmet.source),
        )
    })?;
    Ok(SemanticViewResult::of(rows.into_iter().map(|row| {
        ViewRow::from([
            (
                "suspension".to_owned(),
                codec::from_suspension_key(&row.suspension),
            ),
            (
                "commission_id".to_owned(),
                Node::Text(row.commission_id.0.0),
            ),
            ("run_id".to_owned(), Node::Text(row.run_id.0.0)),
            (
                "action_request_id".to_owned(),
                Node::Text(row.action_request_id.0.0),
            ),
            ("state".to_owned(), codec::suspension_state(row.state)),
        ])
    })))
}

/// `RevalidateActionRequest`: the request as the input names it, revalidated against the
/// governor the armed external condition describes. The condition lapses after this invocation.
fn revalidate_request(
    live: &mut Live,
    input: &Input,
) -> Result<Option<SemanticCommandResult>, TargetError> {
    let condition = live.forced.take().unwrap_or(Condition::Agrees);
    let Some(data) = action_request(input) else {
        return Ok(None);
    };
    let governor = ScenarioGovernor::new(
        data.case_id.clone(),
        data.expected_case_revision,
        data.action.clone(),
        condition,
    );
    let outcome = revalidate(&governor, &ActionRequest::new(data)).map_err(|error| {
        TargetError::unavailable(
            format!("revalidating through `{REVALIDATE}`"),
            format!("the governor could not answer: {error:?}"),
        )
    })?;
    Ok(Some(match outcome {
        RevalidateActionRequestOutcome::Stale { error: stale } => took(REVALIDATE, "stale")
            .with_error(
                error(ACTION_REQUEST_STALE)
                    .with(
                        "expected_case_revision",
                        codec::number(stale.expected_case_revision),
                    )
                    .with(
                        "current_case_revision",
                        codec::number(stale.current_case_revision),
                    ),
            ),
        RevalidateActionRequestOutcome::NotAdmitted { error: refused } => {
            took(REVALIDATE, "not-admitted").with_error(
                error(ACTION_NOT_ADMITTED)
                    .with("action", Node::Text(refused.action))
                    .with("reasons", codec::texts(&refused.reasons)),
            )
        }
        RevalidateActionRequestOutcome::NeedsAuthority { error: needs } => {
            took(REVALIDATE, "needs-authority").with_error(
                error(ACTION_NEEDS_AUTHORITY)
                    .with("action", Node::Text(needs.action))
                    .with("capability", Node::Text(needs.capability)),
            )
        }
        RevalidateActionRequestOutcome::Admitted => took(REVALIDATE, "admitted"),
    }))
}

/// `JudgeEffectRefusal`: the refusal the input describes, judged as the runtime judges an effect
/// port's refusal. It reads and writes no store, so every view stays as it was.
fn judge_effect_refusal(input: &Input) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(command) = refusal(input) else {
        return Ok(None);
    };
    let outcome = judge_refusal(command).map_err(|unmet| {
        TargetError::unavailable(
            format!("judging through `{JUDGE_REFUSAL}`"),
            format!("{} is owed: {}", unmet.capability, unmet.source),
        )
    })?;
    Ok(Some(match outcome {
        JudgeEffectRefusalOutcome::Final { error: refused } => took(JUDGE_REFUSAL, "final")
            .with_error(
                error(EFFECT_REFUSED_FINAL)
                    .with(
                        "action_request_id",
                        Node::Text(refused.action_request_id.0.0),
                    )
                    .with("reason", Node::Text(refused.reason)),
            ),
        JudgeEffectRefusalOutcome::Ordinary => took(JUDGE_REFUSAL, "ordinary"),
    }))
}

/// The input of `JudgeEffectRefusal`.
fn refusal(input: &Input) -> Option<JudgeEffectRefusal> {
    Some(JudgeEffectRefusal {
        action_request_id: ActionRequestId(codec::uuid(input, "action_request_id")?),
        reason: codec::text(input, "reason")?,
        ends_run: codec::boolean(input, "ends_run")?,
    })
}

/// `JudgeIdleIteration`: the idle iterations in a row the input counts, judged as the runtime
/// judges a run with no step budget after an iteration that made no progress. It reads and writes
/// no store, so every view stays as it was.
fn judge_idle_iteration(input: &Input) -> Result<Option<SemanticCommandResult>, TargetError> {
    let Some(idle_iterations) = codec::integer(input, "idle_iterations") else {
        return Ok(None);
    };
    let Some(bound) = codec::integer(input, "bound") else {
        return Ok(None);
    };
    let outcome = judge_idle(JudgeIdleIteration {
        idle_iterations,
        bound,
    })
    .map_err(|unmet| {
        TargetError::unavailable(
            format!("judging through `{JUDGE_IDLE}`"),
            format!("{} is owed: {}", unmet.capability, unmet.source),
        )
    })?;
    Ok(Some(match outcome {
        JudgeIdleIterationOutcome::Bound { error: reached } => took(JUDGE_IDLE, "bound")
            .with_error(
                error(IDLE_BOUND_REACHED)
                    .with("idle_iterations", codec::number(reached.idle_iterations)),
            ),
        JudgeIdleIterationOutcome::Below => took(JUDGE_IDLE, "below"),
    }))
}

/// `JudgePerformableActions`: the actions the input counts and lists, judged as the runtime judges
/// a frontier before the executor runs and an admitted request whose action the effect port may
/// not perform. It reads and writes no store, so every view stays as it was.
fn judge_performable_actions(input: &Input) -> Result<Option<SemanticCommandResult>, TargetError> {
    let (Some(performed), Some(unperformed), Some(open_claims)) = (
        codec::integer(input, "performed"),
        codec::text_list(input, "unperformed"),
        codec::text_list(input, "open_claims"),
    ) else {
        return Ok(None);
    };
    let outcome = judge_performable(JudgePerformableActions {
        performed,
        unperformed,
        open_claims,
    })
    .map_err(|unmet| {
        TargetError::unavailable(
            format!("judging through `{JUDGE_PERFORMABLE}`"),
            format!("{} is owed: {}", unmet.capability, unmet.source),
        )
    })?;
    Ok(Some(match outcome {
        JudgePerformableActionsOutcome::Unperformable { error: named } => {
            took(JUDGE_PERFORMABLE, "unperformable").with_error(
                error(ACTIONS_UNPERFORMABLE)
                    .with("actions", codec::texts(&named.actions))
                    .with("open_claims", codec::texts(&named.open_claims)),
            )
        }
        JudgePerformableActionsOutcome::Performable => took(JUDGE_PERFORMABLE, "performable"),
    }))
}

/// `JudgeBudgetStop`: the pending requests and the budget's reason the input names, judged as the
/// runtime judges every budget stop. It reads and writes no store, so every view stays as it was.
fn judge_budget_stop_command(input: &Input) -> Result<Option<SemanticCommandResult>, TargetError> {
    let (Some(pending), Some(reason)) = (
        codec::uuid_list(input, "pending"),
        input.get("reason").and_then(codec::suspension_reason),
    ) else {
        return Ok(None);
    };
    let outcome = judge_budget_stop(JudgeBudgetStop {
        pending: pending.into_iter().map(ActionRequestId).collect(),
        reason,
    })
    .map_err(|unmet| {
        TargetError::unavailable(
            format!("judging through `{JUDGE_BUDGET_STOP}`"),
            format!("{} is owed: {}", unmet.capability, unmet.source),
        )
    })?;
    Ok(match outcome {
        JudgeBudgetStopOutcome::Waiting { error: waiting } => {
            let Some(reason) = codec::from_suspension_reason(&waiting.reason) else {
                return Ok(None);
            };
            let ids = waiting
                .action_request_ids
                .into_iter()
                .map(|id| Node::Text(id.0.0))
                .collect();
            Some(
                took(JUDGE_BUDGET_STOP, "waiting").with_error(
                    error(REQUESTS_WAITING)
                        .with("action_request_ids", Node::Seq(ids))
                        .with("reason", reason),
                ),
            )
        }
        JudgeBudgetStopOutcome::Suspended => Some(took(JUDGE_BUDGET_STOP, "suspended")),
    })
}

/// The action request the input of `RevalidateActionRequest` names, identity included.
fn action_request(input: &Input) -> Option<ActionRequestData> {
    Some(ActionRequestData {
        action_request_id: ActionRequestId(codec::uuid(input, "action_request_id")?),
        run_id: RunId(codec::uuid(input, "run_id")?),
        case_id: CaseId(codec::text(input, "case_id")?),
        expected_case_revision: codec::integer(input, "expected_case_revision")?,
        action: codec::text(input, "action")?,
        arguments: ProposedActionArguments(codec::json(input, "arguments")?),
    })
}

/// One run of a suite against [`CommissionTarget`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executed {
    /// `ess-conformance-report/2`, canonical: the document the verdict is read from.
    pub report: String,
    /// The runner's per-scenario results with their diagnostics, for a reader of a red run.
    pub diagnostics: String,
}

/// Admit `suite` (the JSON `ess verify conform synthesize` writes), run it against
/// [`CommissionTarget`] and return the report.
///
/// # Errors
///
/// Returns the admission failure when the bytes are not an admissible suite, and the report
/// failure when the run and the suite disagree.
pub fn run_suite(suite: &str) -> Result<Executed, String> {
    let admitted = AdmittedSuite::from_json(suite).map_err(|error| error.to_string())?;
    let target = CommissionTarget::default();
    let executed = Runner::for_suite(admitted.suite()).run_admitted(&admitted, &target);
    let report = CountReport::from_run(&executed, &admitted)
        .and_then(|report| report.to_canonical_json())
        .map_err(|error| error.to_string())?;
    let diagnostics =
        serde_json::to_string_pretty(&*executed).map_err(|error| error.to_string())?;
    Ok(Executed {
        report,
        diagnostics,
    })
}
