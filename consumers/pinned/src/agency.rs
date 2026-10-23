//! An agent defined as data, run through `loom_sdk` alone.
//!
//! [`SPEC`] is a `loom.agent/1` document; a host decodes it with `loom_sdk::agent::parse`. [`run`]
//! maps the spec's `session.budget` and `session.stop` onto Commission's `RunBudget` and
//! `StopPolicy` (the `loom.agent` domain mirrors both field for field) and keeps them as the
//! agent's [`Limits`]; ids, the clock and the host's control (`NoEvents`, no host stop) are the
//! host's own parts of the loop context. It opens a case on `system-query@1` in a memory case
//! store and runs one commission through a [`Runtime`].
//!
//! The executor answers what a recorded agent answered: read the clock. The effect port performs
//! that read and reports a recorded reading. It submits no evidence, because a report is not
//! evidence, so the case never completes and only the spec's limits end the run. No model, network
//! or harness program is reached.

use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

use loom_sdk::agent::{self, AgentSpec};
use loom_sdk::commission::model::behaviour::Generated;
use loom_sdk::commission::model::json;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionBudget, ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, BreachStep, BudgetLimit, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, LimitBreachStep, ObservationId,
    PrincipalId, ProposedActionArguments, RunBudget, RunId, StopPolicy, commission_state,
    frontier_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError};
use loom_sdk::commission::ports::executor::AgentExecutor;
use loom_sdk::{
    CanonGovernor, CaseInputs, Clock, EffectPort, EventSink, HostControl, Ids, Limits, LoopEnd,
    MemoryCaseStore, NoEvents, ProtocolCatalog, Runtime,
};

/// The agent, as data.
pub const SPEC: &str = include_str!("../agents/clock-reader.yaml");

/// The protocol the agent's case is opened on.
pub const PROTOCOL: &str = "system-query@1";

/// The one action the recorded agent proposes.
pub const ACTION: &str = "system.time.read";

/// The recorded clock reading the effect port reports.
const READING: &str = r#"{"local":"2026-01-01T01:00:00+01:00","utc":"2026-01-01T00:00:00Z"}"#;

/// Runs `spec`'s agent on one `system-query@1` case until the runtime stops it, returning how the
/// loop ended.
///
/// # Errors
/// When the catalog or the case cannot be opened, or the runtime fails without a run outcome.
pub fn run(spec: &AgentSpec) -> Result<LoopEnd, Box<dyn Error>> {
    let catalog = ProtocolCatalog::bundled()?;
    let governor = CanonGovernor::new(MemoryCaseStore::default()).with_catalog(&catalog)?;
    let case = governor.open(
        PROTOCOL,
        [("intent".to_owned(), "what-time-is-it".to_owned())].into(),
        CaseInputs::none(),
    )?;

    let context = Host {
        limits: AgentLimits::of(spec),
        events: NoEvents,
    };
    let runs = Generated::new(RunStore::new(|| RunId(fresh(Kind::Run))));
    let mut runtime = Runtime::new(
        &governor,
        &RecordedAgent,
        &NoGrant,
        &RecordedClockRead,
        runs,
        context,
    );
    Ok(runtime.run(&commission(&case))?)
}

/// The limits one agent's spec declares, as Commission's types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLimits {
    /// The spec's `session.budget`; none without a session.
    pub budget: Option<RunBudget>,
    /// The spec's `session.stop`; Commission's default without a session.
    pub stop: Option<StopPolicy>,
}

impl AgentLimits {
    /// The limits `spec` declares.
    pub fn of(spec: &AgentSpec) -> Self {
        let session = spec.session.as_ref();
        Self {
            budget: session.map(|session| run_budget(&session.budget)),
            stop: session.map(|session| stop_policy(&session.stop)),
        }
    }
}

/// The spec's run budget as Commission's, field for field.
fn run_budget(budget: &agent::RunBudget) -> RunBudget {
    RunBudget {
        max_steps: budget.max_steps,
        max_tokens_in: budget.max_tokens_in,
        max_tokens_out: budget.max_tokens_out,
        max_duration_ms: budget.max_duration_ms,
        max_cost_micro_usd: budget.max_cost_micro_usd,
        max_tokens: budget.max_tokens,
        deadline: budget
            .deadline
            .as_ref()
            .map(|deadline| Timestamp(deadline.0.clone())),
        per_action: budget
            .per_action
            .iter()
            .map(|action| ActionBudget {
                action: action.action.clone(),
                max_trials: action.max_trials,
                max_tokens_in: action.max_tokens_in,
                max_tokens_out: action.max_tokens_out,
                max_duration_ms: action.max_duration_ms,
                max_cost_micro_usd: action.max_cost_micro_usd,
            })
            .collect(),
    }
}

/// The spec's stop policy as Commission's, field for field.
fn stop_policy(stop: &agent::StopPolicy) -> StopPolicy {
    StopPolicy {
        approval_gate: stop.approval_gate,
        idle_bound: stop.idle_bound,
        on_breach: breach_step(stop.on_breach),
        breach_by_limit: stop
            .breach_by_limit
            .iter()
            .map(|entry| LimitBreachStep {
                limit: budget_limit(entry.limit),
                step: breach_step(entry.step),
            })
            .collect(),
    }
}

fn breach_step(step: agent::BreachStep) -> BreachStep {
    match step {
        agent::BreachStep::DiscardStep => BreachStep::DiscardStep,
        agent::BreachStep::KeepStep => BreachStep::KeepStep,
    }
}

fn budget_limit(limit: agent::BudgetLimit) -> BudgetLimit {
    match limit {
        agent::BudgetLimit::MaxSteps => BudgetLimit::MaxSteps,
        agent::BudgetLimit::MaxTrials => BudgetLimit::MaxTrials,
        agent::BudgetLimit::MaxTokensIn => BudgetLimit::MaxTokensIn,
        agent::BudgetLimit::MaxTokensOut => BudgetLimit::MaxTokensOut,
        agent::BudgetLimit::MaxDurationMs => BudgetLimit::MaxDurationMs,
        agent::BudgetLimit::MaxCostMicroUsd => BudgetLimit::MaxCostMicroUsd,
        agent::BudgetLimit::MaxTokens => BudgetLimit::MaxTokens,
        agent::BudgetLimit::Deadline => BudgetLimit::Deadline,
    }
}

impl Limits for AgentLimits {
    /// None of its own: the spec's `max_steps` is the run budget's, which bounds the loop as a
    /// step budget does.
    fn step_budget(&self) -> Option<usize> {
        None
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.budget.clone()
    }

    fn stop_policy(&self) -> StopPolicy {
        match &self.stop {
            Some(stop) => stop.clone(),
            None => Limits::stop_policy(&Defaults),
        }
    }
}

/// Commission's own limits, for an agent whose spec declares no session.
struct Defaults;

impl Limits for Defaults {
    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The loop context: the host's ids, clock and control, and the agent's limits.
struct Host {
    limits: AgentLimits,
    events: NoEvents,
}

impl Ids for Host {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(fresh(Kind::ActionRequest))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(fresh(Kind::Observation))
    }
}

impl Clock for Host {
    fn now(&mut self) -> Timestamp {
        Timestamp("2026-01-01T00:00:00Z".to_owned())
    }
}

impl Limits for Host {
    fn step_budget(&self) -> Option<usize> {
        self.limits.step_budget()
    }

    fn run_budget(&self) -> Option<RunBudget> {
        self.limits.run_budget()
    }

    fn stop_policy(&self) -> StopPolicy {
        Limits::stop_policy(&self.limits)
    }
}

impl HostControl for Host {
    fn events(&mut self) -> &mut dyn EventSink {
        &mut self.events
    }
}

/// The recorded agent: on every frontier it proposes to read the clock.
struct RecordedAgent;

impl AgentExecutor for RecordedAgent {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: ACTION.to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
        })
    }
}

/// The effect port: performs the clock read and reports the recorded reading. It submits no
/// evidence.
struct RecordedClockRead;

impl EffectPort for RecordedClockRead {
    fn performs(&self, action: &str) -> bool {
        action == ACTION
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if request.data().action != ACTION {
            return Err(EffectError::new(format!(
                "`{}` is not performed here",
                request.data().action
            )));
        }
        let report =
            json::parse(READING).map_err(|error| EffectError::new(format!("{error:?}")))?;
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// The authority provider: no capability is delegated. The clock read needs none.
struct NoGrant;

impl AuthorityProvider for NoGrant {
    fn decide(
        &self,
        _commission: &CommissionData,
        _request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: format!("approval for `{capability}`"),
            },
        ))
    }
}

/// The commission the agent works under: for an operator, with no authority context.
fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(fresh(Kind::Commission)),
        agent_revision_id: AgentRevisionId(fresh(Kind::AgentRevision)),
        case_id: case.clone(),
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(json::Value::Null),
    })
}

/// What a new id is for: its first group, so ids of two kinds never collide.
#[derive(Clone, Copy)]
enum Kind {
    Run = 1,
    ActionRequest = 2,
    Observation = 3,
    Commission = 4,
    AgentRevision = 5,
}

/// A new version-8 UUID: the kind and a process-wide counter.
fn fresh(kind: Kind) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(1);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    Uuid(format!("{:08x}-0000-8000-8000-{issued:012x}", kind as u8))
}
