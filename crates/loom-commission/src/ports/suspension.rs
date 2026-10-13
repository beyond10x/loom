//! The suspension port: where a commission's Suspension is kept, so a run suspended on an approval
//! outlives the process that suspended it (`story:approval-suspend-resume-slice`,
//! `decision-blocker:suspension-durable-record`, option B).
//!
//! Commission defines the port; the host implements it over durable storage, as it implements the
//! governor's case store. A commission holds at most one [`Suspension`]: the run it suspended, the
//! action request pending the approval and the reason. The record lives and dies with the
//! commission, never with the case: it is not kept in the governor's record of the case, nor by the
//! authority provider.
//!
//! The port is fallible. A failure is a [`SuspensionStoreError`], never an empty answer: a store
//! that cannot read is not one that holds nothing. Each write is atomic: a failed write leaves what
//! was stored before.
//!
//! The runtime writes through it only when it is given one
//! ([`crate::runtime::run_with_suspension`]) and reads and consumes the record when it resumes
//! ([`crate::runtime::resume_after_grant`]). A caller of [`crate::runtime::run_until_blocked`]
//! passes no port, and its runs are never suspended on an approval.

use std::fmt;

use crate::model::responsibility::{CommissionId, Suspension, suspension_state};

/// Keeps the Suspension of each commission.
pub trait SuspensionPort {
    /// Stores `suspension` as its commission's one Suspension, replacing any held before.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when it cannot be written; what was stored before is kept.
    fn put(
        &self,
        suspension: &Suspension<suspension_state::Pending>,
    ) -> Result<(), SuspensionStoreError>;

    /// The Suspension of `commission`, or `None` when none is stored.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when the store cannot be read, or what it holds is not a
    /// Suspension of `commission`.
    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<Suspension<suspension_state::Pending>>, SuspensionStoreError>;

    /// Removes the Suspension of `commission`; `false` when none was stored.
    ///
    /// # Errors
    ///
    /// [`SuspensionStoreError`] when it cannot be removed; it is then still stored.
    fn remove(&self, commission: &CommissionId) -> Result<bool, SuspensionStoreError>;
}

/// Why a suspension store could not read or write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionStoreError {
    /// What the store reported.
    pub message: String,
}

impl SuspensionStoreError {
    /// A failure carrying the store's own message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SuspensionStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "suspension store failed: {}", self.message)
    }
}

impl std::error::Error for SuspensionStoreError {}
