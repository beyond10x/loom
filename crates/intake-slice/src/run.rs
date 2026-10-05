//! The slice's loop: from an intent to a stop reason (story `slice-loop-cli`).
//!
//! [`run`] does what Commission's runtime loop will do, in this order:
//!
//! 1. extracts the intent's references ([`intake_references::references`]);
//! 2. classifies the intent ([`intake_router::classify`]) on a current-thread Tokio runtime made
//!    for that call and dropped before the loop starts, because the selector and the argument
//!    generator make their own runtime and must not run inside one;
//! 3. opens the governed case ([`crate::case::open`]);
//! 4. loops: the frontier, Loom's run over it, the local executor performing the proposed action,
//!    the verifier submitting evidence, and the completion.
//!
//! It ends with a [`SliceRun`] and its [`StopReason`] (`intake.routing.SliceRun` and
//! `intake.routing.StopReason` in `ess/intake/domains/routing.yaml`):
//!
//! | Stop reason | When |
//! | --- | --- |
//! | [`StopReason::ApprovalRequired`] | Loom proposes an action the frontier lists as needing approval, or a step leaves a frontier that lists one unchanged (every action's status and reasons); the detail names those actions in frontier order |
//! | [`StopReason::NothingAdmissible`] | Loom proposes nothing and the frontier lists no admissible action |
//! | [`StopReason::StepBudget`] | the steps taken reach the request's `max_steps` |
//! | [`StopReason::NoLocalExecutor`] | the pick is not `software-change@1`; the case opens and one frontier is read |
//! | [`StopReason::Refused`] | the router refuses the pick: outside the registry, or unsure |
//!
//! Any other failure is a [`SliceError`]: a model that gives no usable pick, a workspace the case
//! cannot open on, a governor that refuses, an executor or verifier failing on the workspace, or
//! Loom suspended because the agent model could not be reached.
//!
//! # Refused steps
//!
//! A step that is not performed still counts against the step budget, is printed as refused, and
//! is recorded in the briefing with [`Briefing::record_refusal`], so the next selection is told
//! `refused: <reason>`. Three kinds:
//!
//! - a selection Loom does not turn into a proposal while the frontier lists an admissible action:
//!   an action the frontier lists as blocked, or one it does not list;
//! - a selection answer that names no action ([`crate::selector::NO_ACTION`]);
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
//! before each Loom run and the one a step left unchanged at an approval gate (`frontier: <action>
//! (<status>), ...`), each step (`step <n>: <action>
//! <arguments>`, then indented `effect:` and `evidence:` lines; further lines of an effect are
//! indented and start with `|`), and last `stopped: <reason>`, with its detail in parentheses where
//! it has one.
//!
//! # Identity
//!
//! A [`SliceRun`] carries the specification's `protocol`, `steps` and `stop_reason`. Its `run_id`
//! and `intent_id` are not assigned here: the run is not recorded anywhere yet.

use std::fmt::{self, Write as _};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_commission::model::json;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Frontier, GovernorError, PrincipalId, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor as _;
use b10x_commission::ports::governor::Governor;
use b10x_loom::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom::selection::{Choice, SelectionContext, SelectorError};
use b10x_loom::{ActionSelector, Loom};
use governor::{CanonGovernor, CaseStore};
use intake_references::references;
use intake_router::{ProtocolPick, RouterError, classify};
use llm_core::Model;

use crate::case::{self, CaseError};
use crate::executor::{ExecuteError, INSPECT, LocalExecutor, Report, TestCommand, fresh_uuid};
use crate::selector::{Briefing, ModelArguments, ModelSelector, NO_ACTION};
use crate::verifier::{TestResultVerifier, VerifyError};

/// The one protocol the slice executes locally.
pub const LOCAL_PROTOCOL: &str = "software-change@1";

/// The producer the slice's verifier attributes its evidence to.
pub const PRODUCER: &str = "intake-slice/verifier";

/// What one run is asked to do.
#[derive(Debug, Clone)]
pub struct SliceRequest {
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

/// Why a slice run ended (`intake.routing.StopReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    ApprovalRequired,
    NothingAdmissible,
    StepBudget,
    NoLocalExecutor,
    Refused,
}

impl fmt::Display for StopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ApprovalRequired => "ApprovalRequired",
            Self::NothingAdmissible => "NothingAdmissible",
            Self::StepBudget => "StepBudget",
            Self::NoLocalExecutor => "NoLocalExecutor",
            Self::Refused => "Refused",
        })
    }
}

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
}

impl fmt::Display for SliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
/// `governor` holds the case: the case is opened on it, and the executor and verifier report to
/// it. The loop reads every frontier and the completion from `frontiers`; in use that is
/// `governor` itself. `classifier` picks the protocol, `agent` selects actions and writes their
/// arguments. Must not be called inside a Tokio runtime.
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
    let found = references(&request.intent);
    for reference in &found {
        writeln!(
            out,
            "reference: {:?} {}",
            reference.kind,
            printable(&reference.value)
        )?;
    }

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
    if pick.protocol != LOCAL_PROTOCOL {
        let frontier = frontiers.frontier(&case)?;
        print_frontier(out, &frontier)?;
        return stop(out, pick.protocol, 0, StopReason::NoLocalExecutor, None);
    }

    let briefing = Briefing::new(request.intent.clone(), found);
    let chosen = Arc::new(Mutex::new(None));
    let loom = Loom::new(
        Recording {
            inner: ModelSelector::new(agent, briefing.clone()),
            chosen: Arc::clone(&chosen),
        },
        ModelArguments::new(agent, briefing.clone()),
        request.intent.clone(),
    );
    let executor = LocalExecutor::new(
        governor,
        case.clone(),
        request.workspace.clone(),
        request.test.clone(),
    );
    let verifier = TestResultVerifier::new(governor, case.clone(), PRODUCER);
    let commission = commission(&case);

    let mut steps = 0;
    // The frontier read after the last step, when it was read to compare with the one before.
    let mut next = None;
    loop {
        if steps >= request.max_steps {
            return stop(out, pick.protocol, steps, StopReason::StepBudget, None);
        }
        let frontier = match next.take() {
            Some(frontier) => frontier,
            None => frontiers.frontier(&case)?,
        };
        print_frontier(out, &frontier)?;
        *lock(&chosen) = None;
        let outcome = loom.run(&commission, &frontier);
        let chose = lock(&chosen).take();
        match outcome {
            ExecutorOutcome::ProposedAction(proposal) => {
                let needs_approval = frontier.data().actions.iter().any(|listed| {
                    listed.action == proposal.action
                        && listed.status == ActionStatus::ApprovalRequired
                });
                if needs_approval {
                    let action = proposal.action;
                    return stop(
                        out,
                        pick.protocol,
                        steps,
                        StopReason::ApprovalRequired,
                        Some(action),
                    );
                }

                steps += 1;
                writeln!(
                    out,
                    "step {steps}: {} {}",
                    printable(&proposal.action),
                    printable(&json_text(&proposal.arguments.0))
                )?;
                perform(
                    out, &executor, &verifier, governor, &case, &briefing, &proposal,
                )?;

                if let CompletionDetermination::Complete(complete) = frontiers.completion(&case)? {
                    return Err(SliceError::Complete(complete.outcome));
                }
            }
            ExecutorOutcome::NoUsefulAction(_) => {
                let admissible = frontier
                    .data()
                    .actions
                    .iter()
                    .any(|listed| listed.status == ActionStatus::Admissible);
                if !admissible {
                    return stop(
                        out,
                        pick.protocol,
                        steps,
                        StopReason::NothingAdmissible,
                        None,
                    );
                }
                steps += 1;
                refuse_selection(out, &briefing, &frontier, chose, steps)?;
            }
            ExecutorOutcome::Suspended(_) if matches!(chose, Some(Chosen::NoAction)) => {
                steps += 1;
                refuse_selection(out, &briefing, &frontier, chose, steps)?;
            }
            ExecutorOutcome::Suspended(suspended) => {
                return Err(SliceError::Suspended(format!("{:?}", suspended.reason)));
            }
            other => return Err(SliceError::Unexpected(format!("{other:?}"))),
        }

        // A step, performed or refused, that leaves a frontier with an action needing approval
        // exactly as it was: the only useful action left needs authority.
        let gated = approval_required(&frontier);
        if !gated.is_empty() {
            let after = frontiers.frontier(&case)?;
            if unchanged(&frontier, &after) {
                print_frontier(out, &after)?;
                return stop(
                    out,
                    pick.protocol,
                    steps,
                    StopReason::ApprovalRequired,
                    Some(gated.join(", ")),
                );
            }
            next = Some(after);
        }
    }
}

/// The actions `frontier` lists as needing approval, in its order.
fn approval_required(frontier: &Frontier<frontier_state::Issued>) -> Vec<String> {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.status == ActionStatus::ApprovalRequired)
        .map(|listed| listed.action.clone())
        .collect()
}

/// Whether `after` lists the same actions as `before`, each with the same status and reasons.
fn unchanged(
    before: &Frontier<frontier_state::Issued>,
    after: &Frontier<frontier_state::Issued>,
) -> bool {
    let (before, after) = (&before.data().actions, &after.data().actions);
    before.len() == after.len()
        && before.iter().zip(after).all(|(was, is)| {
            was.action == is.action && was.status == is.status && was.reasons == is.reasons
        })
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

/// Performs one proposal, verifies what it reported and records it in the briefing.
fn perform<S: CaseStore>(
    out: &mut dyn Write,
    executor: &LocalExecutor<'_, S>,
    verifier: &TestResultVerifier<'_, S>,
    governor: &CanonGovernor<S>,
    case: &CaseId,
    briefing: &Briefing,
    proposal: &ExecutorOutcomeProposedAction,
) -> Result<(), SliceError> {
    let report = match executor.execute(proposal) {
        Ok(report) => report,
        Err(
            refused @ (ExecuteError::NotExecuted { .. }
            | ExecuteError::OutsideWorkspace { .. }
            | ExecuteError::Ignored { .. }
            | ExecuteError::InvalidArguments { .. }),
        ) => return refuse(out, briefing, &proposal.action, &refused.to_string()),
        // An inspect that cannot read a path (most often one that does not exist) changed nothing.
        Err(refused @ ExecuteError::Workspace { .. }) if proposal.action == INSPECT => {
            return refuse(out, briefing, &proposal.action, &refused.to_string());
        }
        Err(error) => return Err(SliceError::Execute(error)),
    };
    print_effect(out, &report)?;
    match verifier.verify(&report) {
        Ok(Some(evidence)) => {
            // What the governor holds, not what the report says.
            let held = governor.evidence(case)?;
            let record = held.iter().find(|record| record.evidence_id == evidence);
            let kind = record.map_or("unknown", |record| record.kind.as_str());
            let result = match record.and_then(|record| record.facts.member("result")) {
                Some(json::Value::Text(result)) => result.as_str(),
                _ => "unknown",
            };
            writeln!(out, "  evidence: {} {}", printable(kind), printable(result))?;
        }
        Ok(None) => writeln!(out, "  evidence: none")?,
        Err(VerifyError::NoRevision) => {
            writeln!(out, "  evidence: none ({})", VerifyError::NoRevision)?;
        }
        Err(error) => return Err(SliceError::Verify(error)),
    }
    briefing.record(proposal, &report);
    Ok(())
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
        Some(detail) => writeln!(out, "stopped: {stop_reason} ({})", printable(&detail))?,
        None => writeln!(out, "stopped: {stop_reason}")?,
    }
    Ok(SliceRun {
        protocol,
        steps,
        stop_reason,
    })
}

fn print_frontier(
    out: &mut dyn Write,
    frontier: &Frontier<frontier_state::Issued>,
) -> Result<(), SliceError> {
    let actions: Vec<String> = frontier
        .data()
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

/// The report's first line as the effect, every further line indented and marked with `|`, so no
/// line of a report can pass for a line of the run's own output.
fn print_effect(out: &mut dyn Write, report: &Report) -> Result<(), SliceError> {
    let text = report.to_string();
    let mut lines = text.lines();
    writeln!(
        out,
        "  effect: {}",
        printable(lines.next().unwrap_or_default())
    )?;
    for line in lines {
        writeln!(out, "    | {}", printable(line))?;
    }
    Ok(())
}

/// Prints a step that was not performed, for `reason`, and tells the model so.
fn refuse(
    out: &mut dyn Write,
    briefing: &Briefing,
    action: &str,
    reason: &str,
) -> Result<(), SliceError> {
    writeln!(out, "  effect: refused: {}", printable(reason))?;
    writeln!(out, "  evidence: none")?;
    briefing.record_refusal(action, reason);
    Ok(())
}

/// Prints step `step`, a selection Loom did not turn into a proposal, as refused, and tells the
/// model why.
fn refuse_selection(
    out: &mut dyn Write,
    briefing: &Briefing,
    frontier: &Frontier<frontier_state::Issued>,
    chose: Option<Chosen>,
    step: usize,
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
    writeln!(out, "step {step}: {} (not proposed)", printable(&action))?;
    refuse(out, briefing, &action, &reason)
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

fn lock(chosen: &Mutex<Option<Chosen>>) -> MutexGuard<'_, Option<Chosen>> {
    chosen.lock().unwrap_or_else(PoisonError::into_inner)
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
fn json_text(value: &json::Value) -> String {
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
