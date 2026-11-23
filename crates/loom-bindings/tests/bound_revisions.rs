//! Acceptance for `story:bound-artifact-revisions`: after a bound action ran, each artifact the
//! bindings place at a path takes the content digest of what is there as its revision, written
//! with the step's records in one store write, and computed again before every evaluation.
//!
//! A step reaches the program effect port through Commission's own loop (`run_until_blocked`), a
//! scripted executor proposing the action, and a `CanonGovernor` over a `MemoryCaseStore`, with a
//! fixture protocol of two artifacts and `/bin/sh` stub programs under the `local` runner opened
//! with the flag's reason; no model, no network. The crash at the store write is
//! `crates/loom-governor/tests/step_write.rs`. Scratch goes under `CARGO_TARGET_TMPDIR`.

use std::collections::BTreeMap;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use b10x_loom_bindings::{
    ABSENT, MAX_ARTIFACT_BYTES, PlacedRevisions, ProgramEffects, check, content_revision, decode,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ObservationId, PrincipalId, ProposedActionArguments, RunId,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, EngineConfinement, EngineRunner, TestRunner,
};
use loom::bindings::{BindingsDocument, RevisionRefusalReason};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};
use serde_json::json;

const REGISTRATION: &str = "fixture@1";
const CASE: &str = "case-1";
/// An action that writes.
const CHECK: &str = "spec.check";
/// An action that reads.
const LOOK: &str = "spec.look";
/// An action with no effect class.
const SIGN: &str = "impl.sign";

const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: fixture
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  gate: {}
actions:
  spec.check:
    effect: write
    may_produce:
      - evidence: gate
  spec.look:
    effect: read
    may_produce:
      - evidence: gate
  impl.sign:
    may_produce:
      - evidence: gate
";

/// The revision of a file holding `abc`: `t` and the first 16 hex digits of its SHA-256.
const ABC: &str = "tba7816bf8f01cfea";

// ---------------------------------------------------------------------------------------------
// Fixture

struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("bound-revisions-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let root = dir.path().to_owned();
        let workspace = root.join("workspace");
        let store = root.join("store");
        std::fs::create_dir_all(workspace.join("docs")).unwrap();
        std::fs::create_dir(&store).unwrap();
        Self {
            _dir: dir,
            root,
            workspace,
            store,
        }
    }

    /// Writes `bytes` at `path` in the workspace, its directories made.
    fn put(&self, path: &str, bytes: &[u8]) {
        let path = self.workspace.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn script(&self, name: &str, body: &str) -> String {
        let path = self.root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn revision(&self, path: &str) -> String {
        content_revision(&self.workspace, path).expect("the path is digested")
    }
}

/// The document binding `action` to `argv`, recording one `gate` record about `spec` that follows
/// `impl`, `valid` on exit 0; `spec` is placed at `docs/spec`, `impl` at `src`.
fn document(action: &str, argv: &[&str]) -> BindingsDocument {
    let argv = serde_json::to_string(argv).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  spec: docs/spec\n  impl: src\nactions:\n  - action: {action}\n    executor: {{kind: program, argv: {argv}}}\n    evidence:\n      - {{kind: gate, subject: spec, producer: ess, from: exit, results: {{\"0\": valid, \"*\": invalid}}, upstream: [impl]}}\n"
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(
        &document,
        REGISTRATION,
        &b10x_canon::model::parse(PROTOCOL).unwrap(),
    )
    .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// The governor holding the case, opened with the revision of what the workspace holds now.
fn governor(scratch: &Scratch) -> CanonGovernor<MemoryCaseStore> {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers");
    governor
        .open_case(
            case(),
            REGISTRATION,
            [
                ("spec".to_owned(), scratch.revision("docs/spec")),
                ("impl".to_owned(), scratch.revision("src")),
            ]
            .into(),
            CaseInputs::none(),
        )
        .expect("the case opens");
    governor
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn flag_runner() -> Arc<dyn TestRunner> {
    Arc::new(
        EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .expect("local opens with the flag's reason"),
    )
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn uuid() -> Uuid {
    Uuid(format!(
        "00000000-0000-4000-8000-{:012x}",
        ISSUED.fetch_add(1, Ordering::Relaxed)
    ))
}

struct Context;

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(uuid())
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(uuid())
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// Runs one step in which the executor proposes `action`, bound to a script holding `body`.
fn step(
    scratch: &Scratch,
    governor: &CanonGovernor<MemoryCaseStore>,
    action: &str,
    body: &str,
) -> LoopEnd {
    let program = scratch.script("program.sh", body);
    let document = document(action, &[&program]);
    let effects = ProgramEffects::new(
        governor,
        case(),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(flag_runner()),
    )
    .unwrap();
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid()),
        agent_revision_id: AgentRevisionId(uuid()),
        case_id: case(),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: action.to_owned(),
            arguments: ProposedActionArguments(json::parse("{}").unwrap()),
        },
    )]);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid())));
    run_until_blocked(
        governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &mut Context,
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn plain(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Number(spelling) => serde_json::from_str(spelling).unwrap(),
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Array(items) => serde_json::Value::Array(items.iter().map(plain).collect()),
        Value::Object(members) => serde_json::Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), plain(member)))
                .collect(),
        ),
    }
}

/// The report of the one performed effect of `end`.
fn report(end: &LoopEnd) -> serde_json::Value {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    match &end.effects[0] {
        EffectOutcome::Performed(performed) => plain(&performed.report),
        other => panic!("not performed: {other:?}"),
    }
}

fn refusal(end: &LoopEnd) -> String {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    match &end.effects[0] {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

fn records(governor: &CanonGovernor<MemoryCaseStore>) -> Vec<EvidenceData> {
    governor.evidence(&case()).unwrap()
}

fn revisions(governor: &CanonGovernor<MemoryCaseStore>) -> BTreeMap<String, String> {
    governor.revisions(&case()).unwrap()
}

/// The kinds of `action` no record counts at the case's current revision.
fn open_evidence(governor: &CanonGovernor<MemoryCaseStore>, action: &str) -> Vec<String> {
    governor
        .action_facts(&case())
        .unwrap()
        .actions
        .into_iter()
        .find(|facts| facts.action == action)
        .unwrap()
        .open_evidence
}

// ---------------------------------------------------------------------------------------------
// The digest

#[test]
fn a_placed_file_holding_abc_has_its_digest_and_an_absent_path_has_none() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"abc");
    assert_eq!(scratch.revision("docs/spec"), ABC);
    assert_eq!(scratch.revision("docs/absent"), ABSENT);
    assert_eq!(scratch.revision("absent/below"), ABSENT);
    // A file where a directory of the path belongs leaves nothing at the path.
    assert_eq!(scratch.revision("docs/spec/below"), ABSENT);
}

/// Writes the files of `entries` under `tree`, in the order given.
fn tree(scratch: &Scratch, tree: &str, entries: &[(&str, &[u8])]) {
    for (path, bytes) in entries {
        scratch.put(&format!("{tree}/{path}"), bytes);
    }
}

#[test]
fn a_tree_digest_does_not_depend_on_listing_order_and_one_byte_or_name_changes_it() {
    let scratch = Scratch::new();
    let entries: Vec<(String, Vec<u8>)> = (0..40)
        .map(|n| {
            (
                format!("d{}/f{n:02}", n % 3),
                format!("body {n}").into_bytes(),
            )
        })
        .collect();
    let forward: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    let mut backward = forward.clone();
    backward.reverse();
    tree(&scratch, "a", &forward);
    tree(&scratch, "b", &backward);
    assert_eq!(scratch.revision("a"), scratch.revision("b"));

    let mut one_byte = forward.clone();
    one_byte[7].1 = b"body 8";
    tree(&scratch, "c", &one_byte);
    assert_ne!(scratch.revision("c"), scratch.revision("a"));

    let mut one_name = forward.clone();
    one_name[7].0 = "d1/g07";
    tree(&scratch, "d", &one_name);
    assert_ne!(scratch.revision("d"), scratch.revision("a"));

    // An empty directory is an entry too.
    std::fs::create_dir(scratch.workspace.join("a/empty")).unwrap();
    assert_ne!(scratch.revision("a"), scratch.revision("b"));
    // A file's bytes moved to its neighbour's name keep no digest.
    let swapped = scratch.workspace.join("b");
    std::fs::write(swapped.join("d0/f00"), b"body 3").unwrap();
    std::fs::write(swapped.join("d0/f03"), b"body 0").unwrap();
    assert_ne!(scratch.revision("b"), scratch.revision("c"));
}

#[test]
fn a_symbolic_link_under_a_placed_path_is_digested_as_its_target_and_never_followed() {
    let scratch = Scratch::new();
    let outside = scratch.root.join("outside");
    std::fs::write(&outside, b"secret one").unwrap();
    scratch.put("src/main", b"fn main() {}");
    symlink(&outside, scratch.workspace.join("src/link")).unwrap();
    let first = scratch.revision("src");
    // What the link points at is never read: changing it moves nothing.
    std::fs::write(&outside, b"secret two, and longer").unwrap();
    assert_eq!(scratch.revision("src"), first);
    // The target string is: another target moves the revision.
    std::fs::remove_file(scratch.workspace.join("src/link")).unwrap();
    symlink(
        scratch.root.join("elsewhere"),
        scratch.workspace.join("src/link"),
    )
    .unwrap();
    assert_ne!(scratch.revision("src"), first);

    // A link at the placed path itself is digested as its target string.
    symlink(&outside, scratch.workspace.join("docs/spec")).unwrap();
    let target = outside.to_str().unwrap().as_bytes();
    scratch.put("docs/copy", target);
    assert_eq!(scratch.revision("docs/spec"), scratch.revision("docs/copy"));

    // A link among the directories of a placed path is refused, wherever it points.
    std::fs::create_dir(scratch.root.join("away")).unwrap();
    std::fs::write(scratch.root.join("away/spec"), b"abc").unwrap();
    symlink(scratch.root.join("away"), scratch.workspace.join("linked")).unwrap();
    let (reason, detail) = content_revision(&scratch.workspace, "linked/spec").unwrap_err();
    assert_eq!(reason, RevisionRefusalReason::OutsideWorkspace, "{detail}");
    symlink("docs", scratch.workspace.join("inside")).unwrap();
    let (reason, _) = content_revision(&scratch.workspace, "inside/copy").unwrap_err();
    assert_eq!(reason, RevisionRefusalReason::OutsideWorkspace);
    for path in ["/etc/hostname", "../outside", "docs/../../outside"] {
        let (reason, _) = content_revision(&scratch.workspace, path).unwrap_err();
        assert_eq!(reason, RevisionRefusalReason::OutsideWorkspace, "{path}");
    }
}

#[test]
fn a_file_over_the_bound_and_an_entry_that_is_no_file_are_refused_unread() {
    let scratch = Scratch::new();
    let big = std::fs::File::create(scratch.workspace.join("docs/big")).unwrap();
    big.set_len(MAX_ARTIFACT_BYTES + 1).unwrap();
    let (reason, _) = content_revision(&scratch.workspace, "docs/big").unwrap_err();
    assert_eq!(reason, RevisionRefusalReason::TooLarge);
    let (reason, _) = content_revision(&scratch.workspace, "docs").unwrap_err();
    assert_eq!(reason, RevisionRefusalReason::TooLarge);

    let fifo = scratch.workspace.join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    let (reason, _) = content_revision(&scratch.workspace, "pipe").unwrap_err();
    assert_eq!(reason, RevisionRefusalReason::Unreadable);
}

// ---------------------------------------------------------------------------------------------
// The step

#[test]
fn a_write_action_that_changes_a_placed_file_moves_its_revision_and_the_case_revision_by_one() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"first");
    let governor = governor(&scratch);
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
    let end = step(&scratch, &governor, CHECK, "printf abc > docs/spec\n");
    let report = report(&end);
    assert_eq!(revisions(&governor)["spec"], ABC);
    assert_eq!(revisions(&governor)["impl"], ABSENT);
    assert_eq!(governor.current_revision(&case()).unwrap(), 2);
    assert_eq!(report["step"]["outcome"], json!("Recorded"));
    assert_eq!(
        report["step"]["moves"],
        json!([{"artifact": "spec", "from": content_revision_of(b"first"), "to": ABC}])
    );
}

/// The revision of a file holding `bytes`.
fn content_revision_of(bytes: &[u8]) -> String {
    let scratch = Scratch::new();
    scratch.put("file", bytes);
    scratch.revision("file")
}

#[test]
fn a_program_that_changes_nothing_moves_no_revision() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"first");
    let governor = governor(&scratch);
    let before = revisions(&governor);
    let end = step(&scratch, &governor, CHECK, "exit 0\n");
    assert_eq!(report(&end)["step"]["moves"], json!([]));
    assert_eq!(revisions(&governor), before);
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
    assert_eq!(records(&governor).len(), 1);
}

#[test]
fn a_record_of_a_writing_step_names_the_moved_revision_and_counts_at_the_next_evaluation() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"first");
    scratch.put("src/lib", b"one");
    let governor = governor(&scratch);
    assert_eq!(open_evidence(&governor, CHECK), vec!["gate".to_owned()]);
    step(
        &scratch,
        &governor,
        CHECK,
        "printf abc > docs/spec\nprintf two > src/lib\n",
    );
    let records = records(&governor);
    assert_eq!(records.len(), 1, "{records:#?}");
    let facts = plain(&records[0].facts);
    assert_eq!(facts["subject_revision"], json!(ABC));
    assert_eq!(
        facts["upstream_revisions"],
        json!({"impl": revisions(&governor)["impl"]})
    );
    assert_ne!(revisions(&governor)["impl"], content_revision_of(b"one"));
    assert_eq!(records[0].subject_revision, 3);
    assert_eq!(governor.current_revision(&case()).unwrap(), 3);
    assert!(
        open_evidence(&governor, CHECK).is_empty(),
        "the record about the current revision counts"
    );
}

#[test]
fn an_action_that_does_not_write_and_changed_a_placed_artifact_is_discarded_and_still_moves() {
    for action in [LOOK, SIGN] {
        let scratch = Scratch::new();
        scratch.put("docs/spec", b"first");
        let governor = governor(&scratch);
        let end = step(&scratch, &governor, action, "printf abc > docs/spec\n");
        let report = report(&end);
        assert!(records(&governor).is_empty(), "{action}: a record was kept");
        assert_eq!(report["evidence"], json!([]), "{action}");
        assert_eq!(report["step"]["outcome"], json!("Discarded"), "{action}");
        assert_eq!(
            report["step"]["discarded"],
            json!("ChangedByNonWriter"),
            "{action}"
        );
        assert_eq!(revisions(&governor)["spec"], ABC, "{action}");
        assert_eq!(governor.current_revision(&case()).unwrap(), 2, "{action}");
    }
}

#[test]
fn an_action_that_does_not_write_and_changed_nothing_records_its_evidence() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"first");
    let governor = governor(&scratch);
    let end = step(&scratch, &governor, LOOK, "exit 0\n");
    assert_eq!(report(&end)["step"]["outcome"], json!("Recorded"));
    assert_eq!(records(&governor).len(), 1);
}

#[test]
fn a_placed_path_through_a_link_starts_nothing_and_moves_nothing() {
    let scratch = Scratch::new();
    std::fs::remove_dir(scratch.workspace.join("docs")).unwrap();
    std::fs::create_dir(scratch.root.join("away")).unwrap();
    symlink(scratch.root.join("away"), scratch.workspace.join("docs")).unwrap();
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .unwrap();
    governor
        .open_case(
            case(),
            REGISTRATION,
            [
                ("spec".to_owned(), "s1".to_owned()),
                ("impl".to_owned(), "i1".to_owned()),
            ]
            .into(),
            CaseInputs::none(),
        )
        .unwrap();
    let marker = scratch.root.join("marker");
    let reason = refusal(&step(
        &scratch,
        &governor,
        CHECK,
        &format!("/usr/bin/touch '{}'\n", marker.display()),
    ));
    assert!(reason.starts_with("ArtifactUnreadable"), "{reason}");
    assert!(reason.contains("OutsideWorkspace"), "{reason}");
    assert!(!marker.exists(), "the program started");
    assert_eq!(revisions(&governor)["spec"], "s1");
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
}

// ---------------------------------------------------------------------------------------------
// Before every evaluation

#[test]
fn a_file_changed_in_the_workspace_between_two_steps_moves_at_the_next_evaluation() {
    let scratch = Scratch::new();
    scratch.put("docs/spec", b"first");
    let governor = governor(&scratch);
    let document = document(CHECK, &["/bin/true"]);
    let placed = PlacedRevisions::new(&governor, &document, &scratch.workspace);
    assert_eq!(placed.frontier(&case()).unwrap().data().case_revision, 1);

    // A person's edit, between two steps.
    scratch.put("docs/spec", b"abc");
    let frontier = placed.frontier(&case()).unwrap();
    assert_eq!(
        frontier.data().case_revision,
        2,
        "the frontier is read after the move"
    );
    assert_eq!(revisions(&governor)["spec"], ABC);
    // Read again with nothing changed, nothing moves.
    placed.completion(&case()).unwrap();
    assert_eq!(governor.current_revision(&case()).unwrap(), 2);
}

#[test]
fn an_evaluation_whose_placed_path_cannot_be_digested_decides_nothing() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    let document = document(CHECK, &["/bin/true"]);
    std::fs::remove_dir(scratch.workspace.join("docs")).unwrap();
    symlink("/", scratch.workspace.join("docs")).unwrap();
    let placed = PlacedRevisions::new(&governor, &document, &scratch.workspace);
    assert!(placed.frontier(&case()).is_err());
    assert!(placed.completion(&case()).is_err());
    assert_eq!(governor.current_revision(&case()).unwrap(), 1);
}
