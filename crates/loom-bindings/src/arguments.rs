//! The one run of an `arguments_from` program, shared by the program effect port and the open
//! effect port (story:open-children-from-arguments-from), and the names a case's programs read.
//!
//! [`run_arguments_from`] builds the child with [`bound_case_child`], runs it once through the
//! admitted runner's `run_child` in the workspace with the request's arguments on standard input,
//! and answers the one JSON object it printed, its bytes and their SHA-256, or why not. Nothing it
//! answers quotes what the program printed. [`case_names`] reads the names the governor stored for
//! the opening of a case and of its parent.

use std::path::Path;
use std::time::Duration;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_commission::ports::effect::EffectError;
use b10x_loom_intake_slice::bindings::MAX_REPORT_BYTES;
use b10x_loom_intake_slice::confinement::{
    Cancel, ConfinementError, ConfinementRefusal, TestRunner,
};
use loom_governor::{CanonGovernor, EvaluationTime, FallibleCaseStore};

use crate::program::sha256;
use crate::{CaseNames, bound_case_child};

/// What an `arguments_from` program printed: one JSON object, the bytes and their SHA-256.
#[derive(Debug, Clone)]
pub(crate) struct Printed {
    pub(crate) object: serde_json::Value,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: String,
}

/// Why an `arguments_from` program gave no object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotPrinted {
    /// The child could not be built: nothing was started.
    NotStarted(String),
    /// It was refused by its runner, killed at its timeout, ended with an exit other than 0, or
    /// printed anything but one JSON object of at most [`MAX_REPORT_BYTES`].
    Unreadable(String),
    /// Its runner refused it [`ConfinementRefusal::Cancelled`]: its cancel was set before it
    /// started, or killed it while it ran.
    Cancelled(ConfinementError),
}

/// Runs `argv`, the `arguments_from` program of a binding, once for `case` (whose opening names
/// are `names`) through `runner`, in `workspace`, with `input` on standard input and `timeout`;
/// its printed object, or why not, naming `argv[0]` and never the text it printed. Given `cancel`,
/// the child holds it (`ChildCommand::with_cancel`).
pub(crate) fn run_arguments_from(
    argv: &[String],
    input: &str,
    timeout: Duration,
    workspace: &Path,
    runner: &dyn TestRunner,
    case: &CaseId,
    names: &CaseNames,
    cancel: Option<&Cancel>,
) -> Result<Printed, NotPrinted> {
    let program = argv
        .first()
        .map_or("the arguments_from program", String::as_str);
    let child = bound_case_child(
        argv,
        input,
        timeout,
        workspace,
        runner.local_reason(),
        case,
        names,
    )
    .map_err(|why| NotPrinted::NotStarted(format!("{program} was not started: {why}")))?;
    let child = match cancel {
        Some(cancel) => child.with_cancel(cancel.clone()),
        None => child,
    };
    let unreadable = |detail: String| Err(NotPrinted::Unreadable(detail));
    let execution = match runner.run_child(&child) {
        Ok(execution) => execution,
        Err(refusal) if refusal.reason == ConfinementRefusal::Cancelled => {
            return Err(NotPrinted::Cancelled(refusal));
        }
        Err(refusal) => {
            return unreadable(format!("{program} was refused by its runner: {refusal}"));
        }
    };
    if execution.timed_out {
        return unreadable(format!("{program} was killed at its timeout"));
    }
    if execution.exit_code != Some(0) {
        return unreadable(format!(
            "{program} ended exit status {}",
            execution
                .exit_code
                .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
        ));
    }
    let output = execution.child_output;
    match one_object(&output.stdout, output.stdout_truncated) {
        Ok(object) => Ok(Printed {
            object,
            sha256: sha256(&output.stdout),
            bytes: output.stdout,
        }),
        Err(why) => unreadable(format!("{program}'s output {why}")),
    }
}

/// `bytes` read as one JSON object of at most [`MAX_REPORT_BYTES`], nested at most 64 deep and
/// naming no member of an object twice; `truncated` says the runner kept only part of them. `Err`
/// says why not, as a predicate.
pub(crate) fn one_object(bytes: &[u8], truncated: bool) -> Result<serde_json::Value, String> {
    if truncated || bytes.len() > MAX_REPORT_BYTES {
        return Err(format!("is longer than {MAX_REPORT_BYTES} bytes"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "is not UTF-8".to_owned())?;
    // Commission's reader keeps every member as written and bounds the nesting, so a member named
    // twice, which a pointer could read either way, is found.
    let read =
        commission_json::parse(text).map_err(|error| format!("is not one JSON value: {error}"))?;
    if !matches!(read, commission_json::Value::Object(_)) {
        return Err("is not a JSON object".to_owned());
    }
    if let Some(name) = named_twice(&read) {
        return Err(format!("names the member {name:?} twice in one object"));
    }
    serde_json::from_str(text).map_err(|error| format!("is not one JSON value: {error}"))
}

/// The first member name an object of `value` holds twice.
fn named_twice(value: &commission_json::Value) -> Option<String> {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            commission_json::Value::Array(items) => pending.extend(items),
            commission_json::Value::Object(members) => {
                let mut seen = std::collections::BTreeSet::new();
                for (name, member) in members {
                    if !seen.insert(name.as_str()) {
                        return Some(name.clone());
                    }
                    pending.push(member);
                }
            }
            _ => {}
        }
    }
    None
}

/// The names the governor stored for the opening of `case` and of its parent: none for a root
/// case, the name of its link for a child, and the name of the parent's own link when the parent
/// was opened too. The inner error is a parent that cannot be read or does not list `case` among
/// its children; it names the two cases and quotes nothing the store holds.
pub(crate) fn case_names<S: FallibleCaseStore, T: EvaluationTime>(
    governor: &CanonGovernor<S, T>,
    case: &CaseId,
) -> Result<Result<CaseNames, String>, EffectError> {
    let state = governor
        .case_state(case)
        .map_err(|error| EffectError::new(format!("the governor: {error:?}")))?;
    let Some(link) = state.parent else {
        return Ok(Ok(CaseNames::default()));
    };
    let parent = CaseId(link.parent.0.clone());
    // The governor's error is not quoted: it may hold text read from the parent's file.
    let Ok(parent_state) = governor.case_state(&parent) else {
        return Ok(Err(format!(
            "case {} is a child of `{}`, which cannot be read",
            case.0, parent.0
        )));
    };
    if !parent_state
        .children
        .iter()
        .any(|child| child.child.0 == case.0)
    {
        return Ok(Err(format!(
            "case {} is a child of `{}`, which does not list it among its children",
            case.0, parent.0
        )));
    }
    Ok(Ok(CaseNames {
        case_name: Some(link.name),
        parent_case_name: parent_state.parent.map(|grand| grand.name),
    }))
}
