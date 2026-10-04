//! The `AgentExecutor` port: what an executor proposes for a frontier.
//!
//! An executor is given the commission and the current frontier and returns one generated
//! [`ExecutorOutcome`]. No outcome completes the case: completion is the governor's determination,
//! never the executor's, and `CompletedLocalReasoning` only says the executor's own reasoning ended.

use crate::model::responsibility::{
    Commission, ExecutorOutcome, Frontier, commission_state, frontier_state,
};

/// Runs an agent over one frontier of its commission.
///
/// Loom, an external harness, a workflow, a human or a test fake implements this; Commission
/// depends on none of them (Atlas ADR 0075).
pub trait AgentExecutor {
    /// What the executor proposes for `frontier`, under `commission`.
    ///
    /// There is no error channel: an executor that fails returns
    /// `Suspended` with `SuspensionReason::ExternalAvailability`.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome;
}
