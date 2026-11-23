//! `tree hold` and `tree reopen` (`story:tree-suite-stop-resume-reopen-steps`): a principal's hold
//! on the whole tree, recorded in the root case's run log, and its lift.
//!
//! `tree hold` takes the store lock and appends one `stop recorded` line (`StopRecorded`: the
//! principal and the reason) to the root's log; `tree reopen` appends one `stop lifted` line
//! (`StopLifted`: the principal and `stop`, the `seq` of the line it lifts). The tree is held while
//! the root's log holds a `stop recorded` line with no `stop lifted` line after it
//! ([`current_hold`]); the log is the record and no other file holds it. `tree run` on a held tree
//! is refused `TreeStopped` once it holds the store and before it writes anything; every other
//! command works on a held tree as on any other. Both lines come from the command line alone.
//!
//! A hold records a decision; it is no lock against the store's owner. A log whose last lines were
//! cut away whole still verifies, so a person who can write the store can cut the hold's line
//! while it is the last one.

use std::path::Path;

use loom::primitives::Timestamp;
use loom::runlog::{RunLogEvent, RunLogLine, StopLifted, StopRecorded};
use loom::tree::TreeRefusalReason;

use crate::log::Logs;
use crate::run::read_lines;
use crate::store::{existing_store, read_manifest};
use crate::{MAX_NAME_BYTES, Refused, StoreLock, TreeTime};

/// The most bytes of a hold's reason.
pub const MAX_REASON_BYTES: usize = 1024;

/// What `tree hold` is given.
#[derive(Debug, Clone)]
pub struct HoldRequest<'a> {
    /// The store directory `tree open` made.
    pub store: &'a Path,
    /// The principal who holds the tree: not empty, at most [`MAX_NAME_BYTES`] bytes, no control
    /// character.
    pub principal: &'a str,
    /// Why: 1 to [`MAX_REASON_BYTES`] bytes, no control character.
    pub reason: &'a str,
    /// Where the command reads its instant.
    pub time: TreeTime,
}

/// What `tree reopen` is given.
#[derive(Debug, Clone)]
pub struct ReopenRequest<'a> {
    /// The store directory `tree open` made.
    pub store: &'a Path,
    /// The principal who lifts the hold, checked as [`HoldRequest::principal`] is.
    pub principal: &'a str,
    /// Where the command reads its instant.
    pub time: TreeTime,
}

/// A hold of a tree, as its `stop recorded` line in the root's log holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeHold {
    /// The root case, whose log holds the line.
    pub case: String,
    /// The line's `seq`.
    pub seq: i64,
    /// The line's instant.
    pub at: Timestamp,
    /// Who held the tree.
    pub principal: String,
    /// Why.
    pub reason: String,
}

/// A hold `tree reopen` lifted: the hold, and the `seq` and instant of the `stop lifted` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldLifted {
    /// The hold lifted.
    pub hold: TreeHold,
    /// The `stop lifted` line's `seq`.
    pub seq: i64,
    /// Its instant.
    pub at: Timestamp,
    /// Who lifted the hold.
    pub principal: String,
}

/// The hold `lines`, a root's run log, hold now: its last `stop recorded` line when no `stop
/// lifted` line comes after it; `None` when the tree is not held.
pub fn current_hold(case: &str, lines: &[RunLogLine]) -> Option<TreeHold> {
    let mut hold = None;
    for line in lines {
        match &line.event {
            RunLogEvent::StopRecorded(recorded) => {
                hold = Some(TreeHold {
                    case: case.to_owned(),
                    seq: line.seq,
                    at: line.at.clone(),
                    principal: recorded.principal.clone(),
                    reason: recorded.reason.clone(),
                });
            }
            RunLogEvent::StopLifted(_) => hold = None,
            _ => {}
        }
    }
    hold
}

/// `member` (`principal` or `reason`) as a hold takes it: 1 to `max` bytes with no control
/// character; else `StopUnusable` naming the member, what is wrong and the rule, never the text.
fn usable(member: &str, text: &str, max: usize) -> Result<(), Refused> {
    let problem = if text.is_empty() {
        "is empty".to_owned()
    } else if text.len() > max {
        format!("is {} bytes long", text.len())
    } else if text.chars().any(char::is_control) {
        "holds a control character".to_owned()
    } else {
        return Ok(());
    };
    Err(Refused::new(
        TreeRefusalReason::StopUnusable,
        format!(
            "the {member} {problem}: a {member} is 1 to {max} bytes of UTF-8 with no control character"
        ),
    ))
}

/// The root of the tree in `store` and its log's lines, read holding the store.
fn held_root(store: &Path) -> Result<(String, Vec<RunLogLine>), Refused> {
    let manifest = read_manifest(store)?;
    let lines = read_lines(store, &manifest.root)?;
    Ok((manifest.root, lines))
}

/// Holds the tree `request` names: appends one `stop recorded` line to its root's run log, under
/// the store lock, and returns the hold.
///
/// # Errors
/// `StopUnusable` naming the principal or the reason and its rule, before the store is opened;
/// `NotATree`; `StoreInUse` naming the holder while another writer holds the store;
/// `TreeAlreadyStopped` when the tree is held already; `LogRefused` when the root's log does not
/// verify; `InvalidInstant` or `StoreUnreadable`. Nothing is written on a refusal.
pub fn hold(request: &HoldRequest<'_>) -> Result<TreeHold, Refused> {
    usable("principal", request.principal, MAX_NAME_BYTES)?;
    usable("reason", request.reason, MAX_REASON_BYTES)?;
    let store = existing_store(request.store)?;
    read_manifest(&store)?;
    let _held = StoreLock::acquire(&store)?;
    let (root, lines) = held_root(&store)?;
    if let Some(hold) = current_hold(&root, &lines) {
        return Err(Refused::new(
            TreeRefusalReason::TreeAlreadyStopped,
            format!(
                "the tree is held already, since {} by {}: {}",
                hold.at.0, hold.principal, hold.reason
            ),
        )
        .case(root));
    }
    let logs = Logs::open(&store, request.time.clone())?;
    logs.reconcile(&root)?;
    let line = logs.append(
        &root,
        RunLogEvent::StopRecorded(StopRecorded {
            principal: request.principal.to_owned(),
            reason: request.reason.to_owned(),
        }),
    )?;
    Ok(TreeHold {
        case: root,
        seq: line.seq,
        at: line.at,
        principal: request.principal.to_owned(),
        reason: request.reason.to_owned(),
    })
}

/// Lifts the hold of the tree `request` names: appends one `stop lifted` line naming the `stop
/// recorded` line it lifts to its root's run log, under the store lock.
///
/// # Errors
/// `StopUnusable` naming the principal and its rule, before the store is opened; `NotATree`;
/// `StoreInUse` naming the holder; `TreeNotStopped` when the tree is not held; `LogRefused`;
/// `InvalidInstant` or `StoreUnreadable`. Nothing is written on a refusal.
pub fn reopen(request: &ReopenRequest<'_>) -> Result<HoldLifted, Refused> {
    usable("principal", request.principal, MAX_NAME_BYTES)?;
    let store = existing_store(request.store)?;
    read_manifest(&store)?;
    let _held = StoreLock::acquire(&store)?;
    let (root, lines) = held_root(&store)?;
    let Some(hold) = current_hold(&root, &lines) else {
        return Err(Refused::new(
            TreeRefusalReason::TreeNotStopped,
            "the tree is not held: its root's run log holds no stop recorded line that no stop lifted line follows",
        )
        .case(root));
    };
    let logs = Logs::open(&store, request.time.clone())?;
    logs.reconcile(&root)?;
    let line = logs.append(
        &root,
        RunLogEvent::StopLifted(StopLifted {
            principal: request.principal.to_owned(),
            stop: hold.seq,
        }),
    )?;
    Ok(HoldLifted {
        hold,
        seq: line.seq,
        at: line.at,
        principal: request.principal.to_owned(),
    })
}

/// `TreeStopped` for a `tree run` of the held tree in `store`: the hold's instant, principal and
/// reason, and the `tree reopen` command that lifts it.
pub(crate) fn stopped(store: &Path, hold: &TreeHold) -> Refused {
    Refused::new(
        TreeRefusalReason::TreeStopped,
        format!(
            "the tree is held since {} by {}: {}; nothing ran. Lift the hold with: b10x-loom tree reopen --store {} --principal {}",
            hold.at.0,
            hold.principal,
            hold.reason,
            store.display(),
            hold.principal
        ),
    )
    .case(hold.case.clone())
}

/// Refuses a run of the tree in `store`, whose root is `root`, while it is held.
pub(crate) fn refuse_held(store: &Path, root: &str) -> Result<(), Refused> {
    match current_hold(root, &read_lines(store, root)?) {
        Some(hold) => Err(stopped(store, &hold)),
        None => Ok(()),
    }
}
