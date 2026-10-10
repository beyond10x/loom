//! Acceptance for `story:laya-arguments-slice`, the TASKBOARD I-004 vertical slice: Laya picks the
//! action, the reasoning model writes only its arguments.
//!
//! Loom runs as Commission's `AgentExecutor` on the frontier Commission's scripted fake governor
//! serves: `metrics.inspect`, `logs.search`, `release.inspect` (`docs/examples/laya-fast-selection.md`).
//! It selects with a `HybridSelector` whose fast selector is the Laya selector, against a loopback
//! stub of a Laya endpoint, and whose stronger path is the reasoning-model selector over a scripted
//! `ModelPort`; the threshold is the test's own. A counting `ArgumentGenerator` stands in for the
//! reasoning model's argument turn. With the governor, Loom revalidates the selection against the
//! case's current frontier before it proposes it (Atlas ADR 0073 § Decision;
//! `docs/integrations/laya-fast-selection.md` § Flow).
//!
//! Three runs differ only in what the stub answers:
//!
//! 1. an in-set action above the threshold: that action is proposed;
//! 2. an in-set action below the threshold: the reasoning-model selector's choice is proposed;
//! 3. `release.rollback`, not in the frontier, at 0.99: refused, the run falls back, and the
//!    reasoning-model selector's choice is proposed.
//!
//! In each, the argument generator is called exactly once, for the action finally selected, and
//! `release.rollback` reaches neither argument generation nor revalidation. Each selection carries
//! the strategy of the selector that made it (`story:fallback-selection-recording`): Laya's is
//! `FastTyped`, the reasoning model's `ReasoningModel`. In run 2 the record holds two selections,
//! Laya's `Overruled` and naming the reasoning model's, which alone reaches argument generation and
//! revalidation; in run 3 Laya's choice is not a candidate, so it is no selection. Schema validation of
//! the arguments (`decision-blocker:action-argument-schema`) and selection telemetry are not here.
//! No test makes a network call beyond 127.0.0.1.

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, PrincipalId,
    ProposedActionArguments, commission_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, TurnOutcome, TurnRequest, WireError,
    WireId,
};
use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    CatalogueEntry, RevalidateSelectionOutcome, SelectionAdmitted, SelectionId, SelectionState,
    SelectionStrategy,
};
use b10x_loom_executor::{
    ArgumentContext, ArgumentGenerator, HybridSelector, Loom, ReasoningModelSelector,
};
use b10x_loom_selector_laya::{LayaSelector, QUESTION};
use serde_json::{Value, json};

const CASE: &str = "CASE-I-004";
const REVISION: i64 = 12;
const PROMPT: &str = "Identify the likely source of the latency spike after the last deploy.";

const METRICS: &str = "metrics.inspect";
const LOGS: &str = "logs.search";
const RELEASE: &str = "release.inspect";
/// The frontier, as the example has it.
const FRONTIER: [&str; 3] = [METRICS, LOGS, RELEASE];

/// Listed nowhere in the frontier.
const ROLLBACK: &str = "release.rollback";

/// The threshold this test supplies; the hybrid has no default.
const THRESHOLD: &str = "0.9";

// ---------------------------------------------------------------------------------------------
// The stub Laya endpoint.

/// A Laya server on a loopback port of this process. It answers every `POST /v1/systemone` with
/// one fixed body and counts the requests it received.
struct StubLaya {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StubLaya {
    /// A stub whose every answer chooses `choice` with `answer_confidence`.
    fn choosing(choice: &str, answer_confidence: &str) -> Self {
        let body = format!(
            r#"{{"answers":{{"{QUESTION}":{{"choice":"{choice}","confidence":0.41,"answer_confidence":{answer_confidence}}}}}}}"#
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let addr = listener.local_addr().expect("the bound address");
        let received = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let received = Arc::clone(&received);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    serve(stream, &received, &body);
                }
            })
        };
        Self {
            addr,
            received,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn received(&self) -> Vec<Value> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for StubLaya {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one HTTP/1.1 request from `stream`, records its JSON body, answers `POST /v1/systemone`
/// with `body` (404 for anything else), then closes.
fn serve(stream: TcpStream, received: &Mutex<Vec<Value>>, body: &str) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut request = vec![0; length];
    if reader.read_exact(&mut request).is_err() {
        return;
    }
    let (status, answer) = if method == "POST" && path == "/v1/systemone" {
        received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(serde_json::from_slice(&request).unwrap_or(Value::Null));
        (200, body)
    } else {
        (404, r#"{"detail":"Not Found"}"#)
    };
    let mut stream = reader.into_inner();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Scripted\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n{answer}",
        answer.len()
    );
    let _ = stream.flush();
}

// ---------------------------------------------------------------------------------------------
// The scripted reasoning model and the counting argument generator.

/// A `ModelPort` that answers every turn by calling the tool the turn is held to with `action`,
/// and keeps every request it was sent.
struct ScriptedModel {
    wire: WireId,
    action: &'static str,
    requests: Arc<Mutex<Vec<TurnRequest>>>,
}

impl ModelPort for ScriptedModel {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        let name = request
            .tool_choice
            .named()
            .cloned()
            .ok_or_else(|| WireError::protocol("the turn is held to no tool"))?;
        Ok(TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![Item::ToolCall(ToolCall {
                call_id: CallId::new("call-1").expect("valid call id"),
                name,
                arguments: json!({ "action": self.action }),
            })],
            usage: None,
        })
    }
}

/// The ids a selection turn held the model to: the `enum` of the forced tool's `action`.
fn offered_to_model(request: &TurnRequest) -> BTreeSet<String> {
    let name = request
        .tool_choice
        .named()
        .expect("the selection turn is held to one tool");
    request
        .tool(name)
        .expect("the turn publishes the tool it is held to")
        .input_schema["properties"]["action"]["enum"]
        .as_array()
        .expect("the action is a fixed choice")
        .iter()
        .map(|id| id.as_str().expect("an id is a string").to_owned())
        .collect()
}

/// The arguments the generator writes for `action`.
fn arguments_for(action: &str) -> CommissionValue {
    CommissionValue::Object(vec![
        (
            "service".to_owned(),
            CommissionValue::Text("checkout-api".to_owned()),
        ),
        ("for".to_owned(), CommissionValue::Text(action.to_owned())),
    ])
}

/// An argument generator that records every entry it is handed and answers with
/// [`arguments_for`] it.
#[derive(Clone, Default)]
struct CountingArguments {
    handed: Arc<Mutex<Vec<CatalogueEntry>>>,
}

impl CountingArguments {
    fn handed(&self) -> Vec<CatalogueEntry> {
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ArgumentGenerator for CountingArguments {
    fn generate(
        &self,
        context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<CommissionValue, String> {
        assert_eq!(
            context.prompt, PROMPT,
            "the generator works on the run's prompt"
        );
        self.handed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(entry.clone());
        Ok(arguments_for(&entry.action))
    }
}

// ---------------------------------------------------------------------------------------------
// Commission's side.

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000401".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000402".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-sre".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

/// The fake governor, holding the case at [`REVISION`] with the three-action frontier.
fn governor() -> FakeGovernor {
    let governor = FakeGovernor::new();
    let actions = FRONTIER
        .iter()
        .map(|action| FrontierAction {
            action: (*action).to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        })
        .collect();
    governor.script(
        case(),
        [Answer::at(REVISION).with_items(Vec::new(), Vec::new(), actions)],
    );
    governor
}

// ---------------------------------------------------------------------------------------------
// One run.

/// What one run produced, and what each collaborator saw.
struct Observed {
    outcome: ExecutorOutcome,
    laya_requests: Vec<Value>,
    model_requests: Vec<TurnRequest>,
    handed: Vec<CatalogueEntry>,
    selections: Vec<(String, Option<Decimal>, SelectionStrategy, SelectionState)>,
    selection_ids: Vec<String>,
    replaced_by: Vec<Option<String>>,
    argument_requests: Vec<String>,
    revalidations: Vec<RevalidateSelectionOutcome>,
    governor_calls: Vec<GovernorCall>,
}

/// One run of Loom on the governor's frontier, with the stub answering `laya_choice` at
/// `laya_probability` and the reasoning model, if asked, naming `model_choice`.
fn run(laya_choice: &str, laya_probability: &str, model_choice: &'static str) -> Observed {
    let stub = StubLaya::choosing(laya_choice, laya_probability);
    let model_requests = Arc::new(Mutex::new(Vec::new()));
    let model = ScriptedModel {
        wire: WireId::new("scripted").expect("valid wire id"),
        action: model_choice,
        requests: Arc::clone(&model_requests),
    };
    let selector = HybridSelector::new(
        LayaSelector::new(&stub.url()).expect("a loopback endpoint is accepted"),
        ReasoningModelSelector::new(model, "reasoning-model"),
        &Decimal(THRESHOLD.to_owned()),
    )
    .expect("the supplied threshold is a decimal in [0, 1]");
    let arguments = CountingArguments::default();
    let governor = governor();
    let loom = Loom::new(selector, arguments.clone(), PROMPT).with_governor(&governor);

    // Commission hands the executor the frontier its governor issues for the case.
    let frontier = governor
        .frontier(&case())
        .expect("the governor serves the case");
    let outcome = loom.run(&commission(), &frontier);

    let selections = loom.selections();
    Observed {
        outcome,
        laya_requests: stub.received(),
        model_requests: model_requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        handed: arguments.handed(),
        selection_ids: selections
            .iter()
            .map(|held| held.data.selection_id.0.0.clone())
            .collect(),
        replaced_by: selections
            .iter()
            .map(|held| held.data.replaced_by.as_ref().map(|id| id.0.0.clone()))
            .collect(),
        selections: selections
            .into_iter()
            .map(|held| {
                (
                    held.data.action,
                    held.data.confidence,
                    held.data.strategy,
                    held.state,
                )
            })
            .collect(),
        argument_requests: loom
            .argument_requests()
            .into_iter()
            .map(|request| request.data.selection_id.0.0)
            .collect(),
        revalidations: loom.revalidations(),
        governor_calls: governor.calls(),
    }
}

/// The checks every run shares: `expected` was proposed with the generator's arguments for it,
/// the generator was called exactly once and only for it, and the selection `strategy` made of it,
/// the last recorded, is the one the argument request and the revalidation serve. When Laya's
/// choice `overruled` was overruled, it is recorded first, `Overruled` and naming that selection,
/// and reaches neither. `ROLLBACK` reaches none of them.
fn assert_slice(
    name: &str,
    observed: &Observed,
    expected: &str,
    strategy: SelectionStrategy,
    overruled: Option<&str>,
) {
    assert_eq!(
        observed.outcome,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: expected.to_owned(),
            arguments: ProposedActionArguments(arguments_for(expected)),
        }),
        "{name}: the proposal"
    );

    let handed: Vec<&str> = observed.handed.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(
        handed,
        [expected],
        "{name}: the argument generator is called exactly once, for the selected action only"
    );

    assert_eq!(
        observed.selections.len(),
        1 + usize::from(overruled.is_some()),
        "{name}: the selections: {:?}",
        observed.selections
    );
    let proposed = observed.selections.len() - 1;
    let proposed_id = observed.selection_ids[proposed].clone();
    let (action, _, made_by, state) = &observed.selections[proposed];
    assert_eq!(action, expected, "{name}: the selection");
    assert_eq!(*made_by, strategy, "{name}: the selection's strategy");
    assert_eq!(
        *state,
        SelectionState::Admitted,
        "{name}: the selection is admitted"
    );
    assert_eq!(
        observed.replaced_by[proposed], None,
        "{name}: the proposed selection has no replacement"
    );
    if let Some(overruled) = overruled {
        let (action, _, made_by, state) = &observed.selections[0];
        assert_eq!(action, overruled, "{name}: Laya's selection");
        assert_eq!(
            *made_by,
            SelectionStrategy::FastTyped,
            "{name}: Laya's selection carries Laya's strategy"
        );
        assert_eq!(
            *state,
            SelectionState::Overruled,
            "{name}: Laya's selection is overruled"
        );
        assert_eq!(
            observed.replaced_by[0],
            Some(proposed_id.clone()),
            "{name}: Laya's selection names the selection that replaced it"
        );
    }

    assert_eq!(
        observed.argument_requests,
        [proposed_id.clone()],
        "{name}: one argument request, serving the proposed selection"
    );
    assert_eq!(
        observed.revalidations,
        [RevalidateSelectionOutcome::Admitted {
            selection_admitted: SelectionAdmitted {
                selection_id: SelectionId(Uuid(proposed_id)),
            },
        }],
        "{name}: one revalidation, admitting the proposed selection"
    );
    assert_eq!(
        observed.governor_calls,
        [
            GovernorCall::Frontier(case()),
            GovernorCall::Frontier(case())
        ],
        "{name}: the governor served the run's frontier, then the current one at revalidation"
    );

    // Laya was asked once, offered exactly the frontier.
    assert_eq!(observed.laya_requests.len(), 1, "{name}: one Laya request");
    let offered: BTreeSet<&str> = observed.laya_requests[0]["questions"][QUESTION]["criteria"]
        .as_object()
        .expect("the criteria are an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        offered,
        FRONTIER.into_iter().collect(),
        "{name}: Laya is offered exactly the frontier"
    );
    for request in &observed.model_requests {
        assert_eq!(
            offered_to_model(request),
            FRONTIER.iter().map(|a| (*a).to_owned()).collect(),
            "{name}: the reasoning model is offered exactly the frontier"
        );
    }

    // The action outside the frontier reaches no argument generation and no revalidation.
    assert!(
        observed.handed.iter().all(|entry| entry.action != ROLLBACK),
        "{name}: {ROLLBACK} reached the argument generator"
    );
    assert!(
        observed.selections.iter().all(|(a, ..)| a != ROLLBACK),
        "{name}: {ROLLBACK} became a selection"
    );
    assert!(
        observed
            .revalidations
            .iter()
            .all(|outcome| !matches!(outcome, RevalidateSelectionOutcome::NotInFrontier { .. })),
        "{name}: an action outside the frontier reached revalidation"
    );
}

// ---------------------------------------------------------------------------------------------
// The acceptance.

#[test]
fn an_in_set_choice_above_the_threshold_is_proposed_with_arguments_for_it_alone() {
    let observed = run(RELEASE, "0.96", LOGS);
    assert_slice(
        "above the threshold",
        &observed,
        RELEASE,
        SelectionStrategy::FastTyped,
        None,
    );
    assert!(
        observed.model_requests.is_empty(),
        "the reasoning model is not asked to select when Laya's choice is accepted"
    );
    assert_eq!(
        observed.selections[0].1,
        Some(Decimal("0.96".to_owned())),
        "the selection carries Laya's probability"
    );
}

#[test]
fn an_in_set_choice_below_the_threshold_falls_back_to_the_reasoning_selector() {
    let observed = run(RELEASE, "0.42", LOGS);
    assert_slice(
        "below the threshold",
        &observed,
        LOGS,
        SelectionStrategy::ReasoningModel,
        Some(RELEASE),
    );
    assert_eq!(
        observed.model_requests.len(),
        1,
        "the reasoning model selects once"
    );
    assert_eq!(
        observed.selections[0].1,
        Some(Decimal("0.42".to_owned())),
        "the overruled selection carries Laya's probability"
    );
    assert_eq!(
        observed.selections[1].1, None,
        "the fallback carries no confidence"
    );
}

#[test]
fn a_choice_outside_the_frontier_is_rejected_and_never_reaches_arguments_or_revalidation() {
    let observed = run(ROLLBACK, "0.99", METRICS);
    assert_slice(
        "outside the frontier",
        &observed,
        METRICS,
        SelectionStrategy::ReasoningModel,
        None,
    );
    assert_eq!(
        observed.model_requests.len(),
        1,
        "the reasoning model selects once"
    );
    assert_eq!(
        observed.selections[0].1, None,
        "the fallback carries no confidence"
    );
}
