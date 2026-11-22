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

// ---------------------------------------------------------------------------------------------
// A cancelled tree run (`story:cancel-a-running-bound-program`)
//
// The tree writes no `action started` line, so "at a step's start" is read from the program
// itself: `do.sh` writes `target/started` first.

mod cancel {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use b10x_loom_tree::model::{TreeRun, TreeStop};
    use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
    use b10x_loom_tree::{
        ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings, TreeTime,
        UNCONFINED_FLAG_REASON, open_tree, read_protocols_directory, run_tree, tree_status,
    };
    use loom_protocols::ProtocolCatalog;

    const PROTOCOL: &str = "format: protocol/1
protocol:
  id: halt
  revision: 1
artifacts:
  work: {}
evidence_kinds:
  looked: {}
  done: {}
claims:
  work.done:
    true_when:
      evidence: {kind: done, subject: work, result: ok}
actions:
  work.look:
    effect: read
    may_produce:
      - evidence: looked
  work.do:
    effect: write
    may_produce:
      - evidence: done
outcomes:
  finished:
    requires: {claim: work.done}
";

    const BINDINGS: &str = r#"format: loom.bindings/1
protocol: halt@1
artifacts:
  work: work.txt
actions:
  - action: work.look
    executor: {kind: program, argv: [sh, look.sh]}
    evidence:
      - {kind: looked, subject: work, producer: looker, from: exit, results: {"0": ok, "*": failed}}
  - action: work.do
    executor: {kind: program, argv: [sh, do.sh]}
    timeout_seconds: 60
    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
"#;

    /// The program a cancel ends: it marks its start, a process of its own writes `target/late`
    /// 3 s later, and it sleeps 30 s.
    const DO: &str = "touch target/started\nsleep 3 && touch target/late & sleep 30\n";

    /// A tree of one `halt@1` root, opened, its catalog and its directories.
    struct Halt {
        store: PathBuf,
        workspace: PathBuf,
        catalog: ProtocolCatalog,
    }

    impl Halt {
        fn open(label: &str) -> Self {
            let dir = super::scratch(label);
            let protocols = dir.join("protocols");
            let workspace = dir.join("workspace");
            std::fs::create_dir_all(&protocols).unwrap();
            std::fs::create_dir_all(workspace.join("target")).unwrap();
            std::fs::write(protocols.join("halt.yaml"), PROTOCOL).unwrap();
            std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
            std::fs::write(workspace.join("look.sh"), "exit 0\n").unwrap();
            std::fs::write(workspace.join("do.sh"), DO).unwrap();
            let bindings = dir.join("halt.bindings.yaml");
            std::fs::write(&bindings, BINDINGS).unwrap();
            let directory = read_protocols_directory(&protocols).unwrap();
            let store = dir.join("store");
            open_tree(&OpenRequest {
                store: store.clone(),
                workspace: workspace.clone(),
                protocol: "halt@1".into(),
                bindings,
                revisions: Vec::new(),
                case: "root".into(),
                time: TreeTime::At("2026-11-22T08:00:00Z".into()),
                catalog: &directory.catalog,
                protocols: directory.kept,
            })
            .unwrap();
            Self {
                store,
                workspace,
                catalog: directory.catalog,
            }
        }

        fn settings(max_wall_seconds: Option<i64>) -> RunSettings {
            let runner = EngineRunner::open(&EngineConfinement {
                adapter: ConfinementAdapter::Local,
                local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
                cgroup_root: None,
            })
            .unwrap();
            RunSettings {
                max_steps: None,
                max_wall_seconds,
                time: TreeTime::At("2026-11-22T08:00:00Z".into()),
                runner: std::sync::Arc::new(runner),
                confinement: None,
            }
        }

        /// `run_tree` with `flag`, the lines it wrote in the order written.
        fn run(&self, settings: &RunSettings, flag: &AtomicBool) -> (TreeRun, Vec<RunLogLine>) {
            let mut lines = Vec::new();
            let mut observer = |_: &str, line: &RunLogLine| lines.push(line.clone());
            let run = run_tree(
                &self.store,
                &self.catalog,
                settings,
                Some(&mut observer),
                Some(flag),
            )
            .expect("the tree runs");
            (run, lines)
        }

        /// [`Halt::run`] with `flag` set from a thread of its own `after` `do.sh` started;
        /// answers the run, its lines, the instant the flag was set and the instant `run_tree`
        /// returned.
        fn cancelled_after(&self, after: Duration) -> (TreeRun, Vec<RunLogLine>, Instant, Instant) {
            let flag = AtomicBool::new(false);
            let started = self.workspace.join("target/started");
            std::thread::scope(|scope| {
                let setter = scope.spawn(|| {
                    wait_for(&started);
                    std::thread::sleep(after);
                    flag.store(true, Ordering::SeqCst);
                    Instant::now()
                });
                let (run, lines) = self.run(&Self::settings(None), &flag);
                let returned = Instant::now();
                let set = setter.join().expect("the flag was set");
                (run, lines, set, returned)
            })
        }

        fn late(&self) -> PathBuf {
            self.workspace.join("target/late")
        }
    }

    /// Waits until `path` exists, at most 30 s.
    fn wait_for(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "{} never appeared",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The `stopped` line `lines` end with: its stop and its `next`.
    fn stopped(lines: &[RunLogLine]) -> (String, Vec<String>) {
        match &lines.last().expect("the run wrote lines").event {
            RunLogEvent::Stopped(stopped) => (stopped.stop.clone(), stopped.next.clone()),
            other => panic!("the last line is no stop: {other:?}"),
        }
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
    fn a_tree_run_cancelled_during_a_step_stops_after_it() {
        let tree = Halt::open("cancel-during");
        let (run, lines, _, _) = tree.cancelled_after(Duration::ZERO);
        assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
        let cancelled: Vec<_> = lines
            .iter()
            .filter_map(|line| match &line.event {
                RunLogEvent::ActionFinished(finished)
                    if finished
                        .refusal
                        .as_deref()
                        .is_some_and(|refusal| refusal.starts_with("Cancelled")) =>
                {
                    Some(finished.clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(cancelled.len(), 1, "{lines:#?}");
        assert_eq!(cancelled[0].action, "work.do");
        assert_eq!(cancelled[0].exit, None);
        assert!(cancelled[0].evidence.is_empty(), "{:?}", cancelled[0]);
        assert_eq!(
            stopped(&lines),
            ("Cancelled".to_owned(), vec!["work.do".to_owned()])
        );
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
    fn a_tree_run_cancelled_from_another_thread_returns_within_ten_seconds() {
        let tree = Halt::open("cancel-thread");
        let (run, _, set, returned) = tree.cancelled_after(Duration::from_secs(1));
        let taken = returned.saturating_duration_since(set);
        assert!(
            returned > set && taken < Duration::from_secs(10),
            "run_tree returned {taken:?} after the flag was set"
        );
        assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
        std::thread::sleep(Duration::from_secs(4));
        assert!(
            !tree.late().exists(),
            "a process of the program outlived the cancel"
        );
    }

    #[test]
    fn a_cancel_before_a_step_stops_cancelled() {
        let tree = Halt::open("cancel-before");
        let flag = AtomicBool::new(true);
        let (run, lines) = tree.run(&Halt::settings(None), &flag);
        assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
        assert_eq!(run.case, "root");
        assert_eq!(
            stopped(&lines),
            ("Cancelled".to_owned(), vec!["work.look".to_owned()])
        );
        assert!(
            !lines
                .iter()
                .any(|line| matches!(line.event, RunLogEvent::ActionFinished(_))),
            "a step was taken: {lines:#?}"
        );
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
    fn a_status_names_a_cancelled_stop_and_its_action() {
        let tree = Halt::open("cancel-status-stop");
        let (run, _, _, _) = tree.cancelled_after(Duration::ZERO);
        assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
        let status = tree_status(&tree.store, &tree.catalog).unwrap();
        let root = &status.cases[0];
        assert_eq!(root.stop.as_deref(), Some("Cancelled"));
        assert_eq!(root.stop_next, ["work.do"]);
        // A run whose wall-time budget is spent still stops `BudgetSpent`.
        let spent = Halt::open("cancel-status-wall");
        let (run, lines) = spent.run(&Halt::settings(Some(0)), &AtomicBool::new(false));
        assert_eq!(run.stop, TreeStop::BudgetSpent, "{run:?}");
        assert_eq!(stopped(&lines).0, "BudgetSpent");
        let status = tree_status(&spent.store, &spent.catalog).unwrap();
        assert_eq!(status.cases[0].stop.as_deref(), Some("BudgetSpent"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
    fn a_status_marks_the_cancelled_run() {
        let tree = Halt::open("cancel-status-run");
        tree.cancelled_after(Duration::ZERO);
        let status = tree_status(&tree.store, &tree.catalog).unwrap();
        let runs = &status.cases[0].runs;
        assert_eq!(runs.len(), 2, "{runs:#?}");
        assert_eq!(
            (runs[0].action.as_str(), runs[0].exit, runs[0].cancelled),
            ("work.look", Some(0), None)
        );
        assert_eq!(
            (runs[1].action.as_str(), runs[1].exit, runs[1].cancelled),
            ("work.do", None, Some(true))
        );
        let printed = b10x_loom_tree::status_json(&status);
        assert_eq!(
            printed["cases"][0]["runs"][1]["cancelled"], true,
            "{printed}"
        );
        assert!(
            printed["cases"][0]["runs"][0].get("cancelled").is_none(),
            "{printed}"
        );
        let read = b10x_loom_tree::read_status(printed.to_string().as_bytes()).unwrap();
        assert_eq!(read.cases[0].runs, status.cases[0].runs);
    }
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
