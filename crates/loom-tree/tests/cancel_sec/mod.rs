//! The cancel of a tree run, from the security review of the w7 batch (lane 2), kept as the
//! acceptance of `story:handoff-check-holds-the-cancel`.
//!
//! - `run_tree`'s documented bound: a cancel set from any thread ends the run "within 10 s of being
//!   set", including while a writer's `handoff_when` check runs (the check is a bound child of
//!   the step, `crates/loom-tree/src/handoff.rs`).
//! - One cancel per run, never process-wide: two runs in one process, one cancelled, the other
//!   ends as if nothing happened.
//!
//! Every program is POSIX sh under Substrate's `local` adapter; no network, no model, no
//! credential.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_tree::model::{TreeRefusalReason, TreeRun, TreeStop};
use b10x_loom_tree::runlog::RunLogEvent;
use b10x_loom_tree::{
    ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings, TreeTime,
    UNCONFINED_FLAG_REASON, open_tree, read_protocols_directory, run_tree,
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

/// The bindings, `{do}` the extra members of `work.do`'s binding.
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
{do}    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
"#;

struct Tree {
    store: PathBuf,
    workspace: PathBuf,
    catalog: ProtocolCatalog,
}

impl Tree {
    /// A one-case `halt@1` tree whose `do.sh` is `program`, `work.do`'s binding carrying `extra`
    /// (indented four spaces, each line ended), and `check.sh` holding `check`.
    fn open(label: &str, program: &str, extra: &str, check: &str) -> Self {
        let dir = super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(workspace.join("target")).unwrap();
        std::fs::write(protocols.join("halt.yaml"), PROTOCOL).unwrap();
        std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
        std::fs::write(workspace.join("look.sh"), "exit 0\n").unwrap();
        std::fs::write(workspace.join("do.sh"), program).unwrap();
        std::fs::write(workspace.join("check.sh"), check).unwrap();
        let bindings = dir.join("halt.bindings.yaml");
        std::fs::write(&bindings, BINDINGS.replace("{do}", extra)).unwrap();
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

    fn run(&self, flag: &AtomicBool) -> TreeRun {
        run_tree(&self.store, &self.catalog, &settings(), None, Some(flag)).expect("the tree runs")
    }
}

/// A run's settings: no budget, the `local` adapter.
fn settings() -> RunSettings {
    let runner = EngineRunner::open(&EngineConfinement {
        adapter: ConfinementAdapter::Local,
        local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
        cgroup_root: None,
    })
    .unwrap();
    RunSettings {
        max_steps: None,
        max_wall_seconds: None,
        time: TreeTime::At("2026-11-22T08:00:00Z".into()),
        runner: std::sync::Arc::new(runner),
        confinement: None,
        cgroup_root: None,
    }
}

/// `run_tree` with a cancel flag returns when the run ends, not when its cancel watcher next
/// wakes (`story:run-tree-returns-when-the-run-ends`). The measured runs are of a tree whose root
/// is already terminated: each reads the store and ends `Terminal` without a step, in about
/// 10 ms. A run that waits out the watcher's poll (`CANCEL_WATCH`, 100 ms) takes 100 ms or more,
/// every time, so the fastest of 20 is held below half of it. A run refused at once (an early
/// return) is held to the same bound.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_run_with_a_cancel_flag_returns_when_it_ends() {
    let tree = Tree::open("rs-cancel-returns", "exit 0\n", "", "exit 1\n");
    let flag = AtomicBool::new(false);
    assert_eq!(tree.run(&flag).stop, TreeStop::Terminal);
    let settings = settings();
    let missing = tree.store.with_file_name("no-store");
    let mut ended = Vec::new();
    let mut refused = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let run = run_tree(&tree.store, &tree.catalog, &settings, None, Some(&flag))
            .expect("the tree runs");
        ended.push(started.elapsed());
        assert_eq!(run.stop, TreeStop::Terminal, "{run:?}");
        let started = Instant::now();
        let refusal = run_tree(&missing, &tree.catalog, &settings, None, Some(&flag))
            .expect_err("no store there");
        refused.push(started.elapsed());
        assert_eq!(refusal.reason(), TreeRefusalReason::NotATree, "{refusal:?}");
    }
    let bound = Duration::from_millis(100) / 2;
    for (what, taken) in [("ended", &ended), ("refused", &refused)] {
        let fastest = taken.iter().min().copied().unwrap_or_default();
        assert!(
            fastest < bound,
            "the fastest of 20 runs with a cancel flag that {what} at once took {fastest:?}: {taken:?}"
        );
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

/// `run_tree` promises its cancel ends the run within 10 s of being set, also while the
/// `handoff_when` check of the step runs: the check holds the run's cancel handle. The check here
/// sleeps 15 s, so a check that holds none fails the bound.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_during_a_handoff_check_ends_the_run_within_ten_seconds() {
    let tree = Tree::open(
        "rs-cancel-check",
        "exit 0\n",
        "    handoff: a person finishes the work\n    handoff_when: [sh, check.sh]\n",
        "touch target/checking\nsleep 15\nexit 1\n",
    );
    let flag = AtomicBool::new(false);
    let checking = tree.workspace.join("target/checking");
    let (run, set, returned) = std::thread::scope(|scope| {
        let setter = scope.spawn(|| {
            wait_for(&checking);
            flag.store(true, Ordering::SeqCst);
            Instant::now()
        });
        let run = tree.run(&flag);
        let returned = Instant::now();
        (run, setter.join().expect("the flag was set"), returned)
    });
    let taken = returned.saturating_duration_since(set);
    assert!(
        taken < Duration::from_secs(10),
        "run_tree returned {taken:?} after its cancel was set during the handoff_when check (stop {:?})",
        run.stop
    );
    assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
}

/// One cancel per run: two runs in one process at once, the first cancelled while both run a
/// program; the second's program is not ended and its tree reaches its outcome.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_ends_its_own_run_and_no_other_in_the_process() {
    let cancelled = Tree::open(
        "rs-cancel-own-a",
        "touch target/started\nsleep 30\n",
        "",
        "exit 1\n",
    );
    let other = Tree::open(
        "rs-cancel-own-b",
        "touch target/started\nwhile [ ! -e target/go ]; do sleep 0.05; done\nexit 0\n",
        "",
        "exit 1\n",
    );
    let flag_a = AtomicBool::new(false);
    let flag_b = AtomicBool::new(false);
    let started = Instant::now();
    let (run_a, run_b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| cancelled.run(&flag_a));
        let b = scope.spawn(|| other.run(&flag_b));
        wait_for(&cancelled.workspace.join("target/started"));
        wait_for(&other.workspace.join("target/started"));
        flag_a.store(true, Ordering::SeqCst);
        // Long enough for the first run's watcher to set its handle and its runner to end its
        // program (100 ms each under `local`).
        std::thread::sleep(Duration::from_secs(1));
        std::fs::write(other.workspace.join("target/go"), "").unwrap();
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(run_a.stop, TreeStop::Cancelled, "{run_a:?}");
    assert_eq!(run_b.stop, TreeStop::Terminal, "{run_b:?}");
    assert_eq!(run_b.detail.as_deref(), Some("finished"), "{run_b:?}");
    assert!(!flag_b.load(Ordering::SeqCst));
    assert!(started.elapsed() < Duration::from_secs(30));
}

/// A step its cancel ended after it moved its artifact names the move on its `action finished`
/// line (`story:action-dated-by-revisions-its-step-left`): the moves are the difference of the
/// case's revisions before and after the step, whatever its end, never read from a report the
/// cancelled step does not have.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancelled_step_that_moved_an_artifact_names_its_move() {
    let started = Instant::now();
    let tree = Tree::open(
        "rs-cancel-move",
        "echo more >> work.txt\ntouch target/started\nsleep 30\n",
        "",
        "exit 1\n",
    );
    let flag = AtomicBool::new(false);
    let run = std::thread::scope(|scope| {
        scope.spawn(|| {
            wait_for(&tree.workspace.join("target/started"));
            flag.store(true, Ordering::SeqCst);
        });
        tree.run(&flag)
    });
    assert_eq!(run.stop, TreeStop::Cancelled, "{run:?}");
    let finished = super::replay::fixture::lines(&tree.store, "root")
        .into_iter()
        .rev()
        .find_map(|line| match line.event {
            RunLogEvent::ActionFinished(finished) => Some(finished),
            _ => None,
        })
        .expect("an action finished line");
    assert_eq!(finished.action, "work.do", "{finished:?}");
    assert!(
        finished
            .refusal
            .as_deref()
            .is_some_and(|refusal| refusal.starts_with("Cancelled: ")),
        "{finished:?}"
    );
    let held = super::replay::fixture::governor(&tree.store)
        .revisions(&CaseId("root".to_owned()))
        .unwrap()["work"]
        .clone();
    assert_eq!(finished.revisions.len(), 1, "{finished:?}");
    assert_eq!(finished.revisions[0].artifact, "work", "{finished:?}");
    assert_eq!(finished.revisions[0].to, held, "{finished:?}");
    assert!(started.elapsed() < Duration::from_secs(30));
}
