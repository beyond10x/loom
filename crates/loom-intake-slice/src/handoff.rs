//! The merge hand-off (`intake.routing.MergeHandoff`, `ess/intake/domains/routing.yaml`): what a
//! granted `repository.merge` performs when `b10x-loom resume` continues its run. It records the
//! commission, the granted request, the workspace and its `HEAD` in the state directory, and moves
//! no Git ref: merging happens outside Loom.
//!
//! [`MergeHandoffStore`] keeps one file per request, `merge-<hex of the action request id>.json`,
//! holding the generated [`MergeHandoffData`] as a JSON object with one member per declared field,
//! under the write discipline of [`crate::state_dir`]. It answers
//! `intake.routing.RecordMergeHandoff`: a request is recorded once, and a second record for it is
//! refused (`already-recorded`) and changes nothing.
//!
//! [`HandoffEffects`] is the effect port a resume runs on: `repository.merge`, for a
//! `software-change@1` case, is performed by recording its hand-off; every other action goes to
//! the slice's local effect adapter unchanged. The slice's run never builds one, so a run outside
//! a resume performs no merge at all.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, Commission, EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused,
    ExecutorOutcomeProposedAction, commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use intake_model::obligation::UnmetObligation;
use intake_model::primitives::Uuid;
use intake_model::routing::obligations::RecordMergeHandoffBehavior;
pub use intake_model::routing::{
    ActionRequestId, CommissionId, MergeHandedOff, MergeHandoffAlreadyRecorded, MergeHandoffData,
    MergeHandoffStorageFailed, RecordMergeHandoff, RecordMergeHandoffOutcome,
};
use serde_json::{Value, json};

use crate::effect::Console;
use crate::grant::{exactly, text_of};
use crate::resume::ResumeStoreError;
use crate::run::{json_text, printable};
use crate::selector::Briefing;
use crate::state_dir::{StateDir, failed, read_text};

/// The action a hand-off performs.
pub const MERGE: &str = "repository.merge";

/// The prefix of a hand-off's file name.
const PREFIX: &str = "merge-";

/// The members a hand-off has, in the order written.
const MEMBERS: [&str; 4] = ["action_request_id", "commission_id", "workspace", "head"];

/// The merge hand-offs recorded in one directory, one file per request.
#[derive(Debug)]
pub struct MergeHandoffStore {
    dir: StateDir,
}

impl MergeHandoffStore {
    /// A store over `dir`, created with its lock file when it does not exist yet, and proven to
    /// take a write, as [`crate::resume::ResumeContextStore::open`] opens it.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or its lock file cannot be created, or the directory
    /// does not take a write.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ResumeStoreError> {
        Ok(Self {
            dir: StateDir::open(dir.into(), "merge")?,
        })
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.dir()
    }

    /// Records `input`'s hand-off once (`intake.routing.RecordMergeHandoff`): `Recorded`, with the
    /// `MergeHandedOff` it publishes, when none was recorded for the request; `AlreadyRecorded`,
    /// changing nothing, when one was; `StorageFailed`, with the store's reason, when it cannot be
    /// written.
    pub fn record(&self, input: &RecordMergeHandoff) -> RecordMergeHandoffOutcome {
        let storage_failed = |error: ResumeStoreError| RecordMergeHandoffOutcome::StorageFailed {
            error: MergeHandoffStorageFailed {
                reason: error.to_string(),
            },
        };
        let handoff = MergeHandoffData {
            action_request_id: input.action_request_id.clone(),
            commission_id: input.commission_id.clone(),
            workspace: input.workspace.clone(),
            head: input.head.clone(),
        };
        let path = match self.path(&handoff.action_request_id) {
            Ok(path) => path,
            Err(error) => return storage_failed(error),
        };
        let text = encode(&handoff);
        if decode(&text, &handoff.action_request_id).as_ref() != Ok(&handoff) {
            return storage_failed(failed(&path, "the hand-off would not read back as written"));
        }
        let recorded = self.dir.locked(|| {
            if path.exists() {
                return Ok(false);
            }
            self.dir.replace(&path, &text).map(|()| true)
        });
        match recorded.and_then(|recorded| recorded) {
            Ok(true) => RecordMergeHandoffOutcome::Recorded {
                merge_handed_off: MergeHandedOff {
                    action_request_id: handoff.action_request_id,
                    commission_id: handoff.commission_id,
                    workspace: handoff.workspace,
                    head: handoff.head,
                },
            },
            Ok(false) => RecordMergeHandoffOutcome::AlreadyRecorded {
                error: MergeHandoffAlreadyRecorded {
                    action_request_id: handoff.action_request_id,
                },
            },
            Err(error) => storage_failed(error),
        }
    }

    /// The hand-off recorded for `request`, or `None` when none is.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the file cannot be read or is not a hand-off of `request`.
    pub fn get(
        &self,
        request: &ActionRequestId,
    ) -> Result<Option<MergeHandoffData>, ResumeStoreError> {
        let path = self.path(request)?;
        self.dir.locked(|| {
            let Some(text) = read_text(&path)? else {
                return Ok(None);
            };
            decode(&text, request)
                .map(Some)
                .map_err(|problem| failed(&path, problem))
        })?
    }

    fn path(&self, request: &ActionRequestId) -> Result<PathBuf, ResumeStoreError> {
        self.dir.file(PREFIX, "an action request id", &request.0.0)
    }
}

/// `intake.routing.RecordMergeHandoff`, the obligation the specification leaves to the store: the
/// store decides `storage-failed` by whether its file can be written.
impl RecordMergeHandoffBehavior for MergeHandoffStore {
    fn record_merge_handoff(
        &mut self,
        input: RecordMergeHandoff,
    ) -> Result<RecordMergeHandoffOutcome, UnmetObligation> {
        Ok(self.record(&input))
    }
}

/// The effect port a resume runs on (see the module documentation).
pub struct HandoffEffects<'a, 'o> {
    inner: &'a dyn EffectPort,
    store: &'a MergeHandoffStore,
    workspace: PathBuf,
    workspace_text: String,
    briefing: Briefing,
    console: &'a Console<'o>,
    local: bool,
}

impl<'a, 'o> HandoffEffects<'a, 'o> {
    /// Merges handed off into `store` for the workspace `workspace` (kept as `workspace_text`),
    /// every other action performed by `inner`; a merge only when `local`, the case's protocol
    /// being `software-change@1`.
    pub(crate) fn new(
        inner: &'a dyn EffectPort,
        store: &'a MergeHandoffStore,
        workspace: PathBuf,
        workspace_text: String,
        briefing: Briefing,
        console: &'a Console<'o>,
        local: bool,
    ) -> Self {
        Self {
            inner,
            store,
            workspace,
            workspace_text,
            briefing,
            console,
            local,
        }
    }

    /// Records the hand-off of `request`, printing the step, or refuses it.
    fn hand_off(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &ActionRequestData,
    ) -> Result<EffectOutcome, crate::run::SliceError> {
        let proposal = ExecutorOutcomeProposedAction {
            action: request.action.clone(),
            arguments: request.arguments.clone(),
        };
        let step = self.console.next_step();
        self.console.write(|out| {
            writeln!(
                out,
                "step {step}: {} {}",
                printable(&proposal.action),
                printable(&json_text(&proposal.arguments.0))
            )?;
            Ok(())
        })?;
        let head =
            crate::case::current_head(&self.workspace).map_err(crate::run::SliceError::Case)?;
        let input = RecordMergeHandoff {
            action_request_id: ActionRequestId(Uuid(request.action_request_id.0.0.clone())),
            commission_id: CommissionId(Uuid(commission.data().commission_id.0.0.clone())),
            workspace: self.workspace_text.clone(),
            head: head.clone(),
        };
        match self.store.record(&input) {
            RecordMergeHandoffOutcome::Recorded { merge_handed_off } => {
                let report = format!(
                    "handed off: request {} at HEAD {}; no Git ref moved, merging is outside Loom",
                    merge_handed_off.action_request_id.0.0, merge_handed_off.head
                );
                self.console.write(|out| {
                    writeln!(out, "  effect: {}", printable(&report))?;
                    writeln!(out, "  evidence: none")?;
                    Ok(())
                })?;
                self.briefing.record_handoff(
                    &proposal,
                    json!({"kind": "merge_handoff", "action_request_id":
                        merge_handed_off.action_request_id.0.0, "head": merge_handed_off.head,
                        "git_ref_moved": false}),
                );
                Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                    report: json::Value::Text(report),
                    attempt: None,
                    audit: None,
                }))
            }
            RecordMergeHandoffOutcome::AlreadyRecorded { error } => {
                let reason = format!(
                    "a merge hand-off is already recorded for request {}",
                    error.action_request_id.0.0
                );
                self.console.write(|out| {
                    crate::effect::refuse(out, &self.briefing, &proposal.action, &reason)
                })?;
                Ok(EffectOutcome::Refused(EffectOutcomeRefused { reason }))
            }
            RecordMergeHandoffOutcome::StorageFailed { error } => {
                Err(crate::run::SliceError::State(format!(
                    "the merge hand-off cannot be recorded: {}",
                    error.reason
                )))
            }
        }
    }
}

impl EffectPort for HandoffEffects<'_, '_> {
    fn performs(&self, action: &str) -> bool {
        (self.local && action == MERGE) || self.inner.performs(action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if !(self.local && request.data().action == MERGE) {
            return self.inner.invoke(commission, request);
        }
        self.hand_off(commission, request.data())
            .map_err(|failure| {
                let error = EffectError::new(failure.to_string());
                self.console.fail(failure);
                error
            })
    }
}

/// `handoff` as the JSON document the store writes.
fn encode(handoff: &MergeHandoffData) -> String {
    let value = json!({
        "action_request_id": handoff.action_request_id.0.0,
        "commission_id": handoff.commission_id.0.0,
        "workspace": handoff.workspace,
        "head": handoff.head,
    });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}

/// The hand-off `text` holds, refused unless it is `request`'s and names exactly its members.
fn decode(text: &str, request: &ActionRequestId) -> Result<MergeHandoffData, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = exactly(&value, &MEMBERS)?;
    let handoff = MergeHandoffData {
        action_request_id: ActionRequestId(Uuid(text_of(object, "action_request_id")?)),
        commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
        workspace: text_of(object, "workspace")?,
        head: text_of(object, "head")?,
    };
    if &handoff.action_request_id != request {
        return Err(format!(
            "holds the hand-off of request {}, not {}",
            handoff.action_request_id.0.0, request.0.0
        ));
    }
    Ok(handoff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_handed_off_once() {
        let dir = tempfile::Builder::new()
            .prefix("merge-store-")
            .tempdir()
            .unwrap();
        let store = MergeHandoffStore::open(dir.path()).unwrap();
        let input = RecordMergeHandoff {
            action_request_id: ActionRequestId(Uuid("request-1".to_owned())),
            commission_id: CommissionId(Uuid("commission-1".to_owned())),
            workspace: "/work/space".to_owned(),
            head: "0123abcd".to_owned(),
        };
        assert_eq!(store.get(&input.action_request_id).unwrap(), None);
        assert!(matches!(
            store.record(&input),
            RecordMergeHandoffOutcome::Recorded { .. }
        ));
        let mut again = input.clone();
        again.head = "ffff".to_owned();
        assert_eq!(
            store.record(&again),
            RecordMergeHandoffOutcome::AlreadyRecorded {
                error: MergeHandoffAlreadyRecorded {
                    action_request_id: input.action_request_id.clone()
                }
            }
        );
        let kept = store.get(&input.action_request_id).unwrap().unwrap();
        assert_eq!(kept.head, "0123abcd", "the second record changed nothing");
        assert_eq!(kept.workspace, "/work/space");
    }
}
