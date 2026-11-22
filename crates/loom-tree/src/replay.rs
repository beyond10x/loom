//! `tree replay`: every case of the tree decided again from what the store holds, as of its last
//! `evaluated` line, and compared with what that line recorded (`story:tree-replay-command`).
//!
//! A case is rebuilt as it was at its last `evaluated` line by undoing, newest first, every line
//! of its run log after it ([`rebuild_case`]): `tree run` writes that line at the start of the step
//! of the case it picks, and only when the case revision or the decision changed, so the state as
//! of that line is also the state at every later step start until the next change. The undo of
//! each line kind is exact:
//!
//! - `action finished`: each record its `evidence` names is taken out, and each move of its
//!   `revisions` is moved back, the artifact from `to` to `from` and the case revision one down;
//! - `child opened` of a child: the link to the child and the child's `case_outcome` record (the
//!   link's `outcome_evidence`) are taken out;
//! - `child closed` of a child: nothing of its own; the undo of the child's `child opened` line,
//!   which `tree run` always writes after the parent's last `evaluated` line when it writes the
//!   `child closed` line after it, takes out the record in whatever form the termination left it;
//! - `terminated`: the termination is taken out;
//! - every other kind writes nothing of the case and is undone by nothing.
//!
//! A line whose undo does not apply (a record or a link the case does not hold, a move whose `to`
//! is not the artifact's revision, a `child closed` with no `child opened` of that child after the
//! `evaluated` line) is the case's `unmatched_line`, and the undo stops there. The rebuilt case is
//! decided through the governor path `tree run` decides with ([`crate::run`]'s `decided`), over a
//! memory store holding it, and its digest compared with the logged one.
//!
//! The store is read and never held, as `tree status` reads it: nothing is written and no lock is
//! taken, so a replay reads the tree while a writer may write, and a torn read shows as a
//! difference, never as a write.

use std::collections::BTreeSet;
use std::path::Path;

use b10x_loom_commission::model::responsibility::CaseId;
use loom::runlog::{Evaluated, RunLogEvent, RunLogLine};
use loom::tree::{CaseReplay, TreeRefusalReason, TreeReplay};
use loom_governor::{CanonGovernor, CaseState, CaseStore as _, FileCaseStore, MemoryCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::run::{decided, read_lines};
use crate::status::members;
use crate::store::{existing_store, read_manifest};
use crate::{Refused, unreadable};

/// A case as it was at its last `evaluated` line, rebuilt from the case as held by undoing every
/// line of its run log after that one ([`rebuild_case`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuiltCase {
    /// The case as rebuilt; as far as the undo went when a line was unmatched.
    pub state: CaseState,
    /// The `seq` of the last `evaluated` line.
    pub seq: i64,
    /// The `at` of that line.
    pub at: String,
    /// What that line recorded.
    pub logged: Evaluated,
    /// The `seq` of the first line, newest first, whose undo does not apply; `None` when every
    /// line's does.
    pub unmatched_line: Option<i64>,
}

/// `held`, the case as the store holds it, rebuilt as it was at the last `evaluated` line of
/// `lines`, its run log: every line after that one undone, newest first, as the module
/// documentation says. `None` when the log holds no `evaluated` line. Reads nothing and writes
/// nothing.
pub fn rebuild_case(held: &CaseState, lines: &[RunLogLine]) -> Option<RebuiltCase> {
    let at = lines
        .iter()
        .rposition(|line| matches!(line.event, RunLogEvent::Evaluated(_)))?;
    let RunLogEvent::Evaluated(logged) = &lines[at].event else {
        return None;
    };
    let after = &lines[at + 1..];
    let mut state = held.clone();
    let mut unmatched_line = None;
    for (index, line) in after.iter().enumerate().rev() {
        let mut undone = state.clone();
        if undo(&mut undone, &line.event, &after[..index]) {
            state = undone;
        } else {
            unmatched_line = Some(line.seq);
            break;
        }
    }
    Some(RebuiltCase {
        state,
        seq: lines[at].seq,
        at: lines[at].at.0.clone(),
        logged: logged.clone(),
        unmatched_line,
    })
}

/// Undoes `event` on `state`; `earlier` are the lines between the `evaluated` line and this one.
/// Whether the undo applies; `state` is left half undone when it does not, and is not used then.
fn undo(state: &mut CaseState, event: &RunLogEvent, earlier: &[RunLogLine]) -> bool {
    match event {
        RunLogEvent::ActionFinished(finished) => {
            for id in &finished.evidence {
                let Some(index) = state
                    .evidence
                    .iter()
                    .position(|held| held.data.evidence_id.0.0 == *id)
                else {
                    return false;
                };
                state.evidence.remove(index);
            }
            for moved in finished.revisions.iter().rev() {
                let Some(from) = &moved.from else {
                    return false;
                };
                match state.artifacts.get_mut(&moved.artifact) {
                    Some(held) if *held == moved.to => from.clone_into(held),
                    _ => return false,
                }
                state.revision -= 1;
            }
            true
        }
        RunLogEvent::ChildOpened(opened) => {
            let Some(link) = state
                .children
                .iter()
                .position(|link| link.child.0 == opened.child)
            else {
                return false;
            };
            let record = state.children.remove(link).outcome_evidence.0;
            let Some(index) = state
                .evidence
                .iter()
                .position(|held| held.data.evidence_id.0.0 == record)
            else {
                return false;
            };
            state.evidence.remove(index);
            true
        }
        RunLogEvent::ChildClosed(closed) => earlier.iter().any(|line| {
            matches!(&line.event, RunLogEvent::ChildOpened(opened) if opened.child == closed.child)
        }),
        RunLogEvent::Terminated(_) => {
            state.termination = None;
            true
        }
        RunLogEvent::ActionStarted(_)
        | RunLogEvent::AuthorityRecorded(_)
        | RunLogEvent::CaseOpened(_)
        | RunLogEvent::Discarded(_)
        | RunLogEvent::Evaluated(_)
        | RunLogEvent::HandoffChecked(_)
        | RunLogEvent::Stopped(_)
        | RunLogEvent::StopLifted(_)
        | RunLogEvent::StopRecorded(_) => true,
    }
}

/// Every case of the tree in `store` decided again, read with `catalog`, in tree order; with
/// `case`, that case and the cases below it alone. Per case whether its log holds an `evaluated`
/// line and, when it does, the case rebuilt as of that line ([`rebuild_case`]) and decided again,
/// its revision and its decision's digest each compared with the line's. The store is read, never
/// held, and nothing is written.
///
/// # Errors
/// `NotATree` when the store holds no tree; `CaseNotInTree` when `case` is not a case of it;
/// `LogRefused` when a run log of a replayed case does not verify; `StoreUnreadable` when the store
/// cannot be read or a case cannot be decided.
pub fn replay_tree(
    store: &Path,
    catalog: &ProtocolCatalog,
    case: Option<&str>,
) -> Result<TreeReplay, Refused> {
    let store = existing_store(store)?;
    let manifest = read_manifest(&store)?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let members = members(&governor, &manifest.root)?;
    let mut selected: BTreeSet<String> = BTreeSet::new();
    for member in &members {
        let below = member
            .parent
            .as_ref()
            .is_some_and(|parent| selected.contains(parent));
        if case.is_none_or(|case| member.case == case) || below {
            selected.insert(member.case.clone());
        }
    }
    if let Some(case) = case
        && !selected.contains(case)
    {
        return Err(Refused::new(
            TreeRefusalReason::CaseNotInTree,
            format!("the tree rooted at {} holds no such case", manifest.root),
        )
        .case(crate::time::cut(case)));
    }
    let rebuilt_store = MemoryCaseStore::default();
    let mut rebuilt = Vec::new();
    for member in members
        .iter()
        .filter(|member| selected.contains(&member.case))
    {
        let id = CaseId(member.case.clone());
        let held = governor
            .case_state(&id)
            .map_err(|error| unreadable(format!("case {}", member.case), format!("{error:?}")))?;
        let lines = read_lines(&store, &member.case)?;
        let case_rebuilt = rebuild_case(&held, &lines);
        if let Some(case_rebuilt) = &case_rebuilt {
            rebuilt_store.insert(case_rebuilt.state.clone());
        }
        rebuilt.push((member.case.clone(), case_rebuilt));
    }
    let deciding = CanonGovernor::new(rebuilt_store)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let mut replayed = Vec::new();
    for (case, case_rebuilt) in rebuilt {
        let Some(case_rebuilt) = case_rebuilt else {
            replayed.push(CaseReplay {
                case,
                evaluated: false,
                seq: None,
                at: None,
                case_revision: None,
                logged_case_revision: None,
                unmatched_line: None,
                state_matches_log: None,
                decision_digest: None,
                logged_decision_digest: None,
                decision_matches_log: None,
            });
            continue;
        };
        let decision = decided(&deciding, &case)?;
        let logged = &case_rebuilt.logged;
        replayed.push(CaseReplay {
            evaluated: true,
            seq: Some(case_rebuilt.seq),
            at: Some(case_rebuilt.at.clone()),
            case_revision: Some(decision.case_revision),
            logged_case_revision: Some(logged.case_revision),
            unmatched_line: case_rebuilt.unmatched_line,
            state_matches_log: Some(
                decision.case_revision == logged.case_revision
                    && case_rebuilt.unmatched_line.is_none(),
            ),
            decision_matches_log: Some(decision.decision_digest == logged.decision_digest),
            decision_digest: Some(decision.decision_digest),
            logged_decision_digest: Some(logged.decision_digest.clone()),
            case,
        });
    }
    Ok(TreeReplay { cases: replayed })
}

/// The first comparison of `replayed` that failed: the case, and `state` or `decision`, with the
/// case's unmatched line when it has one; `None` when every compared entry matches. A case with no
/// `evaluated` line is not compared.
pub fn replay_difference(replayed: &TreeReplay) -> Option<(&str, &'static str, Option<i64>)> {
    replayed.cases.iter().find_map(|case| {
        if case.state_matches_log == Some(false) {
            Some((case.case.as_str(), "state", case.unmatched_line))
        } else if case.decision_matches_log == Some(false) {
            Some((case.case.as_str(), "decision", None))
        } else {
            None
        }
    })
}
