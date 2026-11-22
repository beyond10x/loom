//! A spent run allowance read in an outcome's `requires` (`story:outcome-when-reads-the-run-count`):
//! a writer's `from: runs` entry writes `spent` on the run that reaches `budget.max_runs`, a claim
//! over that kind ends the case through the outcome that requires it, and the kind alone never
//! makes the writer useful again. Each tree is one `halt@1`-shaped root under the `local` runner,
//! with `sh` stubs; no model, no network.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use b10x_loom_tree::model::{TreeRun, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings, TreeTime,
    UNCONFINED_FLAG_REASON, open_tree, read_protocols_directory, run_tree,
};

/// `work.do` writes `done` and may write `spent`; `finished` needs `done`, `given_up` the spent
/// allowance.
const GIVE_UP_PROTOCOL: &str = "format: protocol/1
protocol:
  id: halt
  revision: 1
artifacts:
  work: {}
evidence_kinds:
  done: {}
  spent: {}
claims:
  work.done:
    true_when:
      evidence: {kind: done, subject: work, result: ok}
  work.spent:
    true_when:
      evidence: {kind: spent, subject: work, result: spent}
actions:
  work.do:
    effect: write
    may_produce:
      - evidence: done
      - evidence: spent
outcomes:
  finished:
    requires: {claim: work.done}
  given_up:
    requires: {claim: work.spent}
";

/// `work.do` has two runs and is sent back on `failed`; its second run writes `spent`.
const GIVE_UP_BINDINGS: &str = r#"format: loom.bindings/1
protocol: halt@1
artifacts:
  work: work.txt
actions:
  - action: work.do
    executor: {kind: program, argv: [sh, do.sh]}
    budget: {max_runs: 2}
    retry_when: {done: [failed]}
    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
      - {kind: spent, subject: work, producer: loom, from: runs, results: {spent: spent}}
"#;

/// The same tree with `work.do`'s `from: runs` entry left out: the planted defect of test 4.
fn give_up_bindings_without_runs() -> String {
    GIVE_UP_BINDINGS
        .lines()
        .filter(|line| !line.contains("from: runs"))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// `work` and `note`: `note.write` moves the case revision after `work.do`, and `finished` stays
/// unmet, since no action produces `checked`.
const IDLE_PROTOCOL: &str = "format: protocol/1
protocol:
  id: halt
  revision: 1
artifacts:
  work: {}
  note: {}
evidence_kinds:
  done: {}
  spent: {}
  noted: {}
  checked: {}
claims:
  work.done:
    true_when:
      evidence: {kind: done, subject: work, result: ok}
  work.spent:
    true_when:
      evidence: {kind: spent, subject: work, result: spent}
  note.written:
    true_when:
      evidence: {kind: noted, subject: note, result: ok}
  note.checked:
    true_when:
      evidence: {kind: checked, subject: note, result: ok}
actions:
  work.do:
    effect: write
    may_produce:
      - evidence: done
      - evidence: spent
  note.write:
    effect: write
    precondition: {claim: work.done}
    may_produce:
      - evidence: noted
outcomes:
  finished:
    requires:
      all:
        - {claim: work.done}
        - {claim: note.written}
        - {claim: note.checked}
  given_up:
    requires: {claim: work.spent}
";

const IDLE_BINDINGS: &str = r#"format: loom.bindings/1
protocol: halt@1
artifacts:
  work: work.txt
  note: note.txt
actions:
  - action: work.do
    executor: {kind: program, argv: [sh, do.sh]}
    budget: {max_runs: 3}
    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
      - {kind: spent, subject: work, producer: loom, from: runs, results: {spent: spent}}
  - action: note.write
    executor: {kind: program, argv: [sh, note.sh]}
    evidence:
      - {kind: noted, subject: note, producer: noter, from: exit, results: {"0": ok, "*": failed}}
"#;

/// A tree of one root opened from `protocol` and `bindings`, `do.sh` holding `program`.
struct Tree {
    store: PathBuf,
    catalog: loom_protocols::ProtocolCatalog,
}

impl Tree {
    fn open(label: &str, protocol: &str, bindings: &str, program: &str) -> Self {
        let dir = super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(protocols.join("halt.yaml"), protocol).unwrap();
        std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
        std::fs::write(workspace.join("note.txt"), "note\n").unwrap();
        std::fs::write(workspace.join("do.sh"), program).unwrap();
        std::fs::write(
            workspace.join("note.sh"),
            "echo noted >> note.txt\nexit 0\n",
        )
        .unwrap();
        let file = dir.join("halt.bindings.yaml");
        std::fs::write(&file, bindings).unwrap();
        let directory = read_protocols_directory(&protocols).unwrap();
        let store = dir.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace,
            protocol: "halt@1".into(),
            bindings: file,
            revisions: Vec::new(),
            case: "root".into(),
            time: TreeTime::At("2026-11-22T08:00:00Z".into()),
            catalog: &directory.catalog,
            protocols: directory.kept,
        })
        .unwrap();
        Self {
            store,
            catalog: directory.catalog,
        }
    }

    /// One `run_tree` under `local`, the lines it wrote in the order written.
    fn run(&self) -> (TreeRun, Vec<RunLogLine>) {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        let settings = RunSettings {
            max_steps: None,
            max_wall_seconds: None,
            time: TreeTime::At("2026-11-22T08:00:00Z".into()),
            runner: std::sync::Arc::new(runner),
            confinement: Some("local".into()),
            cgroup_root: None,
        };
        let mut lines = Vec::new();
        let mut observer = |_: &str, line: &RunLogLine| lines.push(line.clone());
        let run = run_tree(
            &self.store,
            &self.catalog,
            &settings,
            Some(&mut observer),
            Some(&AtomicBool::new(false)),
        )
        .expect("the tree runs");
        (run, lines)
    }
}

/// The steps of `action` the run took: its `action finished` lines (the tree writes no `action
/// started` line, so each step is one `action finished` line).
fn steps_of(lines: &[RunLogLine], action: &str) -> usize {
    lines
        .iter()
        .filter(|line| {
            matches!(&line.event, RunLogEvent::ActionFinished(finished) if finished.action == action)
        })
        .count()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_outcome_waits_for_the_spent_claim() {
    let tree = Tree::open(
        "runs-spent-give-up",
        GIVE_UP_PROTOCOL,
        GIVE_UP_BINDINGS,
        "exit 1\n",
    );
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Terminal, "{run:?}\n{lines:#?}");
    assert_eq!(run.detail.as_deref(), Some("given_up"), "{run:?}");
    assert_eq!(steps_of(&lines, "work.do"), 2, "{lines:#?}");
}

/// The planted defect of [`an_outcome_waits_for_the_spent_claim`]: without the `from: runs`
/// entry nothing says the writer is spent, and the run is stuck after its second `work.do`.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn without_the_runs_entry_the_case_is_stuck() {
    let tree = Tree::open(
        "runs-spent-no-entry",
        GIVE_UP_PROTOCOL,
        &give_up_bindings_without_runs(),
        "exit 1\n",
    );
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Stuck, "{run:?}\n{lines:#?}");
    let performed = lines
        .iter()
        .filter(|line| {
            matches!(&line.event, RunLogEvent::ActionFinished(finished)
                if finished.action == "work.do" && finished.refusal.is_none())
        })
        .count();
    assert_eq!(performed, 2, "{lines:#?}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_spent_kind_does_not_make_the_writer_useful() {
    let tree = Tree::open("runs-spent-idle", IDLE_PROTOCOL, IDLE_BINDINGS, "exit 0\n");
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Stuck, "{run:?}\n{lines:#?}");
    assert_eq!(steps_of(&lines, "work.do"), 1, "{lines:#?}");
    assert_eq!(steps_of(&lines, "note.write"), 1, "{lines:#?}");
}
