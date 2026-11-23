//! A run stopped between the two lines of one child event is taken over by the next writer.
//!
//! A child event is two appends: the child's line and then the parent's (`case opened` then
//! `child opened`, `terminated` then `child closed`). A stop between them is simulated by an
//! observer that panics at the child's line, which `run_tree` gives right after that line and its
//! head are written and before the parent's line is appended, as a killed process leaves the
//! store; `tree close` takes no observer, so its stop is simulated by putting the parent's log and
//! head back as they were before it. The next `run_tree` writes the missing line once. A local
//! runner, `sh` stubs, scratch under `CARGO_TARGET_TMPDIR`; no network, no model.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use b10x_loom_tree::model::TreeStop;
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    CloseRequest, ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings,
    TreeTime, UNCONFINED_FLAG_REASON, close, head_file_name, log_file_name, open_tree, read_log,
    read_protocols_directory, run_tree, verify_tree,
};
use loom_protocols::ProtocolCatalog;

const NEST: &str = "format: protocol/1
protocol:
  id: nest
  revision: 1
artifacts:
  plan: {}
  nest: {}
evidence_kinds:
  case_outcome: {}
  kid_opened: {}
claims:
  kid.opened:
    true_when:
      evidence: {kind: kid_opened, subject: nest, result: opened}
  kids.passed:
    true_when:
      children: {in: [passed]}
actions:
  kid.open:
    effect: write
    case_open: {protocol: nest.kid, subject: plan}
    may_produce:
      - evidence: kid_opened
outcomes:
  done:
    requires:
      all:
        - {claim: kid.opened}
        - {claim: kids.passed}
";

/// The child: it ends through `passed` once its check ran, by itself.
const KID: &str = "format: protocol/1
protocol:
  id: nest.kid
  revision: 1
artifacts:
  plan: {}
evidence_kinds:
  checked: {}
claims:
  plan.ok:
    true_when:
      evidence: {kind: checked, subject: plan, result: ok}
actions:
  plan.check:
    effect: read
    may_produce:
      - evidence: checked
outcomes:
  passed:
    requires: {claim: plan.ok}
";

/// The child with two legitimate outcomes once its check ran: the run waits for `tree close`.
const KID_TWO_OUTCOMES: &str = "format: protocol/1
protocol:
  id: nest.kid
  revision: 1
artifacts:
  plan: {}
evidence_kinds:
  checked: {}
claims:
  plan.ok:
    true_when:
      evidence: {kind: checked, subject: plan, result: ok}
actions:
  plan.check:
    effect: read
    may_produce:
      - evidence: checked
outcomes:
  passed:
    requires: {claim: plan.ok}
  waived:
    requires: {claim: plan.ok}
";

const NEST_BINDINGS: &str = r#"format: loom.bindings/1
protocol: nest@1
artifacts:
  plan: plan.txt
  nest: nest.txt
actions:
  - action: kid.open
    executor: {kind: open, name: kid, bindings: kid.bindings.yaml}
    evidence:
      - {kind: kid_opened, subject: nest, producer: loom, from: effect, results: {performed: opened}}
"#;

const KID_BINDINGS: &str = r#"format: loom.bindings/1
protocol: nest-kid@1
artifacts:
  plan: plan.txt
actions:
  - action: plan.check
    executor: {kind: program, argv: [sh, check.sh]}
    evidence:
      - {kind: checked, subject: plan, producer: checker, from: exit, results: {"0": ok, "*": bad}}
"#;

const ROOT: &str = "root";
const CHILD: &str = "root.kid";
const AT: &str = "2026-11-22T08:00:00Z";

struct Nest {
    store: PathBuf,
    catalog: ProtocolCatalog,
}

impl Nest {
    /// A root of `nest@1` opened, its child of the protocol `kid`.
    fn open(label: &str, kid: &str) -> Self {
        let dir = super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(protocols.join("nest.yaml"), NEST).unwrap();
        std::fs::write(protocols.join("nest-kid.yaml"), kid).unwrap();
        std::fs::write(workspace.join("plan.txt"), "plan\n").unwrap();
        std::fs::write(workspace.join("nest.txt"), "nest\n").unwrap();
        std::fs::write(workspace.join("check.sh"), "exit 0\n").unwrap();
        let bindings = dir.join("nest.bindings.yaml");
        std::fs::write(&bindings, NEST_BINDINGS).unwrap();
        std::fs::write(dir.join("kid.bindings.yaml"), KID_BINDINGS).unwrap();
        let directory = read_protocols_directory(&protocols).unwrap();
        let store = dir.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace,
            protocol: "nest@1".into(),
            bindings,
            revisions: Vec::new(),
            case: ROOT.into(),
            time: TreeTime::At(AT.into()),
            catalog: &directory.catalog,
            protocols: directory.kept,
        })
        .unwrap();
        Self {
            store,
            catalog: directory.catalog,
        }
    }

    fn settings() -> RunSettings {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        RunSettings {
            max_steps: None,
            max_wall_seconds: Some(30),
            time: TreeTime::At(AT.into()),
            runner: Arc::new(runner),
            confinement: None,
            cgroup_root: None,
        }
    }

    /// `run_tree` whose observer panics right after `stop_at` accepts a line of `case`: the
    /// store as a process killed at that instant leaves it. Whether the panic came.
    fn run_stopped_at(&self, stop_at: fn(&str, &RunLogLine) -> bool) -> bool {
        let settings = Self::settings();
        catch_unwind(AssertUnwindSafe(|| {
            let mut observer = |case: &str, line: &RunLogLine| {
                if stop_at(case, line) {
                    panic!("the process stops here");
                }
            };
            run_tree(
                &self.store,
                &self.catalog,
                &settings,
                Some(&mut observer),
                None,
            )
        }))
        .is_err()
    }

    /// `run_tree` to its end.
    fn run(&self) -> TreeStop {
        let ran = run_tree(&self.store, &self.catalog, &Self::settings(), None, None)
            .expect("the tree runs");
        if ran.stop != TreeStop::Terminal {
            eprintln!(
                "the run: {ran:#?}\nroot: {:#?}\nchild: {:#?}",
                self.lines(ROOT),
                self.lines(CHILD)
            );
        }
        ran.stop
    }

    fn lines(&self, case: &str) -> Vec<RunLogLine> {
        let bytes = std::fs::read(self.store.join(log_file_name(case))).unwrap_or_default();
        read_log(&bytes).expect("the log reads")
    }

    fn count(&self, case: &str, kind: fn(&RunLogEvent) -> bool) -> usize {
        self.lines(case)
            .iter()
            .filter(|line| kind(&line.event))
            .count()
    }

    fn child_opened(&self) -> usize {
        self.count(
            ROOT,
            |event| matches!(event, RunLogEvent::ChildOpened(opened) if opened.child == CHILD),
        )
    }

    fn child_closed(&self) -> usize {
        self.count(
            ROOT,
            |event| matches!(event, RunLogEvent::ChildClosed(closed) if closed.child == CHILD),
        )
    }

    /// Every log verifies against its head: the store is fit for the next writer.
    fn verifies(&self) {
        let verified = verify_tree(&self.store, &self.catalog).expect("the tree verifies");
        assert!(verified.failure.is_none(), "{:?}", verified.failure);
    }
}

/// The control: run through, the parent's log names the child's opening and its close once
/// each, so the fixture reaches both child events.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_uninterrupted_run_writes_both_child_lines_once() {
    let tree = Nest::open("take-over-control", KID);
    assert_eq!(tree.run(), TreeStop::Terminal);
    assert_eq!(tree.child_opened(), 1, "{:#?}", tree.lines(ROOT));
    assert_eq!(tree.child_closed(), 1, "{:#?}", tree.lines(ROOT));
}

/// A stop after the child's `terminated` line and before the parent's `child closed` line: the
/// next run writes the parent's line, once.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_stop_between_terminated_and_child_closed_is_taken_over() {
    let tree = Nest::open("take-over-closed", KID);
    let stopped = tree.run_stopped_at(|case, line| {
        case == CHILD && matches!(line.event, RunLogEvent::Terminated(_))
    });
    assert!(stopped, "the run never wrote the child's terminated line");
    assert_eq!(
        tree.child_closed(),
        0,
        "the stop came after the parent's line"
    );
    tree.verifies();

    assert_eq!(tree.run(), TreeStop::Terminal);
    assert_eq!(
        tree.child_closed(),
        1,
        "after the next run the parent's log names the child's close {} times: {:#?}",
        tree.child_closed(),
        tree.lines(ROOT)
    );
    assert_eq!(
        tree.count(CHILD, |event| matches!(event, RunLogEvent::Terminated(_))),
        1
    );
    tree.verifies();
}

/// A stop after the child's `case opened` line and before the parent's `child opened` line: the
/// next run writes the parent's line, once.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_stop_between_case_opened_and_child_opened_is_taken_over() {
    let tree = Nest::open("take-over-opened", KID);
    let stopped = tree.run_stopped_at(|case, line| {
        case == CHILD && matches!(line.event, RunLogEvent::CaseOpened(_))
    });
    assert!(stopped, "the run never wrote the child's case opened line");
    assert_eq!(
        tree.child_opened(),
        0,
        "the stop came after the parent's line"
    );
    tree.verifies();

    assert_eq!(tree.run(), TreeStop::Terminal);
    assert_eq!(
        tree.child_opened(),
        1,
        "after the next run the parent's log names the child's opening {} times: {:#?}",
        tree.child_opened(),
        tree.lines(ROOT)
    );
    assert_eq!(
        tree.count(CHILD, |event| matches!(event, RunLogEvent::CaseOpened(_))),
        1
    );
    // The parent names the child's opening before its close.
    let kinds: Vec<&str> = tree
        .lines(ROOT)
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ChildOpened(_) => Some("opened"),
            RunLogEvent::ChildClosed(_) => Some("closed"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["opened", "closed"]);
    tree.verifies();
}

/// A stop of a run before the child's `case opened` line, after the open step's own line: the
/// child is linked in the governor and neither log names it; the next run writes both lines.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_stop_before_case_opened_is_taken_over() {
    let tree = Nest::open("take-over-before-opened", KID);
    let stopped = tree.run_stopped_at(|case, line| {
        case == ROOT
            && matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == "kid.open")
    });
    assert!(stopped, "the run never wrote the open step's line");
    assert!(tree.lines(CHILD).is_empty());
    tree.verifies();

    assert_eq!(tree.run(), TreeStop::Terminal);
    assert_eq!(tree.child_opened(), 1, "{:#?}", tree.lines(ROOT));
    assert_eq!(
        tree.count(CHILD, |event| matches!(event, RunLogEvent::CaseOpened(_))),
        1
    );
    tree.verifies();
}

/// `tree close` of a child stopped between the child's `terminated` line and the parent's
/// `child closed` line: the next run writes the parent's line, once.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_close_stopped_between_its_two_lines_is_taken_over() {
    let tree = Nest::open("take-over-close", KID_TWO_OUTCOMES);
    let waits = run_tree(&tree.store, &tree.catalog, &Nest::settings(), None, None).unwrap();
    assert_eq!(waits.stop, TreeStop::AwaitingPerson, "{waits:?}");
    assert_eq!(waits.case, CHILD);

    // The parent's log and head as they are before the close: put back after it, they are the
    // store a close stopped after the child's line leaves.
    let log = tree.store.join(log_file_name(ROOT));
    let head = tree.store.join(head_file_name(ROOT));
    let (log_before, head_before) = (std::fs::read(&log).unwrap(), std::fs::read(&head).unwrap());
    close(&CloseRequest {
        store: &tree.store,
        case: CHILD,
        outcome: "passed",
        time: TreeTime::At(AT.into()),
        catalog: &tree.catalog,
    })
    .unwrap();
    assert_eq!(tree.child_closed(), 1, "the close wrote the parent's line");
    std::fs::write(&log, log_before).unwrap();
    std::fs::write(&head, head_before).unwrap();
    assert_eq!(tree.child_closed(), 0);
    tree.verifies();

    assert_eq!(tree.run(), TreeStop::Terminal);
    assert_eq!(tree.child_closed(), 1, "{:#?}", tree.lines(ROOT));
    let closed = tree
        .lines(ROOT)
        .into_iter()
        .find_map(|line| match line.event {
            RunLogEvent::ChildClosed(closed) => Some(closed),
            _ => None,
        })
        .unwrap();
    assert_eq!(closed.outcome.as_deref(), Some("passed"));
    assert!(
        closed.record.is_some() && closed.record_digest.is_some(),
        "{closed:?}"
    );
    tree.verifies();
}
