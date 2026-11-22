//! Acceptance for `story:bound-program-contract`: a bound program can be named beside its bindings
//! file (`program_root: bindings`, `resolve_programs`), sees its run's timeout in its environment
//! (`LOOM_TIMEOUT_SECONDS`), and an exit its binding classes as a program error or as not called
//! (`exits`) records nothing and is not counted.
//!
//! Every step reaches the program effect port through Commission's own loop
//! (`run_until_blocked`), a scripted executor and a `CanonGovernor` over a `MemoryCaseStore`, with
//! a fixture protocol of two artifacts and stub programs the test writes under
//! `CARGO_TARGET_TMPDIR`; no model, no network. A case that starts a process gives the port a
//! `local` runner opened with `UNCONFINED_FLAG_REASON`; a case that reads the `ChildCommand` the
//! port builds gives it a recording runner that starts nothing. The bindings file sits in directory
//! `A`, the workspace is directory `B`, and the process's working directory is a third one.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use b10x_loom_bindings::{
    PROGRAM_SOURCE, ProgramEffects, TIMEOUT_VARIABLE, check, decode, resolve_programs,
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
use loom_governor::{CanonGovernor, CaseInputs, FallibleCaseStore, MemoryCaseStore};

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
    children: Mutex<Vec<ChildCommand>>,
}

impl Recording {
    fn new(answers: &[&[u8]]) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(answers.iter().rev().map(|bytes| bytes.to_vec()).collect()),
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
        Ok(TestExecution {
            exit_code: Some(0),
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn program_output_never_enters_a_refusal() {
    const CANARY: &str = "canary-6f1d";
    let (governor, _scratch, ends) = exit_steps(
        |scratch| {
            path_text(&scratch.script(
                "stub",
                &format!("echo {CANARY}\necho {CANARY} >&2\nexit 127\n"),
            ))
        },
        PROGRAM_ERRORS,
        VALID_TABLE,
        1,
    );
    let (reason, _) = refusal(&ends[0]);
    assert!(reason.starts_with("ProgramError"), "{reason}");
    assert!(!reason.contains(CANARY), "{reason}");
    let runs = runs(&governor);
    assert_eq!(runs.len(), 1);
    assert!(!runs[0].to_string().contains(CANARY), "{}", runs[0]);
}
