//! Acceptance for `story:writer-not-called-leaves-the-due-set`: a writer whose program exits a code
//! its binding lists in `exits.not_called` leaves the due set until the case is evaluated again
//! for a reason other than its own moves, so `tree run` starts it once per evaluation and goes on
//! to the next useful action or to the handoff check that asks the decision for the waiting
//! finding.
//!
//! Each tree is one `nudge@1` root opened and run in process through `run_tree` under Substrate's
//! `local` adapter, `max_steps: Some(10)`, its programs POSIX sh. `work.look` is a reader that
//! records `looked: failed`, which opens `work.do`'s retry record; `work.do` touches
//! `target/called` and exits 79; `work.ask` is never useful (it may produce `looked`, which
//! `work.look` records first) and hands off when `target/called` exists, a check that runs only
//! when nothing is useful. No network, no model, no credential.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Timestamp as CommissionTimestamp;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort as _, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_tree::model::{DecisionKind, TreeRun, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, RunSettings, TreeTime,
    UNCONFINED_FLAG_REASON, open_tree, read_protocols_directory, run_tree,
};
use loom_protocols::ProtocolCatalog;

/// The instant every run records.
const AT: &str = "2026-11-22T08:00:00Z";

/// The protocol; `{tidy}` is replaced by the declaration of `work.tidy` or by nothing.
pub const PROTOCOL: &str = "format: protocol/1
protocol:
  id: nudge
  revision: 1
artifacts:
  work: {}
  notes: {}
evidence_kinds:
  looked: {}
  done: {}
  tidied: {}
claims:
  work.looked:
    true_when:
      evidence: {kind: looked, subject: work, result: ok}
  work.done:
    true_when:
      evidence: {kind: done, subject: work, result: ok}
  notes.tidied:
    true_when:
      evidence: {kind: tidied, subject: notes, result: ok}
actions:
  work.ask:
    effect: write
    may_produce:
      - evidence: looked
  work.look:
    effect: read
    may_produce:
      - evidence: looked
  work.do:
    effect: write
    may_produce:
      - evidence: done
{tidy}outcomes:
  finished:
    requires: {claim: work.done}
";

/// `work.tidy`: a writer of `tidied` on `notes` that moves nothing.
const TIDY_ACTION: &str = "  work.tidy:
    effect: write
    may_produce:
      - evidence: tidied
";

/// The bindings; `{exits}` is replaced by the `exits` member of `work.do`'s binding (indented four
/// spaces, ended) or by nothing, `{tidy}` by the binding of `work.tidy` or by nothing.
pub const BINDINGS: &str = r#"format: loom.bindings/1
protocol: nudge@1
artifacts:
  work: work.txt
  notes: notes.txt
actions:
  - action: work.ask
    executor: {kind: program, argv: [sh, stubs/ask.sh]}
    handoff: a person answers the finding
    handoff_when: [sh, stubs/check.sh]
    evidence:
      - {kind: looked, subject: work, producer: asker, from: exit, results: {"0": ok, "*": failed}}
  - action: work.look
    executor: {kind: program, argv: [sh, stubs/look.sh]}
    evidence:
      - {kind: looked, subject: work, producer: looker, from: exit, results: {"0": ok, "*": failed}}
  - action: work.do
    executor: {kind: program, argv: [sh, stubs/do.sh]}
    timeout_seconds: 60
    retry_when: {looked: [failed]}
{exits}    evidence:
      - {kind: done, subject: work, producer: doer, from: exit, results: {"0": ok, "*": failed}}
{tidy}"#;

/// The binding of `work.tidy`.
const TIDY_BINDING: &str = r#"  - action: work.tidy
    executor: {kind: program, argv: [sh, stubs/tidy.sh]}
    evidence:
      - {kind: tidied, subject: notes, producer: tidier, from: exit, results: {"0": ok, "*": failed}}
"#;

/// `work.do`'s `exits` member listing 79 as not called.
pub const NOT_CALLED: &str = "    exits: {not_called: [79]}\n";

/// `work.do`'s program: it touches `target/called`, outside every placed artifact, and exits 79.
pub const DO: &str = "mkdir -p target\ntouch target/called\nexit 79\n";

/// `work.ask`'s check: it hands off only once `work.do` ran.
pub const CHECK: &str = "test -e target/called\n";

/// The stubs every tree holds, `do.sh` aside: `(file, text)`.
pub const STUBS: [(&str, &str); 4] = [
    ("look.sh", "exit 1\n"),
    ("check.sh", CHECK),
    ("ask.sh", "exit 0\n"),
    ("tidy.sh", "exit 0\n"),
];

/// The protocol with `work.tidy` declared or not.
pub fn protocol(tidy: bool) -> String {
    PROTOCOL.replace("{tidy}", if tidy { TIDY_ACTION } else { "" })
}

/// The bindings with `work.do`'s `exits` member `exits` and `work.tidy` bound or not.
pub fn bindings(exits: &str, tidy: bool) -> String {
    BINDINGS
        .replace("{exits}", exits)
        .replace("{tidy}", if tidy { TIDY_BINDING } else { "" })
}

struct Tree {
    store: PathBuf,
    workspace: PathBuf,
    catalog: ProtocolCatalog,
}

impl Tree {
    /// A one-case `nudge@1` tree whose `do.sh` is `program`, `work.do`'s `exits` member `exits`,
    /// with `work.tidy` or without.
    fn open(label: &str, program: &str, exits: &str, tidy: bool) -> Self {
        let dir = super::scratch(label);
        let protocols = dir.join("protocols");
        let workspace = dir.join("workspace");
        std::fs::create_dir_all(&protocols).unwrap();
        std::fs::create_dir_all(workspace.join("stubs")).unwrap();
        std::fs::write(protocols.join("nudge.yaml"), protocol(tidy)).unwrap();
        std::fs::write(workspace.join("work.txt"), "work\n").unwrap();
        std::fs::write(workspace.join("notes.txt"), "notes\n").unwrap();
        for (file, text) in STUBS {
            std::fs::write(workspace.join("stubs").join(file), text).unwrap();
        }
        std::fs::write(workspace.join("stubs/do.sh"), program).unwrap();
        let bindings_file = dir.join("nudge.bindings.yaml");
        std::fs::write(&bindings_file, bindings(exits, tidy)).unwrap();
        let directory = read_protocols_directory(&protocols).unwrap();
        let store = dir.join("store");
        open_tree(&OpenRequest {
            store: store.clone(),
            workspace: workspace.clone(),
            protocol: "nudge@1".into(),
            bindings: bindings_file,
            revisions: Vec::new(),
            case: "root".into(),
            time: TreeTime::At(AT.into()),
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

    /// One `tree run` of at most 10 steps.
    fn run(&self) -> TreeRun {
        let runner = EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap();
        let settings = RunSettings {
            max_steps: Some(10),
            max_wall_seconds: Some(50),
            time: TreeTime::At(AT.into()),
            runner: std::sync::Arc::new(runner),
            confinement: None,
            cgroup_root: None,
        };
        run_tree(&self.store, &self.catalog, &settings, None, None).expect("the tree runs")
    }

    /// The root's log.
    fn lines(&self) -> Vec<RunLogLine> {
        super::replay::fixture::lines(&self.store, "root")
    }

    /// Receives a `tidied: ok` record on `notes` through the governor's `receive`, writing no log
    /// line: a person's answer that moves no artifact.
    fn receive_tidied(&self) {
        let governor = super::replay::fixture::governor(&self.store);
        let id = CaseId("root".to_owned());
        let observation =
            ObservationId(b10x_loom_tree::derived_uuid(&["not-called", "observation"]));
        governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: "test:not-called".into(),
                subject: "notes".into(),
                observed_at: CommissionTimestamp(AT.into()),
                payload: commission_json::Value::Null,
            }))
            .unwrap();
        let revision = governor.revisions(&id).unwrap()["notes"].clone();
        submit_evidence(
            &governor,
            "tidier",
            EvidenceData {
                evidence_id: EvidenceId(b10x_loom_tree::derived_uuid(&["not-called", "evidence"])),
                case_id: id.clone(),
                kind: "tidied".into(),
                subject_revision: governor.current_revision(&id).unwrap(),
                producer: "tidier".into(),
                observation_ids: vec![observation],
                facts: commission_json::parse(
                    &serde_json::json!({
                        "format": "canon-evidence/1",
                        "id": "tidied-outside-the-log",
                        "kind": "tidied",
                        "subject": "notes",
                        "subject_revision": revision,
                        "result": "ok",
                    })
                    .to_string(),
                )
                .unwrap(),
                provenance: commission_json::Value::Null,
            },
        )
        .unwrap();
    }
}

/// The `action finished` and `handoff checked` lines of `lines`, one text each:
/// `finished <action> exit <code>` or `finished <action> refused`, then ` moved <artifacts>` when
/// the line names moves; `checked <action> exit <code>`.
fn steps(lines: &[RunLogLine]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) => {
                let end = match (finished.exit, &finished.refusal) {
                    (_, Some(_)) => "refused".to_owned(),
                    (Some(exit), None) => format!("exit {exit}"),
                    (None, None) => "no exit".to_owned(),
                };
                let moved: Vec<&str> = finished
                    .revisions
                    .iter()
                    .map(|moved| moved.artifact.as_str())
                    .collect();
                let moved = if moved.is_empty() {
                    String::new()
                } else {
                    format!(" moved {}", moved.join(","))
                };
                Some(format!("finished {} {end}{moved}", finished.action))
            }
            RunLogEvent::HandoffChecked(checked) => Some(format!(
                "checked {} exit {}",
                checked.action,
                checked
                    .exit
                    .map_or("none".to_owned(), |exit| exit.to_string())
            )),
            _ => None,
        })
        .collect()
}

/// The case revisions of the `evaluated` lines of `lines`, in order.
fn evaluated(lines: &[RunLogLine]) -> Vec<i64> {
    lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::Evaluated(evaluated) => Some(evaluated.case_revision),
            _ => None,
        })
        .collect()
}

/// `run` stopped `AwaitingPerson` with one `Handoff` decision, for `work.ask`.
fn handed_off(run: &TreeRun) {
    assert_eq!(run.stop, TreeStop::AwaitingPerson, "{run:?}");
    assert_eq!(run.decisions.len(), 1, "{run:?}");
    assert_eq!(run.decisions[0].kind, DecisionKind::Handoff, "{run:?}");
    assert_eq!(
        run.decisions[0].action.as_deref(),
        Some("work.ask"),
        "{run:?}"
    );
}

fn texts(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|text| (*text).to_owned()).collect()
}

/// The lines of test 1's run: `work.look`, one not-called `work.do`, `work.ask`'s check.
const ONCE: [&str; 3] = [
    "finished work.look exit 1",
    "finished work.do exit 79",
    "checked work.ask exit 0",
];

/// Test 1 (Runbook's item a): with its retry record open, a not-called writer starts once, and the
/// run reaches the handoff check, which hands off.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_not_called_writer_with_an_open_retry_record_hands_off_and_starts_once() {
    let tree = Tree::open("nc-once", DO, NOT_CALLED, false);
    let run = tree.run();
    let lines = tree.lines();
    assert_eq!(steps(&lines), texts(&ONCE), "{run:?}");
    handed_off(&run);
    let finished = lines
        .iter()
        .find_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) if finished.action == "work.do" => {
                Some(finished.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (finished.exit, finished.refusal.as_deref()),
        (Some(79), None)
    );
    assert!(finished.evidence.is_empty(), "{finished:?}");
}

/// Test 2 (Runbook's item a, the other branch): after a not-called exit another useful action is
/// taken; its record changes the decision, so the writer is due again once.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn another_useful_action_is_taken_after_a_not_called_exit() {
    let tree = Tree::open("nc-tidy", DO, NOT_CALLED, true);
    let run = tree.run();
    assert_eq!(
        steps(&tree.lines()),
        texts(&[
            "finished work.look exit 1",
            "finished work.do exit 79",
            "finished work.tidy exit 0",
            "finished work.do exit 79",
            "checked work.ask exit 0",
        ]),
        "{run:?}"
    );
    handed_off(&run);
}

/// Test 3 (D1 a, D2 a): after test 1's run, a moved `work`, a moved `notes` or a record received
/// each make the writer due again, and it starts once more.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_not_called_writer_is_due_again_after_a_change() {
    type Change = fn(&Tree);
    let changes: [(&str, Change, &[&str]); 3] = [
        (
            "work-moved",
            |tree| std::fs::write(tree.workspace.join("work.txt"), "changed\n").unwrap(),
            &ONCE,
        ),
        (
            "notes-moved",
            |tree| std::fs::write(tree.workspace.join("notes.txt"), "changed\n").unwrap(),
            &ONCE[1..],
        ),
        ("received", Tree::receive_tidied, &ONCE[1..]),
    ];
    for (label, change, expected) in changes {
        let tree = Tree::open(&format!("nc-again-{label}"), DO, NOT_CALLED, false);
        let first = tree.run();
        assert_eq!(steps(&tree.lines()), texts(&ONCE), "{label}: {first:?}");
        handed_off(&first);
        let before = tree.lines().len();
        let revision = *evaluated(&tree.lines()).last().unwrap();
        change(&tree);
        let second = tree.run();
        let lines = tree.lines();
        assert_eq!(
            steps(&lines[before..]),
            texts(expected),
            "{label}: {second:?}"
        );
        handed_off(&second);
        let again = evaluated(&lines[before..]);
        if label == "received" {
            assert_eq!(again.first(), Some(&revision), "{label}: {again:?}");
        } else {
            assert_eq!(again.first(), Some(&(revision + 1)), "{label}: {again:?}");
        }
    }
}

/// Test 4 (D4 a): a not-called run that moves a placed artifact stays set aside across the
/// `evaluated` line its own move gives; a person's answer received after it ends the set-aside,
/// since the interval that answer's `evaluated` line closes names no move.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_not_called_run_that_moves_its_artifact_stays_set_aside() {
    let moving = format!("echo more >> notes.txt\n{DO}");
    let tree = Tree::open("nc-moves", &moving, NOT_CALLED, false);
    let first = tree.run();
    let lines = tree.lines();
    assert_eq!(
        steps(&lines),
        texts(&[
            "finished work.look exit 1",
            "finished work.do exit 79 moved notes",
            "checked work.ask exit 0",
        ]),
        "{first:?}"
    );
    handed_off(&first);
    let revision = *evaluated(&lines).last().unwrap();
    let before = lines.len();
    tree.receive_tidied();
    let second = tree.run();
    let lines = tree.lines();
    assert_eq!(
        steps(&lines[before..]),
        texts(&[
            "finished work.do exit 79 moved notes",
            "checked work.ask exit 0",
        ]),
        "{second:?}"
    );
    assert_eq!(
        evaluated(&lines[before..]),
        [revision, revision + 1],
        "{second:?}"
    );
    handed_off(&second);
}

/// Test 5: an exit in `exits.error` and an exit in no list behave as before this story.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn classed_errors_and_unclassed_exits_behave_as_before() {
    let error = Tree::open("nc-error", DO, "    exits: {error: [79]}\n", false);
    let run = error.run();
    let lines = error.lines();
    assert_eq!(
        steps(&lines),
        texts(&[
            "finished work.look exit 1",
            "finished work.do refused",
            "checked work.ask exit 0",
        ]),
        "{run:?}"
    );
    handed_off(&run);
    let refusal = lines
        .iter()
        .find_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) if finished.action == "work.do" => {
                Some((finished.exit, finished.refusal.clone().unwrap_or_default()))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(refusal.0, None, "{refusal:?}");
    assert!(
        refusal.1.starts_with("ProgramError: action work.do:") && refusal.1.contains("79"),
        "{refusal:?}"
    );

    // With no `exits`, exit 79 goes through the table and records `done: failed`, and each run
    // counts against `budget.max_runs` (8 when the binding names none): the 9th start is refused
    // `RunsSpent`, and the 10th step spends the run's budget.
    let unclassed = Tree::open("nc-unclassed", DO, "", false);
    let run = unclassed.run();
    let lines = unclassed.lines();
    let mut expected = vec!["finished work.look exit 1"];
    expected.extend(["finished work.do exit 79"; 8]);
    expected.push("finished work.do refused");
    assert_eq!(steps(&lines), texts(&expected), "{run:?}");
    assert_eq!(run.stop, TreeStop::BudgetSpent, "{run:?}");
    let done: Vec<_> = lines
        .iter()
        .filter_map(|line| match &line.event {
            RunLogEvent::ActionFinished(finished) if finished.action == "work.do" => Some(finished),
            _ => None,
        })
        .collect();
    for finished in &done[..8] {
        assert_eq!(finished.evidence.len(), 1, "{finished:?}");
    }
    assert!(
        done[8]
            .refusal
            .as_deref()
            .is_some_and(|refusal| refusal.starts_with("RunsSpent: ")),
        "{:?}",
        done[8]
    );
}

/// The workspace stubs and files a suite seeds, written under `dir`: `stubs/`, `work.txt` and
/// `notes.txt`.
pub fn write_seed(dir: &Path) {
    std::fs::create_dir_all(dir.join("stubs")).unwrap();
    for (file, text) in STUBS {
        std::fs::write(dir.join("stubs").join(file), text).unwrap();
    }
    std::fs::write(dir.join("stubs/do.sh"), DO).unwrap();
    std::fs::write(dir.join("work.txt"), "work\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "notes\n").unwrap();
}
