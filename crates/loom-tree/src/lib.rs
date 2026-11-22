//! A tree of protocol cases run from a case store (`loom.tree`, `ess/domains/tree.yaml`).
//!
//! A store directory holds one tree: the root case opened by [`open_tree`] from a registered
//! protocol, a `loom.bindings/1` file and the initial artifact revisions, and every case opened
//! below it. Beside the governor's case files (`case-<hex>.json`, [`loom_governor::FileCaseStore`])
//! and their run logs (`runlog-<stem>.jsonl`, [`b10x_loom_runlog::RunLog`]) the store keeps:
//!
//! - [`MANIFEST`], `tree.json`: the tree ([`model::TreeManifest`]), written once by `tree open`:
//!   the root case, the workspace every case of the tree works in and the root's bindings file,
//!   both absolute, and where its protocols come from;
//! - `loghead-<stem>.json` per case: the `seq` and SHA-256 of the last line the tree appended to
//!   the case's run log ([`model::TreeLogHead`]), so [`verify_tree`] sees a changed byte in the
//!   last line too, which the chain of the log alone cannot;
//! - [`LOCK_FILE`], `tree.lock`, which one writer holds for the whole of a command
//!   ([`StoreLock`]), and [`HOLDER_FILE`], `tree.holder`, the holder's process id while it holds
//!   it. A second writer is refused `StoreInUse` naming that process id and writes nothing.
//!
//! [`tree_status`] and [`verify_tree`] read and never take the store: a live view reads the tree
//! while a run holds it.
//!
//! Every instant a command writes is the host clock's, or the one `--at` gives ([`TreeTime`]), so
//! two runs from the same inputs and the same `--at` write the same bytes.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod approvals;
mod approve;
mod close;
mod context;
mod encode;
mod handoff;
mod history;
mod lock;
mod log;
mod open;
mod person;
mod protocols;
mod read;
mod run;
mod status;
mod store;
mod time;
mod verify;

use std::fmt;

/// The runners a [`RunSettings`] starts bound programs through, and the reason a `local` runner
/// is admitted for, so that an embedder builds the settings from this crate alone.
pub use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
pub use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, EngineConfinement, EngineRunner, TestRunner,
};
/// The name of a case's run log in the store, and the reading of a log's bytes into its lines.
pub use b10x_loom_runlog::{log_file_name, verify as read_log};
/// The run-log lines a [`RunObserver`] is given.
pub use loom::runlog;
pub use loom::tree as model;

pub use approvals::{TreeAuthority, approvals_file_name, read_approvals};
pub use approve::{ApproveRequest, MAX_NAME_BYTES, Reasons, approve};
pub use close::{CaseClosed, CloseRequest, close};
pub use context::derived_uuid;
pub use encode::{decision_json, refusal_json, run_json, status_json, verify_json};
pub use history::LogHistory;
pub use lock::{HOLDER_FILE, LOCK_FILE, StoreLock};
pub use log::{head_file_name, verify_case_log};
pub use open::{OpenRequest, OpenedTree, open_tree};
pub use protocols::{
    KeptProtocol, PROTOCOLS_FILE, ProtocolDirectory, read_protocols_directory, stored_catalog,
};
pub use read::read_status;
pub use run::{RunObserver, RunSettings, TREE_PRINCIPAL, run_tree};
pub use status::{STATUS_FORMAT, tree_status};
pub use store::{FORMAT, MANIFEST, read_manifest};
pub use time::TreeTime;
pub use verify::verify_tree;

use model::{TreeRefusal, TreeRefusalReason};

/// The story that runs an `agent` binding; until it lands `tree open` refuses one
/// (`AgentNotRunYet`).
pub const AGENT_STORY: &str = "story:bound-agent-node";

/// A refused tree command: the generated `loom.tree.TreeRefusal`, boxed so that a `Result` of a
/// tree function stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub Box<TreeRefusal>);

impl Refused {
    /// A refusal of `reason` saying `detail`, naming nothing else.
    pub fn new(reason: TreeRefusalReason, detail: impl Into<String>) -> Self {
        Self(Box::new(TreeRefusal {
            reason,
            detail: detail.into(),
            case: None,
            file: None,
            action: None,
            story: None,
            holder: None,
        }))
    }

    /// This refusal naming `case`.
    #[must_use]
    pub fn case(mut self, case: impl Into<String>) -> Self {
        self.0.case = Some(case.into());
        self
    }

    /// This refusal naming `file`.
    #[must_use]
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.0.file = Some(file.into());
        self
    }

    /// This refusal naming `action`.
    #[must_use]
    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.0.action = Some(action.into());
        self
    }

    /// The reason.
    pub fn reason(&self) -> TreeRefusalReason {
        self.0.reason
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let refusal = &self.0;
        write!(out, "{:?}", refusal.reason)?;
        if let Some(case) = &refusal.case {
            write!(out, ": case {case:?}")?;
        }
        if let Some(file) = &refusal.file {
            write!(out, ": file {file:?}")?;
        }
        if let Some(action) = &refusal.action {
            write!(out, ": action {action:?}")?;
        }
        if let Some(story) = &refusal.story {
            write!(out, ": {story}")?;
        }
        if let Some(holder) = refusal.holder {
            write!(out, ": held by process {holder}")?;
        }
        write!(out, ": {}", refusal.detail)
    }
}

impl std::error::Error for Refused {}

/// `StoreUnreadable` saying `what` failed with `error`.
pub(crate) fn unreadable(what: impl fmt::Display, error: impl fmt::Display) -> Refused {
    Refused::new(
        TreeRefusalReason::StoreUnreadable,
        format!("{what}: {error}"),
    )
}
