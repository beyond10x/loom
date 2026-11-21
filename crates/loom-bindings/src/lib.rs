//! `loom.bindings/1`: the document in which an operator declares, per action of one registered
//! protocol, which executor performs it and how its result becomes evidence.
//!
//! [`decode`] reads the text (YAML; JSON is YAML) into the generated `loom.bindings` types and
//! refuses what no protocol could make right: an undeclared member, a missing or other format, an
//! `argv` that is empty or not a list of strings, a result table that is not one. [`check`] holds
//! the decoded document against the compiled protocol it names, as it is loaded. Both refuse the
//! first defect found, naming the action and the member. [`unbound`] lists the actions the
//! document binds nothing to; that is not an error.
//!
//! The crate executes nothing, starts no process and opens no network connection.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod decode;
mod load;

pub use decode::decode;
pub use load::{check, unbound};

use loom::bindings::BindingsRefusal;
use std::fmt;

/// The `format` member every document holds.
pub const FORMAT: &str = "loom.bindings/1";
/// The model an `agent` executor holds when the document names none.
pub const DEFAULT_AGENT_MODEL: &str = "claude-haiku-5-5";
/// The runs an action gets when the document names no `budget.max_runs`.
pub const DEFAULT_MAX_RUNS: i64 = 8;
/// The shortest `timeout_seconds` an action may name.
pub const MIN_TIMEOUT_SECONDS: i64 = 1;
/// The longest `timeout_seconds` an action may name.
pub const MAX_TIMEOUT_SECONDS: i64 = 3600;
/// The most bytes of a document [`decode`] reads.
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// A refused document: the generated `loom.bindings.BindingsRefusal`, which names the reason, the
/// action when the defect is in one, the member and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal(pub BindingsRefusal);

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let refusal = &self.0;
        write!(out, "{:?}: ", refusal.reason)?;
        if let Some(action) = &refusal.action {
            write!(out, "action {action}, ")?;
        }
        write!(out, "member {}: {}", refusal.member, refusal.detail)
    }
}

impl std::error::Error for Refusal {}

/// A refusal of `reason` at `member` of `action` (the document when `None`).
pub(crate) fn refuse(
    reason: loom::bindings::BindingsRefusalReason,
    action: Option<&str>,
    member: impl Into<String>,
    detail: impl Into<String>,
) -> Refusal {
    Refusal(BindingsRefusal {
        reason,
        action: action.map(str::to_owned),
        member: member.into(),
        detail: detail.into(),
    })
}

/// Whether `path` stays in the workspace: relative, non-empty, with no `..` segment.
pub(crate) fn workspace_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("is empty");
    }
    if path.contains('\0') {
        return Err("holds a NUL byte");
    }
    let mut chars = path.chars();
    let first = chars.next();
    if matches!(first, Some('/' | '\\')) {
        return Err("is absolute");
    }
    if first.is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':') {
        return Err("is absolute");
    }
    if path.split(['/', '\\']).any(|segment| segment == "..") {
        return Err("leaves the workspace");
    }
    Ok(())
}
