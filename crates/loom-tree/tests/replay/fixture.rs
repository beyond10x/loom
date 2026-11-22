//! The trees `tree replay` is tested on (`story:tree-replay-command`), made once per test run.
//!
//! The two-level tree of `crates/loom-cli/tests/fixtures/tree-two-level/` (`tour@1` and its legs,
//! `tour-leg@1`) is opened from its protocols directory, so every store keeps its catalog, and run
//! through the library under the `local` runner, each printed decision taken in process, until its
//! root ends. Three stores of that one run are kept: `opened` (opened, never run), `wait` (the wait
//! where one leg opened by `legs.open` is closed and its sibling open) and `ended` (every case
//! closed). Beside them, for the moves the log names: `moved` (the tree stopped by `max_steps`
//! right after the first leg's `route.write`), `refused` (a leg alone whose `route.write` ends
//! refused after its move) and `person` (an `answer@1` case whose person program moved its
//! artifact and exited 1).
//!
//! Each set is made once for every test process under `CARGO_TARGET_TMPDIR`, holding a lock file,
//! keyed by the SHA-256 of the fixture's files, this file and the test binary's size and time; a
//! test copies the store it reads into a directory of its own. Every write syncs; no test reaches
//! the network, a model or a credential. This file is shared by `b10x-loom-tree`'s module `replay`
//! and `b10x-loom-cli`'s module `tree_replay` (`#[path]`).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Timestamp as CommissionTimestamp;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
};
use b10x_loom_commission::ports::evidence::{ObservationPort as _, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_runlog::RunLog;
use b10x_loom_tree::model::{ApprovalDecision, DecisionKind, NextDecision, TreeStop};
use b10x_loom_tree::runlog::{RunLogEvent, RunLogLine};
use b10x_loom_tree::{
    ApproveRequest, ConfinementAdapter, EngineConfinement, EngineRunner, OpenRequest, Refused,
    RunSettings, TreeTime, UNCONFINED_FLAG_REASON, approve, log_file_name, open_tree, read_log,
    read_protocols_directory, run_tree, stored_catalog,
};
use loom_governor::{CanonGovernor, FileCaseStore};
use loom_protocols::ProtocolCatalog;
use sha2::{Digest as _, Sha256};

/// The fixture directory.
pub const TWO_LEVEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../loom-cli/tests/fixtures/tree-two-level"
);

/// The instant every command of the fixture records.
pub const AT: &str = "2026-11-22T08:00:00Z";

/// The cases of the two-level tree: the root, its first leg and the two legs `legs.open` opens
/// (`stubs/legs.sh`).
pub const ROOT: &str = "root";
pub const FIRST: &str = "root.first";
pub const NORTH: &str = "root.north";
pub const SOUTH: &str = "root.south";

/// The settings of a run through the library: the `local` runner, the instant [`AT`], `max_steps`.
pub fn settings(max_steps: Option<i64>) -> RunSettings {
    RunSettings {
        max_steps,
        max_wall_seconds: Some(50),
        time: TreeTime::At(AT.to_owned()),
        runner: runner(),
        confinement: Some("local".to_owned()),
        cgroup_root: None,
    }
}

fn runner() -> Arc<dyn b10x_loom_tree::TestRunner> {
    Arc::new(
        EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .unwrap(),
    )
}

/// A fresh directory of the test's own under `CARGO_TARGET_TMPDIR`.
pub fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "tree-replay-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Copies the directory `from` to `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// Every file below `dir` and its bytes, by path, in path order.
pub fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, found: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), found);
            } else {
                found.push((entry.path(), fs::read(entry.path()).unwrap()));
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, &mut found);
    found.sort();
    found
}

/// The catalog the store at `store` keeps: the protocols directory it was opened from.
pub fn catalog(store: &Path) -> ProtocolCatalog {
    stored_catalog(store).unwrap().expect("a kept catalog")
}

/// The lines of `case`'s run log in `store`.
pub fn lines(store: &Path, case: &str) -> Vec<RunLogLine> {
    read_log(&fs::read(store.join(log_file_name(case))).unwrap()).unwrap()
}

/// The position in `lines` of the last `evaluated` line.
pub fn last_evaluated(lines: &[RunLogLine]) -> usize {
    lines
        .iter()
        .rposition(|line| matches!(line.event, RunLogEvent::Evaluated(_)))
        .expect("an evaluated line")
}

/// The governor over the case files of `store`.
pub fn governor(store: &Path) -> CanonGovernor<FileCaseStore> {
    CanonGovernor::new(FileCaseStore::open(store.to_owned()).unwrap())
        .with_catalog(&catalog(store))
        .unwrap()
}

/// A directory made once for every test process by `build`, keyed by `label`, the fixture's
/// files, this file and the test binary: built in place holding `<label>.lock`, finished once its
/// `made` file is there.
fn made_once(label: &str, build: impl FnOnce(&Path)) -> PathBuf {
    let mut hasher = Sha256::new();
    let mut files = snapshot(Path::new(TWO_LEVEL));
    files.push((
        PathBuf::from("fixture.rs"),
        include_bytes!("fixture.rs").to_vec(),
    ));
    for (path, bytes) in files {
        hasher.update(path.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    let binary = fs::metadata(std::env::current_exe().unwrap()).unwrap();
    hasher.update(binary.len().to_be_bytes());
    let built = binary
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    hasher.update(built.as_nanos().to_be_bytes());
    let key: String = hasher.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let root = base.join(format!("tree-replay-{label}-{key}"));
    let lock = fs::File::create(base.join(format!("tree-replay-{label}-{key}.lock"))).unwrap();
    lock.lock().unwrap();
    if !root.join("made").exists() {
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        build(&root);
        fs::write(root.join("made"), "").unwrap();
    }
    lock.unlock().unwrap();
    root
}

/// A workspace at `dir` holding the fixture's stubs and bindings files, and the catalog and kept
/// protocols of its protocols directory.
fn workspace(dir: &Path) -> b10x_loom_tree::ProtocolDirectory {
    let fixture = Path::new(TWO_LEVEL);
    fs::create_dir_all(dir.join("stubs")).unwrap();
    for entry in fs::read_dir(fixture.join("stubs")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), dir.join("stubs").join(entry.file_name())).unwrap();
    }
    for entry in fs::read_dir(fixture).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
        }
    }
    read_protocols_directory(&fixture.join("protocols")).unwrap()
}

/// Opens a tree of `protocol` with the bindings file `bindings` of `workspace` into `store`.
fn open(
    store: &Path,
    workspace: &Path,
    protocol: &str,
    bindings: &str,
    directory: b10x_loom_tree::ProtocolDirectory,
) {
    open_tree(&OpenRequest {
        store: store.to_owned(),
        workspace: workspace.to_owned(),
        protocol: protocol.to_owned(),
        bindings: workspace.join(bindings),
        revisions: Vec::new(),
        case: ROOT.to_owned(),
        time: TreeTime::At(AT.to_owned()),
        catalog: &directory.catalog,
        protocols: directory.kept,
    })
    .unwrap();
}

/// Takes `decision`, the first a waiting run printed, as its command would: `tree approve`.
fn decide(store: &Path, decision: &NextDecision) {
    let action = match decision.kind {
        DecisionKind::ApproveAction => decision.action.as_deref(),
        DecisionKind::Approve => None,
        DecisionKind::Close | DecisionKind::Handoff => {
            panic!(
                "the two-level tree waits on no {:?}: {decision:?}",
                decision.kind
            )
        }
    };
    approve(&ApproveRequest {
        store,
        case: &decision.case,
        capability: decision.capability.as_deref(),
        principal: decision.principal.as_deref().unwrap(),
        decision: ApprovalDecision::Allow,
        action,
        result: None,
        reasons: None,
        time: TreeTime::At(AT.to_owned()),
        catalog: &catalog(store),
        runner: Some(runner()),
    })
    .unwrap();
}

/// Whether the log of `case` in `store` holds a `child closed` line after its last `evaluated`
/// line.
fn closed_after_evaluation(store: &Path, case: &str) -> bool {
    let lines = lines(store, case);
    let last = last_evaluated(&lines);
    lines[last + 1..]
        .iter()
        .any(|line| matches!(line.event, RunLogEvent::ChildClosed(_)))
}

/// The stores of the two-level tree: opened, at the wait with one leg closed and its sibling open,
/// and ended.
pub struct Trees {
    root: PathBuf,
}

impl Trees {
    /// The three stores, made once for every test process.
    pub fn made() -> Self {
        let root = made_once("main", |root| {
            let work = root.join("workspace");
            let directory = workspace(&work);
            let store = root.join("run");
            open(&store, &work, "tour@1", "tour.bindings.yaml", directory);
            copy_dir(&store, &root.join("opened"));
            let mut waited = false;
            for _ in 0..12 {
                let ran = run_tree(&store, &catalog(&store), &settings(None), None, None).unwrap();
                if ran.stop == TreeStop::Terminal {
                    assert!(waited, "the tree ended without the wait");
                    copy_dir(&store, &root.join("ended"));
                    return;
                }
                assert!(
                    matches!(
                        ran.stop,
                        TreeStop::AwaitingPerson | TreeStop::AwaitingAuthority
                    ),
                    "{ran:?}"
                );
                if !waited && closed_after_evaluation(&store, ROOT) {
                    copy_dir(&store, &root.join("wait"));
                    waited = true;
                }
                decide(&store, &ran.decisions[0]);
            }
            panic!("the two-level tree did not end within 12 runs");
        });
        Self { root }
    }

    /// A copy, in a directory of the test's own, of the store `which` (`opened`, `wait` or
    /// `ended`).
    pub fn copy(&self, which: &str, label: &str) -> PathBuf {
        let store = scratch(label).join("store");
        copy_dir(&self.root.join(which), &store);
        store
    }
}

/// The stores whose logs name the moves of a step: `moved`, `refused` and `person`.
pub struct Moves {
    root: PathBuf,
}

/// The bindings of an `answer@1` case whose person action `answer.free` runs a program that
/// moves the subject and exits 1.
const ANSWER_BINDINGS: &str = "format: loom.bindings/1
protocol: answer@1
artifacts:
  subject: answer/subject.txt
actions:
  - action: answer.free
    executor:
      kind: person
      principal: owner
      argv: [sh, write-and-fail.sh]
    evidence:
      - kind: answer_freed
        subject: subject
        producer: owner
        from: effect
        results: {performed: freed}
";

impl Moves {
    /// The three stores, made once for every test process.
    pub fn made() -> Self {
        let root = made_once("moves", |root| {
            // The two-level tree, stopped right after the first leg's `route.write`: the root's
            // `first.open`, the leg's `route.check`, then its `route.write`.
            let work = root.join("moved-workspace");
            let directory = workspace(&work);
            let store = root.join("moved");
            open(&store, &work, "tour@1", "tour.bindings.yaml", directory);
            let ran = run_tree(&store, &catalog(&store), &settings(Some(3)), None, None).unwrap();
            assert_eq!(ran.stop, TreeStop::BudgetSpent, "{ran:?}");

            // The leg alone, its `route.write` refused after its move: its `route.check`, then
            // its `route.write`.
            let work = root.join("refused-workspace");
            let directory = workspace(&work);
            let store = root.join("refused");
            open(
                &store,
                &work,
                "tour-leg@1",
                "tour-leg-refused-write.bindings.yaml",
                directory,
            );
            let ran = run_tree(&store, &catalog(&store), &settings(Some(2)), None, None).unwrap();
            assert_eq!(ran.stop, TreeStop::BudgetSpent, "{ran:?}");

            // An `answer@1` case whose person program moved its subject and exited 1.
            let work = root.join("person-workspace");
            fs::create_dir_all(work.join("answer")).unwrap();
            fs::write(work.join("answer/subject.txt"), "question\n").unwrap();
            fs::copy(
                Path::new(TWO_LEVEL).join("extra/write-and-fail.sh"),
                work.join("write-and-fail.sh"),
            )
            .unwrap();
            fs::write(work.join("answer.bindings.yaml"), ANSWER_BINDINGS).unwrap();
            let directory = read_protocols_directory(&Path::new(TWO_LEVEL).join("extra")).unwrap();
            let store = root.join("person");
            open(&store, &work, "answer@1", "answer.bindings.yaml", directory);
            let refused = approve(&ApproveRequest {
                store: &store,
                case: ROOT,
                capability: None,
                principal: "owner",
                decision: ApprovalDecision::Allow,
                action: Some("answer.free"),
                result: None,
                reasons: None,
                time: TreeTime::At(AT.to_owned()),
                catalog: &catalog(&store),
                runner: Some(runner()),
            })
            .unwrap_err();
            assert_eq!(
                format!("{:?}", refused.reason()),
                "PersonProgramFailed",
                "{refused}"
            );
        });
        Self { root }
    }

    /// A copy, in a directory of the test's own, of the store `which` (`moved`, `refused` or
    /// `person`).
    pub fn copy(&self, which: &str, label: &str) -> PathBuf {
        let store = scratch(label).join("store");
        copy_dir(&self.root.join(which), &store);
        store
    }
}

/// Moves `artifact` of `case` in `store` to a revision of its own through the governor's
/// `update_revision`, writing no log line; the new case revision.
pub fn move_outside_the_log(store: &Path, case: &str, artifact: &str) -> i64 {
    governor(store)
        .update_revision(&CaseId(case.to_owned()), artifact, "moved-outside-the-log")
        .unwrap()
}

/// Receives into `case` in `store`, through the governor's `receive`, a `canon-evidence/1` record
/// approving its route at the route's revision, writing no log line: the record the open
/// obligation of a leg waiting on its guide reads.
pub fn receive_outside_the_log(store: &Path, case: &str) {
    let governor = governor(store);
    let id = CaseId(case.to_owned());
    let observation = ObservationId(commission_json_uuid("observation"));
    governor
        .observe(Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "test:replay".into(),
            subject: "route".into(),
            observed_at: CommissionTimestamp(AT.into()),
            payload: commission_json::Value::Null,
        }))
        .unwrap();
    let revision = governor.revisions(&id).unwrap()["route"].clone();
    submit_evidence(
        &governor,
        "guide",
        EvidenceData {
            evidence_id: EvidenceId(commission_json_uuid("evidence")),
            case_id: id.clone(),
            kind: "route_approval".into(),
            subject_revision: governor.current_revision(&id).unwrap(),
            producer: "guide".into(),
            observation_ids: vec![observation],
            facts: commission_json::parse(
                &serde_json::json!({
                    "format": "canon-evidence/1",
                    "id": "route-approval-outside-the-log",
                    "kind": "route_approval",
                    "subject": "route",
                    "subject_revision": revision,
                    "result": "approved",
                })
                .to_string(),
            )
            .unwrap(),
            provenance: commission_json::Value::Null,
        },
    )
    .unwrap();
}

/// A UUID of the fixture's, named by `what`.
fn commission_json_uuid(what: &str) -> b10x_loom_commission::model::primitives::Uuid {
    b10x_loom_tree::derived_uuid(&["replay", what])
}

/// Appends to `parent`'s log in `store`, through `RunLog::append`, a copy of the event of its
/// last `evaluated` line, then a copy of the event of its `child closed` line of `child`: a log
/// whose last `evaluated` line comes after that child's `child opened` line, which `tree run`
/// never writes. The `seq` of the appended `child closed` line.
pub fn append_a_late_child_closed(store: &Path, parent: &str, child: &str) -> i64 {
    let before = lines(store, parent);
    let evaluated = before[last_evaluated(&before)].event.clone();
    let closed = before
        .iter()
        .find_map(|line| match &line.event {
            RunLogEvent::ChildClosed(closed) if closed.child == child => Some(line.event.clone()),
            _ => None,
        })
        .expect("a child closed line of the child");
    let log = RunLog::open(store).unwrap();
    let at = before[0].at.clone();
    log.append(parent, at.clone(), evaluated).unwrap();
    log.append(parent, at, closed).unwrap();
    let after = lines(store, parent);
    assert!(matches!(
        after.last().unwrap().event,
        RunLogEvent::ChildClosed(_)
    ));
    after.last().unwrap().seq
}

/// Whether `refused` is the refusal `reason`.
pub fn refused_as(refused: &Refused, reason: &str) -> bool {
    format!("{:?}", refused.reason()) == reason
}
