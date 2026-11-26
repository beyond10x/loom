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
    LAUNCH_FAILURE_EXITS, PROGRAM_SOURCE, ProgramEffects, RETRY_FEEDBACK_FORMAT,
    RETRY_FEEDBACK_VARIABLE, RETRY_LOG_BYTES, RETRY_LOG_LINES, check, decode, retry_records,
    run_handoff_check, settle_spent,
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
use loom::bindings::{
    BindingsDocument, BoundAction, RetryFeedback, RetryLog, RetryRecord, WorkspaceWrites,
};
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
  draft_spent: {}
  conformance_run: {}
  code_review: {}
  merge_approval: {}
actions:
  draft.check:
    effect: read
    may_produce:
      - evidence: draft_check
      - evidence: conformance_run
      - evidence: code_review
  draft.lint:
    effect: read
    may_produce:
      - evidence: draft_lint
      - evidence: merge_approval
  draft.write:
    effect: write
    may_produce:
      - evidence: draft_written
      - evidence: draft_check
      - evidence: draft_spent
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
    /// [`Stubs::write`], after it writes a second draft to `docs/draft`.
    write_draft: String,
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
            write_draft: script(
                "write-draft.sh",
                "echo 'the second draft' > docs/draft\necho run >> runs\nprintf '%s' \"${LOOM_RETRY_FEEDBACK-}\"\nexit 0\n",
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
    document_with(check_program, write, members, "")
}

/// [`document`], the writer's evidence list ending with the lines `entries` (each indented six
/// spaces).
fn document_with(
    check_program: &str,
    write: &str,
    members: &str,
    entries: &str,
) -> BindingsDocument {
    let argv = |program: &str| serde_json::to_string(&[program]).unwrap();
    let text = format!(
        "format: loom.bindings/1\nprotocol: {REGISTRATION}\nartifacts:\n  brief: docs/brief\n  draft: docs/draft\nactions:\n\
         \x20 - action: {CHECK}\n    executor: {{kind: program, argv: {check}}}\n    evidence:\n\
         \x20     - {{kind: draft_check, subject: draft, producer: checker, from: exit, results: {{\"0\": valid, \"*\": invalid}}, upstream: [brief]}}\n\
         \x20 - action: {LINT}\n    executor: {{kind: program, argv: {lint}}}\n    evidence:\n\
         \x20     - {{kind: draft_lint, subject: draft, producer: linter, from: exit, results: {{\"0\": valid, \"*\": invalid}}, upstream: []}}\n\
         \x20 - action: {WRITE}\n    executor: {{kind: program, argv: {write}}}\n{members}    evidence:\n\
         \x20     - {{kind: draft_written, subject: draft, producer: writer, from: exit, results: {{\"*\": written}}, upstream: [brief]}}\n{entries}",
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
        // The placed artifacts hold something, so a step moves them to a revision that is not
        // `none` (story bound-artifact-revisions).
        std::fs::create_dir(workspace.join("docs")).unwrap();
        std::fs::write(workspace.join("docs/brief"), b"the brief\n").unwrap();
        std::fs::write(workspace.join("docs/draft"), b"the first draft\n").unwrap();
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
    /// When set, every child ends with this exit status (`None`: none) and timeout flag.
    ending: Option<(Option<i32>, bool)>,
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

    /// The same runner, every child ending with `exit` (`None`: no exit status) and `timed_out`.
    fn ending(mut self, exit: Option<i32>, timed_out: bool) -> Self {
        self.ending = Some((exit, timed_out));
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
        let (exit_code, timed_out) = self.ending.unwrap_or((Some(exit), false));
        Ok(TestExecution {
            exit_code,
            timed_out,
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
    hold_citing(
        governor,
        case,
        n,
        (kind, revision, result),
        "test/held",
        Value::Null,
        "checker",
    )
}

/// [`hold`], the record by `producer`, citing an observation of `source` whose payload is
/// `payload`, received after every observation before it.
fn hold_citing(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &str,
    n: u64,
    record: (&str, &str, &str),
    source: &str,
    payload: Value,
    producer: &str,
) -> String {
    let observation = ObservationId(uuid(next()));
    ObservationPort::observe(
        governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: source.to_owned(),
            subject: "draft".to_owned(),
            observed_at: Timestamp(HELD_AT.to_owned()),
            payload,
        }),
    )
    .unwrap();
    hold_on(governor, case, n, record, vec![observation], producer)
}

/// Holds in `case` a record numbered `n` of `kind` about `draft` at `revision` with `result`, by
/// `producer`, citing the observations `cited`, at the instant of the first; answers its evidence
/// id.
fn hold_on(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &str,
    n: u64,
    (kind, revision, result): (&str, &str, &str),
    cited: Vec<ObservationId>,
    producer: &str,
) -> String {
    let observed_at = governor
        .try_observations()
        .unwrap()
        .into_iter()
        .find(|observation| observation.observation_id == cited[0])
        .expect("the cited observation is held")
        .observed_at
        .0
        .clone();
    let id = Uuid(format!("00000000-0000-4000-9000-{n:012x}"));
    let facts = serde_json::json!({
        "format": "canon-evidence/1",
        "id": format!("held-{n}"),
        "kind": kind,
        "subject": "draft",
        "subject_revision": revision,
        "observed_at": observed_at,
        "result": result,
    });
    submit_evidence(
        governor,
        producer,
        EvidenceData {
            evidence_id: EvidenceId(id.clone()),
            case_id: CaseId(case.to_owned()),
            kind: kind.to_owned(),
            subject_revision: 0,
            producer: producer.to_owned(),
            observation_ids: cited,
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
                        "action",
                        "exit",
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
                    action: record["action"].as_str().map(str::to_owned),
                    exit: record["exit"].as_i64(),
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
            action: None,
            exit: None,
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

#[test]
fn a_retry_record_names_the_action_and_the_exit_of_the_run_its_record_cites() {
    let step = retried();
    assert_eq!(step.expected[0].action.as_deref(), Some(CHECK));
    assert_eq!(step.expected[0].exit, Some(1));
    // The document carries both, as the records `retry_records` returns.
    let feedback = feedback_at(&step.path());
    assert_eq!(feedback.records[0].action.as_deref(), Some(CHECK));
    assert_eq!(feedback.records[0].exit, Some(1));
}

#[test]
fn a_record_citing_no_bound_program_run_of_its_case_names_no_action_and_no_exit() {
    let governor = governor();
    let document = recorded_document(RETRY);
    open(&governor, "case-2", REGISTRATION, DRAFT_REVISION);
    let forged = |case: &str| {
        args(&format!(
            r#"{{"case": "{case}", "action": "{WRITE}", "exit": 0, "blob_sha256": "{}"}}"#,
            "a".repeat(64)
        ))
    };
    let listed = ("draft_check", DRAFT_REVISION, "invalid");
    // An observation of another source shaped as a run, and a run of another case.
    hold_citing(
        &governor,
        CASE,
        1,
        listed,
        "test/held",
        forged(CASE),
        "checker",
    );
    hold_citing(
        &governor,
        CASE,
        2,
        listed,
        PROGRAM_SOURCE,
        forged("case-2"),
        "checker",
    );
    let records = retries(&governor, writer(&document));
    assert_eq!(records.len(), 2, "{records:#?}");
    for record in records {
        assert_eq!(
            (record.action, record.exit, record.blob_sha256),
            (None, None, None)
        );
    }
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

// ---------------------------------------------------------------------------------------------
// A retry meets the revisions a step writes (story bound-artifact-revisions)

/// What the writer's blob holds: the value of `LOOM_RETRY_FEEDBACK` it was given, empty for none.
fn printed_by_the_writer(
    governor: &CanonGovernor<MemoryCaseStore>,
    scratch: &Scratch,
    run: usize,
) -> String {
    let runs = runs_of(governor, WRITE);
    let blob = runs[run]["blob_sha256"].as_str().unwrap().to_owned();
    std::fs::read_to_string(scratch.blobs().join(blob)).unwrap()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "runs Linux host processes")]
fn a_retried_writer_that_moves_the_draft_records_the_moved_revision_and_is_not_sent_back_again() {
    let scratch = Scratch::new();
    let document = document(&stubs().check_invalid, &stubs().write_draft, RETRY);
    let governor = governor();
    let case = CaseId(CASE.to_owned());
    let effects = port(&governor, &document, &scratch, flag_runner());
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    // The check's step moved `draft` to the digest of the first draft; its record is about it.
    let first = governor.revisions(&case).unwrap()["draft"].clone();
    assert_ne!(first, DRAFT_REVISION);
    let sent_back = retries(&governor, writer(&document));
    assert_eq!(sent_back.len(), 1, "{sent_back:#?}");
    assert_eq!(sent_back[0].subject_revision, first);
    // The retried writer writes a second draft: its step moves `draft`, and its record is about
    // the moved revision, which the step's one write accepts (no `StaleRecord`).
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    assert!(!printed_by_the_writer(&governor, &scratch, 0).is_empty());
    assert_eq!(
        runs_of(&governor, WRITE)[0]["retry_of"],
        serde_json::json!(ids(&sent_back))
    );
    let second = governor.revisions(&case).unwrap()["draft"].clone();
    assert_ne!(second, first);
    let held = governor.evidence(&case).unwrap();
    let written = held.last().unwrap();
    assert_eq!(written.kind, "draft_written");
    assert_eq!(
        plain(&written.facts)["subject_revision"],
        serde_json::json!(second)
    );
    // The check's record is about the first draft: nothing sends the writer back now.
    assert!(retries(&governor, writer(&document)).is_empty());
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    assert_eq!(printed_by_the_writer(&governor, &scratch, 1), "");
    assert_eq!(
        runs_of(&governor, WRITE)[1]["retry_of"],
        serde_json::json!([])
    );
}

// ---------------------------------------------------------------------------------------------
// A cancelled step (`story:cancel-a-running-bound-program`)

/// A recording runner whose first child of `program` its cancel ended: the runner answers it as
/// one whose kill a cancel handle sent.
struct CancelsFirst {
    inner: Arc<Recording>,
    program: &'static str,
    cancelled: std::sync::atomic::AtomicBool,
}

impl TestRunner for CancelsFirst {
    fn run(&self, command: &TestCommand, root: &Path) -> Result<TestExecution, ConfinementError> {
        self.inner.run(command, root)
    }

    fn backend(&self) -> Backend {
        self.inner.backend()
    }

    fn local_reason(&self) -> Option<&str> {
        self.inner.local_reason()
    }

    fn run_child(&self, child: &ChildCommand) -> Result<TestExecution, ConfinementError> {
        let answer = self.inner.run_child(child);
        if child.program() == self.program && !self.cancelled.swap(true, Ordering::SeqCst) {
            return Err(ConfinementError {
                reason: ConfinementRefusal::Cancelled,
                detail: "the child was killed by its cancel after 1.0 s".to_owned(),
            });
        }
        answer
    }

    fn preflight_child(&self, root: &Path) -> Result<(), ConfinementError> {
        self.inner.preflight_child(root)
    }
}

#[test]
fn a_cancelled_step_is_not_retried() {
    let scratch = Scratch::new();
    let document = recorded_document(RETRY);
    let governor = governor();
    let recording = Arc::new(Recording::new().answering(CHECK_STUB, 1, b"the check failed\n"));
    let runner = Arc::new(CancelsFirst {
        inner: Arc::clone(&recording),
        program: WRITE_STUB,
        cancelled: std::sync::atomic::AtomicBool::new(false),
    });
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    // One recorded check sends the writer back.
    assert_performed(&drive(&governor, &effects, CHECK, "{}"));
    let check = retries(&governor, writer(&document));
    assert_eq!(check.len(), 1, "{check:#?}");
    // The writer's step is cancelled: no record, its run observed `cancelled`.
    let reason = refusal(&drive(&governor, &effects, WRITE, "{}"));
    assert!(
        reason.starts_with("Cancelled: action draft.write: "),
        "{reason}"
    );
    let cancelled = runs_of(&governor, WRITE);
    assert_eq!(cancelled.len(), 1, "{cancelled:#?}");
    assert_eq!(cancelled[0]["cancelled"], serde_json::json!(true));
    assert_eq!(retries(&governor, writer(&document)), check);
    // The writer's second run is given the check's record alone.
    assert_performed(&drive(&governor, &effects, WRITE, "{}"));
    let second = recording.writer_child();
    let feedback = feedback_at(Path::new(
        &variable(&second).expect("the second run is given the variable"),
    ));
    assert_eq!(feedback.records, check);
    let blob = check[0].blob_sha256.clone().expect("a cited blob");
    assert_eq!(
        feedback.logs,
        [RetryLog {
            blob_sha256: blob,
            tail: "the check failed\n".to_owned(),
        }]
    );
    let runs = runs_of(&governor, WRITE);
    assert_eq!(runs.len(), 2, "{runs:#?}");
    assert_eq!(runs[1]["retry_of"], serde_json::json!(ids(&check)));
    assert_eq!(runs[1]["cancelled"], serde_json::json!(false));
}

// ---------------------------------------------------------------------------------------------
// handoff_when (story handoff-when-a-program-decides)

/// The `handoff_when` check of the writer, a program the recording runner answers.
const HANDOFF_STUB: &str = "/opt/stub/draft-handoff-when";

/// A writer sent back by an `invalid` `draft_check`, whose check exits `exit`: what
/// `run_handoff_check` answered and the one child the runner was given.
fn handoff_checked(exit: i32) -> (b10x_loom_bindings::HandoffCheck, ChildCommand, Scratch) {
    handoff_checked_by(Recording::new(), exit)
}

/// [`handoff_checked`] through `runner`.
fn handoff_checked_by(
    runner: Recording,
    exit: i32,
) -> (b10x_loom_bindings::HandoffCheck, ChildCommand, Scratch) {
    let scratch = Scratch::new();
    let members =
        format!("{RETRY}    handoff: Answer the finding.\n    handoff_when: [{HANDOFF_STUB}]\n");
    let document = recorded_document(&members);
    let governor = governor();
    hold(&governor, CASE, 1, "draft_check", DRAFT_REVISION, "invalid");
    // The writer itself would be given its feedback now.
    assert_eq!(retries(&governor, writer(&document)).len(), 1);
    let runner = Arc::new(runner.answering(HANDOFF_STUB, exit, b"said\n"));
    let check = run_handoff_check(
        &governor,
        &CaseId(CASE.to_owned()),
        &document,
        writer(&document),
        &scratch.workspace,
        &scratch.store,
        &(Arc::clone(&runner) as Arc<dyn TestRunner>),
        None,
    )
    .expect("the writer carries a check");
    let children = runner.children();
    assert_eq!(children.len(), 1, "the check starts once");
    (check, children[0].clone(), scratch)
}

/// Exit 126 and 127, the statuses a launcher gives a program it cannot find or execute (Substrate
/// starts every program through `/usr/bin/env`), are no answer of the check: a refusal, its output
/// still kept. Every other exit is the answer. The child gets no more than the writer: the whole
/// workspace read-only, no writable directory, no aperture, no retry feedback although the writer
/// is retried, empty standard input.
#[test]
fn a_handoff_check_launch_failure_is_a_refusal_and_its_child_no_more_than_the_writers() {
    assert_eq!(LAUNCH_FAILURE_EXITS, [126, 127]);
    for code in LAUNCH_FAILURE_EXITS {
        let (check, _, scratch) = handoff_checked(i32::try_from(code).unwrap());
        assert_eq!(check.exit, None, "{code}: {check:?}");
        let refusal = check.refusal.as_deref().expect("a refusal");
        assert!(refusal.contains(&format!("exit {code}")), "{refusal}");
        assert!(!check.hands_off(), "{code}");
        let kept = check.blob_digest.as_deref().expect("the output is kept");
        assert_eq!(
            std::fs::read(scratch.blobs().join(kept)).unwrap(),
            b"said\n"
        );
    }
    for code in [0, 1, 3, 125, 128] {
        let (check, child, _) = handoff_checked(code);
        assert_eq!(
            (check.exit, check.refusal.as_deref()),
            (Some(i64::from(code)), None),
            "{code}"
        );
        assert_eq!(check.hands_off(), code == 0, "{code}");
        assert_eq!(child.program(), HANDOFF_STUB);
        assert!(child.read_only_workspace(), "{child:?}");
        assert!(child.writable().is_empty(), "{child:?}");
        assert!(child.egress().is_empty(), "{child:?}");
        assert_eq!(variable(&child), None, "{child:?}");
        assert!(child.stdin().unwrap_or_default().is_empty(), "{child:?}");
    }
}

/// Under the document's `workspace: read_write` (`story:bound-program-writes-its-workspace`) the
/// check reaches what its writer does then: the whole workspace writable, no named path beside,
/// never read-only.
#[test]
fn a_handoff_check_of_a_read_write_document_writes_the_workspace_as_its_writer_does() {
    let scratch = Scratch::new();
    let members =
        format!("{RETRY}    handoff: Answer the finding.\n    handoff_when: [{HANDOFF_STUB}]\n");
    let mut document = recorded_document(&members);
    document.workspace = Some(WorkspaceWrites::ReadWrite);
    let governor = governor();
    let runner = Arc::new(Recording::new().answering(HANDOFF_STUB, 0, b"said\n"));
    let check = run_handoff_check(
        &governor,
        &CaseId(CASE.to_owned()),
        &document,
        writer(&document),
        &scratch.workspace,
        &scratch.store,
        &(Arc::clone(&runner) as Arc<dyn TestRunner>),
        None,
    )
    .expect("the writer carries a check");
    assert!(check.hands_off(), "{check:?}");
    let children = runner.children();
    assert_eq!(children.len(), 1, "the check starts once");
    let child = &children[0];
    assert!(child.read_write_workspace(), "{child:?}");
    assert!(!child.read_only_workspace(), "{child:?}");
    assert!(child.writable().is_empty(), "{child:?}");
}

/// A check killed at its timeout, whatever status it reported, and a check a signal ended (no
/// exit status) give no answer either: each a refusal naming why, its output kept.
#[test]
fn a_handoff_check_ended_by_its_timeout_or_a_signal_is_a_refusal() {
    for (exit, timed_out, why) in [
        (Some(0), true, "timeout"),
        (Some(1), true, "timeout"),
        (None, true, "timeout"),
        (None, false, "signal"),
    ] {
        let (check, _, scratch) = handoff_checked_by(Recording::new().ending(exit, timed_out), 0);
        let said = format!("{exit:?} {timed_out}: {check:?}");
        assert_eq!(check.exit, None, "{said}");
        assert!(!check.hands_off(), "{said}");
        assert!(check.refusal.as_deref().unwrap().contains(why), "{said}");
        let kept = check.blob_digest.as_deref().expect("the output is kept");
        assert_eq!(
            std::fs::read(scratch.blobs().join(kept)).unwrap(),
            b"said\n"
        );
    }
}

/// Substrate reports a child a signal ended as exit 128 plus the signal (SIGKILL: 137, as an
/// out-of-memory or cgroup kill ends it; `a_confined_child_a_signal_ended_reports_128_and_the_signal`
/// in `crates/loom-intake-slice/tests/confined_child.rs`). Such a check gave no answer: each
/// status from 129 to 192 (signals 1 to 64) is a refusal naming the signal, its output kept, never
/// a skip. 128 and 193 are answers.
#[test]
fn a_handoff_check_a_signal_ended_as_substrate_reports_it_is_a_refusal() {
    for code in [129, 130, 137, 143, 192] {
        let (check, _, scratch) = handoff_checked(code);
        let said = format!("{code}: {check:?}");
        assert_eq!(check.exit, None, "{said}");
        assert!(!check.hands_off(), "{said}");
        let refusal = check.refusal.as_deref().expect("a refusal");
        assert!(
            refusal.contains(&format!("exit {code}")) && refusal.contains("signal"),
            "{said}"
        );
        let kept = check.blob_digest.as_deref().expect("the output is kept");
        assert_eq!(
            std::fs::read(scratch.blobs().join(kept)).unwrap(),
            b"said\n"
        );
    }
    for code in [128, 193, 255] {
        let (check, _, _) = handoff_checked(code);
        assert_eq!(
            (check.exit, check.refusal.as_deref()),
            (Some(i64::from(code)), None),
            "{code}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// A fresh allowance after a rejection (story rework-allowance-after-a-rejection)

/// The writer's members: sent back by an `invalid` `draft_check`, with `budget` (flow YAML).
fn reworked(budget: &str) -> String {
    format!("    retry_when: {{draft_check: [invalid]}}\n    budget: {budget}\n")
}

/// The writer's `from: runs` entry: `draft_spent: spent` on the run that spends its allowance.
const SPENT_ENTRY: &str = "      - {kind: draft_spent, subject: draft, producer: loom, from: runs, results: {spent: spent}}\n";

/// One case of the fixture driven by a recording runner, its writer bound with the given members.
struct Rework {
    scratch: Scratch,
    governor: CanonGovernor<MemoryCaseStore>,
    document: BindingsDocument,
    runner: Arc<Recording>,
}

impl Rework {
    fn new(members: &str) -> Self {
        Self::with(members, "")
    }

    /// The writer's evidence also ends with `entries`.
    fn with(members: &str, entries: &str) -> Self {
        Self {
            scratch: Scratch::new(),
            governor: governor(),
            document: document_with(CHECK_STUB, WRITE_STUB, members, entries),
            runner: Arc::new(Recording::new()),
        }
    }

    /// The same case driven under another document from now on.
    fn rebound(&mut self, members: &str) {
        self.document = document_with(CHECK_STUB, WRITE_STUB, members, "");
    }

    fn drive(&self, action: &str) -> LoopEnd {
        let effects = port(
            &self.governor,
            &self.document,
            &self.scratch,
            Arc::clone(&self.runner) as Arc<dyn TestRunner>,
        );
        drive(&self.governor, &effects, action, "{}")
    }

    /// A step of the writer that performs.
    fn run(&self) {
        assert_performed(&self.drive(WRITE));
    }

    /// A step of the writer that is refused: the refusal's text.
    fn refused(&self) -> String {
        refusal(&self.drive(WRITE))
    }

    /// A step of the writer under a second runner that answers it 137, as Substrate reports a
    /// child a signal ended: refused `ProgramSignalled`, counted, nothing recorded.
    fn run_recording_nothing(&self) {
        let runner: Arc<dyn TestRunner> =
            Arc::new(Recording::new().answering(WRITE_STUB, 137, b""));
        let effects = port(&self.governor, &self.document, &self.scratch, runner);
        let reason = refusal(&drive(&self.governor, &effects, WRITE, "{}"));
        assert!(reason.starts_with("ProgramSignalled"), "{reason}");
    }

    /// `settle_spent` on the writer: the number of records it wrote.
    fn settle(&self) -> usize {
        settle_spent(
            &self.governor,
            &CaseId(CASE.to_owned()),
            writer(&self.document),
        )
        .expect("the record is written")
        .len()
    }

    /// The revision the case holds for `draft`.
    fn draft(&self) -> String {
        self.governor.revisions(&CaseId(CASE.to_owned())).unwrap()["draft"].clone()
    }

    /// A record of `kind` with `result` about `draft` at `revision`, by `producer`, citing an
    /// observation that names this case (`placed`) or another case.
    fn hold(
        &self,
        (kind, revision, result): (&str, &str, &str),
        producer: &str,
        placed: bool,
    ) -> String {
        let named = if placed { CASE } else { "case-2" };
        hold_citing(
            &self.governor,
            CASE,
            next(),
            (kind, revision, result),
            "test/held",
            args(&format!(r#"{{"case": "{named}"}}"#)),
            producer,
        )
    }

    /// A rejection by `producer`: an `invalid` `draft_check` about the current `draft`, placed.
    fn reject_by(&self, producer: &str) -> String {
        let draft = self.draft();
        self.hold(("draft_check", &draft, "invalid"), producer, true)
    }

    fn reject(&self) -> String {
        self.reject_by("checker")
    }

    /// The records of `kind` the case holds.
    fn records(&self, kind: &str) -> usize {
        self.governor
            .evidence(&CaseId(CASE.to_owned()))
            .unwrap()
            .iter()
            .filter(|held| held.kind == kind)
            .count()
    }
}

/// The refusal of a writer whose window is full: `runs` runs since the rejection `by` (evidence
/// id, producer), or since the case's start.
fn max_runs_text(runs: i64, by: Option<(&str, &str)>, max: i64) -> String {
    let since = by.map_or_else(String::new, |(id, producer)| {
        format!(" since the rejection {id} by {producer}")
    });
    format!(
        "RunsSpent: action {WRITE}: {WRITE} has run {runs} times in case {CASE}{since}, its budget.max_runs is {max}"
    )
}

/// The refusal of a writer rejected `k` times, the latest `id` by `producer`.
fn rework_limit_text(k: i64, id: &str, producer: &str, limit: i64) -> String {
    format!(
        "RunsSpent: action {WRITE}: {WRITE} was rejected {k} times in case {CASE}, the latest {id} by {producer}; its budget.rework_limit is {limit}"
    )
}

#[test]
fn max_runs_spent_between_rejections_names_max_runs() {
    for producer in ["checker", "decider:x", "rule:y"] {
        let case = Rework::new(&reworked("{max_runs: 2, runs_per: rejection}"));
        case.run();
        case.run();
        let rejection = case.reject_by(producer);
        case.run();
        case.run();
        assert_eq!(
            case.refused(),
            max_runs_text(2, Some((&rejection, producer)), 2),
            "{producer}"
        );
        assert_eq!(case.runner.children_of(WRITE_STUB).len(), 4, "{producer}");
    }
}

#[test]
fn the_rework_limit_ends_the_allowance() {
    let case = Rework::new(&reworked("{max_runs: 1, runs_per: rejection}"));
    case.run();
    case.reject();
    case.run();
    case.reject();
    case.run();
    let third = case.reject();
    assert_eq!(case.refused(), rework_limit_text(3, &third, "checker", 2));
    assert_eq!(case.runner.children_of(WRITE_STUB).len(), 3);
}

#[test]
fn two_rejections_between_two_runs_count_once() {
    let case = Rework::new(&reworked("{max_runs: 1, runs_per: rejection}"));
    case.run();
    case.reject();
    case.reject();
    // Rework count 1: the two records lie between the same two runs.
    case.run();
    case.reject();
    // Rework count 2, the default rework_limit.
    case.run();
    let last = case.reject();
    assert_eq!(case.refused(), rework_limit_text(3, &last, "checker", 2));
}

#[test]
fn records_that_open_no_window() {
    let one = reworked("{max_runs: 1, runs_per: rejection}");
    // A result retry_when does not list, and a kind it does not list.
    for (kind, result) in [("draft_check", "valid"), ("draft_lint", "invalid")] {
        let case = Rework::new(&one);
        case.run();
        let draft = case.draft();
        case.hold((kind, &draft, result), "checker", true);
        assert_eq!(
            case.refused(),
            max_runs_text(1, None, 1),
            "{kind}: {result}"
        );
    }
    // Listed records citing an observation of another case: no window, one interval.
    let case = Rework::new(&one);
    case.run();
    for n in 1..=3 {
        let draft = case.draft();
        case.hold(("draft_check", &draft, "invalid"), "checker", false);
        assert_eq!(
            case.refused(),
            max_runs_text(1, None, 1),
            "after record {n}"
        );
    }
    // A listed record of the writer's own run.
    let case = Rework::with(
        &one,
        "      - {kind: draft_check, subject: draft, producer: writer, from: exit, results: {\"*\": invalid}}\n",
    );
    case.run();
    assert_eq!(case.records("draft_check"), 1);
    assert_eq!(retries(&case.governor, writer(&case.document)).len(), 1);
    assert_eq!(case.refused(), max_runs_text(1, None, 1));
    // A listed record about an earlier revision of draft, and one about none: each opens a window.
    for revision in [DRAFT_REVISION, "none"] {
        let case = Rework::new(&one);
        case.run();
        assert_ne!(case.draft(), DRAFT_REVISION, "the run moved draft");
        case.hold(("draft_check", revision, "invalid"), "checker", true);
        case.run();
    }
}

#[test]
fn runs_per_case_counts_as_before() {
    for budget in ["{max_runs: 2}", "{max_runs: 2, runs_per: case}"] {
        let case = Rework::new(&reworked(budget));
        case.run();
        case.run();
        case.reject();
        assert_eq!(
            case.refused(),
            format!(
                "RunsSpent: action draft.write: draft.write has run 2 times in case {CASE}, its budget.max_runs is 2"
            ),
            "{budget}"
        );
    }
}

#[test]
fn an_older_store_counts_its_rejections() {
    let mut case = Rework::new(&reworked("{max_runs: 8}"));
    case.run();
    case.reject();
    case.run();
    let second = case.reject();
    case.run();
    case.rebound(&reworked(
        "{max_runs: 2, runs_per: rejection, rework_limit: 3}",
    ));
    // The window opened at the second record: one run in it so far, and two rejections.
    case.run();
    assert_eq!(
        case.refused(),
        max_runs_text(2, Some((&second, "checker")), 2)
    );
    case.reject();
    case.run();
    let fourth = case.reject();
    assert_eq!(case.refused(), rework_limit_text(4, &fourth, "checker", 3));
}

#[test]
fn spent_is_written_when_the_rework_limit_is_reached() {
    let case = Rework::with(
        &reworked("{max_runs: 2, runs_per: rejection, rework_limit: 1}"),
        SPENT_ENTRY,
    );
    case.run();
    case.run();
    // The window is full and the rework count 0: no spent record.
    assert_eq!(case.records("draft_spent"), 0);
    assert_eq!(case.refused(), max_runs_text(2, None, 2));
    assert_eq!(case.records("draft_spent"), 0);
    case.reject();
    case.run();
    assert_eq!(case.records("draft_spent"), 1);
    let second = case.reject();
    assert_eq!(case.refused(), rework_limit_text(2, &second, "checker", 1));
    assert_eq!(case.records("draft_spent"), 1);
}

#[test]
fn unplaced_records_between_two_runs_count_once() {
    let case = Rework::with(&reworked("{max_runs: 2, runs_per: rejection}"), SPENT_ENTRY);
    case.run();
    case.reject();
    case.run();
    let draft = case.draft();
    case.hold(("draft_check", &draft, "invalid"), "checker", false);
    case.hold(("draft_check", &draft, "invalid"), "checker", false);
    assert_eq!(case.records("draft_spent"), 0);
    // Rework count 2, the default rework_limit, and one run in the window.
    case.run();
    assert_eq!(case.records("draft_spent"), 1);
}

#[test]
fn spent_is_written_once_per_rework_count() {
    let case = Rework::with(
        &reworked("{max_runs: 3, runs_per: rejection, rework_limit: 1}"),
        SPENT_ENTRY,
    );
    case.run();
    assert_eq!(case.records("draft_spent"), 0);
    let rejection = case.reject();
    case.run();
    assert_eq!(case.records("draft_spent"), 1);
    case.run();
    case.run();
    assert_eq!(case.records("draft_spent"), 1);
    assert_eq!(
        case.refused(),
        max_runs_text(3, Some((&rejection, "checker")), 3)
    );
}

// The record an allowance spent for good owes (story spent-program-has-its-spent-record). A signal
// stands for any recordless end: the function reads only the allowance.

/// The writer under `runs_per: rejection` with `max_runs` and `rework_limit`, and the spent entry.
fn spent_rework(max_runs: i64, rework_limit: i64) -> Rework {
    Rework::with(
        &reworked(&format!(
            "{{max_runs: {max_runs}, runs_per: rejection, rework_limit: {rework_limit}}}"
        )),
        SPENT_ENTRY,
    )
}

/// D10 of the rework story kept: a full window below the rework limit owes nothing.
#[test]
fn a_full_window_below_the_rework_limit_owes_nothing() {
    let case = spent_rework(1, 2);
    case.run();
    case.reject();
    case.run();
    assert_eq!(case.settle(), 0);
    assert_eq!(case.records("draft_spent"), 0);
}

#[test]
fn a_recordless_run_at_the_rework_limit_owes_its_record() {
    let case = spent_rework(1, 2);
    case.run();
    case.reject();
    case.run();
    case.reject();
    case.run_recording_nothing();
    assert_eq!(case.records("draft_spent"), 0);
    assert_eq!(case.settle(), 1);
    assert_eq!(case.records("draft_spent"), 1);
    assert_eq!(case.settle(), 0);
    assert_eq!(case.records("draft_spent"), 1);
}

#[test]
fn a_recordless_first_run_at_the_limit_with_runs_left_owes_nothing_yet() {
    let case = spent_rework(2, 2);
    case.run();
    case.reject();
    case.run();
    case.reject();
    case.run_recording_nothing();
    assert_eq!(case.settle(), 0);
    let written = case.records("draft_written");
    case.run();
    assert_eq!(case.records("draft_written"), written + 1);
    assert_eq!(
        case.records("draft_spent"),
        0,
        "not the first run at that count"
    );
    assert_eq!(case.settle(), 1);
    assert_eq!(case.records("draft_spent"), 1);
}

/// The guard of "P": a rework count an unplaced rejection carried to the limit, with a full
/// window, owes nothing; the next placed rejection lets the writer run and record `spent` itself.
#[test]
fn an_unplaced_rejection_at_the_rework_limit_owes_nothing() {
    let case = spent_rework(1, 1);
    case.run();
    let draft = case.draft();
    case.hold(("draft_check", &draft, "invalid"), "checker", false);
    assert_eq!(case.settle(), 0);
    assert_eq!(case.records("draft_spent"), 0);
    case.reject();
    case.run();
    assert_eq!(case.records("draft_spent"), 1);
    assert_eq!(case.settle(), 0);
    assert_eq!(case.records("draft_spent"), 1);
}

/// The rework-count branch: an unplaced rejection carries the count past the limit after the
/// first run at the limit recorded nothing, its window not full.
#[test]
fn an_unplaced_rejection_past_the_rework_limit_owes_its_record() {
    let case = spent_rework(2, 1);
    case.run();
    case.reject();
    case.run_recording_nothing();
    let draft = case.draft();
    let unplaced = case.hold(("draft_check", &draft, "invalid"), "checker", false);
    assert_eq!(
        case.refused(),
        rework_limit_text(2, &unplaced, "checker", 1)
    );
    assert_eq!(case.settle(), 1);
    assert_eq!(case.settle(), 0);
    assert_eq!(case.records("draft_spent"), 1);
}

/// Runbook's `change.implement` members on the fixture's writer: `draft_check` stands for its own
/// `implementation_change`.
fn runbook_members(rework_on: bool) -> String {
    let on = if rework_on {
        ", rework_on: {code_review: [rejected], merge_approval: [rejected]}"
    } else {
        ""
    };
    format!(
        "    retry_when: {{draft_check: [invalid], conformance_run: [failed], code_review: [rejected], merge_approval: [rejected]}}\n    budget: {{max_runs: 3, runs_per: rejection, rework_limit: 3{on}}}\n"
    )
}

#[test]
fn rework_on_tells_a_failed_check_from_a_rejection() {
    let case = Rework::new(&runbook_members(true));
    case.run();
    let draft = case.draft();
    case.hold(("conformance_run", &draft, "failed"), "ess", true);
    // The failed check makes the writer due, as before.
    assert_eq!(retries(&case.governor, writer(&case.document)).len(), 1);
    case.run();
    let draft = case.draft();
    case.hold(("conformance_run", &draft, "failed"), "ess", true);
    case.run();
    assert_eq!(case.refused(), max_runs_text(3, None, 3));
    // A program check's rejection, citing the run of draft.check in the case.
    assert_performed(&case.drive(CHECK));
    let check_run = case
        .governor
        .try_observations()
        .unwrap()
        .into_iter()
        .rfind(|observation| {
            observation.source == PROGRAM_SOURCE
                && plain(&observation.payload)["action"] == serde_json::json!(CHECK)
        })
        .expect("the check's run")
        .observation_id;
    let draft = case.draft();
    hold_on(
        &case.governor,
        CASE,
        next(),
        ("code_review", &draft, "rejected"),
        vec![check_run],
        "checker",
    );
    case.run();
    // A decider's rejection, citing a person's observation of the case, as an approval writes it.
    let draft = case.draft();
    hold_citing(
        &case.governor,
        CASE,
        next(),
        ("merge_approval", &draft, "rejected"),
        b10x_loom_bindings::PERSON_SOURCE,
        args(&format!(
            r#"{{"case": "{CASE}", "action": "{LINT}", "principal": "decider:x"}}"#
        )),
        "decider:x",
    );
    case.run();
    // Without rework_on every retry_when pair is a rejection: the failed checks open windows.
    let case = Rework::new(&runbook_members(false));
    case.run();
    let draft = case.draft();
    case.hold(("conformance_run", &draft, "failed"), "ess", true);
    case.run();
    let draft = case.draft();
    case.hold(("conformance_run", &draft, "failed"), "ess", true);
    case.run();
    case.run();
}

// Adversary (wave 7, unit rw): the story's stated limit for records placed nowhere is "fewer
// counts, never more" (D9, The rule). A counted run that writes no record (a cancelled run here;
// in Runbook's shape every exit-0 run of `change.implement`, `results: {"0": null}`) anchors
// nothing, so a record placed nowhere received after it falls into the interval before it. Beside
// a placed rejection received in the same interval, the one interval is counted twice.
#[test]
fn adv_an_unplaced_rejection_beside_a_placed_one_after_a_recordless_run_counts_once() {
    let scratch = Scratch::new();
    let document = document_with(
        CHECK_STUB,
        WRITE_STUB,
        &reworked("{max_runs: 1, runs_per: rejection, rework_limit: 1}"),
        "",
    );
    let governor = governor();
    let recording = Arc::new(Recording::new());
    let runner = Arc::new(CancelsFirst {
        inner: Arc::clone(&recording),
        program: WRITE_STUB,
        cancelled: std::sync::atomic::AtomicBool::new(false),
    });
    let effects = port(
        &governor,
        &document,
        &scratch,
        Arc::clone(&runner) as Arc<dyn TestRunner>,
    );
    // Run 1: cancelled, counted against max_runs, no record.
    let reason = refusal(&drive(&governor, &effects, WRITE, "{}"));
    assert!(
        reason.starts_with("Cancelled: action draft.write: "),
        "{reason}"
    );
    let full = refusal(&drive(&governor, &effects, WRITE, "{}"));
    assert!(full.contains("budget.max_runs is 1"), "{full}");
    // After run 1: one placed rejection and one placed nowhere. Both lie between run 1 and the
    // next run, one interval: rework count 1, equal to rework_limit 1, and the window is open.
    let draft = governor.revisions(&CaseId(CASE.to_owned())).unwrap()["draft"].clone();
    hold_citing(
        &governor,
        CASE,
        next(),
        ("draft_check", &draft, "invalid"),
        "test/held",
        args(&format!(r#"{{"case": "{CASE}"}}"#)),
        "checker",
    );
    hold_citing(
        &governor,
        CASE,
        next(),
        ("draft_check", &draft, "invalid"),
        "test/held",
        args(r#"{"case": "case-2"}"#),
        "checker",
    );
    let end = drive(&governor, &effects, WRITE, "{}");
    match only_effect(&end) {
        EffectOutcome::Performed(_) => {}
        other => panic!("one interval holds both rejections, so the step performs: {other:?}"),
    }
}

/// Security review of unit RW (wave 7), D12 on the person path: `tree approve` writes a person
/// action's own record citing its `PERSON_SOURCE` observation alone, which names the case and the
/// action (`crates/loom-tree/src/person.rs:159-178`), after its program's run observation. That
/// record is the action's own step, so it is no rejection and opens no window: the next step is
/// refused naming `budget.max_runs is 1`, with no rejection named.
#[test]
fn security_rw_an_actions_own_person_record_opens_no_window() {
    let case = Rework::new(&reworked("{max_runs: 1, runs_per: rejection}"));
    case.run();
    let draft = case.draft();
    hold_citing(
        &case.governor,
        CASE,
        next(),
        ("draft_check", &draft, "invalid"),
        b10x_loom_bindings::PERSON_SOURCE,
        args(&format!(
            r#"{{"case": "{CASE}", "action": "{WRITE}", "principal": "ada"}}"#
        )),
        "ada",
    );
    assert_eq!(case.refused(), max_runs_text(1, None, 1));
    assert_eq!(case.runner.children_of(WRITE_STUB).len(), 1);
}

/// Correction round of unit rw (adversary mutant M1): a rejection is a record whose `kind` fact is
/// its own kind. A `draft_lint` record whose facts name `draft_check: invalid`, a pair the writer
/// lists, is none: it opens no window.
#[test]
fn a_record_whose_facts_name_another_kind_is_no_rejection() {
    let case = Rework::new(&reworked("{max_runs: 1, runs_per: rejection}"));
    case.run();
    let observation = ObservationId(uuid(next()));
    ObservationPort::observe(
        &case.governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "test/held".to_owned(),
            subject: "draft".to_owned(),
            observed_at: Timestamp(HELD_AT.to_owned()),
            payload: args(&format!(r#"{{"case": "{CASE}"}}"#)),
        }),
    )
    .unwrap();
    let facts = serde_json::json!({
        "format": "canon-evidence/1",
        "id": "held-other-kind",
        "kind": "draft_check",
        "subject": "draft",
        "subject_revision": case.draft(),
        "observed_at": HELD_AT,
        "result": "invalid",
    });
    submit_evidence(
        &case.governor,
        "checker",
        EvidenceData {
            evidence_id: EvidenceId(uuid(next())),
            case_id: CaseId(CASE.to_owned()),
            kind: "draft_lint".to_owned(),
            subject_revision: 0,
            producer: "checker".to_owned(),
            observation_ids: vec![observation],
            facts: json::parse(&facts.to_string()).unwrap(),
            provenance: Value::Null,
        },
    )
    .expect("the record is held");
    assert_eq!(case.records("draft_lint"), 1);
    assert_eq!(case.refused(), max_runs_text(1, None, 1));
}

/// Correction round of unit rw (adversary mutant M3): two rejections placed at one observation lie
/// at one position; the window starts at the later received, and the refusal names it.
#[test]
fn of_two_rejections_at_one_observation_the_later_opens_the_window() {
    let case = Rework::new(&reworked("{max_runs: 1, runs_per: rejection}"));
    case.run();
    let observation = ObservationId(uuid(next()));
    ObservationPort::observe(
        &case.governor,
        Observation::new(ObservationData {
            observation_id: observation.clone(),
            source: "test/held".to_owned(),
            subject: "draft".to_owned(),
            observed_at: Timestamp(HELD_AT.to_owned()),
            payload: args(&format!(r#"{{"case": "{CASE}"}}"#)),
        }),
    )
    .unwrap();
    let draft = case.draft();
    let listed = ("draft_check", draft.as_str(), "invalid");
    hold_on(
        &case.governor,
        CASE,
        next(),
        listed,
        vec![observation.clone()],
        "checker",
    );
    let later = hold_on(
        &case.governor,
        CASE,
        next(),
        listed,
        vec![observation],
        "decider:x",
    );
    case.run();
    assert_eq!(
        case.refused(),
        max_runs_text(1, Some((&later, "decider:x")), 1)
    );
}
