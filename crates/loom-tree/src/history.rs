//! What a case of the tree already did, read from its run log: the deterministic selector's
//! [`StepHistory`].
//!
//! An action was performed at a case revision when the log holds an `action finished` line for it
//! after an `evaluated` line of that revision and before the next `evaluated` line, and at the
//! revision its step left the case at, so that a writer is not taken again on its own write. Whether an
//! action is due again whatever its history (a retry, a pending copy) is the host's answer, given
//! as a set of actions; so is whether an action is idle whatever its history (a copy whose source
//! holds nothing new), which counts as performed at every revision.

use std::collections::{BTreeMap, BTreeSet};

use b10x_loom_executor::selection::StepHistory;
use loom::runlog::{RunLogEvent, RunLogLine};

/// The history of one case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogHistory {
    performed: BTreeSet<(String, i64)>,
    finished: BTreeMap<String, usize>,
    due: BTreeSet<String>,
    idle: BTreeSet<String>,
}

impl LogHistory {
    /// The history the lines of a case's log give, the case being at `current` now, with the
    /// actions `due` again. An action counts as performed at the revision it was taken at and at
    /// the one its step left: that of the next `evaluated` line, else `current`.
    pub fn new(lines: &[RunLogLine], current: i64, due: BTreeSet<String>) -> Self {
        let mut performed = BTreeSet::new();
        let mut count = BTreeMap::new();
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
                    *count.entry(finished.action.clone()).or_insert(0) += 1;
                    left.push(finished.action.clone());
                }
                _ => {}
            }
        }
        for action in left {
            performed.insert((action, current));
        }
        Self {
            performed,
            finished: count,
            due,
            idle: BTreeSet::new(),
        }
    }

    /// The same history with `idle` actions, never useful now.
    #[must_use]
    pub fn with_idle(mut self, idle: BTreeSet<String>) -> Self {
        self.idle = idle;
        self
    }

    /// The number of `action finished` lines for `action` the history read, at any revision.
    pub fn performed(&self, action: &str) -> usize {
        self.finished.get(action).copied().unwrap_or(0)
    }
}

impl StepHistory for LogHistory {
    fn performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String> {
        Ok(self.idle.contains(action)
            || self.performed.contains(&(action.to_owned(), case_revision)))
    }

    fn due(&self, action: &str) -> Result<bool, String> {
        Ok(self.due.contains(action))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use b10x_loom_executor::selection::StepHistory as _;
    use loom::primitives::Timestamp;
    use loom::runlog::{ActionFinished, Evaluated, RunLogEvent, RunLogLine};

    use super::LogHistory;

    /// A log of `evaluated` at revision 3, then `action finished` for `a` refused `refusal`.
    fn log(refusal: &str) -> Vec<RunLogLine> {
        let at = Timestamp("2026-11-22T08:00:00Z".into());
        vec![
            RunLogLine {
                seq: 0,
                prev: None,
                at: at.clone(),
                event: RunLogEvent::Evaluated(Evaluated {
                    case_revision: 3,
                    frontier: "f".into(),
                    decision_digest: String::new(),
                }),
            },
            RunLogLine {
                seq: 1,
                prev: None,
                at,
                event: RunLogEvent::ActionFinished(ActionFinished {
                    action: "a".into(),
                    exit: None,
                    refusal: Some(refusal.into()),
                    blob_digest: None,
                    evidence: Vec::new(),
                    revisions: Vec::new(),
                }),
            },
        ]
    }

    #[test]
    fn history_knows_a_cancelled_line() {
        let cancelled = LogHistory::new(
            &log("Cancelled: action a: the step was cancelled"),
            3,
            BTreeSet::new(),
        );
        assert_eq!(cancelled.performed_at("a", 3), Ok(false));
        assert_eq!(cancelled.performed("a"), 1);
        let spent = LogHistory::new(
            &log("RunsSpent: action a: a has run 1 times"),
            3,
            BTreeSet::new(),
        );
        assert_eq!(spent.performed_at("a", 3), Ok(true));
    }
}
