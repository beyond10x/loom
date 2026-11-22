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

#[test]
fn a_relative_store_is_refused_and_never_resolved_against_the_current_directory() {
    let catalog = loom_protocols::ProtocolCatalog::bundled().unwrap();
    for refused in [
        b10x_loom_tree::tree_status(std::path::Path::new("store"), &catalog).unwrap_err(),
        b10x_loom_tree::verify_tree(std::path::Path::new("store"), &catalog).unwrap_err(),
    ] {
        assert_eq!(refused.reason(), TreeRefusalReason::NotATree, "{refused}");
        assert!(
            refused.0.detail.contains("not an absolute path"),
            "{refused}"
        );
    }
}

#[test]
fn run_settings_are_built_from_items_of_this_crate_alone() {
    use b10x_loom_tree::{
        ConfinementAdapter, EngineConfinement, EngineRunner, RunSettings, TestRunner,
        UNCONFINED_FLAG_REASON,
    };
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .unwrap();
    let runner: std::sync::Arc<dyn TestRunner> = std::sync::Arc::new(runner);
    let settings = RunSettings {
        max_steps: Some(1),
        max_wall_seconds: None,
        time: TreeTime::At("2026-11-22T08:00:00Z".into()),
        runner,
        confinement: None,
    };
    assert_eq!(settings.max_steps, Some(1));
}

/// A `tree status` of the two-level tree after one run with `max_steps: 1`, as Loom printed it
/// before its case entries held `log` and `budget`: the child stopped `BudgetSpent`.
const OLD_STATUS: &str = r#"{"format":"loom.tree-status/1","version":"0.58.0","store":"/srv/tree/store","workspace":"/srv/tree/workspace","root":"root","cases":[{"case":"root","protocol":"tour@1","revision":2,"open_obligations":[],"case_file":"/srv/tree/store/case-726f6f74.json","run_log":"/srv/tree/store/runlog-case-726f6f74.jsonl","protocol_id":"tour","protocol_revision":"1","stop_next":[],"evidence":[{"index":0,"evidence_id":"5842f400-803c-84aa-847f-3704886e183b","kind":"case_outcome","subject":"root","subject_revision":"1","upstream_revisions":{},"producer":"loom-governor","name":"first"}],"runs":[],"approvals":[],"copies":[]},{"case":"root.first","parent":"root","protocol":"tour-leg@1","revision":1,"stop":"BudgetSpent","open_obligations":["route.approved"],"case_file":"/srv/tree/store/case-726f6f742e6669727374.json","run_log":"/srv/tree/store/runlog-case-726f6f742e6669727374.jsonl","name":"first","protocol_id":"tour-leg","protocol_revision":"1","stop_next":[],"evidence":[],"runs":[],"approvals":[],"copies":[]}],"protocols":[]}"#;

#[test]
fn an_old_status_without_log_and_budget_reads() {
    let status = b10x_loom_tree::read_status(OLD_STATUS.as_bytes())
        .unwrap_or_else(|refused| panic!("an old status reads: {refused}"));
    assert_eq!(status.cases.len(), 2);
    for case in &status.cases {
        assert_eq!(case.log, None, "{}: no log member, none read", case.case);
        assert_eq!(
            case.budget, None,
            "{}: no budget member, none read",
            case.case
        );
    }
    let child = &status.cases[1];
    assert_eq!(child.stop.as_deref(), Some("BudgetSpent"));
    assert_eq!(child.name.as_deref(), Some("first"));
    assert_eq!(status.cases[0].evidence[0].name.as_deref(), Some("first"));
    assert!(status.protocols.is_empty());
    // Printed again it is the old document: an absent `log` and `budget` are left out.
    let printed = b10x_loom_tree::status_json(&status);
    let old: serde_json::Value = serde_json::from_str(OLD_STATUS).unwrap();
    assert_eq!(printed, old);
}
