//! What a case of the tree already did, read from its run log: the deterministic selector's
//! [`StepHistory`].
//!
//! An action was performed at the revision of its step and at the revision its step left, as the
//! artifact moves its `action finished` lines name date them from the last `evaluated` line
//! (`story:action-dated-by-revisions-its-step-left`), so that a writer is not taken again on its
//! own write and an action whose inputs a later step moved is useful again. A log whose lines name
//! no move (one an older Loom wrote) is dated by its `evaluated` lines, as before. Whether an
//! action is due again whatever its history (a retry, a pending copy) is the host's answer, given
//! as a set of actions; so is whether an action is idle whatever its history (a copy whose source
//! holds nothing new), which counts as performed at every revision.
//!
//! A step its cancel ended ([`cancelled`]) was performed at no revision
//! (`story:cancelled-step-may-run-again`): the next run at the same revision takes it again. Its
//! line still counts in [`LogHistory::performed`], and its run still counts against
//! `budget.max_runs`.

use std::collections::{BTreeMap, BTreeSet};

use b10x_loom_executor::selection::StepHistory;
use loom::runlog::{ActionFinished, RunLogEvent, RunLogLine};

/// The prefix of the refusal of a step its cancel ended, as `b10x_loom_bindings` writes it:
/// `Cancelled: action <action>: <detail>`.
const CANCELLED_PREFIX: &str = "Cancelled: ";

/// Whether `finished` is the line of a step its cancel ended: refused `Cancelled`.
pub(crate) fn cancelled(finished: &ActionFinished) -> bool {
    finished
        .refusal
        .as_deref()
        .is_some_and(|refusal| refusal.starts_with(CANCELLED_PREFIX))
}

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
    /// actions `due` again.
    ///
    /// An action counts as performed at the revision of its step and at the revision its step
    /// left: the revision of the last `evaluated` line raised by one per move each `action
    /// finished` line since names, its own included. A step its cancel ended counts at neither,
    /// and its moves still raise the revision. When the moves an interval's lines name do not
    /// account for the revision of its closing `evaluated` line (else `current`), as in a log an
    /// older Loom wrote without moves, each line of it that names no move also counts at that
    /// revision; a line before the first `evaluated` line counts at that revision alone.
    pub fn new(lines: &[RunLogLine], current: i64, due: BTreeSet<String>) -> Self {
        let mut performed = BTreeSet::new();
        let mut count = BTreeMap::new();
        let mut interval = Interval::default();
        for line in lines {
            match &line.event {
                RunLogEvent::Evaluated(evaluated) => {
                    let closed = std::mem::replace(
                        &mut interval,
                        Interval::opened_at(evaluated.case_revision),
                    );
                    closed.close(evaluated.case_revision, &mut performed);
                }
                RunLogEvent::ActionFinished(finished) => {
                    *count.entry(finished.action.clone()).or_insert(0) += 1;
                    interval.step(finished);
                }
                _ => {}
            }
        }
        interval.close(current, &mut performed);
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

/// The `action finished` lines of a log between one `evaluated` line (none before the first) and
/// the next, or the end of the log.
#[derive(Debug, Default)]
struct Interval {
    /// The case revision of the `evaluated` line that opened it; `None` before the first.
    opened: Option<i64>,
    /// The moves its lines named so far, a cancelled step's included.
    moves: i64,
    /// Each step its cancel did not end, in log order.
    steps: Vec<Step>,
}

/// One step of an [`Interval`] its cancel did not end.
#[derive(Debug)]
struct Step {
    action: String,
    /// The revision of its step and the revision it left, when the interval has an opening one.
    dated: Option<(i64, i64)>,
    /// Whether its line names a move.
    moved: bool,
}

impl Interval {
    fn opened_at(revision: i64) -> Self {
        Self {
            opened: Some(revision),
            ..Self::default()
        }
    }

    /// Takes in the line of one step.
    fn step(&mut self, finished: &ActionFinished) {
        let taken = self.opened.map(|opened| opened.saturating_add(self.moves));
        let moves = i64::try_from(finished.revisions.len()).unwrap_or(i64::MAX);
        self.moves = self.moves.saturating_add(moves);
        if cancelled(finished) {
            return;
        }
        self.steps.push(Step {
            action: finished.action.clone(),
            dated: taken.map(|taken| (taken, taken.saturating_add(moves))),
            moved: moves > 0,
        });
    }

    /// Counts each step as performed, the interval closing at `revision`.
    fn close(self, revision: i64, performed: &mut BTreeSet<(String, i64)>) {
        let accounted = self
            .opened
            .is_some_and(|opened| opened.saturating_add(self.moves) == revision);
        for step in self.steps {
            if let Some((taken, left)) = step.dated {
                performed.insert((step.action.clone(), taken));
                performed.insert((step.action.clone(), left));
            }
            if !accounted && (!step.moved || step.dated.is_none()) {
                performed.insert((step.action, revision));
            }
        }
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

    use b10x_loom_commission::model::responsibility::CaseId;
    use b10x_loom_executor::model::run::{ActionFacts, CatalogueEntry, CatalogueEntryStatus};
    use b10x_loom_executor::selection::{
        ActionFactsPort, CaseFacts, DeterministicSelector, SelectionContext, StepHistory as _,
    };
    use b10x_loom_executor::{ActionSelector as _, SelectorError};
    use loom::primitives::Timestamp;
    use loom::runlog::{ActionFinished, Evaluated, RevisionMove, RunLogEvent, RunLogLine};

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

    /// An `evaluated` line at `revision`.
    fn evaluated(revision: i64) -> RunLogEvent {
        RunLogEvent::Evaluated(Evaluated {
            case_revision: revision,
            frontier: "f".into(),
            decision_digest: format!("d{revision}"),
        })
    }

    /// An `action finished` line for `action`, refused `refusal` or exit 0, naming `moves` moves.
    fn finished(action: &str, refusal: Option<&str>, moves: usize) -> RunLogEvent {
        RunLogEvent::ActionFinished(ActionFinished {
            action: action.into(),
            exit: refusal.is_none().then_some(0),
            refusal: refusal.map(str::to_owned),
            blob_digest: None,
            evidence: Vec::new(),
            revisions: (0..moves)
                .map(|n| RevisionMove {
                    artifact: format!("{action}-artifact-{n}"),
                    from: Some("before".into()),
                    to: "after".into(),
                })
                .collect(),
        })
    }

    /// The history of a log of `events`, the case at `current` now.
    fn history(events: Vec<RunLogEvent>, current: i64) -> LogHistory {
        let at = Timestamp("2026-11-22T08:00:00Z".into());
        let lines: Vec<RunLogLine> = events
            .into_iter()
            .enumerate()
            .map(|(seq, event)| RunLogLine {
                seq: i64::try_from(seq).unwrap(),
                prev: None,
                at: at.clone(),
                event,
            })
            .collect();
        LogHistory::new(&lines, current, BTreeSet::new())
    }

    /// The host's facts of `CASE` at `revision`: each `(action, effect, may produce, open)`.
    struct Facts(i64, Vec<ActionFacts>);

    impl ActionFactsPort for Facts {
        fn action_facts(&self, _case: &CaseId) -> Result<CaseFacts, String> {
            Ok(CaseFacts {
                case_revision: self.0,
                actions: self.1.clone(),
            })
        }
    }

    fn facts(action: &str, effect: &str, may_produce: &[&str], open: &[&str]) -> ActionFacts {
        ActionFacts {
            action: action.into(),
            effect: Some(effect.into()),
            may_produce: may_produce.iter().map(|kind| (*kind).to_owned()).collect(),
            open_evidence: open.iter().map(|kind| (*kind).to_owned()).collect(),
        }
    }

    /// What the deterministic selector takes from `actions` (all admissible) at `revision` with
    /// `history`.
    fn select(
        history: &LogHistory,
        revision: i64,
        actions: Vec<ActionFacts>,
    ) -> Result<String, SelectorError> {
        let candidates: Vec<CatalogueEntry> = actions
            .iter()
            .map(|known| CatalogueEntry {
                action: known.action.clone(),
                status: CatalogueEntryStatus::Admissible,
            })
            .collect();
        let selector =
            DeterministicSelector::new(CaseId("root".into()), Facts(revision, actions), history);
        selector
            .select(
                &SelectionContext {
                    prompt: "Take the next useful step.".into(),
                },
                &candidates,
            )
            .map(|choice| choice.action)
    }

    #[test]
    fn an_action_is_useful_again_when_a_later_step_moved_an_artifact() {
        // `parity-loom-strong-4` run 2, written with moves: A moved nothing, B moved two.
        let shape = history(
            vec![
                evaluated(9),
                finished("a", None, 0),
                finished("b", None, 2),
                evaluated(11),
            ],
            11,
        );
        assert_eq!(shape.performed_at("a", 9), Ok(true));
        assert_eq!(shape.performed_at("a", 11), Ok(false));
        assert_eq!(shape.performed_at("b", 11), Ok(true));
        assert_eq!(
            select(
                &shape,
                11,
                vec![
                    facts("a", "write", &["record"], &["record"]),
                    facts("b", "write", &["made"], &[]),
                ],
            ),
            Ok("a".to_owned())
        );

        // A moved one artifact (9 to 10), B one (10 to 11), no `evaluated` line between.
        let variant = history(
            vec![
                evaluated(9),
                finished("a", None, 1),
                finished("b", None, 1),
                evaluated(11),
            ],
            11,
        );
        assert_eq!(variant.performed_at("a", 9), Ok(true));
        assert_eq!(variant.performed_at("a", 10), Ok(true));
        assert_eq!(variant.performed_at("a", 11), Ok(false));
        assert_eq!(variant.performed_at("b", 10), Ok(true));
        assert_eq!(variant.performed_at("b", 11), Ok(true));
    }

    /// Runbook's case: `amend.upstream` at 9 leaves 9 or 10, `requirements.normalise` writes and
    /// leaves 11, one `evaluated` line at 11, then a check exits non-zero and opens the amend's
    /// evidence again. The selector offers `amend.upstream` again: the run does not stop `Stuck`.
    #[test]
    fn the_amend_step_is_offered_again_after_a_later_step_wrote() {
        for amend_moves in [0, 1] {
            let shape = history(
                vec![
                    evaluated(9),
                    finished("amend.upstream", None, amend_moves),
                    finished("requirements.normalise", None, 2 - amend_moves),
                    evaluated(11),
                    finished("requirements.check", Some("ProgramError: exit 1"), 0),
                    evaluated(11),
                ],
                11,
            );
            assert_eq!(
                select(
                    &shape,
                    11,
                    vec![
                        facts("amend.upstream", "write", &["amended"], &["amended"]),
                        facts("requirements.check", "read", &["checked"], &[]),
                        facts("requirements.normalise", "write", &["normalised"], &[]),
                    ],
                ),
                Ok("amend.upstream".to_owned()),
                "amend.upstream left {} moves",
                amend_moves
            );
        }
    }

    #[test]
    fn a_writer_is_not_taken_again_on_its_own_write() {
        let closed = history(
            vec![evaluated(9), finished("a", None, 1), evaluated(10)],
            10,
        );
        assert_eq!(closed.performed_at("a", 9), Ok(true));
        assert_eq!(closed.performed_at("a", 10), Ok(true));
        let open = history(vec![evaluated(9), finished("a", None, 1)], 10);
        assert_eq!(open.performed_at("a", 9), Ok(true));
        assert_eq!(open.performed_at("a", 10), Ok(true));
    }

    #[test]
    fn an_older_log_with_no_moves_is_read_as_before() {
        for older in [
            history(
                vec![
                    evaluated(9),
                    finished("a", None, 0),
                    finished("b", None, 0),
                    evaluated(10),
                ],
                10,
            ),
            history(
                vec![evaluated(9), finished("a", None, 0), finished("b", None, 0)],
                10,
            ),
        ] {
            for action in ["a", "b"] {
                for revision in [9, 10] {
                    assert_eq!(
                        older.performed_at(action, revision),
                        Ok(true),
                        "{action} at {revision}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_cancelled_step_counts_at_no_revision_and_its_move_still_counts() {
        let log = history(
            vec![
                evaluated(9),
                finished("a", Some("Cancelled: action a: the step was cancelled"), 1),
                finished("b", None, 0),
                evaluated(10),
            ],
            10,
        );
        assert_eq!(log.performed_at("a", 9), Ok(false));
        assert_eq!(log.performed_at("a", 10), Ok(false));
        assert_eq!(log.performed_at("b", 10), Ok(true));
        assert_eq!(log.performed_at("b", 9), Ok(false));
    }

    #[test]
    fn a_refused_step_counts_at_both_revisions() {
        let refused = "ProgramError: action a: exit 1";
        let alone = history(
            vec![evaluated(9), finished("a", Some(refused), 1), evaluated(10)],
            10,
        );
        assert_eq!(alone.performed_at("a", 9), Ok(true));
        assert_eq!(alone.performed_at("a", 10), Ok(true));
        let followed = history(
            vec![
                evaluated(9),
                finished("a", Some(refused), 1),
                finished("b", None, 1),
                evaluated(11),
            ],
            11,
        );
        assert_eq!(followed.performed_at("a", 9), Ok(true));
        assert_eq!(followed.performed_at("a", 10), Ok(true));
        assert_eq!(followed.performed_at("a", 11), Ok(false));
    }
}
