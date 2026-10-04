//! A scripted fake `AgentExecutor`: it returns the outcomes its script names, in order, and logs
//! each call.

use b10x_commission::model::responsibility::{
    Commission, CommissionId, ExecutorOutcome, Frontier, FrontierId, commission_state,
    frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

/// One call to [`ScriptedExecutor`]'s [`AgentExecutor::run`]: which commission, which frontier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorCall {
    /// The commission the executor ran under.
    pub commission_id: CommissionId,
    /// The frontier it was given.
    pub frontier_id: FrontierId,
}

/// An executor that returns the next outcome of its script on each call to
/// [`AgentExecutor::run`], whatever the commission and frontier, and records each call.
///
/// A call after the script is used up panics: a test that runs the executor more often than it
/// scripted is wrong, and saying so beats inventing an outcome.
#[derive(Debug)]
pub struct ScriptedExecutor {
    script: Mutex<VecDeque<ExecutorOutcome>>,
    calls: Mutex<Vec<ExecutorCall>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ScriptedExecutor {
    /// An executor that returns `script`'s outcomes in order.
    pub fn new(script: impl IntoIterator<Item = ExecutorOutcome>) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Every call so far, in the order it was made.
    pub fn calls(&self) -> Vec<ExecutorCall> {
        lock(&self.calls).clone()
    }
}

impl AgentExecutor for ScriptedExecutor {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        lock(&self.calls).push(ExecutorCall {
            commission_id: commission.data().commission_id.clone(),
            frontier_id: frontier.data().frontier_id.clone(),
        });
        lock(&self.script)
            .pop_front()
            .unwrap_or_else(|| panic!("ScriptedExecutor: the script is used up"))
    }
}
