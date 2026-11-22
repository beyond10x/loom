//! `tree run`: the tree driven on Commission's loop, one step of one case at a time, until the
//! root is terminated through its one legitimate outcome or the tree waits.
//!
//! Each step is one [`run_until_blocked`] of one case with a step budget of one: the Loom executor
//! over the [`DeterministicSelector`] and the governor's action facts, with the history the case's
//! run log gives ([`LogHistory`]); the tree's authority ([`TreeAuthority`]); and the case's bound
//! executors as its effect port ([`TreeEffects`]). Every selected action is revalidated by the
//! loop against the case revision, the frontier and authority before its effect.
//!
//! Before each step the run asks the cancel handle, the step budget and the wall-time budget, in
//! that order: the handle ends the run `Cancelled`, naming the action that was to be taken next;
//! either budget ends it `BudgetSpent`, and the `stopped` line names it as `budget` (`MaxSteps`,
//! `MaxWallSeconds`). A handle set during a step ends the program the step runs
//! (`story:cancel-a-running-bound-program`): a thread of the run reads the caller's flag every
//! [`CANCEL_WATCH`] and sets the [`Cancel`] every port of the run holds; the step is refused
//! `Cancelled`, its `action finished` line written, and the run ends `Cancelled` naming its action.
//! A `handoff_when` check holds the same [`Cancel`], before a step and at a stuck stop
//! (`story:handoff-check-holds-the-cancel`): a check it ended has its `handoff checked` line
//! refused `Cancelled`, and the run ends `Cancelled` naming the action, whose program never starts.
//! A case whose completion gives exactly one legitimate outcome and in which no action is useful
//! is terminated through it.
//! A case in which nothing is useful waits: on authority (an admissible action whose capability no
//! approval allows), on a person (an admissible action bound to `person`, or two legitimate
//! outcomes to close between), or is stuck.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use b10x_loom_bindings::{
    CopyEffects, OpenEffects, OpenPortError, PlacedRevisions, ProgramEffects, retry_records,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionJson;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier,
    PrincipalId, ProposedActionArguments, RunId, RunOutcome, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopEnd, run_until_blocked};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::selection::{ActionSelector as _, SelectionContext, StepHistory as _};
use b10x_loom_executor::{DeterministicSelector, EmptyObjectArguments, Loom};
use b10x_loom_intake_slice::confinement::{Cancel, TestRunner};
use b10x_loom_runlog::{BUDGET_SPENT, budget_name, log_file_name};
use loom::bindings::{BindingsDocument, Executor, ResultSource};
use loom::runlog::{
    ActionFinished, CaseOpened, ChildClosed, ChildOpened, Evaluated, RunLogEvent, RunLogLine,
    Stopped, Terminated,
};
use loom::tree::{
    ApprovalDecision, DecisionKind, NextDecision, SpentBudget, TreeManifest, TreeRefusalReason,
    TreeRun, TreeStop,
};
use loom_governor::{CanonGovernor, FileCaseStore};
use loom_protocols::ProtocolCatalog;
use sha2::{Digest as _, Sha256};

use crate::approvals::read_approvals;
use crate::context::{StepContext, derived_uuid};
use crate::handoff::{CheckHeld, Checked, HandoffChecks};
use crate::history::LogHistory;
use crate::log::Logs;
use crate::open::load;
use crate::status::members;
use crate::store::{existing_store, read_manifest, read_regular};
use crate::{Refused, StoreLock, TreeAuthority, TreeTime, unreadable};

/// The principal a tree's commission acts for.
pub const TREE_PRINCIPAL: &str = "loom-tree";

/// The principal a printed `tree approve` command names for a capability no `person` binding of
/// the case holds.
pub const DEFAULT_PRINCIPAL: &str = "operator";

/// Called with each run-log line right after it is written, with the case whose log it is.
pub trait RunObserver {
    /// One line of `case`'s log, as written.
    fn line(&mut self, case: &str, line: &RunLogLine);
}

impl<F: FnMut(&str, &RunLogLine)> RunObserver for F {
    fn line(&mut self, case: &str, line: &RunLogLine) {
        self(case, line);
    }
}

/// How a run goes.
#[derive(Clone)]
pub struct RunSettings {
    /// The most steps the run takes; `None` sets no bound.
    pub max_steps: Option<i64>,
    /// The most seconds of wall time the run takes before a step; `None` sets no bound.
    pub max_wall_seconds: Option<i64>,
    /// Where the run reads its instants.
    pub time: TreeTime,
    /// The runner every bound program of the tree starts through.
    pub runner: Arc<dyn TestRunner>,
    /// The `--confinement` value of the runner, which the `tree run` a handoff prints carries so
    /// that, run unchanged, it starts its programs as this run does; `None` prints no flag.
    pub confinement: Option<String>,
    /// The cgroup root the run was named (`--cgroup-root`), which that `tree run` carries too, so
    /// that it runs under the root this run did; `None` prints no flag, as for a run that started
    /// itself again in a scope of its own.
    pub cgroup_root: Option<PathBuf>,
}

impl std::fmt::Debug for RunSettings {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("RunSettings")
            .field("max_steps", &self.max_steps)
            .field("max_wall_seconds", &self.max_wall_seconds)
            .field("time", &self.time)
            .field("runner", &self.runner.label())
            .field("confinement", &self.confinement)
            .field("cgroup_root", &self.cgroup_root)
            .finish()
    }
}

/// What a case of the run is.
pub(crate) struct Held {
    pub(crate) case: String,
    pub(crate) parent: Option<String>,
    pub(crate) file: PathBuf,
    pub(crate) bindings: BindingsDocument,
}

/// How often a run reads the caller's cancel flag while a step runs.
pub const CANCEL_WATCH: Duration = Duration::from_millis(100);

/// Runs the tree in `store` with `catalog` (see the module documentation). `observer` is given
/// each run-log line as it is written; `cancel`, set from any thread, ends the run `Cancelled`:
/// before its next step, or during a step by ending the program or the `handoff_when` check it
/// runs, within 10 s of being set.
///
/// # Errors
/// [`Refused`]: `StoreInUse` naming the holder, `NotATree`, `TreeStopped` naming the hold and the
/// `tree reopen` command that lifts it ([`crate::hold`]; nothing is written), `LogRefused`,
/// `BindingsRefused`, `ConfinementRefused` or `StoreUnreadable`.
pub fn run_tree(
    store: &Path,
    catalog: &ProtocolCatalog,
    settings: &RunSettings,
    observer: Option<&mut dyn RunObserver>,
    cancel: Option<&AtomicBool>,
) -> Result<TreeRun, Refused> {
    let handle = Cancel::new();
    let Some(flag) = cancel else {
        return run_cancellable(store, catalog, settings, observer, &handle, None);
    };
    // The bridge from the caller's flag to the run's handle: a thread that reads the flag every
    // `CANCEL_WATCH` and sets the handle, until the run returns.
    let returned = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !returned.load(Ordering::SeqCst) {
                if flag.load(Ordering::SeqCst) {
                    handle.cancel();
                    return;
                }
                std::thread::sleep(CANCEL_WATCH);
            }
        });
        // Set however the run returns, a panic included, so the scope ends.
        let _returned = SetOnDrop(&returned);
        run_cancellable(store, catalog, settings, observer, &handle, Some(flag))
    })
}

/// Sets its flag when dropped.
struct SetOnDrop<'a>(&'a AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// [`run_tree`] with `cancel`, the handle every port of the run holds, and `flag`, the caller's.
fn run_cancellable(
    store: &Path,
    catalog: &ProtocolCatalog,
    settings: &RunSettings,
    mut observer: Option<&mut dyn RunObserver>,
    cancel: &Cancel,
    flag: Option<&AtomicBool>,
) -> Result<TreeRun, Refused> {
    let cancelled =
        || cancel.is_cancelled() || flag.is_some_and(|flag| flag.load(Ordering::SeqCst));
    let store = existing_store(store)?;
    read_manifest(&store)?;
    let _held = StoreLock::acquire(&store)?;
    let manifest = read_manifest(&store)?;
    // A held tree runs nothing: refused holding the store, before any store opens or line is
    // written (`story:tree-suite-stop-resume-reopen-steps`).
    crate::stop::refuse_held(&store, &manifest.root)?;
    let workspace = PathBuf::from(&manifest.workspace);
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let logs = Logs::open(&store, settings.time.clone())?;
    let started = Instant::now();
    let wall = settings
        .max_wall_seconds
        .map(|seconds| (started, u64::try_from(seconds).unwrap_or(0)));
    let mut steps = 0i64;
    let mut emit = |case: &str, line: RunLogLine| {
        if let Some(observer) = observer.as_deref_mut() {
            observer.line(case, &line);
        }
    };

    // A child event's two lines are written one after the other in this process (`opened`,
    // `closed`), so only a run stopped before this one leaves one of them out: taken over once.
    let held = tree_cases(&governor, &manifest.root, &manifest.bindings, catalog)?;
    for case in &held {
        logs.reconcile(&case.case)?;
    }
    take_over(&governor, &logs, &store, &held, &mut emit)?;

    loop {
        let held = tree_cases(&governor, &manifest.root, &manifest.bindings, catalog)?;
        for case in &held {
            logs.reconcile(&case.case)?;
        }
        let cancelled_before = cancelled();
        let stop_before = if cancelled_before {
            None
        } else if settings.max_steps.is_some_and(|max| steps >= max) {
            Some((
                SpentBudget::MaxSteps,
                format!("the run took its {steps} steps"),
            ))
        } else if wall.is_some_and(|(started, seconds)| started.elapsed().as_secs() >= seconds) {
            Some((
                SpentBudget::MaxWallSeconds,
                format!(
                    "the run's {} s of wall time are spent",
                    settings.max_wall_seconds.unwrap_or_default()
                ),
            ))
        } else {
            None
        };
        // The deepest open case first: every open child before its parent's next step.
        let Some(case) = held
            .iter()
            .rev()
            .find(|case| !terminated(&store, &case.case))
        else {
            return Ok(TreeRun {
                stop: TreeStop::Terminal,
                case: manifest.root.clone(),
                steps,
                decisions: Vec::new(),
                detail: None,
            });
        };
        if cancelled_before {
            let next = next_action(
                &governor,
                case,
                catalog,
                &workspace,
                &store,
                &settings.runner,
                cancel,
            )?;
            return cancelled_stop(&logs, &case.case, next, steps, &mut emit);
        }
        if let Some((budget, detail)) = stop_before {
            let line = logs.append(
                &case.case,
                RunLogEvent::Stopped(Stopped {
                    stop: BUDGET_SPENT.to_owned(),
                    next: Vec::new(),
                    budget: Some(budget_name(budget).to_owned()),
                }),
            )?;
            emit(&case.case, line);
            return Ok(TreeRun {
                stop: TreeStop::BudgetSpent,
                case: case.case.clone(),
                steps,
                decisions: Vec::new(),
                detail: Some(detail),
            });
        }
        let id = CaseId(case.case.clone());
        // What the workspace holds now: a file changed between two steps or two runs moves its
        // artifact before the case is evaluated.
        PlacedRevisions::new(&governor, &case.bindings, &workspace)
            .refresh(&id)
            .map_err(|error| {
                Refused::new(TreeRefusalReason::StoreUnreadable, error.to_string())
                    .case(case.case.clone())
            })?;
        if let Some(line) = evaluated(&governor, &logs, &store, &case.case)? {
            emit(&case.case, line);
        }
        let legitimate = legitimate_outcomes(&governor, &id)?;
        let before: BTreeSet<String> = evidence_ids(&governor, &id)?;
        let (lines, revision, effects, history) = step_inputs(
            &governor,
            case,
            catalog,
            &workspace,
            &store,
            &settings.runner,
            cancel,
        )?;
        let selector = DeterministicSelector::new(id.clone(), &governor, &history);
        let executor = Loom::new(selector, EmptyObjectArguments, "Run the tree's case.")
            .with_governor(&governor)
            .with_instance(format!("tree-{}", case.case));
        let ordinal = i64::try_from(lines.len()).unwrap_or(i64::MAX);
        let checks = HandoffChecks {
            governor: &governor,
            logs: &logs,
            case: &id,
            bindings: &case.bindings,
            workspace: &workspace,
            store: &store,
            runner: &settings.runner,
            cancel,
        };
        // The `handoff_when` check of the action the selector picks runs before the loop
        // performs it, and its line is written before the step or the stop.
        let checked = Checked::new(&executor, &checks);
        let end = drive(
            &governor,
            &store,
            &id,
            revision,
            ordinal,
            &checked,
            &effects,
            &settings.time,
            wall,
        )?;
        let (check_lines, held_by_check) = checked.finish();
        for line in check_lines {
            emit(&case.case, line);
        }
        match held_by_check {
            Some(CheckHeld::Refused(refused)) => return Err(refused),
            Some(CheckHeld::Cancelled(action)) => {
                return cancelled_stop(&logs, &case.case, vec![action], steps, &mut emit);
            }
            Some(CheckHeld::HandedOff(action)) => {
                let decisions = handoff_decision(
                    &id,
                    &case.bindings,
                    &action,
                    &handoff_command(&store, settings),
                )
                .into_iter()
                .collect::<Vec<_>>();
                let line = logs.append(
                    &case.case,
                    RunLogEvent::Stopped(Stopped {
                        stop: format!("{:?}", TreeStop::AwaitingPerson),
                        next: decisions
                            .iter()
                            .map(|decision| decision.decides.clone())
                            .collect(),
                        budget: None,
                    }),
                )?;
                emit(&case.case, line);
                return Ok(TreeRun {
                    stop: TreeStop::AwaitingPerson,
                    case: case.case.clone(),
                    steps,
                    decisions,
                    detail: None,
                });
            }
            None => {}
        }

        if let (Some(request), Some(outcome)) = (end.admitted.last(), end.effects.last()) {
            steps += 1;
            let after = evidence_ids(&governor, &id)?;
            let (exit, refusal, blob_digest) = effect_summary(outcome);
            let finished = ActionFinished {
                action: request.action.clone(),
                exit,
                refusal,
                blob_digest,
                evidence: after.difference(&before).cloned().collect(),
                revisions: Vec::new(),
            };
            let cancelled_step = crate::history::cancelled(&finished);
            let line = logs.append(&case.case, RunLogEvent::ActionFinished(finished))?;
            emit(&case.case, line);
            if cancelled_step {
                return cancelled_stop(
                    &logs,
                    &case.case,
                    vec![request.action.clone()],
                    steps,
                    &mut emit,
                );
            }
            opened(&governor, &logs, &case.case, outcome, &mut emit)?;
            continue;
        }

        // The deterministic selector takes only admissible actions. One that awaits approval is
        // proposed once `tree approve` allowed its capability for the case at this revision; the
        // loop revalidates it and asks the tree's authority, as for any other request.
        let approvals = TreeAuthority::new(BTreeMap::from([(
            case.case.clone(),
            read_approvals(&store, &case.case)?,
        )]));
        let approved = waiting_actions(&governor, &id, &case.bindings, &history, revision)?
            .into_iter()
            .find(|(action, capability, kind)| {
                *kind != Executor::Person
                    && effects.performs(action)
                    && capability.as_deref().is_some_and(|capability| {
                        approvals.decision(&case.case, capability, revision)
                            == Some(ApprovalDecision::Allow)
                    })
            });
        if let Some((action, _, _)) = approved {
            let end = drive(
                &governor,
                &store,
                &id,
                revision,
                ordinal + 1,
                &Propose(action.clone()),
                &effects,
                &settings.time,
                wall,
            )?;
            if let Some(outcome) = end.effects.last() {
                steps += 1;
                let after = evidence_ids(&governor, &id)?;
                let (exit, refusal, blob_digest) = effect_summary(outcome);
                let finished = ActionFinished {
                    action: action.clone(),
                    exit,
                    refusal,
                    blob_digest,
                    evidence: after.difference(&before).cloned().collect(),
                    revisions: Vec::new(),
                };
                let cancelled_step = crate::history::cancelled(&finished);
                let line = logs.append(&case.case, RunLogEvent::ActionFinished(finished))?;
                emit(&case.case, line);
                if cancelled_step {
                    return cancelled_stop(&logs, &case.case, vec![action], steps, &mut emit);
                }
                opened(&governor, &logs, &case.case, outcome, &mut emit)?;
                continue;
            }
        }

        // No step was taken: the case completes, waits or is stuck.
        let completed = matches!(end.outcome, RunOutcome::Completed(_));
        if legitimate.len() == 1
            && (completed
                || waiting_actions(&governor, &id, &case.bindings, &history, revision)?.is_empty())
        {
            let instant = settings.time.now()?;
            governor
                .terminate(&id, &legitimate[0], &instant.0)
                .map_err(|error| {
                    Refused::new(TreeRefusalReason::StoreUnreadable, error.to_string())
                        .case(case.case.clone())
                })?;
            closed(&governor, &logs, case, &legitimate[0], &mut emit)?;
            if case.case == manifest.root {
                return Ok(TreeRun {
                    stop: TreeStop::Terminal,
                    case: case.case.clone(),
                    steps,
                    decisions: Vec::new(),
                    detail: Some(legitimate[0].clone()),
                });
            }
            continue;
        }
        let (stop, decisions) = match waits(
            &governor,
            &id,
            &case.bindings,
            &legitimate,
            &store,
            &workspace,
            revision,
            &history,
            &handoff_command(&store, settings),
            &confinement_flags(settings),
            &checks,
            &mut emit,
        )? {
            Waits::Stop(stop, decisions) => (stop, decisions),
            Waits::CheckCancelled(action) => {
                return cancelled_stop(&logs, &case.case, vec![action], steps, &mut emit);
            }
        };
        let line = logs.append(
            &case.case,
            RunLogEvent::Stopped(Stopped {
                stop: format!("{stop:?}"),
                next: decisions
                    .iter()
                    .map(|decision| decision.decides.clone())
                    .collect(),
                budget: None,
            }),
        )?;
        emit(&case.case, line);
        return Ok(TreeRun {
            stop,
            case: case.case.clone(),
            steps,
            decisions,
            detail: None,
        });
    }
}

/// The cases of the tree, the root first and each before its children, with the bindings each
/// runs under.
pub(crate) fn tree_cases<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    root: &str,
    root_bindings: &str,
    catalog: &ProtocolCatalog,
) -> Result<Vec<Held>, Refused> {
    let mut held: Vec<Held> = Vec::new();
    for member in members(governor, root)? {
        let id = CaseId(member.case.clone());
        let protocol = governor
            .protocol(&id)
            .map_err(|error| unreadable(format!("case {}", member.case), format!("{error:?}")))?;
        let entry = catalog.get(&protocol).ok_or_else(|| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog holds no protocol registered as {protocol}"),
            )
            .case(member.case.clone())
        })?;
        let file = match (&member.parent, &member.link) {
            (Some(parent), Some(link)) => {
                let parent = held
                    .iter()
                    .find(|held| held.case == *parent)
                    .ok_or_else(|| {
                        unreadable(
                            format!("case {}", member.case),
                            format!("its parent {parent} is not listed before it"),
                        )
                    })?;
                child_file(parent, &link.action)?
            }
            _ => PathBuf::from(root_bindings),
        };
        let loaded = load(&file, entry.name(), &entry.model)?;
        held.push(Held {
            case: member.case,
            parent: member.parent,
            file,
            bindings: loaded.document,
        });
    }
    Ok(held)
}

/// The bindings file of the child that `action` of `parent` opened: the `bindings` member of its
/// `open` executor, relative to the directory of the parent's file, whose real path lies below
/// that directory.
fn child_file(parent: &Held, action: &str) -> Result<PathBuf, Refused> {
    let refused = |detail: String| {
        Refused::new(TreeRefusalReason::BindingsRefused, detail)
            .file(parent.file.display().to_string())
            .action(action.to_owned())
    };
    let relative = parent
        .bindings
        .actions
        .iter()
        .find(|bound| bound.action == action && bound.executor.kind == Executor::Open)
        .and_then(|bound| bound.executor.bindings.clone())
        .ok_or_else(|| {
            refused(format!(
                "case {} names no open executor with a bindings file for {action}",
                parent.case
            ))
        })?;
    let directory = parent.file.parent().map(Path::to_owned).unwrap_or_default();
    let real = std::fs::canonicalize(directory.join(&relative)).map_err(|error| {
        refused(format!(
            "the child bindings file {relative:?} cannot be read: {error}"
        ))
    })?;
    let base = std::fs::canonicalize(&directory).unwrap_or(directory);
    if !real.starts_with(&base) {
        return Err(refused(format!(
            "the child bindings file {relative:?} lies outside the directory of the file that names it"
        )));
    }
    Ok(real)
}

/// Writes that `case` ended through `outcome`: its `terminated` line, then, for a child, the
/// `child closed` line in its parent's log, naming the child's record as the termination replaced
/// it.
fn closed<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    case: &Held,
    outcome: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    closed_lines(
        governor,
        logs,
        &case.case,
        case.parent.as_deref(),
        outcome,
        emit,
    )
}

/// Appends `terminated` to `case`'s log and, for a child, `child closed` to its `parent`'s, naming
/// the child's record as the termination replaced it.
pub(crate) fn closed_lines<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    case: &str,
    parent: Option<&str>,
    outcome: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    terminated_line(logs, case, outcome, emit)?;
    if let Some(parent) = parent {
        child_closed_line(governor, logs, parent, case, outcome, emit)?;
    }
    Ok(())
}

/// Appends `terminated` through `outcome` to `case`'s log.
fn terminated_line(
    logs: &Logs,
    case: &str,
    outcome: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let line = logs.append(
        case,
        RunLogEvent::Terminated(Terminated {
            reason: outcome.to_owned(),
        }),
    )?;
    emit(case, line);
    Ok(())
}

/// Appends `child closed` of `child` through `outcome` to `parent`'s log, naming the child's
/// record as the termination replaced it.
fn child_closed_line<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    parent: &str,
    child: &str,
    outcome: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let (record, record_digest) = child_record(governor, parent, child)?;
    let line = logs.append(
        parent,
        RunLogEvent::ChildClosed(ChildClosed {
            child: child.to_owned(),
            outcome: Some(outcome.to_owned()),
            record,
            record_digest,
        }),
    )?;
    emit(parent, line);
    Ok(())
}

/// Appends `case opened` to `child`'s log, naming the protocol the governor holds it under.
fn case_opened_line<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    child: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let line = logs.append(
        child,
        RunLogEvent::CaseOpened(CaseOpened {
            case: child.to_owned(),
            protocol: child_protocol(governor, child)?,
        }),
    )?;
    emit(child, line);
    Ok(())
}

/// Appends `child opened` of `child` to `parent`'s log, naming the child's record as the parent
/// holds it now.
fn child_opened_line<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    parent: &str,
    child: &str,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let (record, record_digest) = child_record(governor, parent, child)?;
    let line = logs.append(
        parent,
        RunLogEvent::ChildOpened(ChildOpened {
            child: child.to_owned(),
            protocol: child_protocol(governor, child)?,
            record,
            record_digest,
        }),
    )?;
    emit(parent, line);
    Ok(())
}

/// The registration the governor holds `case` under.
fn child_protocol<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> Result<String, Refused> {
    governor
        .protocol(&CaseId(case.to_owned()))
        .map_err(|error| unreadable(format!("case {case}"), format!("{error:?}")))
}

/// The outcome the governor terminated `case` through, when it did.
pub(crate) fn termination<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> Result<Option<String>, Refused> {
    Ok(governor
        .case_state(&CaseId(case.to_owned()))
        .map_err(|error| unreadable(format!("case {case}"), format!("{error:?}")))?
        .termination
        .map(|termination| termination.outcome))
}

/// Writes the lines of a child event a run stopped between its two appends left out, each only
/// when its log does not hold it yet, so a second writer writes none again. In the order of
/// `held`, each case before its children: for every child the governor links, its `case opened`
/// line and the parent's `child opened` line naming it; then for every case the governor holds
/// terminated, its `terminated` line and, for a child, the parent's `child closed` line naming it.
///
/// Each log is read once, before anything is written: a line written here is of another kind, or
/// names another child, than every line looked for after it in the same log.
fn take_over<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    store: &Path,
    held: &[Held],
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let mut read = BTreeMap::new();
    for case in held {
        read.insert(case.case.as_str(), read_lines(store, &case.case)?);
    }
    let holds = |case: &str, wanted: &dyn Fn(&RunLogEvent) -> bool| {
        read.get(case)
            .is_some_and(|lines| lines.iter().any(|line| wanted(&line.event)))
    };
    for case in held {
        let Some(parent) = case.parent.as_deref() else {
            continue;
        };
        let child = case.case.as_str();
        if !holds(child, &|event| matches!(event, RunLogEvent::CaseOpened(_))) {
            case_opened_line(governor, logs, child, emit)?;
        }
        if !holds(
            parent,
            &|event| matches!(event, RunLogEvent::ChildOpened(opened) if opened.child == child),
        ) {
            child_opened_line(governor, logs, parent, child, emit)?;
        }
    }
    for case in held {
        let Some(outcome) = termination(governor, &case.case)? else {
            continue;
        };
        if !holds(&case.case, &|event| {
            matches!(event, RunLogEvent::Terminated(_))
        }) {
            terminated_line(logs, &case.case, &outcome, emit)?;
        }
        if let Some(parent) = case.parent.as_deref()
            && !holds(
                parent,
                &|event| matches!(event, RunLogEvent::ChildClosed(closed) if closed.child == case.case),
            )
        {
            child_closed_line(governor, logs, parent, &case.case, &outcome, emit)?;
        }
    }
    Ok(())
}

/// Writes, for each child an `open` step opened, its `case opened` line and the parent's
/// `child opened` line, naming the child's record as the opening wrote it.
fn opened<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    parent: &str,
    outcome: &EffectOutcome,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<(), Refused> {
    let EffectOutcome::Performed(performed) = outcome else {
        return Ok(());
    };
    let report = plain_commission(&performed.report);
    let Some(children) = report["children"].as_array() else {
        return Ok(());
    };
    for child in children.iter().filter_map(serde_json::Value::as_str) {
        case_opened_line(governor, logs, child, emit)?;
        child_opened_line(governor, logs, parent, child, emit)?;
    }
    Ok(())
}

/// The lines of `case`'s log in `store`.
pub(crate) fn read_lines(store: &Path, case: &str) -> Result<Vec<RunLogLine>, Refused> {
    let bytes = read_regular(&store.join(log_file_name(case)))?.unwrap_or_default();
    b10x_loom_runlog::verify(&bytes).map_err(|refused| {
        Refused::new(
            TreeRefusalReason::LogRefused,
            format!(
                "line {}: {:?}: {}",
                refused.line, refused.refusal, refused.detail
            ),
        )
        .case(case)
    })
}

/// Whether `case`'s log holds its `terminated` line.
fn terminated(store: &Path, case: &str) -> bool {
    read_lines(store, case).is_ok_and(|lines| {
        lines
            .iter()
            .any(|line| matches!(line.event, RunLogEvent::Terminated(_)))
    })
}

/// Appends `evaluated` to `case`'s log when the case revision or the decision differs from the
/// last `evaluated` line's.
fn evaluated<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    logs: &Logs,
    store: &Path,
    case: &str,
) -> Result<Option<RunLogLine>, Refused> {
    let id = CaseId(case.to_owned());
    let frontier = governor
        .frontier(&id)
        .map_err(|error| unreadable(format!("case {case}"), format!("{error:?}")))?;
    let data = frontier.data();
    let document = governor
        .decision_document(&id, &data.frontier_id)
        .map_err(|error| unreadable(format!("case {case}"), format!("{error:?}")))?;
    let digest = document.as_ref().map_or_else(String::new, |document| {
        let bytes = serde_json::to_vec(&plain(document)).unwrap_or_default();
        hex(&Sha256::digest(&bytes))
    });
    let last = read_lines(store, case)?
        .into_iter()
        .rev()
        .find_map(|line| match line.event {
            RunLogEvent::Evaluated(evaluated) => Some(evaluated),
            _ => None,
        });
    if last.as_ref().is_some_and(|last| {
        last.case_revision == data.case_revision && last.decision_digest == digest
    }) {
        return Ok(None);
    }
    logs.append(
        case,
        RunLogEvent::Evaluated(Evaluated {
            case_revision: data.case_revision,
            frontier: data.frontier_id.0.0.clone(),
            decision_digest: digest,
        }),
    )
    .map(Some)
}

/// The outcomes Canon reports `legitimate` for `case` now, in the decision's order.
pub(crate) fn legitimate_outcomes<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
) -> Result<Vec<String>, Refused> {
    let failed = |error: b10x_loom_commission::model::responsibility::GovernorError| {
        unreadable(format!("case {}", case.0), format!("{error:?}"))
    };
    let frontier = governor.frontier(case).map_err(failed)?;
    let Some(document) = governor
        .decision_document(case, &frontier.data().frontier_id)
        .map_err(failed)?
    else {
        return Ok(Vec::new());
    };
    let outcomes = plain(&document);
    Ok(outcomes
        .get("outcomes")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(_, entry)| entry["status"] == "legitimate")
        .map(|(outcome, _)| outcome.clone())
        .collect())
}

/// The actions due again whatever their history: each writer with a retry record.
fn due_actions<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    bindings: &BindingsDocument,
) -> Result<BTreeSet<String>, Refused> {
    let mut due = BTreeSet::new();
    for binding in &bindings.actions {
        let records = retry_records(governor, case, binding)
            .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?;
        if !records.is_empty() {
            due.insert(binding.action.clone());
        }
    }
    Ok(due)
}

/// What a step of `case` is chosen from: the lines of its log, its revision, its effect port
/// (every program of it holding `cancel`) and its history, with the actions due again (a writer
/// with a retry record, a copy whose source holds something new) but those refused since the case
/// was last evaluated (a cancel is no such refusal), and the idle actions: the copies whose source
/// holds nothing new and the writers whose only open evidence is a spent allowance
/// ([`spent_writers_idle`]).
#[allow(clippy::type_complexity)]
fn step_inputs<'a, S: loom_governor::FallibleCaseStore>(
    governor: &'a CanonGovernor<S>,
    case: &'a Held,
    catalog: &ProtocolCatalog,
    workspace: &Path,
    store: &Path,
    runner: &Arc<dyn TestRunner>,
    cancel: &Cancel,
) -> Result<(Vec<RunLogLine>, i64, TreeEffects<'a, S>, LogHistory), Refused> {
    let id = CaseId(case.case.clone());
    let lines = read_lines(store, &case.case)?;
    let mut due = due_actions(governor, &id, &case.bindings)?;
    // A step refused since the case was last evaluated is refused again until the case
    // changes (a spent run budget): it is not due, whatever its retry records say. A step its
    // cancel ended is no such refusal: it stays due.
    let refused_since: BTreeSet<String> = lines
        .iter()
        .rev()
        .take_while(|line| !matches!(line.event, RunLogEvent::Evaluated(_)))
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished)
                if finished.refusal.is_some() && !crate::history::cancelled(finished) =>
            {
                Some(finished.action.clone())
            }
            _ => None,
        })
        .collect();
    due.retain(|action| !refused_since.contains(action));
    let revision = governor
        .current_revision(&id)
        .map_err(|error| unreadable(format!("case {}", case.case), format!("{error:?}")))?;
    let effects = TreeEffects::new(
        governor,
        &id,
        &case.bindings,
        &case.file,
        catalog,
        workspace,
        store,
        runner,
        cancel,
    )?;
    // A copy is taken when its source holds something new, and is idle otherwise.
    let mut idle = BTreeSet::new();
    for (action, copy_due) in effects.copies_due(&case.bindings)? {
        if copy_due {
            due.insert(action);
        } else {
            idle.insert(action);
        }
    }
    idle.extend(spent_writers_idle(governor, &id, &case.bindings, &due)?);
    let history = LogHistory::new(&lines, revision, due).with_idle(idle);
    Ok((lines, revision, effects, history))
}

/// The writers idle for this step because only a spent allowance is open
/// (`story:outcome-when-reads-the-run-count`, D3 a): each action not `due` whose binding has a
/// `from: runs` entry and whose open evidence is not empty and holds only the kinds those entries
/// write. Without this the open kind would make the writer useful until its allowance is spent,
/// and it would run again with nothing to do. An action with no open evidence is left as it was:
/// the selector finds it useless already, and a wait on its approval is unchanged.
fn spent_writers_idle<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    bindings: &BindingsDocument,
    due: &BTreeSet<String>,
) -> Result<BTreeSet<String>, Refused> {
    let counted: BTreeMap<&str, BTreeSet<&str>> = bindings
        .actions
        .iter()
        .filter_map(|bound| {
            let kinds: BTreeSet<&str> = bound
                .evidence
                .iter()
                .filter(|entry| entry.from == ResultSource::Runs)
                .map(|entry| entry.kind.as_str())
                .collect();
            (!kinds.is_empty()).then_some((bound.action.as_str(), kinds))
        })
        .collect();
    if counted.is_empty() {
        return Ok(BTreeSet::new());
    }
    let facts = governor
        .action_facts(case)
        .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?;
    Ok(facts
        .actions
        .iter()
        .filter(|facts| !due.contains(&facts.action))
        .filter(|facts| {
            counted.get(facts.action.as_str()).is_some_and(|kinds| {
                !facts.open_evidence.is_empty()
                    && facts
                        .open_evidence
                        .iter()
                        .all(|kind| kinds.contains(kind.as_str()))
            })
        })
        .map(|facts| facts.action.clone())
        .collect())
}

/// The action the deterministic selector takes next in `case`, chosen as a step would choose it
/// from the frontier Commission's loop offers; nothing is written and nothing is started. The
/// placed artifacts are read as the case holds them: a file changed since the last step moves its
/// artifact only when the next step starts. Empty when the selector takes none.
fn next_action<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &Held,
    catalog: &ProtocolCatalog,
    workspace: &Path,
    store: &Path,
    runner: &Arc<dyn TestRunner>,
    cancel: &Cancel,
) -> Result<Vec<String>, Refused> {
    let id = CaseId(case.case.clone());
    let (_, _, effects, history) =
        step_inputs(governor, case, catalog, workspace, store, runner, cancel)?;
    let frontier = governor
        .frontier(&id)
        .map_err(|error| unreadable(format!("case {}", case.case), format!("{error:?}")))?;
    // What Commission's loop hands the executor: the actions the case's effect port performs, and
    // every action behind an authority gate.
    let mut offered = frontier.data().clone();
    let gated: BTreeSet<String> = offered
        .actions
        .iter()
        .filter(|entry| {
            entry.status == ActionStatus::ApprovalRequired
                || entry.capability.as_deref().is_some_and(|c| !c.is_empty())
        })
        .map(|entry| entry.action.clone())
        .collect();
    offered
        .actions
        .retain(|entry| effects.performs(&entry.action) || gated.contains(&entry.action));
    let frontier = Frontier::new(offered);
    let catalogue = project(
        &frontier,
        loom::run::CatalogueId(loom::primitives::Uuid(
            derived_uuid(&["catalogue", &case.case, "cancelled"]).0,
        )),
        loom::run::TurnId(loom::primitives::Uuid(
            derived_uuid(&["turn", &case.case, "cancelled"]).0,
        )),
    );
    let selector = DeterministicSelector::new(id, governor, &history);
    let context = SelectionContext {
        prompt: "Run the tree's case.".to_owned(),
    };
    Ok(selector
        .select(&context, &catalogue.data().entries)
        .map(|choice| vec![choice.action])
        .unwrap_or_default())
}

/// The run's end on its cancel: `case`'s `stopped` line, `stop` `Cancelled` and `next` the action
/// whose step was cancelled or was to be taken next, and the run `Cancelled`.
fn cancelled_stop(
    logs: &Logs,
    case: &str,
    next: Vec<String>,
    steps: i64,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<TreeRun, Refused> {
    let line = logs.append(
        case,
        RunLogEvent::Stopped(Stopped {
            stop: format!("{:?}", TreeStop::Cancelled),
            next,
            budget: None,
        }),
    )?;
    emit(case, line);
    Ok(TreeRun {
        stop: TreeStop::Cancelled,
        case: case.to_owned(),
        steps,
        decisions: Vec::new(),
        detail: Some("the run was cancelled".to_owned()),
    })
}

/// The ids of the records `case` holds, but the `case_outcome` records the governor writes for
/// its children, which no step of the case gives.
fn evidence_ids<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
) -> Result<BTreeSet<String>, Refused> {
    Ok(governor
        .evidence(case)
        .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?
        .into_iter()
        .filter(|record| record.kind != "case_outcome")
        .map(|record| record.evidence_id.0.0)
        .collect())
}

/// The `case_outcome` record `parent` holds now for its child `child`: its evidence id, the
/// link's `outcome_evidence`, and the SHA-256 of its facts, encoded as `decision_digest` is
/// (`serde_json::to_vec` of the plain facts). Both `None` when the parent holds no link to
/// `child` or no record under the link's id.
fn child_record<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    parent: &str,
    child: &str,
) -> Result<(Option<String>, Option<String>), Refused> {
    let state = governor
        .case_state(&CaseId(parent.to_owned()))
        .map_err(|error| unreadable(format!("case {parent}"), format!("{error:?}")))?;
    let Some(link) = state.children.iter().find(|link| link.child.0 == child) else {
        return Ok((None, None));
    };
    let id = &link.outcome_evidence.0;
    Ok(state
        .evidence
        .iter()
        .find(|held| held.data.evidence_id.0.0 == *id)
        .map_or((None, None), |held| {
            let bytes = serde_json::to_vec(&plain_commission(&held.data.facts)).unwrap_or_default();
            (Some(id.clone()), Some(hex(&Sha256::digest(&bytes))))
        }))
}

/// The exit, the refusal and the blob digest an effect's outcome names: an exit or a refusal,
/// exactly one, as an `action finished` line holds.
fn effect_summary(outcome: &EffectOutcome) -> (Option<i64>, Option<String>, Option<String>) {
    match outcome {
        EffectOutcome::Performed(performed) => {
            let report = plain_commission(&performed.report);
            let blob = report["blob_sha256"]
                .as_str()
                .or_else(|| report["run"]["blob_sha256"].as_str())
                .map(str::to_owned);
            let Some(run) = report.get("run") else {
                // An effect that starts no program (a person's, an opening, a copy) ended.
                return (Some(0), None, blob);
            };
            match run["exit"].as_i64() {
                Some(exit) => (Some(exit), None, blob),
                None => {
                    let refusal = run["runner_refusal"].as_str().map_or_else(
                        || {
                            if run["timed_out"].as_bool() == Some(true) {
                                "timed out".to_owned()
                            } else {
                                "ended with no exit".to_owned()
                            }
                        },
                        str::to_owned,
                    );
                    (None, Some(refusal), blob)
                }
            }
        }
        EffectOutcome::Refused(refused) => (None, Some(refused.reason.clone()), None),
        EffectOutcome::Pending(_) => (None, Some("pending".to_owned()), None),
    }
}

/// The actions of `case` a person or an approval would let go on: admissible or awaiting approval,
/// bound to a person or under a capability, and not taken at `revision` unless due again.
fn waiting_actions<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    bindings: &BindingsDocument,
    history: &LogHistory,
    revision: i64,
) -> Result<Vec<(String, Option<String>, Executor)>, Refused> {
    let frontier = governor
        .frontier(case)
        .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?;
    Ok(frontier
        .data()
        .actions
        .iter()
        .filter(|action| action.status != ActionStatus::Blocked)
        .filter(|action| {
            history.due(&action.action).unwrap_or(true)
                || !history
                    .performed_at(&action.action, revision)
                    .unwrap_or(false)
        })
        .filter_map(|action| {
            let bound = bindings
                .actions
                .iter()
                .find(|bound| bound.action == action.action)?;
            let waits = bound.executor.kind == Executor::Person || action.capability.is_some();
            waits.then(|| {
                (
                    action.action.clone(),
                    action.capability.clone(),
                    bound.executor.kind,
                )
            })
        })
        .collect())
}

/// The command a `Handoff` decision prints: `tree run` of `store` again, under the run's
/// `confinement` and on its `cgroup_root` when `settings` name them.
fn handoff_command(store: &Path, settings: &RunSettings) -> Vec<String> {
    let mut command = vec![
        "b10x-loom".to_owned(),
        "tree".to_owned(),
        "run".to_owned(),
        "--store".to_owned(),
        store.display().to_string(),
    ];
    command.extend(confinement_flags(settings));
    command
}

/// The run's `--confinement` and `--cgroup-root` as `settings` name them, which a printed command
/// that starts programs carries: a handoff's `tree run`, and the `tree approve` of a person action
/// whose binding names an `argv` (`story:person-action-runs-a-program`).
fn confinement_flags(settings: &RunSettings) -> Vec<String> {
    let mut flags = Vec::new();
    if let Some(confinement) = &settings.confinement {
        flags.extend(["--confinement".to_owned(), confinement.clone()]);
    }
    if let Some(root) = &settings.cgroup_root {
        flags.extend(["--cgroup-root".to_owned(), root.display().to_string()]);
    }
    flags
}

/// The `Handoff` decision of `action` of `case` when its binding is not a person's and names a
/// `handoff`: the text is what is decided, and `command` is the command.
fn handoff_decision(
    case: &CaseId,
    bindings: &BindingsDocument,
    action: &str,
    command: &[String],
) -> Option<NextDecision> {
    let bound = bindings
        .actions
        .iter()
        .find(|bound| bound.action == action && bound.executor.kind != Executor::Person)?;
    Some(NextDecision {
        case: case.0.clone(),
        kind: DecisionKind::Handoff,
        action: Some(action.to_owned()),
        capability: None,
        principal: None,
        decides: bound.handoff.clone()?,
        subject: None,
        revision: None,
        path: None,
        outcome: None,
        command: command.to_vec(),
    })
}

/// How a case in which no step was taken waits ([`waits`]), or that the run's cancel ended a
/// `handoff_when` check it ran, naming the action.
enum Waits {
    /// The stop and the decisions that let the case go on.
    Stop(TreeStop, Vec<NextDecision>),
    /// The run's cancel ended the check of this action: the run stops `Cancelled`.
    CheckCancelled(String),
}

/// One `Handoff` decision per action of `case` the frontier admits whose binding is not a person's
/// and names a `handoff` ([`handoff_decision`]), the command `command` ([`handoff_command`]). A
/// binding with `handoff_when` gives its decision only when its check, run now with its line
/// written, exits 0; a check that gives no exit status refuses the run `HandoffCheckFailed`, and
/// one the run's cancel ended answers [`Waits::CheckCancelled`].
fn handoffs<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    bindings: &BindingsDocument,
    command: &[String],
    checks: &HandoffChecks<'_, S>,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<Waits, Refused> {
    let frontier = governor
        .frontier(case)
        .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?;
    let mut decisions = Vec::new();
    for action in frontier
        .data()
        .actions
        .iter()
        .filter(|action| action.status != ActionStatus::Blocked)
    {
        let Some(decision) = handoff_decision(case, bindings, &action.action, command) else {
            continue;
        };
        if checks.has_check(&action.action) {
            let Some((check, line)) = checks.check(&action.action)? else {
                continue;
            };
            emit(&case.0, line);
            if check.cancelled {
                return Ok(Waits::CheckCancelled(action.action.clone()));
            }
            if check.refusal.is_some() {
                return Err(checks.failed(&action.action, &check));
            }
            if !check.hands_off() {
                continue;
            }
        }
        decisions.push(decision);
    }
    Ok(Waits::Stop(TreeStop::AwaitingPerson, decisions))
}

/// Why `case`, in which no step was taken, waits, and the decisions that let it go on, or the
/// action whose `handoff_when` check the run's cancel ended.
#[allow(clippy::too_many_arguments)]
fn waits<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &CaseId,
    bindings: &BindingsDocument,
    legitimate: &[String],
    store: &Path,
    workspace: &Path,
    revision: i64,
    history: &LogHistory,
    handoff: &[String],
    confinement: &[String],
    checks: &HandoffChecks<'_, S>,
    emit: &mut impl FnMut(&str, RunLogLine),
) -> Result<Waits, Refused> {
    let store_text = store.display().to_string();
    let mut decisions = Vec::new();
    let mut person = false;
    for (action, capability, kind) in waiting_actions(governor, case, bindings, history, revision)?
    {
        let bound = bindings.actions.iter().find(|bound| bound.action == action);
        let subject = bound
            .and_then(|bound| bound.evidence.first())
            .map(|entry| entry.subject.clone());
        let path = subject.as_ref().and_then(|subject| {
            bindings
                .artifacts
                .iter()
                .find(|placed| placed.artifact == *subject)
                .map(|placed| workspace.join(&placed.path).display().to_string())
        });
        let subject_revision = subject.as_ref().and_then(|subject| {
            governor
                .revisions(case)
                .ok()
                .and_then(|revisions| revisions.get(subject).cloned())
        });
        let principal = bound
            .and_then(|bound| bound.executor.principal.clone())
            .unwrap_or_else(|| DEFAULT_PRINCIPAL.to_owned());
        let mut command = vec![
            "b10x-loom".to_owned(),
            "tree".to_owned(),
            "approve".to_owned(),
            "--store".to_owned(),
            store_text.clone(),
            "--case".to_owned(),
            case.0.clone(),
        ];
        if let Some(capability) = &capability {
            command.extend(["--capability".to_owned(), capability.clone()]);
        }
        // One value for the command and the decision: the printed principal is the one recorded.
        command.extend(["--principal".to_owned(), principal.clone()]);
        let kind = if kind == Executor::Person {
            person = true;
            command.extend(["--action".to_owned(), action.clone()]);
            // A person action whose approval runs a program starts it as this run starts its own.
            if bound.is_some_and(|bound| bound.executor.argv.is_some()) {
                command.extend(confinement.iter().cloned());
            }
            DecisionKind::ApproveAction
        } else {
            DecisionKind::Approve
        };
        decisions.push(NextDecision {
            case: case.0.clone(),
            kind,
            action: Some(action.clone()),
            capability,
            principal: Some(principal),
            decides: format!("{action} of case {} at revision {revision}", case.0),
            subject,
            revision: subject_revision,
            path,
            outcome: None,
            command,
        });
    }
    if decisions.is_empty() && legitimate.len() > 1 {
        for outcome in legitimate {
            decisions.push(NextDecision {
                case: case.0.clone(),
                kind: DecisionKind::Close,
                action: None,
                capability: None,
                principal: None,
                decides: format!("close case {} through {outcome}", case.0),
                subject: None,
                revision: None,
                path: None,
                outcome: Some(outcome.clone()),
                command: vec![
                    "b10x-loom".to_owned(),
                    "tree".to_owned(),
                    "close".to_owned(),
                    "--store".to_owned(),
                    store_text.clone(),
                    "--case".to_owned(),
                    case.0.clone(),
                    "--outcome".to_owned(),
                    outcome.clone(),
                ],
            });
        }
        return Ok(Waits::Stop(TreeStop::AwaitingPerson, decisions));
    }
    if decisions.is_empty() {
        // Nothing is useful and nothing awaits approval: an admissible writer with a `handoff`
        // has done what it may, and a person continues from its text, then runs the tree again.
        match handoffs(governor, case, bindings, handoff, checks, emit)? {
            Waits::Stop(_, handed) if handed.is_empty() => {}
            waits => return Ok(waits),
        }
    }
    let stop = if decisions.is_empty() {
        TreeStop::Stuck
    } else if person {
        TreeStop::AwaitingPerson
    } else {
        TreeStop::AwaitingAuthority
    };
    Ok(Waits::Stop(stop, decisions))
}

/// The effect port of one case: its `program` bindings through [`ProgramEffects`], its `open`
/// bindings through [`OpenEffects`] and its `copy` bindings through [`CopyEffects`]. A `person`
/// action is never performed by a run.
pub(crate) struct TreeEffects<'a, S: loom_governor::FallibleCaseStore> {
    programs: ProgramEffects<'a, S>,
    opens: OpenEffects<'a, S>,
    copies: CopyEffects<'a, S>,
}

impl<'a, S: loom_governor::FallibleCaseStore> TreeEffects<'a, S> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        governor: &'a CanonGovernor<S>,
        case: &CaseId,
        bindings: &'a BindingsDocument,
        file: &Path,
        catalog: &ProtocolCatalog,
        workspace: &Path,
        store: &Path,
        runner: &Arc<dyn TestRunner>,
        cancel: &Cancel,
    ) -> Result<Self, Refused> {
        let refused = |error: String| {
            Refused::new(TreeRefusalReason::BindingsRefused, error)
                .case(case.0.clone())
                .file(file.display().to_string())
        };
        let opens = OpenEffects::new(
            governor,
            case.clone(),
            bindings,
            file,
            catalog,
            workspace,
            store,
            Some(Arc::clone(runner)),
        )
        .map(|opens| opens.with_cancel(cancel.clone()))
        .map_err(|error| match error {
            OpenPortError::Refused(refusal) => refused(refusal.to_string()),
            OpenPortError::Runner(error) => {
                Refused::new(TreeRefusalReason::ConfinementRefused, error.to_string())
                    .case(case.0.clone())
            }
        })?;
        let copies = CopyEffects::new(governor, case.clone(), bindings, catalog)
            .map_err(|error| refused(error.to_string()))?;
        let programs = ProgramEffects::new(
            governor,
            case.clone(),
            bindings,
            workspace,
            store,
            Some(Arc::clone(runner)),
        )
        .map(|programs| programs.with_cancel(cancel.clone()))
        .map_err(|error| {
            Refused::new(TreeRefusalReason::ConfinementRefused, error.to_string())
                .case(case.0.clone())
        })?;
        Ok(Self {
            programs,
            opens,
            copies,
        })
    }

    /// The `copy` actions of the case and whether a step of each would write a copy now.
    fn copies_due(&self, bindings: &BindingsDocument) -> Result<Vec<(String, bool)>, Refused> {
        bindings
            .actions
            .iter()
            .filter(|bound| bound.executor.kind == Executor::Copy)
            .map(|bound| {
                self.copies
                    .copy_due(&bound.action)
                    .map(|due| (bound.action.clone(), due))
                    .map_err(|error| {
                        unreadable(format!("action {}", bound.action), error.to_string())
                    })
            })
            .collect()
    }
}

impl<S: loom_governor::FallibleCaseStore> EffectPort for TreeEffects<'_, S> {
    fn performs(&self, action: &str) -> bool {
        self.programs.performs(action)
            || self.opens.performs(action)
            || self.copies.performs(action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.as_str();
        if self.opens.performs(action) {
            self.opens.invoke(commission, request)
        } else if self.copies.performs(action) {
            self.copies.invoke(commission, request)
        } else {
            self.programs.invoke(commission, request)
        }
    }
}

/// `value` as a `serde_json` value.
pub(crate) fn plain(value: &loom::json::Value) -> serde_json::Value {
    use loom::json::Value;
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Number(spelling) => {
            serde_json::from_str(spelling).unwrap_or(serde_json::Value::Null)
        }
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Array(items) => serde_json::Value::Array(items.iter().map(plain).collect()),
        Value::Object(members) => serde_json::Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// A Commission `value` as a `serde_json` value.
pub(crate) fn plain_commission(value: &CommissionJson) -> serde_json::Value {
    match value {
        CommissionJson::Null => serde_json::Value::Null,
        CommissionJson::Bool(value) => serde_json::Value::Bool(*value),
        CommissionJson::Number(spelling) => {
            serde_json::from_str(spelling).unwrap_or(serde_json::Value::Null)
        }
        CommissionJson::Text(text) => serde_json::Value::String(text.clone()),
        CommissionJson::Array(items) => {
            serde_json::Value::Array(items.iter().map(plain_commission).collect())
        }
        CommissionJson::Object(members) => serde_json::Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain_commission(member)))
                .collect(),
        ),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// One step of `case` on Commission's loop: `executor` proposes, the tree's authority over the
/// approvals the store holds for the case decides, and `effects` performs. The ids of the step are
/// derived from the case, its revision and `ordinal` (the lines its log holds), so two runs from
/// the same store give the same ids.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive<S, E, F>(
    governor: &CanonGovernor<S>,
    store: &Path,
    case: &CaseId,
    revision: i64,
    ordinal: i64,
    executor: &E,
    effects: &F,
    time: &TreeTime,
    wall: Option<(Instant, u64)>,
) -> Result<LoopEnd, Refused>
where
    S: loom_governor::FallibleCaseStore,
    E: AgentExecutor + ?Sized,
    F: EffectPort + ?Sized,
{
    let authority = TreeAuthority::new(BTreeMap::from([(
        case.0.clone(),
        read_approvals(store, &case.0)?,
    )]));
    let instant = time.now()?;
    let mut context = StepContext::new(
        &case.0,
        revision,
        ordinal,
        time.clone(),
        b10x_loom_commission::model::primitives::Timestamp(instant.0),
        wall,
    );
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(derived_uuid(&["commission", &case.0, &ordinal.to_string()])),
        agent_revision_id: AgentRevisionId(derived_uuid(&["agent", TREE_PRINCIPAL])),
        case_id: case.clone(),
        principal: PrincipalId(TREE_PRINCIPAL.to_owned()),
        authority_context: AuthorityContext(CommissionJson::Null),
    });
    let run_case = case.0.clone();
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(derived_uuid(&["run", &run_case, &ordinal.to_string()]))
    }));
    run_until_blocked(
        governor,
        executor,
        &authority,
        effects,
        &commission,
        &mut runs,
        &mut context,
    )
    .map_err(|error| unreadable(format!("the loop of case {}", case.0), format!("{error:?}")))
}

/// The executor of `tree approve --action`: it proposes its one action while the frontier lists it.
pub(crate) struct Propose(pub(crate) String);

impl AgentExecutor for Propose {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        if frontier
            .data()
            .actions
            .iter()
            .any(|listed| listed.action == self.0)
        {
            ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: self.0.clone(),
                arguments: ProposedActionArguments(CommissionJson::Object(Vec::new())),
            })
        } else {
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }
}

/// The bindings document `case` of the tree runs under.
///
/// # Errors
/// `CaseNotInTree`, or as the tree's bindings files are read.
pub(crate) fn case_bindings<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    manifest: &TreeManifest,
    catalog: &ProtocolCatalog,
    case: &str,
) -> Result<BindingsDocument, Refused> {
    tree_cases(governor, &manifest.root, &manifest.bindings, catalog)?
        .into_iter()
        .find(|held| held.case == case)
        .map(|held| held.bindings)
        .ok_or_else(|| {
            Refused::new(
                TreeRefusalReason::CaseNotInTree,
                format!("the tree rooted at {} holds no such case", manifest.root),
            )
            .case(crate::time::cut(case))
        })
}

/// One step of `case` in which `action` is proposed and `effects` performs it, as the line the
/// case's log gains.
///
/// # Errors
/// `NotAPersonAction` when the loop does not perform the action, naming why.
pub(crate) fn one_step<S, F>(
    governor: &CanonGovernor<S>,
    store: &Path,
    case: &CaseId,
    action: &str,
    effects: &F,
    time: &TreeTime,
) -> Result<ActionFinished, Refused>
where
    S: loom_governor::FallibleCaseStore,
    F: EffectPort + ?Sized,
{
    let revision = governor
        .current_revision(case)
        .map_err(|error| unreadable(format!("case {}", case.0), format!("{error:?}")))?;
    let ordinal = i64::try_from(read_lines(store, &case.0)?.len()).unwrap_or(i64::MAX);
    let before = evidence_ids(governor, case)?;
    let end = drive(
        governor,
        store,
        case,
        revision,
        ordinal,
        &Propose(action.to_owned()),
        effects,
        time,
        None,
    )?;
    match end.effects.last() {
        Some(outcome @ EffectOutcome::Performed(_)) => {
            let after = evidence_ids(governor, case)?;
            let (exit, refusal, blob_digest) = effect_summary(outcome);
            Ok(ActionFinished {
                action: action.to_owned(),
                exit,
                refusal,
                blob_digest,
                evidence: after.difference(&before).cloned().collect(),
                revisions: Vec::new(),
            })
        }
        Some(EffectOutcome::Refused(refused)) => Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            refused.reason.clone(),
        )
        .case(case.0.clone())
        .action(action.to_owned())),
        _ => Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            format!("the loop did not perform the action: {:?}", end.outcome),
        )
        .case(case.0.clone())
        .action(action.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
    use b10x_loom_intake_slice::confinement::{
        ConfinementAdapter, EngineConfinement, EngineRunner,
    };

    use super::{RunSettings, handoff_command};
    use crate::TreeTime;

    fn settings(confinement: Option<&str>, cgroup_root: Option<&str>) -> RunSettings {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        RunSettings {
            max_steps: None,
            max_wall_seconds: None,
            time: TreeTime::At("2026-11-22T08:00:00Z".into()),
            runner: Arc::new(runner),
            confinement: confinement.map(str::to_owned),
            cgroup_root: cgroup_root.map(PathBuf::from),
        }
    }

    /// A handoff's `tree run` carries the run's `--cgroup-root` when the run was named one, and
    /// no such flag when it was not, so that, run unchanged, it runs where this run did.
    #[test]
    fn a_handoff_command_carries_the_cgroup_root_the_run_had() {
        let store = Path::new("/srv/tree/store");
        assert_eq!(
            handoff_command(
                store,
                &settings(Some("substrate"), Some("/sys/fs/cgroup/held"))
            ),
            [
                "b10x-loom",
                "tree",
                "run",
                "--store",
                "/srv/tree/store",
                "--confinement",
                "substrate",
                "--cgroup-root",
                "/sys/fs/cgroup/held",
            ]
        );
        assert_eq!(
            handoff_command(store, &settings(Some("substrate"), None)),
            [
                "b10x-loom",
                "tree",
                "run",
                "--store",
                "/srv/tree/store",
                "--confinement",
                "substrate",
            ]
        );
        assert_eq!(
            handoff_command(store, &settings(None, None)),
            ["b10x-loom", "tree", "run", "--store", "/srv/tree/store"]
        );
    }
}
