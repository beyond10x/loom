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
//! Every observation's instant is read from the host clock: [`SystemClock`] for [`run`] and
//! [`run_with_options`], the caller's for [`run_with_clock`]. The governor is generic over its
//! evaluation time; the one a host builds for the slice ([`slice_governor`]) evaluates at the
//! host instant, so an evidence kind's `max_age` expires an old observation.
//!
//! The runtime runs under a commission for the operator with no authority, and an authority
//! provider that answers every capability with approval required: the slice never supplies
//! authority on the operator's behalf. Its step budget is the request's `max_steps`.
//!
//! A run given a state directory ([`crate::intent::run_prepared_intent_with_state`], `run --state`)
//! keeps its commission's resume context there before the commission runs, then runs it through
//! Commission's `run_with_suspension` over the directory's suspension store instead, so the
//! Suspension a withheld approval leaves outlives the process. Its output and stop reasons are the
//! same. A resume ([`crate::intent::resume_with_state`], `resume`) continues that commission
//! through Commission's `resume_after_grant`, under the authority provider over its grant
//! ([`crate::grant::GrantAuthority`]), with `repository.merge` handed off
//! ([`crate::handoff::HandoffEffects`]), and a merge proposed again at a `HEAD` the commission
//! already handed off held as no Suspension ([`crate::handoff::SettledHandoffs`]); its output and
//! stop reasons are a run's too.
//!
//! It ends with a [`SliceRun`] and its [`StopReason`] (`intake.routing.SliceRun` and
//! `intake.routing.StopReason` in `ess/intake/domains/routing.yaml`), read from the runtime's
//! outcome:
//!
//! | Stop reason | Runtime outcome | When |
//! | --- | --- | --- |
//! | [`StopReason::ApprovalRequired`] | `NeedsAuthority`, `AwaitingApproval` | Loom proposes an action the frontier lists as needing approval, or a step leaves a frontier that lists one unchanged (every action's status and reasons); the detail names the proposed action, or the awaited actions in frontier order |
//! | [`StopReason::NothingAdmissible`] | `NoAdmissibleAction`, `NeedsExternalEvidence` | Loom proposes nothing and the frontier lists no admissible action |
//! | [`StopReason::NothingAdmissible`] | `IdleBound` | Commission's idle bound ended the run: with no step budget, two iterations in a row made no progress. The slice's runs always take `max_steps` as their step budget, so they never end so; the stop is the one above, with no detail |
//! | [`StopReason::NothingAdmissible`] | `CaseMovedOn` | the case moved to another revision while the run worked, and its frontier there still admits an action; the run, bound to the revision it left, stops, and the detail names both revisions (`case moved on from revision <bound> to <current>`) |
//! | [`StopReason::StepBudget`] | `Suspended` for `Budget` | the steps taken reach the request's `max_steps` |
//! | [`StopReason::NoLocalExecutor`] | `NoPerformableAction` | the pick is not `software-change@1`; the case opens and one frontier is read. [`SliceRun::unperformed`] carries the actions the outcome names, each once, in frontier order |
//! | [`StopReason::NoLocalExecutor`] | none: no case is opened | the pick is a protocol the request's action bindings bind ([`crate::bindings`]) and one of its actions has no command; the detail names it, and [`SliceRun::unperformed`] carries every action with no command, in the protocol's order |
//! | [`StopReason::Refused`] | none: no case is opened | the router refuses the pick: outside the registry, or unsure |
//! | [`StopReason::Refused`] | `EffectRefused` | an effect port refused an admitted request as final; the detail is its reason. The slice's own effect ports never mark a refusal final |
//!
//! Any other end is a [`SliceError`]: a model that gives no usable pick, a workspace the case
//! cannot open on, a governor that refuses, an executor or verifier failing on the workspace, Loom
//! suspended because the agent model could not be reached, or the governor holding the case
//! complete.
//!
//! # Actions the slice cannot perform
//!
//! On a `software-change@1` case the runtime reads every frontier without the actions the slice
//! can never perform: those that need approval or name a capability and are not among
//! [`PERFORMED`] (`release.rollback`; see [`never_performed`]). Such an action follows the rule an
//! action the effects do not perform and that needs no authority already follows: it is never
//! handed to the agent, so it is never offered, never awaited at an approval gate and never held
//! as a Suspension that no resume could perform. A proposal naming it is refused as not in the
//! frontier. `repository.merge` stays: a resume hands it off.
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
//! - an action the executor refuses (a path outside the workspace, an ignored path, a
//!   `tests.write` path outside the request's [`SliceRequest::test_paths`], arguments the action
//!   does not take), and a `repository.inspect` that cannot read a path it names.
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
//! A run with a state directory or a resume that stops [`StopReason::ApprovalRequired`] prints
//! that line only when the commission holds a Suspension `approve` can approve
//! ([`ApprovalGate`]): it prints `stopped: ApprovalRequired (<action>), nothing suspended` when it
//! holds none, and a resume prints `stopped: MergeHandedOff (<request>, HEAD <head>)` when the
//! case's `HEAD` is one the commission already handed off. A run without a state directory prints
//! as it always did.
//!
//! # Identity
//!
//! A [`SliceRun`] carries the specification's `protocol`, `steps` and `stop_reason`. Its `run_id`
//! and `intent_id` are not assigned here: the run is not recorded anywhere yet. Its `risk`,
//! `change_kind` and `affects_security_boundary` are the request's [`SliceRequest::inputs`] (an
//! [`IntentRequest`](crate::intent::IntentRequest)'s, through `intent`), which the caller holds;
//! for a resume, the held case's, which
//! [`resumed_request`](crate::intent::resumed_request) reads from the governor. Its
//! `test_paths` are the request's [`SliceRequest::test_paths`], and a resume's those its resume
//! context keeps.

use std::fmt::{self, Write as _};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::Timestamp;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, Frontier, FrontierAction, FrontierData,
    GovernorError, Observation, ObservationId, PrincipalId, RevalidateActionRequestOutcome, RunId,
    RunOutcome, SuspensionReason, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopError, LoopFailure, ResumeError, resume_after_grant,
    run_until_blocked, run_with_suspension,
};
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext, SelectorError};
use b10x_loom_executor::{ActionSelector, ArgumentContext, ArgumentGenerator, Loom};
use b10x_loom_intake_references::references;
use b10x_loom_intake_router::{ProtocolPick, RouterError};
use llm_core::Model;
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore, OpenError};
use loom_protocols::ProtocolCatalog;

use crate::case::{self, CaseError};
use crate::clock::{Clock, ClockTime, HostTime, SystemClock};
use crate::confinement::TestRunner;
use crate::context_metrics::{ContextMetrics, ContextPolicy, MeasuredModel};
use crate::effect::{Console, LocalEffects, refuse};
use crate::executor::backend_name;
use crate::executor::{ExecuteError, LocalExecutor, TestCommand, TestPaths, fresh_uuid, now};
use crate::handoff::{HandoffEffects, MergeHandoffStore, SettledHandoffs};
use crate::selector::{ArgumentsError, Briefing, ModelArguments, ModelSelector, NO_ACTION};
use crate::verifier::{TestResultVerifier, VerifyError};

/// The one protocol the slice executes locally.
pub const LOCAL_PROTOCOL: &str = "software-change@1";

/// The producer the slice's verifier attributes its evidence to.
pub const PRODUCER: &str = "intake-slice/verifier";

/// The governor a slice host builds over `store`: the host `catalog` admitted, evaluating at
/// the host instant ([`HostTime`]), so an evidence kind's `max_age` expires an observation older
/// than it. `b10x-loom run` and `resume` build theirs with it.
///
/// # Errors
/// [`CanonGovernor::with_catalog`]'s.
pub fn slice_governor<S: FallibleCaseStore>(
    store: S,
    catalog: &ProtocolCatalog,
) -> Result<CanonGovernor<S, HostTime>, OpenError> {
    Ok(CanonGovernor::new(store)
        .with_catalog(catalog)?
        .with_evaluation_time(ClockTime(SystemClock)))
}

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
    /// The case inputs the host supplies, never a model: the case opens with each one its
    /// protocol declares ([`crate::case`], "Inputs a case starts with").
    pub inputs: case::CaseInputs,
    /// The host's test-path patterns, never a model's: `tests.write` changes only paths they
    /// match ([`crate::test_paths`]); `TestPaths::default()` for the defaults.
    pub test_paths: TestPaths,
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
    /// The actions the run awaits approval for, in frontier order; empty unless it stopped
    /// [`StopReason::ApprovalRequired`].
    pub approvals: Vec<String>,
    /// The actions no local executor performs, each once; empty unless it stopped
    /// [`StopReason::NoLocalExecutor`] (`story:unperformable-names-actions`). After a case opened,
    /// the ones Commission's `NoPerformableAction` names, in frontier order; before, every action
    /// the picked protocol declares that the run cannot perform, in the protocol's order (see
    /// [`crate::intent`]).
    pub unperformed: Vec<String>,
    /// What a run with a state directory that stopped [`StopReason::ApprovalRequired`] left to
    /// approve; [`ApprovalGate::Approvable`] for every other run.
    pub gate: ApprovalGate,
}

/// What a run that stopped [`StopReason::ApprovalRequired`] left for `approve`. A run without a
/// state directory (`run_until_blocked`) holds nothing anyone could approve and is always
/// [`ApprovalGate::Approvable`], printed as it always was.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ApprovalGate {
    /// The stop line reads `stopped: ApprovalRequired (<action>)`: with a state directory, the
    /// commission holds a Suspension `approve` can approve.
    #[default]
    Approvable,
    /// The commission holds no Suspension: `stopped: ApprovalRequired (<action>), nothing
    /// suspended`. Nothing can be approved.
    NothingSuspended,
    /// The case's `HEAD` is one the commission already handed off:
    /// `stopped: MergeHandedOff (<request>, HEAD <head>)`. Nothing is left to approve for it.
    HandedOff {
        /// The request that handed it off.
        action_request_id: String,
        /// The `HEAD` handed off.
        head: String,
    },
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
    /// Commission's runtime stopped for a failure of its own: a run command that refused, or a
    /// suspension store that could not be read or written.
    Loop(LoopError),
    /// The run's state directory could not keep what it holds: the resume context.
    State(String),
    /// A resume continued nothing, or its continued loop stopped without a run outcome: why, as
    /// Commission's `resume_after_grant` answered.
    Resume(ResumeError),
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
            Self::State(why) => write!(f, "the state directory cannot be used: {why}"),
            Self::Resume(error) => write!(f, "the resume stopped: {error}"),
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
pub fn run<S: FallibleCaseStore, T: EvaluationTime>(
    request: &SliceRequest,
    governor: &CanonGovernor<S, T>,
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
pub fn run_with_options<S: FallibleCaseStore, T: EvaluationTime>(
    request: &SliceRequest,
    governor: &CanonGovernor<S, T>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    out: &mut dyn Write,
    options: &RunOptions,
) -> Result<SliceRun, SliceError> {
    run_with_clock(
        request,
        governor,
        frontiers,
        classifier,
        agent,
        &SystemClock,
        out,
        options,
    )
}

/// [`run_with_options`], reading every observation's instant from the host's `clock`, never a
/// model's.
///
/// # Errors
/// As [`run_with_options`].
#[allow(clippy::too_many_arguments)]
pub fn run_with_clock<S: FallibleCaseStore, T: EvaluationTime>(
    request: &SliceRequest,
    governor: &CanonGovernor<S, T>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    clock: &dyn Clock,
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
        clock,
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
fn run_measured<S: FallibleCaseStore, T: EvaluationTime>(
    request: &SliceRequest,
    governor: &CanonGovernor<S, T>,
    frontiers: &dyn Governor,
    classifier: &dyn Model,
    agent: &dyn Model,
    clock: &dyn Clock,
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
        &request.inputs,
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
            test_paths: &request.test_paths,
            clock,
        },
        Mode::Once,
    )
}

/// What a run whose commission outlives its process holds it through (`run --state`): the store
/// the runtime keeps a withheld approval's Suspension in, and what keeps the commission's resume
/// context before the commission runs.
pub(crate) struct Holding<'h> {
    pub(crate) suspensions: &'h dyn SuspensionPort,
    pub(crate) keep: &'h dyn Fn(&CommissionData) -> Result<(), SliceError>,
}

/// What a resume continues a held commission through (`resume`): the commission as its resume
/// context names it, the store its Suspension is held in, the authority provider over its grant,
/// and the store a granted `repository.merge` is handed off into.
pub(crate) struct Resuming<'r> {
    pub(crate) commission: CommissionData,
    pub(crate) suspensions: &'r dyn SuspensionPort,
    pub(crate) authority: &'r dyn AuthorityProvider,
    pub(crate) handoffs: &'r MergeHandoffStore,
    /// The workspace, as the resume context keeps it and the hand-off records it.
    pub(crate) workspace: String,
}

/// How [`drive`] runs the commission.
#[derive(Clone, Copy)]
pub(crate) enum Mode<'m> {
    /// Once, through `run_until_blocked`, holding nothing.
    Once,
    /// Through `run_with_suspension`, holding the commission ([`Holding`]).
    Holding(&'m Holding<'m>),
    /// Through `resume_after_grant`, continuing the commission's suspended run ([`Resuming`]).
    Resuming(&'m Resuming<'m>),
}

pub(crate) enum Execution<'a> {
    Software {
        workspace: &'a std::path::Path,
        test: &'a TestCommand,
        runner: Arc<dyn TestRunner>,
        test_paths: &'a TestPaths,
        /// The host clock every observation's instant is read from.
        clock: &'a dyn Clock,
    },
    Clock {
        clock: &'a dyn crate::clock::Clock,
        intent_revision: &'a str,
    },
    /// An installed protocol's actions, performed through the commands the operator bound to
    /// them ([`crate::bindings`]).
    Commands {
        bindings: &'a crate::bindings::ActionBindings,
        /// The evidence kinds each action declares it may produce.
        productions: std::collections::BTreeMap<String, Vec<String>>,
        intent_revision: &'a str,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn drive<S: FallibleCaseStore, T: EvaluationTime>(
    intent: &str,
    protocol: &str,
    case: CaseId,
    budget: usize,
    governor: &CanonGovernor<S, T>,
    frontiers: &dyn Governor,
    agent: &dyn Model,
    out: &mut dyn Write,
    briefing: Briefing,
    execution: Execution<'_>,
    mode: Mode<'_>,
) -> Result<SliceRun, SliceError> {
    let commission = match &mode {
        Mode::Resuming(resuming) => Commission::new(resuming.commission.clone()),
        _ => commission(&case),
    };
    if let Mode::Holding(holding) = &mode {
        (holding.keep)(commission.data())?;
    }
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
    // A run without a workspace ends `Completed` when the governor holds its case complete.
    let query = !matches!(&execution, Execution::Software { .. });
    let workspace = match &execution {
        Execution::Software { workspace, .. } => Some(workspace.to_path_buf()),
        Execution::Clock { .. } | Execution::Commands { .. } => None,
    };
    let local: Box<dyn EffectPort + '_> = match execution {
        Execution::Software {
            workspace,
            test,
            runner,
            test_paths,
            clock,
        } => Box::new(LocalEffects::new(
            LocalExecutor::new(
                governor,
                case.clone(),
                workspace.to_path_buf(),
                test.clone(),
            )
            .with_runner(runner)
            .with_test_paths(test_paths.clone())
            .with_clock(clock),
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
        Execution::Commands {
            bindings,
            productions,
            intent_revision,
        } => Box::new(crate::bindings::CommandEffects::new(
            governor,
            case.clone(),
            intent_revision,
            bindings,
            productions,
            briefing.clone(),
            &console,
        )),
    };
    // A resume hands a granted `repository.merge` off; nothing else performs one.
    let held_head = || implementation_revision(governor, &case);
    let handoff = match (&mode, workspace) {
        (Mode::Resuming(resuming), Some(workspace)) => Some(HandoffEffects::new(
            &*local,
            resuming.handoffs,
            workspace,
            resuming.workspace.clone(),
            &held_head,
            briefing.clone(),
            &console,
            protocol == LOCAL_PROTOCOL,
        )),
        _ => None,
    };
    let effects: &dyn EffectPort = match &handoff {
        Some(handoff) => handoff,
        None => &*local,
    };
    let reads = Reads {
        frontiers,
        governor,
        local: protocol == LOCAL_PROTOCOL,
    };
    let seed = case.0.clone();
    let runs = move || RunStore::new(move || RunId(fresh_uuid("run", &seed)));
    let mut context = SliceContext {
        case: case.0.clone(),
        budget,
    };
    let result = match &mode {
        Mode::Holding(holding) => run_with_suspension(
            &reads,
            &step,
            &NoDelegatedAuthority,
            effects,
            &commission,
            &mut Generated::new(runs()),
            holding.suspensions,
            &mut context,
        )
        .map_err(loop_failed),
        Mode::Once => run_until_blocked(
            &reads,
            &step,
            &NoDelegatedAuthority,
            effects,
            &commission,
            &mut Generated::new(runs()),
            &mut context,
        )
        .map_err(loop_failed),
        // A merge proposed again at a HEAD the commission already handed off holds no
        // Suspension: the hold is settled, and nothing is left to approve.
        Mode::Resuming(resuming) => resume_after_grant(
            &reads,
            &step,
            resuming.authority,
            effects,
            &commission,
            &mut runs(),
            &SettledHandoffs::new(resuming.suspensions, resuming.handoffs, &held_head),
            &mut context,
        )
        .map_err(SliceError::Resume),
    };
    drop(handoff);
    drop(local);
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

    let end = result?;
    if query && let RunOutcome::Completed(ref complete) = end.outcome {
        return stop(
            out,
            protocol.to_owned(),
            end.steps,
            StopReason::Completed,
            Some(complete.outcome.clone()),
        );
    }
    let gate = if matches!(
        end.outcome,
        RunOutcome::NeedsAuthority(_) | RunOutcome::AwaitingApproval(_)
    ) {
        approval_gate(&mode, governor, &case, &commission.data().commission_id)?
    } else {
        ApprovalGate::Approvable
    };
    finish(out, protocol.to_owned(), end, gate)
}

/// The case's implementation revision as `governor` holds it: the `HEAD` the case is at.
fn implementation_revision<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
) -> Result<String, SliceError> {
    governor
        .revisions(case)?
        .remove(case::IMPLEMENTATION)
        .ok_or_else(|| {
            SliceError::State(format!(
                "case {} holds no {} revision",
                case.0,
                case::IMPLEMENTATION
            ))
        })
}

/// What a run in `mode` that stopped `ApprovalRequired` left to approve: in a resume, a `HEAD`
/// the commission already handed off first; with a state directory, whether the commission holds
/// its Suspension. A run without one is always approvable as it prints.
fn approval_gate<S: FallibleCaseStore, T: EvaluationTime>(
    mode: &Mode<'_>,
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    commission: &CommissionId,
) -> Result<ApprovalGate, SliceError> {
    let held = |suspensions: &dyn SuspensionPort| {
        let held = suspensions.get(commission).map_err(|error| {
            SliceError::State(format!(
                "whether commission {} holds a Suspension cannot be read: {error}",
                commission.0.0
            ))
        })?;
        Ok(match held {
            Some(_) => ApprovalGate::Approvable,
            None => ApprovalGate::NothingSuspended,
        })
    };
    match mode {
        Mode::Once => Ok(ApprovalGate::Approvable),
        Mode::Holding(holding) => held(holding.suspensions),
        Mode::Resuming(resuming) => {
            let head = implementation_revision(governor, case)?;
            let commission_id = crate::handoff::CommissionId(intake_model::primitives::Uuid(
                commission.0.0.clone(),
            ));
            let handed_off = resuming
                .handoffs
                .handed_off(&commission_id, &head)
                .map_err(|error| SliceError::State(error.to_string()))?;
            match handed_off {
                Some(handoff) => Ok(ApprovalGate::HandedOff {
                    action_request_id: handoff.action_request_id.0.0,
                    head: handoff.head,
                }),
                None => held(resuming.suspensions),
            }
        }
    }
}

/// The slice's failure for a loop that stopped without a run outcome: the governor's, or the
/// runtime's own.
fn loop_failed(error: LoopError) -> SliceError {
    match error {
        LoopError {
            failure: LoopFailure::Governor(error),
            ..
        } => SliceError::Governor(error),
        error => SliceError::Loop(error),
    }
}

/// The stop reason the runtime's `end` stands for, as the module documents, printed; a stop
/// `ApprovalRequired` as `gate` says what it left to approve.
fn finish(
    out: &mut dyn Write,
    protocol: String,
    end: LoopEnd,
    gate: ApprovalGate,
) -> Result<SliceRun, SliceError> {
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
            let approvals = action.iter().cloned().collect();
            gated_stop(out, protocol, steps, action, approvals, gate)
        }
        RunOutcome::AwaitingApproval(awaiting) => {
            if let Some(frontier) = &end.last_frontier {
                print_frontier(out, frontier)?;
            }
            let detail = Some(awaiting.actions.join(", "));
            gated_stop(out, protocol, steps, detail, awaiting.actions, gate)
        }
        RunOutcome::NoAdmissibleAction(_)
        | RunOutcome::NeedsExternalEvidence(_)
        | RunOutcome::IdleBound(_) => {
            stop(out, protocol, steps, StopReason::NothingAdmissible, None)
        }
        RunOutcome::CaseMovedOn(moved) => {
            let detail = format!(
                "case moved on from revision {} to {}",
                moved.bound_case_revision, moved.current_case_revision
            );
            stop(
                out,
                protocol,
                steps,
                StopReason::NothingAdmissible,
                Some(detail),
            )
        }
        RunOutcome::NoPerformableAction(unperformable) => {
            if let Some(frontier) = &end.last_frontier {
                print_frontier(out, frontier)?;
            }
            stop(out, protocol, steps, StopReason::NoLocalExecutor, None).map(|run| SliceRun {
                unperformed: unperformable.actions,
                ..run
            })
        }
        RunOutcome::NeedsHumanJudgment(needs) => Err(SliceError::Unexpected(format!(
            "NeedsHumanJudgment {needs:?}"
        ))),
        RunOutcome::EffectRefused(refused) => stop(
            out,
            protocol,
            steps,
            StopReason::Refused,
            Some(refused.reason),
        ),
        RunOutcome::Waiting(waiting) => {
            Err(SliceError::Unexpected(format!("Waiting {waiting:?}")))
        }
    }
}

/// The governor as the runtime reads it: revisions, frontiers and completions from `frontiers`,
/// observations to the governor that holds the case.
///
/// On a `software-change@1` case (`local`) every frontier is read without the actions the slice
/// can never perform ([`never_performed`]): `release.rollback`, which needs a capability and which
/// neither a run's effects nor a resume's hand-off performs. Commission hands an executor an
/// action behind an authority gate whether or not its effect port performs it, so left in, it
/// would be offered to the agent, end every run whose step leaves the frontier unchanged
/// `AwaitingApproval`, and, proposed, hold a Suspension no resume can perform. Left out, it is
/// never offered, awaited or held, as an action the slice does not perform and that needs no
/// authority is never handed to the agent; a proposal of it is refused as not in the frontier.
struct Reads<'a, S, T> {
    frontiers: &'a dyn Governor,
    governor: &'a CanonGovernor<S, T>,
    local: bool,
}

/// The actions of a `software-change@1` frontier the slice performs: the local effect adapter's
/// four, and `repository.merge`, which a resume hands off.
pub const PERFORMED: [&str; 5] = [
    crate::executor::INSPECT,
    crate::executor::TESTS_WRITE,
    crate::executor::IMPLEMENTATION_EDIT,
    crate::executor::TESTS_RUN,
    crate::handoff::MERGE,
];

/// Whether `entry` is an action the slice can never perform on a `software-change@1` case: one
/// that needs approval or names a capability, and that is not among [`PERFORMED`].
pub fn never_performed(entry: &FrontierAction) -> bool {
    let gated = entry.status == ActionStatus::ApprovalRequired
        || entry
            .capability
            .as_deref()
            .is_some_and(|capability| !capability.is_empty());
    gated && !PERFORMED.contains(&entry.action.as_str())
}

impl<S, T> Governor for Reads<'_, S, T> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.frontiers.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let frontier = self.frontiers.frontier(case)?;
        Ok(if self.local {
            performable(frontier)
        } else {
            frontier
        })
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.frontiers.completion(case)
    }
}

/// `frontier` without the actions the slice can never perform ([`never_performed`]); same id, case
/// and revision.
pub fn performable(frontier: Frontier<frontier_state::Issued>) -> Frontier<frontier_state::Issued> {
    let mut data = frontier.into_data();
    data.actions.retain(|entry| !never_performed(entry));
    Frontier::new(data)
}

/// A governor read as the slice reads one for Commission's runtime: a frontier of a
/// `software-change@1` case [`performable`], and the frontier of a case on any other protocol
/// passed through unchanged, so an action that awaits approval there (an incident's
/// `release.rollback`) stays in sight of the host. Everything else and every observation are the
/// governor's own. An embedder that runs the slice's effects under its own loop reads the
/// governor through it, so on a software change `release.rollback` is never offered, awaited or
/// held. The case's protocol is the one the governor holds it under
/// ([`CanonGovernor::protocol`]).
pub struct SliceReads<'g, G: ?Sized>(pub &'g G);

impl<S: FallibleCaseStore, T: EvaluationTime> Governor for SliceReads<'_, CanonGovernor<S, T>> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.0.current_revision(case)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let local = self.0.protocol(case)? == LOCAL_PROTOCOL;
        let frontier = self.0.frontier(case)?;
        Ok(if local {
            performable(frontier)
        } else {
            frontier
        })
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.0.completion(case)
    }
}

impl<G: ObservationPort + ?Sized> ObservationPort for SliceReads<'_, G> {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.0.observe(observation)
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> ObservationPort for Reads<'_, S, T> {
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
    let classified = runtime.block_on(crate::model_retry::classify_with_retries(
        &request.intent,
        classifier,
        request.threshold,
        None,
    ));
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

/// Prints the stop line of a run that stopped `ApprovalRequired`, as `gate` says what it left to
/// approve (see [`ApprovalGate`]), and returns the run.
fn gated_stop(
    out: &mut dyn Write,
    protocol: String,
    steps: usize,
    detail: Option<String>,
    approvals: Vec<String>,
    gate: ApprovalGate,
) -> Result<SliceRun, SliceError> {
    let reason = StopReason::ApprovalRequired;
    match (&gate, &detail) {
        (ApprovalGate::Approvable, _) => {
            return stop(out, protocol, steps, reason, detail)
                .map(|run| SliceRun { approvals, ..run });
        }
        (ApprovalGate::NothingSuspended, Some(detail)) => writeln!(
            out,
            "stopped: {reason:?} ({}), nothing suspended",
            printable(detail)
        )?,
        (ApprovalGate::NothingSuspended, None) => {
            writeln!(out, "stopped: {reason:?}, nothing suspended")?;
        }
        (
            ApprovalGate::HandedOff {
                action_request_id,
                head,
            },
            _,
        ) => writeln!(
            out,
            "stopped: MergeHandedOff ({}, HEAD {})",
            printable(action_request_id),
            printable(head)
        )?,
    }
    Ok(SliceRun {
        protocol,
        steps,
        stop_reason: reason,
        approvals,
        unperformed: Vec::new(),
        gate,
    })
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
        approvals: Vec::new(),
        unperformed: Vec::new(),
        gate: ApprovalGate::Approvable,
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
    Commission::new(operator_commission(
        CommissionId(fresh_uuid("commission", &case.0)),
        AgentRevisionId(fresh_uuid("agent-revision", &case.0)),
        case.clone(),
    ))
}

/// A commission of the slice's: for the operator, with no authority, under these ids.
pub(crate) fn operator_commission(
    commission_id: CommissionId,
    agent_revision_id: AgentRevisionId,
    case_id: CaseId,
) -> CommissionData {
    CommissionData {
        commission_id,
        agent_revision_id,
        case_id,
        principal: PrincipalId("operator".to_owned()),
        authority_context: AuthorityContext(json::Value::Null),
    }
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

#[cfg(test)]
mod tests {
    use b10x_loom_commission::model::primitives::Uuid;
    use b10x_loom_commission::model::responsibility::{
        ActionRequestId, ChargedUsage, RunCharges, RunId, RunOutcome, RunOutcomeCaseMovedOn,
        RunOutcomeEffectRefused, RunOutcomeIdleBound, RunOutcomeNoPerformableAction, Unit,
    };
    use b10x_loom_commission::runtime::LoopEnd;

    use super::{ApprovalGate, SliceRun, StopReason, finish};

    /// The end of a run with `outcome` after `steps` steps, which made no request.
    fn ended(outcome: RunOutcome, steps: usize) -> LoopEnd {
        LoopEnd {
            run_id: RunId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
            outcome,
            requests: Vec::new(),
            admitted: Vec::new(),
            effects: Vec::new(),
            steps,
            last_frontier: None,
            charges: RunCharges {
                run: ChargedUsage {
                    trials: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    duration_ms: 0,
                    cost_micro_usd: Some(0),
                },
                actions: Vec::new(),
            },
        }
    }

    /// story:refused-effect-ends-run: a run an effect port ended by refusing a request as final
    /// stops `Refused`, a stop reason the specification already declares, and its stop line
    /// carries the port's reason, so the `run --output jsonl` contract does not change.
    #[test]
    fn a_final_effect_refusal_stops_refused_with_its_reason() {
        let end = ended(
            RunOutcome::EffectRefused(RunOutcomeEffectRefused {
                action_request_id: ActionRequestId(Uuid(
                    "00000000-0000-4000-8000-000000000002".to_owned(),
                )),
                reason: "the step guard does not admit this change".to_owned(),
            }),
            1,
        );
        let mut out = Vec::new();

        let run = finish(
            &mut out,
            "software-change@1".to_owned(),
            end,
            ApprovalGate::Approvable,
        )
        .unwrap_or_else(|error| panic!("the stop failed: {error}"));

        assert_eq!(
            run,
            SliceRun {
                protocol: "software-change@1".to_owned(),
                steps: 1,
                stop_reason: StopReason::Refused,
                approvals: Vec::new(),
                unperformed: Vec::new(),
                gate: ApprovalGate::Approvable,
            }
        );
        assert_eq!(
            String::from_utf8_lossy(&out),
            "stopped: Refused (the step guard does not admit this change)\n"
        );
    }

    /// story:idle-bound-outcome: a run the idle bound ended stops `NothingAdmissible`, as it did
    /// when the runtime ended it `NoAdmissibleAction`: the same `SliceRun` and the same stop line,
    /// with no detail, so neither `b10x-loom`'s output nor the `run --output jsonl` contract
    /// changes.
    #[test]
    fn an_idle_bound_run_stops_nothing_admissible_as_before() {
        let stopped = |outcome: RunOutcome| {
            let mut out = Vec::new();
            let run = finish(
                &mut out,
                "software-change@1".to_owned(),
                ended(outcome, 1),
                ApprovalGate::Approvable,
            )
            .unwrap_or_else(|error| panic!("the stop failed: {error}"));
            (run, String::from_utf8_lossy(&out).into_owned())
        };

        let idle = stopped(RunOutcome::IdleBound(RunOutcomeIdleBound {
            idle_iterations: 2,
        }));

        assert_eq!(
            idle,
            (
                SliceRun {
                    protocol: "software-change@1".to_owned(),
                    steps: 1,
                    stop_reason: StopReason::NothingAdmissible,
                    approvals: Vec::new(),
                    unperformed: Vec::new(),
                    gate: ApprovalGate::Approvable,
                },
                "stopped: NothingAdmissible\n".to_owned()
            )
        );
        assert_eq!(
            idle,
            stopped(RunOutcome::NoAdmissibleAction(Unit(true))),
            "the stop the idle bound ended a run with on 0.26.0"
        );
    }

    /// story:unperformable-names-actions: a run Commission ended `NoPerformableAction` stops
    /// `NoLocalExecutor` carrying the actions it names, in its order, for the `run --output jsonl`
    /// terminal record; its stop line is the one it printed before, with no detail.
    #[test]
    fn a_run_with_nothing_performable_stops_no_local_executor_carrying_its_actions() {
        let mut out = Vec::new();
        let run = finish(
            &mut out,
            "incident-response@1".to_owned(),
            ended(
                RunOutcome::NoPerformableAction(RunOutcomeNoPerformableAction {
                    actions: vec!["metrics.inspect".to_owned(), "logs.search".to_owned()],
                    open_claims: vec!["service.healthy".to_owned()],
                }),
                0,
            ),
            ApprovalGate::Approvable,
        )
        .unwrap_or_else(|error| panic!("the stop failed: {error}"));

        assert_eq!(
            run,
            SliceRun {
                protocol: "incident-response@1".to_owned(),
                steps: 0,
                stop_reason: StopReason::NoLocalExecutor,
                approvals: Vec::new(),
                unperformed: vec!["metrics.inspect".to_owned(), "logs.search".to_owned()],
                gate: ApprovalGate::Approvable,
            }
        );
        assert_eq!(String::from_utf8_lossy(&out), "stopped: NoLocalExecutor\n");
    }

    /// story:moved-run-named-outcome: a run whose case moved on to a frontier that still admits an
    /// action stops as one with no admissible action does, a stop that is not a success, and its
    /// stop line names the revision the run is bound to and the current one.
    #[test]
    fn a_run_whose_case_moved_on_stops_nothing_admissible_naming_both_revisions() {
        let end = LoopEnd {
            run_id: RunId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
            outcome: RunOutcome::CaseMovedOn(RunOutcomeCaseMovedOn {
                bound_case_revision: 7,
                current_case_revision: 8,
            }),
            requests: Vec::new(),
            admitted: Vec::new(),
            effects: Vec::new(),
            steps: 2,
            last_frontier: None,
            charges: RunCharges {
                run: ChargedUsage {
                    trials: 0,
                    input_tokens: 0,
                    output_tokens: 0,
                    duration_ms: 0,
                    cost_micro_usd: Some(0),
                },
                actions: Vec::new(),
            },
        };
        let mut out = Vec::new();

        let run = finish(
            &mut out,
            "software-change@1".to_owned(),
            end,
            ApprovalGate::Approvable,
        )
        .unwrap_or_else(|error| panic!("the stop failed: {error}"));

        assert_eq!(
            run,
            SliceRun {
                protocol: "software-change@1".to_owned(),
                steps: 2,
                stop_reason: StopReason::NothingAdmissible,
                approvals: Vec::new(),
                unperformed: Vec::new(),
                gate: ApprovalGate::Approvable,
            }
        );
        assert_eq!(
            String::from_utf8_lossy(&out),
            "stopped: NothingAdmissible (case moved on from revision 7 to 8)\n"
        );
    }
}
