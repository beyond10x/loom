//! Acceptance for `story:bound-program-retry-records`: an action bound to a `program` whose
//! binding declares `retry_when` is sent back by the records of those kinds and results about the
//! current revision of their subject (`retry_records`), runs again within its `budget.max_runs`,
//! and is given those records as one `loom.retry-feedback/1` document named by
//! `LOOM_RETRY_FEEDBACK`, a variable the port alone sets.
//!
//! The fixture is one protocol with the artifacts `brief` and `draft`: an observation
//! `draft.check` (`draft_check`, exit `0` `valid`, `*` `invalid`), a second observation
//! `draft.lint` (`draft_lint`, the same table) and a writer `draft.write` (effect class `write`)
//! with `retry_when: {draft_check: [invalid]}` and `budget.max_runs: 3`; and a second protocol
//! for a case of another protocol. Every step reaches the port through Commission's own loop.
//! A case that reads the `ChildCommand` uses a recording runner that starts nothing; a case that
//! reads a blob runs `/bin/sh` stubs under an `EngineRunner` on `local` opened with
//! `UNCONFINED_FLAG_REASON`. Revisions move through `CanonGovernor::update_revision`. No model,
//! no network, no timer. The parsed protocols and the stub programs are made once per binary.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use b10x_loom_bindings::{
    PROGRAM_SOURCE, ProgramEffects, RETRY_FEEDBACK_FORMAT, RETRY_FEEDBACK_VARIABLE,
    RETRY_LOG_BYTES, RETRY_LOG_LINES, check, decode, retry_records,
};
use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EvidenceData, EvidenceId, ExecutorOutcome,
    ExecutorOutcomeProposedAction, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    AppliedConfinement, Backend, ChildCommand, ChildOutput, ConfinementAdapter, ConfinementError,
    ConfinementRefusal, EngineConfinement, EngineRunner, TestExecution, TestRunner,
};
use b10x_loom_intake_slice::executor::TestCommand;
use loom::bindings::{BindingsDocument, BoundAction, RetryFeedback, RetryLog, RetryRecord};
use loom_governor::{CanonGovernor, CaseInputs, MemoryCaseStore};
use sha2::{Digest as _, Sha256};

const REGISTRATION: &str = "draft@1";
const OTHER_REGISTRATION: &str = "other@1";
const CASE: &str = "case-1";
const CHECK: &str = "draft.check";
const LINT: &str = "draft.lint";
const WRITE: &str = "draft.write";

const PROTOCOL: &str = r"format: protocol/1
protocol:
  id: draft
  revision: 1
artifacts:
  brief: {}
  draft: {}
evidence_kinds:
  draft_check: {}
  draft_lint: {}
  draft_written: {}
actions:
  draft.check:
    effect: read
    may_produce:
      - evidence: draft_check
  draft.lint:
    effect: read
    may_produce:
      - evidence: draft_lint
  draft.write:
    effect: write
    may_produce:
      - evidence: draft_written
";

/// The protocol of the case of another protocol: the same artifacts and check.
const OTHER_PROTOCOL: &str = r"format: protocol/1
protocol:
  id: other
  revision: 1
artifacts:
  brief: {}
  draft: {}
evidence_kinds:
  draft_check: {}
actions:
  draft.check:
    effect: read
    may_produce:
      - evidence: draft_check
";

const BRIEF_REVISION: &str = "b1";
const DRAFT_REVISION: &str = "d1";

/// The writer's members in the fixture: it is sent back by an `invalid` `draft_check`.
const RETRY: &str = "    retry_when: {draft_check: [invalid]}\n    budget: {max_runs: 3}\n";
/// The writer's members without `retry_when`.
const NO_RETRY: &str = "    budget: {max_runs: 3}\n";

/// The programs a recording runner is given: none of them exists, none starts.
const CHECK_STUB: &str = "/opt/stub/draft-check";
const LINT_STUB: &str = "/opt/stub/draft-lint";
const WRITE_STUB: &str = "/opt/stub/draft-write";
const ARGUMENTS_STUB: &str = "/opt/stub/draft-arguments";

// ---------------------------------------------------------------------------------------------
// Fixture, made once per binary

fn protocol() -> &'static b10x_canon::model::Protocol {
    static PARSED: OnceLock<b10x_canon::model::Protocol> = OnceLock::new();
    PARSED.get_or_init(|| b10x_canon::model::parse(PROTOCOL).expect("the fixture protocol parses"))
}

/// The stub programs a case under the `local` runner starts, written once into a directory of
/// this binary's scratch root through a temporary file and a rename.
struct Stubs {
    /// Exits 1: an `invalid` `draft_check`.
    check_invalid: String,
    /// Prints the lines `line 1` to `line 100`, then exits 1.
    check_hundred_lines: String,
    /// Prints one line of 200000 bytes, then exits 1.
    check_long_line: String,
    /// Appends `run` to the file `runs` of its start directory, prints `LOOM_RETRY_FEEDBACK` and
    /// exits 0.
    write: String,
}

fn stubs() -> &'static Stubs {
    static STUBS: OnceLock<Stubs> = OnceLock::new();
    STUBS.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bound-retry-stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let script = |name: &str, body: &str| {
            let path = dir.join(name);
            let partial = dir.join(format!(".{name}.{}", std::process::id()));
            std::fs::write(&partial, format!("#!/bin/sh\n{body}")).unwrap();
            std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::rename(&partial, &path).unwrap();
            path.to_str().unwrap().to_owned()
        };
        Stubs {
            check_invalid: script("check-invalid.sh", "echo invalid\nexit 1\n"),
            check_hundred_lines: script(
                "check-hundred-lines.sh",
                "i=1\nwhile [ $i -le 100 ]; do echo \"line $i\"; i=$((i + 1)); done\nexit 1\n",
            ),
            check_long_line: script(
                "check-long-line.sh",
                "/usr/bin/head -c 200000 /dev/zero | /usr/bin/tr '\\0' 'a'\necho\nexit 1\n",
            ),
            write: script(
                "write.sh",
                "echo run >> runs\nprintf '%s' \"${LOOM_RETRY_FEEDBACK-}\"\nexit 0\n",
            ),
        }
    })
}

/// A governor holding [`CASE`] under the fixture protocol, `brief` at [`BRIEF_REVISION`] and
/// `draft` at `draft`.
fn governor_at(draft: &str) -> CanonGovernor<MemoryCaseStore> {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_protocol_yaml(REGISTRATION, PROTOCOL)
        .expect("the fixture protocol registers")
        .with_protocol_yaml(OTHER_REGISTRATION, OTHER_PROTOCOL)
        .expect("the other protocol registers");
    open(&governor, CASE, REGISTRATION, draft);
    governor
}

fn governor() -> CanonGovernor<MemoryCaseStore> {
    governor_at(DRAFT_REVISION)
}

fn open(governor: &CanonGovernor<MemoryCaseStore>, case: &str, registration: &str, draft: &str) {
    governor
        .open_case(
            CaseId(case.to_owned()),
            registration,
            [
                ("brief".to_owned(), BRIEF_REVISION.to_owned()),
                ("draft".to_owned(), draft.to_owned()),
            ]
            .into(),
            CaseInputs::none(),
        )
        .expect("the case opens");
}

/// The document binding [`CHECK`] to `check`, [`LINT`] to [`LINT_STUB`] and [`WRITE`] to
/// `write` with the writer `members` (each line indented four spaces), decoded and checked.
fn document(check_program: &str, write: &str, members: &str) -> BindingsDocument {
    let argv = |program: &str| serde_json::to_string(&[program]).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  brief: docs/brief\n  draft: docs/draft\nactions:\n\
         \x20 - action: {CHECK}\n    executor: {{kind: program, argv: {check}}}\n    evidence:\n\
         \x20     - {{kind: draft_check, subject: draft, producer: checker, from: exit, results: {{\"0\": valid, \"*\": invalid}}, upstream: [brief]}}\n\
         \x20 - action: {LINT}\n    executor: {{kind: program, argv: {lint}}}\n    evidence:\n\
         \x20     - {{kind: draft_lint, subject: draft, producer: linter, from: exit, results: {{\"0\": valid, \"*\": invalid}}, upstream: []}}\n\
         \x20 - action: {WRITE}\n    executor: {{kind: program, argv: {write}}}\n{members}    evidence:\n\
         \x20     - {{kind: draft_written, subject: draft, producer: writer, from: exit, results: {{\"*\": written}}, upstream: [brief]}}\n",
        check = argv(check_program),
        lint = argv(LINT_STUB),
        write = argv(write),
    );
    let document = decode(&text).unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    check(&document, REGISTRATION, protocol())
        .unwrap_or_else(|refusal| panic!("{refusal}\n{text}"));
    document
}

/// The fixture's document for a recording runner.
fn recorded_document(members: &str) -> BindingsDocument {
    document(CHECK_STUB, WRITE_STUB, members)
}

fn writer(document: &BindingsDocument) -> &BoundAction {
    document
        .actions
        .iter()
        .find(|bound| bound.action == WRITE)
        .expect("the document binds the writer")
}

/// The workspace and the store directory of one case.
struct Scratch {
    _dir: tempfile::TempDir,
    workspace: PathBuf,
    store: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("bound-retry-")
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

    fn blobs(&self) -> PathBuf {
        self.store.join("blobs")
    }

    fn blob_names(&self) -> Vec<String> {
        blob_names(&self.blobs())
    }

    /// The lines the writer stub appended to `runs`, one per start.
    fn runs(&self) -> usize {
        std::fs::read_to_string(self.workspace.join("runs"))
            .map(|runs| runs.lines().count())
            .unwrap_or(0)
    }
}

/// The name of each file in `blobs`, sorted; none when it does not exist.
fn blob_names(blobs: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(blobs)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// A runner that keeps every child it is given and starts nothing. A program it holds an answer
/// for exits with that code and prints that output; any other exits 0 printing `{}`. When
/// `watch` is set, it keeps the names in that directory at each call.
#[derive(Default)]
struct Recording {
    reason: Option<&'static str>,
    answers: BTreeMap<String, (i32, Vec<u8>)>,
    watch: Option<PathBuf>,
    children: Mutex<Vec<ChildCommand>>,
    seen: Mutex<Vec<Vec<String>>>,
}

impl Recording {
    fn new() -> Self {
        Self::default()
    }

    fn local() -> Self {
        Self {
            reason: Some(UNCONFINED_FLAG_REASON),
            ..Self::default()
        }
    }

    /// The same runner, answering `program` with `exit` and `stdout`.
    fn answering(mut self, program: &str, exit: i32, stdout: &[u8]) -> Self {
        self.answers
            .insert(program.to_owned(), (exit, stdout.to_vec()));
        self
    }

    /// The same runner, keeping the names in `blobs` at each call.
    fn watching(mut self, blobs: PathBuf) -> Self {
        self.watch = Some(blobs);
        self
    }

    fn children(&self) -> Vec<ChildCommand> {
        self.children.lock().unwrap().clone()
    }

    /// The children of `program`, in the order they were given.
    fn children_of(&self, program: &str) -> Vec<ChildCommand> {
        self.children()
            .into_iter()
            .filter(|child| child.program() == program)
            .collect()
    }

    /// The last child of the writer.
    fn writer_child(&self) -> ChildCommand {
        self.children_of(WRITE_STUB)
            .pop()
            .expect("the writer was given to the runner")
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
        if let Some(blobs) = &self.watch {
            self.seen.lock().unwrap().push(blob_names(blobs));
        }
        let (exit, stdout) = self
            .answers
            .get(child.program())
            .cloned()
            .unwrap_or_else(|| (0, b"{}".to_vec()));
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

    fn preflight_child(&self, _root: &Path) -> Result<(), ConfinementError> {
        Ok(())
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

fn port<'a>(
    governor: &'a CanonGovernor<MemoryCaseStore>,
    document: &'a BindingsDocument,
    scratch: &Scratch,
    runner: Arc<dyn TestRunner>,
) -> ProgramEffects<'a, MemoryCaseStore> {
    ProgramEffects::new(
        governor,
        CaseId(CASE.to_owned()),
        document,
        &scratch.workspace,
        &scratch.store,
        Some(runner),
    )
    .expect("the port is built")
}

static ISSUED: AtomicU64 = AtomicU64::new(1);

fn next() -> u64 {
    ISSUED.fetch_add(1, Ordering::Relaxed)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

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

fn args(text: &str) -> Value {
    json::parse(text).expect("the arguments parse")
}

/// One step of [`CASE`] in which the executor proposes `action` with `arguments`.
fn try_drive(
    governor: &CanonGovernor<MemoryCaseStore>,
    effects: &ProgramEffects<'_, MemoryCaseStore>,
    action: &str,
    arguments: &str,
) -> Result<LoopEnd, b10x_loom_commission::runtime::LoopError> {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(next())),
        agent_revision_id: AgentRevisionId(uuid(next())),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-p".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: action.to_owned(),
            arguments: ProposedActionArguments(args(arguments)),
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

fn drive(
    governor: &CanonGovernor<MemoryCaseStore>,
    effects: &ProgramEffects<'_, MemoryCaseStore>,
    action: &str,
    arguments: &str,
) -> LoopEnd {
    try_drive(governor, effects, action, arguments)
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

fn refusal(end: &LoopEnd) -> String {
    match only_effect(end) {
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

/// The payload of each observation the port kept, in the order kept.
fn runs_of(governor: &CanonGovernor<MemoryCaseStore>, action: &str) -> Vec<serde_json::Value> {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .map(|observation| plain(&observation.payload))
        .filter(|run| run["action"] == serde_json::json!(action))
        .collect()
}

fn port_observations(governor: &CanonGovernor<MemoryCaseStore>) -> usize {
    governor
        .try_observations()
        .unwrap()
        .into_iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .count()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The records `binding` is sent back by in [`CASE`].
fn retries(governor: &CanonGovernor<MemoryCaseStore>, binding: &BoundAction) -> Vec<RetryRecord> {
    retries_in(governor, CASE, binding)
}

fn retries_in(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &str,
    binding: &BoundAction,
) -> Vec<RetryRecord> {
    retry_records(governor, &CaseId(case.to_owned()), binding).expect("the records are read")
}

fn ids(records: &[RetryRecord]) -> Vec<String> {
    records
        .iter()
        .map(|record| record.evidence_id.clone())
        .collect()
}

/// The instant of every record [`hold`] submits.
const HELD_AT: &str = "2026-11-21T09:30:00Z";

/// Holds in `case` a record numbered `n` of `kind` about `draft` at `revision` with `result`, by
/// the producer `checker`, citing an observation of a source that is no bound program; answers
/// its evidence id.
fn hold(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &str,
    n: u64,
    kind: &str,
    revision: &str,
    result: &str,
) -> String {
    let observation = ObservationId(uuid(next()));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "test/held".to_owned(),
            subject: "draft".to_owned(),
            observed_at: Timestamp(HELD_AT.to_owned()),
            payload: Value::Null,
        }),
    )
    .unwrap();
    let id = Uuid(format!("00000000-0000-4000-9000-{n:012x}"));
    let facts = serde_json::json!({
        "format": "canon-evidence/1",
        "id": format!("held-{n}"),
        "kind": kind,
        "subject": "draft",
        "subject_revision": revision,
        "observed_at": HELD_AT,
        "result": result,
    });
    submit_evidence(
        governor,
        "checker",
        EvidenceData {
            evidence_id: EvidenceId(id.clone()),
            case_id: CaseId(case.to_owned()),
            kind: kind.to_owned(),
            subject_revision: 0,
            producer: "checker".to_owned(),
            observation_ids: vec![observation],
            facts: json::parse(&facts.to_string()).unwrap(),
            provenance: Value::Null,
        },
    )
    .expect("the record is held");
    id.0
}

/// The `RetryFeedback` the file at `path` holds, decoded member by member; a member it does not
/// declare, or one missing, fails.
fn feedback_at(path: &Path) -> RetryFeedback {
    let bytes = std::fs::read(path).expect("the feedback document is kept");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("one JSON document");
    let object = members(
        &value,
        &["format", "case", "action", "records", "logs"],
        "the document",
    );
    RetryFeedback {
        format: text(&object["format"]),
        case: text(&object["case"]),
        action: text(&object["action"]),
        records: object["records"]
            .as_array()
            .expect("records is a list")
            .iter()
            .map(|record| {
                let record = members(
                    record,
                    &[
                        "evidence_id",
                        "kind",
                        "subject",
                        "subject_revision",
                        "result",
                        "producer",
                        "observed_at",
                        "blob_sha256",
                    ],
                    "a record",
                );
                RetryRecord {
                    evidence_id: text(&record["evidence_id"]),
                    kind: text(&record["kind"]),
                    subject: text(&record["subject"]),
                    subject_revision: text(&record["subject_revision"]),
                    result: text(&record["result"]),
                    producer: text(&record["producer"]),
                    observed_at: loom::primitives::Timestamp(text(&record["observed_at"])),
                    blob_sha256: record["blob_sha256"].as_str().map(str::to_owned),
                }
            })
            .collect(),
        logs: object["logs"]
            .as_array()
            .expect("logs is a list")
            .iter()
            .map(|log| {
                let log = members(log, &["blob_sha256", "tail"], "a log");
                RetryLog {
                    blob_sha256: text(&log["blob_sha256"]),
                    tail: text(&log["tail"]),
                }
            })
            .collect(),
    }
}

fn members<'v>(
    value: &'v serde_json::Value,
    names: &[&str],
    what: &str,
) -> &'v serde_json::Map<String, serde_json::Value> {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{what} is no object: {value}"));
    let mut held: Vec<&str> = object.keys().map(String::as_str).collect();
    let mut declared = names.to_vec();
    held.sort_unstable();
    declared.sort_unstable();
    assert_eq!(held, declared, "{what}: {value}");
    object
}

fn text(value: &serde_json::Value) -> String {
    value
        .as_str()
        .unwrap_or_else(|| panic!("not a text: {value}"))
        .to_owned()
}

/// The value of `LOOM_RETRY_FEEDBACK` in `child`'s environment.
fn variable(child: &ChildCommand) -> Option<String> {
    child
        .environment()
        .set
        .get(RETRY_FEEDBACK_VARIABLE)
        .cloned()
}

/// A recording step of the writer after an `invalid` `draft_check` of [`DRAFT_REVISION`]: the
/// check is driven through the port, its runner answering exit 1.
struct Retried {
    scratch: Scratch,
    governor: CanonGovernor<MemoryCaseStore>,
    runner: Arc<Recording>,
    /// The retry records just before the writer's step.
    expected: Vec<RetryRecord>,
}

impl Retried {
    fn writer(&self) -> ChildCommand {
        self.runner.writer_child()
    }

    fn path(&self) -> PathBuf {
        PathBuf::from(variable(&self.writer()).expect("the writer is given the variable"))
    }
}

/// [`Retried`] with the writer `members` and `runner`, the writer driven with `arguments`.
fn retried_with(members: &str, runner: Recording, arguments: &str) -> Retried {
    let scratch = Scratch::new();
    let document = recorded_document(members);
    let governor = governor();
    let runner = Arc::new(runner.answering(CHECK_STUB, 1, b"the check failed\n"));
    {
        let effects = port(
            &governor,
            &document,
            &scratch,
            Arc::clone(&runner) as Arc<dyn TestRunner>,
        );
        assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    }
    let expected = retries(&governor, writer(&document));
    assert_eq!(expected.len(), 1, "{expected:#?}");
    {
        let effects = port(
            &governor,
            &document,
            &scratch,
            Arc::clone(&runner) as Arc<dyn TestRunner>,
        );
        assert_performed(&drive(&governor, &effects, WRITE, arguments));
    }
    Retried {
        scratch,
        governor,
        runner,
        expected,
    }
}

fn retried() -> Retried {
    retried_with(RETRY, Recording::new(), "{}")
}

/// A step of the writer under the `local` runner after the check stub `check_program` ran and
/// exited 1: the scratch, the governor, the retry records before the writer's step and the
/// feedback document the writer was given (it prints the variable).
fn retried_locally(
    check_program: &str,
    after_check: impl FnOnce(&Scratch, &CanonGovernor<MemoryCaseStore>),
) -> (
    Scratch,
    CanonGovernor<MemoryCaseStore>,
    Vec<RetryRecord>,
    RetryFeedback,
) {
    let scratch = Scratch::new();
    let document = document(check_program, &stubs().write, RETRY);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, flag_runner());
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    after_check(&scratch, &governor);
    let expected = retries(&governor, writer(&document));
    assert_eq!(expected.len(), 1, "{expected:#?}");
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    let runs = runs_of(&governor, WRITE);
    let blob = runs.last().unwrap()["blob_sha256"]
        .as_str()
        .expect("the writer's run kept a blob")
        .to_owned();
    let printed = std::fs::read_to_string(scratch.blobs().join(blob)).unwrap();
    assert!(!printed.is_empty(), "the writer was given no variable");
    let feedback = feedback_at(Path::new(&printed));
    (scratch, governor, expected, feedback)
}

// ---------------------------------------------------------------------------------------------
// retry_records

#[test]
fn retry_records_returns_a_listed_record_about_the_current_revision() {
    let governor = governor();
    let document = recorded_document(RETRY);
    let id = hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    assert_eq!(ids(&retries(&governor, writer(&document))), [id]);
}

#[test]
fn retry_records_skips_a_record_about_an_earlier_revision() {
    let governor = governor();
    let document = recorded_document(RETRY);
    hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    assert_eq!(retries(&governor, writer(&document)).len(), 1);
    governor
        .update_revision(&CaseId(CASE.to_owned()), "draft", "d2")
        .unwrap();
    assert!(retries(&governor, writer(&document)).is_empty());
}

#[test]
fn retry_records_skips_a_listed_kind_with_another_result() {
    let governor = governor();
    let document = recorded_document(RETRY);
    hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "valid");
    assert!(retries(&governor, writer(&document)).is_empty());
}

#[test]
fn retry_records_skips_a_kind_the_binding_does_not_list() {
    let governor = governor();
    let document = recorded_document(RETRY);
    hold(&governor, CASE, 1, "draft_lint", DRAFT_REVISION, "invalid");
    assert!(retries(&governor, writer(&document)).is_empty());
}

#[test]
fn retry_records_is_empty_while_the_subject_is_at_none() {
    let governor = governor_at("none");
    let document = recorded_document(RETRY);
    hold(&governor, CASE, 1, "draft_check", "none", "invalid");
    assert!(retries(&governor, writer(&document)).is_empty());
    // The same record about a written revision is one.
    governor
        .update_revision(&CaseId(CASE.to_owned()), "draft", DRAFT_REVISION)
        .unwrap();
    let id = hold(&governor, CASE, 2, "draft_check", DRAFT_REVISION, "invalid");
    assert_eq!(ids(&retries(&governor, writer(&document))), [id]);
}

#[test]
fn retry_records_is_empty_for_a_binding_without_retry_when() {
    let governor = governor();
    let document = recorded_document(NO_RETRY);
    assert!(writer(&document).retry_when.is_empty());
    hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    assert!(retries(&governor, writer(&document)).is_empty());
}

#[test]
fn a_record_held_by_another_case_is_not_a_retry_record() {
    let governor = governor();
    let document = recorded_document(RETRY);
    open(&governor, "case-2", REGISTRATION, DRAFT_REVISION);
    open(&governor, "case-3", OTHER_REGISTRATION, DRAFT_REVISION);
    let same = hold(
        &governor,
        "case-2",
        1,
        "draft_check",
        DRAFT_REVISION,
        "invalid",
    );
    hold(
        &governor,
        "case-3",
        2,
        "draft_check",
        DRAFT_REVISION,
        "invalid",
    );
    assert!(retries(&governor, writer(&document)).is_empty());
    // It is one for its own case.
    assert_eq!(
        ids(&retries_in(&governor, "case-2", writer(&document))),
        [same]
    );
}

#[test]
fn retry_records_keep_the_order_the_case_received_them() {
    let governor = governor();
    let document = recorded_document(RETRY);
    // Numbered so the first received sorts after the second.
    let first = hold(&governor, CASE, 9, "draft_check", DRAFT_REVISION, "invalid");
    hold(&governor, CASE, 5, "draft_check", DRAFT_REVISION, "valid");
    let second = hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    assert_eq!(ids(&retries(&governor, writer(&document))), [first, second]);
}

#[test]
fn a_retry_record_names_the_members_of_the_held_record() {
    let governor = governor();
    let document = recorded_document(RETRY);
    let id = hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    assert_eq!(
        retries(&governor, writer(&document)),
        [RetryRecord {
            evidence_id: id,
            kind: "draft_check".to_owned(),
            subject: "draft".to_owned(),
            subject_revision: DRAFT_REVISION.to_owned(),
            result: "invalid".to_owned(),
            producer: "checker".to_owned(),
            observed_at: loom::primitives::Timestamp(HELD_AT.to_owned()),
            blob_sha256: None,
        }]
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_retry_record_names_the_blob_of_the_run_its_record_cites() {
    let scratch = Scratch::new();
    let document = document(&stubs().check_invalid, WRITE_STUB, RETRY);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, flag_runner());
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    let runs = runs_of(&governor, CHECK);
    assert_eq!(runs.len(), 1, "{runs:#?}");
    let blob = runs[0]["blob_sha256"].as_str().unwrap().to_owned();
    assert_eq!(
        std::fs::read(scratch.blobs().join(&blob)).unwrap(),
        b"invalid\n"
    );
    let records = retries(&governor, writer(&document));
    assert_eq!(records.len(), 1, "{records:#?}");
    assert_eq!(records[0].blob_sha256.as_deref(), Some(blob.as_str()));
    assert_eq!(records[0].producer, "checker");
}

// ---------------------------------------------------------------------------------------------
// The feedback document

#[test]
fn a_retried_writer_is_given_the_path_of_its_feedback_document() {
    let step = retried();
    let path = step.path();
    assert!(path.is_absolute(), "{}", path.display());
    assert_eq!(std::fs::canonicalize(&path).unwrap(), path);
    assert_eq!(
        path.parent().unwrap(),
        std::fs::canonicalize(step.scratch.blobs()).unwrap()
    );
    assert!(path.is_file());
}

#[test]
fn the_feedback_document_is_named_by_its_sha256() {
    let step = retried();
    let path = step.path();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        sha256(&bytes).as_str()
    );
}

#[test]
fn the_feedback_document_holds_the_case_the_action_and_the_retry_records() {
    let step = retried();
    let feedback = feedback_at(&step.path());
    assert_eq!(feedback.format, RETRY_FEEDBACK_FORMAT);
    assert_eq!(RETRY_FEEDBACK_FORMAT, "loom.retry-feedback/1");
    assert_eq!(feedback.case, CASE);
    assert_eq!(feedback.action, WRITE);
    assert!(!step.expected.is_empty());
    assert_eq!(feedback.records, step.expected);
    // The check's run kept a blob, so the record names it and the document holds its log.
    let blob = step.expected[0].blob_sha256.clone().expect("a cited blob");
    assert_eq!(
        feedback.logs,
        [RetryLog {
            blob_sha256: blob,
            tail: "the check failed\n".to_owned(),
        }]
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn the_feedback_document_holds_the_last_60_lines_of_each_cited_blob() {
    let (_scratch, _governor, expected, feedback) =
        retried_locally(&stubs().check_hundred_lines, |_, _| {});
    assert_eq!(RETRY_LOG_LINES, 60);
    assert_eq!(feedback.logs.len(), 1, "{:#?}", feedback.logs);
    let log = &feedback.logs[0];
    assert_eq!(Some(&log.blob_sha256), expected[0].blob_sha256.as_ref());
    let lines: Vec<&str> = log.tail.lines().collect();
    let wanted: Vec<String> = (41..=100).map(|n| format!("line {n}")).collect();
    assert_eq!(lines, wanted);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_log_tail_holds_at_most_65536_bytes() {
    let (_scratch, _governor, _expected, feedback) =
        retried_locally(&stubs().check_long_line, |_, _| {});
    assert_eq!(RETRY_LOG_BYTES, 65536);
    assert_eq!(feedback.logs.len(), 1, "{:#?}", feedback.logs);
    let tail = &feedback.logs[0].tail;
    assert_eq!(tail.len(), 65536);
    assert!(tail.ends_with("aaa\n"), "{:?}", &tail[tail.len() - 8..]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_blob_whose_content_does_not_hash_to_its_name_gives_no_log() {
    let mut replaced = String::new();
    let (_scratch, _governor, expected, feedback) =
        retried_locally(&stubs().check_invalid, |scratch, governor| {
            let runs = runs_of(governor, CHECK);
            replaced = runs[0]["blob_sha256"].as_str().unwrap().to_owned();
            std::fs::write(scratch.blobs().join(&replaced), b"replaced after the run\n").unwrap();
        });
    assert!(feedback.logs.is_empty(), "{:#?}", feedback.logs);
    assert_eq!(feedback.records, expected);
    assert_eq!(
        feedback.records[0].blob_sha256.as_deref(),
        Some(replaced.as_str())
    );
}

#[test]
fn the_feedback_document_is_mounted_read_only_under_a_non_local_runner() {
    let step = retried_with(RETRY, Recording::new(), "{}");
    let path = step.path();
    assert!(
        step.writer().host_paths().contains(&path),
        "{:?}",
        step.writer().host_paths()
    );
}

#[test]
fn the_feedback_document_is_not_mounted_under_a_local_runner() {
    let step = retried_with(RETRY, Recording::local(), "{}");
    let path = step.path();
    assert!(
        !step.writer().host_paths().contains(&path),
        "{:?}",
        step.writer().host_paths()
    );
}

/// A recording step of the writer after a `valid` `draft_check`, with `arguments`.
fn not_retried(arguments: &str) -> (Scratch, CanonGovernor<MemoryCaseStore>, Arc<Recording>) {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().watching(scratch.blobs()));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    assert!(retries(&governor, writer(&document)).is_empty());
    assert_performed(&drive(&governor, &effects, WRITE, arguments));
    (scratch, governor, runner)
}

#[test]
fn a_writer_with_no_retry_records_gets_no_feedback_variable() {
    let (_scratch, _governor, runner) = not_retried("{}");
    assert_eq!(variable(&runner.writer_child()), None);
}

#[test]
fn a_writer_with_no_retry_records_writes_no_feedback_document() {
    let (scratch, _governor, runner) = not_retried("{}");
    let seen = runner.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "{seen:?}");
    // After the check's step and before the writer started, the store gained no file.
    let after_check = seen[0].len() + 1;
    assert_eq!(seen[1].len(), after_check, "{seen:?}");
    let _ = scratch;
}

#[test]
fn a_retried_writers_environment_gains_only_the_feedback_variable() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(CHECK_STUB, 1, b"failed\n"));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    let plain_step = runner.writer_child().environment().clone();
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    let retried_step = runner.writer_child().environment().clone();
    assert_eq!(retried_step.allow, plain_step.allow);
    let mut gained = retried_step.set.clone();
    assert!(gained.remove(RETRY_FEEDBACK_VARIABLE).is_some());
    assert_eq!(gained, plain_step.set);
    assert_eq!(retried_step.set.len(), plain_step.set.len() + 1);
}

#[test]
fn an_arguments_from_program_is_given_no_feedback() {
    let members = format!(
        "{RETRY}    arguments_from: {}\n",
        serde_json::to_string(&[ARGUMENTS_STUB]).unwrap()
    );
    let step = retried_with(&members, Recording::new(), "{}");
    let from = step.runner.children_of(ARGUMENTS_STUB);
    assert_eq!(from.len(), 1);
    assert_eq!(variable(&from[0]), None);
    assert!(!from[0].host_paths().contains(&step.path()));
    assert!(variable(&step.writer()).is_some());
}

#[test]
fn the_run_observation_names_the_records_it_was_given() {
    let step = retried();
    let runs = runs_of(&step.governor, WRITE);
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0]["retry_of"], serde_json::json!(ids(&step.expected)));
}

#[test]
fn a_run_with_no_retry_records_has_an_empty_retry_of() {
    let (_scratch, governor, _runner) = not_retried("{}");
    let runs = runs_of(&governor, WRITE);
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0]["retry_of"], serde_json::json!([]));
    // The check's run was given none either.
    assert_eq!(
        runs_of(&governor, CHECK)[0]["retry_of"],
        serde_json::json!([])
    );
}

// ---------------------------------------------------------------------------------------------
// Security

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_retry_at_max_runs_is_refused_runs_spent_and_starts_nothing() {
    let scratch = Scratch::new();
    let document = document(&stubs().check_invalid, &stubs().write, RETRY);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, flag_runner());
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    for _ in 0..3 {
        assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    }
    assert_eq!(scratch.runs(), 3);
    let marker = scratch.workspace.join("runs");
    std::fs::remove_file(&marker).unwrap();
    assert!(!retries(&governor, writer(&document)).is_empty());
    let reason = refusal(&drive(&governor, &effects, WRITE, "{}"));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
    assert!(!marker.exists(), "the writer started at its max_runs");
}

#[test]
fn a_step_refused_runs_spent_writes_no_feedback_document() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(CHECK_STUB, 1, b"failed\n"));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    // Three runs given no records, then the check sends the writer back.
    for _ in 0..3 {
        assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    }
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    assert!(!retries(&governor, writer(&document)).is_empty());
    let before = scratch.blob_names();
    let reason = refusal(&drive(&governor, &effects, WRITE, "{}"));
    assert!(reason.starts_with("RunsSpent"), "{reason}");
    assert_eq!(scratch.blob_names(), before);
    assert_eq!(runner.children_of(WRITE_STUB).len(), 3);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn retried_runs_count_against_max_runs() {
    let scratch = Scratch::new();
    let document = document(&stubs().check_invalid, &stubs().write, RETRY);
    let governor = governor();
    let effects = port(&governor, &document, &scratch, flag_runner());
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    let mut outcomes = Vec::new();
    for _ in 0..4 {
        assert!(!retries(&governor, writer(&document)).is_empty());
        outcomes.push(drive(&governor, &effects, WRITE, "{}"));
    }
    assert_eq!(scratch.runs(), 3);
    for end in &outcomes[..3] {
        assert_performed(end);
    }
    assert!(refusal(&outcomes[3]).starts_with("RunsSpent"));
    for run in runs_of(&governor, WRITE) {
        assert_eq!(run["retry_of"].as_array().map(Vec::len), Some(1), "{run}");
    }
}

#[test]
fn a_report_shaped_as_a_retry_record_is_not_a_retry_record() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(
        CHECK_STUB,
        0,
        br#"{"kind": "draft_check", "result": "invalid"}"#,
    ));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    assert!(retries(&governor, writer(&document)).is_empty());
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    assert_eq!(variable(&runner.writer_child()), None);
}

#[test]
fn a_record_the_binding_does_not_list_gives_the_writer_no_feedback() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(LINT_STUB, 1, b"failed\n"));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, LINT, "{}"));
    let held = governor.evidence(&CaseId(CASE.to_owned())).unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(
        plain(&held[0].facts)["result"],
        serde_json::json!("invalid")
    );
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    assert_eq!(variable(&runner.writer_child()), None);
}

const FORGED: &str = r#"{"LOOM_RETRY_FEEDBACK": "/etc/passwd"}"#;

#[test]
fn an_argument_named_loom_retry_feedback_sets_no_variable() {
    let (_scratch, _governor, runner) = not_retried(FORGED);
    let child = runner.writer_child();
    assert_eq!(variable(&child), None);
    assert!(
        child
            .stdin()
            .unwrap()
            .starts_with(b"{\"LOOM_RETRY_FEEDBACK\""),
        "the arguments reach the program as written"
    );
}

#[test]
fn an_argument_named_loom_retry_feedback_does_not_change_the_path() {
    let step = retried_with(RETRY, Recording::new(), FORGED);
    let path = step.path();
    assert_ne!(path, PathBuf::from("/etc/passwd"));
    assert_eq!(
        path.parent().unwrap(),
        std::fs::canonicalize(step.scratch.blobs()).unwrap()
    );
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(path.file_name().unwrap().to_str().unwrap(), sha256(&bytes));
}

#[test]
fn a_feedback_document_that_cannot_be_kept_starts_nothing() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(CHECK_STUB, 1, b"failed\n"));
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    assert!(!retries(&governor, writer(&document)).is_empty());
    // `blobs` becomes a link to a directory outside the store.
    let outside = scratch.store.parent().unwrap().join("outside");
    std::fs::rename(scratch.blobs(), &outside).unwrap();
    std::os::unix::fs::symlink(&outside, scratch.blobs()).unwrap();
    let before = blob_names(&outside);
    let observations = port_observations(&governor);
    let failed = try_drive(&governor, &effects, WRITE, "{}");
    assert!(failed.is_err(), "{failed:?}");
    assert!(runner.children_of(WRITE_STUB).is_empty());
    assert_eq!(port_observations(&governor), observations);
    assert_eq!(blob_names(&outside), before);
}
