//! The child of every program the crate starts (`story:bound-program-case-id`): the bound program
//! and its `arguments_from` program alike are built by [`bound_case_child`], so each reads the id
//! of the case it runs for from [`CASE_VARIABLE`] and nothing else steers it through its
//! environment.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_intake_slice::bindings::bound_program_child;
use b10x_loom_intake_slice::confinement::{ChildCommand, ExecEnvironment};
use loom::bindings::ProgramEnvironment;

/// The variable that holds the id of the case a program runs for.
pub const CASE_VARIABLE: &str = "LOOM_CASE";
/// The most bytes of a case id [`CASE_VARIABLE`] holds; a longer one starts nothing.
pub const MAX_CASE_VARIABLE_BYTES: usize = 4096;
/// The variable that holds the host process's `PATH`.
const PATH_VARIABLE: &str = "PATH";

/// The child a program the crate starts is started as: the child
/// [`b10x_loom_intake_slice::bindings::bound_program_child`] builds from `argv`, `arguments`,
/// `timeout`, `workspace` and `local_reason`, whose environment is then replaced by the one
/// [`ProgramEnvironment`] describes: `PATH` at the host process's value, as that child holds it,
/// and [`CASE_VARIABLE`] = `case.0`, set last, with no host variable inherited. `case` is the case
/// the port was built for, never a value of a request, a report, a program's output or the
/// bindings document.
///
/// # Errors
/// `case` is longer than [`MAX_CASE_VARIABLE_BYTES`], checked before anything is built; or the
/// child cannot be built (the host's `PATH` is not UTF-8, `argv` is empty).
pub fn bound_case_child(
    argv: &[String],
    arguments: &str,
    timeout: Duration,
    workspace: &Path,
    local_reason: Option<&str>,
    case: &CaseId,
) -> Result<ChildCommand, String> {
    if case.0.len() > MAX_CASE_VARIABLE_BYTES {
        return Err(format!(
            "the case id is longer than {MAX_CASE_VARIABLE_BYTES} bytes"
        ));
    }
    let child = bound_program_child(argv, arguments, timeout, workspace, local_reason)?;
    let described = ProgramEnvironment {
        path: child.environment().set.get(PATH_VARIABLE).cloned(),
        loom_case: case.0.clone(),
        loom_retry_feedback: None,
    };
    Ok(child.with_environment(environment(&described)))
}

/// The variables `described` names, and nothing else: `PATH` when the host has one, the retry
/// feedback variable when the port gives one (`crate::retry`), then [`CASE_VARIABLE`].
pub(crate) fn environment(described: &ProgramEnvironment) -> ExecEnvironment {
    let mut set = BTreeMap::new();
    if let Some(path) = &described.path {
        set.insert(PATH_VARIABLE.to_owned(), path.clone());
    }
    if let Some(feedback) = &described.loom_retry_feedback {
        set.insert(crate::RETRY_FEEDBACK_VARIABLE.to_owned(), feedback.clone());
    }
    set.insert(CASE_VARIABLE.to_owned(), described.loom_case.clone());
    ExecEnvironment {
        allow: Vec::new(),
        set,
    }
}
