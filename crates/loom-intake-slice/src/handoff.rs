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
//!
//! [`SettledHandoffs`] is the suspension port a resume runs on (`story:handoff-holds-no-suspension`):
//! a `repository.merge` the agent proposes again at a `HEAD` the commission already handed off is
//! settled, so Commission's `HoldSuspension` answers `settled` and holds no Suspension; every other
//! call goes to the state directory's suspension store unchanged.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, Commission, CommissionId as HeldCommissionId, EffectOutcome,
    EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcomeProposedAction, HoldSuspension,
    HoldSuspensionOutcome, ReleaseSuspension, ReleaseSuspensionOutcome, SuspensionSettled,
    SuspensionSnapshot, SuspensionStorageFailed, TakeSuspension, TakeSuspensionOutcome,
    commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStoreError};
use intake_model::obligation::UnmetObligation;
use intake_model::primitives::Uuid;
use intake_model::routing::obligations::RecordMergeHandoffBehavior;
pub use intake_model::routing::{
    ActionRequestId, CommissionId, HeadAlreadyHandedOff, MergeHandedOff,
    MergeHandoffAlreadyRecorded, MergeHandoffData, MergeHandoffStorageFailed, RecordMergeHandoff,
    RecordMergeHandoffOutcome,
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
    /// `MergeHandedOff` it publishes, when none was recorded for the request nor for the
    /// commission and `HEAD`; `AlreadyRecorded`, changing nothing, when one was for the request;
    /// `HeadAlreadyHandedOff`, changing nothing, when the commission handed that `HEAD` off under
    /// another request; `StorageFailed`, with the store's reason, when the directory cannot be
    /// read or written.
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
                return Ok(Recording::AlreadyRecorded);
            }
            if self.find(&handoff.commission_id, &handoff.head)?.is_some() {
                return Ok(Recording::HeadHandedOff);
            }
            self.dir.replace(&path, &text).map(|()| Recording::Recorded)
        });
        match recorded.and_then(|recorded| recorded) {
            Ok(Recording::Recorded) => RecordMergeHandoffOutcome::Recorded {
                merge_handed_off: MergeHandedOff {
                    action_request_id: handoff.action_request_id,
                    commission_id: handoff.commission_id,
                    workspace: handoff.workspace,
                    head: handoff.head,
                },
            },
            Ok(Recording::AlreadyRecorded) => RecordMergeHandoffOutcome::AlreadyRecorded {
                error: MergeHandoffAlreadyRecorded {
                    action_request_id: handoff.action_request_id,
                },
            },
            Ok(Recording::HeadHandedOff) => RecordMergeHandoffOutcome::HeadAlreadyHandedOff {
                error: HeadAlreadyHandedOff {
                    commission_id: handoff.commission_id,
                    head: handoff.head,
                },
            },
            Err(error) => storage_failed(error),
        }
    }

    /// The hand-off `commission` recorded for `head`, under whichever request, or `None` when it
    /// handed that `HEAD` off under none: the rows `head-already-handed-off` selects.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or a hand-off in it cannot be read: a hand-off that
    /// cannot be read might be that one.
    pub fn handed_off(
        &self,
        commission: &CommissionId,
        head: &str,
    ) -> Result<Option<MergeHandoffData>, ResumeStoreError> {
        self.dir.locked(|| self.find(commission, head))?
    }

    /// The hand-off of `commission` at `head`, as [`Self::handed_off`] answers. Only called holding
    /// the lock.
    fn find(
        &self,
        commission: &CommissionId,
        head: &str,
    ) -> Result<Option<MergeHandoffData>, ResumeStoreError> {
        let dir = self.dir.dir();
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|error| failed(dir, error))? {
            let entry = entry.map_err(|error| failed(dir, error))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(PREFIX) && name.ends_with(".json") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        for path in paths {
            let Some(text) = read_text(&path)? else {
                continue;
            };
            let handoff = decode_any(&text).map_err(|problem| failed(&path, problem))?;
            if self.path(&handoff.action_request_id)? != path {
                return Err(failed(
                    &path,
                    format!(
                        "holds the hand-off of request {}",
                        handoff.action_request_id.0.0
                    ),
                ));
            }
            if &handoff.commission_id == commission && handoff.head == head {
                return Ok(Some(handoff));
            }
        }
        Ok(None)
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

/// The case's implementation revision as the governor holds it, read at each hand-off.
pub(crate) type HeldHead<'a> = &'a dyn Fn() -> Result<String, crate::run::SliceError>;

/// The effect port a resume runs on (see the module documentation).
pub struct HandoffEffects<'a, 'o> {
    inner: &'a dyn EffectPort,
    store: &'a MergeHandoffStore,
    workspace: PathBuf,
    workspace_text: String,
    held_head: HeldHead<'a>,
    briefing: Briefing,
    console: &'a Console<'o>,
    local: bool,
}

impl<'a, 'o> HandoffEffects<'a, 'o> {
    /// Merges handed off into `store` for the workspace `workspace` (kept as `workspace_text`),
    /// every other action performed by `inner`; a merge only when `local`, the case's protocol
    /// being `software-change@1`, and only at the `HEAD` `held_head` answers, the case's
    /// implementation revision the governor holds.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        inner: &'a dyn EffectPort,
        store: &'a MergeHandoffStore,
        workspace: PathBuf,
        workspace_text: String,
        held_head: HeldHead<'a>,
        briefing: Briefing,
        console: &'a Console<'o>,
        local: bool,
    ) -> Self {
        Self {
            inner,
            store,
            workspace,
            workspace_text,
            held_head,
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
        // Only the HEAD the case holds as its implementation revision, the one the request was
        // revalidated at, is handed off; a HEAD that moved since is work nobody approved.
        let held = (self.held_head)()?;
        if head != held {
            let reason = format!(
                "the workspace's HEAD {head} is not the implementation revision {held} the case \
                 holds; nothing is handed off"
            );
            self.console.write(|out| {
                crate::effect::refuse(out, &self.briefing, &proposal.action, &reason)
            })?;
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused { reason }));
        }
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
                    usage: None,
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
            RecordMergeHandoffOutcome::HeadAlreadyHandedOff { error } => {
                let reason = format!(
                    "commission {} already handed off HEAD {}",
                    error.commission_id.0.0, error.head
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

/// The suspension port a resume runs on (see the module documentation): `HoldSuspension` for a
/// `repository.merge` whose request would hand off a `HEAD` the commission already handed off is
/// answered `settled` and stores nothing, since the work it would approve is already handed off;
/// every other hold, and every take, release and read, goes to `inner`.
pub struct SettledHandoffs<'a> {
    inner: &'a dyn SuspensionPort,
    store: &'a MergeHandoffStore,
    held_head: HeldHead<'a>,
}

impl<'a> SettledHandoffs<'a> {
    /// The suspension store `inner`, with a merge settled when `store` holds a hand-off of its
    /// commission at the `HEAD` `held_head` answers: the case's implementation revision the
    /// governor holds, the one a hand-off of the request would record.
    pub(crate) fn new(
        inner: &'a dyn SuspensionPort,
        store: &'a MergeHandoffStore,
        held_head: HeldHead<'a>,
    ) -> Self {
        Self {
            inner,
            store,
            held_head,
        }
    }
}

impl SuspensionPort for SettledHandoffs<'_> {
    /// A merge at a handed-off `HEAD` is `settled`; a `HEAD` or a hand-off that cannot be read is
    /// `storage-failed`, holding nothing; every other hold is `inner`'s.
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        if input.action != MERGE {
            return self.inner.hold(input);
        }
        let unreadable = |reason: String| HoldSuspensionOutcome::StorageFailed {
            error: SuspensionStorageFailed { reason },
        };
        let head = match (self.held_head)() {
            Ok(head) => head,
            Err(error) => {
                return unreadable(format!(
                    "the HEAD a merge would hand off cannot be read: {error}"
                ));
            }
        };
        let commission = CommissionId(Uuid(input.commission_id.0.0.clone()));
        match self.store.handed_off(&commission, &head) {
            Ok(None) => self.inner.hold(input),
            Ok(Some(handoff)) => HoldSuspensionOutcome::Settled {
                error: SuspensionSettled {
                    suspension: input.suspension,
                    action_request_id: input.action_request_id,
                    reason: format!(
                        "commission {} already handed off HEAD {} under request {}; the same \
                         work is not handed off twice",
                        handoff.commission_id.0.0, handoff.head, handoff.action_request_id.0.0
                    ),
                },
            },
            Err(error) => unreadable(error.to_string()),
        }
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        self.inner.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.inner.release(input)
    }

    fn get(
        &self,
        commission: &HeldCommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        self.inner.get(commission)
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

/// What [`MergeHandoffStore::record`] found holding the lock.
enum Recording {
    Recorded,
    AlreadyRecorded,
    HeadHandedOff,
}

/// The hand-off `text` holds, refused unless it names exactly its members.
fn decode_any(text: &str) -> Result<MergeHandoffData, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = exactly(&value, &MEMBERS)?;
    Ok(MergeHandoffData {
        action_request_id: ActionRequestId(Uuid(text_of(object, "action_request_id")?)),
        commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
        workspace: text_of(object, "workspace")?,
        head: text_of(object, "head")?,
    })
}

/// The hand-off `text` holds, refused unless it is `request`'s and names exactly its members.
fn decode(text: &str, request: &ActionRequestId) -> Result<MergeHandoffData, String> {
    let handoff = decode_any(text)?;
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

    /// `head-already-handed-off`: a commission hands one HEAD off once, under whichever request;
    /// another HEAD, or another commission's hand-off of the same HEAD, is recorded.
    #[test]
    fn a_head_of_a_commission_is_handed_off_once() {
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
        assert_eq!(
            store.handed_off(&input.commission_id, &input.head).unwrap(),
            None
        );
        assert!(matches!(
            store.record(&input),
            RecordMergeHandoffOutcome::Recorded { .. }
        ));
        let found = store.handed_off(&input.commission_id, &input.head).unwrap();
        assert_eq!(
            found.map(|handoff| handoff.action_request_id),
            Some(input.action_request_id.clone())
        );

        let mut again = input.clone();
        again.action_request_id = ActionRequestId(Uuid("request-2".to_owned()));
        assert_eq!(
            store.record(&again),
            RecordMergeHandoffOutcome::HeadAlreadyHandedOff {
                error: HeadAlreadyHandedOff {
                    commission_id: input.commission_id.clone(),
                    head: input.head.clone(),
                }
            }
        );
        assert_eq!(store.get(&again.action_request_id).unwrap(), None);

        let mut new_work = again.clone();
        new_work.head = "4567cdef".to_owned();
        assert!(matches!(
            store.record(&new_work),
            RecordMergeHandoffOutcome::Recorded { .. }
        ));
        let mut other = again.clone();
        other.action_request_id = ActionRequestId(Uuid("request-3".to_owned()));
        other.commission_id = CommissionId(Uuid("commission-2".to_owned()));
        assert!(matches!(
            store.record(&other),
            RecordMergeHandoffOutcome::Recorded { .. }
        ));

        // A hand-off that cannot be read might be the one asked about: the lookup fails closed.
        std::fs::write(
            store.path(&ActionRequestId(Uuid("x".to_owned()))).unwrap(),
            "{",
        )
        .unwrap();
        assert!(store.handed_off(&input.commission_id, "ffff").is_err());
    }

    /// `story:handoff-holds-no-suspension`: the resume's suspension port settles a merge at a
    /// `HEAD` the commission already handed off, storing nothing; a merge at another `HEAD`, a
    /// merge of another commission and every other action are held by the store as before.
    #[test]
    fn a_merge_at_a_handed_off_head_is_settled_and_holds_no_suspension() {
        use b10x_loom_commission::model::json;
        use b10x_loom_commission::model::primitives::Uuid as HeldUuid;
        use b10x_loom_commission::model::responsibility::{
            ActionRequestId as HeldRequestId, CaseId, ProposedActionArguments, RunId,
            SuspensionKey, SuspensionReason,
        };
        use loom_governor::FileSuspensionStore;

        let dir = tempfile::Builder::new()
            .prefix("settled-handoffs-")
            .tempdir()
            .unwrap();
        let suspensions = FileSuspensionStore::open(dir.path().join("suspensions")).unwrap();
        let handoffs = MergeHandoffStore::open(dir.path().join("merges")).unwrap();
        let commission = "00000000-0000-4000-8000-000000000001";
        let other_commission = "00000000-0000-4000-8000-000000000002";
        let recorded = handoffs.record(&RecordMergeHandoff {
            action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-0000000000a1".into())),
            commission_id: CommissionId(Uuid(commission.to_owned())),
            workspace: "/work/space".to_owned(),
            head: "0123abcd".to_owned(),
        });
        assert!(
            matches!(recorded, RecordMergeHandoffOutcome::Recorded { .. }),
            "{recorded:?}"
        );
        let hold = |commission: &str, action: &str| HoldSuspension {
            suspension: SuspensionKey {
                commission_id: HeldCommissionId(HeldUuid(commission.to_owned())),
            },
            commission_id: HeldCommissionId(HeldUuid(commission.to_owned())),
            run_id: RunId(HeldUuid("00000000-0000-4000-8000-000000000003".to_owned())),
            run_case_revision: 1,
            action_request_id: HeldRequestId(HeldUuid(
                "00000000-0000-4000-8000-0000000000a2".to_owned(),
            )),
            case_id: CaseId("case-1".to_owned()),
            expected_case_revision: 3,
            action: action.to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
            reason: SuspensionReason::Authority(json::Value::Object(vec![(
                "capability".to_owned(),
                json::Value::Text(action.to_owned()),
            )])),
        };
        let held = |commission: &str| {
            suspensions
                .get(&HeldCommissionId(HeldUuid(commission.to_owned())))
                .unwrap()
        };

        let at_handed_off: HeldHead<'_> = &|| Ok("0123abcd".to_owned());
        let port = SettledHandoffs::new(&suspensions, &handoffs, at_handed_off);
        let input = hold(commission, MERGE);
        match port.hold(input.clone()) {
            HoldSuspensionOutcome::Settled { error } => {
                assert_eq!(error.suspension, input.suspension);
                assert_eq!(error.action_request_id, input.action_request_id);
                assert!(error.reason.contains("0123abcd"), "{}", error.reason);
            }
            other => panic!("a merge at a handed-off HEAD is settled: {other:?}"),
        }
        assert_eq!(held(commission), None, "nothing is held");

        let other = port.hold(hold(other_commission, MERGE));
        assert!(
            matches!(other, HoldSuspensionOutcome::Held { .. }),
            "another commission handed off nothing: {other:?}"
        );
        assert!(held(other_commission).is_some());

        let edit = port.hold(hold(commission, "implementation.edit"));
        assert!(
            matches!(edit, HoldSuspensionOutcome::Held { .. }),
            "{edit:?}"
        );
        let stored = held(commission).expect("another action is held");
        assert_eq!(stored.data.action, "implementation.edit");

        let suspensions = FileSuspensionStore::open(dir.path().join("fresh")).unwrap();
        let at_new_work: HeldHead<'_> = &|| Ok("4567cdef".to_owned());
        let port = SettledHandoffs::new(&suspensions, &handoffs, at_new_work);
        let new_work = port.hold(hold(commission, MERGE));
        assert!(
            matches!(new_work, HoldSuspensionOutcome::Held { .. }),
            "a merge at a new HEAD is held: {new_work:?}"
        );
        assert!(
            suspensions
                .get(&HeldCommissionId(HeldUuid(commission.to_owned())))
                .unwrap()
                .is_some()
        );

        let unreadable: HeldHead<'_> = &|| {
            Err(crate::run::SliceError::State(
                "no implementation revision".into(),
            ))
        };
        let suspensions = FileSuspensionStore::open(dir.path().join("unreadable")).unwrap();
        let port = SettledHandoffs::new(&suspensions, &handoffs, unreadable);
        let failed = port.hold(hold(commission, MERGE));
        assert!(
            matches!(failed, HoldSuspensionOutcome::StorageFailed { .. }),
            "a HEAD that cannot be read holds nothing and settles nothing: {failed:?}"
        );
        assert!(
            suspensions
                .get(&HeldCommissionId(HeldUuid(commission.to_owned())))
                .unwrap()
                .is_none()
        );
    }
}
