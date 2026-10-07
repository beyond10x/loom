//! The slice's run: from an intent to a stop reason (story `slice-loop-cli`), on Commission's
//! runtime (`story:runtime-merge`, Atlas ADR 0082).
//!
//! [`run`], in this order:
//!
//! 1. extracts the intent's references ([`b10x_loom_intake_references::references`]);
//! 2. classifies the intent ([`b10x_loom_intake_router::classify`]) on a current-thread Tokio runtime made
//!    for that call and dropped before the loop starts, because the selector and the argument
//!    generator make their own runtime and must not run inside one;
//! 3. opens the governed case ([`crate::case::open`]);
//! 4. runs Commission's loop, [`run_until_blocked`], over the case: Loom proposes on each frontier,
//!    the runtime revalidates each proposal and hands what it admits to the local effect adapter
//!    ([`crate::effect::LocalEffects`]), which performs it and lets the verifier submit evidence.
//!    The slice has no loop of its own.
//!
//! The runtime runs under a commission for the operator with no authority, and an authority
//! provider that answers every capability with approval required: the slice never supplies
//! authority on the operator's behalf. Its step budget is the request's `max_steps`.
//!
//! It ends with a [`SliceRun`] and its [`StopReason`] (`intake.routing.SliceRun` and
//! `intake.routing.StopReason` in `ess/intake/domains/routing.yaml`), read from the runtime's
//! outcome:
//!
//! | Stop reason | Runtime outcome | When |
//! | --- | --- | --- |
//! | [`StopReason::ApprovalRequired`] | `NeedsAuthority`, `AwaitingApproval` | Loom proposes an action the frontier lists as needing approval, or a step leaves a frontier that lists one unchanged (every action's status and reasons); the detail names the proposed action, or the awaited actions in frontier order |
//! | [`StopReason::NothingAdmissible`] | `NoAdmissibleAction`, `NeedsExternalEvidence` | Loom proposes nothing and the frontier lists no admissible action |
//! | [`StopReason::StepBudget`] | `Suspended` for `Budget` | the steps taken reach the request's `max_steps` |
//! | [`StopReason::NoLocalExecutor`] | `NoPerformableAction` | the pick is not `software-change@1`; the case opens and one frontier is read |
//! | [`StopReason::Refused`] | none: no case is opened | the router refuses the pick: outside the registry, or unsure |
//!
//! Any other end is a [`SliceError`]: a model that gives no usable pick, a workspace the case
//! cannot open on, a governor that refuses, an executor or verifier failing on the workspace, Loom
//! suspended because the agent model could not be reached, or the governor holding the case
//! complete.
//!
//! # Refused steps
//!
//! A step that is not performed still counts against the step budget, is printed as refused, and
//! is recorded in the briefing with [`Briefing::record_refusal`], so the next selection is told
//! `refused: <reason>`. Four kinds:
//!
//! - a selection Loom does not turn into a proposal while the frontier lists an admissible action:
//!   an action the frontier lists as blocked, or one it does not list;
//! - a selection answer that names no action ([`crate::selector::NO_ACTION`]);
//! - arguments the model wrote that cannot become the action's arguments: edit contents that do
//!   not resolve, or a stored-result lookup after the last one allowed
//!   ([`ArgumentsError::Refused`]). Nothing is proposed, so Commission admits nothing;
//! - an action the executor refuses (a path outside the workspace, an ignored path, arguments the
//!   action does not take), and a `repository.inspect` that cannot read a path it names.
//!
//! # Terminal safety
//!
//! Every text that comes from a model, a file, a command or the governor is printed through
//! [`printable`], so no control character but the tab reaches the terminal raw.
//!
//! # Output
//!
//! One line per reference (`reference: <kind> <value>`), the pick (`picked <protocol> (confidence
//! <c>)`, then one indented `reason:` line each) or the refusal (`refused: <why>`), the frontier
//! before each Loom run and the one the run stopped on at an approval gate or for want of a local
//! executor (`frontier: <action> (<status>), ...`), each step (`step <n>: <action> <arguments>`,
//! then indented `effect:` and `evidence:` lines; further lines of an effect are indented and
//! start with `|`), and last `stopped: <reason>`, with its detail in parentheses where it has one.
//!
//! # Identity
//!
//! A [`SliceRun`] carries the specification's `protocol`, `steps` and `stop_reason`. Its `run_id`
//! and `intent_id` are not assigned here: the run is not recorded anywhere yet.

use std::fmt::{self, Write as _};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::Timestamp;
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, ExecutorOutcome, Frontier, FrontierData, GovernorError, Observation,
    ObservationId, PrincipalId, RevalidateActionRequestOutcome, RunId, RunOutcome,
    SuspensionReason, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, run_until_blocked,
};
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext, SelectorError};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom};
use b10x_loom_intake_references::references;
use b10x_loom_intake_router::{ProtocolPick, RouterError, classify};
use llm_core::Model;
use loom_governor::{CanonGovernor, CaseStore};

use crate::case::{self, CaseError};
use crate::confinement::TestRunner;
use crate::context_metrics::{ContextMetrics, ContextPolicy, MeasuredModel};
use crate::effect::{Console, LocalEffects, refuse};
use crate::executor::backend_name;
use crate::executor::{ExecuteError, LocalExecutor, TestCommand, fresh_uuid, now};
use crate::selector::{ArgumentsError, Briefing, ModelArguments, ModelSelector, NO_ACTION};
use crate::verifier::{TestResultVerifier, VerifyError};

/// The one protocol the slice executes locally.
pub const LOCAL_PROTOCOL: &str = "software-change@1";

/// The producer the slice's verifier attributes its evidence to.
pub const PRODUCER: &str = "intake-slice/verifier";

/// What one run is asked to do.
#[derive(Clone)]
pub struct SliceRequest {
    /// The explicit execution policy. Use Substrate unless the operator opts out.
    pub runner: Arc<dyn TestRunner>,
    /// The intent, as given.
    pub intent: String,
    /// The root of the git work tree the case is about.
    pub workspace: PathBuf,
    /// The command `tests.run` runs in the workspace.
    pub test: TestCommand,
    /// The most actions performed before the run stops with [`StopReason::StepBudget`].
    pub max_steps: usize,
    /// The confidence, from 0 to 1, below which the router refuses its pick.
    pub threshold: f64,
}

/// Optional context policy and measurement destination for one run. Existing callers stay legacy.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub context_policy: ContextPolicy,
    pub context_report: Option<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            context_policy: ContextPolicy::Legacy,
            context_report: None,
        }
    }
}

/// Why a slice run ended (`intake.routing.StopReason`).
pub use intake_model::routing::StopReason;

/// One run of the slice, ended for a stated reason (`intake.routing.SliceRun` without its
/// identities; see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRun {
    /// The protocol the router proposed, as `name@major`; for a refused pick, the one it refused.
    pub protocol: String,
    /// The actions performed, refused ones included.
    pub steps: usize,
    /// Why the run ended.
    pub stop_reason: StopReason,
}

/// Why a run ended without a stop reason.
#[derive(Debug)]
pub enum SliceError {
    /// Context capacity was exhausted after preserving the effects already completed.
    Context(String),
    /// A trusted clock operation failed.
    Clock(String),
    /// Measurement output failed; a simultaneous run failure is retained too.
    ContextReport {
        error: io::Error,
        run_error: Option<Box<SliceError>>,
    },
    /// The output could not be written.
    Output(io::Error),
    /// No runtime could be made for the classification.
    Runtime(io::Error),
    /// The router gave no pick, for a reason other than a refusal.
    Router(RouterError),
    /// The case could not be opened.
    Case(CaseError),
    /// The governor refused a frontier, a completion or the evidence it holds.
    Governor(GovernorError),
    /// The executor failed on the workspace, git, the test command or the governor.
    Execute(ExecuteError),
    /// The verifier could not submit a test result.
    Verify(VerifyError),
    /// Loom suspended: the agent model gave no usable answer.
    Suspended(String),
    /// Loom answered with an outcome the slice does not handle.
    Unexpected(String),
    /// The governor holds the case complete. `software-change@1` completes only with evidence the
    /// slice never produces, so this is not a stop reason.
    Complete(String),
    /// Commission's runtime stopped for a failure of its own: a run command that refused.
    Loop(LoopError),
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => write!(f, "clock unavailable: {error}"),
            Self::Context(why) => write!(f, "working context refused: {why}"),
            Self::ContextReport { error, run_error } => {
                if let Some(run_error) = run_error {
                    write!(f, "{run_error}; ")?;
                }
                write!(f, "context report cannot be written: {error}")
            }
            Self::Output(error) => write!(f, "the output cannot be written: {error}"),
            Self::Runtime(error) => write!(f, "no runtime for the classification: {error}"),
            Self::Router(error) => error.fmt(f),
            Self::Case(error) => error.fmt(f),
            Self::Governor(error) => write!(f, "the governor refused: {error:?}"),
            Self::Execute(error) => error.fmt(f),
            Self::Verify(error) => error.fmt(f),
            Self::Suspended(why) => write!(f, "the agent was suspended: {why}"),
            Self::Unexpected(outcome) => write!(f, "Loom answered {outcome}"),
            Self::Complete(outcome) => write!(f, "the case completed as `{outcome}`"),
            Self::Loop(error) => write!(f, "the runtime stopped: {error}"),
        }
    }
}

impl std::error::Error for SliceError {}

impl From<io::Error> for SliceError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}

impl From<GovernorError> for SliceError {
    fn from(error: GovernorError) -> Self {
        Self::Governor(error)
    }
}

/// Runs the slice for `request` until it stops, printing what it does to `out`.
///
/// `governor` holds the case: the case is opened on it, and the executor, the verifier and the
/// runtime report to it. The runtime reads every revision, frontier and completion from
/// `frontiers`; in use that is `governor` itself. `classifier` picks the protocol, `agent` selects
/// actions and writes their arguments. Must not be called inside a Tokio runtime.
///
/// # Errors
/// A [`SliceError`] for every failure that is not a [`StopReason`] (see the module
/// documentation). What was printed before the failure stays printed.
pub fn run<S: CaseStore>(
    request: &SliceRequest,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    out: &mut dyn Write,
) -> Result<SliceRun, SliceError> {
    run_with_options(
        request,
        governor,
        frontiers,
        classifier,
        agent,
        out,
        &RunOptions::default(),
    )
}

/// Runs with an opt-in context policy and writes payload-free measurements even on run failure.
///
/// # Errors
/// As [`run`], plus explicit context capacity and report output failures. A report output failure
/// retains any original run failure; already printed output and completed effects remain visible.
pub fn run_with_options<S: CaseStore>(
    request: &SliceRequest,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    out: &mut dyn Write,
    options: &RunOptions,
) -> Result<SliceRun, SliceError> {
    let metrics = ContextMetrics::new(options.context_policy);
    let classifier = MeasuredModel::new(classifier, metrics.clone());
    let agent = MeasuredModel::new(agent, metrics.clone());
    let result = run_measured(
        request,
        governor,
        frontiers,
        &classifier,
        &agent,
        out,
        options,
        &metrics,
    );
    if let Some(path) = &options.context_report
        && let Err(error) = metrics.write(path)
    {
        return Err(SliceError::ContextReport {
            error,
            run_error: result.err().map(Box::new),
        });
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_measured<S: CaseStore>(
    request: &SliceRequest,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    out: &mut dyn Write,
    options: &RunOptions,
    metrics: &ContextMetrics,
) -> Result<SliceRun, SliceError> {
    let found = references(&request.intent);
    for reference in &found {
        writeln!(
            out,
            "reference: {:?} {}",
            reference.kind,
            printable(&reference.value)
        )?;
    }

    writeln!(
        out,
        "confinement: {}",
        backend_name(request.runner.backend())
    )?;
    let pick = match pick(request, classifier)? {
        Ok(pick) => pick,
        Err((protocol, detail, why)) => {
            writeln!(out, "refused: {}", printable(&why))?;
            return stop(out, protocol, 0, StopReason::Refused, Some(detail));
        }
    };
    writeln!(
        out,
        "picked {} (confidence {})",
        printable(&pick.protocol),
        pick.confidence
    )?;
    for reason in &pick.reasons {
        writeln!(out, "  reason: {}", printable(reason))?;
    }

    let case = case::open(
        governor,
        &pick.protocol,
        &request.intent,
        &request.workspace,
    )
    .map_err(SliceError::Case)?;

    let briefing = Briefing::with_options(
        request.intent.clone(),
        found,
        options.context_policy,
        metrics.clone(),
    );
    drive(
        &request.intent,
        &pick.protocol,
        case,
        request.max_steps,
        governor,
        frontiers,
        agent,
        out,
        briefing,
        Execution::Software {
            workspace: &request.workspace,
            test: &request.test,
            runner: Arc::clone(&request.runner),
        },
    )
}

pub(crate) enum Execution<'a> {
    Software {
        workspace: &'a std::path::Path,
        test: &'a TestCommand,
        runner: Arc<dyn TestRunner>,
    },
    Clock {
        clock: &'a dyn crate::clock::Clock,
        intent_revision: &'a str,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn drive<S: CaseStore>(
    intent: &str,
    protocol: &str,
    case: CaseId,
    budget: usize,
    governor: &CanonGovernor<S>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    out: &mut dyn Write,
    briefing: Briefing,
    execution: Execution<'_>,
) -> Result<SliceRun, SliceError> {
    let chosen = Arc::new(Mutex::new(None));
    let refused = Arc::new(Mutex::new(None));
    let console = Console::new(out);
    let step = LoomStep {
        loom: Loom::new(
            Recording {
                inner: ModelSelector::new(agent, briefing.clone()),
                chosen: Arc::clone(&chosen),
            },
            Arguments {
                inner: ModelArguments::new(agent, briefing.clone()),
                refused: Arc::clone(&refused),
            },
            intent.to_owned(),
        ),
        chosen,
        refused,
        briefing: briefing.clone(),
        console: &console,
    };
    let query = matches!(&execution, Execution::Clock { .. });
    let effects: Box<dyn b10x_loom_commission::ports::effect::EffectPort + '_> = match execution {
        Execution::Software {
            workspace,
            test,
            runner,
        } => Box::new(LocalEffects::new(
            LocalExecutor::new(
                governor,
                case.clone(),
                workspace.to_path_buf(),
                test.clone(),
            )
            .with_runner(runner),
            TestResultVerifier::new(governor, case.clone(), PRODUCER),
            governor,
            case.clone(),
            protocol,
            briefing.clone(),
            &console,
        )),
        Execution::Clock {
            clock,
            intent_revision,
        } => Box::new(crate::clock::ClockEffects::new(
            governor,
            case.clone(),
            intent_revision,
            clock,
            briefing.clone(),
            &console,
        )),
    };
    let reads = Reads {
        frontiers,
        governor,
    };
    let seed = case.0.clone();
    let mut runs = Generated::new(RunStore::new(move || RunId(fresh_uuid("run", &seed))));
    let mut context = SliceContext {
        case: case.0.clone(),
        budget,
    };
    let result = run_until_blocked(
        &reads,
        &step,
        &NoDelegatedAuthority,
        &*effects,
        &commission(&case),
        &mut runs,
        &mut context,
    );
    drop(effects);
    drop(step);
    let failure = console.take_failure();
    let context_failure = briefing.failure();
    let steps = console.steps();
    drop(console);
    if let Some(failure) = failure {
        if let SliceError::Execute(ExecuteError::Confinement(refusal)) = failure {
            return stop(
                out,
                protocol.to_owned(),
                steps,
                StopReason::ConfinementUnavailable,
                Some(refusal.to_string()),
            );
        }
        return Err(failure);
    }
    if let Some(why) = context_failure {
        return Err(SliceError::Context(why));
    }

    let end = match result {
        Ok(end) => end,
        Err(LoopError {
            failure: LoopFailure::Governor(error),
            ..
        }) => return Err(SliceError::Governor(error)),
        Err(error) => return Err(SliceError::Loop(error)),
    };
    if query && let RunOutcome::Completed(ref complete) = end.outcome {
        return stop(
            out,
            protocol.to_owned(),
            end.steps,
            StopReason::Completed,
            Some(complete.outcome.clone()),
        );
    }
    finish(out, protocol.to_owned(), end)
}

/// The stop reason the runtime's `end` stands for, as the module documents, printed.
fn finish(out: &mut dyn Write, protocol: String, end: LoopEnd) -> Result<SliceRun, SliceError> {
    let steps = end.steps;
    match end.outcome {
        RunOutcome::Completed(complete) => Err(SliceError::Complete(complete.outcome)),
        RunOutcome::Suspended(suspended) => match suspended.reason {
            SuspensionReason::Budget(_) => stop(out, protocol, steps, StopReason::StepBudget, None),
            reason => Err(SliceError::Suspended(format!("{reason:?}"))),
        },
        RunOutcome::NeedsAuthority(_) => {
            let action = end
                .requests
                .iter()
                .rev()
                .find_map(|made| match &made.outcome {
                    RevalidateActionRequestOutcome::NeedsAuthority { error } => {
                        Some(error.action.clone())
                    }
                    _ => None,
                });
            stop(out, protocol, steps, StopReason::ApprovalRequired, action)
        }
        RunOutcome::AwaitingApproval(awaiting) => {
            if let Some(frontier) = &end.last_frontier {
                print_frontier(out, frontier)?;
            }
            let detail = Some(awaiting.actions.join(", "));
            stop(out, protocol, steps, StopReason::ApprovalRequired, detail)
        }
        RunOutcome::NoAdmissibleAction(_) | RunOutcome::NeedsExternalEvidence(_) => {
            stop(out, protocol, steps, StopReason::NothingAdmissible, None)
        }
        RunOutcome::NoPerformableAction(_) => {
            if let Some(frontier) = &end.last_frontier {
                print_frontier(out, frontier)?;
            }
            stop(out, protocol, steps, StopReason::NoLocalExecutor, None)
        }
        RunOutcome::NeedsHumanJudgment(needs) => Err(SliceError::Unexpected(format!(
            "NeedsHumanJudgment {needs:?}"
        ))),
    }
}

/// The governor as the runtime reads it: revisions, frontiers and completions from `frontiers`,
/// observations to the governor that holds the case.
struct Reads<'a, S> {
    frontiers: &'a dyn Governor,
    governor: &'a CanonGovernor<S>,
}

impl<S> Governor for Reads<'_, S> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.frontiers.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.frontiers.frontier(case)
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.frontiers.completion(case)
    }
}

impl<S: CaseStore> ObservationPort for Reads<'_, S> {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.governor.observe(observation)
    }
}

/// The slice's authority provider: the operator delegated no authority to the slice, so every
/// capability needs approval.
struct NoDelegatedAuthority;

impl AuthorityProvider for NoDelegatedAuthority {
    fn decide(
        &self,
        _commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: format!("approval for `{capability}`"),
            },
        ))
    }
}

/// New ids, the machine's clock and the request's step budget.
struct SliceContext {
    case: String,
    budget: usize,
}

impl LoopContext for SliceContext {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(fresh_uuid("action-request", &self.case))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(fresh_uuid("observation", &self.case))
    }

    fn now(&mut self) -> Timestamp {
        now()
    }

    fn step_budget(&self) -> Option<usize> {
        Some(self.budget)
    }
}

/// One Loom run on a frontier, as the runtime's executor: the frontier printed before it, and a
/// selection Loom does not turn into a proposal, or arguments refused before one, printed and
/// recorded as a refused step.
struct LoomStep<'a, 'o, 'm> {
    loom: Loom<Recording<'m>, Arguments<'m>>,
    chosen: Arc<Mutex<Option<Chosen>>>,
    refused: Arc<Mutex<Option<String>>>,
    briefing: Briefing,
    console: &'a Console<'o>,
}

impl AgentExecutor for LoomStep<'_, '_, '_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        match self.step(commission, frontier) {
            Ok(outcome) => outcome,
            Err(failure) => {
                let why = failure.to_string();
                self.console.fail(failure);
                ExecutorOutcome::Suspended(
                    b10x_loom_commission::model::responsibility::ExecutorOutcomeSuspended {
                        reason: SuspensionReason::ExternalAvailability(json::Value::Text(why)),
                    },
                )
            }
        }
    }
}

impl LoomStep<'_, '_, '_> {
    fn step(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> Result<ExecutorOutcome, SliceError> {
        self.console
            .write(|out| print_frontier(out, frontier.data()))?;
        *lock(&self.chosen) = None;
        *lock(&self.refused) = None;
        let outcome = self.loom.run(commission, frontier);
        let chose = lock(&self.chosen).take();
        let refused = lock(&self.refused).take();
        if let (ExecutorOutcome::Suspended(_), Some(reason), Some(Chosen::Action(action))) =
            (&outcome, refused, &chose)
        {
            self.refuse_unproposed(action, &reason)?;
            return Ok(ExecutorOutcome::NoUsefulAction(Unit(true)));
        }
        match outcome {
            ExecutorOutcome::NoUsefulAction(_) => {
                let admissible = frontier
                    .data()
                    .actions
                    .iter()
                    .any(|listed| listed.status == ActionStatus::Admissible);
                if admissible {
                    self.refuse_selection(frontier, chose)?;
                }
                Ok(outcome)
            }
            ExecutorOutcome::Suspended(_) if matches!(chose, Some(Chosen::NoAction)) => {
                self.refuse_selection(frontier, chose)?;
                Ok(ExecutorOutcome::NoUsefulAction(Unit(true)))
            }
            other => Ok(other),
        }
    }

    /// Prints the next step, a selection Loom did not turn into a proposal, as refused, and tells
    /// the model why.
    fn refuse_selection(
        &self,
        frontier: &Frontier<frontier_state::Issued>,
        chose: Option<Chosen>,
    ) -> Result<(), SliceError> {
        let (action, reason) = match chose {
            Some(Chosen::Action(action)) => {
                let reason = selection_refusal(frontier, &action);
                (action, reason)
            }
            Some(Chosen::NoAction) => (NO_SELECTION.to_owned(), NO_ACTION.to_owned()),
            None => (
                NO_SELECTION.to_owned(),
                "Loom proposed no action".to_owned(),
            ),
        };
        self.refuse_unproposed(&action, &reason)
    }

    /// Prints the next step, an `action` that was not proposed, as refused for `reason`, and tells
    /// the model why: a selection Loom refused, or arguments it could not use.
    fn refuse_unproposed(&self, action: &str, reason: &str) -> Result<(), SliceError> {
        let step = self.console.next_step();
        self.console.write(|out| {
            writeln!(out, "step {step}: {} (not proposed)", printable(action))?;
            refuse(out, &self.briefing, action, reason)
        })
    }
}

/// The agent's argument generator, keeping a refused answer ([`ArgumentsError::Refused`]) so the
/// step is refused and the model told, rather than the run suspended as if the model were out of
/// reach. Loom still sees an `Err` either way.
struct Arguments<'m> {
    inner: ModelArguments<'m>,
    refused: Arc<Mutex<Option<String>>>,
}

impl ArgumentGenerator for Arguments<'_> {
    fn generate(
        &self,
        context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<json::Value, String> {
        let answer = self.inner.arguments(context, entry);
        if let Err(ArgumentsError::Refused(reason)) = &answer {
            *lock(&self.refused) = Some(reason.clone());
        }
        answer.map_err(|error| error.to_string())
    }
}

/// The router's pick, or a refusal as the protocol it refused, the stop detail and the reason.
type Pick = Result<ProtocolPick, (String, String, String)>;

/// Classifies the intent on a runtime made for the call and dropped with it.
fn pick(request: &SliceRequest, classifier: &dyn Model) -> Result<Pick, SliceError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(SliceError::Runtime)?;
    let classified = runtime.block_on(classify(&request.intent, classifier, request.threshold));
    drop(runtime);
    let error = match classified {
        Ok(pick) => return Ok(Ok(pick)),
        Err(error) => error,
    };
    let (protocol, detail) = match &error {
        RouterError::Unsure { protocol, .. } => (protocol.clone(), "unsure"),
        RouterError::OutsideRegistry { protocol } => (protocol.clone(), "outside the registry"),
        _ => return Err(SliceError::Router(error)),
    };
    Ok(Err((protocol, detail.to_owned(), error.to_string())))
}

/// Prints the stop line and returns the run.
fn stop(
    out: &mut dyn Write,
    protocol: String,
    steps: usize,
    stop_reason: StopReason,
    detail: Option<String>,
) -> Result<SliceRun, SliceError> {
    match detail {
        Some(detail) => writeln!(out, "stopped: {stop_reason:?} ({})", printable(&detail))?,
        None => writeln!(out, "stopped: {stop_reason:?}")?,
    }
    Ok(SliceRun {
        protocol,
        steps,
        stop_reason,
    })
}

fn print_frontier(out: &mut dyn Write, frontier: &FrontierData) -> Result<(), SliceError> {
    let actions: Vec<String> = frontier
        .actions
        .iter()
        .map(|listed| {
            let status = match listed.status {
                ActionStatus::Admissible => "admissible",
                ActionStatus::ApprovalRequired => "approval required",
                ActionStatus::Blocked => "blocked",
            };
            format!("{} ({status})", printable(&listed.action))
        })
        .collect();
    writeln!(out, "frontier: {}", actions.join(", "))?;
    Ok(())
}

/// What a selection that names no action is shown as.
const NO_SELECTION: &str = "(no action)";

/// Why Loom did not propose `action`: what the frontier says of it.
fn selection_refusal(frontier: &Frontier<frontier_state::Issued>, action: &str) -> String {
    let listed = frontier
        .data()
        .actions
        .iter()
        .find(|listed| listed.action == action);
    match listed {
        None => format!("`{action}` is not one of the candidate actions"),
        Some(listed) if listed.status == ActionStatus::Blocked => {
            if listed.reasons.is_empty() {
                format!("`{action}` is blocked")
            } else {
                format!("`{action}` is blocked: {}", listed.reasons.join("; "))
            }
        }
        Some(_) => format!("`{action}` was not admitted"),
    }
}

/// What the agent's selector answered on one Loom run.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Chosen {
    /// It named this action.
    Action(String),
    /// It answered without naming an action ([`NO_ACTION`]).
    NoAction,
}

/// The agent's selector, keeping what it answered so a selection Loom refuses can be told apart
/// from a frontier with nothing to propose.
struct Recording<'m> {
    inner: ModelSelector<'m>,
    chosen: Arc<Mutex<Option<Chosen>>>,
}

impl ActionSelector for Recording<'_> {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        let answer = self.inner.select(context, candidates);
        *lock(&self.chosen) = match &answer {
            Ok(choice) => Some(Chosen::Action(choice.action.clone())),
            Err(SelectorError::Unavailable(why)) if why == NO_ACTION => Some(Chosen::NoAction),
            Err(_) => None,
        };
        answer
    }

    fn strategy(&self) -> SelectionStrategy {
        self.inner.strategy()
    }
}

fn lock<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `text` safe to print on a terminal: every control character (C0, DEL and C1) but the tab is
/// shown as a visible escape, `\u{1b}` for ESC, so text from a model, a file or a command can
/// neither move the cursor, redraw a line nor start a new one. Everything else is unchanged.
pub fn printable(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() && character != '\t' {
            let _ = write!(shown, "\\u{{{:x}}}", u32::from(character));
        } else {
            shown.push(character);
        }
    }
    shown
}

/// The commission the slice's Loom runs under: one per case, for the operator, with no authority.
fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(fresh_uuid("commission", &case.0)),
        agent_revision_id: AgentRevisionId(fresh_uuid("agent-revision", &case.0)),
        case_id: case.clone(),
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(json::Value::Null),
    })
}

/// `value` as compact JSON text.
pub(crate) fn json_text(value: &json::Value) -> String {
    let mut text = String::new();
    push_json(&mut text, value);
    text
}

fn push_json(out: &mut String, value: &json::Value) {
    match value {
        json::Value::Null => out.push_str("null"),
        json::Value::Bool(value) => {
            let _ = write!(out, "{value}");
        }
        json::Value::Number(number) => out.push_str(number),
        json::Value::Text(text) => json::push_text(out, text),
        json::Value::Array(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                push_json(out, item);
            }
            out.push(']');
        }
        json::Value::Object(members) => {
            out.push('{');
            for (n, (name, item)) in members.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                json::push_text(out, name);
                out.push(':');
                push_json(out, item);
            }
            out.push('}');
        }
    }
}
