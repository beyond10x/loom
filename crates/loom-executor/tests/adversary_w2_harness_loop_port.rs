// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1, wave 2026-10-06-w2, `story:harness-loop-port`: `Loom::run_loop` driven by a
//! scripted in-process `ModelPort` (no socket, no network), with the Commission fake governor
//! serving the software-change frontiers `tests/harness_loop_port.rs` uses.
//!
//! Two cases hold the unit to its own documentation in `src/harness/governed.rs`:
//!
//! - `LoopPorts::config`: "Its tools are the catalogue's, whatever it says", and acceptance item 1:
//!   every request's tool list equals the catalogue projected from the current frontier;
//! - the module docs: "An admitted selection stops the loop at an approval checkpoint immediately
//!   before the effect, and Loom returns it as Commission's `ProposedAction`".
//!
//! The rest pin branches the implementor named untested: a denial after a failed revalidation and
//! a call against the previous turn's catalogue, the name-collision refusal, a session on another
//! wire, and the path without a governor (with two calls in one turn).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended,
    Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId, ProposedActionArguments,
    SuspensionReason, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use b10x_loom_executor::harness::governed::{LoopPorts, LoopRun, tool_name};
use b10x_loom_executor::harness::turn_loop::{
    Delegation, LoopConfig, LoopStop, OutputSchema, VecLoopSink,
};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, Risk, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome,
    TurnRequest, WireError, WireId,
};
use b10x_loom_executor::harness::{messages, responses};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueId, CommissionRunId, RevalidateSelectionOutcome, SelectionState, SelectionStrategy,
    SessionData, SessionId, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::{Value, json};

const CASE: &str = "CHG-1842";
const MODEL: &str = "scripted-model";
const PROMPT: &str = "PROMPT-land-the-change";
const SESSION: &str = "00000000-0000-4000-8000-00000000a012";
const RUN: &str = "00000000-0000-4000-8000-00000000a0ab";
const MERGE: &str = "repository.merge";
const TESTS_RUN: &str = "tests.run";

// --- the unit's own documentation ---------------------------------------------------------------

/// `LoopPorts::config` says the run's tools "are the catalogue's, whatever it says", and acceptance
/// item 1 says every request's tool list equals the projected catalogue. A config carrying an
/// output schema (a loop-owned tool, like `delegate`, `skill` and `recall`) is passed to the loop
/// unchanged, and the loop appends its own tools after the port's.
#[test]
fn adversary_w2_request_tools_are_the_catalogue_whatever_the_config_says() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_tests",
        "tests_run",
        json!({}),
    )])]);
    let schema = OutputSchema::new(json!({
        "type": "object",
        "properties": {"summary": {"type": "string"}}
    }))
    .expect("an object schema");
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config().with_output_schema(Some(schema)),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    let sent = requests.lock().expect("lock").clone();
    assert!(!sent.is_empty(), "no request was sent: {:?}", run.outcome);
    assert_eq!(
        tool_names(&sent[0]),
        published(2, &after_tests_actions()),
        "acceptance 1 and `LoopPorts::config` (\"Its tools are the catalogue's, whatever it \
         says\"): the request's tool list is the projected catalogue and nothing else"
    );
}

/// The module docs: "An admitted selection stops the loop at an approval checkpoint immediately
/// before the effect, and Loom returns it as Commission's `ProposedAction`." An in-process delegate
/// cannot hold that checkpoint, so a governed run runs none (correction 1, F1 and F2): with
/// delegation in the caller's config, no `delegate` tool is published, a call of `delegate` is
/// refused like any name outside the catalogue, and a catalogue call is proposed from the run's own
/// loop.
#[test]
fn adversary_w2_with_delegation_configured_no_delegate_is_published_and_the_call_is_proposed() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![
        // The run's first turn: try to hand the work to a delegate.
        calls(vec![call(
            "call_delegate",
            "delegate",
            json!({"task": "Run the unit tests of the change."}),
        )]),
        // The run's second turn: the catalogue action itself.
        calls(vec![call(
            "call_tests",
            "tests_run",
            json!({"suite": "unit"}),
        )]),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config().with_delegation(Some(Delegation::default())),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    let sent = requests.lock().expect("lock").clone();
    assert_eq!(
        sent.len(),
        2,
        "one request per turn of the run, none of a delegate"
    );
    for (at, request) in sent.iter().enumerate() {
        assert_eq!(
            tool_names(request),
            published(2, &after_tests_actions()),
            "request {}: the catalogue and no `delegate`",
            at + 1
        );
    }
    let (refused, failed) =
        result_for(&sent[1], "call_delegate").expect("the delegate call was answered");
    assert!(failed, "{refused}");
    assert!(refused.to_string().contains("delegate"), "{refused}");

    assert_eq!(
        run.outcome,
        proposed(TESTS_RUN, json_object("suite", "unit")),
        "loop stop {:?}",
        stop_of(&run)
    );
    assert!(
        matches!(stop_of(&run), Some(LoopStop::AwaitingApproval { .. })),
        "{:?}",
        stop_of(&run)
    );
    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "{selections:?}");
    assert_eq!(selections[0].data.action, TESTS_RUN);
    assert_eq!(selections[0].state, SelectionState::Admitted);
    assert_eq!(loom.turns().len(), 2, "{:?}", loom.turns());
}

// --- branches the implementor named untested ----------------------------------------------------

/// The case moves between the turn's projection and the call: revalidation refuses the selection,
/// the model is denied, the next turn's catalogue no longer lists the action, and a second call of
/// it (against the previous turn's catalogue) is refused before the selector.
#[test]
fn adversary_w2_failed_revalidation_is_denied_and_the_previous_catalogue_is_gone() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [
            answer(2, after_tests_actions()),
            answer(2, after_tests_actions()),
            answer(3, tests_blocked_actions()),
        ],
    );
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![call("call_1", "tests_run", json!({}))]),
        calls(vec![call("call_2", "tests_run", json!({}))]),
        prose("NOTHING-LEFT"),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert_eq!(stop_of(&run), Some(LoopStop::Completed));
    assert_eq!(
        run.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true))
    );

    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "{selections:?}");
    assert_eq!(selections[0].data.action, TESTS_RUN);
    assert_eq!(selections[0].data.case_revision, 2);
    assert_eq!(selections[0].state, SelectionState::Refused);
    assert_eq!(loom.argument_requests().len(), 1);
    let revalidations = loom.revalidations();
    assert_eq!(revalidations.len(), 1, "{revalidations:?}");
    assert!(
        !matches!(
            revalidations[0],
            RevalidateSelectionOutcome::Admitted { .. }
        ),
        "{revalidations:?}"
    );

    let sent = requests.lock().expect("lock").clone();
    assert_eq!(sent.len(), 3, "one request per turn");
    assert_eq!(tool_names(&sent[0]), published(2, &after_tests_actions()));
    assert_eq!(tool_names(&sent[1]), published(3, &tests_blocked_actions()));
    assert_eq!(tool_names(&sent[2]), published(3, &tests_blocked_actions()));
    assert!(!tool_names(&sent[1]).contains(&"tests_run".to_owned()));

    let (denied, failed) = result_for(&sent[1], "call_1").expect("the first call was answered");
    assert!(failed, "{denied}");
    assert!(denied.to_string().contains("was not proposed"), "{denied}");
    let (refused, failed) = result_for(&sent[2], "call_2").expect("the second call was answered");
    assert!(failed, "{refused}");
    assert!(refused.to_string().contains("tests_run"), "{refused}");

    assert_eq!(loom.turns().len(), 3, "{:?}", loom.turns());
    assert_eq!(
        governor.calls(),
        vec![GovernorCall::Frontier(case.clone()); 5],
        "handed, turn 1, revalidation, turn 2, turn 3"
    );
}

/// Two frontier actions published under one name end the run before any request, naming both.
#[test]
fn adversary_w2_colliding_tool_names_suspend_before_any_request() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [answer(
            2,
            vec![
                action(MERGE, ActionStatus::Admissible, None),
                action("repository_merge", ActionStatus::Admissible, None),
            ],
        )],
    );
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({}),
    )])]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert!(is_outage(&run.outcome), "{:?}", run.outcome);
    let described = format!("{:?}", run.outcome);
    assert!(
        described.contains("`repository.merge`") && described.contains("`repository_merge`"),
        "{described}"
    );
    assert!(matches!(run.run, Some(Err(_))), "{:?}", run.run);
    assert!(requests.lock().expect("lock").is_empty());
    assert!(loom.selections().is_empty());
    assert!(loom.turns().is_empty());
}

/// A session recorded on another wire is refused before the loop is built and is not opened: the
/// same session id is still free for a run on the model's own wire afterwards.
#[test]
fn adversary_w2_a_session_on_another_wire_is_refused_and_not_opened() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);

    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_tests",
        "tests_run",
        json!({}),
    )])]);
    let mut sink = VecLoopSink::new();
    let refused = loom.run_loop(
        &session(messages::WIRE),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );
    assert!(is_outage(&refused.outcome), "{:?}", refused.outcome);
    assert!(
        format!("{:?}", refused.outcome).contains(messages::WIRE),
        "{:?}",
        refused.outcome
    );
    assert!(refused.run.is_none());
    assert!(requests.lock().expect("lock").is_empty());
    assert!(loom.turns().is_empty());

    let handed = issued(&governor, &case);
    let mut sink = VecLoopSink::new();
    let accepted = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );
    assert_eq!(
        accepted.outcome,
        proposed(TESTS_RUN, CommissionValue::Object(Vec::new()))
    );
}

/// Without a governor every turn's catalogue is the handed frontier's and nothing is revalidated.
/// Two catalogue calls in one turn: the first is proposed, the second never reaches the pipeline.
#[test]
fn adversary_w2_without_a_governor_the_handed_frontier_is_the_catalogue() {
    let case = CaseId(CASE.to_owned());
    let loom = Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT);
    let handed = frontier(&case, 2, after_tests_actions());
    let (mut model, requests) = Scripted::new(vec![calls(vec![
        call("call_tests", "tests_run", json!({"suite": "unit"})),
        call("call_edit", "repository_edit", json!({})),
    ])]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert_eq!(
        run.outcome,
        proposed(TESTS_RUN, json_object("suite", "unit"))
    );
    assert!(
        matches!(stop_of(&run), Some(LoopStop::AwaitingApproval { .. })),
        "{:?}",
        stop_of(&run)
    );
    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "{selections:?}");
    assert_eq!(selections[0].data.action, TESTS_RUN);
    assert_eq!(
        selections[0].data.strategy,
        SelectionStrategy::ReasoningModel
    );
    assert_eq!(selections[0].state, SelectionState::Selected);
    assert_eq!(loom.argument_requests().len(), 1);
    assert!(loom.revalidations().is_empty());

    let sent = requests.lock().expect("lock").clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(tool_names(&sent[0]), published(2, &after_tests_actions()));
    assert_eq!(loom.turns().len(), 1);
}

/// A summary turn the loop spends between two conversation turns is not a turn of the session: the
/// session holds the conversation turns with their own items (`governed.rs`, `Recording::turn`).
/// The unit's acceptance test has one model turn per catalogue, so nothing else reaches the guard
/// that records only the first turn after an offer.
#[test]
fn adversary_w2_a_summary_turn_is_not_a_turn_of_the_session() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    // Two calls of an unpublished name, each with 40 KB of arguments: refused before the selector,
    // and heavy enough that a 10 000-token window folds the first into a summary before turn 3.
    let pad = "x".repeat(40 * 1024);
    let first = call("call_1", "no_such_tool", json!({"pad": pad}));
    let second = call("call_2", "no_such_tool", json!({"pad": pad}));
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![first.clone()]),
        calls(vec![second.clone()]),
        prose("SUMMARY-OF-THE-EARLIER-TURNS"),
        prose("DONE"),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config().with_context_window(Some(10_000)),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert_eq!(stop_of(&run), Some(LoopStop::Completed), "{:?}", run.run);
    assert_eq!(
        run.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true))
    );
    let sent = requests.lock().expect("lock").clone();
    assert_eq!(sent.len(), 4, "turn 1, turn 2, the summary, turn 3");
    assert!(
        sent[2].tools.is_empty(),
        "the third request is the summary request"
    );
    assert!(loom.selections().is_empty());

    let encode = |item: Item| serde_json::to_string(&item).expect("encodes");
    let turns = loom.turns();
    assert_eq!(turns.len(), 3, "{turns:?}");
    let expected = [
        vec![encode(Item::ToolCall(first))],
        vec![encode(Item::ToolCall(second))],
        vec![encode(Item::assistant("DONE"))],
    ];
    for (at, (turn, items)) in turns.iter().zip(&expected).enumerate() {
        assert_eq!(turn.data.index, i64::try_from(at + 1).expect("small"));
        assert_eq!(&turn.data.items, items, "turn {}", at + 1);
    }
}

/// "Every one asks before it runs, whatever the run's unattended ceiling" (`governed.rs`, `spec`).
/// At the `Destructive` ceiling a `High` envelope no longer asks (`Envelope::needs_approval`), so
/// only `Approval::Required`, a field the wire marks as being retired, keeps the call off
/// `ToolPort::call`: the call is still proposed, not run and refused.
#[test]
fn adversary_w2_the_most_permissive_ceiling_still_proposes() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![call("call_tests", "tests_run", json!({}))]),
        prose("AFTER-A-CALL-THAT-RAN"),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(responses::WIRE),
        LoopPorts {
            model: &mut model,
            config: config().with_unattended_ceiling(Risk::Destructive),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert_eq!(
        run.outcome,
        proposed(TESTS_RUN, CommissionValue::Object(Vec::new()))
    );
    assert_eq!(requests.lock().expect("lock").len(), 1);
    assert_eq!(loom.selections().len(), 1);
}

// --- the scripted model -------------------------------------------------------------------------

/// A model that answers its scripted turns in order and keeps every request it was sent; past the
/// script it fails the turn.
struct Scripted {
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    requests: Arc<Mutex<Vec<TurnRequest>>>,
}

impl Scripted {
    fn new(turns: Vec<TurnOutcome>) -> (Self, Arc<Mutex<Vec<TurnRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let model = Self {
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: turns.into(),
            requests: requests.clone(),
        };
        (model, requests)
    }
}

impl ModelPort for Scripted {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests.lock().expect("lock").push(request.clone());
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

fn call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        call_id: CallId::new(id).expect("valid"),
        name: ToolName::new(name).expect("valid"),
        arguments,
    }
}

fn calls(calls: Vec<ToolCall>) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: calls.into_iter().map(Item::ToolCall).collect(),
        usage: None,
    }
}

fn prose(text: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant(text)],
        usage: None,
    }
}

fn tool_names(request: &TurnRequest) -> Vec<String> {
    request
        .tools
        .iter()
        .map(|spec| spec.name.as_str().to_owned())
        .collect()
}

/// The answer the conversation of `request` carries for the call `call_id`: its output and whether
/// it failed.
fn result_for(request: &TurnRequest, call_id: &str) -> Option<(Value, bool)> {
    let wanted = CallId::new(call_id).expect("valid");
    request.items.iter().find_map(|item| match item {
        Item::ToolResult {
            call_id,
            output,
            failed,
        } if *call_id == wanted => Some((output.clone(), *failed)),
        _ => None,
    })
}

fn stop_of(run: &LoopRun) -> Option<LoopStop> {
    match &run.run {
        Some(Ok(answered)) => Some(answered.stop.clone()),
        _ => None,
    }
}

// --- the case -----------------------------------------------------------------------------------

fn action(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

/// "After `tests.run` on R2": merge needs approval; the others are admissible.
fn after_tests_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Admissible, None),
        action(TESTS_RUN, ActionStatus::Admissible, None),
        action(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]
}

/// The case moved on: `tests.run` is blocked.
fn tests_blocked_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("repository.edit", ActionStatus::Admissible, None),
        action(TESTS_RUN, ActionStatus::Blocked, None),
        action(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]
}

fn answer(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

fn frontier(
    case: &CaseId,
    revision: i64,
    actions: Vec<FrontierAction>,
) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000e0".to_owned(),
        )),
        case_id: case.clone(),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

/// The tool names of the catalogue projected from a frontier at `revision` listing `actions`.
fn published(revision: i64, actions: &[FrontierAction]) -> Vec<String> {
    let id = || Uuid("00000000-0000-4000-8000-0000000000f1".to_owned());
    project(
        &frontier(&CaseId(CASE.to_owned()), revision, actions.to_vec()),
        CatalogueId(id()),
        TurnId(id()),
    )
    .data()
    .entries
    .iter()
    .map(|entry| {
        tool_name(&entry.action)
            .expect("publishable")
            .as_str()
            .to_owned()
    })
    .collect()
}

fn issued(governor: &FakeGovernor, case: &CaseId) -> Frontier<frontier_state::Issued> {
    governor
        .frontier(case)
        .unwrap_or_else(|error| panic!("frontier for {} failed: {error:?}", case.0))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(CommissionValue::Null),
    })
}

fn session(wire: &str) -> SessionData {
    SessionData {
        session_id: SessionId(Uuid(SESSION.to_owned())),
        commission_run: CommissionRunId(Uuid(RUN.to_owned())),
        wire: wire.to_owned(),
    }
}

fn config() -> LoopConfig {
    LoopConfig::new(MODEL, "INSTRUCTIONS-standing").with_retry_backoff(Duration::from_millis(1))
}

fn proposed(action: &str, arguments: CommissionValue) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(arguments),
    })
}

fn json_object(key: &str, text: &str) -> CommissionValue {
    CommissionValue::Object(vec![(
        key.to_owned(),
        CommissionValue::Text(text.to_owned()),
    )])
}

fn is_outage(outcome: &ExecutorOutcome) -> bool {
    matches!(
        outcome,
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::ExternalAvailability(_),
        })
    )
}
