// SPDX-License-Identifier: Apache-2.0

//! `story:dw-workflow-definition-executor`: a Loom workflow definition runs its deterministic steps
//! without a model and asks one only in its judgement steps, through Commission's runtime.
//!
//! The case is scripted: a governor whose frontier lists the `software-change@1` actions this
//! workflow names, moving one revision per performed effect, and listing `repository.merge`
//! (approval required) once `tests.run` was performed. A judgement step runs on Loom's chooser
//! over an llm `Model`, the default (`story:judgement-step-any-executor`): one selection request
//! and one argument request per call. The model is scripted, records each request, and is metered
//! (`MeteredModel`) under a `MeteredExecutor` sharing one `RunMeter`; nothing reaches a
//! network. One case supplies the host's own executor instead.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePending, EffectOutcomePerformed,
    EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeProposedReads, ExecutorStep, ExecutorUsage, Frontier, FrontierAction,
    FrontierData, FrontierId, GovernorError, Observation, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, RunOutcomeSuspended, SuspensionReason, Unit,
    commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::metering::{MeteredExecutor, MeteredModel, RunMeter};
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model::run::CommissionRunId;
use b10x_loom_executor::model::workflow::{WorkflowDefinition, WorkflowRunRecord};
use b10x_loom_executor::workflow::{
    self, DefinitionError, Judgement, RecordingEffects, RunStoreError, WorkflowExecutor,
    WorkflowRunStore,
};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error as ModelError, Id, Item, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use serde_json::json;

const CASE: &str = "CASE-WORKFLOW";
const INSPECT: &str = "repository.inspect";
const WRITE: &str = "tests.write";
const EDIT: &str = "implementation.edit";
const RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";

/// The actions of `software-change@1` this suite's protocol lists, as the host's catalog would
/// hand them to the check (`loom_protocols::ProtocolEntry::actions`).
const PROTOCOL_ACTIONS: [&str; 6] = [INSPECT, WRITE, EDIT, RUN, MERGE, "release.rollback"];

/// The workflow of the acceptance: inspect (deterministic), edit (judgement), run the tests
/// (deterministic), then propose the merge (deterministic), which needs approval.
const DEFINITION: &str = r#"
name: change
protocol: software-change@1
steps:
  - name: inspect
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "src"}
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Edit the implementation so the failing test passes.
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {suite: "unit", flags: ["--locked"]}
  - name: merge
    kind: Deterministic
    action: repository.merge
    arguments: {}
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

fn definition(text: &str) -> WorkflowDefinition {
    workflow::load(text, &protocol).unwrap_or_else(|error| panic!("refused: {error}"))
}

/// The scripted case: one revision per performed effect; every action of the workflow listed
/// admissible unless the test blocks it, and the merge listed, approval required, once `tests.run`
/// was performed.
struct Case {
    revision: AtomicI64,
    performed: Mutex<Vec<String>>,
    blocked: Vec<&'static str>,
}

impl Case {
    fn new(blocked: Vec<&'static str>) -> Self {
        Self {
            revision: AtomicI64::new(1),
            performed: Mutex::new(Vec::new()),
            blocked,
        }
    }

    fn performed(&self) -> Vec<String> {
        self.performed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn perform(&self, action: &str) {
        self.performed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(action.to_owned());
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision.load(Ordering::SeqCst))
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision.load(Ordering::SeqCst);
        let mut actions: Vec<FrontierAction> = [INSPECT, WRITE, EDIT, RUN]
            .into_iter()
            .map(|action| {
                let blocked = self.blocked.contains(&action);
                FrontierAction {
                    action: action.to_owned(),
                    status: if blocked {
                        ActionStatus::Blocked
                    } else {
                        ActionStatus::Admissible
                    },
                    capability: None,
                    reasons: if blocked {
                        vec![format!("{action} is blocked by the test")]
                    } else {
                        Vec::new()
                    },
                }
            })
            .collect();
        if self.performed().iter().any(|done| done == RUN) {
            actions.push(FrontierAction {
                action: MERGE.to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(MERGE.to_owned()),
                reasons: Vec::new(),
            });
        }
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

/// The host's effect port: performs every action on the case, except that it refuses the first
/// invocation of each action named in `refuse_once`, and answers the first invocation of each
/// named in `pending_once` pending (`story:pending-effect`), each changing nothing.
struct CaseEffects<'c> {
    case: &'c Case,
    refuse_once: Mutex<Vec<&'static str>>,
    pending_once: Mutex<Vec<&'static str>>,
}

impl<'c> CaseEffects<'c> {
    fn new(case: &'c Case, refuse_once: Vec<&'static str>) -> Self {
        Self {
            case,
            refuse_once: Mutex::new(refuse_once),
            pending_once: Mutex::new(Vec::new()),
        }
    }

    /// The same port, answering the first invocation of each of `actions` pending.
    fn pending_once(self, actions: Vec<&'static str>) -> Self {
        Self {
            pending_once: Mutex::new(actions),
            ..self
        }
    }
}

impl EffectPort for CaseEffects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.as_str();
        let mut refuse = self
            .refuse_once
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = refuse.iter().position(|named| *named == action) {
            refuse.remove(at);
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!("{action} refused once by the test"),
                ends_run: false,
            }));
        }
        let mut pending = self
            .pending_once
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = pending.iter().position(|named| *named == action) {
            pending.remove(at);
            return Ok(EffectOutcome::Pending(EffectOutcomePending {
                report: CommissionValue::Text(format!("{action} still running")),
            }));
        }
        self.case.perform(action);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// One scripted answer of the model: a call of the tool the request is held to, with these
/// arguments, reporting this many input tokens as final usage (none reported when `None`); or
/// prose with no call.
#[derive(Debug, Clone)]
enum Answer {
    Calls(serde_json::Value, Option<u64>),
    Prose,
}

/// The selection answer naming `action`.
fn select(action: &str) -> Answer {
    Answer::Calls(json!({ "action": action }), None)
}

/// The argument answer carrying `arguments`.
fn arguments(arguments: serde_json::Value) -> Answer {
    Answer::Calls(arguments, None)
}

/// One request the model was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    performed_before: Vec<String>,
    /// The tool the request is held to.
    tool: String,
    /// The actions a selection request offers (its tool's `action` enum); none for another.
    offered: Vec<String>,
    instructions: String,
}

/// A scripted llm `Model` that records its requests: each answers with the next scripted answer,
/// and records which actions the case had performed when it was sent, the tool it is held to and
/// the actions it offers. A request past the script is an error.
struct CountingModel<'c> {
    case: &'c Case,
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Answer>>,
    asked: Mutex<Vec<Asked>>,
}

impl<'c> CountingModel<'c> {
    fn new(case: &'c Case, script: Vec<Answer>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            case,
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
            asked: Mutex::default(),
        }
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, ModelError> {
        let ToolChoice::Named(tool) = &request.tool_choice else {
            return Err(ModelError::protocol("the request is held to no tool"));
        };
        let offered = request
            .tools
            .iter()
            .filter(|spec| spec.name == *tool)
            .filter_map(|spec| spec.input_schema.pointer("/properties/action/enum"))
            .flat_map(|actions| actions.as_array().cloned().unwrap_or_default())
            .filter_map(|action| action.as_str().map(str::to_owned))
            .collect();
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Asked {
                performed_before: self.case.performed(),
                tool: tool.to_string(),
                offered,
                instructions: request.instructions.clone(),
            });
        let answer = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| ModelError::protocol("the script has no further turn"))?;
        // Terminal, as llm requires of a successful outcome; the usage only when scripted.
        let mut observation = TurnObservation {
            final_usage: true,
            ..TurnObservation::new(self.provenance.clone())
        };
        Ok(match answer {
            Answer::Calls(arguments, input_tokens) => {
                if let Some(input_tokens) = input_tokens {
                    observation.usage = Some(Usage {
                        input_tokens: Some(input_tokens),
                        output_tokens: Some(10),
                        ..Usage::default()
                    });
                }
                TurnOutcome {
                    stop_reason: StopReason::ToolCalls,
                    items: vec![Item::ToolCall(ToolCall {
                        call_id: CallId::new("call-1").expect("valid"),
                        name: tool.clone(),
                        arguments,
                    })],
                    observation,
                }
            }
            Answer::Prose => TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("I would edit the code.")],
                observation,
            },
        })
    }
}

impl Model for CountingModel<'_> {
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

/// The judgement answers of the acceptance: the model selects the edit and writes its arguments.
fn edits() -> Vec<Answer> {
    vec![
        select(EDIT),
        arguments(json!({"path": "src/lib.rs", "patch": "fix"})),
    ]
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-14T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// What one run of the workflow ended with.
struct Ran {
    end: LoopEnd,
    asked: Vec<Asked>,
    performed: Vec<String>,
    attempted: Vec<String>,
    report: RunUsageReport,
}

impl Ran {
    /// Every request the runtime made of a proposal, in order: its action and arguments.
    fn proposed(&self) -> Vec<(String, CommissionValue)> {
        self.end
            .requests
            .iter()
            .map(|made| {
                (
                    made.request.action.clone(),
                    made.request.arguments.0.clone(),
                )
            })
            .collect()
    }
}

/// Runs `text` on a fresh case blocking `blocked`, whose effect port refuses the first invocation
/// of each of `refuse_once`, with a model answering `script`.
fn run(
    text: &str,
    blocked: Vec<&'static str>,
    refuse_once: Vec<&'static str>,
    script: Vec<Answer>,
) -> Ran {
    run_effects(text, blocked, refuse_once, Vec::new(), script, None)
}

/// [`run`], the effect port answering the first invocation of each of `pending_once` pending, and
/// the chooser held to `max_input_tokens` per call when it is given.
fn run_effects(
    text: &str,
    blocked: Vec<&'static str>,
    refuse_once: Vec<&'static str>,
    pending_once: Vec<&'static str>,
    script: Vec<Answer>,
    max_input_tokens: Option<u64>,
) -> Ran {
    let case = Case::new(blocked);
    let effects =
        RecordingEffects::new(CaseEffects::new(&case, refuse_once).pending_once(pending_once));
    let meter = RunMeter::new(None);
    let model = CountingModel::new(&case, script);
    let metered = MeteredModel::new(&model, &meter);
    let mut workflow =
        WorkflowExecutor::new(&case, &metered, definition(text), effects.log()).with_meter(&meter);
    if let Some(limit) = max_input_tokens {
        workflow = workflow.with_max_input_tokens(limit);
    }
    let executor = MeteredExecutor::new(workflow, &meter);
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let authority = StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "merge-approval".to_owned(),
        }),
    );
    let end = run_until_blocked(
        &case,
        &executor,
        &authority,
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let report = meter.report(&end.run_id);
    let attempted = executor.into_inner().attempted();
    Ran {
        end,
        asked: model.asked(),
        performed: case.performed(),
        attempted,
        report,
    }
}

fn object(members: &[(&str, CommissionValue)]) -> CommissionValue {
    CommissionValue::Object(
        members
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

fn text(value: &str) -> CommissionValue {
    CommissionValue::Text(value.to_owned())
}

/// The acceptance: the workflow runs to the merge approval; the model is asked only while the
/// judgement step is the step taken (after `repository.inspect`, before the edit), and each
/// deterministic step is proposed with its declared arguments.
#[test]
fn deterministic_steps_call_no_model() {
    let ran = run(DEFINITION, Vec::new(), Vec::new(), edits());

    assert!(
        matches!(&ran.end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "the workflow runs to the merge approval: {:?}",
        ran.end.outcome
    );
    assert_eq!(ran.performed, [INSPECT, EDIT, RUN]);
    assert_eq!(
        ran.report.model_calls, 2,
        "the metered model counted the chooser's two requests, the selection and the edit's \
         arguments: {:?}",
        ran.report
    );
    let steps: Vec<(Option<&str>, i64)> = ran
        .report
        .steps
        .iter()
        .map(|step| (step.label.as_deref(), step.model_calls))
        .collect();
    assert_eq!(
        steps,
        [
            (Some("inspect"), 0),
            (Some("edit"), 2),
            (Some("test"), 0),
            (Some("merge"), 0)
        ],
        "each metered step carries the workflow step's name, and only the judgement step calls the \
         model: {:?}",
        ran.report
    );
    let tools: Vec<&str> = ran.asked.iter().map(|asked| asked.tool.as_str()).collect();
    assert_eq!(
        tools,
        ["select_action", "action_arguments"],
        "two model requests, in the judgement step: {:?}",
        ran.asked
    );
    for asked in &ran.asked {
        assert_eq!(
            asked.performed_before,
            [INSPECT],
            "the model is asked only during the judgement step: {asked:?}"
        );
        assert!(
            asked
                .instructions
                .starts_with("Edit the implementation so the failing test passes."),
            "the step's instructions lead every request: {asked:?}"
        );
    }
    assert_eq!(
        ran.proposed(),
        vec![
            (INSPECT.to_owned(), object(&[("path", text("src"))])),
            // The model's arguments, as the chooser carries them: keys in serde_json's sorted
            // order. The deterministic steps keep the definition's own order.
            (
                EDIT.to_owned(),
                object(&[("patch", text("fix")), ("path", text("src/lib.rs"))])
            ),
            (
                RUN.to_owned(),
                object(&[
                    ("suite", text("unit")),
                    ("flags", CommissionValue::Array(vec![text("--locked")]))
                ])
            ),
            (MERGE.to_owned(), object(&[])),
        ]
    );
    assert_eq!(ran.attempted, ["inspect", "edit", "test", "merge"]);
}

/// A call that takes no step (here, the one step waits on a claim the frontier never reports)
/// leaves its metered step unlabelled: a label names a workflow step the call took, never a guess.
#[test]
fn a_metered_call_that_took_no_step_carries_no_label() {
    let waits = r#"
name: change
protocol: software-change@1
steps:
  - name: inspect
    when: {claim: never.reported, value: 'True'}
    kind: Deterministic
    action: repository.inspect
    arguments: {}
"#;
    let ran = run(waits, Vec::new(), Vec::new(), Vec::new());
    assert!(ran.attempted.is_empty(), "{:?}", ran.attempted);
    assert!(
        !ran.report.steps.is_empty(),
        "the executor was called: {:?}",
        ran.report
    );
    assert!(
        ran.report.steps.iter().all(|step| step.label.is_none()),
        "{:?}",
        ran.report
    );
    assert_eq!(ran.report.model_calls, 0);
}

/// Two runs with the same inputs and the same judgement answers propose the same deterministic
/// steps with the same arguments, in the same order.
#[test]
fn a_rerun_with_the_same_inputs_proposes_the_same_deterministic_steps() {
    let first = run(DEFINITION, Vec::new(), Vec::new(), edits());
    let second = run(DEFINITION, Vec::new(), Vec::new(), edits());
    assert_eq!(first.proposed().len(), 4);
    assert_eq!(first.proposed(), second.proposed());
    assert_eq!(first.end.outcome, second.end.outcome);
}

/// A step whose action the frontier does not list as admissible or approval-required ends the
/// call `NeedsHumanJudgment`, naming the step and the frontier's reasons; nothing is proposed and
/// no later step is taken instead.
#[test]
fn a_step_the_frontier_does_not_list_ends_needing_human_judgment() {
    let ran = run(DEFINITION, vec![INSPECT], Vec::new(), edits());

    let RunOutcome::NeedsHumanJudgment(needs) = &ran.end.outcome else {
        panic!("not NeedsHumanJudgment: {:?}", ran.end.outcome);
    };
    let request = &needs.request.0;
    assert_eq!(request.member("step"), Some(&text("inspect")));
    assert_eq!(
        request.member("reasons"),
        Some(&CommissionValue::Array(vec![text(
            "repository.inspect is blocked by the test"
        )]))
    );
    assert!(ran.end.requests.is_empty(), "{:?}", ran.end.requests);
    assert!(ran.asked.is_empty(), "no model was asked: {:?}", ran.asked);
    assert!(ran.performed.is_empty());
}

/// The judgement step offers the model only its own actions: a selection naming another action
/// the frontier admits is a selection error, never proposed; the model then chooses the step's
/// action, which is.
#[test]
fn a_judgement_step_offers_the_model_only_its_actions() {
    let ran = run(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        vec![
            select(RUN),
            select(EDIT),
            arguments(json!({"path": "src/lib.rs"})),
        ],
    );

    let selections: Vec<&Asked> = ran
        .asked
        .iter()
        .filter(|asked| asked.tool == "select_action")
        .collect();
    assert_eq!(selections.len(), 2, "{:?}", ran.asked);
    for asked in selections {
        assert_eq!(
            asked.offered,
            [EDIT],
            "the judgement step offers only its own actions: {asked:?}"
        );
    }
    let proposed = ran.proposed();
    let judged = &proposed[1];
    assert_eq!(judged.0, EDIT, "{proposed:?}");
    assert_eq!(
        proposed.iter().filter(|(action, _)| action == RUN).count(),
        1,
        "tests.run is proposed only by its own deterministic step: {proposed:?}"
    );
    assert_eq!(ran.performed, [INSPECT, EDIT, RUN]);
}

/// At load, a definition naming an action its protocol does not declare is refused, naming the
/// step; so is a repeated step name, a judgement step naming no action and an unknown protocol.
#[test]
fn a_definition_naming_an_action_outside_its_protocol_is_refused() {
    let outside = DEFINITION.replace("action: tests.run", "action: tests.skip");
    match workflow::load(&outside, &protocol) {
        Err(DefinitionError::Step { step, why }) => {
            assert_eq!(step, "test");
            assert!(why.contains("tests.skip"), "{why}");
        }
        other => panic!("not refused naming the step: {other:?}"),
    }

    let judged = DEFINITION.replace("actions: [implementation.edit]", "actions: [deploy.prod]");
    assert!(matches!(
        workflow::load(&judged, &protocol),
        Err(DefinitionError::Step { step, .. }) if step == "edit"
    ));

    let repeated = DEFINITION.replace("name: merge", "name: inspect");
    assert!(matches!(
        workflow::load(&repeated, &protocol),
        Err(DefinitionError::Step { step, .. }) if step == "inspect"
    ));

    let empty = DEFINITION.replace("actions: [implementation.edit]", "actions: []");
    assert!(matches!(
        workflow::load(&empty, &protocol),
        Err(DefinitionError::Step { step, .. }) if step == "edit"
    ));

    let unknown = DEFINITION.replace("software-change@1", "software-change@9");
    assert!(matches!(
        workflow::load(&unknown, &protocol),
        Err(DefinitionError::Protocol(name)) if name == "software-change@9"
    ));

    let misspelt = DEFINITION.replace("instructions:", "instruction:");
    assert!(matches!(
        workflow::load(&misspelt, &protocol),
        Err(DefinitionError::Shape(_))
    ));
}

/// A judgement step whose chooser stops on its input-token budget (100, against a selection
/// request reporting 150) suspends the run for that budget, carrying the stop, not
/// `NoUsefulAction`; the over-budget selection is never taken, so no argument request is sent and
/// nothing is proposed for it.
#[test]
fn a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it() {
    let ran = run_effects(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            Answer::Calls(json!({ "action": EDIT }), Some(150)),
            arguments(json!({"path": "src/lib.rs"})),
        ],
        Some(100),
    );

    assert_eq!(
        ran.end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::LoopBudget(object(&[
                ("kind", text("max-input-tokens")),
                ("limit", CommissionValue::Number("100".to_owned())),
                ("reported", CommissionValue::Number("150".to_owned())),
            ])),
        }),
        "the run is suspended for the chooser's budget stop"
    );
    assert_eq!(ran.asked.len(), 1, "{:?}", ran.asked);
    assert_eq!(
        ran.report.input_tokens, 150,
        "the metered model counted the over-budget request: {:?}",
        ran.report
    );
    assert_eq!(
        ran.performed,
        [INSPECT],
        "the over-budget selection is never performed"
    );
    assert_eq!(
        ran.proposed()
            .into_iter()
            .map(|(action, _)| action)
            .collect::<Vec<_>>(),
        [INSPECT]
    );
}

/// Held to an input-token budget, a chooser whose model reports no final input-token count cannot
/// keep it: the run is suspended for the budget it could not observe, and the answer is not taken.
#[test]
fn a_judgement_step_whose_budget_cannot_be_observed_suspends_the_run_for_it() {
    let ran = run_effects(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        edits(),
        Some(100),
    );

    assert_eq!(
        ran.end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::LoopBudget(object(&[
                ("kind", text("budget-unobservable")),
                ("name", text("max_input_tokens")),
                (
                    "reason",
                    text("the model's answer reported no final input-token count")
                ),
            ])),
        }),
    );
    assert_eq!(ran.asked.len(), 1, "{:?}", ran.asked);
    assert_eq!(ran.performed, [INSPECT]);
}

/// A model that answers the judgement step's selection request without calling the tool (prose
/// only, as a refusal reads) proposes nothing: each such call is no useful action, never an
/// outage, and Commission's idle rule ends the run after two of them.
#[test]
fn a_judgement_answer_without_a_usable_call_proposes_nothing() {
    let ran = run(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        vec![Answer::Prose, Answer::Prose],
    );

    assert!(
        matches!(&ran.end.outcome, RunOutcome::IdleBound(_)),
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(ran.asked.len(), 2, "{:?}", ran.asked);
    assert!(
        ran.asked.iter().all(|asked| asked.tool == "select_action"),
        "no argument request follows an unusable selection: {:?}",
        ran.asked
    );
    assert_eq!(ran.performed, [INSPECT]);
    assert_eq!(
        ran.proposed()
            .into_iter()
            .map(|(action, _)| action)
            .collect::<Vec<_>>(),
        [INSPECT]
    );
}

/// A step advances only once its request was `Performed`: a refused `repository.inspect` is
/// proposed again, with the same arguments, before the workflow goes on.
#[test]
fn a_step_advances_only_after_its_effect_was_performed() {
    let ran = run(DEFINITION, Vec::new(), vec![INSPECT], edits());

    let proposed = ran.proposed();
    assert_eq!(proposed[0], proposed[1], "{proposed:?}");
    assert_eq!(proposed[0].0, INSPECT);
    assert_eq!(proposed[2].0, EDIT, "{proposed:?}");
    assert_eq!(ran.performed, [INSPECT, EDIT, RUN]);
    assert_eq!(ran.attempted[..2], ["inspect", "inspect"]);
    assert!(
        matches!(&ran.end.outcome, RunOutcome::NeedsAuthority(_)),
        "{:?}",
        ran.end.outcome
    );
}

/// story:pending-effect: the effect port answers `repository.inspect` pending once. The runtime
/// invokes the same request again, with no executor call between, and `RecordingEffects` settles
/// the step on the `Performed` that follows: the workflow's next call takes the edit, once, and
/// the run reaches the merge approval as it does when nothing is pending.
#[test]
fn a_step_advances_on_the_performed_effect_after_a_pending_one() {
    let ran = run_effects(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        vec![INSPECT],
        edits(),
        None,
    );

    let requests = &ran.end.requests;
    assert_eq!(
        requests[0].request, requests[1].request,
        "the pending inspect is invoked again as the same request: {requests:?}"
    );
    assert_eq!(requests[0].request.action, INSPECT);
    assert_eq!(requests[2].request.action, EDIT, "{requests:?}");
    assert!(
        matches!(
            ran.end.effects[..2],
            [EffectOutcome::Pending(_), EffectOutcome::Performed(_)]
        ),
        "{:?}",
        ran.end.effects
    );
    assert_eq!(
        ran.attempted[..2],
        ["inspect", "edit"],
        "the workflow is called once for the inspect, and its next call takes the edit"
    );
    assert_eq!(ran.performed, [INSPECT, EDIT, RUN]);
    assert!(
        matches!(&ran.end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "{:?}",
        ran.end.outcome
    );
}

/// One call of the host's executor: the judgement it was built for, the actions of the frontier
/// it was handed, and what the case had performed when it was called.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HostCall {
    workflow: String,
    step: String,
    instructions: String,
    actions: Vec<String>,
    handed: Vec<String>,
    performed_before: Vec<String>,
}

/// The host's own executor of the workflow's judgement steps, an agent the workflow knows nothing
/// of: each call proposes the next scripted action and reports what the call spent.
struct Host<'c> {
    case: &'c Case,
    script: Mutex<VecDeque<&'static str>>,
    calls: Mutex<Vec<HostCall>>,
}

/// What each call of the host's executor reports it spent.
const HOST_INPUT_TOKENS: i64 = 40;

impl<'c> Host<'c> {
    fn new(case: &'c Case, script: Vec<&'static str>) -> Self {
        Self {
            case,
            script: Mutex::new(script.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<HostCall> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The host's executor built for one judgement.
struct HostStep<'h> {
    host: &'h Host<'h>,
    judgement: Judgement,
}

impl AgentExecutor for HostStep<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        self.host
            .calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(HostCall {
                workflow: self.judgement.workflow.clone(),
                step: self.judgement.step.clone(),
                instructions: self.judgement.instructions.clone(),
                actions: self.judgement.actions.clone(),
                handed: frontier
                    .data()
                    .actions
                    .iter()
                    .map(|listed| listed.action.clone())
                    .collect(),
                performed_before: self.host.case.performed(),
            });
        let next = self
            .host
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front();
        ExecutorStep {
            outcome: next.map_or(ExecutorOutcome::NoUsefulAction(Unit(true)), |action| {
                ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                    action: action.to_owned(),
                    arguments: ProposedActionArguments(object(&[("path", text("src/lib.rs"))])),
                })
            }),
            usage: Some(ExecutorUsage {
                input_tokens: HOST_INPUT_TOKENS,
                output_tokens: 5,
                duration_ms: 10,
                cost_micro_usd: None,
            }),
            transcript_digest: None,
            summary: None,
        }
    }
}

/// story:judgement-step-any-executor: the host supplies its own executor for the judgement step.
/// It is called for the judgement step only (after `repository.inspect`, before the edit), built
/// for that step and told its instructions, and handed a frontier listing only the step's actions.
/// Its first proposal, `tests.run`, an action the case admits but the step does not name, is
/// refused: it is never proposed, and `tests.run` is proposed only by its own deterministic step.
/// What each of its calls reports it spent is the usage of that call's step. No model is given.
#[test]
fn a_judgement_step_runs_on_a_supplied_executor() {
    let case = Case::new(Vec::new());
    let effects = RecordingEffects::new(CaseEffects::new(&case, Vec::new()));
    let host = Host::new(&case, vec![RUN, EDIT]);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        WorkflowExecutor::with_executor(
            |judgement: &Judgement| HostStep {
                host: &host,
                judgement: judgement.clone(),
            },
            definition(DEFINITION),
            effects.log(),
        )
        .with_meter(&meter),
        &meter,
    );
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let authority = StaticAuthorityProvider::new().answer(
        MERGE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "merge-approval".to_owned(),
        }),
    );
    let end = run_until_blocked(
        &case,
        &executor,
        &authority,
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let report = meter.report(&end.run_id);

    assert!(
        matches!(&end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "the workflow runs to the merge approval: {:?}",
        end.outcome
    );
    assert_eq!(case.performed(), [INSPECT, EDIT, RUN]);
    let call = HostCall {
        workflow: "change".to_owned(),
        step: "edit".to_owned(),
        instructions: "Edit the implementation so the failing test passes.".to_owned(),
        actions: vec![EDIT.to_owned()],
        handed: vec![EDIT.to_owned()],
        performed_before: vec![INSPECT.to_owned()],
    };
    assert_eq!(
        host.calls(),
        [call.clone(), call],
        "the host's executor is called for the judgement step only, told its instructions and \
         offered only its actions"
    );
    let proposed: Vec<String> = end
        .requests
        .iter()
        .map(|made| made.request.action.clone())
        .collect();
    assert_eq!(
        proposed,
        [INSPECT, EDIT, RUN, MERGE],
        "the host's proposal outside the step is never proposed"
    );
    let steps: Vec<(Option<&str>, i64, i64)> = report
        .steps
        .iter()
        .map(|step| (step.label.as_deref(), step.model_calls, step.input_tokens))
        .collect();
    assert_eq!(
        steps,
        [
            (Some("inspect"), 0, 0),
            (Some("edit"), 0, HOST_INPUT_TOKENS),
            (Some("edit"), 0, HOST_INPUT_TOKENS),
            (Some("test"), 0, 0),
            (Some("merge"), 0, 0)
        ],
        "each call of the host's executor reports its usage in its own step: {report:?}"
    );
}

/// Runs `text` on a fresh case blocking `blocked`, its judgement steps on the host's executor
/// `executor` builds: how the run ended, and the actions the case performed.
fn run_supplied<F, E>(text: &str, blocked: Vec<&'static str>, executor: F) -> (LoopEnd, Vec<String>)
where
    F: Fn(&Judgement) -> E,
    E: AgentExecutor,
{
    let case = Case::new(blocked);
    let effects = RecordingEffects::new(CaseEffects::new(&case, Vec::new()));
    let workflow = WorkflowExecutor::with_executor(executor, definition(text), effects.log());
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let end = run_until_blocked(
        &case,
        &workflow,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    (end, case.performed())
}

/// A host's executor that answers every call with one outcome.
struct Answers(ExecutorOutcome);

impl AgentExecutor for Answers {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.0.clone()
    }
}

/// `action` proposed with no arguments.
fn proposal(action: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(object(&[])),
    }
}

/// A host's executor that proposes reads together, even of the step's own action, proposes
/// nothing: a judgement step proposes one action, so the reads never reach Commission, and the
/// idle rule ends the run.
#[test]
fn a_supplied_executors_reads_proposed_together_are_never_proposed() {
    let (end, performed) = run_supplied(DEFINITION, Vec::new(), |_: &Judgement| {
        Answers(ExecutorOutcome::ProposedReads(
            ExecutorOutcomeProposedReads {
                reads: vec![proposal(EDIT), proposal(INSPECT)],
            },
        ))
    });

    assert!(
        matches!(&end.outcome, RunOutcome::IdleBound(_)),
        "{:?}",
        end.outcome
    );
    assert_eq!(performed, [INSPECT]);
    let proposed: Vec<&str> = end
        .requests
        .iter()
        .map(|made| made.request.action.as_str())
        .collect();
    assert_eq!(proposed, [INSPECT], "{:?}", end.requests);
}

/// A step naming two actions, one of which the frontier blocks: the host's executor is told both,
/// and its proposal of the blocked one never reaches Commission.
#[test]
fn a_supplied_executors_proposal_the_frontier_blocks_is_never_proposed() {
    let two = DEFINITION.replace(
        "actions: [implementation.edit]",
        "actions: [implementation.edit, tests.write]",
    );
    let (end, performed) = run_supplied(&two, vec![WRITE], |judgement: &Judgement| {
        assert_eq!(judgement.actions, [EDIT, WRITE]);
        Answers(ExecutorOutcome::ProposedAction(proposal(WRITE)))
    });

    assert!(
        matches!(&end.outcome, RunOutcome::IdleBound(_)),
        "{:?}",
        end.outcome
    );
    assert_eq!(performed, [INSPECT]);
    assert!(
        end.requests.iter().all(|made| made.request.action != WRITE),
        "{:?}",
        end.requests
    );
}

/// A run store in this process, keyed by the Run.
#[derive(Default)]
struct Records(Mutex<BTreeMap<String, WorkflowRunRecord>>);

impl Records {
    fn get(&self, run: &RunId) -> WorkflowRunRecord {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&run.0.0)
            .cloned()
            .unwrap_or_else(|| panic!("no record of run `{}`", run.0.0))
    }
}

impl WorkflowRunStore for Records {
    fn load(&self, run: &CommissionRunId) -> Result<Option<WorkflowRunRecord>, RunStoreError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&run.0.0)
            .cloned())
    }

    fn save(&self, record: &WorkflowRunRecord) -> Result<(), RunStoreError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(record.run.0.0.clone(), record.clone());
        Ok(())
    }
}

/// An executor over another, held by reference: it forwards `run_with_usage` when `forwards`, and
/// otherwise calls the inner `run` alone, as a wrapper that does not forward `run_with_usage` does,
/// handing the inner call's usage on to nobody.
struct Over<'e, E> {
    inner: &'e E,
    forwards: bool,
}

impl<E: AgentExecutor> AgentExecutor for Over<'_, E> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.inner.run(commission, frontier)
    }

    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        if self.forwards {
            self.inner.run_with_usage(commission, frontier)
        } else {
            ExecutorStep {
                outcome: self.inner.run(commission, frontier),
                usage: None,
                transcript_digest: None,
                summary: None,
            }
        }
    }
}

/// The workflow whose last executor call is the judgement step on the host's executor: its merge
/// needs an approval the provider withholds, so the run stops right after that call.
const DECIDES_LAST: &str = r#"
name: change
protocol: software-change@1
steps:
  - name: inspect
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "src"}
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {}
  - name: decide
    kind: Judgement
    actions: [repository.merge]
    instructions: Merge once the tests ran.
"#;

/// story:judgement-step-any-executor, adversary pass 1: the record written at the end of the host's
/// judgement call carries the usage the call reported exactly as the run's meter counts it once
/// the call returned, and saving the record again (`WorkflowExecutor::save`) counts it once. A call
/// whose usage nobody hands on (a wrapper that calls `run` alone) leaves it out of the meter and of
/// the record alike.
#[test]
fn the_run_record_carries_a_host_executors_usage_once() {
    for handed_on in [true, false] {
        let case = Case::new(Vec::new());
        let effects = RecordingEffects::new(CaseEffects::new(&case, Vec::new()));
        let host = Host::new(&case, vec![MERGE]);
        let meter = RunMeter::new(None);
        let records = Arc::new(Records::default());
        let run = RunId(uuid(0x900));
        let workflow = WorkflowExecutor::with_executor(
            |judgement: &Judgement| HostStep {
                host: &host,
                judgement: judgement.clone(),
            },
            definition(DECIDES_LAST),
            effects.log(),
        )
        .with_meter(&meter)
        .with_store(Arc::clone(&records) as Arc<dyn WorkflowRunStore + Send + Sync>);
        workflow.begin(&run).expect("the run begins");
        let executor = MeteredExecutor::new(
            Over {
                inner: &workflow,
                forwards: handed_on,
            },
            &meter,
        );
        let ids = AtomicU64::new(0x500);
        let mut runs = Generated::new(RunStore::new(move || {
            RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
        }));
        let authority = StaticAuthorityProvider::new().answer(
            MERGE,
            AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: "merge-approval".to_owned(),
            }),
        );
        let end = run_until_blocked(
            &case,
            &executor,
            &authority,
            &effects,
            &commission(),
            &mut runs,
            &mut Context::default(),
        )
        .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
        assert!(
            matches!(&end.outcome, RunOutcome::NeedsAuthority(_)),
            "the run stops at the merge approval: {:?}",
            end.outcome
        );
        assert_eq!(
            host.calls().len(),
            1,
            "the host's executor made the run's last call"
        );
        let decided = |record: &WorkflowRunRecord| -> i64 {
            record
                .usage
                .steps
                .iter()
                .filter(|step| step.label.as_deref() == Some("decide"))
                .map(|step| step.input_tokens)
                .sum()
        };
        let reported = if handed_on { HOST_INPUT_TOKENS } else { 0 };

        let at_the_call = records.get(&run);
        assert_eq!(
            decided(&at_the_call),
            reported,
            "handed on {handed_on}: {:?}",
            at_the_call.usage.steps
        );
        assert_eq!(
            at_the_call.usage.steps,
            meter.carried().steps,
            "handed on {handed_on}: the record written at the end of the call agrees with the meter"
        );

        workflow.save().expect("the record is written again");
        let saved = records.get(&run);
        assert_eq!(
            decided(&saved),
            reported,
            "handed on {handed_on}: saving again counts the call's usage once: {:?}",
            saved.usage.steps
        );
        assert_eq!(saved.usage, at_the_call.usage, "handed on {handed_on}");
    }
}
