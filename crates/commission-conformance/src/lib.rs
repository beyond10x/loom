//! The Rust ESS conformance target for `b10x-commission`.
//!
//! [`CommissionTarget`] is the `ess_conformance::ConformanceTarget` the suite synthesized from
//! `ess/` runs against, and [`run_suite`] runs one suite against it and returns the
//! `ess-conformance-report/2` document a caller reads the verdict from.
//!
//! Every command is answered by `b10x-commission`:
//!
//! * `StartRun`, `SuspendRun` and `ResumeRun` by the generated behaviours
//!   (`b10x_commission::model::behaviour::Generated`) over `b10x_commission::outcome::RunStore`;
//! * `RevalidateActionRequest` by `b10x_commission::action_request::revalidate`, against the
//!   governor in [`governor`];
//! * the view `RunStates` by the generated query over the same store.
//!
//! This crate translates values and records what was published. It decides no outcome: each
//! answer is the outcome the implementation returned, and each event is the one it returned.
//!
//! Each scenario starts from an empty store, an empty event log and no forced outcome, so no
//! observation of one scenario can satisfy another. Run ids and consistency tokens come from
//! counters, so two runs of one suite report the same thing.

pub mod codec;
pub mod governor;

use std::cell::RefCell;

use b10x_commission::action_request::revalidate;
use b10x_commission::model::behaviour::Generated;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::obligations::{
    ResumeRunBehavior, RunStatesQuery, StartRunBehavior, SuspendRunBehavior,
};
use b10x_commission::model::responsibility::{
    ActionRequest, ActionRequestData, ActionRequestId, CaseId, CommissionId,
    ProposedActionArguments, ResumeRun, ResumeRunOutcome, RevalidateActionRequestOutcome, RunId,
    RunStateConflict, StartRun, StartRunOutcome, SuspendRun, SuspendRunOutcome,
};
use b10x_commission::outcome::RunStore;
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

/// The name this implementation is reported under.
pub const IMPLEMENTATION: &str = "b10x-commission";

const START_RUN: &str = "commission.responsibility.StartRun";
const SUSPEND_RUN: &str = "commission.responsibility.SuspendRun";
const RESUME_RUN: &str = "commission.responsibility.ResumeRun";
const REVALIDATE: &str = "commission.responsibility.RevalidateActionRequest";
const RUN_STARTED: &str = "commission.responsibility.RunStarted";
const RUN_SUSPENDED: &str = "commission.responsibility.RunSuspended";
const RUN_RESUMED: &str = "commission.responsibility.RunResumed";
const RUN_STATE_CONFLICT: &str = "commission.responsibility.RunStateConflict";
const ACTION_REQUEST_STALE: &str = "commission.responsibility.ActionRequestStale";
const ACTION_NOT_ADMITTED: &str = "commission.responsibility.ActionNotAdmitted";
const ACTION_NEEDS_AUTHORITY: &str = "commission.responsibility.ActionNeedsAuthority";
const RUN_STATES: &str = "commission.responsibility.RunStates";

/// One scenario's state.
struct Live {
    /// The runs, behind the generated behaviours.
    runs: Generated<RunStore>,
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
            runs: Generated::new(RunStore::new(ids)),
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

/// The conformance target over `b10x-commission`.
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
            REVALIDATE => revalidate_request(&mut live, input)?,
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
        let condition = forced
            .strip_prefix(REVALIDATE)
            .and_then(|rest| rest.strip_prefix('/'))
            .and_then(Condition::forcing)
            .ok_or_else(|| {
                TargetError::unavailable(
                    format!("forcing `{forced}`"),
                    "the specification declares no such external outcome",
                )
            })?;
        self.live.borrow_mut().forced = Some(condition);
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
    }
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
    })
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
