// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1 on `story:metering-on-llm-model` (wave 2026-10-20-w4, U4).
//!
//! Two contracts the unit claims, driven from their own documents:
//!
//! - The reasoning-model selector's contract, which the unit says `model_selector::ModelSelector`
//!   already holds, so `ReasoningModelSelector` could be removed. The removed selector's own
//!   documentation named "more than one call" among the answers that are
//!   `SelectorError::Unavailable`, "never a guess", and the story's contract is that an answer
//!   outside the candidates is a selection error, never a selection.
//! - The metering domain (`ess/domains/metering.yaml`): a call is one `turn` of the llm `Model`,
//!   and a step holds "the model calls made while it ran"; the acceptance name is that the
//!   reported calls equal the turns the model saw.
//!
//! Every model here is a scripted llm `Model`; nothing calls a live model or the network.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId,
    FrontierObligation, PrincipalId, RunId, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    ActionCatalogue, ActionCatalogueData, CatalogueEntry, CatalogueEntryStatus, CatalogueId,
    SelectionId, TurnId, action_catalogue_state,
};
use b10x_loom_executor::model_selector::{Briefing, ModelSelector};
use b10x_loom_executor::selection::{SelectionContext, select};
use b10x_loom_executor::{MeteredExecutor, MeteredModelPort, RunMeter};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome, TurnRequest, Usage,
    VecSink,
};
use serde_json::{Value, json};

const MODEL: &str = "reasoning-model";
const INSPECT: &str = "repository.inspect";
const TESTS_RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
/// Listed nowhere in the candidate set.
const OUTSIDE: &str = "release.publish";

fn provenance() -> Provenance {
    let id = |value: &str| Id::new(value).expect("a fixture id");
    Provenance {
        protocol: Protocol::Responses,
        provider: id("scripted"),
        account: id("fixture"),
        endpoint: id("in-process"),
        model: id(MODEL),
        binding_revision: id("scripted-1"),
    }
}

/// An llm `Model` that counts its `turn` calls and answers each turn held to a named tool with one
/// call of that tool per scripted action, each with its own call id; once the script is spent it
/// answers with one call naming [`TESTS_RUN`]. Every answer reports 100 input and 10 output tokens
/// as final.
struct Scripted {
    provenance: Provenance,
    capabilities: Capabilities,
    answers: Mutex<VecDeque<Vec<&'static str>>>,
    seen: Mutex<u64>,
}

impl Scripted {
    fn new(answers: Vec<Vec<&'static str>>) -> Self {
        Self {
            provenance: provenance(),
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            answers: Mutex::new(answers.into()),
            seen: Mutex::new(0),
        }
    }

    fn seen(&self) -> i64 {
        i64::try_from(*self.seen.lock().unwrap_or_else(PoisonError::into_inner))
            .expect("a small count")
    }

    fn answer(&self, request: &TurnRequest) -> Result<TurnOutcome, Error> {
        let actions = self
            .answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| vec![TESTS_RUN]);
        let observation = TurnObservation {
            usage: Some(Usage {
                input_tokens: Some(100),
                output_tokens: Some(10),
                ..Usage::default()
            }),
            final_usage: true,
            ..TurnObservation::new(self.provenance.clone())
        };
        let ToolChoice::Named(name) = &request.tool_choice else {
            return Ok(TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant("{}")],
                observation,
            });
        };
        Ok(TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: actions
                .iter()
                .enumerate()
                .map(|(at, action)| {
                    Item::ToolCall(ToolCall {
                        call_id: CallId::new(format!("call-{}", at + 1)).expect("a call id"),
                        name: name.clone(),
                        arguments: json!({ "action": action }),
                    })
                })
                .collect(),
            observation,
        })
    }
}

impl Model for Scripted {
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
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        let answered = self.answer(request);
        Box::pin(async move { answered })
    }
}

struct Brief;

impl Briefing for Brief {
    fn text(&self) -> String {
        "Instruction: the build on main is red; find out why\n".to_owned()
    }

    fn selection_instructions(&self) -> String {
        "Choose the one candidate action that best advances the instruction.".to_owned()
    }

    fn argument_instructions(&self) -> String {
        "Write the selected action's arguments.".to_owned()
    }

    fn arguments_schema(&self, _action: &str) -> Value {
        json!({"type": "object"})
    }
}

fn entry(action: &str, status: CatalogueEntryStatus) -> CatalogueEntry {
    CatalogueEntry {
        action: action.to_owned(),
        status,
    }
}

fn catalogue() -> ActionCatalogue<action_catalogue_state::Projected> {
    ActionCatalogue::new(ActionCatalogueData {
        catalogue_id: CatalogueId(Uuid("00000000-0000-4000-8000-0000000007c1".to_owned())),
        turn_id: TurnId(Uuid("00000000-0000-4000-8000-0000000007a1".to_owned())),
        frontier: "frontier of the adversary case".to_owned(),
        case_revision: 7,
        entries: vec![
            entry(INSPECT, CatalogueEntryStatus::Admissible),
            entry(TESTS_RUN, CatalogueEntryStatus::Admissible),
            entry(MERGE, CatalogueEntryStatus::ApprovalRequired),
        ],
    })
}

fn context() -> SelectionContext {
    SelectionContext {
        prompt: "the build on main is red; find out why".to_owned(),
    }
}

fn selection_id() -> SelectionId {
    SelectionId(Uuid("00000000-0000-4000-8000-0000000007e1".to_owned()))
}

/// The model answers the selection turn with two calls of `select_action`: a candidate first, then
/// an action outside the candidate set. The answer names an action outside the candidates, so it is
/// a selection error and never a selection; the removed `ReasoningModelSelector` refused any answer
/// of more than one call as `Unavailable`. `ModelSelector` takes the first call and selects it.
#[test]
fn adversary_w20261020w4_u4_an_answer_naming_an_outside_action_beside_a_candidate_is_never_a_selection()
 {
    let model = Scripted::new(vec![vec![TESTS_RUN, OUTSIDE]]);
    let selector = ModelSelector::new(&model, Brief);

    let selected = select(&selector, &context(), &catalogue(), selection_id())
        .map(|selection| selection.into_data().action);

    assert!(
        selected.is_err(),
        "an answer that also names `{OUTSIDE}`, outside the candidates, became a selection: \
         {selected:?}"
    );
}

/// The model answers the selection turn with two calls of `select_action`, each naming a different
/// candidate. Which one it chose is not stated, so the selector has no answer: the removed
/// `ReasoningModelSelector` refused it as `Unavailable`, "never a guess". `ModelSelector` takes the
/// first call.
#[test]
fn adversary_w20261020w4_u4_an_answer_of_two_selection_calls_is_no_selection_never_a_guess() {
    let model = Scripted::new(vec![vec![INSPECT, TESTS_RUN]]);
    let selector = ModelSelector::new(&model, Brief);

    let selected = select(&selector, &context(), &catalogue(), selection_id())
        .map(|selection| selection.into_data().action);

    assert!(
        selected.is_err(),
        "two calls naming `{INSPECT}` and `{TESTS_RUN}` were resolved to one of them: {selected:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// The meter.

fn uuid(n: u64) -> CommissionUuid {
    CommissionUuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId("CASE-ADVERSARY-W4-U4".to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn frontier() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: CaseId("CASE-ADVERSARY-W4-U4".to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: vec![FrontierObligation {
            obligation: "tests-pass".to_owned(),
            open: true,
        }],
        actions: vec![FrontierAction {
            action: INSPECT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    })
}

fn run_id() -> RunId {
    RunId(uuid(0x501))
}

/// One turn of `model` with no tools, driven to completion on this thread.
fn turn(model: &dyn Model) -> Result<TurnOutcome, Error> {
    let request = TurnRequest::new(MODEL, vec![Item::user("go")]);
    let mut sink = VecSink::new(64, 64 * 1024);
    let cancel = Cancel::new();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(model.turn(&request, &mut sink, &cancel))
}

fn steps(report: &RunUsageReport) -> Vec<(i64, i64)> {
    report
        .steps
        .iter()
        .map(|step| (step.step, step.model_calls))
        .collect()
}

/// A metered model handed on as an llm `Model` and metered again on the same meter, as a host does
/// that wraps "every llm `Model` the run uses" and is handed one already metered. The model saw
/// one turn; the report counts it once, with its tokens once.
#[test]
fn adversary_w20261020w4_u4_a_metered_model_metered_again_on_one_meter_counts_one_call_per_turn() {
    let model = Scripted::new(Vec::new());
    let meter = RunMeter::new(None);
    let inner: Arc<dyn Model> = Arc::new(MeteredModelPort::new(&model, &meter));
    let outer = MeteredModelPort::new(inner, &meter);

    assert!(turn(&outer).is_ok(), "the scripted turn answers");

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 1, "the model saw one turn");
    assert_eq!(
        (report.model_calls, report.input_tokens),
        (model.seen(), 100),
        "the reported calls equal the turns the model saw, and its tokens are counted once: \
         {report:?}"
    );
}

/// An executor call that makes its model turn on a thread of its own, while it runs. The metering
/// domain holds in a step "the model calls made while it ran", and step 0 the calls "made while no
/// executor call ran"; this call was made while step 1 ran, and no other executor call ran.
#[test]
fn adversary_w20261020w4_u4_a_turn_made_on_a_thread_the_executor_call_spawned_counts_in_its_step() {
    struct Worker<'m> {
        model: MeteredModelPort<&'m Scripted>,
    }

    impl AgentExecutor for Worker<'_> {
        fn run(
            &self,
            _commission: &Commission<commission_state::Assigned>,
            _frontier: &Frontier<frontier_state::Issued>,
        ) -> ExecutorOutcome {
            std::thread::scope(|scope| {
                let worker = scope.spawn(|| turn(&self.model));
                assert!(
                    worker.join().expect("the worker returned").is_ok(),
                    "the scripted turn answers"
                );
            });
            ExecutorOutcome::NoUsefulAction(Unit(true))
        }
    }

    let model = Scripted::new(Vec::new());
    let meter = RunMeter::new(None);
    let executor = MeteredExecutor::new(
        Worker {
            model: MeteredModelPort::new(&model, &meter),
        },
        &meter,
    );
    let _ = executor.run(&commission(), &frontier());

    let report = meter.report(&run_id());
    assert_eq!(model.seen(), 1);
    assert_eq!(
        steps(&report),
        vec![(1, 1)],
        "the turn made while step 1 ran belongs to step 1: {report:?}"
    );
}
