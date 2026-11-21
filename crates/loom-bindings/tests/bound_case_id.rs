//! Acceptance for `story:bound-program-case-id`: every program the program effect port of
//! `b10x-loom-bindings` starts, the bound program and its `arguments_from` program alike, has in
//! its environment `PATH` at the host's value and `LOOM_CASE`, the id of the case the port was
//! built for, and nothing else; no request, report, program output, bindings document or host
//! variable sets `LOOM_CASE`.
//!
//! Every case reaches the port through Commission's own loop (`run_until_blocked`), a scripted
//! executor proposing `gate.check`, and a `CanonGovernor` over a `MemoryCaseStore`, with a fixture
//! protocol of two artifacts; no model, no network. A case that reads the `ChildCommand` gives the
//! port a recording runner that starts nothing and answers the standard outputs it was given; a
//! case that starts a process runs `/usr/bin/env` under an `EngineRunner` on `local` opened with
//! `UNCONFINED_FLAG_REASON`, and is ignored off Linux. Scratch goes under `CARGO_TARGET_TMPDIR`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use b10x_loom_bindings::{
    CASE_VARIABLE, MAX_CASE_VARIABLE_BYTES, PROGRAM_SOURCE, ProgramEffects, check, decode,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, ExecutorOutcome, ExecutorOutcomeProposedAction, ObservationId,
    PrincipalId, ProposedActionArguments, RunId,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, ChildCommand, ChildOutput, ConfinementAdapter, ConfinementError,
    ConfinementRefusal, EngineConfinement, EngineRunner, TestExecution, TestRunner,
};
use b10x_loom_intake_slice::executor::TestCommand;
use loom::bindings::{BindingsDocument, BindingsRefusalReason};
use loom_governor::{CanonGovernor, CaseInputs, FallibleCaseStore, MemoryCaseStore};

const REGISTRATION: &str = "fixture@1";
const OTHER: &str = "other@1";
const CASE: &str = "case-1";
const CHILD_CASE: &str = "case-1.north.2";
const ACTION: &str = "gate.check";

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
  gate.check:
    effect: write
    may_produce:
      - evidence: gate
";

/// Another protocol that declares the same action, so a case of it admits `gate.check`.
const OTHER_PROTOCOL: &str = r"format: protocol/1
protocol:
  id: other
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  gate: {}
actions:
  gate.check:
    effect: write
    may_produce:
      - evidence: gate
";

/// The variable the re-executed test binary reads to know it is the child run.
const CHILD_RUN: &str = "LOOM_BOUND_CASE_ID_CHILD_RUN";
/// The line the child run prints once its checks held.
const CHILD_CHECKED: &str = "bound-case-id: the child run checked LOOM_CASE";

// ---------------------------------------------------------------------------------------------
// Fixture

/// The fixture protocol, parsed once for the binary.
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

/// A governor holding the cases `cases` of [`REGISTRATION`] and `others` of [`OTHER`].
fn governor_holding(cases: &[&str], others: &[&str]) -> CanonGovernor<MemoryCaseStore> {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers")
        .with_protocol_yaml(OTHER, OTHER_PROTOCOL)
        .expect("the other protocol registers");
    for (case, registration) in cases
        .iter()
        .map(|case| (case, REGISTRATION))
        .chain(others.iter().map(|case| (case, OTHER)))
    {
        governor
            .open_case(
                CaseId((*case).to_owned()),
                registration,
                revisions(),
                CaseInputs::none(),
            )
            .expect("the case opens");
    }
    governor
}

/// The document binding [`ACTION`] to `argv` with the extra action `members`, one `gate` entry
/// read at `/status` of the report, decoded and checked against the fixture protocol.
fn document(argv: &[&str], members: &str) -> BindingsDocument {
    let argv = serde_json::to_string(argv).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  spec: docs/spec\n  impl: src\nactions:\n  - action: {ACTION}\n    executor: {{kind: program, argv: {argv}}}\n{members}    evidence:\n      - {{kind: gate, subject: spec, producer: ess, from: \"/status\", results: {{\"ok\": valid, \"*\": invalid}}, upstream: [impl]}}\n"
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(&document, REGISTRATION, protocol())
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// The `arguments_from` member naming `argv`.
fn arguments_from(argv: &[&str]) -> String {
    format!(
        "    arguments_from: {}\n",
        serde_json::to_string(argv).unwrap()
    )
}

struct Scratch {
    _dir: tempfile::TempDir,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("bound-case-id-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let workspace = dir.path().join("workspace");
        let store = dir.path().join("store");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&store).unwrap();
        Self {
            _dir: dir,
            workspace,
            store,
        }
    }
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
/// it was given (exit 0), then `{"status": "ok"}`.
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
        let stdout = self
            .answers
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| br#"{"status": "ok"}"#.to_vec());
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

/// One step of a commission for `case` on the loop's `governor`, the executor proposing
/// [`ACTION`] with `arguments`, performed by `effects`.
fn try_drive<S: FallibleCaseStore, P: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &ProgramEffects<'_, P>,
    case: &str,
    arguments: &str,
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
            action: ACTION.to_owned(),
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

/// The reason the port refused the step with.
fn refusal(end: &LoopEnd) -> String {
    match only_effect(end) {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

/// One performed step of `case`, its port built for `case` under a recording runner answering
/// `answers`; the children it received.
fn recorded(case: &str, members: &str, arguments: &str, answers: &[&[u8]]) -> Vec<ChildCommand> {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], members);
    let governor = governor_holding(&[case], &[]);
    let runner = Recording::new(answers);
    let effects = ProgramEffects::new(
        &governor,
        CaseId(case.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    assert_performed(&try_drive(&governor, &effects, case, arguments).unwrap());
    runner.children()
}

/// The only environment a child of the port has: the host's `PATH` and `LOOM_CASE` = `case`.
fn expected_environment(case: &str) -> BTreeMap<String, String> {
    let mut expected = BTreeMap::new();
    if let Ok(path) = std::env::var("PATH") {
        expected.insert("PATH".to_owned(), path);
    }
    expected.insert("LOOM_CASE".to_owned(), case.to_owned());
    expected
}

/// The value of `LOOM_CASE` in `child`'s environment.
fn loom_case(child: &ChildCommand) -> Option<&str> {
    child.environment().set.get("LOOM_CASE").map(String::as_str)
}

/// The step that `/usr/bin/env`, bound to [`ACTION`] for [`CASE`] under the flag's runner, prints:
/// its lines as a set.
fn env_lines() -> BTreeSet<String> {
    let scratch = Scratch::new();
    let document = document(&["/usr/bin/env"], "");
    let governor = governor_holding(&[CASE], &[]);
    let effects = ProgramEffects::new(
        &governor,
        CaseId(CASE.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(flag_runner()),
    )
    .unwrap();
    let end = try_drive(&governor, &effects, CASE, "{}").unwrap();
    // env prints no JSON object: the step reads the report and is refused, after the run.
    assert!(refusal(&end).starts_with("ReportUnreadable"), "{end:?}");
    let observations: Vec<_> = governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .collect();
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let digest = match observations[0].payload.member("blob_sha256") {
        Some(Value::Text(digest)) => digest.clone(),
        other => panic!("the observation names no blob: {other:?}"),
    };
    let blob = std::fs::read(scratch.store.join("blobs").join(digest)).expect("the blob is kept");
    String::from_utf8(blob)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn expected_env_lines() -> BTreeSet<String> {
    let path = std::env::var("PATH").expect("the host has a UTF-8 PATH");
    [format!("LOOM_CASE={CASE}"), format!("PATH={path}")].into()
}

// ---------------------------------------------------------------------------------------------
// The environment

#[test]
fn a_bound_programs_environment_is_path_and_loom_case() {
    assert_eq!(CASE_VARIABLE, "LOOM_CASE");
    let children = recorded(CASE, "", "{}", &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].environment().set, expected_environment(CASE));
}

#[test]
fn a_bound_program_inherits_no_other_host_variable() {
    let children = recorded(CASE, "", "{}", &[]);
    assert_eq!(children.len(), 1);
    assert!(
        children[0].environment().allow.is_empty(),
        "{:?}",
        children[0].environment().allow
    );
}

#[test]
fn an_arguments_from_program_is_given_loom_case() {
    let members = arguments_from(&["/opt/stub/args"]);
    let children = recorded(CASE, &members, "{}", &[b"{}"]);
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].program(), "/opt/stub/args");
    assert_eq!(children[0].environment().set, expected_environment(CASE));
    assert!(children[0].environment().allow.is_empty());
    assert_eq!(children[1].environment().set, expected_environment(CASE));
}

#[test]
fn a_child_case_id_reaches_the_program_as_written() {
    let children = recorded(CHILD_CASE, "", "{}", &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].environment().set,
        expected_environment(CHILD_CASE)
    );
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "runs /usr/bin/env, a Linux program path"
)]
fn a_bound_program_reads_its_case_id_from_loom_case() {
    assert_eq!(env_lines(), expected_env_lines());
}

// ---------------------------------------------------------------------------------------------
// Security: no other source sets the variable

#[test]
fn an_argument_named_loom_case_does_not_change_the_variable() {
    let children = recorded(CASE, "", r#"{"LOOM_CASE": "case-2"}"#, &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(loom_case(&children[0]), Some(CASE));
    assert_eq!(children[0].environment().set, expected_environment(CASE));
}

#[test]
fn an_argument_named_loom_case_reaches_standard_input_as_written() {
    let arguments = r#"{"LOOM_CASE": "case-2"}"#;
    let children = recorded(CASE, "", arguments, &[]);
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0].stdin(),
        Some(arguments.replace(' ', "").as_bytes())
    );
}

#[test]
fn arguments_from_output_named_loom_case_does_not_change_the_variable() {
    let members = arguments_from(&["/opt/stub/args"]);
    let printed = br#"{"LOOM_CASE": "case-2"}"#;
    let children = recorded(CASE, &members, "{}", &[printed]);
    assert_eq!(children.len(), 2);
    assert_eq!(loom_case(&children[1]), Some(CASE));
    assert_eq!(children[1].environment().set, expected_environment(CASE));
    let stdin: serde_json::Value = serde_json::from_slice(children[1].stdin().unwrap()).unwrap();
    assert_eq!(stdin, serde_json::json!({"LOOM_CASE": "case-2"}));
}

#[test]
fn a_report_cannot_change_the_variable_of_the_next_step() {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "");
    let governor = governor_holding(&[CASE], &[]);
    let runner = Recording::new(&[br#"{"status": "ok", "LOOM_CASE": "case-2"}"#]);
    let effects = ProgramEffects::new(
        &governor,
        CaseId(CASE.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    assert_performed(&try_drive(&governor, &effects, CASE, "{}").unwrap());
    assert_performed(&try_drive(&governor, &effects, CASE, "{}").unwrap());
    let children = runner.children();
    assert_eq!(children.len(), 2);
    assert_eq!(loom_case(&children[1]), Some(CASE));
    assert_eq!(children[1].environment().set, expected_environment(CASE));
}

/// The parent run starts this test binary again, this test alone, with `LOOM_CASE=host-value` in
/// its environment; the child run starts `/usr/bin/env` through the port and checks it printed
/// `LOOM_CASE=case-1`.
#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "runs /usr/bin/env, a Linux program path"
)]
fn a_loom_case_in_the_hosts_environment_does_not_reach_the_program() {
    if std::env::var_os(CHILD_RUN).is_some() {
        assert_eq!(
            std::env::var("LOOM_CASE").as_deref(),
            Ok("host-value"),
            "the child run holds the host's LOOM_CASE"
        );
        assert_eq!(env_lines(), expected_env_lines());
        println!("{CHILD_CHECKED}");
        return;
    }
    // The test's name as the harness lists it: its module path below the crate root, so the
    // filter still selects it once this file is a module of the crate's one test binary.
    let function = "a_loom_case_in_the_hosts_environment_does_not_reach_the_program";
    let name = match module_path!().split_once("::") {
        Some((_, module)) => format!("{module}::{function}"),
        None => function.to_owned(),
    };
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            name.as_str(),
            "--exact",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env(CHILD_RUN, "1")
        .env("LOOM_CASE", "host-value")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains(CHILD_CHECKED), "{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
}

#[test]
fn a_bindings_document_cannot_name_the_variable() {
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nactions:\n  - action: {ACTION}\n    executor: {{kind: program, argv: [/opt/stub/program], environment: {{LOOM_CASE: x}}}}\n"
    );
    let refusal = decode(&text).expect_err("the document is refused");
    assert_eq!(refusal.0.reason, BindingsRefusalReason::UndeclaredMember);
    assert_eq!(refusal.0.action.as_deref(), Some(ACTION));
    assert_eq!(refusal.0.member, "executor.environment");
}

#[test]
fn a_request_for_another_case_starts_no_child() {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "");
    let governor = governor_holding(&[CASE], &["case-2"]);
    let runner = Recording::new(&[]);
    let effects = ProgramEffects::new(
        &governor,
        CaseId(CASE.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    let end = try_drive(&governor, &effects, "case-2", "{}").unwrap();
    let reason = refusal(&end);
    assert!(reason.contains("this port runs case case-1"), "{reason}");
    assert!(runner.children().is_empty());
}

#[test]
fn a_port_for_a_case_the_governor_does_not_hold_starts_no_child() {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "");
    // The loop's governor holds the case; the port's governor does not.
    let loop_governor = governor_holding(&["case-9"], &[]);
    let port_governor = governor_holding(&[CASE], &[]);
    let runner = Recording::new(&[]);
    let effects = ProgramEffects::new(
        &port_governor,
        CaseId("case-9".to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    let answer = try_drive(&loop_governor, &effects, "case-9", "{}");
    // The port's governor answers that it holds no such case: the effect fails, nothing starts.
    let error = format!("{:?}", answer.expect_err("the step is not performed"));
    assert!(error.contains("the governor"), "{error}");
    assert!(runner.children().is_empty());
}

// ---------------------------------------------------------------------------------------------
// The bound on the case id

/// A case id one byte over [`MAX_CASE_VARIABLE_BYTES`].
fn long_case() -> String {
    format!(
        "case-{}",
        "x".repeat(MAX_CASE_VARIABLE_BYTES + 1 - "case-".len())
    )
}

/// One step of [`long_case`] under a recording runner: the step, the runner and the governor.
fn long_case_step() -> (LoopEnd, Arc<Recording>, CanonGovernor<MemoryCaseStore>) {
    let case = long_case();
    assert_eq!(case.len(), 4097);
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "");
    let governor = governor_holding(&[&case], &[]);
    let runner = Recording::new(&[]);
    let end = {
        let effects = ProgramEffects::new(
            &governor,
            CaseId(case.clone()),
            &document,
            &scratch.workspace,
            &scratch.store,
            Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
        )
        .unwrap();
        try_drive(&governor, &effects, &case, "{}").unwrap()
    };
    (end, runner, governor)
}

#[test]
fn a_case_id_longer_than_4096_bytes_refuses_the_step() {
    assert_eq!(MAX_CASE_VARIABLE_BYTES, 4096);
    let (end, _runner, _governor) = long_case_step();
    assert_eq!(
        refusal(&end),
        "/opt/stub/program was not started: the case id is longer than 4096 bytes"
    );
}

#[test]
fn a_case_id_longer_than_4096_bytes_starts_and_records_nothing() {
    let (end, runner, governor) = long_case_step();
    assert!(matches!(only_effect(&end), EffectOutcome::Refused(_)));
    assert!(runner.children().is_empty());
    let kept: Vec<_> = governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .collect();
    assert!(kept.is_empty(), "{kept:#?}");
    assert!(governor.evidence(&CaseId(long_case())).unwrap().is_empty());
}

// ---------------------------------------------------------------------------------------------
// The one builder of a child

#[test]
fn every_child_of_the_crate_is_built_by_bound_case_child() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![source];
    let mut calls = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            // The function each line is in: the last `fn <name>` above it.
            let mut function = String::new();
            for line in text.lines() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if let Some(at) = code.find("fn ")
                    && (at == 0 || code[..at].ends_with(' '))
                {
                    let name: String = code[at + 3..]
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        function = name;
                    }
                }
                if code.contains("bound_program_child(") {
                    calls.push((path.clone(), function.clone()));
                }
            }
        }
    }
    assert!(
        !calls.is_empty(),
        "no call of bound_program_child was found"
    );
    for (path, function) in &calls {
        assert_eq!(
            (
                path.file_name().and_then(|name| name.to_str()),
                function.as_str()
            ),
            (Some("child.rs"), "bound_case_child"),
            "{} calls bound_program_child in {function}",
            path.display()
        );
    }
}
