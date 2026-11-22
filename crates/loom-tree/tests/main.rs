//! The parts of `b10x-loom-tree` a command does not reach alone: the `--at` forms, the decision an
//! approval list gives, and a run log the tree kept no head for. The commands themselves are
//! `crates/loom-cli/tests/tree_run.rs`. Scratch is under `CARGO_TARGET_TMPDIR`; no test reaches the
//! network, a model or a credential.

mod suite_format;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_runlog::RunLog;
use b10x_loom_tree::model::{ApprovalDecision, CaseApproval, TreeRefusalReason};
use b10x_loom_tree::{TreeAuthority, TreeTime, verify_case_log};
use loom::primitives::Timestamp;
use loom::runlog::{CaseOpened, RunLogEvent};

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loom-tree-{label}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn at_takes_only_an_instant_in_utc_to_the_second() {
    assert_eq!(
        TreeTime::from_flag(Some("2026-11-22T08:00:00Z")).unwrap(),
        TreeTime::At("2026-11-22T08:00:00Z".into())
    );
    assert_eq!(TreeTime::from_flag(None).unwrap(), TreeTime::Host);
    for refused in [
        "2026-11-22T08:00:00+01:00",
        "2026-11-22 08:00:00Z",
        "2026-11-22T08:00:00.5Z",
        "2026-13-01T08:00:00Z",
        "",
    ] {
        let error = TreeTime::from_flag(Some(refused)).unwrap_err();
        assert_eq!(
            error.reason(),
            TreeRefusalReason::InvalidInstant,
            "{refused}"
        );
    }
}

fn approval(
    case: &str,
    capability: &str,
    revision: i64,
    decision: ApprovalDecision,
) -> CaseApproval {
    CaseApproval {
        case: case.into(),
        capability: capability.into(),
        decision,
        principal: "guide".into(),
        case_revision: revision,
        at: Timestamp("2026-11-22T08:00:00Z".into()),
    }
}

#[test]
fn an_approval_holds_for_its_case_capability_and_revision_and_a_denial_wins() {
    let authority = TreeAuthority::new(BTreeMap::from([(
        "root".to_owned(),
        vec![
            approval("root", "guide.issue", 2, ApprovalDecision::Allow),
            approval("root", "guide.approve", 2, ApprovalDecision::Allow),
            approval("root", "guide.approve", 2, ApprovalDecision::Deny),
        ],
    )]));
    assert_eq!(
        authority.decision("root", "guide.issue", 2),
        Some(ApprovalDecision::Allow)
    );
    assert_eq!(authority.decision("root", "guide.issue", 3), None);
    assert_eq!(authority.decision("root.first", "guide.issue", 2), None);
    assert_eq!(authority.decision("root", "guide.other", 2), None);
    assert_eq!(
        authority.decision("root", "guide.approve", 2),
        Some(ApprovalDecision::Deny)
    );
}

#[test]
fn a_log_the_tree_kept_no_head_for_fails_at_its_first_line() {
    let dir = scratch("no-head");
    assert_eq!(verify_case_log(&dir, "root").unwrap(), None);
    let log = RunLog::open(dir.clone()).unwrap();
    log.append(
        "root",
        Timestamp("2026-11-22T08:00:00Z".into()),
        RunLogEvent::CaseOpened(CaseOpened {
            case: "root".into(),
            protocol: "tour@1".into(),
        }),
    )
    .unwrap();
    let failure = verify_case_log(&dir, "root").unwrap().unwrap();
    assert_eq!((failure.case.as_str(), failure.line), ("root", 0));
    let _ = fs::remove_dir_all(&dir);
}
