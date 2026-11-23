//! A fresh run allowance after a rejection (`story:rework-allowance-after-a-rejection`): a writer
//! bound with `budget.runs_per: rejection` counts its `max_runs` from the latest rejection in its
//! case, `budget.rework_limit` bounds the rejections it answers, and the refusal that ends it
//! names the bound. A rejection is a record whatever its principal: a decider's, a rule's or a
//! person's approval with `result: rejected`.
//!
//! Each tree is one root under the `local` runner opened in process, a fixed `--at` instant and
//! `max_steps: 10`; the writer is an `sh` stub that appends a line to the artifact, and every
//! approval is `approve` in process, as `tree approve` takes it. No model, no network.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_tree::model::{ApprovalDecision, DecisionKind, TreeRun, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    ApproveRequest, ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings,
    TreeTime, UNCONFINED_FLAG_REASON, approve, open_tree, read_protocols_directory, run_tree,
};
use loom::runlog::ActionFinished;
use loom_governor::{CanonGovernor, FileCaseStore};

/// The instant every command of these trees records.
const AT: &str = "2026-11-22T08:00:00Z";

/// `change.implement` writes `change` and records `implemented`; `change.review` is a person's
/// review of it. `accepted` needs an accepting review; with `given_up`, the spent writer ends the
/// case too.
fn protocol(given_up: bool) -> String {
    let outcome = if given_up {
        "  given_up:\n    requires: {claim: change.implement_spent}\n"
    } else {
        ""
    };
    format!(
        "format: protocol/1
protocol:
  id: rework
  revision: 1
artifacts:
  change: {{}}
evidence_kinds:
  implemented: {{}}
  review: {{}}
  implement_spent: {{}}
claims:
  change.implemented:
    true_when:
      evidence: {{kind: implemented, subject: change, result: done}}
  change.accepted:
    true_when:
      evidence: {{kind: review, subject: change, result: accepted}}
  change.implement_spent:
    true_when:
      evidence: {{kind: implement_spent, subject: change, result: spent}}
actions:
  change.implement:
    effect: write
    may_produce:
      - evidence: implemented
      - evidence: implement_spent
  change.review:
    effect: write
    precondition: {{claim: change.implemented}}
    may_produce:
      - evidence: review
outcomes:
  accepted:
    requires: {{claim: change.accepted}}
{outcome}"
    )
}

/// The writer under `budget`, sent back by a rejected review, with the `from: runs` entry when
/// `spent`; the review taken by `principal`.
fn bindings(principal: &str, budget: &str, spent: bool) -> String {
    let spent = if spent {
        "      - {kind: implement_spent, subject: change, producer: loom, from: runs, results: {spent: spent}}\n"
    } else {
        ""
    };
    format!(
        "format: loom.bindings/1
protocol: rework@1
artifacts:
  change: change.txt
actions:
  - action: change.implement
    executor: {{kind: program, argv: [sh, implement.sh]}}
    retry_when: {{review: [rejected]}}
    budget: {budget}
    evidence:
      - {{kind: implemented, subject: change, producer: implementer, from: exit, results: {{\"0\": done, \"*\": failed}}}}
{spent}  - action: change.review
    executor: {{kind: person, principal: \"{principal}\"}}
    evidence:
      - {{kind: review, subject: change, producer: \"{principal}\", from: effect, results: {{accepted: accepted, rejected: rejected}}}}
"
    )
}

/// Runbook's kinds (`bindings/sdc-story.yaml`): the writer records `implementation_change`, a
/// decider approves the merge; `conformance_run` and `code_review` are declared, and no action of
/// the tree writes them.
const RUNBOOK_PROTOCOL: &str = "format: protocol/1
protocol:
  id: rework
  revision: 1
artifacts:
  change: {}
evidence_kinds:
  implementation_change: {}
  conformance_run: {}
  code_review: {}
  merge_approval: {}
claims:
  change.merged:
    true_when:
      evidence: {kind: merge_approval, subject: change, result: accepted}
actions:
  change.implement:
    effect: write
    may_produce:
      - evidence: implementation_change
  change.merge:
    effect: write
    may_produce:
      - evidence: merge_approval
outcomes:
  merged:
    requires: {claim: change.merged}
";

/// Runbook's `change.implement` members and budget, and a merge a decider takes.
const RUNBOOK_BINDINGS: &str = r#"format: loom.bindings/1
protocol: rework@1
artifacts:
  change: change.txt
actions:
  - action: change.implement
    executor: {kind: program, argv: [sh, implement.sh]}
    retry_when:
      implementation_change: [coverage_refused]
      conformance_run: [failed]
      code_review: [rejected]
      merge_approval: [rejected]
    budget: {max_runs: 3, runs_per: rejection, rework_limit: 3, rework_on: {code_review: [rejected], merge_approval: [rejected]}}
    evidence:
      - {kind: implementation_change, subject: change, producer: implementer, from: exit, results: {"0": null, "8": coverage_refused}}
  - action: change.merge
    executor: {kind: person, principal: "decider:x"}
    evidence:
      - {kind: merge_approval, subject: change, producer: "decider:x", from: effect, results: {accepted: accepted, rejected: rejected}}
"#;

/// The writer's program: one line more in the artifact, exit 0.
const IMPLEMENT: &str = "echo implemented >> change.txt\nexit 0\n";

/// A tree of one root opened from `protocol` and `bindings`.
struct Tree {
    store: PathBuf,
    catalog: loom_protocols::ProtocolCatalog,
}

impl Tree {
    fn open(label: &str, protocol: &str, bindings: &str) -> Self {
        let dir = super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(protocols.join("rework.yaml"), protocol).unwrap();
        std::fs::write(workspace.join("change.txt"), "change\n").unwrap();
        std::fs::write(workspace.join("implement.sh"), IMPLEMENT).unwrap();
        let file = dir.join("rework.bindings.yaml");
        std::fs::write(&file, bindings).unwrap();
        let directory = read_protocols_directory(&protocols).unwrap();
        let store = dir.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace,
            protocol: "rework@1".into(),
            bindings: file,
            revisions: Vec::new(),
            case: "root".into(),
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

    fn runner() -> std::sync::Arc<EngineRunner> {
        std::sync::Arc::new(
            EngineRunner::open(&EngineConfinement {
                adapter: ConfinementAdapter::Local,
                local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
                cgroup_root: None,
            })
            .unwrap(),
        )
    }

    /// One `run_tree` of at most 10 steps: the run and the lines it wrote, in the order written.
    fn round(&self) -> (TreeRun, Vec<RunLogLine>) {
        let settings = RunSettings {
            max_steps: Some(10),
            max_wall_seconds: None,
            time: TreeTime::At(AT.into()),
            runner: Self::runner(),
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

    /// Asserts `run` waits on `principal`'s `action`, and takes it with `result: rejected`.
    fn reject(&self, run: &TreeRun, action: &str, principal: &str) {
        assert_waits_on(run, action, principal);
        let decision = &run.decisions[0];
        approve(&ApproveRequest {
            store: &self.store,
            case: &decision.case,
            capability: decision.capability.as_deref(),
            principal,
            decision: ApprovalDecision::Allow,
            action: Some(action),
            result: Some("rejected"),
            reasons: None,
            time: TreeTime::At(AT.into()),
            catalog: &self.catalog,
            runner: Some(Self::runner()),
        })
        .unwrap_or_else(|refused| panic!("the rejection is taken: {refused}"));
    }

    /// The records of `kind` the root holds.
    fn records(&self, kind: &str) -> usize {
        CanonGovernor::new(FileCaseStore::open(self.store.clone()).unwrap())
            .with_catalog(&self.catalog)
            .unwrap()
            .evidence(&CaseId("root".into()))
            .unwrap()
            .iter()
            .filter(|held| held.kind == kind)
            .count()
    }
}

/// Asserts `run` stopped `AwaitingPerson` with one decision: `principal` taking `action`.
fn assert_waits_on(run: &TreeRun, action: &str, principal: &str) {
    assert_eq!(run.stop, TreeStop::AwaitingPerson, "{run:?}");
    assert_eq!(run.decisions.len(), 1, "{run:?}");
    let decision = &run.decisions[0];
    assert_eq!(decision.kind, DecisionKind::ApproveAction, "{run:?}");
    assert_eq!(decision.action.as_deref(), Some(action), "{run:?}");
    assert_eq!(decision.principal.as_deref(), Some(principal), "{run:?}");
}

/// The `action finished` lines of `action` in `lines`.
fn finished<'a>(lines: &'a [RunLogLine], action: &str) -> Vec<&'a ActionFinished> {
    lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) if finished.action == action => Some(finished),
            _ => None,
        })
        .collect()
}

/// Rounds of `tree` with a rejection of the person action `action` by `principal` after each:
/// `rejections` of them. Each round runs the writer once and stops waiting on the review; the
/// writer's lines of every round, in order, and the last round, which waits on the next review.
fn rejected(
    tree: &Tree,
    action: &str,
    principal: &str,
    rejections: usize,
) -> (Vec<ActionFinished>, TreeRun) {
    let mut writer = Vec::new();
    for _ in 0..rejections {
        let (run, lines) = tree.round();
        writer.extend(finished(&lines, "change.implement").into_iter().cloned());
        tree.reject(&run, action, principal);
    }
    let (run, lines) = tree.round();
    writer.extend(finished(&lines, "change.implement").into_iter().cloned());
    (writer, run)
}

/// Asserts each of `lines` is a run that exited 0 and was not refused.
fn assert_ran(lines: &[ActionFinished]) {
    for line in lines {
        assert_eq!(
            (line.exit, line.refusal.as_deref()),
            (Some(0), None),
            "{lines:#?}"
        );
    }
}

/// The round after the last rejection: one refused step naming `budget.rework_limit is <limit>`
/// and `principal`, then the stop `Stuck`.
fn assert_rework_spent(tree: &Tree, principal: &str, limit: i64) {
    let (run, lines) = tree.round();
    let writer = finished(&lines, "change.implement");
    assert_eq!(writer.len(), 1, "{lines:#?}");
    let refusal = writer[0].refusal.as_deref().expect("the step is refused");
    assert!(
        refusal.starts_with("RunsSpent: action change.implement:"),
        "{refusal}"
    );
    assert!(
        refusal.contains(&format!("budget.rework_limit is {limit}")),
        "{refusal}"
    );
    assert!(refusal.contains(principal), "{refusal}");
    assert_eq!(writer[0].exit, None, "{lines:#?}");
    assert_eq!(run.stop, TreeStop::Stuck, "{run:?}\n{lines:#?}");
}

/// Tests 1 to 3: three rejections by `principal` and a fourth run, then a fourth rejection ends
/// the writer.
fn a_rejection_gives_a_fresh_allowance(label: &str, principal: &str) {
    let tree = Tree::open(
        label,
        &protocol(false),
        &bindings(
            principal,
            "{max_runs: 1, runs_per: rejection, rework_limit: 3}",
            false,
        ),
    );
    let (writer, run) = rejected(&tree, "change.review", principal, 3);
    assert_eq!(writer.len(), 4, "{writer:#?}");
    assert_ran(&writer);
    tree.reject(&run, "change.review", principal);
    assert_rework_spent(&tree, principal, 3);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_decider_rejection_gives_a_fresh_allowance() {
    a_rejection_gives_a_fresh_allowance("rework-decider", "decider:x");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_rule_rejection_gives_a_fresh_allowance() {
    a_rejection_gives_a_fresh_allowance("rework-rule", "rule:y");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_person_rejection_gives_a_fresh_allowance() {
    a_rejection_gives_a_fresh_allowance("rework-person", "ada");
}

/// Runbook's budget: `max_runs: 3, runs_per: rejection, rework_limit: 3`.
const RUNBOOK_BUDGET: &str = "{max_runs: 3, runs_per: rejection, rework_limit: 3}";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn runbooks_shape_writes_spent_on_the_last_renewal() {
    let tree = Tree::open(
        "rework-spent",
        &protocol(false),
        &bindings("decider:x", RUNBOOK_BUDGET, true),
    );
    // Runs 1 to 3, after rejections 0 to 2: the rework count stays below 3.
    for rejection in 0..3 {
        let (run, lines) = tree.round();
        assert_eq!(finished(&lines, "change.implement").len(), 1, "{lines:#?}");
        assert_eq!(tree.records("implement_spent"), 0, "run {}", rejection + 1);
        tree.reject(&run, "change.review", "decider:x");
    }
    // Run 4, after the third rejection: one run in the window, the rework count at 3.
    let (run, lines) = tree.round();
    assert_ran(
        &finished(&lines, "change.implement")
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
    );
    assert_eq!(tree.records("implement_spent"), 1);
    tree.reject(&run, "change.review", "decider:x");
    assert_rework_spent(&tree, "decider:x", 3);
    assert_eq!(tree.records("implement_spent"), 1);

    // An outcome over the spent claim ends the case right after run 4, with no fourth review.
    let tree = Tree::open(
        "rework-spent-given-up",
        &protocol(true),
        &bindings("decider:x", RUNBOOK_BUDGET, true),
    );
    let (writer, run) = rejected(&tree, "change.review", "decider:x", 3);
    assert_eq!(writer.len(), 4, "{writer:#?}");
    assert_ran(&writer);
    assert_eq!(run.stop, TreeStop::Terminal, "{run:?}");
    assert_eq!(run.detail.as_deref(), Some("given_up"), "{run:?}");
    assert_eq!(tree.records("implement_spent"), 1);
}

/// The `runs_spent` give-up tree (`work.do` sent back by its own `done: failed`), counting per
/// rejection.
const OWN_PROTOCOL: &str = "format: protocol/1
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

const OWN_BINDINGS: &str = r#"format: loom.bindings/1
protocol: halt@1
artifacts:
  work: work.txt
actions:
  - action: work.do
    executor: {kind: program, argv: [sh, do.sh]}
    budget: {max_runs: 2, runs_per: rejection}
    retry_when: {done: [failed]}
    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
      - {kind: spent, subject: work, producer: loom, from: runs, results: {spent: spent}}
"#;

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn own_records_alone_open_no_window() {
    let dir = super::scratch("rework-own");
    let protocols = dir.join("protocols");
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&protocols).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(protocols.join("halt.yaml"), OWN_PROTOCOL).unwrap();
    std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
    std::fs::write(workspace.join("do.sh"), "exit 1\n").unwrap();
    let file = dir.join("halt.bindings.yaml");
    std::fs::write(&file, OWN_BINDINGS).unwrap();
    let directory = read_protocols_directory(&protocols).unwrap();
    let store = dir.join("store");
    open_tree(&OpenRequest {
        store: store.clone(),
        workspace,
        protocol: "halt@1".into(),
        bindings: file,
        revisions: Vec::new(),
        case: "root".into(),
        time: TreeTime::At(AT.into()),
        catalog: &directory.catalog,
        protocols: directory.kept,
    })
    .expect("the bindings load");
    let tree = Tree {
        store,
        catalog: directory.catalog,
    };
    let (run, lines) = tree.round();
    let writer = finished(&lines, "work.do");
    assert_eq!(writer.len(), 3, "{lines:#?}");
    for line in &writer[..2] {
        assert_eq!(
            (line.exit, line.refusal.as_deref()),
            (Some(1), None),
            "{lines:#?}"
        );
    }
    let refusal = writer[2]
        .refusal
        .as_deref()
        .expect("the third step is refused");
    assert!(
        refusal.starts_with("RunsSpent: action work.do:"),
        "{refusal}"
    );
    assert!(refusal.contains("budget.max_runs is 2"), "{refusal}");
    assert_eq!(tree.records("spent"), 0);
    assert_eq!(run.stop, TreeStop::Stuck, "{run:?}\n{lines:#?}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn runbooks_merge_approval_opens_a_window() {
    let tree = Tree::open("rework-runbook", RUNBOOK_PROTOCOL, RUNBOOK_BINDINGS);
    let (writer, run) = rejected(&tree, "change.merge", "decider:x", 3);
    assert_eq!(writer.len(), 4, "{writer:#?}");
    assert_ran(&writer);
    tree.reject(&run, "change.merge", "decider:x");
    assert_rework_spent(&tree, "decider:x", 3);
}

// Adversary (wave 7, unit rw): Runbook's two suite entries (`runbook/cycles/tree-suite.yaml`
// `decider-rework-spent` and `rework-fresh-allowance`) driven on Runbook's `change.implement`
// shape with `rework_limit: 2`, the bound those two entries hold (the coordinator, correction round of
// unit rw): the third rejection ends the writer, and two rejections leave it its fourth run.

/// [`RUNBOOK_BINDINGS`] with `rework_limit: 2`.
fn runbook_bindings_limit_two() -> String {
    assert!(RUNBOOK_BINDINGS.contains("rework_limit: 3"));
    RUNBOOK_BINDINGS.replace("rework_limit: 3", "rework_limit: 2")
}

/// [`Tree::open`] with `implement` as the writer's program.
fn open_with(label: &str, protocol: &str, bindings: &str, implement: &str) -> Tree {
    let dir = super::scratch(label);
    let protocols = dir.join("protocols");
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&protocols).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(protocols.join("rework.yaml"), protocol).unwrap();
    std::fs::write(workspace.join("change.txt"), "change\n").unwrap();
    std::fs::write(workspace.join("implement.sh"), implement).unwrap();
    let file = dir.join("rework.bindings.yaml");
    std::fs::write(&file, bindings).unwrap();
    let directory = read_protocols_directory(&protocols).unwrap();
    let store = dir.join("store");
    open_tree(&OpenRequest {
        store: store.clone(),
        workspace,
        protocol: "rework@1".into(),
        bindings: file,
        revisions: Vec::new(),
        case: "root".into(),
        time: TreeTime::At(AT.into()),
        catalog: &directory.catalog,
        protocols: directory.kept,
    })
    .unwrap();
    Tree {
        store,
        catalog: directory.catalog,
    }
}

/// The results of the root's records of `kind`, in the order held.
fn results(tree: &Tree, kind: &str) -> Vec<String> {
    CanonGovernor::new(FileCaseStore::open(tree.store.clone()).unwrap())
        .with_catalog(&tree.catalog)
        .unwrap()
        .evidence(&CaseId("root".into()))
        .unwrap()
        .iter()
        .filter(|held| held.kind == kind)
        .map(|held| match &held.facts {
            b10x_loom_commission::model::json::Value::Object(members) => members
                .iter()
                .find(|(name, _)| name == "result")
                .map(|(_, value)| format!("{value:?}"))
                .unwrap_or_default(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// Runbook `decider-rework-spent`: a decider rejects the merge every time it is asked; the run
/// ends `Stuck` (Runbook exit 4) with three `merge_approval: rejected` records.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn adv_runbook_decider_rework_spent_after_three_rejections() {
    let tree = Tree::open(
        "adv-decider-rework-spent",
        RUNBOOK_PROTOCOL,
        &runbook_bindings_limit_two(),
    );
    let (writer, run) = rejected(&tree, "change.merge", "decider:x", 2);
    assert_eq!(writer.len(), 3, "{writer:#?}");
    tree.reject(&run, "change.merge", "decider:x");
    assert_eq!(results(&tree, "merge_approval").len(), 3);
    // The suite's expectation: after the third rejection the run stops, the writer refused
    // RunsSpent on its rework bound, without a fourth run and a fourth review.
    assert_rework_spent(&tree, "decider:x", 2);
}

/// Runbook `rework-fresh-allowance`: four `change.implement` runs, the merge rejected, rejected,
/// approved; the case ends `merged`. The first run is sent back by its own `coverage_refused`
/// (exit 8, D12: counted inside the window, opens none).
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn adv_runbook_rework_fresh_allowance() {
    let implement = "if [ -f ../first-done ]; then echo implemented >> change.txt; exit 0; fi\n\
                     : > ../first-done\nexit 8\n";
    let tree = open_with(
        "adv-rework-fresh",
        RUNBOOK_PROTOCOL,
        &runbook_bindings_limit_two(),
        implement,
    );
    let (writer, run) = rejected(&tree, "change.merge", "decider:x", 2);
    let exits: Vec<Option<i64>> = writer.iter().map(|line| line.exit).collect();
    assert_eq!(exits, [Some(8), Some(0), Some(0), Some(0)], "{writer:#?}");
    assert!(
        writer.iter().all(|line| line.refusal.is_none()),
        "{writer:#?}"
    );
    assert_waits_on(&run, "change.merge", "decider:x");
    let decision = &run.decisions[0];
    approve(&ApproveRequest {
        store: &tree.store,
        case: &decision.case,
        capability: decision.capability.as_deref(),
        principal: "decider:x",
        decision: ApprovalDecision::Allow,
        action: Some("change.merge"),
        result: Some("accepted"),
        reasons: None,
        time: TreeTime::At(AT.into()),
        catalog: &tree.catalog,
        runner: Some(Tree::runner()),
    })
    .unwrap_or_else(|refused| panic!("the approval is taken: {refused}"));
    let (run, lines) = tree.round();
    assert!(
        finished(&lines, "change.implement").is_empty(),
        "{lines:#?}"
    );
    assert_eq!(run.stop, TreeStop::Terminal, "{run:?}\n{lines:#?}");
    assert_eq!(run.detail.as_deref(), Some("merged"), "{run:?}");
    assert_eq!(results(&tree, "merge_approval").len(), 3);
}
