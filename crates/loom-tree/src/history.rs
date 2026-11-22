//! What a case of the tree already did, read from its run log: the deterministic selector's
//! [`StepHistory`].
//!
//! An action was performed at a case revision when the log holds an `action finished` line for it
//! after an `evaluated` line of that revision and before the next `evaluated` line, and at the
//! revision its step left the case at, so that a writer is not taken again on its own write. Whether an
//! action is due again whatever its history (a retry, a pending copy) is the host's answer, given
//! as a set of actions.

use std::collections::BTreeSet;

use b10x_loom_executor::selection::StepHistory;
use loom::runlog::{RunLogEvent, RunLogLine};

/// The history of one case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogHistory {
    performed: BTreeSet<(String, i64)>,
    due: BTreeSet<String>,
}

impl LogHistory {
    /// The history the lines of a case's log give, the case being at `current` now, with the
    /// actions `due` again. An action counts as performed at the revision it was taken at and at
    /// the one its step left: that of the next `evaluated` line, else `current`.
    pub fn new(lines: &[RunLogLine], current: i64, due: BTreeSet<String>) -> Self {
        let mut performed = BTreeSet::new();
        let mut revision = None;
        let mut left: Vec<String> = Vec::new();
        for line in lines {
            match &line.event {
                RunLogEvent::Evaluated(evaluated) => {
                    revision = Some(evaluated.case_revision);
                    for action in left.drain(..) {
                        performed.insert((action, evaluated.case_revision));
                    }
                }
                RunLogEvent::ActionFinished(finished) => {
                    if let Some(revision) = revision {
                        performed.insert((finished.action.clone(), revision));
                    }
                    left.push(finished.action.clone());
                }
                _ => {}
            }
        }
        for action in left {
            performed.insert((action, current));
        }
        Self { performed, due }
    }

    /// The number of `action finished` lines for `action` the history read, at any revision.
    pub fn performed(&self, action: &str) -> usize {
        self.performed
            .iter()
            .filter(|(performed, _)| performed == action)
            .count()
    }
}

impl StepHistory for LogHistory {
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String> {
        Ok(self.performed.contains(&(action.to_owned(), case_revision)))
    }

    fn due(&self, action: &str) -> Result<bool, String> {
        Ok(self.due.contains(action))
    }
}
