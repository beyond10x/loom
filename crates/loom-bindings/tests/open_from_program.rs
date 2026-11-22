//! Acceptance for `story:open-children-from-arguments-from`: an action bound to an `open` executor
//! with `each` and `arguments_from` opens one child case per name its program prints, runs that
//! program as a `program` binding's `arguments_from` program runs, checks the object against the
//! action's `arguments` schema and the names before anything is written, keeps the list as one
//! `loom/open` observation and a blob, and resumes a cut opening without opening a child twice.
//!
//! This base has one file per test binary, so the module is this file. Every step reaches the open
//! effect port through Commission's own loop (`run_until_blocked`), a scripted executor and a
//! `CanonGovernor` over a `FileCaseStore` under `CARGO_TARGET_TMPDIR`, wrapped in a store that
//! fails the next `insert` or `update` of a chosen case; a `local` runner opened with
//! `UNCONFINED_FLAG_REASON` runs `/bin/sh` stubs the test writes; no model, no network. The
//! fixtures (`tests/fixtures/open-from-program/`) have the shape of Runbook's and copy no file of
//! it: `roadmap/1` opens `epic/1`, which opens `story/1`, which opens `task/1`; `loose/1` opens
//! with the schema `{type: object}`. Their bindings files sit in directory `A`, beside `A/bin/`;
//! the workspace is directory `B`.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use b10x_loom_bindings::{
    OPEN_SOURCE, OpenEffects, OpenPortError, ProgramEffects, check, check_case_open, decode,
    resolve_programs,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, EvidenceId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Truth,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, ConfinementRefusal, EngineConfinement, EngineRunner, TestRunner,
};
use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason};
use loom_governor::{CanonGovernor, CaseInputs, CaseState, FallibleCaseStore, FileCaseStore};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use sha2::{Digest as _, Sha256};

const ROOT: &str = "root";
const EPICS: &str = "epics.open";
const STORIES: &str = "stories.open";
const TASKS: &str = "tasks.open";
const NOTE: &str = "roadmap.note";

/// The fixture directory.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/open-from-program")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

/// The registrations and their protocol files.
const PROTOCOLS: [(&str, &str); 5] = [
    ("roadmap@1", "roadmap-1.yaml"),
    ("epic@1", "epic-1.yaml"),
    ("story@1", "story-1.yaml"),
    ("task@1", "task-1.yaml"),
    ("loose@1", "loose-1.yaml"),
];

/// The bindings files, each copied into `A`.
const BINDINGS: [&str; 5] = [
    "roadmap.yaml",
    "epic.yaml",
    "story.yaml",
    "task.yaml",
    "loose.yaml",
];

/// The fixture protocols, and `extra` (registration and text).
fn catalog_with(extra: &[(&str, String)]) -> ProtocolCatalog {
    let source = || ProtocolSource {
        kind: SourceKind::Memory,
        location: "open-from-program-fixture".into(),
        revision: String::new(),
        path: String::new(),
    };
    let mut catalog = ProtocolCatalog::default();
    for (name, file) in PROTOCOLS {
        catalog.add_yaml(name, &fixture(file), source()).unwrap();
    }
    for (name, text) in extra {
        catalog.add_yaml(name, text, source()).unwrap();
    }
    catalog
}

fn catalog() -> ProtocolCatalog {
    catalog_with(&[])
}

// ---------------------------------------------------------------------------------------------
// The store that fails one write

/// A store that fails the next `insert` or the next `update` of a chosen case, as a process that
/// dies in that write would leave it (the form of `Faulty` in the governor's child-case tests,
/// written again here, since a test file of another crate cannot be imported).
struct Faulty {
    inner: FileCaseStore,
    fail_insert_of: Mutex<Option<String>>,
    fail_update_of: Mutex<Option<String>>,
}

impl Faulty {
    fn fail_next_insert_of(&self, case: &str) {
        *self.fail_insert_of.lock().unwrap() = Some(case.to_owned());
    }

    fn fail_next_update_of(&self, case: &str) {
        *self.fail_update_of.lock().unwrap() = Some(case.to_owned());
    }

    /// Whether `slot` names `case`, emptying it when it does.
    fn fails(slot: &Mutex<Option<String>>, case: &str) -> bool {
        let mut slot = slot.lock().unwrap();
        if slot.as_deref() == Some(case) {
            *slot = None;
            return true;
        }
        false
    }
}

impl FallibleCaseStore for Faulty {
    type Error = String;
    fn insert(&self, state: CaseState) -> Result<bool, String> {
        if Self::fails(&self.fail_insert_of, &state.id.0) {
            return Err(format!(
                "the write of {} failed (the process died)",
                state.id.0
            ));
        }
        self.inner.insert(state).map_err(|e| e.to_string())
    }
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, String> {
        self.inner.get(case).map_err(|e| e.to_string())
    }
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, String> {
        if Self::fails(&self.fail_update_of, &case.0) {
            return Err(format!("the write of {} failed (the process died)", case.0));
        }
        self.inner.update(case, change).map_err(|e| e.to_string())
    }
    fn observe(&self, observation: ObservationData) -> Result<(), String> {
        self.inner.observe(observation).map_err(|e| e.to_string())
    }
    fn observations(&self) -> Result<Vec<ObservationData>, String> {
        self.inner.observations().map_err(|e| e.to_string())
    }
}

type Governor = CanonGovernor<Faulty>;

// ---------------------------------------------------------------------------------------------
// Scratch, documents and ports

/// One test's directories: `A` (the bindings files and `A/bin/`), `B` (the workspace) and the
/// store; the stubs count their runs in files of the scratch root.
struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
    bindings: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    /// The directories, the fixture bindings files in `A`, and stubs: `bin/list-epics` prints
    /// `{"epics": ["a", "b"]}`, `bin/list-stories` `{"stories": ["s1"]}`, `bin/list-tasks`
    /// `{"tasks": ["t1"]}`, and `bin/note` exits 0.
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("open-from-program-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let root = dir.path().canonicalize().unwrap();
        let scratch = Self {
            bindings: root.join("A"),
            workspace: root.join("B"),
            store: root.join("store"),
            root,
            _dir: dir,
        };
        for directory in [&scratch.bindings, &scratch.workspace, &scratch.store] {
            std::fs::create_dir_all(directory).unwrap();
        }
        for file in BINDINGS {
            std::fs::write(scratch.bindings.join(file), fixture(file)).unwrap();
        }
        scratch.prints("list-epics", r#"{"epics": ["a", "b"]}"#);
        scratch.prints("list-stories", r#"{"stories": ["s1"]}"#);
        scratch.prints("list-tasks", r#"{"tasks": ["t1"]}"#);
        scratch.stub("A/bin/note", "exit 0\n");
        scratch
    }

    /// An executable `/bin/sh` script at `path` under the scratch root, holding `body`.
    fn stub(&self, path: &str, body: &str) -> PathBuf {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The file `A/bin/<name>` counts its runs in.
    fn count_file(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.count"))
    }

    /// The runs of `A/bin/<name>`, counted by the line it appends per run.
    fn runs(&self, name: &str) -> usize {
        std::fs::read_to_string(self.count_file(name)).map_or(0, |text| text.lines().count())
    }

    /// `A/bin/<name>` as a stub that counts its run, then runs `body`.
    fn counting(&self, name: &str, body: &str) {
        let count = self.count_file(name);
        self.stub(
            &format!("A/bin/{name}"),
            &format!("echo run >> '{}'\n{body}", count.display()),
        );
    }

    /// `A/bin/<name>` as a stub that counts its run and prints `json`, which holds no `'`.
    fn prints(&self, name: &str, json: &str) {
        self.counting(name, &format!("printf '%s' '{json}'\n"));
    }

    /// The bindings file `file` of `A`, rewritten by `change`.
    fn rewrite(&self, file: &str, change: impl FnOnce(String) -> String) {
        let path = self.bindings.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, change(text)).unwrap();
    }
}

/// The document in `A/<file>`, decoded, checked against `registration` and resolved beside it.
fn document(scratch: &Scratch, file: &str) -> BindingsDocument {
    let text = std::fs::read_to_string(scratch.bindings.join(file)).unwrap();
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    let catalog = catalog();
    let entry = catalog.get(&document.protocol).unwrap();
    check(&document, &document.protocol, &entry.model)
        .and_then(|()| check_case_open(&document, &entry.model))
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    resolve_programs(&document, &scratch.bindings).unwrap_or_else(|refusal| panic!("{refusal}"))
}

/// A governor over a fresh store of `scratch`, holding the case [`ROOT`] of `roadmap@1`.
fn governor(scratch: &Scratch) -> Governor {
    let store = Faulty {
        inner: FileCaseStore::open(&scratch.store).unwrap(),
        fail_insert_of: Mutex::default(),
        fail_update_of: Mutex::default(),
    };
    let governor = CanonGovernor::new(store).with_catalog(&catalog()).unwrap();
    open_root(&governor, ROOT, "roadmap@1", "roadmap");
    governor
}

fn open_root(governor: &Governor, case: &str, registration: &str, artifact: &str) {
    governor
        .open_case(
            CaseId(case.to_owned()),
            registration,
            BTreeMap::from([(artifact.to_owned(), "r1".to_owned())]),
            CaseInputs::none(),
        )
        .unwrap();
}

/// The host runner: `local`, opened with the flag's reason.
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

/// The open port of `case` for the document of `A/<file>`, with `runner`.
fn try_port<'a>(
    governor: &'a Governor,
    scratch: &Scratch,
    case: &str,
    document: &'a BindingsDocument,
    file: &str,
    runner: Option<Arc<dyn TestRunner>>,
) -> Result<OpenEffects<'a, Faulty>, OpenPortError> {
    OpenEffects::new(
        governor,
        CaseId(case.to_owned()),
        document,
        &scratch.bindings.join(file),
        &catalog(),
        &scratch.workspace,
        &scratch.store,
        runner,
    )
}

fn port<'a>(
    governor: &'a Governor,
    scratch: &Scratch,
    case: &str,
    document: &'a BindingsDocument,
    file: &str,
) -> OpenEffects<'a, Faulty> {
    try_port(governor, scratch, case, document, file, Some(flag_runner()))
        .unwrap_or_else(|error| panic!("the port is built: {error}"))
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
        Timestamp("2026-11-22T12:00:00Z".to_owned())
    }
    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// One step of `case` in which the executor proposes `action` with the arguments `{}`.
fn try_step(
    governor: &Governor,
    effects: &impl EffectPort,
    case: &str,
    action: &str,
) -> Result<LoopEnd, LoopError> {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid()),
        agent_revision_id: AgentRevisionId(uuid()),
        case_id: CaseId(case.to_owned()),
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
        effects,
        &commission,
        &mut runs,
        &mut Context,
    )
}

fn step(governor: &Governor, effects: &impl EffectPort, case: &str, action: &str) -> LoopEnd {
    try_step(governor, effects, case, action)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// The report of the one performed effect of `end`.
fn performed(end: &LoopEnd) -> serde_json::Value {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    match &end.effects[0] {
        EffectOutcome::Performed(performed) => plain(&performed.report),
        other => panic!("not performed: {other:?}"),
    }
}

/// The reason of the one refused effect of `end`.
fn refusal(end: &LoopEnd) -> String {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    match &end.effects[0] {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

/// `value` as a `serde_json` value.
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

/// The ids of `case` and the cases below it, in the governor's order (`tree` lists the whole tree).
fn tree(governor: &Governor, case: &str) -> Vec<String> {
    let below = format!("{case}.");
    governor
        .tree(&CaseId(case.to_owned()))
        .unwrap()
        .into_iter()
        .map(|member| member.case.0)
        .filter(|id| id == case || id.starts_with(&below))
        .collect()
}

/// The payloads of the observations of [`OPEN_SOURCE`].
fn openings(governor: &Governor) -> Vec<serde_json::Value> {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == OPEN_SOURCE)
        .map(|observation| plain(&observation.payload))
        .collect()
}

/// Every file under `directory` and its bytes, but `observations.json`, where Commission's loop
/// logs each proposal and each refusal of a step whatever the port does.
fn files(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![directory.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name != "observations.json")
            {
                found.insert(path.clone(), std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    found
}

/// Opens the epics of [`ROOT`] with the stubs as they are; the step's report.
fn open_epics(governor: &Governor, scratch: &Scratch) -> serde_json::Value {
    let document = document(scratch, "roadmap.yaml");
    let effects = port(governor, scratch, ROOT, &document, "roadmap.yaml");
    performed(&step(governor, &effects, ROOT, EPICS))
}

/// The refusal of one step of `action` in `case` under the document of `A/<file>`, which must
/// write nothing: no case file changes, no blob is kept, no `loom/open` observation is added.
fn refused_writing_nothing(
    governor: &Governor,
    scratch: &Scratch,
    case: &str,
    file: &str,
    action: &str,
) -> String {
    let before = files(&scratch.store);
    let kept = openings(governor).len();
    let document = document(scratch, file);
    let effects = port(governor, scratch, case, &document, file);
    let reason = refusal(&step(governor, &effects, case, action));
    assert_eq!(
        files(&scratch.store),
        before,
        "a byte of the store changed: {reason}"
    );
    assert_eq!(openings(governor).len(), kept, "{reason}");
    reason
}

/// The refusal of [`EPICS`] in [`ROOT`] when `A/bin/list-epics` prints `json`; no child opens.
fn epics_refused(json: &str) -> String {
    let scratch = Scratch::new();
    scratch.prints("list-epics", json);
    let governor = governor(&scratch);
    let reason = refused_writing_nothing(&governor, &scratch, ROOT, "roadmap.yaml", EPICS);
    assert_eq!(tree(&governor, ROOT), [ROOT], "{reason}");
    reason
}

/// The refusal of [`EPICS`] in the `loose@1` case `free` when `A/bin/list-epics` prints `json`.
fn loose_refused(json: &str) -> String {
    let scratch = Scratch::new();
    scratch.prints("list-epics", json);
    let governor = governor(&scratch);
    open_root(&governor, "free", "loose@1", "loose");
    let reason = refused_writing_nothing(&governor, &scratch, "free", "loose.yaml", EPICS);
    assert_eq!(tree(&governor, "free"), ["free"], "{reason}");
    reason
}

// ---------------------------------------------------------------------------------------------
// Opening from the printed names

#[test]
fn an_each_opening_opens_one_child_per_printed_name() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    let report = open_epics(&governor, &scratch);
    assert_eq!(report["children"], serde_json::json!(["root.a", "root.b"]));
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.a", "root.b"]);
    for child in ["root.a", "root.b"] {
        assert_eq!(
            governor.protocol(&CaseId(child.to_owned())).unwrap(),
            "epic@1"
        );
    }
    let outcomes = governor
        .evidence(&CaseId(ROOT.to_owned()))
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == "case_outcome")
        .count();
    assert_eq!(outcomes, 2);
    assert_eq!(scratch.runs("list-epics"), 1);
}

#[test]
fn a_child_opens_its_own_children_from_its_program() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    scratch.prints("list-stories", r#"{"stories": ["s1", "s2", "s3"]}"#);
    let document = document(&scratch, "epic.yaml");
    let effects = port(&governor, &scratch, "root.a", &document, "epic.yaml");
    performed(&step(&governor, &effects, "root.a", STORIES));
    assert_eq!(
        tree(&governor, "root.a"),
        ["root.a", "root.a.s1", "root.a.s2", "root.a.s3"]
    );
    assert_eq!(
        governor.protocol(&CaseId("root.a.s2".to_owned())).unwrap(),
        "story@1"
    );
}

/// An observation the governor received, for a submitted record to cite.
fn observed(governor: &Governor) -> ObservationId {
    let observation = ObservationId(uuid());
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "trusted:open-from-program".into(),
            subject: "epic".into(),
            observed_at: Timestamp("2026-11-22T12:00:00Z".into()),
            payload: Value::Null,
        }),
    )
    .unwrap();
    observation
}

/// Records a passing verdict about the epic of `case` and terminates it `passed`.
fn pass(governor: &Governor, case: &str) {
    let case = CaseId(case.to_owned());
    let n = ISSUED.fetch_add(1, Ordering::Relaxed);
    let revision = governor.revisions(&case).unwrap()["epic"].clone();
    submit_evidence(
        governor,
        "judge",
        EvidenceData {
            evidence_id: EvidenceId(uuid()),
            case_id: case.clone(),
            kind: "verdict".into(),
            subject_revision: governor.current_revision(&case).unwrap(),
            producer: "judge".into(),
            observation_ids: vec![observed(governor)],
            facts: json::parse(
                &serde_json::json!({
                    "format": "canon-evidence/1",
                    "id": format!("verdict-{n}"),
                    "kind": "verdict",
                    "subject": "epic",
                    "subject_revision": revision,
                    "result": "passed",
                })
                .to_string(),
            )
            .unwrap(),
            provenance: Value::Null,
        },
    )
    .unwrap();
    governor
        .terminate(&case, "passed", "2026-11-22T12:00:01Z")
        .unwrap();
}

fn children_passed(governor: &Governor) -> Truth {
    governor
        .frontier(&CaseId(ROOT.to_owned()))
        .unwrap()
        .data()
        .claims
        .iter()
        .find(|entry| entry.claim == "children.passed")
        .unwrap()
        .value
}

#[test]
fn children_passed_reads_the_children_the_program_named() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    assert_eq!(children_passed(&governor), Truth::Unknown);
    pass(&governor, "root.a");
    assert_eq!(children_passed(&governor), Truth::Unknown);
    pass(&governor, "root.b");
    assert_eq!(children_passed(&governor), Truth::True);
}

// ---------------------------------------------------------------------------------------------
// The schema

#[test]
fn a_printed_object_outside_the_schema_is_refused() {
    let reason = epics_refused(r#"{"epics": ["a"], "extra": 1}"#);
    assert!(
        reason.starts_with("ArgumentsMismatched: action epics.open: "),
        "{reason}"
    );
    assert!(reason.contains("additionalProperties"), "{reason}");
}

#[test]
fn an_empty_list_is_refused_by_min_items() {
    let reason = epics_refused(r#"{"epics": []}"#);
    assert!(reason.starts_with("ArgumentsMismatched: "), "{reason}");
    assert!(
        reason.contains("\"/epics\"") && reason.contains("minItems"),
        "{reason}"
    );
}

// ---------------------------------------------------------------------------------------------
// The program's environment

/// The variables `/usr/bin/env` printed into `path`, but those `/bin/sh` sets itself (`PWD`,
/// `SHLVL`, `_`).
fn printed_environment(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| {
            !["PWD=", "SHLVL=", "_="]
                .iter()
                .any(|own| line.starts_with(own))
        })
        .map(str::to_owned)
        .collect()
}

/// `PATH` at the host's value, `LOOM_TIMEOUT_SECONDS=60` and `more`.
fn expected_environment(more: &[&str]) -> BTreeSet<String> {
    let mut expected: BTreeSet<String> = more.iter().map(|line| (*line).to_owned()).collect();
    expected.insert(format!(
        "PATH={}",
        std::env::var("PATH").unwrap_or_default()
    ));
    expected.insert("LOOM_TIMEOUT_SECONDS=60".to_owned());
    expected
}

/// `A/bin/<name>` as a stub that writes its environment to a file and prints `json`; the file.
fn printing_environment(scratch: &Scratch, name: &str, json: &str) -> PathBuf {
    let file = scratch.root.join(format!("{name}.env"));
    scratch.counting(
        name,
        &format!(
            "/usr/bin/env > '{}'\nprintf '%s' '{json}'\n",
            file.display()
        ),
    );
    file
}

#[test]
fn a_roots_opening_program_gets_no_case_name() {
    let scratch = Scratch::new();
    let file = printing_environment(&scratch, "list-epics", r#"{"epics": ["a"]}"#);
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    assert_eq!(
        printed_environment(&file),
        expected_environment(&["LOOM_CASE=root"])
    );
}

#[test]
fn a_childs_opening_program_gets_its_name() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    let file = printing_environment(&scratch, "list-stories", r#"{"stories": ["s1"]}"#);
    let document = document(&scratch, "epic.yaml");
    let effects = port(&governor, &scratch, "root.a", &document, "epic.yaml");
    performed(&step(&governor, &effects, "root.a", STORIES));
    assert_eq!(
        printed_environment(&file),
        expected_environment(&["LOOM_CASE=root.a", "LOOM_CASE_NAME=a"])
    );
}

#[test]
fn a_grandchilds_opening_program_gets_both_names() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    let epic = document(&scratch, "epic.yaml");
    let effects = port(&governor, &scratch, "root.a", &epic, "epic.yaml");
    performed(&step(&governor, &effects, "root.a", STORIES));
    let file = printing_environment(&scratch, "list-tasks", r#"{"tasks": ["t1"]}"#);
    let story = document(&scratch, "story.yaml");
    let effects = port(&governor, &scratch, "root.a.s1", &story, "story.yaml");
    performed(&step(&governor, &effects, "root.a.s1", TASKS));
    assert_eq!(
        printed_environment(&file),
        expected_environment(&[
            "LOOM_CASE=root.a.s1",
            "LOOM_CASE_NAME=s1",
            "LOOM_PARENT_CASE_NAME=a",
        ])
    );
    assert_eq!(tree(&governor, "root.a.s1"), ["root.a.s1", "root.a.s1.t1"]);
}

#[test]
fn an_opening_whose_parent_cannot_be_read_starts_nothing() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    let root_file = scratch
        .store
        .join(format!("{}.json", loom_governor::case_file_stem(ROOT)));
    std::fs::remove_file(&root_file).unwrap();
    let reason = refused_writing_nothing(&governor, &scratch, "root.a", "epic.yaml", STORIES);
    assert!(
        reason.contains("root.a") && reason.contains("`root`"),
        "{reason}"
    );
    assert!(
        !reason.contains("roadmap@1") && !reason.contains("epics.open"),
        "{reason}"
    );
    assert!(!scratch.count_file("list-stories").exists(), "the stub ran");
}

#[test]
fn an_open_arguments_program_reads_the_request_and_starts_in_the_workspace() {
    let scratch = Scratch::new();
    let input = scratch.root.join("stdin");
    let directory = scratch.root.join("pwd");
    scratch.counting(
        "list-epics",
        &format!(
            "/usr/bin/cat > '{}'\n/usr/bin/pwd -P > '{}'\nprintf '%s' '{{\"epics\": [\"a\"]}}'\n",
            input.display(),
            directory.display()
        ),
    );
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    assert_eq!(std::fs::read_to_string(&input).unwrap(), "{}");
    assert_eq!(
        std::fs::read_to_string(&directory).unwrap().trim_end(),
        scratch.workspace.to_str().unwrap()
    );
}

// ---------------------------------------------------------------------------------------------
// A program that gives no object

#[test]
fn an_arguments_program_ending_non_zero_opens_nothing() {
    let scratch = Scratch::new();
    scratch.counting("list-epics", "printf '%s' '{\"epics\": [\"a\"]}'\nexit 1\n");
    let governor = governor(&scratch);
    let reason = refused_writing_nothing(&governor, &scratch, ROOT, "roadmap.yaml", EPICS);
    assert!(
        reason.starts_with("ArgumentsUnreadable: action epics.open: "),
        "{reason}"
    );
    assert!(reason.contains("exit status 1"), "{reason}");
    assert_eq!(tree(&governor, ROOT), [ROOT]);
}

#[test]
fn an_arguments_program_killed_at_its_timeout_opens_nothing() {
    let scratch = Scratch::new();
    scratch.counting(
        "list-epics",
        "/usr/bin/sleep 5\nprintf '%s' '{\"epics\": [\"a\"]}'\n",
    );
    scratch.rewrite("roadmap.yaml", |text| {
        text.replace(
            "    arguments_from: [bin/list-epics]\n",
            "    arguments_from: [bin/list-epics]\n    timeout_seconds: 1\n",
        )
    });
    let governor = governor(&scratch);
    let started = std::time::Instant::now();
    let reason = refused_writing_nothing(&governor, &scratch, ROOT, "roadmap.yaml", EPICS);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "waited for the stub"
    );
    assert!(reason.starts_with("ArgumentsUnreadable: "), "{reason}");
    assert!(reason.contains("timeout"), "{reason}");
    assert_eq!(tree(&governor, ROOT), [ROOT]);
}

#[test]
fn an_output_over_the_bound_opens_nothing() {
    let scratch = Scratch::new();
    scratch.counting(
        "list-epics",
        "/usr/bin/head -c 1048577 /dev/zero | /usr/bin/tr '\\000' ' '\n",
    );
    let governor = governor(&scratch);
    let reason = refused_writing_nothing(&governor, &scratch, ROOT, "roadmap.yaml", EPICS);
    assert!(reason.starts_with("ArgumentsUnreadable: "), "{reason}");
    assert_eq!(tree(&governor, ROOT), [ROOT]);
}

#[test]
fn an_output_that_is_not_one_object_opens_nothing() {
    for printed in ["[]", r#"{"epics": ["a"]}{"epics": ["b"]}"#] {
        let reason = epics_refused(printed);
        assert!(
            reason.starts_with("ArgumentsUnreadable: "),
            "{printed}: {reason}"
        );
        assert!(!reason.contains("\"epics\""), "{printed}: {reason}");
    }
}

#[test]
fn an_arguments_program_that_does_not_start_opens_nothing() {
    // Beside the bindings file, a missing program is refused when the document is resolved.
    let scratch = Scratch::new();
    scratch.rewrite("roadmap.yaml", |text| {
        text.replace("[bin/list-epics]", "[bin/absent]")
    });
    let text = std::fs::read_to_string(scratch.bindings.join("roadmap.yaml")).unwrap();
    let refused = resolve_programs(&decode(&text).unwrap(), &scratch.bindings)
        .expect_err("a missing program is refused");
    assert_eq!(
        refused.0.reason,
        Reason::ProgramNotBesideBindings,
        "{refused}"
    );
    // Read as a workspace path, it does not start: the step is refused and writes nothing.
    scratch.rewrite("roadmap.yaml", |text| {
        text.replace("program_root: bindings\n", "")
            .replace("argv: [bin/note]", "argv: [/bin/true]")
    });
    let governor = governor(&scratch);
    let reason = refused_writing_nothing(&governor, &scratch, ROOT, "roadmap.yaml", EPICS);
    assert!(reason.contains("bin/absent"), "{reason}");
    assert_eq!(tree(&governor, ROOT), [ROOT]);
}

// ---------------------------------------------------------------------------------------------
// The names

#[test]
fn a_missing_or_scalar_member_opens_nothing() {
    for printed in ["{}", r#"{"epics": "a"}"#] {
        let reason = loose_refused(printed);
        assert!(
            reason.starts_with("EachNotAList: action epics.open: "),
            "{reason}"
        );
        assert!(reason.contains("epics"), "{reason}");
    }
}

#[test]
fn a_non_text_name_opens_nothing() {
    let reason = loose_refused(r#"{"epics": ["a", 1]}"#);
    assert!(reason.starts_with("EachItemNotText: "), "{reason}");
    assert!(reason.contains("entry 1"), "{reason}");
}

#[test]
fn an_empty_list_without_min_items_opens_nothing() {
    let reason = loose_refused(r#"{"epics": []}"#);
    assert!(reason.starts_with("NoChildNamed: "), "{reason}");
}

#[test]
fn a_repeated_name_opens_nothing() {
    let reason = epics_refused(r#"{"epics": ["a", "a"]}"#);
    assert!(reason.starts_with("ChildNameRepeated: "), "{reason}");
    assert!(reason.contains("entry 1"), "{reason}");
}

#[test]
fn a_name_that_is_not_one_segment_opens_nothing() {
    for (printed, entry) in [
        (r#"{"epics": ["a b"]}"#, "a b"),
        (r#"{"epics": ["../x"]}"#, "../x"),
        (r#"{"epics": ["a.b"]}"#, "a.b"),
    ] {
        let reason = epics_refused(printed);
        assert!(reason.starts_with("ChildNameInvalid: "), "{reason}");
        assert!(reason.contains("entry 0"), "{reason}");
        assert!(!reason.contains(entry), "{entry}: {reason}");
    }
}

// ---------------------------------------------------------------------------------------------
// The port

#[test]
fn an_arguments_program_needs_a_declared_schema() {
    let bare = fixture("loose-1.yaml")
        .replace("  id: loose\n", "  id: bare\n")
        .replace("    arguments: {type: object}\n", "");
    let scratch = Scratch::new();
    let text = fixture("loose.yaml").replace("protocol: loose@1", "protocol: bare@1");
    std::fs::write(scratch.bindings.join("bare.yaml"), &text).unwrap();
    let document = decode(&text).unwrap();
    let governor = governor(&scratch);
    let refused = OpenEffects::new(
        &governor,
        CaseId(ROOT.to_owned()),
        &document,
        &scratch.bindings.join("bare.yaml"),
        &catalog_with(&[("bare@1", bare)]),
        &scratch.workspace,
        &scratch.store,
        Some(flag_runner()),
    )
    .expect_err("the port is refused");
    let OpenPortError::Refused(refused) = refused else {
        panic!("not a refusal of the document: {refused}");
    };
    assert_eq!(refused.0.reason, Reason::ArgumentsUndeclared, "{refused}");
    assert_eq!(refused.0.action.as_deref(), Some(EPICS));
    assert_eq!(refused.0.member, "arguments_from");
}

#[test]
fn an_open_with_an_arguments_program_needs_a_runner() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    let with = document(&scratch, "roadmap.yaml");
    match try_port(&governor, &scratch, ROOT, &with, "roadmap.yaml", None) {
        Err(OpenPortError::Runner(error)) => {
            assert_eq!(error.reason, ConfinementRefusal::NoRunner, "{error}");
        }
        other => panic!("not refused for its runner: {other:?}"),
    }
    scratch.rewrite("roadmap.yaml", |text| {
        text.replace("    arguments_from: [bin/list-epics]\n", "")
    });
    let without = document(&scratch, "roadmap.yaml");
    try_port(&governor, &scratch, ROOT, &without, "roadmap.yaml", None)
        .expect("a document with no arguments_from needs no runner");
}

#[test]
fn the_list_is_recorded_with_its_digest() {
    let scratch = Scratch::new();
    let printed = r#"{"epics": ["b", "a"]}"#;
    scratch.prints("list-epics", printed);
    let governor = governor(&scratch);
    let revision = governor.current_revision(&CaseId(ROOT.to_owned())).unwrap();
    let report = open_epics(&governor, &scratch);
    let digest: String = Sha256::digest(printed.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        openings(&governor),
        [serde_json::json!({
            "case": ROOT,
            "action": EPICS,
            "case_revision": revision,
            "source": "ArgumentsFrom",
            "names": ["b", "a"],
            "blob_sha256": digest,
        })]
    );
    assert_eq!(
        std::fs::read(scratch.store.join("blobs").join(&digest)).unwrap(),
        printed.as_bytes()
    );
    assert_eq!(report["names"], serde_json::json!(["b", "a"]));
    assert_eq!(report["source"], "ArgumentsFrom");
    assert_eq!(report["blob_sha256"], digest.as_str());
    assert_eq!(report["children"], serde_json::json!(["root.b", "root.a"]));
}

// ---------------------------------------------------------------------------------------------
// Resuming a cut opening

/// One step of [`EPICS`] in [`ROOT`] that ends in an effect error; the case revision stays.
fn cut(governor: &Governor, scratch: &Scratch) {
    let root = CaseId(ROOT.to_owned());
    let revision = governor.current_revision(&root).unwrap();
    let document = document(scratch, "roadmap.yaml");
    let effects = port(governor, scratch, ROOT, &document, "roadmap.yaml");
    let error = try_step(governor, &effects, ROOT, EPICS).expect_err("the step is cut");
    assert!(
        format!("{error:?}").contains("failed (the process died)"),
        "{error:?}"
    );
    assert_eq!(governor.current_revision(&root).unwrap(), revision);
}

#[test]
fn an_opening_cut_before_its_children_resumes_without_a_second_run() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    governor.store().fail_next_insert_of("root.a");
    cut(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT]);
    open_epics(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.a", "root.b"]);
    assert_eq!(scratch.runs("list-epics"), 1);
    assert_eq!(openings(&governor).len(), 1);
}

#[test]
fn an_opening_cut_after_its_children_opens_none_twice() {
    let scratch = Scratch::new();
    let governor = governor(&scratch);
    governor.store().fail_next_update_of(ROOT);
    cut(&governor, &scratch);
    open_epics(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.a", "root.b"]);
    assert_eq!(scratch.runs("list-epics"), 1);
}

/// `A/bin/list-epics` as a stub that prints `["a", "b"]` on its first run and `["c"]` on every
/// later one.
fn changing_list(scratch: &Scratch) {
    let count = scratch.count_file("list-epics");
    scratch.counting(
        "list-epics",
        &format!(
            "if [ \"$(/usr/bin/wc -l < '{}')\" -le 1 ]; then printf '%s' '{{\"epics\": [\"a\", \"b\"]}}'; else printf '%s' '{{\"epics\": [\"c\"]}}'; fi\n",
            count.display()
        ),
    );
}

#[test]
fn a_list_once_observed_is_the_list_at_that_revision() {
    let scratch = Scratch::new();
    changing_list(&scratch);
    let governor = governor(&scratch);
    governor.store().fail_next_insert_of("root.a");
    cut(&governor, &scratch);
    open_epics(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.a", "root.b"]);
    assert_eq!(scratch.runs("list-epics"), 1);
}

#[test]
fn a_list_is_not_reused_at_another_case_revision() {
    let scratch = Scratch::new();
    changing_list(&scratch);
    let governor = governor(&scratch);
    governor.store().fail_next_insert_of("root.a");
    cut(&governor, &scratch);
    let root = CaseId(ROOT.to_owned());
    let before = governor.current_revision(&root).unwrap();
    {
        let document = document(&scratch, "roadmap.yaml");
        let notes = ProgramEffects::new(
            &governor,
            root.clone(),
            &document,
            &scratch.workspace,
            &scratch.store,
            Some(flag_runner()),
        )
        .unwrap();
        step(&governor, &notes, ROOT, NOTE);
    }
    assert_ne!(governor.current_revision(&root).unwrap(), before);
    open_epics(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.c"]);
    assert_eq!(scratch.runs("list-epics"), 2);
}

// ---------------------------------------------------------------------------------------------
// Beside the bindings file

#[test]
fn an_open_arguments_program_beside_the_bindings_file_runs() {
    let scratch = Scratch::new();
    scratch.prints("list-epics", r#"{"epics": ["beside"]}"#);
    scratch.stub(
        "B/bin/list-epics",
        "printf '%s' '{\"epics\": [\"workspace\"]}'\n",
    );
    let governor = governor(&scratch);
    open_epics(&governor, &scratch);
    assert_eq!(tree(&governor, ROOT), [ROOT, "root.beside"]);
}
