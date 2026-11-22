//! The parts of `b10x-loom-tree` a command does not reach alone: the `--at` forms, the decision an
//! approval list gives, and a run log the tree kept no head for. The commands themselves are
//! `crates/loom-cli/tests/tree_run.rs`. Scratch is under `CARGO_TARGET_TMPDIR`; no test reaches the
//! network, a model or a credential.

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

/// The fixture of the tree command tests: `sign@1` and its bindings, `road.txt` and the stubs.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../loom-cli/tests/fixtures/tree-two-level"
);

#[test]
fn run_tree_returns_the_principal_of_a_person_decision() {
    use b10x_loom_tree::model::DecisionKind;
    use b10x_loom_tree::{
        ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings,
        UNCONFINED_FLAG_REASON, open_tree, run_tree,
    };
    let dir = scratch("principal");
    let fixture = std::path::Path::new(FIXTURE);
    let workspace = dir.join("workspace");
    fs::create_dir_all(workspace.join("stubs")).unwrap();
    for file in ["road.txt", "sign.bindings.yaml", "stubs/post-stamp.sh"] {
        fs::copy(fixture.join(file), workspace.join(file)).unwrap();
    }
    let installed = loom_protocols::InstallStore::new(dir.join("protocols"));
    installed
        .install_file("sign@1", &fixture.join("extra/sign.yaml"), false)
        .unwrap();
    let catalog = installed.catalog().unwrap();
    let store = dir.join("store");
    let at = TreeTime::At("2026-11-22T08:00:00Z".into());
    open_tree(&OpenRequest {
        store: store.clone(),
        workspace: workspace.clone(),
        protocol: "sign@1".into(),
        bindings: workspace.join("sign.bindings.yaml"),
        revisions: Vec::new(),
        case: "root".into(),
        time: at.clone(),
        catalog: &catalog,
        protocols: Vec::new(),
    })
    .unwrap();
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .unwrap();
    let settings = RunSettings {
        max_steps: None,
        max_wall_seconds: None,
        time: at,
        runner: std::sync::Arc::new(runner),
        confinement: Some("local".into()),
    };
    let ran = run_tree(&store, &catalog, &settings, None, None).unwrap();

    let bindings = b10x_loom_bindings::decode(
        &fs::read_to_string(fixture.join("sign.bindings.yaml")).unwrap(),
    )
    .unwrap();
    for action in ["post.sign", "post.seal"] {
        let decision = ran
            .decisions
            .iter()
            .find(|decision| decision.action.as_deref() == Some(action))
            .unwrap_or_else(|| panic!("no decision about {action}: {ran:?}"));
        assert_eq!(decision.kind, DecisionKind::ApproveAction, "{decision:?}");
        let bound = bindings
            .actions
            .iter()
            .find(|bound| bound.action == action)
            .and_then(|bound| bound.executor.principal.clone());
        assert!(bound.is_some(), "{action}");
        assert_eq!(decision.principal, bound, "{decision:?}");
        let at = decision
            .command
            .iter()
            .position(|word| word == "--principal")
            .unwrap();
        assert_eq!(Some(&decision.command[at + 1]), bound.as_ref());
    }
    let _ = fs::remove_dir_all(&dir);
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
