//! Adversary cases for `story:handoff-holds-no-suspension` (wave 2026-10-13-w3, U1): the hand-off
//! record a resume's suspension port (`handoff::SettledHandoffs`) reads before it answers a hold
//! `settled`.
//!
//! `SettledHandoffs` is crate-private, so these cases drive what it asks: `MergeHandoffStore::
//! handed_off`. An answer `Ok(Some(_))` there is a `settled` hold (a Suspension never held, an
//! approval never asked); an `Err` is `storage-failed`, holding nothing and settling nothing.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_intake_slice::handoff::{
    ActionRequestId, CommissionId, MergeHandoffStore, RecordMergeHandoff, RecordMergeHandoffOutcome,
};
use intake_model::primitives::Uuid;

const COMMISSION: &str = "00000000-0000-4000-8000-000000000001";
const REQUEST: &str = "00000000-0000-4000-8000-0000000000a1";
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-w8-u1-{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn commission() -> CommissionId {
    CommissionId(Uuid(COMMISSION.to_owned()))
}

/// Records the hand-off of `REQUEST` by `COMMISSION` at `HEAD` into a store over `dir`, and
/// returns the file it wrote.
fn recorded(dir: &Path) -> PathBuf {
    let store = MergeHandoffStore::open(dir).unwrap();
    let outcome = store.record(&RecordMergeHandoff {
        action_request_id: ActionRequestId(Uuid(REQUEST.to_owned())),
        commission_id: commission(),
        workspace: "/work/space".to_owned(),
        head: HEAD.to_owned(),
    });
    assert!(
        matches!(outcome, RecordMergeHandoffOutcome::Recorded { .. }),
        "{outcome:?}"
    );
    let mut written: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("merge-") && name.ends_with(".json"))
        })
        .collect();
    assert_eq!(written.len(), 1, "{written:?}");
    written.remove(0)
}

/// A hand-off record in the state directory that is a symlink, here to a well-formed record kept
/// outside it, is not a hand-off the commission recorded there: the brief for this unit says a
/// corrupt or symlinked hand-off record never yields `settled`. `handed_off` follows the link
/// (`std::fs::read_to_string` in `state_dir::read_text`) and answers the commission handed `HEAD`
/// off, which `SettledHandoffs::hold` turns into `settled`: no Suspension is held, and no approval
/// is asked, for a merge nothing in this state directory handed off.
#[test]
fn a_symlinked_hand_off_record_is_not_read_as_a_hand_off() {
    let root = scratch("symlinked");
    let elsewhere = root.join("elsewhere");
    let real = recorded(&elsewhere);
    let state = root.join("state");
    let store = MergeHandoffStore::open(&state).unwrap();
    assert_eq!(
        store.handed_off(&commission(), HEAD),
        Ok(None),
        "control: the state directory handed nothing off yet"
    );
    std::os::unix::fs::symlink(&real, state.join(real.file_name().unwrap())).unwrap();

    let answered = store.handed_off(&commission(), HEAD);
    assert!(
        answered.is_err(),
        "a symlinked hand-off record is refused, so the hold is `storage-failed`, never \
         `settled`; it answered {answered:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A hand-off record cut short (a crash mid-write by another tool, a full disk) is not read as no
/// hand-off and not as one: `handed_off` fails, so the hold is `storage-failed` and nothing is
/// held or settled. Each truncation of the record is tried.
#[test]
fn a_truncated_hand_off_record_is_neither_a_hand_off_nor_none() {
    let root = scratch("truncated");
    let state = root.join("state");
    let path = recorded(&state);
    let text = std::fs::read_to_string(&path).unwrap();
    let store = MergeHandoffStore::open(&state).unwrap();
    assert!(
        matches!(store.handed_off(&commission(), HEAD), Ok(Some(_))),
        "control: the whole record is the commission's hand-off of HEAD"
    );
    for cut in 0..text.len() {
        // A cut that leaves the whole JSON document (only trailing whitespace dropped) is the
        // record still, not a truncated one.
        if !text.is_char_boundary(cut)
            || serde_json::from_str::<serde_json::Value>(&text[..cut]).is_ok()
        {
            continue;
        }
        std::fs::write(&path, &text[..cut]).unwrap();
        let answered = store.handed_off(&commission(), HEAD);
        assert!(
            answered.is_err(),
            "the record cut at byte {cut} of {} answered {answered:?}",
            text.len()
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
