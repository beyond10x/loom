//! The `handoff_when` check of a program binding (story:handoff-when-a-program-decides): an argv
//! whose exit decides whether the binding's `handoff` is offered.
//!
//! [`run_handoff_check`] runs it once, as a child [`bound_case_child`] builds, through the runner
//! every bound program of the run starts through, admitted as theirs is, with the binding's
//! timeout. Its reach is never more than the writer's: the whole workspace read-only, `target/`
//! included (`ChildCommand::with_read_only_workspace`); `PATH` and the `LOOM_*` variables of
//! the case alone, never the retry feedback; no aperture, no secret slot, no read-only root but
//! the program file a runner that is not `local` mounts at its own path; empty standard input.
//! Its standard output followed by its standard error is kept as `blobs/<sha256>` in the store
//! directory. It records nothing in the case and is not counted as a run of the action.
//!
//! An exit status other than 126, 127 and 129 to 192 is the check's answer: 0 offers the handoff
//! and any other skips it. A launch error (the child cannot be built, the runner refuses it, the
//! program cannot be found or executed, which the launcher reports as 126 or 127), a run killed
//! at its timeout, a run a signal ended (with no exit status, as `local` reports it, or with 128
//! plus the signal, as Substrate does: 137 for SIGKILL, an out-of-memory or cgroup kill
//! included) and output that cannot be kept are never a skip: each is a refusal, which stops the
//! run. A check that exits 129 to 192 itself is refused as well.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_intake_slice::bindings::{DEFAULT_TIMEOUT_SECONDS, admit_bound_program_runner};
use b10x_loom_intake_slice::confinement::TestRunner;
use loom::bindings::{BindingsDocument, BoundAction, Executor, ProgramRoot};
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore};

use crate::arguments::case_names;
use crate::bound_case_child;
use crate::program::{keep_blob, sha256};
use crate::resolve::beside;

/// The exit statuses a launcher gives a program it cannot execute (126) or find (127): Substrate
/// starts a program through `/usr/bin/env`, which answers so. Neither is a check's answer.
pub const LAUNCH_FAILURE_EXITS: [i64; 2] = [126, 127];

/// The exit statuses Substrate gives a child a signal ended, 128 plus the signal (1 to 64), as a
/// shell does: SIGKILL, which an out-of-memory or cgroup kill sends, is 137. None is a check's
/// answer.
pub const SIGNAL_EXITS: std::ops::RangeInclusive<i64> = 129..=192;

/// What one run of a `handoff_when` check answered: its exit status or why it gave none, exactly
/// one, and the SHA-256 of the blob holding its output when it was kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffCheck {
    /// The exit status, when the check ended with one that is its answer.
    pub exit: Option<i64>,
    /// Why the check gave no answer: it was not started, was killed at its timeout or by a
    /// signal, or its output could not be kept.
    pub refusal: Option<String>,
    /// The SHA-256 of the blob holding its standard output followed by its standard error.
    pub blob_digest: Option<String>,
}

impl HandoffCheck {
    /// Whether the check offers the handoff: it exited 0.
    pub fn hands_off(&self) -> bool {
        self.refusal.is_none() && self.exit == Some(0)
    }

    fn refused(refusal: String, blob_digest: Option<String>) -> Self {
        Self {
            exit: None,
            refusal: Some(refusal),
            blob_digest,
        }
    }
}

/// The binding of `action` in `document` that carries a `handoff_when` check: a `program`
/// binding with a non-empty `handoff_when`.
pub fn handoff_check_of<'a>(
    document: &'a BindingsDocument,
    action: &str,
) -> Option<&'a BoundAction> {
    document.actions.iter().find(|bound| {
        bound.action == action
            && bound.executor.kind == Executor::Program
            && !bound.handoff_when.is_empty()
    })
}

/// Runs the `handoff_when` check of `binding`, a binding of `document`, once for `case`, held by
/// `governor`, in `workspace` through `runner`, keeping its output under `store` (see the module
/// documentation). `None` when the binding carries no check.
pub fn run_handoff_check<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
    document: &BindingsDocument,
    binding: &BoundAction,
    workspace: &Path,
    store: &Path,
    runner: &Arc<dyn TestRunner>,
) -> Option<HandoffCheck> {
    let argv = binding.handoff_when.as_slice();
    let program = argv.first()?;
    let not_started = |why: &str| {
        Some(HandoffCheck::refused(
            format!("the check {program} was not started: {why}"),
            None,
        ))
    };
    let runner = match admit_bound_program_runner(Some(runner)) {
        Ok(runner) => runner,
        Err(refusal) => return not_started(&refusal.to_string()),
    };
    if document.program_root == Some(ProgramRoot::Bindings) && beside(program) {
        return not_started(
            "handoff_when[0] names a file beside the bindings file, and the document was not resolved with resolve_programs",
        );
    }
    let seconds = binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    let timeout = Duration::from_secs(u64::try_from(seconds).unwrap_or(1));
    let names = match case_names(governor, case) {
        Ok(Ok(names)) => names,
        Ok(Err(why)) => return not_started(&why),
        Err(error) => return not_started(&error.to_string()),
    };
    let child = match bound_case_child(
        argv,
        "",
        timeout,
        workspace,
        runner.local_reason(),
        case,
        &names,
    ) {
        Ok(child) => child.with_read_only_workspace(),
        Err(why) => return not_started(&why),
    };
    let execution = match runner.run_child(&child) {
        Ok(execution) => execution,
        Err(refusal) => {
            return Some(HandoffCheck::refused(
                format!("the check {program} was refused by its runner: {refusal}"),
                None,
            ));
        }
    };
    let output = &execution.child_output;
    let mut blob = output.stdout.clone();
    blob.extend_from_slice(&output.stderr);
    let digest = sha256(&blob);
    if let Err(error) = keep_blob(store, &digest, &blob) {
        return Some(HandoffCheck::refused(
            format!(
                "the output of the check {program} could not be kept as blob {digest}: {error}"
            ),
            None,
        ));
    }
    let kept = Some(digest);
    let exit = execution.exit_code.map(i64::from);
    Some(if execution.timed_out {
        HandoffCheck::refused(
            format!("the check {program} was killed at its timeout of {seconds} s"),
            kept,
        )
    } else if let Some(code) = exit.filter(|code| LAUNCH_FAILURE_EXITS.contains(code)) {
        HandoffCheck::refused(
            format!(
                "the check {program} could not be started: exit {code}, the status a launcher gives a program it cannot find or execute"
            ),
            kept,
        )
    } else if let Some(code) = exit.filter(|code| SIGNAL_EXITS.contains(code)) {
        HandoffCheck::refused(
            format!(
                "the check {program} was ended by a signal: exit {code}, the status Substrate gives a child signal {} ended",
                code - 128
            ),
            kept,
        )
    } else if let Some(code) = exit {
        HandoffCheck {
            exit: Some(code),
            refusal: None,
            blob_digest: kept,
        }
    } else {
        HandoffCheck::refused(
            format!("the check {program} ended with no exit status: a signal ended it"),
            kept,
        )
    })
}
