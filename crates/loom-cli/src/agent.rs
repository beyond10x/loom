//! What `b10x-loom agent check` reads and prints: the spec from a regular file, checked by
//! `b10x_loom_agent::check_text` against the plugin catalog, and one line for either answer.
//!
//! A serveable spec is `checked <agent>: <n> states, <m> bindings, protocol <protocol>` on
//! standard output (`protocol none` without loops); a refused one is
//! `refused: <reason> at <member>: <detail>` on standard error, the reason in the enum's
//! spelling. Both lines are made printable, so a value of the document can start no new line.

use std::io::Read as _;
use std::path::Path;

use b10x_loom_agent::{Checked, SpecRefused, check_text, describe};
use b10x_loom_intake_slice::run::printable;
use loom_protocols::ProtocolCatalog;

use crate::regular_file;

/// The exit status of a refused spec, as [`crate::AGENT_CHECK_HELP`] states it.
pub const REFUSED: u8 = 3;

/// How many bytes of a spec are read: one past the decoder's bound, so a larger document is
/// refused by the decoder as `Malformed` rather than read whole.
const MAX_SPEC_BYTES: u64 = 2 * 1024 * 1024 + 1;

/// Why `agent check` gave no answer, or the refusal it gave.
#[derive(Debug)]
pub enum CheckFailure {
    /// The spec or the plugin catalog cannot be read (exit status 1), with the reason.
    Unreadable(String),
    /// The spec cannot be served (exit status [`REFUSED`]).
    Refused(SpecRefused),
}

/// Reads the spec at `path` and checks it against the plugin catalog.
///
/// # Errors
/// [`CheckFailure::Unreadable`] naming the file when it is no regular file, cannot be read or is
/// not UTF-8, or when the plugin catalog cannot be loaded; [`CheckFailure::Refused`] with the
/// first refusal.
pub fn check_spec(path: &Path) -> Result<Checked, CheckFailure> {
    let named = |why: &str| CheckFailure::Unreadable(format!("--spec {}: {why}", path.display()));
    let mut bytes = Vec::new();
    regular_file::open(path)
        .map_err(|why| named(&why))?
        .take(MAX_SPEC_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| named(&format!("cannot be read: {error}")))?;
    let text = String::from_utf8(bytes).map_err(|_| named("is not UTF-8"))?;
    let catalog = ProtocolCatalog::plugins()
        .map_err(|why| CheckFailure::Unreadable(format!("the plugin catalog: {why}")))?;
    let actions = |name: &str| catalog.get(name).map(|entry| entry.actions());
    check_text(&text, &actions).map_err(CheckFailure::Refused)
}

/// The line a serveable spec prints.
pub fn checked_line(checked: &Checked) -> String {
    printable(&format!(
        "checked {}: {} states, {} bindings, protocol {}",
        checked.agent,
        checked.states,
        checked.bindings,
        checked.protocol.as_deref().unwrap_or("none")
    ))
}

/// The line a refused spec prints on standard error.
pub fn refused_line(refusal: &SpecRefused) -> String {
    printable(&format!("refused: {}", describe(refusal)))
}
