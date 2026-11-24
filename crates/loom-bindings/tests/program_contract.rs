//! Acceptance for `story:bound-program-contract`: a bound program can be named beside its bindings
//! file (`program_root: bindings`, `resolve_programs`), sees its run's timeout in its environment
//! (`LOOM_TIMEOUT_SECONDS`), and an exit its binding classes as a program error or as not called
//! (`exits`) records nothing and is not counted. For `story:program-error-carries-its-reason`, a
//! program error's refusal and its run's observation (`reason`) carry its stderr tail, walked for
//! credentials and bounded.
//!
//! Every step reaches the program effect port through Commission's own loop
//! (`run_until_blocked`), a scripted executor and a `CanonGovernor` over a `MemoryCaseStore`, with
//! a fixture protocol of two artifacts and stub programs the test writes under
//! `CARGO_TARGET_TMPDIR`; no model, no network. A case that starts a process gives the port a
//! `local` runner opened with `UNCONFINED_FLAG_REASON`; a case that reads the `ChildCommand` the
//! port builds gives it a recording runner that starts nothing. The bindings file sits in directory
//! `A`, the workspace is directory `B`, and the process's working directory is a third one.
//!
//! The names a child case reads (`LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME`) come from a tree of
//! four fixture protocols, each level opening the next through `CanonGovernor::open_children`.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use b10x_loom_bindings::{
    PROGRAM_SOURCE, ProgramEffects, RETRY_FEEDBACK_VARIABLE, TIMEOUT_VARIABLE, check, decode,
    resolve_programs, run_handoff_check,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ObservationData, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, ChildCommand, ChildOutput, ConfinementAdapter, ConfinementError,
    ConfinementRefusal, EngineConfinement, EngineRunner, TestExecution, TestRunner,
};
use b10x_loom_intake_slice::executor::TestCommand;
use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason};
use loom_governor::{
    CanonGovernor, CaseIndependence, CaseInputs, ChildOpening, FallibleCaseStore, FileCaseStore,
    MemoryCaseStore,
};

const REGISTRATION: &str = "fixture@1";
/// The root case.
const ROOT: &str = "P";
/// The writer every step proposes.
const CHECK: &str = "spec.check";

const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: fixture
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  gate: {}
  review: {}
actions:
  spec.check:
    effect: write
    may_produce:
      - evidence: gate
  impl.write:
    effect: write
    may_produce:
      - evidence: review
  impl.sign:
    may_produce:
      - evidence: review
  impl.open:
    may_produce:
      - evidence: review
  impl.copy:
    may_produce:
      - evidence: review
";

/// The table every exit case shares.
const VALID_TABLE: &str = r#"{"0": valid, "*": invalid}"#;

// ---------------------------------------------------------------------------------------------
// Fixture

/// The fixture protocol, parsed once for the binary (a few milliseconds).
fn protocol() -> &'static b10x_canon::model::Protocol {
    static PARSED: OnceLock<b10x_canon::model::Protocol> = OnceLock::new();
    PARSED.get_or_init(|| b10x_canon::model::parse(PROTOCOL).expect("the fixture protocol parses"))
}

fn revisions() -> BTreeMap<String, String> {
    [
        ("spec".to_owned(), "s1".to_owned()),
        ("impl".to_owned(), "i1".to_owned()),
    ]
    .into()
}

/// A governor holding the root case [`ROOT`].
fn governor() -> CanonGovernor<MemoryCaseStore> {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers");
    governor
        .open_case(
            CaseId(ROOT.to_owned()),
            REGISTRATION,
            revisions(),
            CaseInputs::none(),
        )
        .expect("the case opens");
    governor
}

/// The text of a document with the top-level `head` lines (each ending in a line break) and
/// `actions`.
fn text(head: &str, actions: &str) -> String {
    format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\n{head}artifacts:\n  spec: docs/spec\n  impl: src\nactions:\n{actions}"
    )
}

/// The document of `head` and `actions`, decoded and checked against the fixture protocol.
fn document(head: &str, actions: &str) -> BindingsDocument {
    let text = text(head, actions);
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(&document, REGISTRATION, protocol())
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// The refusal decoding, then checking, `head` and `actions` earns.
fn refused(head: &str, actions: &str) -> loom::bindings::BindingsRefusal {
    let text = text(head, actions);
    match decode(&text) {
        Err(refusal) => refusal.0,
        Ok(document) => {
            check(&document, REGISTRATION, protocol())
                .expect_err(&format!("the document is refused\n{text}"))
                .0
        }
    }
}

const BESIDE: &str = "program_root: bindings\n";

/// [`CHECK`] bound to `argv`, with the extra action `members` (each line indented four spaces)
/// and one `gate` entry read from the exit through `table`.
fn bound(argv: &[&str], members: &str, table: &str) -> String {
    let argv = serde_json::to_string(argv).unwrap();
    format!(
        "  - action: {CHECK}\n    executor: {{kind: program, argv: {argv}}}\n{members}    evidence:\n      - {{kind: gate, subject: spec, producer: ess, from: exit, results: {table}, upstream: [impl]}}\n"
    )
}

/// The scratch of one case: `A` (the bindings file's directory), `B` (the workspace), the store
/// directory and a directory outside both.
struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
    bindings: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("program-contract-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let root = dir.path().canonicalize().unwrap();
        let bindings = root.join("A");
        let workspace = root.join("B");
        let store = root.join("store");
        for directory in [&bindings, &workspace, &store] {
            std::fs::create_dir(directory).unwrap();
        }
        let cwd = std::env::current_dir().unwrap();
        assert!(cwd != bindings && cwd != workspace, "a third directory");
        Self {
            _dir: dir,
            root,
            bindings,
            workspace,
            store,
        }
    }

    /// An executable `/bin/sh` script at `path` under the scratch, holding `body`; its absolute
    /// path.
    fn script(&self, path: &str, body: &str) -> PathBuf {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A script that counts its runs in the file `count` beside it and exits `codes[n]` on its
    /// `n`th run (from 0), the last code on every later one.
    fn counting(&self, path: &str, codes: &[u8]) -> String {
        let count = self.root.join("count");
        let mut body = format!(
            "n=$(/usr/bin/cat '{count}' 2>/dev/null || echo 0)\necho $((n + 1)) > '{count}'\n",
            count = count.display()
        );
        for (index, code) in codes.iter().enumerate() {
            body.push_str(&format!("[ \"$n\" -eq {index} ] && exit {code}\n"));
        }
        body.push_str(&format!("exit {}\n", codes.last().unwrap()));
        path_text(&self.script(path, &body))
    }

    fn blob(&self, digest: &str) -> Vec<u8> {
        std::fs::read(self.store.join("blobs").join(digest)).expect("the blob is kept")
    }
}

fn path_text(path: &Path) -> String {
    path.to_str().unwrap().to_owned()
}

/// The host runner of a case that starts a process: `local`, opened with the flag's reason.
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

/// A runner that starts nothing: it keeps each child and answers, in order, the standard outputs
/// it was given (exit 0), then nothing.
#[derive(Default)]
struct Recording {
    answers: Mutex<Vec<Vec<u8>>>,
    exits: Mutex<Vec<i32>>,
    children: Mutex<Vec<ChildCommand>>,
}

impl Recording {
    fn new(answers: &[&[u8]]) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(answers.iter().rev().map(|bytes| bytes.to_vec()).collect()),
            exits: Mutex::default(),
            children: Mutex::default(),
        })
    }

    /// A runner whose runs end with `codes` in turn, then with 0, printing nothing.
    fn exiting(codes: &[i32]) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::default(),
            exits: Mutex::new(codes.iter().rev().copied().collect()),
            children: Mutex::default(),
        })
    }

    fn children(&self) -> Vec<ChildCommand> {
        self.children.lock().unwrap().clone()
    }
}

impl TestRunner for Recording {
    fn run(&self, _: &TestCommand, _: &Path) -> Result<TestExecution, ConfinementError> {
        Err(ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: "runs no test command".to_owned(),
        })
    }

    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.children.lock().unwrap().push(child.clone());
        let stdout = self.answers.lock().unwrap().pop().unwrap_or_default();
        let exit = self.exits.lock().unwrap().pop().unwrap_or(0);
        Ok(TestExecution {
            exit_code: Some(exit),
            timed_out: false,
            output: String::new(),
            applied: AppliedConfinement {
                backend: Backend::Substrate,
                profile_digest: String::new(),
                capability_snapshot: String::new(),
                cgroup: String::new(),
                filesystem: "scoped".to_owned(),
                network: None,
                profile: "bindings".to_owned(),
                writable_scopes: Vec::new(),
                read_only_roots: Vec::new(),
                read_only_files: Vec::new(),
                workdir: None,
                adapter: None,
                facts: None,
                local_reason: None,
            },
            driver_record: None,
            child_output: ChildOutput {
                stdout,
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
        })
    }

    fn preflight_child(&self, _: &Path) -> Result<(), ConfinementError> {
        Ok(())
    }
}

fn port<'a, S: FallibleCaseStore>(
    governor: &'a CanonGovernor<S>,
    document: &'a BindingsDocument,
    scratch: &Scratch,
    case: &str,
    runner: Arc<dyn TestRunner>,
) -> ProgramEffects<'a, S> {
    ProgramEffects::new(
        governor,
        CaseId(case.to_owned()),
        document,
        &scratch.workspace,
        &scratch.store,
        Some(runner),
    )
    .expect("the port is built")
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn uuid() -> Uuid {
    Uuid(format!(
        "00000000-0000-4000-8000-{:012x}",
        ISSUED.fetch_add(1, Ordering::Relaxed)
    ))
}

/// One step: the executor proposes nothing else.
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

/// Runs one step of `case` in which the executor proposes [`CHECK`] with `arguments`.
fn drive<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &ProgramEffects<'_, S>,
    case: &str,
    arguments: &str,
) -> LoopEnd {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid()),
        agent_revision_id: AgentRevisionId(uuid()),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: CHECK.to_owned(),
            arguments: ProposedActionArguments(json::parse(arguments).unwrap()),
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
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

fn only_effect(end: &LoopEnd) -> &EffectOutcome {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    &end.effects[0]
}

fn assert_performed(end: &LoopEnd) {
    match only_effect(end) {
        EffectOutcome::Performed(_) => {}
        other => panic!("not performed: {other:?}"),
    }
}

/// The reason the port refused the step with, and whether it ends the run.
fn refusal(end: &LoopEnd) -> (String, bool) {
    match only_effect(end) {
        EffectOutcome::Refused(refused) => (refused.reason.clone(), refused.ends_run),
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

/// The runs the port observed, as `loom.bindings.ProgramRun` payloads.
fn runs<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> Vec<serde_json::Value> {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation: &ObservationData| observation.source == PROGRAM_SOURCE)
        .map(|observation| plain(&observation.payload))
        .collect()
}

fn records<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str) -> Vec<EvidenceData> {
    governor.evidence(&CaseId(case.to_owned())).unwrap()
}

/// The `result` of each record of `case`.
fn results<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str) -> Vec<String> {
    records(governor, case)
        .iter()
        .map(|record| plain(&record.facts)["result"].as_str().unwrap().to_owned())
        .collect()
}

/// The blob of the last run observed.
fn last_blob<S: FallibleCaseStore>(governor: &CanonGovernor<S>, scratch: &Scratch) -> Vec<u8> {
    let runs = runs(governor);
    let digest = runs.last().expect("a run")["blob_sha256"]
        .as_str()
        .expect("the run names a blob")
        .to_owned();
    scratch.blob(&digest)
}

/// The environment `PATH` (at the host's value), `LOOM_CASE` = `case`, `LOOM_TIMEOUT_SECONDS` =
/// `seconds` and the `more` variables.
fn environment(case: &str, seconds: u64, more: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut expected = BTreeMap::new();
    if let Ok(path) = std::env::var("PATH") {
        expected.insert("PATH".to_owned(), path);
    }
    expected.insert("LOOM_CASE".to_owned(), case.to_owned());
    expected.insert(TIMEOUT_VARIABLE.to_owned(), seconds.to_string());
    for (name, value) in more {
        expected.insert((*name).to_owned(), (*value).to_owned());
    }
    expected
}

/// The lines of [`environment`], as `/usr/bin/env` prints them.
fn environment_lines(case: &str, seconds: u64, more: &[(&str, &str)]) -> BTreeSet<String> {
    environment(case, seconds, more)
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect()
}

/// What `/usr/bin/env`, bound to [`CHECK`] with the extra `members` and run for [`ROOT`] under the
/// flag's runner, prints: its lines as a set.
fn printed_environment(members: &str) -> BTreeSet<String> {
    let scratch = Scratch::new();
    let document = document("", &bound(&["/usr/bin/env"], members, VALID_TABLE));
    let governor = governor();
    let effects = port(&governor, &document, &scratch, ROOT, flag_runner());
    assert_performed(&drive(&governor, &effects, ROOT, "{}"));
    String::from_utf8(last_blob(&governor, &scratch))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The children a recording runner received for one performed step of [`ROOT`] whose binding
/// has the extra `members`, for a request with `arguments`, the runner answering `answers`.
fn recorded(members: &str, arguments: &str, answers: &[&[u8]]) -> Vec<ChildCommand> {
    let scratch = Scratch::new();
    let document = document("", &bound(&["/opt/stub/program"], members, VALID_TABLE));
    let governor = governor();
    let runner = Recording::new(answers);
    let effects = port(
        &governor,
        &document,
        &scratch,
        ROOT,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, ROOT, arguments));
    runner.children()
}

// ---------------------------------------------------------------------------------------------
// Programs beside the bindings file

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_beside_the_bindings_file_runs() {
    let scratch = Scratch::new();
    scratch.script("A/bin/stub", "echo beside\n");
    scratch.script("B/bin/stub", "echo workspace\n");
    let document = document(BESIDE, &bound(&["bin/stub"], "", VALID_TABLE));
    let resolved = resolve_programs(&document, &scratch.bindings).expect("the program resolves");
    assert_eq!(
        resolved.actions[0].executor.argv.as_deref().unwrap()[0],
        path_text(&scratch.bindings.join("bin/stub"))
    );
    let governor = governor();
    let effects = port(&governor, &resolved, &scratch, ROOT, flag_runner());
    assert_performed(&drive(&governor, &effects, ROOT, "{}"));
    assert_eq!(last_blob(&governor, &scratch), b"beside\n");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_relative_program_is_a_workspace_path_by_default() {
    let scratch = Scratch::new();
    scratch.script("A/bin/stub", "echo beside\n");
    scratch.script("B/bin/stub", "echo workspace\n");
    let document = document("", &bound(&["bin/stub"], "", VALID_TABLE));
    let resolved = resolve_programs(&document, &scratch.bindings).expect("nothing to resolve");
    assert_eq!(resolved, document);
    let governor = governor();
    let effects = port(&governor, &resolved, &scratch, ROOT, flag_runner());
    assert_performed(&drive(&governor, &effects, ROOT, "{}"));
    assert_eq!(last_blob(&governor, &scratch), b"workspace\n");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_arguments_program_beside_the_bindings_file_runs() {
    let scratch = Scratch::new();
    scratch.script("A/bin/args", "echo '{\"from\": \"beside\"}'\n");
    scratch.script("B/bin/args", "echo '{\"from\": \"workspace\"}'\n");
    let document = document(
        BESIDE,
        &bound(
            &["/usr/bin/cat"],
            "    arguments_from: [bin/args]\n",
            VALID_TABLE,
        ),
    );
    let resolved = resolve_programs(&document, &scratch.bindings).expect("the program resolves");
    assert_eq!(
        resolved.actions[0].arguments_from.as_deref().unwrap()[0],
        path_text(&scratch.bindings.join("bin/args"))
    );
    let governor = governor();
    let effects = port(&governor, &resolved, &scratch, ROOT, flag_runner());
    assert_performed(&drive(&governor, &effects, ROOT, "{}"));
    assert_eq!(last_blob(&governor, &scratch), br#"{"from":"beside"}"#);
}

/// An `open` binding of `impl.open` taking its names from `arguments_from: [argv0]`.
fn open_binding(argv0: &str) -> String {
    format!(
        "  - action: impl.open\n    executor: {{kind: open, each: names, bindings: child.yaml}}\n    arguments_from: [{argv0}]\n"
    )
}

#[test]
fn an_open_bindings_arguments_program_is_resolved_too() {
    let scratch = Scratch::new();
    scratch.script("A/bin/list", "echo '{\"names\": [\"a\"]}'\n");
    scratch.script("bin/list", "echo '{\"names\": [\"b\"]}'\n");
    let leaving = decode(&text(BESIDE, &open_binding("../bin/list"))).unwrap();
    let refusal = resolve_programs(&leaving, &scratch.bindings)
        .expect_err("a path out of A is refused")
        .0;
    assert_eq!(refusal.reason, Reason::ProgramNotBesideBindings);
    assert_eq!(refusal.action.as_deref(), Some("impl.open"));
    assert_eq!(refusal.member, "arguments_from[0]");
    let beside = decode(&text(BESIDE, &open_binding("bin/list"))).unwrap();
    let resolved = resolve_programs(&beside, &scratch.bindings).expect("the program resolves");
    assert_eq!(
        resolved.actions[0].arguments_from.as_deref().unwrap(),
        [path_text(&scratch.bindings.join("bin/list"))]
    );
}

/// The refusal `resolve_programs` gives [`CHECK`] bound to `argv0` beside the bindings file.
fn not_beside(scratch: &Scratch, argv0: &str) -> loom::bindings::BindingsRefusal {
    let document = document(BESIDE, &bound(&[argv0], "", VALID_TABLE));
    let refusal = resolve_programs(&document, &scratch.bindings)
        .expect_err("the program is refused")
        .0;
    assert_eq!(
        refusal.reason,
        Reason::ProgramNotBesideBindings,
        "{refusal:?}"
    );
    assert_eq!(refusal.action.as_deref(), Some(CHECK));
    assert_eq!(refusal.member, "executor.argv[0]");
    refusal
}

#[test]
fn a_program_path_leaving_the_bindings_directory_is_refused() {
    let scratch = Scratch::new();
    scratch.script("bin/stub", "echo outside\n");
    not_beside(&scratch, "../bin/stub");
    not_beside(&scratch, "bin/../../bin/stub");
}

#[test]
fn a_linked_program_outside_the_bindings_directory_is_refused() {
    let scratch = Scratch::new();
    let outside = scratch.script("outside/stub", "echo outside\n");
    std::fs::create_dir_all(scratch.bindings.join("bin")).unwrap();
    std::os::unix::fs::symlink(&outside, scratch.bindings.join("bin/stub")).unwrap();
    not_beside(&scratch, "bin/stub");
    // A directory link out of A is the same.
    std::os::unix::fs::symlink(scratch.root.join("outside"), scratch.bindings.join("out")).unwrap();
    not_beside(&scratch, "out/stub");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_missing_program_beside_the_bindings_file_is_refused() {
    let scratch = Scratch::new();
    scratch.script("B/bin/absent", "echo workspace\n");
    not_beside(&scratch, "bin/absent");
    std::fs::create_dir_all(scratch.bindings.join("bin/directory")).unwrap();
    not_beside(&scratch, "bin/directory");
    // An unresolved document whose programs are beside its file starts nothing.
    let document = document(BESIDE, &bound(&["bin/absent"], "", VALID_TABLE));
    let governor = governor();
    let effects = port(&governor, &document, &scratch, ROOT, flag_runner());
    let (reason, _) = refusal(&drive(&governor, &effects, ROOT, "{}"));
    assert!(reason.contains("resolve_programs"), "{reason}");
    assert!(runs(&governor).is_empty());
}

#[test]
fn an_unknown_program_root_is_refused() {
    let refusal = refused(
        "program_root: elsewhere\n",
        &bound(&["bin/stub"], "", VALID_TABLE),
    );
    assert_eq!(refusal.reason, Reason::Malformed);
    assert_eq!(refusal.member, "program_root");
}

// ---------------------------------------------------------------------------------------------
// The run's timeout in its environment

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_starts_with_its_timeout_in_its_environment() {
    assert_eq!(TIMEOUT_VARIABLE, "LOOM_TIMEOUT_SECONDS");
    assert_eq!(
        printed_environment("    timeout_seconds: 7\n"),
        environment_lines(ROOT, 7, &[])
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_without_a_timeout_reads_the_default() {
    assert_eq!(printed_environment(""), environment_lines(ROOT, 60, &[]));
}

#[test]
fn an_arguments_program_starts_with_its_timeout_in_its_environment() {
    let children = recorded(
        "    timeout_seconds: 7\n    arguments_from: [/opt/stub/args]\n",
        "{}",
        &[b"{}"],
    );
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].program(), "/opt/stub/args");
    assert_eq!(children[0].environment().set, environment(ROOT, 7, &[]));
    assert!(children[0].environment().allow.is_empty());
    assert_eq!(children[1].environment().set, environment(ROOT, 7, &[]));
}

#[test]
fn an_argument_named_loom_timeout_seconds_does_not_change_the_variable() {
    let arguments = r#"{"LOOM_TIMEOUT_SECONDS": "3600"}"#;
    let children = recorded("    timeout_seconds: 7\n", arguments, &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].environment().set, environment(ROOT, 7, &[]));
    let stdin: serde_json::Value = serde_json::from_slice(children[0].stdin().unwrap()).unwrap();
    assert_eq!(stdin, serde_json::json!({"LOOM_TIMEOUT_SECONDS": "3600"}));
}

// ---------------------------------------------------------------------------------------------
// A timeout above an hour (story:bound-program-timeout-above-an-hour)

#[test]
fn the_declared_timeout_reaches_the_child() {
    let children = recorded("    timeout_seconds: 14400\n", "{}", &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].timeout().as_secs(), 14400);
    assert_eq!(children[0].environment().set, environment(ROOT, 14400, &[]));
    let children = recorded("", "{}", &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].timeout().as_secs(), 60);
    assert_eq!(children[0].environment().set, environment(ROOT, 60, &[]));
}

#[test]
fn helper_programs_keep_the_hour() {
    let members = "    timeout_seconds: 14400\n    arguments_from: [/opt/stub/args]\n    handoff: Look at it.\n    handoff_when: [/opt/stub/check]\n";
    let children = recorded(members, "{}", &[b"{}"]);
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].program(), "/opt/stub/args");
    assert_eq!(children[0].timeout().as_secs(), 3600);
    assert_eq!(children[0].environment().set, environment(ROOT, 3600, &[]));
    assert_eq!(children[1].program(), "/opt/stub/program");
    assert_eq!(children[1].timeout().as_secs(), 14400);

    let scratch = Scratch::new();
    let document = document("", &bound(&["/opt/stub/program"], members, VALID_TABLE));
    let governor = governor();
    let runner = Recording::new(&[]);
    let check = run_handoff_check(
        &governor,
        &CaseId(ROOT.to_owned()),
        &document,
        &document.actions[0],
        &scratch.workspace,
        &scratch.store,
        &(Arc::clone(&runner) as Arc<dyn TestRunner>),
        None,
    )
    .expect("the binding carries a check");
    assert!(check.refusal.is_none(), "{:?}", check.refusal);
    let children = runner.children();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].program(), "/opt/stub/check");
    assert_eq!(children[0].timeout().as_secs(), 3600);
}

// ---------------------------------------------------------------------------------------------
// Exit classes

/// Runs the script `body` bound to [`CHECK`] with the extra `members` and `table` for [`ROOT`],
/// `steps` times on one port; the governor, the scratch and each step's end.
fn exit_steps(
    program: impl Fn(&Scratch) -> String,
    members: &str,
    table: &str,
    steps: usize,
) -> (CanonGovernor<MemoryCaseStore>, Scratch, Vec<LoopEnd>) {
    let scratch = Scratch::new();
    let program = program(&scratch);
    let document = document("", &bound(&[&program], members, table));
    let governor = governor();
    let ends = {
        let effects = port(&governor, &document, &scratch, ROOT, flag_runner());
        (0..steps)
            .map(|_| drive(&governor, &effects, ROOT, "{}"))
            .collect()
    };
    (governor, scratch, ends)
}

/// A script that exits `code`.
fn exiting(code: u8) -> impl Fn(&Scratch) -> String {
    move |scratch: &Scratch| path_text(&scratch.script("stub", &format!("exit {code}\n")))
}

const PROGRAM_ERRORS: &str = "    exits: {error: [126, 127, 75]}\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn without_exit_classes_every_exit_goes_through_the_table() {
    let (governor, _scratch, ends) = exit_steps(exiting(127), "", VALID_TABLE, 1);
    assert_performed(&ends[0]);
    assert_eq!(results(&governor, ROOT), ["invalid"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_error_exit_records_nothing_and_ends_the_run() {
    let (governor, scratch, ends) = exit_steps(exiting(127), PROGRAM_ERRORS, VALID_TABLE, 1);
    let (reason, ends_run) = refusal(&ends[0]);
    assert!(reason.starts_with("ProgramError"), "{reason}");
    assert!(reason.contains("127"), "{reason}");
    assert!(ends_run, "a program error ends the run");
    assert!(records(&governor, ROOT).is_empty());
    let runs = runs(&governor);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["class"], "error");
    let digest = runs[0]["blob_sha256"].as_str().unwrap();
    assert!(reason.contains(digest), "{reason}");
    assert!(scratch.store.join("blobs").join(digest).is_file());
    assert!(
        matches!(ends[0].outcome, RunOutcome::EffectRefused(_)),
        "{:?}",
        ends[0].outcome
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_error_exit_does_not_count_against_max_runs() {
    let members = format!("{PROGRAM_ERRORS}    budget: {{max_runs: 2}}\n");
    let (governor, _scratch, ends) = exit_steps(
        |scratch| scratch.counting("stub", &[127, 127, 127, 0]),
        &members,
        VALID_TABLE,
        4,
    );
    for end in &ends[..3] {
        assert!(refusal(end).0.starts_with("ProgramError"), "{end:?}");
    }
    assert_performed(&ends[3]);
    assert_eq!(results(&governor, ROOT), ["valid"]);
}

const NOT_CALLED: &str = "    exits: {not_called: [78]}\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_not_called_exit_records_nothing() {
    let (governor, _scratch, ends) = exit_steps(exiting(78), NOT_CALLED, VALID_TABLE, 1);
    assert_performed(&ends[0]);
    assert!(records(&governor, ROOT).is_empty());
    let runs = runs(&governor);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["class"], "not_called");
    assert_eq!(runs[0]["exit"], 78);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_not_called_run_does_not_count_against_max_runs() {
    let members = format!("{NOT_CALLED}    budget: {{max_runs: 1}}\n");
    let (governor, _scratch, ends) = exit_steps(
        |scratch| scratch.counting("stub", &[78, 0]),
        &members,
        VALID_TABLE,
        2,
    );
    assert_performed(&ends[0]);
    assert_performed(&ends[1]);
    assert_eq!(results(&governor, ROOT), ["valid"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_timeout_is_never_an_exit_class() {
    let members = "    timeout_seconds: 1\n    exits: {error: [0, 137, 143]}\n";
    let (governor, _scratch, ends) = exit_steps(
        |scratch| path_text(&scratch.script("stub", "exec /usr/bin/sleep 5\n")),
        members,
        r#"{"timeout": slow, "*": invalid}"#,
        1,
    );
    assert_performed(&ends[0]);
    assert_eq!(results(&governor, ROOT), ["slow"]);
    assert_eq!(runs(&governor)[0]["class"], serde_json::Value::Null);
}

#[test]
fn an_exit_code_outside_0_to_255_is_refused() {
    for codes in ["[256]", "[-1]", "[\"x\"]"] {
        let refusal = refused(
            "",
            &bound(
                &["bin/stub"],
                &format!("    exits: {{error: {codes}}}\n"),
                VALID_TABLE,
            ),
        );
        assert_eq!(refusal.reason, Reason::Malformed, "{codes}: {refusal:?}");
        assert_eq!(refusal.member, "exits.error[0]", "{codes}");
    }
}

#[test]
fn an_exit_in_both_classes_is_refused() {
    let refusal = refused(
        "",
        &bound(
            &["bin/stub"],
            "    exits: {error: [3], not_called: [3]}\n",
            VALID_TABLE,
        ),
    );
    assert_eq!(refusal.reason, Reason::ExitClassConflict);
    assert_eq!(refusal.action.as_deref(), Some(CHECK));
    assert_eq!(refusal.member, "exits.not_called[0]");
}

#[test]
fn an_exit_class_and_a_table_row_conflict() {
    let refusal = refused(
        "",
        &bound(
            &["bin/stub"],
            PROGRAM_ERRORS,
            r#"{"0": valid, "127": missing, "*": invalid}"#,
        ),
    );
    assert_eq!(refusal.reason, Reason::ExitClassConflict);
    assert_eq!(refusal.action.as_deref(), Some(CHECK));
    assert_eq!(refusal.member, "evidence[0].results.127");
}

#[test]
fn exit_classes_belong_to_a_program() {
    for (action, executor) in [
        ("impl.sign", "{kind: person, principal: reviewer}"),
        ("impl.write", "{kind: agent, adapter: claude-code}"),
        ("impl.open", "{kind: open, name: n, bindings: child.yaml}"),
        ("impl.copy", "{kind: copy, evidence: review, from: other}"),
    ] {
        let refusal = refused(
            "",
            &format!("  - action: {action}\n    executor: {executor}\n    exits: {{error: [1]}}\n"),
        );
        assert_eq!(refusal.reason, Reason::MemberNotForExecutor, "{action}");
        assert_eq!(refusal.action.as_deref(), Some(action));
        assert_eq!(refusal.member, "exits", "{action}");
    }
}

/// Standard output never enters a refusal or the run's observation; since
/// `story:program-error-carries-its-reason` the stderr tail does, as the tests below hold.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn program_output_never_enters_a_refusal() {
    const CANARY: &str = "canary-6f1d";
    const SAID: &str = "said-on-stderr";
    let (governor, _scratch, ends) = exit_steps(
        |scratch| {
            path_text(&scratch.script(
                "stub",
                &format!("echo {CANARY}\necho {SAID} >&2\nexit 127\n"),
            ))
        },
        PROGRAM_ERRORS,
        VALID_TABLE,
        1,
    );
    let (reason, _) = refusal(&ends[0]);
    assert!(reason.starts_with("ProgramError"), "{reason}");
    assert!(!reason.contains(CANARY), "{reason}");
    assert!(reason.ends_with(SAID), "{reason}");
    let runs = runs(&governor);
    assert_eq!(runs.len(), 1);
    assert!(!runs[0].to_string().contains(CANARY), "{}", runs[0]);
}

// ---------------------------------------------------------------------------------------------
// The reason a program error carries (`story:program-error-carries-its-reason`)

/// The `exits` member listing 79 as a program error.
const ERROR_79: &str = "    exits: {error: [79]}\n";

/// What the refusal says before the stderr tail.
const STDERR_SAID: &str = "; the last lines of its stderr:\n";

/// The note the walk stands in for one withheld line with.
const ONE_WITHHELD: &str = "[a line withheld: it holds a credential's shape]";

/// One step of [`CHECK`] bound with [`ERROR_79`] and `table` to a script that writes `stderr` on
/// its standard error, nothing on its standard output, and exits `code`: the step's end, the run's
/// observation and the results the case recorded.
fn run_writing(stderr: &str, code: u8, table: &str) -> (LoopEnd, serde_json::Value, Vec<String>) {
    let stderr = stderr.to_owned();
    let (governor, _scratch, mut ends) = exit_steps(
        move |scratch| {
            let said = scratch.root.join("stderr.txt");
            std::fs::write(&said, &stderr).unwrap();
            path_text(&scratch.script(
                "stub",
                &format!("/usr/bin/cat '{}' >&2\nexit {code}\n", said.display()),
            ))
        },
        ERROR_79,
        table,
        1,
    );
    let runs = runs(&governor);
    assert_eq!(runs.len(), 1, "{runs:?}");
    (ends.remove(0), runs[0].clone(), results(&governor, ROOT))
}

/// A run of [`run_writing`] that exits 79: the refusal text, which ends the run, and the run's
/// observation.
fn error_writing(stderr: &str) -> (String, serde_json::Value) {
    let (end, run, recorded) = run_writing(stderr, 79, VALID_TABLE);
    assert!(recorded.is_empty(), "{recorded:?}");
    let (text, ends_run) = refusal(&end);
    assert!(ends_run, "a program error ends the run: {text}");
    assert_eq!(run["class"], "error", "{run}");
    (text, run)
}

/// The `reason` of a run's observation.
fn reason_of(run: &serde_json::Value) -> &str {
    run["reason"]
        .as_str()
        .unwrap_or_else(|| panic!("the run has a reason: {run}"))
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_error_carries_its_stderr_tail() {
    let (text, run) = error_writing("no such epic: E-12\n");
    assert!(text.starts_with("ProgramError: action"), "{text}");
    assert!(
        text.ends_with("the last lines of its stderr:\nno such epic: E-12"),
        "{text}"
    );
    assert_eq!(reason_of(&run), "no such epic: E-12");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_error_withholds_a_token_line() {
    let token = ["ghp", "_abc123"].concat();
    let (text, run) = error_writing(&format!("token {token}\nfailed\n"));
    let ending = format!("{ONE_WITHHELD}\nfailed");
    for shown in [text.as_str(), reason_of(&run)] {
        assert!(shown.ends_with(&ending), "{shown}");
        assert!(!shown.contains(&token), "{shown}");
    }
    assert!(!run.to_string().contains(&token), "{run}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_error_withholds_a_cut_pem_block() {
    let body = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj";
    let mut stderr = ["-----BEGIN ", "PRIVATE", " KEY-----\n"].concat();
    while stderr.len() < 5000 {
        stderr.push_str(body);
        stderr.push('\n');
    }
    stderr.push_str(&["-----END ", "PRIVATE", " KEY-----\n"].concat());
    stderr.push_str("failed\n");
    assert!(
        stderr.len() - "failed\n".len() > 5000,
        "the block alone is over 5000 bytes"
    );
    assert!(
        stderr.len() > 4096 + "-----BEGIN PRIVATE KEY-----\n".len(),
        "the last 4096 bytes do not hold the block's opening"
    );
    let (text, run) = error_writing(&stderr);
    for shown in [text.as_str(), reason_of(&run)] {
        assert!(
            shown.ends_with(" withheld: they hold a credential's shape]\nfailed"),
            "{shown}"
        );
        assert!(!shown.contains(body), "{shown}");
        assert!(!shown.contains("PRIVATE KEY"), "{shown}");
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_long_stderr_is_cut_to_its_last_whole_lines() {
    let stderr: String = (1..=20_000).map(|n| format!("line {n}\n")).collect();
    let (text, run) = error_writing(&stderr);
    let reason = reason_of(&run);
    assert!(reason.len() <= 4096, "{}", reason.len());
    let numbers: Vec<u32> = reason
        .lines()
        .map(|line| {
            line.strip_prefix("line ")
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("a whole line: {line:?}"))
        })
        .collect();
    assert_eq!(numbers.last(), Some(&20_000), "it ends with the last line");
    assert!(
        numbers.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "whole lines in order: {numbers:?}"
    );
    assert!(
        numbers.len() > 300,
        "the bound is filled: {}",
        numbers.len()
    );
    assert!(text.ends_with(&format!("{STDERR_SAID}{reason}")), "{text}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_program_error_with_no_stderr_has_no_reason() {
    let (text, run) = error_writing("");
    let digest = run["blob_sha256"].as_str().expect("a blob");
    let program = text
        .strip_prefix(&format!("ProgramError: action {CHECK}: "))
        .and_then(|rest| rest.split_once(" ended exit status "))
        .map(|(program, _)| program.to_owned())
        .unwrap_or_else(|| panic!("{text}"));
    assert!(program.ends_with("/stub"), "{program}");
    assert_eq!(
        text,
        format!(
            "ProgramError: action {CHECK}: {program} ended exit status 79, which exits.error lists; nothing was recorded, and its output is the blob {digest}"
        )
    );
    assert!(run.get("reason").is_none(), "{run}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_run_outside_the_error_class_has_no_reason() {
    let (end, run, recorded) = run_writing("warning: slow\n", 0, r#"{"0": valid}"#);
    assert_performed(&end);
    assert_eq!(recorded, ["valid"]);
    assert_eq!(run["exit"], 0, "{run}");
    assert!(run.get("reason").is_none(), "{run}");
}

// ---------------------------------------------------------------------------------------------
// Security review of wave 2026-11-23-w8 (`story:program-error-carries-its-reason`): conformance
// cases the review added, red against `9ce6949e`.

/// The refusal says "the last lines of its stderr" and the guide says the reason is the run's
/// stderr tail. A program that writes more than the runner keeps of a stream (1 MiB, the first
/// bytes: Substrate's `Retention::Head`) and then its error must still end the reason with that
/// error, or the reason must say its stderr was cut.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w8_a_stderr_past_the_runners_bound_still_shows_its_last_line() {
    const LAST: &str = "no such epic: E-12";
    let mut stderr: String = (1..=100_000).map(|n| format!("progress {n}\n")).collect();
    assert!(stderr.len() > 1_048_576 + 4096, "{}", stderr.len());
    stderr.push_str(LAST);
    stderr.push('\n');
    let (text, run) = error_writing(&stderr);
    let reason = reason_of(&run);
    // The runner kept the start of the stream only: the reason says so in this exact text, then
    // shows the last whole lines it kept, never the line the bound cut.
    let kept = reason
        .strip_prefix(STDERR_CUT)
        .unwrap_or_else(|| panic!("the reason starts with the cut statement: {reason:?}"));
    assert!(!reason.contains(LAST), "{reason}");
    let numbers: Vec<u32> = kept
        .lines()
        .map(|line| {
            line.strip_prefix("progress ")
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("a whole kept line: {line:?}"))
        })
        .collect();
    assert!(
        numbers.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "whole lines in order: {numbers:?}"
    );
    // The window is full to within one kept line ("progress 70000\n": 15 bytes).
    assert!((4096 - 15..=4096).contains(&kept.len()), "{}", kept.len());
    assert!(
        text.ends_with(&format!("{STDERR_KEPT_SAID}{reason}")),
        "{text}"
    );
    assert!(!text.contains(STDERR_SAID), "{text}");
}

/// What the reason of a run whose stderr the runner cut says before the last kept lines.
const STDERR_CUT: &str = "[it wrote more on its standard error than the runner keeps: the runner kept the start, and what follows are the last kept lines, not its last lines]\n";

/// What the refusal says before the reason of a run whose stderr the runner cut.
const STDERR_KEPT_SAID: &str = "; the last kept lines of its stderr:\n";

/// A JSON Web Token on stderr is a bearer credential with no prefix in `TOKEN_PREFIXES`: it
/// reaches the `ProgramError` refusal, the run's observation (`reason`) and `tree status`.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn security_w8_a_jwt_on_stderr_is_withheld() {
    let jwt = [
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
        ".",
        "eyJzdWIiOiJydW5ib29rIiwiaWF0IjoxNzAwMDAwMDAwfQ",
        ".",
        "SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    ]
    .concat();
    let (text, run) = error_writing(&format!("calling the API with {jwt}\nfailed: 401\n"));
    for shown in [text.as_str(), reason_of(&run)] {
        assert!(!shown.contains(&jwt), "{shown}");
    }
}

// ---------------------------------------------------------------------------------------------
// The names the openings gave the case and its parent

/// One level of the tree's protocols: `<id>@1`, the writer [`CHECK`] and, but for the last level,
/// `next.open`, a `case_open` of the next level about `spec`.
fn level(id: &str, next: Option<&str>) -> String {
    let mut text = format!(
        "format: protocol/1\nprotocol:\n  id: {id}\n  revision: 1\nartifacts:\n  spec: {{}}\n  impl: {{}}\nevidence_kinds:\n  gate: {{}}\n  case_outcome: {{}}\nactions:\n  spec.check:\n    effect: write\n    may_produce:\n      - evidence: gate\n"
    );
    if let Some(next) = next {
        text.push_str(&format!(
            "  next.open:\n    effect: write\n    case_open: {{protocol: {next}, subject: spec}}\n    may_produce:\n      - evidence: gate\n"
        ));
    }
    text
}

/// The tree's protocols, root first: `root` opens `epic`, `epic` opens `story`, `story` opens
/// `task`.
const LEVELS: [(&str, Option<&str>); 4] = [
    ("root", Some("epic")),
    ("epic", Some("story")),
    ("story", Some("task")),
    ("task", None),
];

/// A governor over `store` holding every level and the root case [`ROOT`] of `root@1`.
fn tree_governor<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    let mut governor = CanonGovernor::new(store);
    for (id, next) in LEVELS {
        governor = governor
            .with_protocol_yaml(&format!("{id}@1"), &level(id, next))
            .unwrap_or_else(|error| panic!("{id}@1 registers: {error:?}"));
    }
    governor
        .open_case(
            CaseId(ROOT.to_owned()),
            "root@1",
            revisions(),
            CaseInputs::none(),
        )
        .expect("the root opens");
    governor
}

/// The registration of the protocol below `case`'s.
fn below<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str) -> String {
    let held = governor.protocol(&CaseId(case.to_owned())).unwrap();
    let id = held.trim_end_matches("@1");
    let (_, next) = LEVELS.iter().find(|(level, _)| *level == id).unwrap();
    format!("{}@1", next.expect("the level opens one below"))
}

/// Opens one child of `parent` per name, through `next.open`; their ids.
fn open<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    parent: &str,
    names: &[&str],
) -> Vec<String> {
    let protocol = below(governor, parent);
    let openings: Vec<ChildOpening> = names
        .iter()
        .map(|name| ChildOpening {
            name: (*name).to_owned(),
            protocol: protocol.clone(),
            independence: CaseIndependence::none(),
        })
        .collect();
    let parent = CaseId(parent.to_owned());
    governor
        .open_children(
            &parent,
            "next.open",
            governor.current_revision(&parent).unwrap(),
            &openings,
            Vec::new(),
            "2026-11-22T12:00:00Z",
        )
        .unwrap_or_else(|error| panic!("the children open: {error:?}"))
        .into_iter()
        .map(|case| case.0)
        .collect()
}

/// The document binding [`CHECK`] of the protocol `case` is held under, with the extra `members`.
fn level_document<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
    argv: &[&str],
    members: &str,
) -> BindingsDocument {
    let registration = governor.protocol(&CaseId(case.to_owned())).unwrap();
    let id = registration.trim_end_matches("@1");
    let (_, next) = LEVELS.iter().find(|(level, _)| *level == id).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {registration}\nartifacts:\n  spec: docs/spec\n  impl: src\nactions:\n{}",
        bound(argv, members, VALID_TABLE)
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    let protocol = b10x_canon::model::parse(&level(id, *next)).unwrap();
    check(&document, &registration, &protocol)
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// The children a recording runner received for one step of `case`, whose binding has the extra
/// `members`, for a request with `arguments`, the runner answering `answers`; and the step.
fn recorded_in<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
    members: &str,
    arguments: &str,
    answers: &[&[u8]],
) -> (Vec<ChildCommand>, LoopEnd) {
    let scratch = Scratch::new();
    let document = level_document(governor, case, &["/opt/stub/program"], members);
    let runner = Recording::new(answers);
    let effects = port(
        governor,
        &document,
        &scratch,
        case,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    let end = drive(governor, &effects, case, arguments);
    (runner.children(), end)
}

/// The environment the bound program of `case` is given, through a recording runner.
fn names_of<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> BTreeMap<String, String> {
    let (children, end) = recorded_in(governor, case, "", "{}", &[]);
    assert_performed(&end);
    assert_eq!(children.len(), 1);
    assert!(children[0].environment().allow.is_empty());
    children[0].environment().set.clone()
}

const NAME: &str = "LOOM_CASE_NAME";
const PARENT_NAME: &str = "LOOM_PARENT_CASE_NAME";

/// A tree `P` -> `P.epic` -> `P.epic.story` over a memory store.
fn three_levels() -> CanonGovernor<MemoryCaseStore> {
    let governor = tree_governor(MemoryCaseStore::default());
    assert_eq!(open(&governor, ROOT, &["epic"]), ["P.epic"]);
    assert_eq!(open(&governor, "P.epic", &["story"]), ["P.epic.story"]);
    governor
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_child_program_reads_the_name_its_opening_gave_it() {
    let governor = tree_governor(MemoryCaseStore::default());
    assert_eq!(open(&governor, ROOT, &["story"]), ["P.story"]);
    // A real process: exactly these lines.
    let scratch = Scratch::new();
    let document = level_document(&governor, "P.story", &["/usr/bin/env"], "");
    let effects = port(&governor, &document, &scratch, "P.story", flag_runner());
    assert_performed(&drive(&governor, &effects, "P.story", "{}"));
    let printed: BTreeSet<String> = String::from_utf8(last_blob(&governor, &scratch))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        printed,
        environment_lines("P.story", 60, &[(NAME, "story")])
    );
}

#[test]
fn a_repeated_child_reads_the_name_of_the_first() {
    let governor = tree_governor(MemoryCaseStore::default());
    assert_eq!(open(&governor, ROOT, &["story"]), ["P.story"]);
    assert_eq!(open(&governor, ROOT, &["story"]), ["P.story.2"]);
    assert_eq!(
        names_of(&governor, "P.story.2"),
        environment("P.story.2", 60, &[(NAME, "story")])
    );
}

#[test]
fn a_root_program_reads_neither_name() {
    let governor = three_levels();
    assert_eq!(names_of(&governor, ROOT), environment(ROOT, 60, &[]));
}

#[test]
fn a_child_of_the_root_reads_its_name_and_no_parent_name() {
    let governor = three_levels();
    assert_eq!(
        names_of(&governor, "P.epic"),
        environment("P.epic", 60, &[(NAME, "epic")])
    );
}

#[test]
fn a_grandchild_reads_its_name_and_its_parents_name() {
    let governor = three_levels();
    assert_eq!(
        names_of(&governor, "P.epic.story"),
        environment(
            "P.epic.story",
            60,
            &[(NAME, "story"), (PARENT_NAME, "epic")]
        )
    );
}

#[test]
fn a_child_of_a_repeated_parent_reads_its_own_name_and_the_first_parents_name() {
    let governor = three_levels();
    assert_eq!(open(&governor, ROOT, &["epic"]), ["P.epic.2"]);
    assert_eq!(open(&governor, "P.epic.2", &["story"]), ["P.epic.2.story"]);
    assert_eq!(
        names_of(&governor, "P.epic.2.story"),
        environment(
            "P.epic.2.story",
            60,
            &[(NAME, "story"), (PARENT_NAME, "epic")]
        )
    );
}

#[test]
fn the_parent_name_is_one_level_up_and_no_further() {
    let governor = three_levels();
    assert_eq!(
        open(&governor, "P.epic.story", &["task"]),
        ["P.epic.story.task"]
    );
    assert_eq!(
        names_of(&governor, "P.epic.story.task"),
        environment(
            "P.epic.story.task",
            60,
            &[(NAME, "task"), (PARENT_NAME, "story")]
        )
    );
}

#[test]
fn a_repeat_at_the_third_level_reads_the_names_of_the_first() {
    let governor = three_levels();
    assert_eq!(open(&governor, "P.epic", &["story"]), ["P.epic.story.2"]);
    assert_eq!(open(&governor, ROOT, &["epic"]), ["P.epic.2"]);
    assert_eq!(open(&governor, "P.epic.2", &["story"]), ["P.epic.2.story"]);
    for case in ["P.epic.story.2", "P.epic.2.story"] {
        assert_eq!(
            names_of(&governor, case),
            environment(case, 60, &[(NAME, "story"), (PARENT_NAME, "epic")]),
            "{case}"
        );
    }
}

#[test]
fn an_arguments_program_reads_both_names() {
    let governor = three_levels();
    let (children, end) = recorded_in(
        &governor,
        "P.epic.story",
        "    arguments_from: [/opt/stub/args]\n",
        "{}",
        &[b"{}"],
    );
    assert_performed(&end);
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].program(), "/opt/stub/args");
    assert_eq!(
        children[0].environment().set,
        environment(
            "P.epic.story",
            60,
            &[(NAME, "story"), (PARENT_NAME, "epic")]
        )
    );
}

#[test]
fn each_child_of_an_each_opening_reads_its_entry() {
    let governor = tree_governor(MemoryCaseStore::default());
    assert_eq!(open(&governor, ROOT, &["a", "b"]), ["P.a", "P.b"]);
    for (case, name) in [("P.a", "a"), ("P.b", "b")] {
        assert_eq!(
            names_of(&governor, case),
            environment(case, 60, &[(NAME, name)])
        );
    }
}

#[test]
fn an_arguments_program_reads_the_case_name() {
    let governor = tree_governor(MemoryCaseStore::default());
    open(&governor, ROOT, &["story"]);
    let (children, end) = recorded_in(
        &governor,
        "P.story",
        "    arguments_from: [/opt/stub/args]\n",
        "{}",
        &[b"{}"],
    );
    assert_performed(&end);
    assert_eq!(
        children[0].environment().set,
        environment("P.story", 60, &[(NAME, "story")])
    );
}

#[test]
fn an_argument_named_loom_case_name_does_not_change_the_variable() {
    let governor = tree_governor(MemoryCaseStore::default());
    open(&governor, ROOT, &["story"]);
    let arguments = r#"{"LOOM_CASE_NAME": "x"}"#;
    let (children, end) = recorded_in(&governor, "P.story", "", arguments, &[]);
    assert_performed(&end);
    assert_eq!(children[0].environment().set[NAME], "story");
    let stdin: serde_json::Value = serde_json::from_slice(children[0].stdin().unwrap()).unwrap();
    assert_eq!(stdin, serde_json::json!({"LOOM_CASE_NAME": "x"}));
    // An arguments program that prints the member changes nothing either.
    let (children, end) = recorded_in(
        &governor,
        "P.story",
        "    arguments_from: [/opt/stub/args]\n",
        "{}",
        &[br#"{"LOOM_CASE_NAME": "x"}"#],
    );
    assert_performed(&end);
    assert_eq!(children[1].environment().set[NAME], "story");
}

#[test]
fn an_argument_named_loom_parent_case_name_does_not_change_the_variable() {
    let governor = three_levels();
    let arguments = r#"{"LOOM_PARENT_CASE_NAME": "x"}"#;
    let (children, end) = recorded_in(&governor, "P.epic.story", "", arguments, &[]);
    assert_performed(&end);
    assert_eq!(children[0].environment().set[PARENT_NAME], "epic");
    let (children, end) = recorded_in(
        &governor,
        "P.epic.story",
        "    arguments_from: [/opt/stub/args]\n",
        "{}",
        &[br#"{"LOOM_PARENT_CASE_NAME": "x"}"#],
    );
    assert_performed(&end);
    assert_eq!(children[1].environment().set[PARENT_NAME], "epic");
}

/// The step of `case` refused before anything started: its reason, after checking the runner
/// received no child and the port kept no run.
fn refused_before_anything<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    case: &str,
) -> String {
    let (children, end) = recorded_in(governor, case, "", "{}", &[]);
    assert!(children.is_empty(), "{children:?}");
    assert!(runs(governor).is_empty(), "{:?}", runs(governor));
    assert!(records(governor, case).is_empty());
    refusal(&end).0
}

#[test]
fn a_stored_name_that_is_not_a_child_name_starts_nothing() {
    for name in ["a b".to_owned(), "a".repeat(4097)] {
        let governor = tree_governor(MemoryCaseStore::default());
        open(&governor, ROOT, &["story"]);
        governor
            .store()
            .update(&CaseId("P.story".to_owned()), |state| {
                state.parent.as_mut().unwrap().name = name.clone();
            })
            .unwrap();
        let reason = refused_before_anything(&governor, "P.story");
        assert_eq!(
            reason,
            "/opt/stub/program was not started: the case name is not a child name"
        );
    }
    // A file store refuses such a link when it reads the case: the step fails and starts nothing.
    let scratch = Scratch::new();
    let governor = tree_governor(FileCaseStore::open(&scratch.store).unwrap());
    open(&governor, ROOT, &["story"]);
    rewrite_case(&scratch.store, "P.story", |case| {
        case["parent"]["name"] = serde_json::json!("a b");
    });
    let document = level_document(&governor, ROOT, &["/opt/stub/program"], "");
    let runner = Recording::new(&[]);
    let effects = port(
        &governor,
        &document,
        &scratch,
        ROOT,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    drop(effects);
    assert!(governor.case_state(&CaseId("P.story".to_owned())).is_err());
}

#[test]
fn a_stored_parent_name_that_is_not_a_child_name_starts_nothing() {
    for name in ["a b".to_owned(), "a".repeat(4097)] {
        let governor = three_levels();
        governor
            .store()
            .update(&CaseId("P.epic".to_owned()), |state| {
                state.parent.as_mut().unwrap().name = name.clone();
            })
            .unwrap();
        let reason = refused_before_anything(&governor, "P.epic.story");
        assert_eq!(
            reason,
            "/opt/stub/program was not started: the parent case name is not a child name"
        );
    }
}

/// Rewrites the case file of `case` under the file store `store` with `change`.
fn rewrite_case(store: &Path, case: &str, change: impl FnOnce(&mut serde_json::Value)) {
    let path = store.join(format!("{}.json", loom_governor::case_file_stem(case)));
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    change(&mut value);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
}

/// Every file of `directory` and its bytes.
fn bytes_of(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![directory.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap_or_default();
                found.insert(path, bytes);
            }
        }
    }
    found
}

/// A change to the store directory and the parent's case file that leaves the parent unreadable.
type Break = fn(&Path, &Path);

#[test]
fn a_parent_that_cannot_be_read_refuses_the_step_before_anything() {
    let epic_file =
        |store: &Path| store.join(format!("{}.json", loom_governor::case_file_stem("P.epic")));
    let breaks: [(&str, Break); 3] = [
        ("removed", |_store, file| {
            std::fs::remove_file(file).unwrap()
        }),
        ("unreadable", |_store, file| {
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o000)).unwrap();
        }),
        ("not listing the story", |store, _file| {
            rewrite_case(store, "P.epic", |case| {
                case["children"] = serde_json::json!([]);
            });
        }),
    ];
    for (what, broken) in breaks {
        let scratch = Scratch::new();
        let governor = tree_governor(FileCaseStore::open(&scratch.store).unwrap());
        open(&governor, ROOT, &["epic"]);
        open(&governor, "P.epic", &["story"]);
        let file = epic_file(&scratch.store);
        broken(&scratch.store, &file);
        let before = bytes_of(&scratch.store);
        let document = level_document(&governor, "P.epic.story", &["/opt/stub/program"], "");
        let runner = Recording::new(&[]);
        let effects = port(
            &governor,
            &document,
            &scratch,
            "P.epic.story",
            Arc::clone(&runner) as Arc<dyn TestRunner>,
        );
        let end = drive(&governor, &effects, "P.epic.story", "{}");
        let (reason, _) = refusal(&end);
        assert!(runner.children().is_empty(), "{what}");
        assert!(
            reason.contains("P.epic.story") && reason.contains("`P.epic`"),
            "{what}: {reason}"
        );
        assert!(
            !reason.contains("story\"") && !reason.contains("next.open"),
            "{what}: {reason}"
        );
        // Commission's loop logs the proposal and the refusal in `observations.json`; the port adds
        // no run to it, writes no case file and keeps no blob.
        assert!(runs(&governor).is_empty(), "{what}: {:?}", runs(&governor));
        let without_log = |mut files: BTreeMap<PathBuf, Vec<u8>>| {
            files.remove(&scratch.store.join("observations.json"));
            files
        };
        assert_eq!(
            without_log(bytes_of(&scratch.store)),
            without_log(before),
            "{what}: a byte of the store changed"
        );
        if what == "unreadable" {
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
}

#[test]
fn a_retried_child_writer_reads_its_name_and_its_feedback() {
    let governor = tree_governor(MemoryCaseStore::default());
    open(&governor, ROOT, &["story"]);
    let scratch = Scratch::new();
    // A placed subject, so the record of the first run is about a revision the retry reads.
    std::fs::create_dir_all(scratch.workspace.join("docs")).unwrap();
    std::fs::write(scratch.workspace.join("docs/spec"), "spec\n").unwrap();
    let document = level_document(
        &governor,
        "P.story",
        &["/opt/stub/program"],
        "    retry_when: {gate: [invalid]}\n",
    );
    // The first run records `invalid` about `spec`: the second is a retry.
    let runner = Recording::exiting(&[1, 0]);
    let effects = port(
        &governor,
        &document,
        &scratch,
        "P.story",
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, "P.story", "{}"));
    assert_performed(&drive(&governor, &effects, "P.story", "{}"));
    let children = runner.children();
    assert_eq!(children.len(), 2);
    let set = &children[1].environment().set;
    let feedback = set
        .get(RETRY_FEEDBACK_VARIABLE)
        .expect("the retry is given its feedback");
    assert_eq!(
        set,
        &environment(
            "P.story",
            60,
            &[(NAME, "story"), (RETRY_FEEDBACK_VARIABLE, feedback)]
        )
    );
}
