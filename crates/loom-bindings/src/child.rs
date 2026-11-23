//! The child of every program the crate starts (`story:bound-program-case-id`): the bound program
//! and its `arguments_from` program alike are built by [`bound_case_child`], so each reads the id
//! of the case it runs for from [`CASE_VARIABLE`], the whole seconds of its timeout from
//! [`TIMEOUT_VARIABLE`] and, for a case a parent opened, the names the governor stored for that
//! opening from [`CASE_NAME_VARIABLE`] and [`PARENT_CASE_NAME_VARIABLE`]
//! (`story:bound-program-contract`), and nothing else steers it through its environment.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_intake_slice::bindings::bound_program_child;
use b10x_loom_intake_slice::confinement::{ChildCommand, ExecEnvironment};
use loom::bindings::ProgramEnvironment;

/// The variable that holds the id of the case a program runs for.
pub const CASE_VARIABLE: &str = "LOOM_CASE";
/// The most bytes of a case id [`CASE_VARIABLE`] holds; a longer one starts nothing. A stored case
/// name ([`CASE_NAME_VARIABLE`], [`PARENT_CASE_NAME_VARIABLE`]) has the same bound.
pub const MAX_CASE_VARIABLE_BYTES: usize = 4096;
/// The variable that holds the whole seconds of the run's timeout.
pub const TIMEOUT_VARIABLE: &str = "LOOM_TIMEOUT_SECONDS";
/// The variable that holds the name the case's opening gave it, for a case a parent opened.
pub const CASE_NAME_VARIABLE: &str = "LOOM_CASE_NAME";
/// The variable that holds the name the parent's opening gave the parent, for a case whose parent
/// was itself opened by a case.
pub const PARENT_CASE_NAME_VARIABLE: &str = "LOOM_PARENT_CASE_NAME";
/// The variable that holds the host process's `PATH`.
const PATH_VARIABLE: &str = "PATH";

/// The names the governor stored for the opening of the case a program runs for: `case_name`, the
/// name in the case's own `parent` link (absent for a root case), and `parent_case_name`, the name
/// in the `parent` link of that parent (absent unless the parent was itself opened by a case). The
/// port reads both from the governor; no request, argument, report, program output or bindings
/// document gives either.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaseNames {
    /// The name the case's opening gave it.
    pub case_name: Option<String>,
    /// The name the parent's opening gave the parent.
    pub parent_case_name: Option<String>,
}

/// The child a program the crate starts is started as: the child
/// [`b10x_loom_intake_slice::bindings::bound_program_child`] builds from `argv`, `arguments`,
/// `timeout`, `workspace` and `local_reason`, whose environment is then replaced by the one
/// [`ProgramEnvironment`] describes: `PATH` at the host process's value, as that child holds it,
/// then, set last, [`CASE_VARIABLE`] = `case.0`, [`TIMEOUT_VARIABLE`] = the whole seconds of
/// `timeout`, and [`CASE_NAME_VARIABLE`] and [`PARENT_CASE_NAME_VARIABLE`] when `names` holds
/// them, with no host variable inherited. `case` is the case the port was built for and `names`
/// what the governor stored for it, never a value of a request, a report, a program's output or
/// the bindings document.
///
/// # Errors
/// `case` is longer than [`MAX_CASE_VARIABLE_BYTES`], or a name of `names` is not one child name
/// (one or more ASCII letters, digits, `-` and `_`) of at most that many bytes, each checked
/// before anything is built; or the child cannot be built (the host's `PATH` is not UTF-8, `argv`
/// is empty). No error quotes a name.
pub fn bound_case_child(
    argv: &[String],
    arguments: &str,
    timeout: Duration,
    workspace: &Path,
    local_reason: Option<&str>,
    case: &CaseId,
    names: &CaseNames,
) -> Result<ChildCommand, String> {
    if case.0.len() > MAX_CASE_VARIABLE_BYTES {
        return Err(format!(
            "the case id is longer than {MAX_CASE_VARIABLE_BYTES} bytes"
        ));
    }
    if names
        .case_name
        .as_deref()
        .is_some_and(|name| !child_name(name))
    {
        return Err("the case name is not a child name".to_owned());
    }
    if names
        .parent_case_name
        .as_deref()
        .is_some_and(|name| !child_name(name))
    {
        return Err("the parent case name is not a child name".to_owned());
    }
    let child = bound_program_child(argv, arguments, timeout, workspace, local_reason)?;
    let described = ProgramEnvironment {
        path: child.environment().set.get(PATH_VARIABLE).cloned(),
        loom_case: case.0.clone(),
        // The bound program of a retried step gets it from `crate::retry::given`, never here.
        loom_retry_feedback: None,
        loom_timeout_seconds: i64::try_from(timeout.as_secs()).unwrap_or(i64::MAX),
        loom_case_name: names.case_name.clone(),
        loom_parent_case_name: names.parent_case_name.clone(),
    };
    Ok(child.with_environment(environment(&described)))
}

/// Whether `name` is one child name: one or more ASCII letters, digits, `-` and `_`, at most
/// [`MAX_CASE_VARIABLE_BYTES`] bytes.
pub(crate) fn child_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_CASE_VARIABLE_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// The variables `described` names, and nothing else: `PATH` when the host has one, the retry
/// feedback variable when it holds one, then [`CASE_VARIABLE`], [`TIMEOUT_VARIABLE`] and each
/// name it holds.
fn environment(described: &ProgramEnvironment) -> ExecEnvironment {
    let mut set = BTreeMap::new();
    if let Some(path) = &described.path {
        set.insert(PATH_VARIABLE.to_owned(), path.clone());
    }
    if let Some(feedback) = &described.loom_retry_feedback {
        set.insert(crate::RETRY_FEEDBACK_VARIABLE.to_owned(), feedback.clone());
    }
    set.insert(CASE_VARIABLE.to_owned(), described.loom_case.clone());
    set.insert(
        TIMEOUT_VARIABLE.to_owned(),
        described.loom_timeout_seconds.to_string(),
    );
    if let Some(name) = &described.loom_case_name {
        set.insert(CASE_NAME_VARIABLE.to_owned(), name.clone());
    }
    if let Some(name) = &described.loom_parent_case_name {
        set.insert(PARENT_CASE_NAME_VARIABLE.to_owned(), name.clone());
    }
    ExecEnvironment {
        allow: Vec::new(),
        set,
    }
}
