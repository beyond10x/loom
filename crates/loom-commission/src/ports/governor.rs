//! The `Governor` port: the frontier and the completion determination for a case.
//!
//! The governor owns the case's truth. Commission asks it, per case and whichever commission is
//! asking, for the case's current revision, the frontier issued for that revision and whether the
//! case is complete. Only the governor determines completion, never the executor
//! (`docs/contracts/commission-executor.md`). Observation and evidence reach the governor through
//! their own ports (`ports::evidence`).

use crate::model::responsibility::{
    CaseId, CompletionDetermination, Frontier, GovernorError, frontier_state,
};

/// A governor, as Commission calls it. Every failure is a typed [`GovernorError`].
pub trait Governor {
    /// The case's current revision.
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError>;

    /// The frontier the governor issues for the case's current revision. It carries the case id
    /// and the revision it was issued for.
    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError>;

    /// Whether the governor holds the case complete, and with which outcome.
    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError>;
}
