//! The `AgentExecutor` port: what an executor proposes for a frontier.
//!
//! An executor is given the commission and the current frontier and returns one generated
//! [`ExecutorOutcome`]. No outcome completes the case: completion is the governor's determination,
//! never the executor's, and `CompletedLocalReasoning` only says the executor's own reasoning ended.
//!
//! The runtime takes each call as an [`ExecutorStep`] ([`AgentExecutor::run_with_usage`],
//! `story:executor-reports-usage`): the outcome, and what the call reports it spent.

use crate::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorStep, Frontier, commission_state, frontier_state,
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

    /// One call as the runtime takes it: what the executor proposes for `frontier`, under
    /// `commission`, and what the call reports it spent ([`ExecutorStep`],
    /// `ess/commission/domains/responsibility.yaml`). The runtime calls this, never [`Self::run`].
    ///
    /// The default is [`Self::run`]'s outcome with no usage, which an executor whose model calls go
    /// through a metered model port of the run keeps: the port counts them, so nothing is counted
    /// twice. An executor that is not a model loop (an agent session, a remote executor) returns
    /// its call's usage here, and its `run` returns this call's outcome; an executor over another
    /// forwards it. The runtime charges the usage to the run and its budget as the module
    /// [`crate::runtime`] documents.
    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        ExecutorStep {
            outcome: self.run(commission, frontier),
            usage: None,
            transcript_digest: None,
            summary: None,
        }
    }
}
