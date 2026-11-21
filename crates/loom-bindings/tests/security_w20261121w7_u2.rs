//! Security review of `story:bound-program-result-table` (wave 2026-11-21-w7, unit U2, pass 1):
//! conformance checks of the program effect port against its own contract. Cases that read only
//! what the port records give it a runner that starts nothing and answers a fixed report; cases
//! about the run count start real programs under `local` with the flag's reason, as the unit's own
//! suite does, and are ignored off Linux. No model, no network.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};

use b10x_loom_bindings::{ProgramEffects, check, decode};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ObservationId, PrincipalId, ProposedActionArguments, RunId,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, LoopError, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::{
    UNCONFINED_FILE_REASON, UNCONFINED_FLAG_REASON, admit_bound_program_runner,
};
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, ChildCommand, ChildOutput, ConfinementAdapter, ConfinementError,
    ConfinementRefusal, EngineConfinement, EngineRunner, TestExecution, TestRunner,
};
use b10x_loom_intake_slice::executor::TestCommand;
use loom::bindings::BindingsDocument;
use loom_governor::{CanonGovernor, CaseInputs, FallibleCaseStore, FileCaseStore, MemoryCaseStore};

const REGISTRATION: &str = "fixture@1";
const OTHER: &str = "other@1";
const CASE: &str = "case-1";
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
      - evidence: review
";

/// A protocol with the same artifacts and action as [`PROTOCOL`] that declares no `gate` kind.
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

fn revisions() -> BTreeMap<String, String> {
    [
        ("spec".to_owned(), "s1".to_owned()),
        ("impl".to_owned(), "i1".to_owned()),
    ]
    .into()
}

fn governor_over<S: FallibleCaseStore>(store: S) -> CanonGovernor<S> {
    CanonGovernor::new(store)
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers")
        .with_protocol_yaml(OTHER, OTHER_PROTOCOL)
        .expect("the other protocol registers")
}

fn open<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str, registration: &str) {
    governor
        .open_case(
            CaseId(case.to_owned()),
            registration,
            revisions(),
            CaseInputs::none(),
        )
        .expect("the case opens");
}

fn governor() -> CanonGovernor<MemoryCaseStore> {
    let governor = governor_over(MemoryCaseStore::default());
    open(&governor, CASE, REGISTRATION);
    governor
}

/// The document binding [`CHECK`] to `argv` with the extra `members` and one `gate` entry about
/// `spec` read `from` through `table`.
fn document(argv: &[&str], members: &str, from: &str, table: &str) -> BindingsDocument {
    let argv = serde_json::to_string(argv).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  spec: docs/spec\n  impl: src\nactions:\n  - action: {CHECK}\n    executor: {{kind: program, argv: {argv}}}\n{members}    evidence:\n      - {{kind: gate, subject: spec, producer: ess, from: \"{from}\", results: {table}, upstream: [impl]}}\n"
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    let protocol = b10x_canon::model::parse(PROTOCOL).unwrap();
    check(&document, REGISTRATION, &protocol).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

const TABLE: &str = r#"{"ok": valid, "*": invalid}"#;

struct Scratch {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("security-w7-u2-")
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

    /// An executable `/bin/sh` script that appends one line to [`Scratch::starts`], then runs
    /// `rest`; its absolute path.
    fn counting(&self, rest: &str) -> String {
        let path = self.root.join("program.sh");
        let body = format!(
            "#!/bin/sh\necho start >> '{}'\n{rest}",
            self.starts_file().display()
        );
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn starts_file(&self) -> PathBuf {
        self.root.join("starts")
    }

    /// How many times the counting program started.
    fn starts(&self) -> usize {
        std::fs::read_to_string(self.starts_file())
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }
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

/// A runner that starts nothing: it keeps each child and answers, in order, the standard outputs
/// it was given (exit 0), then `{}`.
#[derive(Default)]
struct Answering {
    answers: Mutex<Vec<(Vec<u8>, bool)>>,
    children: Mutex<Vec<ChildCommand>>,
}

impl Answering {
    fn new(answers: &[(&[u8], bool)]) -> Arc<Self> {
        Arc::new(Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .rev()
                    .map(|(bytes, cut)| (bytes.to_vec(), *cut))
                    .collect(),
            ),
            children: Mutex::default(),
        })
    }

    fn children(&self) -> Vec<ChildCommand> {
        self.children.lock().unwrap().clone()
    }
}

impl TestRunner for Answering {
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
        let (stdout, stdout_truncated) = self
            .answers
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| (b"{}".to_vec(), false));
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
                stdout_truncated,
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

/// One step of `case` in which the executor proposes [`CHECK`] with `arguments`.
fn try_drive<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    effects: &ProgramEffects<'_, S>,
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
}

fn effect(end: &LoopEnd) -> &EffectOutcome {
    assert_eq!(end.effects.len(), 1, "{:?}", end.effects);
    &end.effects[0]
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

fn records<S: FallibleCaseStore>(governor: &CanonGovernor<S>, case: &str) -> Vec<EvidenceData> {
    governor.evidence(&CaseId(case.to_owned())).unwrap()
}

/// The facts of `record` without the members that differ by run (`id`, `observed_at`).
fn stable_facts(record: &EvidenceData) -> serde_json::Value {
    let mut facts = plain(&record.facts);
    let object = facts.as_object_mut().unwrap();
    object.remove("id");
    object.remove("observed_at");
    facts
}

/// One step of [`CHECK`] read at `from` through [`TABLE`], whose program prints `report`; the
/// governor and the step.
fn report_case(from: &str, report: &[u8]) -> (CanonGovernor<MemoryCaseStore>, LoopEnd) {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "", from, TABLE);
    let governor = governor();
    let runner = Answering::new(&[(report, false)]);
    let end = {
        let effects = ProgramEffects::new(
            &governor,
            CaseId(CASE.to_owned()),
            &document,
            &scratch.workspace,
            &scratch.store,
            Some(runner as Arc<dyn TestRunner>),
        )
        .unwrap();
        try_drive(&governor, &effects, CASE, "{}").unwrap()
    };
    (governor, end)
}

fn refused_reason(end: &LoopEnd) -> String {
    match effect(end) {
        EffectOutcome::Refused(refused) => refused.reason.clone(),
        other => panic!("not refused: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 5 and 6: what the report can and cannot name

#[test]
fn report_members_named_like_every_record_field_change_nothing() {
    let (baseline, _) = report_case("/status", br#"{"status": "ok"}"#);
    let (steered, end) = report_case(
        "/status",
        br#"{"status": "ok", "result": "invalid", "kind": "review", "subject": "impl",
            "producer": "mallory", "revision": "9", "subject_revision": "9",
            "observed_at": "1999-01-01T00:00:00Z", "upstream_revisions": {"impl": "x"},
            "evidence": [{"kind": "review", "subject": "impl", "result": "valid"}]}"#,
    );
    assert!(
        matches!(effect(&end), EffectOutcome::Performed(_)),
        "{end:?}"
    );
    let (baseline, steered) = (records(&baseline, CASE), records(&steered, CASE));
    assert_eq!(steered.len(), 1, "{steered:#?}");
    assert_eq!(steered[0].kind, "gate");
    assert_eq!(steered[0].producer, "ess");
    assert_eq!(steered[0].subject_revision, baseline[0].subject_revision);
    assert_eq!(stable_facts(&steered[0]), stable_facts(&baseline[0]));
}

#[test]
fn a_member_named_twice_through_an_escape_is_refused_and_records_nothing() {
    let (governor, end) = report_case("/status", br#"{"status": "bad", "status": "ok"}"#);
    let reason = refused_reason(&end);
    assert!(reason.starts_with("ReportUnreadable"), "{reason}");
    assert!(records(&governor, CASE).is_empty());
}

#[test]
fn a_pointer_with_escapes_and_an_index_reads_only_its_target() {
    let (governor, end) = report_case(
        "/a~1b/1/c~0d",
        br#"{"status": "bad", "a/b": [{"c~d": "bad"}, {"c~d": "ok"}], "a": {"b": ["ok", "bad"]}}"#,
    );
    assert!(
        matches!(effect(&end), EffectOutcome::Performed(_)),
        "{end:?}"
    );
    let records = records(&governor, CASE);
    assert_eq!(records.len(), 1);
    assert_eq!(plain(&records[0].facts)["result"], "valid");
}

#[test]
fn a_pointer_target_of_another_type_reaches_only_its_own_tables_star() {
    for report in [
        &br#"{"status": {"kind": "review", "subject": "impl", "result": "ok"}}"#[..],
        br#"{"status": ["ok"]}"#,
        br#"{"status": true}"#,
        br#"{"status": null}"#,
    ] {
        let (governor, end) = report_case("/status", report);
        assert!(
            matches!(effect(&end), EffectOutcome::Performed(_)),
            "{end:?}"
        );
        let records = records(&governor, CASE);
        assert_eq!(records.len(), 1, "{records:#?}");
        let facts = plain(&records[0].facts);
        assert_eq!(
            (records[0].kind.as_str(), facts["subject"].as_str()),
            ("gate", Some("spec"))
        );
        assert_eq!(
            facts["result"],
            "invalid",
            "{}",
            String::from_utf8_lossy(report)
        );
    }
}

#[test]
fn a_report_nested_past_the_bound_is_refused_and_records_nothing() {
    let report = format!(
        r#"{{"status": "ok", "deep": {}{}}}"#,
        "[".repeat(80),
        "]".repeat(80)
    );
    let (governor, end) = report_case("/status", report.as_bytes());
    let reason = refused_reason(&end);
    assert!(reason.starts_with("ReportUnreadable"), "{reason}");
    assert!(records(&governor, CASE).is_empty());
}

#[test]
fn a_case_of_another_protocol_records_no_kind_that_protocol_lacks() {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], "", "/status", TABLE);
    let governor = governor_over(MemoryCaseStore::default());
    open(&governor, "case-other", OTHER);
    let runner = Answering::new(&[(br#"{"status": "ok"}"#, false)]);
    let effects = ProgramEffects::new(
        &governor,
        CaseId("case-other".to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(runner as Arc<dyn TestRunner>),
    )
    .unwrap();
    let _ = try_drive(&governor, &effects, "case-other", "{}");
    let gate: Vec<_> = records(&governor, "case-other")
        .into_iter()
        .filter(|record| record.kind == "gate")
        .collect();
    assert!(
        gate.is_empty(),
        "a case of {OTHER}, which declares no gate kind, holds {gate:#?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 3 and 4: model-chosen text reaches standard input alone

#[test]
fn model_chosen_arguments_reach_standard_input_alone() {
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program", "--fixed"], "", "/status", TABLE);
    let governor = governor();
    let runner = Answering::new(&[(br#"{"status": "ok"}"#, false)]);
    let arguments = r#"{"argv": ["--evil"], "PATH": "/evil", "cwd": "/", "$(id)": "`id`"}"#;
    {
        let effects = ProgramEffects::new(
            &governor,
            CaseId(CASE.to_owned()),
            &document,
            &scratch.workspace,
            &scratch.store,
            Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
        )
        .unwrap();
        try_drive(&governor, &effects, CASE, arguments).unwrap();
    }
    let children = runner.children();
    assert_eq!(children.len(), 1);
    let child = &children[0];
    assert_eq!(child.program(), "/opt/stub/program");
    assert_eq!(child.args(), ["--fixed".to_owned()]);
    assert_eq!(child.directory(), scratch.workspace);
    let environment = child.environment();
    assert!(environment.allow.is_empty());
    let expected: BTreeMap<String, String> = std::env::var("PATH")
        .ok()
        .map(|path| ("PATH".to_owned(), path))
        .into_iter()
        .collect();
    assert_eq!(environment.set, expected);
    let stdin: serde_json::Value = serde_json::from_slice(child.stdin().unwrap()).unwrap();
    assert_eq!(
        stdin,
        serde_json::from_str::<serde_json::Value>(arguments).unwrap()
    );
}

#[test]
fn arguments_from_output_reaches_standard_input_alone_and_over_the_bound_starts_nothing() {
    let members = "    arguments_from: [/opt/stub/args]\n";
    // Its object is data: the bound child's argv, environment and directory are the binding's.
    let scratch = Scratch::new();
    let document = document(&["/opt/stub/program"], members, "/status", TABLE);
    let governor = governor();
    let printed = br#"{"x": "; rm -rf / #", "PATH": "/evil"}"#;
    let runner = Answering::new(&[(printed, false), (br#"{"status": "ok"}"#, false)]);
    {
        let effects = ProgramEffects::new(
            &governor,
            CaseId(CASE.to_owned()),
            &document,
            &scratch.workspace,
            &scratch.store,
            Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
        )
        .unwrap();
        try_drive(&governor, &effects, CASE, "{}").unwrap();
    }
    let children = runner.children();
    assert_eq!(children.len(), 2);
    assert_eq!(children[1].program(), "/opt/stub/program");
    assert!(children[1].args().is_empty());
    assert_eq!(children[1].environment().set.len(), 1);
    let stdin: serde_json::Value = serde_json::from_slice(children[1].stdin().unwrap()).unwrap();
    assert_eq!(
        stdin,
        serde_json::from_slice::<serde_json::Value>(printed).unwrap()
    );
    // Over the bound, or cut by the runner: refused, and the bound program never starts.
    let big = format!(r#"{{"pad": "{}"}}"#, "a".repeat(1024 * 1024));
    for (bytes, cut) in [(big.as_bytes(), false), (&br#"{"a": 1}"#[..], true)] {
        let scratch = Scratch::new();
        let governor = self::governor();
        let runner = Answering::new(&[(bytes, cut)]);
        let end = {
            let effects = ProgramEffects::new(
                &governor,
                CaseId(CASE.to_owned()),
                &document,
                &scratch.workspace,
                &scratch.store,
                Some(Arc::clone(&runner) as Arc<dyn TestRunner>),
            )
            .unwrap();
            try_drive(&governor, &effects, CASE, "{}").unwrap()
        };
        let reason = refused_reason(&end);
        assert!(reason.starts_with("ArgumentsUnreadable"), "{reason}");
        assert_eq!(runner.children().len(), 1, "the bound program started");
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 9: the public admission as API

#[test]
fn the_public_admission_refuses_local_for_the_file_reason_and_near_misses_of_the_flags() {
    for reason in [
        UNCONFINED_FILE_REASON.to_owned(),
        format!("{UNCONFINED_FLAG_REASON} "),
        UNCONFINED_FLAG_REASON.to_uppercase(),
    ] {
        let runner: Arc<dyn TestRunner> = Arc::new(
            EngineRunner::open(&EngineConfinement {
                adapter: ConfinementAdapter::Local,
                local_reason: Some(reason.clone()),
                cgroup_root: None,
            })
            .expect("local opens"),
        );
        let refusal = admit_bound_program_runner(Some(&runner))
            .err()
            .unwrap_or_else(|| panic!("admitted local for {reason:?}"));
        assert_eq!(refusal.reason, ConfinementRefusal::CommandUnconfined);
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 8: every run that started counts against `budget.max_runs`

const CAPPED: &str = "    budget: {max_runs: 1}\n";
const EXIT_TABLE: &str = r#"{"0": valid, "*": invalid}"#;

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_program_that_ends_by_a_signal_counts_against_its_max_runs() {
    let scratch = Scratch::new();
    let program = scratch.counting("kill -KILL $$\n");
    let document = document(&[&program], CAPPED, "exit", EXIT_TABLE);
    let governor = governor();
    let effects = ProgramEffects::new(
        &governor,
        CaseId(CASE.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(flag_runner()),
    )
    .unwrap();
    let mut ends = Vec::new();
    for _ in 0..3 {
        ends.push(format!("{:?}", try_drive(&governor, &effects, CASE, "{}")));
    }
    assert_eq!(
        scratch.starts(),
        1,
        "an action with budget.max_runs 1 started {} times; the steps ended {ends:#?}",
        scratch.starts()
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_bound_program_whose_blob_cannot_be_kept_still_counts_against_its_max_runs() {
    let scratch = Scratch::new();
    let blobs = scratch.store.join("blobs");
    let program = scratch.counting(&format!(
        "/usr/bin/rm -rf '{0}'\n: > '{0}'\nexit 0\n",
        blobs.display()
    ));
    let document = document(&[&program], CAPPED, "exit", EXIT_TABLE);
    let governor = governor();
    let effects = ProgramEffects::new(
        &governor,
        CaseId(CASE.to_owned()),
        &document,
        &scratch.workspace,
        &scratch.store,
        Some(flag_runner()),
    )
    .unwrap();
    let mut ends = Vec::new();
    for _ in 0..3 {
        ends.push(format!("{:?}", try_drive(&governor, &effects, CASE, "{}")));
    }
    assert_eq!(
        scratch.starts(),
        1,
        "an action with budget.max_runs 1 started {} times; the steps ended {ends:#?}",
        scratch.starts()
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
#[ignore = "story:bound-program-run-cap-holds-across-ports"]
fn two_ports_over_one_case_store_start_a_capped_action_at_most_max_runs_times() {
    let scratch = Scratch::new();
    let program = scratch.counting("/usr/bin/sleep 1\nexit 0\n");
    let document = document(&[&program], CAPPED, "exit", EXIT_TABLE);
    {
        let governor = governor_over(FileCaseStore::open(&scratch.store).unwrap());
        open(&governor, CASE, REGISTRATION);
    }
    let barrier = Barrier::new(2);
    let ends: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let governor = governor_over(FileCaseStore::open(&scratch.store).unwrap());
                    let effects = ProgramEffects::new(
                        &governor,
                        CaseId(CASE.to_owned()),
                        &document,
                        &scratch.workspace,
                        &scratch.store,
                        Some(flag_runner()),
                    )
                    .unwrap();
                    barrier.wait();
                    format!("{:?}", try_drive(&governor, &effects, CASE, "{}"))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(
        scratch.starts(),
        1,
        "an action with budget.max_runs 1 started {} times through two ports over one case; the steps ended {ends:#?}",
        scratch.starts()
    );
}
