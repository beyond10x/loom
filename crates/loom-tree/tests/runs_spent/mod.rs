//! A spent run allowance read in an outcome's `requires` (`story:outcome-when-reads-the-run-count`):
//! a writer's `from: runs` entry writes `spent` on the run that reaches `budget.max_runs`, a claim
//! over that kind ends the case through the outcome that requires it, and the kind alone never
//! makes the writer useful again. Each tree is one `halt@1`-shaped root under the `local` runner,
//! with `sh` stubs; no model, no network.
//!
//! A decision conditioned on that claim (`story:outcome-decision-when-reads-the-run-count`):
//! `given_up` requires `{decision: give_up, when: {claim: work.spent}}`, and the root's Canon
//! decision document, read through a governor over the tree's store as `tree close` opens one,
//! lists `give_up` as pending only once the spent record arrived.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_tree::model::{TreeRefusalReason, TreeRun, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings, TreeTime,
    UNCONFINED_FLAG_REASON, open_tree, read_protocols_directory, run_tree,
};
use loom_governor::{CanonGovernor, FileCaseStore};
use serde_json::Value;

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
        Self::open_in(&super::scratch(label), protocol, bindings, program)
    }

    /// [`Tree::open`] in the scratch directory `dir`.
    fn open_in(dir: &std::path::Path, protocol: &str, bindings: &str, program: &str) -> Self {
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
        self.run_steps(None)
    }

    /// The root's governor over the tree's store, opened as `tree close` opens one.
    fn governor(&self) -> CanonGovernor<FileCaseStore> {
        CanonGovernor::new(FileCaseStore::open(self.store.clone()).unwrap())
            .with_catalog(&self.catalog)
            .unwrap()
    }

    /// The root's Canon decision document now, as plain JSON.
    fn document(&self) -> Value {
        let governor = self.governor();
        let root = CaseId("root".into());
        let frontier = governor.frontier(&root).expect("the root's frontier");
        let document = governor
            .decision_document(&root, &frontier.data().frontier_id)
            .expect("the root's decision document")
            .expect("the document the frontier was issued from");
        plain(&document)
    }

    /// One `run_tree` under `local` taking at most `max_steps` steps.
    fn run_steps(&self, max_steps: Option<i64>) -> (TreeRun, Vec<RunLogLine>) {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        let settings = RunSettings {
            max_steps,
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

/// [`GIVE_UP_PROTOCOL`] with `given_up` requiring the decision `give_up` under `when`.
fn decision_protocol(when: &str) -> String {
    let plain = "  given_up:\n    requires: {claim: work.spent}\n";
    assert!(GIVE_UP_PROTOCOL.contains(plain));
    GIVE_UP_PROTOCOL.replace(
        plain,
        &format!("  given_up:\n    requires: {{decision: give_up, when: {when}}}\n"),
    )
}

/// The `when` the decision tests read: the spent claim.
const SPENT_CLAIM: &str = "{claim: work.spent}";

/// `loom::json::Value` as `serde_json::Value`, members in the order they arrived.
fn plain(value: &loom::json::Value) -> Value {
    use loom::json::Value as Loom;
    match value {
        Loom::Null => Value::Null,
        Loom::Bool(value) => Value::Bool(*value),
        Loom::Number(spelling) => serde_json::from_str(spelling).unwrap(),
        Loom::Text(text) => Value::String(text.clone()),
        Loom::Array(items) => Value::Array(items.iter().map(plain).collect()),
        Loom::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// The steps of `action` the run performed: its `action finished` lines without a refusal. A
/// spent writer with a retry record is offered once more and refused `RunsSpent`, which is no run.
fn performed(lines: &[RunLogLine], action: &str) -> usize {
    lines
        .iter()
        .filter(|line| {
            matches!(&line.event, RunLogEvent::ActionFinished(finished)
                if finished.action == action && finished.refusal.is_none())
        })
        .count()
}

/// The decisions the document lists under `pending_decisions`, in its order.
fn pending(document: &Value) -> Vec<String> {
    document["pending_decisions"]["decisions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| entry["decision"].as_str().unwrap().to_owned())
        .collect()
}

/// `given_up` blocked by the spent claim, which no record decides yet.
fn blocked_by_the_spent_claim() -> Value {
    serde_json::json!({
        "reasons": [{"claim": "work.spent", "value": "unknown"}],
        "status": "blocked",
    })
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_when_reads_the_spent_claim() {
    let tree = Tree::open(
        "decision-when",
        &decision_protocol(SPENT_CLAIM),
        GIVE_UP_BINDINGS,
        "exit 1\n",
    );

    let (first, lines) = tree.run_steps(Some(1));
    assert_eq!(steps_of(&lines, "work.do"), 1, "{first:?}\n{lines:#?}");
    let before = tree.document();
    assert_eq!(
        before["claims"]["work.spent"]["value"], "unknown",
        "{before:#}"
    );
    assert!(pending(&before).is_empty(), "{before:#}");
    assert_eq!(
        before["outcomes"]["given_up"],
        blocked_by_the_spent_claim(),
        "{before:#}"
    );

    let (second, lines) = tree.run();
    assert_eq!(performed(&lines, "work.do"), 1, "{second:?}\n{lines:#?}");
    assert_ne!(second.stop, TreeStop::Terminal, "{second:?}");
    let after = tree.document();
    assert_eq!(after["claims"]["work.spent"]["value"], "true", "{after:#}");
    assert_eq!(pending(&after), ["give_up"], "{after:#}");
    assert_eq!(
        after["outcomes"]["given_up"],
        serde_json::json!({
            "reasons": [{"decision": "give_up", "present": false}],
            "status": "blocked",
        }),
        "{after:#}"
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_early_decision_counts_once_the_writer_is_spent() {
    let tree = Tree::open(
        "decision-early",
        &decision_protocol(SPENT_CLAIM),
        GIVE_UP_BINDINGS,
        "exit 1\n",
    );
    let root = CaseId("root".into());

    let (first, lines) = tree.run_steps(Some(1));
    assert_eq!(steps_of(&lines, "work.do"), 1, "{first:?}\n{lines:#?}");
    let revision = tree.governor().case_state(&root).unwrap().revision;
    tree.governor()
        .decide(&root, "given_up", "give_up", "operator")
        .expect("a decision is neither counted nor refused while its when is not true");
    let early = tree.document();
    assert!(pending(&early).is_empty(), "{early:#}");
    assert_eq!(
        early["outcomes"]["given_up"],
        blocked_by_the_spent_claim(),
        "{early:#}"
    );

    let (second, lines) = tree.run();
    assert_eq!(performed(&lines, "work.do"), 1, "{second:?}\n{lines:#?}");
    assert_eq!(second.stop, TreeStop::Terminal, "{second:?}\n{lines:#?}");
    assert_eq!(second.detail.as_deref(), Some("given_up"), "{second:?}");
    let state = tree.governor().case_state(&root).unwrap();
    assert_eq!(state.revision, revision, "no step moved the case revision");
    assert_eq!(state.decisions.len(), 1, "{:?}", state.decisions);
    assert_eq!(state.decisions[0].decision, "give_up");
    assert_eq!(state.decisions[0].case_revision, format!("r{revision}"));
    assert_eq!(
        state.termination.map(|termination| termination.outcome),
        Some("given_up".to_owned())
    );
}

// The record an allowance spent for good owes (`story:spent-program-has-its-spent-record`): the
// protocol of `an_outcome_waits_for_the_spent_claim`, `work.do` bound with no `retry_when` and an
// exit table with no `*` row, so a program that exits 3 is refused `ResultNotInTable`.

/// `work.do` with `budget.max_runs` `max_runs`, the extra members `members` (lines indented four
/// spaces), the exit table `{"0": ok}` and the fixture's `from: runs` entry.
fn refused_bindings(max_runs: i64, members: &str) -> String {
    format!(
        r#"format: loom.bindings/1
protocol: halt@1
artifacts:
  work: work.txt
actions:
  - action: work.do
    executor: {{kind: program, argv: [sh, do.sh]}}
    budget: {{max_runs: {max_runs}}}
{members}    evidence:
      - {{kind: done, subject: work, producer: doer, from: exit, results: {{"0": ok}}}}
      - {{kind: spent, subject: work, producer: loom, from: runs, results: {{spent: spent}}}}
"#
    )
}

/// The program of these trees: its exit is in no row of `work.do`'s exit table.
const EXITS_3: &str = "exit 3\n";

/// The ids of the root's `spent` records.
fn spent_ids(tree: &Tree) -> Vec<String> {
    tree.governor()
        .evidence(&CaseId("root".into()))
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == "spent")
        .map(|record| record.evidence_id.0.0)
        .collect()
}

/// The `action finished` lines of `work.do` that name a record with exit 0 and no refusal.
fn spent_lines(lines: &[RunLogLine]) -> Vec<&b10x_loom_tree::runlog::ActionFinished> {
    lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished)
                if finished.action == "work.do"
                    && finished.exit == Some(0)
                    && finished.refusal.is_none()
                    && !finished.evidence.is_empty() =>
            {
                Some(finished)
            }
            _ => None,
        })
        .collect()
}

/// The position of the `action finished` line of `work.do` refused `ResultNotInTable`.
fn refused_line(lines: &[RunLogLine]) -> usize {
    lines
        .iter()
        .position(|line| {
            matches!(&line.event, RunLogEvent::ActionFinished(finished)
                if finished.action == "work.do"
                    && finished.refusal.as_deref().is_some_and(|refusal| refusal.starts_with("ResultNotInTable")))
        })
        .unwrap_or_else(|| panic!("no refused run: {lines:#?}"))
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_refused_last_run_ends_given_up() {
    let tree = Tree::open(
        "spent-refused-last",
        GIVE_UP_PROTOCOL,
        &refused_bindings(1, ""),
        EXITS_3,
    );
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Terminal, "{run:?}\n{lines:#?}");
    assert_eq!(run.detail.as_deref(), Some("given_up"), "{run:?}");
    let spent = spent_ids(&tree);
    assert_eq!(spent.len(), 1, "{spent:?}");
    let refused = refused_line(&lines);
    let RunLogEvent::ActionFinished(written) = &lines[refused + 1].event else {
        panic!("no action finished line after the refused run: {lines:#?}");
    };
    assert_eq!(written.action, "work.do");
    assert_eq!(written.exit, Some(0));
    assert_eq!(written.refusal, None);
    assert_eq!(written.blob_digest, None);
    assert_eq!(written.evidence, spent);
    assert!(written.revisions.is_empty(), "{written:?}");
    assert!(
        matches!(lines[refused + 2].event, RunLogEvent::Evaluated(_)),
        "{lines:#?}"
    );
    let replayed = b10x_loom_tree::replay_tree(&tree.store, &tree.catalog, None).unwrap();
    assert_eq!(
        b10x_loom_tree::replay_difference(&replayed),
        None,
        "{replayed:?}"
    );
}

/// A store 0.60.0 left stuck: the refused last run and its line, then a stop before the next
/// step. The next run writes the record before its first evaluation.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_store_left_stuck_by_0_60_ends_given_up() {
    let tree = Tree::open(
        "spent-left-stuck",
        GIVE_UP_PROTOCOL,
        &refused_bindings(1, ""),
        EXITS_3,
    );
    let (first, lines) = tree.run_steps(Some(1));
    assert_eq!(first.stop, TreeStop::BudgetSpent, "{first:?}\n{lines:#?}");
    refused_line(&lines);
    assert!(spent_ids(&tree).is_empty());

    let (second, lines) = tree.run();
    assert_eq!(second.stop, TreeStop::Terminal, "{second:?}\n{lines:#?}");
    assert_eq!(second.detail.as_deref(), Some("given_up"), "{second:?}");
    assert_eq!(spent_ids(&tree).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_spent_record_is_written_once_per_case() {
    let outcome = "  given_up:\n    requires: {claim: work.spent}\n";
    assert!(GIVE_UP_PROTOCOL.contains(outcome));
    let tree = Tree::open(
        "spent-once",
        &GIVE_UP_PROTOCOL.replace(outcome, ""),
        &refused_bindings(1, ""),
        EXITS_3,
    );
    let (first, mut lines) = tree.run_steps(Some(5));
    assert_ne!(first.stop, TreeStop::Terminal, "{first:?}\n{lines:#?}");
    let (_, again) = tree.run_steps(Some(5));
    lines.extend(again);
    assert_eq!(spent_lines(&lines).len(), 1, "{lines:#?}");
    assert_eq!(spent_ids(&tree).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn no_handoff_check_runs_for_the_spent_record() {
    let dir = super::scratch("spent-no-handoff-check");
    let marks = dir.join("marks");
    std::fs::create_dir_all(&marks).unwrap();
    let members = format!(
        "    handoff: a person finishes the work\n    handoff_when: [sh, check.sh, {}]\n",
        marks.display()
    );
    let tree = Tree::open_in(
        &dir,
        GIVE_UP_PROTOCOL,
        &refused_bindings(1, &members),
        EXITS_3,
    );
    // Hands the proposal on (exit 1), so the writer runs in that step.
    std::fs::write(
        dir.join("workspace/check.sh"),
        "echo checked >> \"$1/checks\"\nexit 1\n",
    )
    .unwrap();
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Terminal, "{run:?}\n{lines:#?}");
    assert_eq!(run.detail.as_deref(), Some("given_up"), "{run:?}");
    let checks = std::fs::read_to_string(marks.join("checks")).unwrap_or_default();
    assert_eq!(checks.lines().count(), 1, "{checks:?}");
    let checked: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| matches!(line.event, RunLogEvent::HandoffChecked(_)))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(checked.len(), 1, "{lines:#?}");
    assert!(checked[0] < refused_line(&lines), "{lines:#?}");
}

/// Today's behaviour below the allowance: a refused run writes no record, and the writer is not
/// offered again before the case changes.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_refused_run_below_the_allowance_stays_set_aside() {
    let tree = Tree::open(
        "spent-below-allowance",
        GIVE_UP_PROTOCOL,
        &refused_bindings(3, ""),
        EXITS_3,
    );
    let (run, lines) = tree.run();
    assert_eq!(run.stop, TreeStop::Stuck, "{run:?}\n{lines:#?}");
    assert_eq!(steps_of(&lines, "work.do"), 1, "{lines:#?}");
    refused_line(&lines);
    assert!(spent_ids(&tree).is_empty());
    assert!(spent_lines(&lines).is_empty(), "{lines:#?}");
}

#[test]
fn an_evidence_form_in_when_is_refused() {
    let protocols = super::scratch("decision-when-evidence").join("protocols");
    std::fs::create_dir_all(&protocols).unwrap();
    std::fs::write(
        protocols.join("halt.yaml"),
        decision_protocol("{evidence: {kind: spent, subject: work, result: spent}}"),
    )
    .unwrap();
    let Err(refused) = read_protocols_directory(&protocols) else {
        panic!("an evidence form in when is registered");
    };
    assert_eq!(
        refused.reason(),
        TreeRefusalReason::ProtocolRefused,
        "{refused}"
    );
    assert!(
        refused
            .to_string()
            .contains("evidence-in-decision-condition"),
        "{refused}"
    );
}
