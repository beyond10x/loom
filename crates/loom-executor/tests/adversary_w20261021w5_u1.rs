// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1, wave 2026-10-21-w5, U1 (`story:judgement-step-any-executor`).
//!
//! The workflow module documents its run record: the executor writes it "after every performed
//! step and at the end of every call: ... and, metered, the usage so far (`RunMeter::carried`)",
//! and `resume` carries that usage into the next process's meter, so the report covers every
//! segment. A judgement step on the host's executor (`WorkflowExecutor::with_executor`) reports
//! what its call spent through `run_with_usage`, and the `MeteredExecutor` around the workflow adds
//! it to the call's step only once the workflow's `run_with_usage` has returned, after the record
//! of that call was written. Here the host's executor proposes a merge the provider withholds, so
//! the run stops after that call with no performed step to write the record again.
//!
//! Two controls run the same scenario on Loom's chooser over a scripted llm `Model` (its requests
//! are counted at the metered model while the call runs, so the record written at the end of the
//! call holds them), and hold the chooser to an input-token budget its two requests reach only
//! together.
//!
//! The governor, the effect port, the authority provider, the run store, the host's executor and
//! the model are scripted in this process; nothing reaches a network.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, ExecutorStep, ExecutorUsage, Frontier, FrontierAction,
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
use b10x_loom_executor::model::run::CommissionRunId;
use b10x_loom_executor::model::workflow::{WorkflowDefinition, WorkflowRunRecord};
use b10x_loom_executor::workflow::{
    self, Judgement, RecordingEffects, RunStoreError, WorkflowExecutor, WorkflowRunStore,
};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error as ModelError, Id, Item, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome,
    TurnRequest, Usage,
};
use serde_json::json;

const CASE: &str = "CASE-ADV-W5-U1";
const INSPECT: &str = "repository.inspect";
const MERGE: &str = "repository.merge";

/// What the host's executor reports each call spent.
const HOST_INPUT_TOKENS: i64 = 40;

/// Inspect without a model, then a judgement step whose one action is the merge, which needs
/// approval.
const DEFINITION: &str = r#"
name: hand-over
protocol: software-change@1
steps:
  - name: inspect
    kind: Deterministic
    action: repository.inspect
    arguments: {}
  - name: decide
    kind: Judgement
    actions: [repository.merge]
    instructions: Merge when the inspection is clean.
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
    (name == "software-change@1").then(|| vec![INSPECT.to_owned(), MERGE.to_owned()])
}

/// The scripted case: one revision per performed effect; the inspection listed admissible and,
/// once an effect was performed, the merge listed approval-required, naming its capability (so
/// the frontier after the inspection is not the one before it, and Commission's approval gate is
/// not read before the judgement step runs).
#[derive(Default)]
struct Case {
    revision: AtomicU64,
}

impl Case {
    fn revision(&self) -> i64 {
        i64::try_from(self.revision.load(Ordering::SeqCst)).unwrap_or(i64::MAX)
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision();
        let mut actions = vec![FrontierAction {
            action: INSPECT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }];
        if revision > 0 {
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

/// The host's effect port: performs every request, moving the case one revision.
struct CaseEffects<'c> {
    case: &'c Case,
}

impl EffectPort for CaseEffects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.case.revision.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// The host's executor for one judgement, an agent session the workflow knows nothing of: it
/// proposes the merge and reports what the call spent.
struct HostStep;

impl AgentExecutor for HostStep {
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
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        ExecutorStep {
            outcome: ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: MERGE.to_owned(),
                arguments: ProposedActionArguments(CommissionValue::Object(Vec::new())),
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

/// A run store in this process, keyed by the Run.
#[derive(Default)]
struct Records(Mutex<BTreeMap<String, WorkflowRunRecord>>);

impl Records {
    fn get(&self, run: &str) -> Option<WorkflowRunRecord> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(run)
            .cloned()
    }
}

impl WorkflowRunStore for Records {
    fn load(&self, run: &CommissionRunId) -> Result<Option<WorkflowRunRecord>, RunStoreError> {
        Ok(self.get(&run.0.0))
    }

    fn save(&self, record: &WorkflowRunRecord) -> Result<(), RunStoreError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(record.run.0.0.clone(), record.clone());
        Ok(())
    }
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
        Timestamp("2026-10-21T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The workflow's Run, which keys its record.
fn workflow_run() -> RunId {
    RunId(uuid(0x900))
}

/// A first segment runs the inspection and the judgement step on the host's executor, whose merge
/// the provider withholds; a second segment resumes the run from the record the first left. The
/// report after the resume covers the earlier segment, as the module documents, so it holds the
/// tokens the host's executor reported in the first: the record written at the end of that call
/// carries the usage so far.
#[test]
fn adversary_w20261021w5_u1_a_resumed_run_keeps_the_usage_the_hosts_executor_reported() {
    let definition = || workflow::load(DEFINITION, &protocol).expect("the definition loads");
    let records = Arc::new(Records::default());
    let store = || Arc::clone(&records) as Arc<dyn WorkflowRunStore + Send + Sync>;

    // The first segment.
    let case = Case::default();
    let effects = RecordingEffects::new(CaseEffects { case: &case });
    let meter = RunMeter::new(None);
    let executor =
        WorkflowExecutor::with_executor(|_: &Judgement| HostStep, definition(), effects.log())
            .with_meter(&meter)
            .with_store(store());
    executor.begin(&workflow_run()).expect("the run begins");
    let executor = MeteredExecutor::new(executor, &meter);
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
        matches!(&end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "the run stops at the merge approval: {:?}",
        end.outcome
    );
    let first = meter.report(&end.run_id);
    assert_eq!(
        first.input_tokens, HOST_INPUT_TOKENS,
        "the first segment's meter holds what the host's executor reported: {first:?}"
    );
    let recorded = records
        .get(&workflow_run().0.0)
        .expect("the store holds the run's record");

    // The second segment, resumed from the record.
    let case = Case::default();
    let effects = RecordingEffects::new(CaseEffects { case: &case });
    let resumed_meter = RunMeter::new(None);
    let resumed =
        WorkflowExecutor::with_executor(|_: &Judgement| HostStep, definition(), effects.log())
            .with_meter(&resumed_meter)
            .with_store(store());
    resumed
        .resume(&workflow_run())
        .expect("the run resumes from its record");
    let report = resumed_meter.report(&end.run_id);

    assert_eq!(
        report.input_tokens, first.input_tokens,
        "the resumed run's report covers the first segment, whose judgement call the host's \
         executor reported spending {HOST_INPUT_TOKENS} input tokens; the record the run left \
         carried {:?}, the first segment's meter {:?}",
        recorded.usage.steps, first.steps
    );
}

/// A scripted llm `Model`: each request answers with the next scripted call of the tool it is
/// held to, reporting these input tokens as final usage. A request past the script is an error.
struct ScriptedModel {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<(serde_json::Value, u64)>>,
}

impl ScriptedModel {
    fn new(script: Vec<(serde_json::Value, u64)>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
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

    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, ModelError> {
        let ToolChoice::Named(tool) = &request.tool_choice else {
            return Err(ModelError::protocol("the request is held to no tool"));
        };
        let (arguments, input_tokens) = self
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
                    input_tokens: Some(input_tokens),
                    output_tokens: Some(5),
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

fn definition() -> WorkflowDefinition {
    workflow::load(DEFINITION, &protocol).expect("the definition loads")
}

/// One segment of the run on `case` through `effects`, the provider withholding the merge.
fn segment<E: AgentExecutor>(
    case: &Case,
    executor: &E,
    effects: &RecordingEffects<CaseEffects<'_>>,
) -> LoopEnd {
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
    run_until_blocked(
        case,
        executor,
        &authority,
        effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"))
}

/// Control: the same two segments with the judgement step on Loom's chooser, whose two requests
/// report 60 and 40 input tokens at the metered model. The record written at the end of the
/// judgement call holds them, so the resumed run's report covers the first segment.
#[test]
fn adversary_w20261021w5_u1_control_a_resumed_run_keeps_the_usage_the_chooser_spent() {
    let records = Arc::new(Records::default());
    let store = || Arc::clone(&records) as Arc<dyn WorkflowRunStore + Send + Sync>;

    let case = Case::default();
    let effects = RecordingEffects::new(CaseEffects { case: &case });
    let meter = RunMeter::new(None);
    let model = ScriptedModel::new(vec![(json!({ "action": MERGE }), 60), (json!({}), 40)]);
    let metered = MeteredModel::new(&model, &meter);
    let executor = WorkflowExecutor::new(&case, &metered, definition(), effects.log())
        .with_meter(&meter)
        .with_store(store());
    executor.begin(&workflow_run()).expect("the run begins");
    let executor = MeteredExecutor::new(executor, &meter);
    let end = segment(&case, &executor, &effects);
    assert!(
        matches!(&end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "the run stops at the merge approval: {:?}",
        end.outcome
    );
    let first = meter.report(&end.run_id);
    assert_eq!(first.input_tokens, 100, "{first:?}");

    let case = Case::default();
    let effects = RecordingEffects::new(CaseEffects { case: &case });
    let resumed_meter = RunMeter::new(None);
    let idle = ScriptedModel::new(Vec::new());
    let resumed = WorkflowExecutor::new(&case, &idle, definition(), effects.log())
        .with_meter(&resumed_meter)
        .with_store(store());
    resumed
        .resume(&workflow_run())
        .expect("the run resumes from its record");
    let report = resumed_meter.report(&end.run_id);
    assert_eq!(report.input_tokens, first.input_tokens, "{report:?}");
    assert_eq!(report.model_calls, 2, "{report:?}");
}

/// The chooser's input-token budget (100) holds a call's requests together: the selection reports
/// 60 and is taken, the argument request reports 60 more and reaches it, so the run is suspended
/// for the budget with 120 reported, the merge is never proposed, and both requests are counted
/// on the judgement step.
#[test]
fn adversary_w20261021w5_u1_the_choosers_budget_sums_both_requests_of_a_call() {
    let case = Case::default();
    let effects = RecordingEffects::new(CaseEffects { case: &case });
    let meter = RunMeter::new(None);
    let model = ScriptedModel::new(vec![(json!({ "action": MERGE }), 60), (json!({}), 60)]);
    let metered = MeteredModel::new(&model, &meter);
    let executor = MeteredExecutor::new(
        WorkflowExecutor::new(&case, &metered, definition(), effects.log())
            .with_meter(&meter)
            .with_max_input_tokens(100),
        &meter,
    );
    let end = segment(&case, &executor, &effects);

    let number = |spelling: &str| CommissionValue::Number(spelling.to_owned());
    let text = |value: &str| CommissionValue::Text(value.to_owned());
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::LoopBudget(CommissionValue::Object(vec![
                ("kind".to_owned(), text("max-input-tokens")),
                ("limit".to_owned(), number("100")),
                ("reported".to_owned(), number("120")),
            ])),
        }),
    );
    let proposed: Vec<&str> = end
        .requests
        .iter()
        .map(|made| made.request.action.as_str())
        .collect();
    assert_eq!(proposed, [INSPECT], "{:?}", end.requests);
    let report = meter.report(&end.run_id);
    let steps: Vec<(Option<&str>, i64, i64)> = report
        .steps
        .iter()
        .map(|step| (step.label.as_deref(), step.model_calls, step.input_tokens))
        .collect();
    assert_eq!(steps, [(Some("inspect"), 0, 0), (Some("decide"), 2, 120)]);
}
