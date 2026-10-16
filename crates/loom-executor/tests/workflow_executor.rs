// SPDX-License-Identifier: Apache-2.0

//! `story:dw-workflow-definition-executor`: a Loom workflow definition runs its deterministic steps
//! without a model and asks one only in its judgement steps, through Commission's runtime.
//!
//! The case is scripted: a governor whose frontier lists the `software-change@1` actions this
//! workflow names, moving one revision per performed effect, and listing `repository.merge`
//! (approval required) once `tests.run` was performed. The model is scripted, records each call,
//! and is metered (`MeteredModelPort`) under a `MeteredExecutor` sharing one `RunMeter`; nothing
//! reaches a network.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePerformed, EffectOutcomeRefused, Frontier,
    FrontierAction, FrontierData, FrontierId, GovernorError, Observation, ObservationId,
    PrincipalId, RunId, RunOutcome, RunOutcomeSuspended, SuspensionReason, Unit, commission_state,
    frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{Budget, LoopConfig};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    Usage, WireError, WireId,
};
use b10x_loom_executor::metering::{MeteredExecutor, MeteredModelPort, RunMeter};
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, SessionData, SessionId};
use b10x_loom_executor::model::workflow::WorkflowDefinition;
use b10x_loom_executor::workflow::{self, DefinitionError, RecordingEffects, WorkflowExecutor};
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
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
/// invocation of each action named in `refuse_once`, changing nothing.
struct CaseEffects<'c> {
    case: &'c Case,
    refuse_once: Mutex<Vec<&'static str>>,
}

impl<'c> CaseEffects<'c> {
    fn new(case: &'c Case, refuse_once: Vec<&'static str>) -> Self {
        Self {
            case,
            refuse_once: Mutex::new(refuse_once),
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
            }));
        }
        self.case.perform(action);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
        }))
    }
}

/// A scripted model that counts its calls: each turn it answers with the next scripted outcome,
/// and records which actions the case had performed when it was asked and the tools it was
/// offered.
struct CountingModel<'c> {
    case: &'c Case,
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    asked: Vec<Asked>,
}

/// One call of the model.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    performed_before: Vec<String>,
    tools: Vec<String>,
}

impl<'c> CountingModel<'c> {
    fn new(case: &'c Case, turns: Vec<TurnOutcome>) -> Self {
        Self {
            case,
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: turns.into(),
            asked: Vec::new(),
        }
    }
}

impl ModelPort for CountingModel<'_> {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked.push(Asked {
            performed_before: self.case.performed(),
            tools: request
                .tools
                .iter()
                .map(|tool| tool.name.to_string())
                .collect(),
        });
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

/// A turn that calls `tool` with `arguments`.
fn calls(id: &str, tool: &str, arguments: serde_json::Value) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(ToolCall {
            call_id: CallId::new(id).expect("valid"),
            name: ToolName::new(tool).expect("valid"),
            arguments,
        })],
        usage: None,
    }
}

/// The judgement answer of the acceptance: the model edits the implementation.
fn edits() -> Vec<TurnOutcome> {
    vec![calls(
        "call_edit",
        "implementation_edit",
        json!({"path": "src/lib.rs", "patch": "fix"}),
    )]
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

fn session() -> SessionData {
    SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000d012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000d0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
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
/// of each of `refuse_once`, with a model answering `turns`.
fn run(
    text: &str,
    blocked: Vec<&'static str>,
    refuse_once: Vec<&'static str>,
    turns: Vec<TurnOutcome>,
) -> Ran {
    run_configured(
        text,
        blocked,
        refuse_once,
        turns,
        LoopConfig::new("scripted-model", "INSTRUCTIONS"),
    )
}

/// [`run`], with the model loop of a judgement step under `config`.
fn run_configured(
    text: &str,
    blocked: Vec<&'static str>,
    refuse_once: Vec<&'static str>,
    turns: Vec<TurnOutcome>,
    config: LoopConfig,
) -> Ran {
    let case = Case::new(blocked);
    let effects = RecordingEffects::new(CaseEffects::new(&case, refuse_once));
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "change the code",
    )
    .with_instance("workflow-test")
    .with_governor(&case);
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        WorkflowExecutor::new(
            &loom,
            MeteredModelPort::new(CountingModel::new(&case, turns), &meter),
            config.with_retry_backoff(Duration::from_millis(1)),
            session(),
            definition(text),
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
    let executor = executor.into_inner();
    let attempted = executor.attempted();
    let asked = executor.into_model().into_inner().asked;
    Ran {
        end,
        asked,
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
        ran.report.model_calls, 1,
        "the metered port counted one model call: {:?}",
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
            (Some("edit"), 1),
            (Some("test"), 0),
            (Some("merge"), 0)
        ],
        "each metered step carries the workflow step's name, and only the judgement step calls the \
         model: {:?}",
        ran.report
    );
    assert_eq!(
        ran.asked.len(),
        1,
        "one model call, in the judgement step: {:?}",
        ran.asked
    );
    for asked in &ran.asked {
        assert_eq!(
            asked.performed_before,
            [INSPECT],
            "the model is asked only during the judgement step: {asked:?}"
        );
    }
    assert_eq!(
        ran.proposed(),
        vec![
            (INSPECT.to_owned(), object(&[("path", text("src"))])),
            // The model's arguments, as the governed loop carries them: keys in serde_json's
            // sorted order. The deterministic steps keep the definition's own order.
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

/// The judgement step offers the model only its own actions: a call naming another action the
/// frontier admits is refused to the model and never proposed; the model then chooses the step's
/// action, which is.
#[test]
fn a_judgement_step_offers_the_model_only_its_actions() {
    let ran = run(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        vec![
            calls("call_run", "tests_run", json!({})),
            calls(
                "call_edit",
                "implementation_edit",
                json!({"path": "src/lib.rs"}),
            ),
        ],
    );

    assert!(ran.asked.len() >= 2, "{:?}", ran.asked);
    for asked in &ran.asked {
        assert_eq!(
            asked.tools,
            ["implementation_edit"],
            "the judgement step publishes only its own actions: {asked:?}"
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

/// A judgement step whose model loop stops on its input-token budget (100, against a turn reporting
/// 150) suspends the run for that budget, carrying the loop's stop, not `NoUsefulAction`; the
/// call of the over-budget turn is never proposed.
#[test]
fn a_judgement_step_stopped_by_its_input_token_budget_suspends_the_run_for_it() {
    let mut over = calls(
        "call_edit",
        "implementation_edit",
        json!({"path": "src/lib.rs"}),
    );
    over.usage = Some(Usage {
        model: "scripted-model".to_owned(),
        input_tokens: 150,
        output_tokens: 10,
        cached_input_tokens: 0,
        cache_creation_input_tokens: None,
    });
    let ran = run_configured(
        DEFINITION,
        Vec::new(),
        Vec::new(),
        vec![over],
        LoopConfig::new("scripted-model", "INSTRUCTIONS").with_budget(Budget {
            max_input_tokens: Some(100),
            ..Budget::default()
        }),
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
        "the run is suspended for the loop's budget stop"
    );
    assert_eq!(ran.asked.len(), 1, "{:?}", ran.asked);
    assert_eq!(
        ran.performed,
        [INSPECT],
        "the over-budget call is never performed"
    );
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
