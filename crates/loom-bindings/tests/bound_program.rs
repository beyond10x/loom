//! Acceptance for `story:bound-program-result-table`: an action bound to a `program` executor runs
//! through the program effect port of `b10x-loom-bindings`, and its result becomes evidence by the
//! binding's result table alone.
//!
//! Every case reaches the port through Commission's own loop (`run_until_blocked`), a scripted
//! executor proposing the action with the request's arguments, and a `CanonGovernor` over a
//! `MemoryCaseStore` (a `FileCaseStore` where the store is reopened), with a fixture protocol of two
//! artifacts and stub programs; no model, no network. A case that starts a process gives the port
//! an `EngineRunner` on `local` opened with `UNCONFINED_FLAG_REASON`; a case that reads the
//! `ChildCommand` the port builds gives it a recording runner that starts nothing. Scratch goes
//! under `CARGO_TARGET_TMPDIR`, outside every mount Substrate owns.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use b10x_loom_bindings::{PROGRAM_SOURCE, ProgramEffects, check, decode};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Observation, ObservationData, ObservationId, PrincipalId, ProposedActionArguments, RunId,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::EffectPort;
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, Cancel, ChildCommand, ChildOutput, ConfinementAdapter,
    ConfinementError, ConfinementRefusal, EngineConfinement, EngineRunner, TestExecution,
    TestRunner,
};
use b10x_loom_intake_slice::executor::{TestCommand, UnconfinedRunner};
use loom::bindings::BindingsDocument;
use loom_governor::{CanonGovernor, CaseInputs, FallibleCaseStore, FileCaseStore, MemoryCaseStore};
use serde_json::json;
use sha2::{Digest as _, Sha256};

const REGISTRATION: &str = "fixture@1";
const CASE: &str = "case-1";
/// The capped action of the run-count cases, a writer.
const CHECK: &str = "spec.check";
/// An action that only reads.
const LOOK: &str = "spec.look";

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
      - evidence: review
  spec.look:
    effect: read
    may_produce:
      - evidence: review
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

/// The revision the case holds for each artifact.
const SPEC_REVISION: &str = "s1";
const IMPL_REVISION: &str = "i1";

// ---------------------------------------------------------------------------------------------
// Fixture

fn protocol() -> b10x_canon::model::Protocol {
    b10x_canon::model::parse(PROTOCOL).expect("the fixture protocol parses")
}

fn revisions() -> BTreeMap<String, String> {
    [
        ("spec".to_owned(), SPEC_REVISION.to_owned()),
        ("impl".to_owned(), IMPL_REVISION.to_owned()),
    ]
    .into()
}

fn governor_over<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store)
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers")
}

fn governor() -> CanonGovernor<MemoryCaseStore> {
    let governor = governor_over(MemoryCaseStore::default());
    open(&governor, CASE);
    governor
}

fn open<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str) {
    governor
        .open_case(
            CaseId(case.to_owned()),
            REGISTRATION,
            revisions(),
            CaseInputs::none(),
        )
        .expect("the case opens");
}

/// The document holding `actions`, decoded and checked against the fixture protocol.
fn document(actions: &str) -> BindingsDocument {
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  spec: docs/spec\n  impl: src\nactions:\n{actions}"
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(&document, REGISTRATION, &protocol())
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// One action list item binding `action` to `argv`, with the extra action `members` (each a line
/// indented four spaces) and the evidence `entries`.
fn bound(action: &str, argv: &[&str], members: &str, entries: &[String]) -> String {
    let argv = serde_json::to_string(argv).unwrap();
    let mut text = format!("  - action: {action}\n    executor: {{kind: program, argv: {argv}}}\n");
    text.push_str(members);
    if !entries.is_empty() {
        text.push_str("    evidence:\n");
        for entry in entries {
            text.push_str(&format!("      - {entry}\n"));
        }
    }
    text
}

/// An evidence entry of `kind` about `spec` by the producer `ess`, read `from`, through `table`,
/// following `impl`.
fn entry(kind: &str, from: &str, table: &str) -> String {
    format!(
        "{{kind: {kind}, subject: spec, producer: ess, from: {from}, results: {table}, upstream: [impl]}}"
    )
}

/// The table every exit case shares.
const VALID_TABLE: &str = r#"{"0": valid, "*": invalid}"#;

/// A scratch directory: the workspace, the store directory, the stub programs and a marker file.
struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("bound-program-")
            .tempdir_in(env!("CARGO_TARGET_TMPDIR"))
            .unwrap();
        let root = dir.path().to_owned();
        let workspace = root.join("workspace");
        let store = root.join("store");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&store).unwrap();
        Self {
            _dir: dir,
            root,
            workspace,
            store,
        }
    }

    /// An executable `/bin/sh` script named `name` holding `body`; its absolute path.
    fn script(&self, name: &str, body: &str) -> String {
        let path = self.root.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_str().unwrap().to_owned()
    }

    /// The file a marking script creates when it starts.
    fn marker(&self) -> PathBuf {
        self.root.join("marker")
    }

    /// A script that creates [`Scratch::marker`], then runs `rest`.
    fn marking(&self, name: &str, rest: &str) -> String {
        let touch = format!("/usr/bin/touch '{}'\n{rest}", self.marker().display());
        self.script(name, &touch)
    }

    fn blobs(&self) -> PathBuf {
        self.store.join("blobs")
    }

    /// The name of each file in the blob directory, sorted.
    fn blob_names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.blobs())
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
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

fn port<'a, S: FallibleCaseStore>(
    governor: &'a CanonGovernor<S>,
    document: &'a BindingsDocument,
    scratch: &Scratch,
    case: &str,
    runner: Option<Arc<dyn TestRunner>>,
) -> Result<ProgramEffects<'a, S>, ConfinementError> {
    ProgramEffects::new(
        governor,
        CaseId(case.to_owned()),
        document,
        &scratch.workspace,
        &scratch.store,
        runner,
    )
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn next() -> u64 {
    ISSUED.fetch_add(1, Ordering::Relaxed)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission(
    case: &str,
) -> Commission<b10x_loom_commission::model::responsibility::commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(next())),
        agent_revision_id: AgentRevisionId(uuid(next())),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// One step: the executor proposes nothing else.
struct Context;

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(uuid(next()))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(uuid(next()))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// The request's arguments, as JSON text.
fn args(text: &str) -> Value {
    json::parse(text).expect("the arguments parse")
}

/// Runs one step of `case` in which the executor proposes `action` with `arguments`.
fn drive<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &ProgramEffects<'_, S>,
    case: &str,
    action: &str,
    arguments: Value,
) -> LoopEnd {
    try_drive(governor, effects, case, action, arguments)
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// [`drive`], answering the loop's failure instead of panicking on it.
fn try_drive<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &ProgramEffects<'_, S>,
    case: &str,
    action: &str,
    arguments: Value,
) -> Result<LoopEnd, b10x_loom_commission::runtime::LoopError> {
    let commission = commission(case);
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: action.to_owned(),
            arguments: ProposedActionArguments(arguments),
        },
    )]);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(next()))));
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

/// The reason the port refused the step with.
fn refusal(end: &LoopEnd) -> String {
    match only_effect(end) {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

fn assert_performed(end: &LoopEnd) {
    match only_effect(end) {
        EffectOutcome::Performed(_) => {}
        other => panic!("not performed: {other:?}"),
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

/// The observations the port kept.
fn port_observations<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> Vec<ObservationData> {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .collect()
}

fn records<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> Vec<EvidenceData> {
    governor.evidence(&CaseId(CASE.to_owned())).unwrap()
}

/// The one record of the case.
fn record<S: FallibleCaseStore>(governor: &CanonGovernor<S>) -> EvidenceData {
    let records = records(governor);
    assert_eq!(records.len(), 1, "{records:#?}");
    records.into_iter().next().unwrap()
}

/// The `result` member of `record`'s facts.
fn result_of(record: &EvidenceData) -> serde_json::Value {
    plain(&record.facts)["result"].clone()
}

/// The blob the one observation of the port names, read from the store.
fn blob(governor: &CanonGovernor<MemoryCaseStore>, scratch: &Scratch) -> Vec<u8> {
    let observations = port_observations(governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let digest = plain(&observations[0].payload)["blob_sha256"]
        .as_str()
        .expect("the observation names a blob")
        .to_owned();
    std::fs::read(scratch.blobs().join(digest)).expect("the blob is kept")
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Runs `body` as the program bound to [`CHECK`] with `table` read from its exit, under the
/// flag's runner, and answers the governor, the scratch and the step.
fn exit_case(
    body: &str,
    members: &str,
    table: &str,
) -> (CanonGovernor<MemoryCaseStore>, Scratch, LoopEnd) {
    let scratch = Scratch::new();
    let program = scratch.marking("program.sh", body);
    let document = document(&bound(
        CHECK,
        &[&program],
        members,
        &[entry("gate", "exit", table)],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    (governor, scratch, end)
}

/// Runs `body` as the program bound to [`CHECK`] with the table `{"ok": valid, "*": invalid}`
/// read at the pointer `/status` of its report.
fn pointer_case(body: &str) -> (CanonGovernor<MemoryCaseStore>, Scratch, LoopEnd) {
    let scratch = Scratch::new();
    let program = scratch.marking("program.sh", body);
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "/status", r#"{"ok": valid, "*": invalid}"#)],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    (governor, scratch, end)
}

/// Runs `/usr/bin/cat` bound to [`CHECK`] with the action `members`, for a request with
/// `arguments`, and answers the blob: what the program read on standard input.
fn cat_case(members: &str, arguments: &str) -> Vec<u8> {
    let scratch = Scratch::new();
    let document = document(&bound(
        CHECK,
        &["/usr/bin/cat"],
        members,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args(arguments))
    };
    assert_performed(&end);
    blob(&governor, &scratch)
}

// ---------------------------------------------------------------------------------------------
// The result table

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn exit_zero_records_the_result_the_table_lists_for_zero() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("valid"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_exit_code_the_table_does_not_list_falls_to_star() {
    let (governor, _scratch, end) = exit_case("exit 3\n", "", VALID_TABLE);
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("invalid"));
}

const SLEEPS: &str = "exec /usr/bin/sleep 30\n";
const ONE_SECOND: &str = "    timeout_seconds: 1\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_timeout_records_the_result_of_the_timeout_key() {
    let (governor, _scratch, end) = exit_case(
        SLEEPS,
        ONE_SECOND,
        r#"{"0": valid, "timeout": slow, "*": invalid}"#,
    );
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("slow"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_timeout_without_a_timeout_key_falls_to_star() {
    let (governor, _scratch, end) = exit_case(SLEEPS, ONE_SECOND, VALID_TABLE);
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("invalid"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_timeout_with_neither_key_is_refused_result_not_in_table() {
    let (governor, _scratch, end) = exit_case(SLEEPS, ONE_SECOND, r#"{"0": valid}"#);
    let reason = refusal(&end);
    assert!(reason.starts_with("ResultNotInTable"), "{reason}");
    assert!(records(&governor).is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_value_the_table_does_not_list_records_nothing() {
    let (governor, scratch, end) = exit_case("exit 3\n", "", r#"{"0": valid}"#);
    let reason = refusal(&end);
    assert!(reason.starts_with("ResultNotInTable"), "{reason}");
    assert!(scratch.marker().exists(), "the program ran");
    assert!(records(&governor).is_empty(), "{:#?}", records(&governor));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_pointer_reads_the_named_member_of_the_report() {
    let (governor, _scratch, end) =
        pointer_case("printf '%s' '{\"status\": \"ok\", \"other\": \"bad\"}'\n");
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("valid"));
    let (governor, _scratch, end) =
        pointer_case("printf '%s' '{\"status\": \"bad\", \"other\": \"ok\"}'\n");
    assert_performed(&end);
    assert_eq!(result_of(&record(&governor)), json!("invalid"));
}

const NOT_ONE_OBJECT: &str = "printf '%s' '[{\"status\": \"ok\"}]'\n";
const LARGER_THAN_ONE_MIB: &str = "printf '{\"status\": \"ok\", \"pad\": \"'\n/usr/bin/head -c 1048576 /dev/zero | /usr/bin/tr '\\0' 'a'\nprintf '\"}'\n";
const WITHOUT_THE_POINTER: &str = "printf '%s' '{\"other\": \"ok\"}'\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_report_that_is_not_one_json_object_is_refused_report_unreadable() {
    for body in [
        NOT_ONE_OBJECT,
        "printf '%s' '{\"status\": \"ok\"} {}'\n",
        "printf 'ok'\n",
    ] {
        let (_governor, _scratch, end) = pointer_case(body);
        let reason = refusal(&end);
        assert!(reason.starts_with("ReportUnreadable"), "{body}: {reason}");
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_report_larger_than_one_mib_is_refused_report_unreadable() {
    let (_governor, _scratch, end) = pointer_case(LARGER_THAN_ONE_MIB);
    let reason = refusal(&end);
    assert!(reason.starts_with("ReportUnreadable"), "{reason}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_report_without_the_pointer_is_refused_report_unreadable() {
    let (_governor, _scratch, end) = pointer_case(WITHOUT_THE_POINTER);
    let reason = refusal(&end);
    assert!(reason.starts_with("ReportUnreadable"), "{reason}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_report_refused_unreadable_records_nothing() {
    for body in [NOT_ONE_OBJECT, LARGER_THAN_ONE_MIB, WITHOUT_THE_POINTER] {
        let (governor, scratch, end) = pointer_case(body);
        let reason = refusal(&end);
        assert!(reason.starts_with("ReportUnreadable"), "{reason}");
        assert!(scratch.marker().exists(), "the program ran");
        assert!(records(&governor).is_empty(), "{:#?}", records(&governor));
    }
}

/// Runs a program bound to [`CHECK`] with two entries, `gate` and `review`, each with its own table,
/// and answers the governor, the scratch and the step; the program appends a line to `runs` each
/// time it starts.
fn two_kinds_case() -> (CanonGovernor<MemoryCaseStore>, Scratch, LoopEnd) {
    let scratch = Scratch::new();
    let runs = scratch.root.join("runs");
    let program = scratch.script(
        "program.sh",
        &format!("echo started >> '{}'\nexit 0\n", runs.display()),
    );
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[
            entry("gate", "exit", r#"{"0": passed, "*": failed}"#),
            entry("review", "exit", r#"{"0": approved, "*": rejected}"#),
        ],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    (governor, scratch, end)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_entry_list_of_two_kinds_records_one_record_per_kind() {
    let (governor, _scratch, end) = two_kinds_case();
    assert_performed(&end);
    let kinds: Vec<String> = records(&governor).into_iter().map(|r| r.kind).collect();
    assert_eq!(kinds, ["gate", "review"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_entry_list_of_two_kinds_starts_the_program_once() {
    let (_governor, scratch, end) = two_kinds_case();
    assert_performed(&end);
    let runs = std::fs::read_to_string(scratch.root.join("runs")).unwrap();
    assert_eq!(runs.lines().count(), 1, "{runs}");
    assert_eq!(port_observations(&_governor).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn each_record_takes_its_result_from_its_own_kinds_table() {
    let (governor, _scratch, end) = two_kinds_case();
    assert_performed(&end);
    let results: Vec<(String, serde_json::Value)> = records(&governor)
        .iter()
        .map(|record| (record.kind.clone(), result_of(record)))
        .collect();
    assert_eq!(
        results,
        [
            ("gate".to_owned(), json!("passed")),
            ("review".to_owned(), json!("approved"))
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// What the program receives

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_program_receives_the_arguments_as_one_json_object_on_standard_input() {
    let blob = cat_case("", r#"{"domain": "bindings", "count": 2}"#);
    let read: serde_json::Value = serde_json::from_slice(&blob).expect("one JSON value");
    assert_eq!(read, json!({"domain": "bindings", "count": 2}));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_program_starts_with_path_and_loom_case_alone_in_its_environment() {
    let scratch = Scratch::new();
    let document = document(&bound(
        CHECK,
        &["/usr/bin/env"],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    assert_performed(&end);
    let path = std::env::var("PATH").expect("the host has a UTF-8 PATH");
    assert_eq!(
        String::from_utf8(blob(&governor, &scratch)).unwrap(),
        // story:bound-program-contract adds the run's timeout, 60 s when the binding names none.
        format!("LOOM_CASE={CASE}\nLOOM_TIMEOUT_SECONDS=60\nPATH={path}\n")
    );
}

const LITERAL: &str = "    arguments: {domain: literal}\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_requests_arguments_win_over_the_bindings_literal_arguments() {
    let blob = cat_case(LITERAL, r#"{"domain": "request"}"#);
    let read: serde_json::Value = serde_json::from_slice(&blob).unwrap();
    assert_eq!(read, json!({"domain": "request"}));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_bindings_literal_arguments_are_used_when_the_request_has_none() {
    let blob = cat_case(LITERAL, "{}");
    let read: serde_json::Value = serde_json::from_slice(&blob).unwrap();
    assert_eq!(read, json!({"domain": "literal"}));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_null_request_argument_reaches_the_program_as_null() {
    let blob = cat_case(LITERAL, "null");
    assert_eq!(String::from_utf8(blob).unwrap(), "null");
}

/// Binds the marking program `bound_body` to [`CHECK`] with `arguments_from` the script
/// `from_body`, and runs one step for a request with no arguments.
fn arguments_from_case(
    from_body: &str,
    bound_body: &str,
) -> (CanonGovernor<MemoryCaseStore>, Scratch, LoopEnd) {
    let scratch = Scratch::new();
    let from = scratch.script("arguments.sh", from_body);
    let program = scratch.marking("program.sh", bound_body);
    let members = format!(
        "    arguments_from: {}\n",
        serde_json::to_string(&[&from]).unwrap()
    );
    let document = document(&bound(
        CHECK,
        &[&program],
        &members,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    (governor, scratch, end)
}

const PRINTS_AN_OBJECT: &str = "printf '%s' '{\"stories\": [\"a\", \"b\"]}'\n";
const EXITS_NON_ZERO: &str = "printf '%s' '{\"stories\": []}'\nexit 2\n";
const PRINTS_A_LIST: &str = "printf '%s' '[\"a\", \"b\"]'\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn arguments_from_output_is_the_programs_standard_input() {
    let (governor, scratch, end) = arguments_from_case(PRINTS_AN_OBJECT, "exec /usr/bin/cat\n");
    assert_performed(&end);
    let read: serde_json::Value = serde_json::from_slice(&blob(&governor, &scratch)).unwrap();
    assert_eq!(read, json!({"stories": ["a", "b"]}));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_arguments_from_program_that_exits_non_zero_is_refused_arguments_unreadable() {
    let (_governor, _scratch, end) = arguments_from_case(EXITS_NON_ZERO, "exit 0\n");
    let reason = refusal(&end);
    assert!(reason.starts_with("ArgumentsUnreadable"), "{reason}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn arguments_from_output_that_is_not_one_json_object_is_refused_arguments_unreadable() {
    for body in [PRINTS_A_LIST, "printf 'stories'\n", "printf '%s' '{} {}'\n"] {
        let (_governor, _scratch, end) = arguments_from_case(body, "exit 0\n");
        let reason = refusal(&end);
        assert!(
            reason.starts_with("ArgumentsUnreadable"),
            "{body}: {reason}"
        );
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_refused_arguments_from_starts_no_bound_program() {
    for body in [EXITS_NON_ZERO, PRINTS_A_LIST] {
        let (governor, scratch, end) = arguments_from_case(body, "exit 0\n");
        let reason = refusal(&end);
        assert!(reason.starts_with("ArgumentsUnreadable"), "{reason}");
        assert!(
            !scratch.marker().exists(),
            "{body}: the bound program started"
        );
        assert!(port_observations(&governor).is_empty());
        assert!(records(&governor).is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// The record

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_records_subject_revision_is_the_revision_the_case_holds() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    let facts = plain(&record(&governor).facts);
    assert_eq!(facts["subject"], json!("spec"));
    // story:bound-artifact-revisions: the case holds the digest of `docs/spec` once the step is
    // written, `none` while the workspace holds nothing there; the case opened with `s1`.
    assert_ne!(SPEC_REVISION, b10x_loom_bindings::ABSENT);
    assert_eq!(facts["subject_revision"], json!(b10x_loom_bindings::ABSENT));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_records_instant_is_the_observations() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    let observations = port_observations(&governor);
    assert_eq!(observations.len(), 1);
    let facts = plain(&record(&governor).facts);
    assert_eq!(facts["observed_at"], json!(observations[0].observed_at.0));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_records_producer_is_the_bindings() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    assert_eq!(record(&governor).producer, "ess");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_records_facts_carry_the_revision_of_each_upstream_artifact() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    let facts = plain(&record(&governor).facts);
    // story:bound-artifact-revisions: `impl` is placed at `src`, which the workspace does not
    // hold, so the case holds `none` for it once the step is written; it opened with `i1`.
    assert_ne!(IMPL_REVISION, b10x_loom_bindings::ABSENT);
    assert_eq!(
        facts["upstream_revisions"],
        json!({"impl": b10x_loom_bindings::ABSENT})
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn producer_observed_at_and_subject_revision_in_the_report_change_nothing() {
    let plain_report = pointer_case("printf '%s' '{\"status\": \"ok\"}'\n");
    let steered = pointer_case(
        "printf '%s' '{\"status\": \"ok\", \"producer\": \"intruder\", \"observed_at\": \"1999-01-01T00:00:00Z\", \"subject_revision\": \"s9\", \"result\": \"forged\", \"kind\": \"review\", \"subject\": \"impl\"}'\n",
    );
    let shape = |(governor, _scratch, end): &(CanonGovernor<MemoryCaseStore>, Scratch, LoopEnd)| {
        assert_performed(end);
        let record = record(governor);
        let observation = port_observations(governor).remove(0);
        let mut facts = plain(&record.facts);
        assert_eq!(facts["observed_at"], json!(observation.observed_at.0));
        let members = facts.as_object_mut().unwrap();
        members.remove("id");
        members.remove("observed_at");
        (
            record.kind,
            record.producer,
            record.subject_revision,
            record.observation_ids.len(),
            facts,
        )
    };
    assert_eq!(shape(&steered), shape(&plain_report));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_record_cites_the_observation() {
    let (governor, _scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    let observations = port_observations(&governor);
    assert_eq!(observations.len(), 1);
    assert_eq!(
        record(&governor).observation_ids,
        [observations[0].observation_id.clone()]
    );
}

// ---------------------------------------------------------------------------------------------
// The blob

const OUT_THEN_ERR: &str = "printf 'out-'\nprintf 'err' >&2\nexit 0\n";

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_observation_names_the_sha256_of_the_blob() {
    let (governor, _scratch, end) = exit_case(OUT_THEN_ERR, "", VALID_TABLE);
    assert_performed(&end);
    let observations = port_observations(&governor);
    assert_eq!(
        plain(&observations[0].payload)["blob_sha256"],
        json!(sha256(b"out-err"))
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_blob_is_the_file_blobs_sha256_in_the_store_directory() {
    let (governor, scratch, end) = exit_case(OUT_THEN_ERR, "", VALID_TABLE);
    assert_performed(&end);
    let digest = sha256(b"out-err");
    assert_eq!(
        std::fs::read(scratch.store.join("blobs").join(&digest)).unwrap(),
        b"out-err"
    );
    assert_eq!(scratch.blob_names(), [digest]);
    drop(governor);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_blob_holds_standard_output_then_standard_error() {
    let (governor, scratch, end) =
        exit_case("printf 'err-' >&2\nprintf 'out'\nexit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    assert_eq!(blob(&governor, &scratch), b"outerr-");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn two_runs_with_the_same_output_leave_one_blob_file() {
    let scratch = Scratch::new();
    let program = scratch.script("program.sh", OUT_THEN_ERR);
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert_eq!(port_observations(&governor).len(), 2);
    assert_eq!(scratch.blob_names(), [sha256(b"out-err")]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_run_leaves_only_sha256_named_files_in_the_blob_directory() {
    let scratch = Scratch::new();
    let program = scratch.script(
        "program.sh",
        "/usr/bin/head -c 300000 /dev/urandom | /usr/bin/od -An -tx1\nexit 0\n",
    );
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    for _ in 0..3 {
        assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    }
    let names = scratch.blob_names();
    assert_eq!(names.len(), 3, "{names:?}");
    for name in names {
        let content = std::fs::read(scratch.blobs().join(&name)).unwrap();
        assert_eq!(name, sha256(&content));
    }
}

/// Beyond the Acceptance: the blob is written inside the store directory alone. A link already at
/// `blobs/<sha256>` is replaced by the blob and never written through, and a file there whose
/// content is not the blob is replaced by it.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_link_or_a_stale_file_at_the_blob_name_is_replaced_never_written_through() {
    for planted in ["link", "stale"] {
        let scratch = Scratch::new();
        let program = scratch.script("program.sh", OUT_THEN_ERR);
        let document = document(&bound(
            CHECK,
            &[&program],
            "",
            &[entry("gate", "exit", VALID_TABLE)],
        ));
        let outside = scratch.root.join("outside");
        std::fs::write(&outside, "kept").unwrap();
        std::fs::create_dir(scratch.blobs()).unwrap();
        let at = scratch.blobs().join(sha256(b"out-err"));
        if planted == "link" {
            std::os::unix::fs::symlink(&outside, &at).unwrap();
        } else {
            std::fs::write(&at, "stale").unwrap();
        }
        let governor = governor();
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "kept",
            "{planted}"
        );
        let kept = std::fs::symlink_metadata(&at).unwrap();
        assert!(kept.file_type().is_file(), "{planted}: {kept:?}");
        assert_eq!(std::fs::read(&at).unwrap(), b"out-err", "{planted}");
    }
}

/// Beyond the Acceptance: a blob directory that is a link is never written through; the step
/// fails, no record is kept, and the run's one observation names no blob.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_blob_directory_that_is_a_link_is_never_written_through() {
    let scratch = Scratch::new();
    let program = scratch.script("program.sh", OUT_THEN_ERR);
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let outside = scratch.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, scratch.blobs()).unwrap();
    let governor = governor();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    let failed = try_drive(&governor, &effects, CASE, CHECK, args("{}"));
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    // The run still counts: its one observation names no blob.
    let observations = port_observations(&governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let run = plain(&observations[0].payload);
    assert!(run["blob_sha256"].is_null(), "{run}");
    assert!(records(&governor).is_empty());
}

/// Beyond the Acceptance: a report naming a member twice is not one JSON object a pointer can read
/// one way; it is refused `ReportUnreadable` and records nothing.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_report_naming_a_member_twice_is_refused_report_unreadable() {
    for body in [
        "printf '%s' '{\"status\": \"bad\", \"status\": \"ok\"}'\n",
        "printf '%s' '{\"status\": \"ok\", \"inner\": {\"a\": 1, \"a\": 2}}'\n",
    ] {
        let (governor, _scratch, end) = pointer_case(body);
        let reason = refusal(&end);
        assert!(reason.starts_with("ReportUnreadable"), "{body}: {reason}");
        assert!(records(&governor).is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// The run count

/// The one protocol and document the four run-count cases drive: [`CHECK`] bound to the marking
/// program with `budget.max_runs` 1, and [`LOOK`] to a plain one.
fn capped(scratch: &Scratch) -> BindingsDocument {
    let program = scratch.marking("program.sh", "exit 0\n");
    let look = scratch.script("look.sh", "exit 0\n");
    let mut actions = bound(
        CHECK,
        &[&program],
        "    budget: {max_runs: 1}\n",
        &[entry("gate", "exit", VALID_TABLE)],
    );
    actions.push_str(&bound(
        LOOK,
        &[&look],
        "    budget: {max_runs: 1}\n",
        &[entry("review", "exit", VALID_TABLE)],
    ));
    document(&actions)
}

/// The protocol [`capped`] runs against: the fixture's.
fn capped_protocol() -> &'static str {
    PROTOCOL
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_action_at_its_max_runs_is_not_performed() {
    let scratch = Scratch::new();
    let document = capped(&scratch);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(scratch.marker().exists());
    std::fs::remove_file(scratch.marker()).unwrap();
    let end = drive(&governor, &effects, CASE, CHECK, args("{}"));
    assert!(
        matches!(only_effect(&end), EffectOutcome::Refused(_)),
        "{end:?}"
    );
    assert!(
        !scratch.marker().exists(),
        "the capped action started again"
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn an_action_at_its_max_runs_is_reported_runs_spent() {
    let scratch = Scratch::new();
    let document = capped(&scratch);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    let reason = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
    assert_eq!(port_observations(&governor).len(), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_run_count_is_the_observations_of_that_action_from_this_port() {
    let scratch = Scratch::new();
    let document = capped(&scratch);
    let governor = governor();
    open(&governor, "case-2");
    // Another action of this case, and this action in another case, each from this port.
    {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        assert_performed(&drive(&governor, &effects, CASE, LOOK, args("{}")));
    }
    {
        let other = port(
            &governor,
            &document,
            &scratch,
            "case-2",
            Some(flag_runner()),
        )
        .unwrap();
        assert_performed(&drive(&governor, &other, "case-2", CHECK, args("{}")));
    }
    // This action in this case from another source.
    ObservationPort::observe(
        &governor,
        Observation::new(ObservationData {
            observation_id: ObservationId(uuid(next())),
            source: "loom/action-command".to_owned(),
            subject: "spec".to_owned(),
            observed_at: Timestamp("2026-11-21T12:00:00Z".to_owned()),
            payload: args(&format!(r#"{{"case": "{CASE}", "action": "{CHECK}"}}"#)),
        }),
    )
    .unwrap();
    std::fs::remove_file(scratch.marker()).unwrap();
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(scratch.marker().exists());
    let reason = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_run_count_survives_a_reopen_of_the_store() {
    let scratch = Scratch::new();
    let document = capped(&scratch);
    {
        let governor = governor_over(FileCaseStore::open(&scratch.store).unwrap());
        open(&governor, CASE);
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
        assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    }
    std::fs::remove_file(scratch.marker()).unwrap();
    let governor = governor_over(FileCaseStore::open(&scratch.store).unwrap());
    let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
    let reason = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
    assert!(!scratch.marker().exists());
}

#[test]
fn the_run_count_tests_drive_an_action_whose_effect_class_is_write() {
    let protocol = b10x_canon::model::parse(capped_protocol()).unwrap();
    let (_, action) = protocol
        .actions
        .iter()
        .find(|(id, _)| id.as_str() == CHECK)
        .expect("the protocol declares the capped action");
    assert_eq!(
        action.effect.as_ref().map(|effect| effect.as_str()),
        Some("write")
    );
}

// ---------------------------------------------------------------------------------------------
// What the port performs

/// A runner that keeps every child and preflight root it is given and starts nothing; its
/// `local_reason` is what the case gives it.
#[derive(Default)]
struct Recording {
    reason: Option<&'static str>,
    preflight: Option<ConfinementError>,
    /// What `run_child` answers instead of a run, when set.
    run_error: Option<ConfinementError>,
    children: Mutex<Vec<ChildCommand>>,
    roots: Mutex<Vec<PathBuf>>,
}

impl Recording {
    fn with_reason(reason: &'static str) -> Self {
        Self {
            reason: Some(reason),
            ..Self::default()
        }
    }

    fn children(&self) -> Vec<ChildCommand> {
        self.children.lock().unwrap().clone()
    }
}

impl TestRunner for Recording {
    fn run(&self, _: &TestCommand, _: &Path) -> Result<TestExecution, ConfinementError> {
        Err(ConfinementError {
            reason: ConfinementRefusal::CapabilityUnserved,
            detail: "the fixture runs no test command".to_owned(),
        })
    }

    fn backend(&self) -> Backend {
        Backend::Substrate
    }

    fn local_reason(&self) -> Option<&str> {
        self.reason
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.children.lock().unwrap().push(child.clone());
        if let Some(error) = &self.run_error {
            return Err(error.clone());
        }
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
                stdout: b"{}".to_vec(),
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
            },
        })
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.roots.lock().unwrap().push(root.to_owned());
        match &self.preflight {
            Some(refusal) => Err(refusal.clone()),
            None => Ok(()),
        }
    }
}

/// Runs `program` bound to [`CHECK`] under `runner` for one step, and answers the child the runner
/// received.
fn recorded_child(program: &str, runner: Recording) -> (Scratch, ChildCommand) {
    let scratch = Scratch::new();
    let document = document(&bound(
        CHECK,
        &[program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let runner = Arc::new(runner);
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    assert_performed(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    let children = runner.children();
    assert_eq!(children.len(), 1);
    (scratch, children.into_iter().next().unwrap())
}

#[test]
fn performs_is_true_for_an_action_bound_to_program() {
    let scratch = Scratch::new();
    let document = document(&bound(
        CHECK,
        &["/usr/bin/true"],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::new(Recording::default())),
    )
    .unwrap();
    assert!(effects.performs(CHECK));
}

#[test]
fn performs_is_false_for_an_action_bound_to_any_other_executor() {
    let scratch = Scratch::new();
    let document = document(
        "  - action: impl.write\n    executor: {kind: agent, adapter: claude-code}\n\
         \x20 - action: impl.sign\n    executor: {kind: person, principal: alice}\n\
         \x20 - action: impl.open\n    executor: {kind: open, each: items, bindings: child.bindings.yaml}\n\
         \x20 - action: impl.copy\n    executor: {kind: copy, evidence: review, from: other@1}\n",
    );
    let governor = governor();
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::new(Recording::default())),
    )
    .unwrap();
    for action in [
        "impl.write",
        "impl.sign",
        "impl.open",
        "impl.copy",
        CHECK,
        LOOK,
    ] {
        assert!(!effects.performs(action), "{action}");
    }
}

/// The document every construction case builds the port with: the marking program bound to
/// [`CHECK`].
fn construction_document(scratch: &Scratch) -> BindingsDocument {
    let program = scratch.marking("program.sh", "exit 0\n");
    document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ))
}

/// Builds the port under `runner` and answers the refusal, the scratch and the governor.
fn refused_at_construction(
    runner: Option<Arc<dyn TestRunner>>,
) -> (ConfinementError, Scratch, CanonGovernor<MemoryCaseStore>) {
    let scratch = Scratch::new();
    let document = construction_document(&scratch);
    let governor = governor();
    let refusal = match port(&governor, &document, &scratch, CASE, runner) {
        Ok(_) => panic!("the port was built"),
        Err(refusal) => refusal,
    };
    (refusal, scratch, governor)
}

#[test]
fn a_host_with_no_runner_is_refused_no_runner_at_construction() {
    let (refusal, _scratch, _governor) = refused_at_construction(None);
    assert_eq!(refusal.reason, ConfinementRefusal::NoRunner, "{refusal}");
}

#[test]
fn a_host_with_no_runner_starts_no_program() {
    let (_refusal, scratch, _governor) = refused_at_construction(None);
    assert!(!scratch.marker().exists());
}

#[test]
fn a_host_with_no_runner_records_nothing() {
    let (_refusal, _scratch, governor) = refused_at_construction(None);
    assert!(governor.try_observations().unwrap().is_empty());
    assert!(records(&governor).is_empty());
}

#[test]
fn the_no_runner_detail_names_no_flag_and_no_file_member() {
    let (refusal, _scratch, _governor) = refused_at_construction(None);
    assert_eq!(refusal.reason, ConfinementRefusal::NoRunner);
    assert!(
        !refusal.detail.contains("--unconfined-bindings"),
        "{refusal}"
    );
    assert!(!refusal.detail.contains("unconfined_bindings"), "{refusal}");
}

#[test]
fn a_host_runner_with_the_default_run_child_is_refused_no_runner_at_construction() {
    let (refusal, _scratch, _governor) = refused_at_construction(Some(Arc::new(UnconfinedRunner)));
    assert_eq!(refusal.reason, ConfinementRefusal::NoRunner, "{refusal}");
}

#[test]
fn a_host_runner_with_the_default_run_child_starts_no_program() {
    let (_refusal, scratch, governor) = refused_at_construction(Some(Arc::new(UnconfinedRunner)));
    assert!(!scratch.marker().exists());
    assert!(governor.try_observations().unwrap().is_empty());
}

fn another_local_runner() -> Arc<dyn TestRunner> {
    Arc::new(
        EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some("the operator debugs a script".to_owned()),
            cgroup_root: None,
        })
        .expect("local opens with any reason"),
    )
}

#[test]
fn a_local_runner_with_another_reason_is_refused_command_unconfined() {
    let (refusal, _scratch, _governor) = refused_at_construction(Some(another_local_runner()));
    assert_eq!(
        refusal.reason,
        ConfinementRefusal::CommandUnconfined,
        "{refusal}"
    );
}

#[test]
fn a_local_runner_with_another_reason_starts_no_program() {
    let (_refusal, scratch, governor) = refused_at_construction(Some(another_local_runner()));
    assert!(!scratch.marker().exists());
    assert!(governor.try_observations().unwrap().is_empty());
}

#[test]
fn a_local_runner_with_the_flags_reason_is_admitted_at_construction() {
    let scratch = Scratch::new();
    let document = construction_document(&scratch);
    let governor = governor();
    if let Err(refusal) = port(&governor, &document, &scratch, CASE, Some(flag_runner())) {
        panic!("refused: {refusal}");
    }
    assert!(!scratch.marker().exists(), "construction starts nothing");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_local_runner_with_the_flags_reason_runs_the_program() {
    let (governor, scratch, end) = exit_case("exit 0\n", "", VALID_TABLE);
    assert_performed(&end);
    assert!(scratch.marker().exists());
    assert_eq!(result_of(&record(&governor)), json!("valid"));
}

#[test]
fn preflight_child_is_called_with_the_workspace_root() {
    let scratch = Scratch::new();
    let document = construction_document(&scratch);
    let governor = governor();
    let runner = Arc::new(Recording::default());
    port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    assert_eq!(
        *runner.roots.lock().unwrap(),
        std::slice::from_ref(&scratch.workspace)
    );
}

#[test]
fn a_preflight_refusal_is_passed_on_unchanged_at_construction() {
    let held = ConfinementError {
        reason: ConfinementRefusal::ScopeInvalid,
        detail: "the workspace is reached through a link".to_owned(),
    };
    let runner = Recording {
        preflight: Some(held.clone()),
        ..Recording::default()
    };
    let (refusal, scratch, governor) = refused_at_construction(Some(Arc::new(runner)));
    assert_eq!(refusal, held);
    assert!(!scratch.marker().exists());
    assert!(governor.try_observations().unwrap().is_empty());
}

#[test]
fn the_child_starts_in_the_cases_workspace_root() {
    let (scratch, child) = recorded_child("/usr/bin/true", Recording::default());
    assert_eq!(child.directory(), scratch.workspace);
}

#[test]
fn a_program_outside_the_system_view_is_mounted_at_its_own_path_under_a_non_local_runner() {
    let scratch = Scratch::new();
    let program = scratch.script("program.sh", "exit 0\n");
    let (_scratch, child) = recorded_child(&program, Recording::default());
    assert_eq!(child.host_paths(), [PathBuf::from(&program)]);
    drop(scratch);
}

#[test]
fn a_program_under_usr_is_not_mounted() {
    let (_scratch, child) = recorded_child("/usr/bin/true", Recording::default());
    assert!(child.host_paths().is_empty(), "{:?}", child.host_paths());
}

#[test]
fn no_program_is_mounted_under_a_local_runner() {
    let scratch = Scratch::new();
    let program = scratch.script("program.sh", "exit 0\n");
    let (_scratch, child) =
        recorded_child(&program, Recording::with_reason(UNCONFINED_FLAG_REASON));
    assert!(child.host_paths().is_empty(), "{:?}", child.host_paths());
    drop(scratch);
}

// ---------------------------------------------------------------------------------------------
// A host `PATH` that is not UTF-8

/// Set in the re-executed test binary, whose `PATH` is not UTF-8.
const NON_UTF8_CHILD: &str = "LOOM_BOUND_PROGRAM_NON_UTF8_PATH_CHILD";

/// What the step came to under a `PATH` that is not UTF-8.
struct NonUtf8Step {
    program: String,
    reason: String,
    marker_exists: bool,
    port_observations: usize,
    records: usize,
}

/// In the parent, re-executes this test binary for `test` alone with a `PATH` that is not UTF-8,
/// asserts it passed and answers `None`; in that child, runs one step of the marking program bound
/// to [`CHECK`] under the flag's runner and answers what it came to.
fn under_a_non_utf8_path(test: &str) -> Option<NonUtf8Step> {
    use std::os::unix::ffi::OsStrExt as _;
    if std::env::var_os(NON_UTF8_CHILD).is_some() {
        let scratch = Scratch::new();
        let document = construction_document(&scratch);
        let program = document.actions[0].executor.argv.as_ref().unwrap()[0].clone();
        let governor = governor();
        let end = {
            let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner())).unwrap();
            drive(&governor, &effects, CASE, CHECK, args("{}"))
        };
        return Some(NonUtf8Step {
            program,
            reason: refusal(&end),
            marker_exists: scratch.marker().exists(),
            port_observations: port_observations(&governor).len(),
            records: records(&governor).len(),
        });
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--nocapture", "--test-threads", "1"])
        .env(NON_UTF8_CHILD, "1")
        .env("PATH", std::ffi::OsStr::from_bytes(b"/usr/bin:/bin\xff"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
    None
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_non_utf8_path_refuses_the_step_with_the_reason_the_outcome_states() {
    let Some(step) = under_a_non_utf8_path(
        "a_non_utf8_path_refuses_the_step_with_the_reason_the_outcome_states",
    ) else {
        return;
    };
    assert_eq!(
        step.reason,
        format!(
            "{} was not started: the host's PATH is not UTF-8",
            step.program
        )
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_non_utf8_path_starts_no_program() {
    let Some(step) = under_a_non_utf8_path("a_non_utf8_path_starts_no_program") else {
        return;
    };
    assert!(!step.marker_exists, "{}", step.reason);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_non_utf8_path_records_nothing() {
    let Some(step) = under_a_non_utf8_path("a_non_utf8_path_records_nothing") else {
        return;
    };
    assert_eq!(step.port_observations, 0, "{}", step.reason);
    assert_eq!(step.records, 0, "{}", step.reason);
}

// ---------------------------------------------------------------------------------------------
// Security pass 1: every call of `run_child` counts, and a case of another protocol starts nothing

#[test]
fn a_runner_error_is_one_observation_and_the_next_step_at_the_cap_is_runs_spent() {
    let scratch = Scratch::new();
    let program = "/opt/stub/program";
    let document = document(&bound(
        CHECK,
        &[program],
        "    budget: {max_runs: 1}\n",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let error = ConfinementError {
        reason: ConfinementRefusal::CapabilityUnserved,
        detail: "the child ended with no exit status (signal Some(9))".to_owned(),
    };
    let runner = Arc::new(Recording {
        run_error: Some(error.clone()),
        ..Recording::default()
    });
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    let first = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert_eq!(
        first,
        format!("{program} was refused by its runner: {error}")
    );
    let observations = port_observations(&governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let run = plain(&observations[0].payload);
    assert_eq!(run["case"], json!(CASE), "{run}");
    assert_eq!(run["action"], json!(CHECK), "{run}");
    assert_eq!(run["runner_refusal"], json!(error.to_string()), "{run}");
    assert!(
        run["exit"].is_null() && run["blob_sha256"].is_null(),
        "{run}"
    );
    assert!(records(&governor).is_empty());
    let second = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(second.starts_with("RunsSpent"), "{second}");
    assert_eq!(
        runner.children().len(),
        1,
        "the capped action started again"
    );
}

/// A protocol with the fixture's artifacts and [`CHECK`] that declares no `gate` kind.
const OTHER_PROTOCOL: &str = r"format: protocol/1
protocol:
  id: other
  revision: 1
artifacts:
  spec: {}
  impl: {}
evidence_kinds:
  review: {}
actions:
  spec.check:
    effect: write
    may_produce:
      - evidence: review
";

#[test]
fn a_case_of_another_protocol_starts_no_program() {
    let scratch = Scratch::new();
    let members = format!(
        "    arguments_from: {}\n",
        serde_json::to_string(&["/opt/stub/arguments"]).unwrap()
    );
    let document = document(&bound(
        CHECK,
        &["/opt/stub/program"],
        &members,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor_over(MemoryCaseStore::default())
        .with_protocol_yaml("other@1", OTHER_PROTOCOL)
        .expect("the other protocol registers");
    governor
        .open_case(
            CaseId(CASE.to_owned()),
            "other@1",
            revisions(),
            CaseInputs::none(),
        )
        .expect("the case opens");
    let runner = Arc::new(Recording::default());
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap();
    let reason = refusal(&drive(&governor, &effects, CASE, CHECK, args("{}")));
    assert!(reason.starts_with("CaseOfAnotherProtocol"), "{reason}");
    assert!(
        reason.contains("other@1") && reason.contains(REGISTRATION),
        "{reason}"
    );
    assert!(
        runner.children().is_empty(),
        "a program started: {:?}",
        runner.children()
    );
    assert!(port_observations(&governor).is_empty());
    assert!(records(&governor).is_empty());
    assert!(scratch.blob_names().is_empty());
}

// ---------------------------------------------------------------------------------------------
// A cancel handle (`story:cancel-a-running-bound-program`)

/// The program a cancel ends, after its marker: a process of its own writes `target/late` 3 s
/// after it started, and it sleeps 30 s itself. A process the cancel left writes the file.
const CANCELLED_STUB: &str = "sleep 3 && touch target/late & sleep 30\n";
/// The program's timeout, far past every bound below.
const SIXTY_SECONDS: &str = "    timeout_seconds: 60\n";
/// How long after the program started its handle is set.
const CANCEL_AFTER: Duration = Duration::from_secs(1);
/// From the cancel to the step's end, under a host's kill bounds.
const SEVEN_SECONDS: Duration = Duration::from_secs(7);
/// How long after the step the test looks for `target/late`: past the 3 s the process sleeps.
const LATE_LOOK: Duration = Duration::from_secs(4);

/// Sets `cancel` once `started` exists and `after` passed, from a thread of its own; answers the
/// instant it set it.
fn cancel_when(
    started: PathBuf,
    after: Duration,
    cancel: Cancel,
) -> std::thread::JoinHandle<Instant> {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !started.exists() {
            assert!(
                Instant::now() < deadline,
                "{} never appeared",
                started.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(after);
        cancel.cancel();
        Instant::now()
    })
}

/// A step of [`CHECK`] bound to the marking [`CANCELLED_STUB`] with the extra `members`, whose
/// handle is set [`CANCEL_AFTER`] the program started.
struct CancelledStep {
    governor: CanonGovernor<MemoryCaseStore>,
    scratch: Scratch,
    end: LoopEnd,
    /// When the handle was set.
    cancelled_at: Instant,
    /// When the step returned.
    returned: Instant,
}

impl CancelledStep {
    fn run(members: &str, entries: &[String]) -> Self {
        let scratch = Scratch::new();
        std::fs::create_dir(scratch.workspace.join("target")).unwrap();
        let program = scratch.marking("program.sh", CANCELLED_STUB);
        let document = document(&bound(
            CHECK,
            &[&program],
            &format!("{SIXTY_SECONDS}{members}"),
            entries,
        ));
        let governor = governor();
        let cancel = Cancel::new();
        let setter = cancel_when(scratch.marker(), CANCEL_AFTER, cancel.clone());
        let (end, returned) = {
            let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner()))
                .unwrap()
                .with_cancel(cancel);
            let end = drive(&governor, &effects, CASE, CHECK, args("{}"));
            (end, Instant::now())
        };
        let cancelled_at = setter.join().expect("the handle was set");
        Self {
            governor,
            scratch,
            end,
            cancelled_at,
            returned,
        }
    }

    fn gate() -> Self {
        Self::run("", &[entry("gate", "exit", VALID_TABLE)])
    }

    /// The step's refusal and whether it ends the run.
    fn refused(&self) -> (String, bool) {
        match only_effect(&self.end) {
            EffectOutcome::Refused(refused) => (refused.reason.clone(), refused.ends_run),
            other => panic!("not refused: {other:?}"),
        }
    }

    fn late(&self) -> PathBuf {
        self.scratch.workspace.join("target/late")
    }
}

/// `step` was refused `Cancelled`, ending the run, within [`SEVEN_SECONDS`] of its cancel.
fn assert_cancelled_within_seven_seconds(step: &CancelledStep) {
    let (reason, ends_run) = step.refused();
    assert!(
        reason.starts_with("Cancelled: action spec.check: "),
        "{reason}"
    );
    assert!(ends_run, "a cancelled step ends the run: {reason}");
    let taken = step.returned.saturating_duration_since(step.cancelled_at);
    assert!(
        step.returned > step.cancelled_at && taken < SEVEN_SECONDS,
        "the step returned {taken:?} after its cancel"
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancelled_program_ends_within_seven_seconds() {
    assert_cancelled_within_seven_seconds(&CancelledStep::gate());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancelled_program_leaves_no_process() {
    let step = CancelledStep::gate();
    assert_cancelled_within_seven_seconds(&step);
    std::thread::sleep(LATE_LOOK);
    assert!(
        !step.late().exists(),
        "a process of the cancelled program outlived it"
    );
}

/// A runner that runs each child through `inner`, then sets `cancel`: the handle is set after the
/// program ended and before the port reads its result.
struct CancelAfterRun {
    inner: Arc<dyn TestRunner>,
    cancel: Cancel,
}

impl TestRunner for CancelAfterRun {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let answer = self.inner.run_child(child);
        self.cancel.cancel();
        answer
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

/// What a step of [`CHECK`] bound to a program that exits 0 at once leaves, its instants and ids
/// aside: the outcome's report, the records' facts and the observation's payload. With `cancel`,
/// the port holds a handle the runner sets once the program ended.
fn exits_at_once(
    cancel: Option<Cancel>,
) -> (
    serde_json::Value,
    Vec<serde_json::Value>,
    Vec<serde_json::Value>,
) {
    let scratch = Scratch::new();
    let program = scratch.script("program.sh", "exit 0\n");
    let document = document(&bound(
        CHECK,
        &[&program],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let runner: Arc<dyn TestRunner> = match &cancel {
        Some(cancel) => Arc::new(CancelAfterRun {
            inner: flag_runner(),
            cancel: cancel.clone(),
        }),
        None => flag_runner(),
    };
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(runner)).unwrap();
        let effects = match cancel.clone() {
            Some(cancel) => effects.with_cancel(cancel),
            None => effects,
        };
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    if let Some(cancel) = &cancel {
        assert!(cancel.is_cancelled(), "the runner set the handle");
    }
    let mut report = match only_effect(&end) {
        EffectOutcome::Performed(performed) => plain(&performed.report),
        other => panic!("not performed: {other:?}"),
    };
    for record in report["evidence"].as_array_mut().unwrap() {
        record["observed_at"] = serde_json::Value::Null;
    }
    let facts = records(&governor)
        .iter()
        .map(|record| {
            let mut facts = plain(&record.facts);
            facts["id"] = serde_json::Value::Null;
            facts["observed_at"] = serde_json::Value::Null;
            facts
        })
        .collect();
    let observations = port_observations(&governor)
        .iter()
        .map(|observation| plain(&observation.payload))
        .collect();
    (report, facts, observations)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_after_the_program_ended_changes_nothing() {
    let without = exits_at_once(None);
    let with = exits_at_once(Some(Cancel::new()));
    assert_eq!(with, without);
    assert_eq!(without.1.len(), 1, "{:?}", without.1);
    assert_eq!(without.2[0]["cancelled"], json!(false), "{:?}", without.2);
}

#[test]
fn a_cancel_before_the_start_refuses_the_step_and_starts_nothing() {
    let scratch = Scratch::new();
    let members = format!(
        "    arguments_from: {}\n",
        serde_json::to_string(&["/opt/stub/arguments"]).unwrap()
    );
    let document = document(&bound(
        CHECK,
        &["/opt/stub/program"],
        &members,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let runner = Arc::new(Recording::default());
    let cancel = Cancel::new();
    cancel.cancel();
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap()
    .with_cancel(cancel);
    let end = drive(&governor, &effects, CASE, CHECK, args("{}"));
    match only_effect(&end) {
        EffectOutcome::Refused(refused) => {
            assert!(
                refused.reason.starts_with("Cancelled: action spec.check: "),
                "{}",
                refused.reason
            );
            assert!(refused.ends_run, "{}", refused.reason);
        }
        other => panic!("not refused: {other:?}"),
    }
    assert!(
        runner.children().is_empty(),
        "a child was given to the runner: {:?}",
        runner.children()
    );
    assert!(port_observations(&governor).is_empty());
    assert!(records(&governor).is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_during_the_arguments_program_starts_no_bound_program() {
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.workspace.join("target")).unwrap();
    let arguments_started = scratch.root.join("arguments-started");
    let from = scratch.script(
        "arguments.sh",
        &format!(
            "/usr/bin/touch '{}'\n{CANCELLED_STUB}",
            arguments_started.display()
        ),
    );
    let program = scratch.marking("program.sh", "exit 0\n");
    let members = format!(
        "{SIXTY_SECONDS}    arguments_from: {}\n",
        serde_json::to_string(&[&from]).unwrap()
    );
    let document = document(&bound(
        CHECK,
        &[&program],
        &members,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let cancel = Cancel::new();
    let setter = cancel_when(arguments_started, CANCEL_AFTER, cancel.clone());
    let (end, returned) = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner()))
            .unwrap()
            .with_cancel(cancel);
        let end = drive(&governor, &effects, CASE, CHECK, args("{}"));
        (end, Instant::now())
    };
    let cancelled_at = setter.join().expect("the handle was set");
    let reason = refusal(&end);
    assert!(
        reason.starts_with("Cancelled: action spec.check: "),
        "{reason}"
    );
    assert!(
        returned.saturating_duration_since(cancelled_at) < SEVEN_SECONDS,
        "{reason}"
    );
    assert!(!scratch.marker().exists(), "the bound program started");
    assert!(records(&governor).is_empty());
    std::thread::sleep(LATE_LOOK);
    assert!(
        !scratch.workspace.join("target/late").exists(),
        "a process of the cancelled arguments program outlived it"
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancelled_step_records_no_evidence() {
    // Every value, a run with no exit included, falls to `*`: only the cancel records nothing.
    let step = CancelledStep::run("", &[entry("gate", "exit", r#"{"*": invalid}"#)]);
    assert_cancelled_within_seven_seconds(&step);
    assert!(
        records(&step.governor).is_empty(),
        "{:#?}",
        records(&step.governor)
    );
    assert!(
        matches!(only_effect(&step.end), EffectOutcome::Refused(_)),
        "a refused step carries no report, so no evidence"
    );
    assert!(step.scratch.blob_names().is_empty(), "a blob was kept");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancelled_run_is_observed_and_counted() {
    let step = CancelledStep::run(
        "    budget: {max_runs: 1}\n",
        &[entry("gate", "exit", VALID_TABLE)],
    );
    assert_cancelled_within_seven_seconds(&step);
    let observations = port_observations(&step.governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    let run = plain(&observations[0].payload);
    assert_eq!(run["cancelled"], json!(true), "{run}");
    assert!(
        run["exit"].is_null() && run["blob_sha256"].is_null(),
        "{run}"
    );
    assert!(
        run["runner_refusal"]
            .as_str()
            .is_some_and(|refusal| refusal.starts_with("Cancelled")),
        "{run}"
    );
    // The next step of the action, with no handle, finds its one run spent.
    let program = step.scratch.marking("program.sh", "exit 0\n");
    std::fs::remove_file(step.scratch.marker()).unwrap();
    let document = document(&bound(
        CHECK,
        &[&program],
        &format!("{SIXTY_SECONDS}    budget: {{max_runs: 1}}\n"),
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let effects = port(
        &step.governor,
        &document,
        &step.scratch,
        CASE,
        Some(flag_runner()),
    )
    .unwrap();
    let reason = refusal(&drive(&step.governor, &effects, CASE, CHECK, args("{}")));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
    assert!(!step.scratch.marker().exists(), "the spent action started");
}

/// A loop context of three steps.
struct ThreeSteps;

impl LoopContext for ThreeSteps {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(uuid(next()))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(uuid(next()))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(3)
    }
}

/// An effect port that counts its invocations and hands each to `inner`.
struct Counted<'p, P> {
    inner: &'p P,
    invoked: AtomicUsize,
}

impl<P: EffectPort> EffectPort for Counted<'_, P> {
    fn performs(&self, action: &str) -> bool {
        self.inner.performs(action)
    }

    fn invoke(
        &self,
        commission: &Commission<
            b10x_loom_commission::model::responsibility::commission_state::Assigned,
        >,
        request: &b10x_loom_commission::ports::effect::AdmittedRequest,
    ) -> Result<EffectOutcome, b10x_loom_commission::ports::effect::EffectError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        self.inner.invoke(commission, request)
    }
}

#[test]
fn a_cancelled_step_ends_the_run_and_is_not_invoked_again() {
    let scratch = Scratch::new();
    let document = document(&bound(
        CHECK,
        &["/opt/stub/program"],
        "",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    // The runner answers as one whose kill a cancel ended the program.
    let runner = Arc::new(Recording {
        run_error: Some(ConfinementError {
            reason: ConfinementRefusal::Cancelled,
            detail: "the child was killed by its cancel after 1.0 s".to_owned(),
        }),
        ..Recording::default()
    });
    let effects = port(
        &governor,
        &document,
        &scratch,
        CASE,
        Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
    )
    .unwrap()
    .with_cancel(Cancel::new());
    let counted = Counted {
        inner: &effects,
        invoked: AtomicUsize::new(0),
    };
    let proposal = || {
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: CHECK.to_owned(),
            arguments: ProposedActionArguments(args("{}")),
        })
    };
    let executor = ScriptedExecutor::new([proposal(), proposal(), proposal()]);
    let mut runs = Generated::new(RunStore::new(|| RunId(uuid(next()))));
    let end = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &counted,
        &commission(CASE),
        &mut runs,
        &mut ThreeSteps,
    )
    .expect("the loop ends");
    assert!(
        matches!(
            end.outcome,
            b10x_loom_commission::model::responsibility::RunOutcome::EffectRefused(_)
        ),
        "{:?}",
        end.outcome
    );
    assert_eq!(counted.invoked.load(Ordering::SeqCst), 1);
    assert_eq!(runner.children().len(), 1);
    let reason = refusal(&end);
    assert!(
        reason.starts_with("Cancelled: action spec.check: "),
        "{reason}"
    );
    let observations = port_observations(&governor);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    assert_eq!(plain(&observations[0].payload)["cancelled"], json!(true));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_of_one_run_leaves_the_other_running() {
    let first = Scratch::new();
    std::fs::create_dir(first.workspace.join("target")).unwrap();
    let second = Scratch::new();
    let cancelled = first.marking("program.sh", CANCELLED_STUB);
    // Still running when the first run's handle is set, then exits 0.
    let finishing = second.marking("program.sh", "sleep 3\nexit 0\n");
    let first_document = document(&bound(
        CHECK,
        &[&cancelled],
        SIXTY_SECONDS,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let second_document = document(&bound(
        CHECK,
        &[&finishing],
        SIXTY_SECONDS,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let first_governor = governor();
    let second_governor = governor();
    let first_cancel = Cancel::new();
    let second_cancel = Cancel::new();
    let setter = cancel_when(first.marker(), CANCEL_AFTER, first_cancel.clone());
    let (first_end, second_end) = std::thread::scope(|scope| {
        let other = scope.spawn(|| {
            let effects = port(
                &second_governor,
                &second_document,
                &second,
                CASE,
                Some(flag_runner()),
            )
            .unwrap()
            .with_cancel(second_cancel.clone());
            drive(&second_governor, &effects, CASE, CHECK, args("{}"))
        });
        let effects = port(
            &first_governor,
            &first_document,
            &first,
            CASE,
            Some(flag_runner()),
        )
        .unwrap()
        .with_cancel(first_cancel.clone());
        let first_end = drive(&first_governor, &effects, CASE, CHECK, args("{}"));
        (first_end, other.join().expect("the second run ends"))
    });
    setter.join().expect("the first handle was set");
    let reason = refusal(&first_end);
    assert!(reason.starts_with("Cancelled"), "{reason}");
    assert!(!second_cancel.is_cancelled());
    assert_performed(&second_end);
    let run = plain(&port_observations(&second_governor)[0].payload);
    assert_eq!(run["exit"], json!(0), "{run}");
    assert_eq!(run["cancelled"], json!(false), "{run}");
    assert_eq!(result_of(&record(&second_governor)), json!("valid"));
}

/// What the documents of a cancel under `local` say of a process the program started in a
/// session of its own: the kill reaches the program's process group and not it.
const LEFT_THE_GROUP: &str = "a process that left the group";

/// The text of `path`, relative to this crate's directory.
fn text_of(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A program that starts two processes, each writing a file of `target/` 3 s later with its
/// output detached: `grouped` in its own process group, `late` in a session of its own
/// (`setsid`); it sleeps 30 s itself.
const SETSID_STUB: &str = "(sleep 3 && touch target/grouped) </dev/null >/dev/null 2>&1 &\n/usr/bin/setsid /bin/sh -c 'sleep 3 && touch target/late' </dev/null >/dev/null 2>&1 &\nsleep 30\n";

/// Under `local` a cancel kills the program's process group: what the program started in its group
/// ends with it, and a process that left the group (`setsid`) survives. The `Cancel` handle, the
/// port's `with_cancel` and the guide pages say so, never "every process it started" alone.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_under_local_kills_the_group_and_not_a_process_that_left_it() {
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.workspace.join("target")).unwrap();
    let program = scratch.marking("program.sh", SETSID_STUB);
    let document = document(&bound(
        CHECK,
        &[&program],
        SIXTY_SECONDS,
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let cancel = Cancel::new();
    let setter = cancel_when(scratch.marker(), CANCEL_AFTER, cancel.clone());
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(flag_runner()))
            .unwrap()
            .with_cancel(cancel);
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    setter.join().expect("the handle was set");
    let reason = refusal(&end);
    assert!(reason.starts_with("Cancelled"), "{reason}");
    let late = scratch.workspace.join("target/late");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !late.exists() {
        assert!(
            Instant::now() < deadline,
            "the process that left the group did not survive the cancel"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !scratch.workspace.join("target/grouped").exists(),
        "a process of the program's group outlived its cancel"
    );
    for path in [
        "../loom-intake-slice/src/confinement.rs",
        "src/program.rs",
        "../../website/docs/guides/bindings.md",
        "../../website/docs/guides/run-a-case-tree.md",
    ] {
        assert!(
            text_of(path).contains(LEFT_THE_GROUP),
            "{path} does not say that {LEFT_THE_GROUP} survives a cancel under local"
        );
    }
}

/// A runner that sets `cancel` when the port hands it a child and before the child starts: the
/// handle is set after the port's own check (`program.rs`, before any program starts) and before
/// the runner's, as a cancel from another thread lands while the port digests the placed
/// artifacts, reads the case names or the retry feedback.
struct CancelAtHandOver {
    inner: Arc<dyn TestRunner>,
    cancel: Cancel,
}

impl TestRunner for CancelAtHandOver {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn label(&self) -> String {
        self.inner.label()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        self.cancel.cancel();
        self.inner.run_child(child)
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

/// What `with_cancel` and the guide page say of a handle set after the port read it and before
/// the runner starts the program.
const HANDED_OVER: &str = "after the port read it and before the runner starts";

/// A handle set after the port's own check and before the runner starts the program starts
/// nothing, and the step is refused `Cancelled`; the port keeps the run's observation
/// (`cancelled: true`), which counts against `budget.max_runs` (the counting rule is
/// `story:cancelled-step-may-run-again`'s). `with_cancel` and the guide page say so.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_cancel_set_at_the_hand_over_starts_nothing_and_keeps_a_counted_observation() {
    let scratch = Scratch::new();
    let program = scratch.marking("program.sh", "exit 0\n");
    let document = document(&bound(
        CHECK,
        &[&program],
        "    budget: {max_runs: 1}\n",
        &[entry("gate", "exit", VALID_TABLE)],
    ));
    let governor = governor();
    let cancel = Cancel::new();
    let runner: Arc<dyn TestRunner> = Arc::new(CancelAtHandOver {
        inner: flag_runner(),
        cancel: cancel.clone(),
    });
    let end = {
        let effects = port(&governor, &document, &scratch, CASE, Some(runner))
            .unwrap()
            .with_cancel(cancel);
        drive(&governor, &effects, CASE, CHECK, args("{}"))
    };
    let reason = refusal(&end);
    assert!(reason.starts_with("Cancelled"), "{reason}");
    assert!(!scratch.marker().exists(), "the program started");
    let observations: Vec<_> = port_observations(&governor)
        .iter()
        .map(|observation| plain(&observation.payload))
        .collect();
    assert_eq!(observations.len(), 1, "{observations:?}");
    assert_eq!(observations[0]["cancelled"], json!(true), "{observations:?}");
    assert!(records(&governor).is_empty());
    for path in ["src/program.rs", "../../website/docs/guides/bindings.md"] {
        assert!(
            text_of(path).contains(HANDED_OVER),
            "{path} does not say what a handle set {HANDED_OVER} the program does"
        );
    }
}
