//! `story:dw-durable-run-state`: a workflow run's typed step results and its usage report survive a
//! restart of the run. The `WorkflowExecutor` writes its run record to the governor's
//! `FileWorkflowRunStore` after every performed step and every call, keyed by the Run id, in the
//! directory that also holds the `FileSuspensionStore`, and `resume(run_id)` in another process
//! continues from it.
//!
//! The restarts are real, as in `approval_suspend_resume.rs`: each case runs this test binary as a
//! child process over one directory, once per process life. The case is scripted and kept in that
//! directory, so it outlives a process: its revision is one more than the effects performed, which
//! the host's effect port appends to `effects.log`, one line per invocation (action, then the
//! arguments as JSON). The frontier lists every action admissible and, from the edit until the
//! merge is performed, the merge, which needs approval and names its capability, so the authority
//! provider decides it. The judgement step runs on Loom's chooser over a scripted llm `Model`
//! (`story:judgement-step-any-executor`), metered through `MeteredModel`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use b10x_loom_commission::model::json::{self, Value as CommissionValue};
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePerformed, Frontier, FrontierAction,
    FrontierData, FrontierId, GovernorError, Observation, ObservationId, PrincipalId, RunId, Unit,
    commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::ports::suspension::SuspensionPort as _;
use b10x_loom_commission::runtime::{
    LoopContext, resume_after_grant, run_until_blocked, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::metering::{MeteredExecutor, MeteredModel, RunMeter};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::CommissionRunId;
use b10x_loom_executor::workflow::{
    self, RecordingEffects, RunRecordError, WorkflowExecutor, WorkflowRunStore,
};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error as ModelError, Id, Item, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use loom_governor::{FileSuspensionStore, FileWorkflowRunStore};

const CASE: &str = "CASE-DURABLE";
const INSPECT: &str = "repository.inspect";
const EDIT: &str = "implementation.edit";
const RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";

const PROTOCOL_ACTIONS: [&str; 6] = [INSPECT, "tests.write", EDIT, RUN, MERGE, "release.rollback"];

const ROLE: &str = "LOOM_WORKFLOW_RUN_STORE_ROLE";
const DIR: &str = "LOOM_WORKFLOW_RUN_STORE_DIR";
const CHILD: &str = "a_child_process_of_these_cases";

/// The status a child exits with when it is killed after its second performed step.
const KILLED: i32 = 23;

/// The first segment's Run, and the Run of the other commission's record.
const RUN_A: u64 = 0x500;
const RUN_B: u64 = 0x900;

/// The workflow across an approval: a read whose report holds the call id, a judgement step
/// (the one model call), the merge the provider withholds, and a test run whose arguments name
/// the read's output. Only the last step comes after the approval.
const ACROSS_APPROVAL: &str = r#"
name: across-approval
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "calls"}
    outputs:
      - {name: call_id, pointer: /call/id, type: String}
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Fix the call.
  - name: merge
    kind: Deterministic
    action: repository.merge
    arguments: {call: {$step: read, $output: call_id}}
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {call: {$step: read, $output: call_id}}
"#;

/// The workflow killed between steps: three deterministic steps, the second and third naming the
/// first's output.
const BETWEEN_STEPS: &str = r#"
name: between-steps
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "calls"}
    outputs:
      - {name: call_id, pointer: /call/id, type: String}
  - name: edit
    kind: Deterministic
    action: implementation.edit
    arguments: {call: {$step: read, $output: call_id}}
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {call: {$step: read, $output: call_id}}
"#;

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn protocol(name: &str) -> Option<Vec<String>> {
    (name == "software-change@1").then(|| PROTOCOL_ACTIONS.map(str::to_owned).to_vec())
}

/// The scripted case, kept in `dir`: one revision per line of `effects.log`.
struct Case {
    dir: PathBuf,
}

impl Case {
    fn log(&self) -> Vec<(String, String)> {
        read_log(&self.dir)
    }

    fn revision(&self) -> i64 {
        1 + i64::try_from(self.log().len()).unwrap_or(0)
    }
}

fn read_log(dir: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(dir.join("effects.log"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let (action, arguments) = line.split_once('\t').expect("action, tab, arguments");
            (action.to_owned(), arguments.to_owned())
        })
        .collect()
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision();
        let done: Vec<String> = self.log().into_iter().map(|(action, _)| action).collect();
        let merging =
            done.iter().any(|action| action == EDIT) && !done.iter().any(|action| action == MERGE);
        let actions = [INSPECT, EDIT, RUN, MERGE]
            .into_iter()
            .filter(|action| *action != MERGE || merging)
            .map(|action| FrontierAction {
                action: action.to_owned(),
                status: if action == MERGE {
                    ActionStatus::ApprovalRequired
                } else {
                    ActionStatus::Admissible
                },
                capability: (action == MERGE).then(|| MERGE.to_owned()),
                reasons: Vec::new(),
            })
            .collect();
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf00 + u64::try_from(revision).unwrap_or(0))),
            case_id: case.clone(),
            case_revision: revision,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions,
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// The host's effect port: appends each invocation to the case's log and reports the call id for
/// the read.
struct LoggedEffects {
    dir: PathBuf,
}

impl EffectPort for LoggedEffects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        use std::io::Write as _;
        let data = request.data();
        let mut line = format!("{}\t", data.action);
        json::push_value(&mut line, &data.arguments.0);
        line.push('\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("effects.log"))
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .expect("the effect log takes a line");
        let report = if data.action == INSPECT {
            json::parse(r#"{"call": {"id": "c-17"}}"#).expect("JSON")
        } else {
            CommissionValue::Null
        };
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// The host's port, killed after it answered its `after`-th request: the process exits there, as
/// a process killed between steps does, once the answer was recorded.
struct Killed<E> {
    inner: E,
    after: usize,
    answered: AtomicU64,
}

impl<E: EffectPort> EffectPort for Killed<E> {
    fn performs(&self, action: &str) -> bool {
        self.inner.performs(action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let outcome = self.inner.invoke(commission, request);
        let answered = self.answered.fetch_add(1, Ordering::SeqCst) + 1;
        if usize::try_from(answered).unwrap_or(usize::MAX) >= self.after {
            std::process::exit(KILLED);
        }
        outcome
    }
}

/// One scripted answer: a call of the tool the request is held to, with these arguments, reporting
/// these input and output tokens as final usage.
type Answer = (serde_json::Value, u64, u64);

/// A scripted llm `Model` answering its script in order; a request past the script is an error.
struct ScriptedModel {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Answer>>,
}

impl ScriptedModel {
    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, ModelError> {
        let ToolChoice::Named(tool) = &request.tool_choice else {
            return Err(ModelError::protocol("the request is held to no tool"));
        };
        let (arguments, input, output) = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| ModelError::protocol("the script has no further turn"))?;
        Ok(TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![Item::ToolCall(ToolCall {
                call_id: CallId::new("call-1").expect("valid"),
                name: tool.clone(),
                arguments,
            })],
            observation: TurnObservation {
                final_usage: true,
                usage: Some(Usage {
                    input_tokens: Some(input),
                    output_tokens: Some(output),
                    ..Usage::default()
                }),
                ..TurnObservation::new(self.provenance.clone())
            },
        })
    }
}

impl Model for ScriptedModel {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, ModelError>> {
        let answered = self.answer(request);
        Box::pin(async move { answered })
    }
}

fn model(script: Vec<Answer>) -> ScriptedModel {
    let id = |value: &str| Id::new(value).expect("a fixture id");
    ScriptedModel {
        provenance: Provenance {
            protocol: Protocol::Responses,
            provider: id("scripted"),
            account: id("fixture"),
            endpoint: id("in-process"),
            model: id("scripted-model"),
            binding_revision: id("scripted-1"),
        },
        capabilities: Capabilities {
            tools: true,
            tool_choice: true,
            ..Capabilities::text(128_000, 8_192)
        },
        script: Mutex::new(script.into()),
    }
}

/// The judgement step's two model requests: the chooser's selection of the edit, then the edit's
/// arguments, reporting 100 input and 20 output tokens between them.
fn edit_answers() -> Vec<Answer> {
    vec![
        (serde_json::json!({ "action": EDIT }), 60, 10),
        (serde_json::json!({"path": "src/lib.rs"}), 40, 10),
    ]
}

struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-16T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn run_store(base: u64) -> RunStore {
    let next = AtomicU64::new(base);
    RunStore::new(move || RunId(uuid(next.fetch_add(1, Ordering::SeqCst))))
}

fn withholding() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "the merge needs the release manager's approval".to_owned(),
        }),
    )
}

fn granting() -> StaticAuthorityProvider {
    StaticAuthorityProvider::new().answer(MERGE, AuthorityVerdict::Allow(Unit(true)))
}

fn run_records(dir: &Path) -> Arc<FileWorkflowRunStore> {
    Arc::new(FileWorkflowRunStore::open(dir).expect("the run store opens"))
}

/// What a child process wrote for the parent: `key=value` lines.
struct Written(Vec<(String, String)>);

impl Written {
    fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        Self(
            text.lines()
                .filter_map(|line| line.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        )
    }

    fn get(&self, key: &str) -> &str {
        self.0
            .iter()
            .find(|(known, _)| known == key)
            .map_or_else(|| panic!("no `{key}` written"), |(_, value)| value.as_str())
    }

    fn write(lines: &[(&str, String)], path: &Path) {
        let text: String = lines
            .iter()
            .map(|(key, value)| format!("{key}={value}\n"))
            .collect();
        std::fs::write(path, text).expect("the child writes its record");
    }
}

/// The first segment of the run across an approval, in `dir`, under the Run `uuid(run)`: the run
/// reaches the merge, which the provider withholds, and is suspended.
fn suspend_segment(dir: &Path, run: u64) {
    let case = Case {
        dir: dir.to_owned(),
    };
    let effects = RecordingEffects::new(LoggedEffects {
        dir: dir.to_owned(),
    });
    let meter = RunMeter::new(None);
    let scripted = model(edit_answers());
    let metered = MeteredModel::new(&scripted, &meter);
    let definition = workflow::load(ACROSS_APPROVAL, &protocol).expect("the definition loads");
    let executor = WorkflowExecutor::new(&case, &metered, definition, effects.log())
        .with_meter(&meter)
        .with_store(run_records(dir));
    executor.begin(&RunId(uuid(run))).expect("the run begins");
    let executor = MeteredExecutor::new(executor, &meter);
    let suspensions = FileSuspensionStore::open(dir).expect("the suspension store opens");
    let end = run_with_suspension(
        &case,
        &executor,
        &withholding(),
        &effects,
        &commission(),
        &mut run_store(run),
        &suspensions,
        &mut Context { ids: 0x300 },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    assert_eq!(end.run_id, RunId(uuid(run)));
    meter.charge_effects(&end.charges);
    executor.into_inner().save().expect("the record is written");
}

/// The second segment, in a new process: the held Suspension names the Run, the executor resumes
/// it from its record, and the provider now grants the merge.
fn resume_segment(dir: &Path) {
    let case = Case {
        dir: dir.to_owned(),
    };
    let effects = RecordingEffects::new(LoggedEffects {
        dir: dir.to_owned(),
    });
    let meter = RunMeter::new(None);
    let scripted = model(Vec::new());
    let metered = MeteredModel::new(&scripted, &meter);
    let definition = workflow::load(ACROSS_APPROVAL, &protocol).expect("the definition loads");
    let suspensions = FileSuspensionStore::open(dir).expect("the suspension store opens");
    let held = suspensions
        .get(&commission().data().commission_id)
        .expect("the suspension store reads")
        .expect("a Suspension is held");
    let executor = WorkflowExecutor::new(&case, &metered, definition, effects.log())
        .with_meter(&meter)
        .with_store(run_records(dir));
    executor
        .resume(&held.data.run_id)
        .expect("the run resumes from its record");
    let executor = MeteredExecutor::new(executor, &meter);
    let end = resume_after_grant(
        &case,
        &executor,
        &granting(),
        &effects,
        &commission(),
        &mut run_store(0x600),
        &suspensions,
        &mut Context { ids: 0x700 },
    )
    .unwrap_or_else(|error| panic!("the resume failed: {error:?}"));
    meter.charge_effects(&end.charges);
    let report = meter.report(&end.run_id);
    let executor = executor.into_inner();
    let labels: Vec<String> = report
        .steps
        .iter()
        .filter_map(|step| step.label.clone())
        .collect();
    let numbers: Vec<String> = report
        .steps
        .iter()
        .map(|step| step.step.to_string())
        .collect();
    let outputs: Vec<String> = executor
        .outputs()
        .iter()
        .map(|(step, output, value)| {
            let mut text = format!("{step}.{output}=");
            json::push_value(&mut text, value);
            text
        })
        .collect();
    Written::write(
        &[
            ("pid", std::process::id().to_string()),
            ("run_id", end.run_id.0.0.clone()),
            ("held_run", held.data.run_id.0.0.clone()),
            ("model_calls", report.model_calls.to_string()),
            ("input_tokens", report.input_tokens.to_string()),
            ("labels", labels.join(",")),
            ("numbers", numbers.join(",")),
            ("effect_trials", report.effects.trials.to_string()),
            ("outputs", outputs.join(";")),
        ],
        &dir.join("resume.txt"),
    );
}

/// The run killed between steps: the host's port exits the process after the second request it
/// answered, once the record of that step was written.
fn killed_segment(dir: &Path) {
    let case = Case {
        dir: dir.to_owned(),
    };
    let recording = RecordingEffects::new(LoggedEffects {
        dir: dir.to_owned(),
    });
    let log = recording.log();
    let effects = Killed {
        inner: recording,
        after: 2,
        answered: AtomicU64::new(0),
    };
    let scripted = model(Vec::new());
    let definition = workflow::load(BETWEEN_STEPS, &protocol).expect("the definition loads");
    let executor =
        WorkflowExecutor::new(&case, &scripted, definition, log).with_store(run_records(dir));
    executor.begin(&RunId(uuid(RUN_A))).expect("the run begins");
    let _ = run_until_blocked(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(),
        &mut run_store(RUN_A),
        &mut Context { ids: 0x300 },
    );
    panic!("the process was to be killed after its second performed step");
}

/// The run resumed in a new process with `resume(run_id)`, under a new Commission run.
fn continued_segment(dir: &Path) {
    let case = Case {
        dir: dir.to_owned(),
    };
    let effects = RecordingEffects::new(LoggedEffects {
        dir: dir.to_owned(),
    });
    let scripted = model(Vec::new());
    let definition = workflow::load(BETWEEN_STEPS, &protocol).expect("the definition loads");
    let executor = WorkflowExecutor::new(&case, &scripted, definition, effects.log())
        .with_store(run_records(dir));
    executor
        .resume(&RunId(uuid(RUN_A)))
        .expect("the run resumes from its record");
    run_until_blocked(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(),
        &mut run_store(0x600),
        &mut Context { ids: 0x700 },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    Written::write(
        &[
            ("pid", std::process::id().to_string()),
            ("attempted", executor.attempted().join(",")),
        ],
        &dir.join("continued.txt"),
    );
}

/// The entry of every child process: does nothing unless the parent named a role.
#[test]
fn a_child_process_of_these_cases() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let dir = PathBuf::from(std::env::var(DIR).expect("the parent names the directory"));
    match role.as_str() {
        "suspend" => {
            suspend_segment(&dir, RUN_A);
            Written::write(
                &[("pid", std::process::id().to_string())],
                &dir.join("suspend.txt"),
            );
        }
        "resume" => resume_segment(&dir),
        "killed" => killed_segment(&dir),
        "continued" => continued_segment(&dir),
        other => panic!("unknown child role `{other}`"),
    }
}

fn child(role: &str, dir: &Path) -> Output {
    Command::new(std::env::current_exe().expect("the test binary"))
        .args([CHILD, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(DIR, dir)
        .output()
        .expect("the child starts")
}

fn run_child(role: &str, dir: &Path) {
    let output = child(role, dir);
    assert!(
        output.status.success(),
        "the {role} child failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the {role} child ran the one test"
    );
}

/// A fresh directory for one case under cargo's per-target temporary directory.
fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("workflow-run-store")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The run across an approval, suspended in one process and resumed in another, once for the
/// cases that read it.
fn across_an_approval() -> &'static (PathBuf, Written) {
    static RAN: OnceLock<(PathBuf, Written)> = OnceLock::new();
    RAN.get_or_init(|| {
        let dir = store_dir("across-approval");
        run_child("suspend", &dir);
        run_child("resume", &dir);
        let suspended = Written::read(&dir.join("suspend.txt"));
        let resumed = Written::read(&dir.join("resume.txt"));
        assert_ne!(
            suspended.get("pid"),
            resumed.get("pid"),
            "the second life is another process"
        );
        (dir, resumed)
    })
}

/// The acceptance: the test run after the approval, in the second process, carries the call id
/// the read produced in the first; the read is invoked once, and the merge once, after the grant.
#[test]
fn a_step_result_survives_a_restart_across_an_approval() {
    let (dir, resumed) = across_an_approval();
    let log = read_log(dir);
    let actions: Vec<&str> = log.iter().map(|(action, _)| action.as_str()).collect();
    assert_eq!(
        actions,
        [INSPECT, EDIT, MERGE, RUN],
        "each step performed once, the merge after the grant"
    );
    assert_eq!(
        log[2].1, r#"{"call":"c-17"}"#,
        "the merge as proposed before"
    );
    assert_eq!(
        log[3].1, r#"{"call":"c-17"}"#,
        "the step after the approval reads the output the first process produced"
    );
    assert_eq!(resumed.get("run_id"), resumed.get("held_run"));
    assert_eq!(
        resumed.get("outputs"),
        r#"read.call_id="c-17""#,
        "the resumed executor holds the output, bound"
    );
}

/// The acceptance: the report after the resume covers both segments: the first segment's steps
/// (the read, the judgement step's two model calls, the chooser's selection and the edit's
/// arguments, and their tokens, the merge) and the second's (the test run), numbered in one
/// sequence, and the effects of both.
#[test]
fn the_usage_report_covers_every_segment_of_a_resumed_run() {
    let (_, resumed) = across_an_approval();
    assert_eq!(
        resumed.get("model_calls"),
        "2",
        "the first segment's model calls, metered at the llm Model"
    );
    assert_eq!(resumed.get("input_tokens"), "100");
    assert_eq!(resumed.get("labels"), "read,edit,merge,test");
    let numbers: Vec<i64> = resumed
        .get("numbers")
        .split(',')
        .map(|number| number.parse().expect("a step number"))
        .collect();
    assert_eq!(numbers.first(), Some(&1), "{numbers:?}");
    assert!(
        numbers.windows(2).all(|pair| pair[0] < pair[1]),
        "one sequence of steps: {numbers:?}"
    );
    assert_eq!(
        resumed.get("effect_trials"),
        "4",
        "the effects of both segments: read and edit, then merge and test"
    );
}

/// The acceptance: the file of the held Suspension's Run holds the record of another Run; resuming
/// that Run refuses it, names both, and binds nothing of it.
#[test]
fn a_step_result_of_another_run_is_refused() {
    let held = store_dir("another-run-held");
    let other = store_dir("another-run-other");
    suspend_segment(&held, RUN_A);
    suspend_segment(&other, RUN_B);
    let records = FileWorkflowRunStore::open(&held).expect("the run store opens");
    let others = FileWorkflowRunStore::open(&other).expect("the run store opens");
    let a = CommissionRunId(Uuid(uuid(RUN_A).0));
    let b = CommissionRunId(Uuid(uuid(RUN_B).0));
    std::fs::copy(
        others.path(&b).expect("a path"),
        records.path(&a).expect("a path"),
    )
    .expect("the other run's record is put in the held run's place");

    let suspension = FileSuspensionStore::open(&held)
        .expect("the suspension store opens")
        .get(&commission().data().commission_id)
        .expect("the suspension store reads")
        .expect("a Suspension is held");
    assert_eq!(suspension.data.run_id, RunId(uuid(RUN_A)));
    assert!(
        records.load(&a).is_err(),
        "the store refuses a record of another Run"
    );

    let case = Case { dir: held.clone() };
    let scripted = model(Vec::new());
    let effects = RecordingEffects::new(LoggedEffects { dir: held.clone() });
    let executor = WorkflowExecutor::new(
        &case,
        &scripted,
        workflow::load(ACROSS_APPROVAL, &protocol).expect("the definition loads"),
        effects.log(),
    )
    .with_store(Arc::new(records));
    let refused = executor
        .resume(&suspension.data.run_id)
        .expect_err("a record of another run is refused");
    let said = refused.to_string();
    assert!(
        matches!(
            refused,
            RunRecordError::Store(_) | RunRecordError::OtherRun { .. }
        ),
        "{refused:?}"
    );
    assert!(said.contains(&a.0.0) && said.contains(&b.0.0), "{said}");
    assert!(
        executor.outputs().is_empty(),
        "nothing of the record is bound"
    );
    assert!(executor.attempted().is_empty());
    assert_eq!(
        executor.bound_run(),
        None,
        "the executor is bound to no run"
    );
}

/// The acceptance: killed after its second performed step, with no suspension, the run resumes in
/// a new process at its third step: the first two steps are not invoked again, and the third
/// carries the first's output.
#[test]
fn a_workflow_resumes_after_a_crash_between_steps() {
    let dir = store_dir("crash-between-steps");
    let killed = child("killed", &dir);
    assert_eq!(
        killed.status.code(),
        Some(KILLED),
        "the first process was killed: {}\n{}",
        String::from_utf8_lossy(&killed.stdout),
        String::from_utf8_lossy(&killed.stderr)
    );
    let before: Vec<String> = read_log(&dir)
        .into_iter()
        .map(|(action, _)| action)
        .collect();
    assert_eq!(
        before,
        [INSPECT, EDIT],
        "killed after its second performed step"
    );

    run_child("continued", &dir);
    let log = read_log(&dir);
    let actions: Vec<&str> = log.iter().map(|(action, _)| action.as_str()).collect();
    assert_eq!(
        actions,
        [INSPECT, EDIT, RUN],
        "the third step next, neither earlier step invoked again"
    );
    assert_eq!(
        log[2].1, r#"{"call":"c-17"}"#,
        "the first step's output is bound"
    );
    let continued = Written::read(&dir.join("continued.txt"));
    assert!(
        continued.get("attempted").starts_with("read,edit,test"),
        "{}",
        continued.get("attempted")
    );
}
