// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 2, wave 2026-10-06-w2, `story:harness-loop-port`, at `1d9c16b`: `Loom::run_loop`
//! driven by a scripted in-process `ModelPort` (no socket, no network), with the Commission fake
//! governor serving the software-change frontiers the unit's own tests use.
//!
//! - A narrowing in the caller's `LoopConfig` meets a port whose names change every turn.
//! - The model's arguments, which are the argument generator's answer, meet Commission's JSON.
//! - Two runs at once in one session meet the turn numbering.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::{self as commission_json, Value as CommissionValue};
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::harness::governed::{LoopPorts, tool_name};
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{LoopConfig, VecLoopSink};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueId, CommissionRunId, SessionData, SessionId, SessionState, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::{Value, json};

const CASE: &str = "CHG-1842";
const MODEL: &str = "scripted-model";
const PROMPT: &str = "PROMPT-land-the-change";
const SESSION: &str = "00000000-0000-4000-8000-00000000b012";
const RUN: &str = "00000000-0000-4000-8000-00000000b0ab";
const MERGE: &str = "repository.merge";
const TESTS_RUN: &str = "tests.run";

/// `LoopPorts::config` (`governed.rs:96-98`): "Its tools are the catalogue's, whatever it says".
/// `run_loop` clears the four fields that publish a loop-owned tool and passes `LoopConfig::admits`
/// through. The ported narrowing was written for a port whose names do not change: a grant that
/// names something the port does not publish turns every published name into a route and admits it
/// (`AgentLoop::needs_routes`, `AgentLoop::routes`). In the governed loop the port is the latest
/// turn's catalogue, so a grant of `tests_run` publishes `tests_run` alone while the frontier lists
/// it, and the whole catalogue, `repository_merge` included, once the frontier blocks `tests.run`.
/// `AGENTS.md` § Rules: "Never silently broaden capability".
#[test]
fn adversary_w2p2_a_narrowing_is_not_widened_when_the_frontier_drops_the_granted_action() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [
            // The handed frontier, then turn 1's.
            answer(2, after_tests_actions()),
            answer(2, after_tests_actions()),
            // Turn 2's, and every read after it: the case moved on and `tests.run` is blocked.
            answer(3, tests_blocked_actions()),
        ],
    );
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);
    let granted = ToolName::new("tests_run").expect("valid");
    let (mut model, requests) = Scripted::new(vec![
        // Not granted: refused by the narrowing, so the run turns again.
        calls(vec![call("call_edit", "repository_edit", json!({}))]),
        calls(vec![call("call_merge", "repository_merge", json!({}))]),
        prose("NOTHING-GRANTED-IS-LEFT"),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(),
        LoopPorts {
            model: &mut model,
            config: config().with_admitted(Some(vec![granted.clone()])),
            sink: &mut sink,
        },
        &commission(&case),
        &handed,
    );

    assert!(
        published(3, &tests_blocked_actions()).contains(&"repository_merge".to_owned()),
        "precondition: turn 2's catalogue lists `repository.merge`"
    );
    let sent = requests.lock().expect("lock").clone();
    for (at, request) in sent.iter().enumerate() {
        let names = tool_names(request);
        assert!(
            names.iter().all(|name| name == granted.as_str()),
            "request {} publishes {names:?} to a run granted only `{granted}`; the run ended {:?}",
            at + 1,
            run.outcome
        );
    }
    assert!(
        !matches!(
            &run.outcome,
            ExecutorOutcome::ProposedAction(proposal) if proposal.action == MERGE
        ),
        "a run granted only `{granted}` proposed {:?}",
        run.outcome
    );
}

/// In the governed loop the model's call is the argument generator (`ModelArguments`), and its
/// arguments become Commission's JSON, whose reader refuses nesting past 64 levels
/// (`generated/rust/commission/src/json.rs`, `DEPTH`). The Responses wire decodes a function call's
/// arguments with serde_json, which admits 128 levels (`tests/json_depth.rs`), so a model can write
/// arguments the wire carries and Commission's reader refuses. What the model wrote is not an
/// unavailable dependency, the class `adversary_executor_admission.rs`
/// (`nothing_admissible_is_not_an_external_outage`) already holds Loom to: the run ends with no
/// proposal, `NoUsefulAction`, and is not retried (correction 2, B).
#[test]
fn adversary_w2p2_arguments_the_model_nests_too_deep_are_not_an_external_outage() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(2, after_tests_actions())]);
    let loom =
        Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let handed = issued(&governor, &case);

    let mut nested = json!("innermost");
    for _ in 0..70 {
        nested = json!([nested]);
    }
    let deep = json!({ "suite": nested });
    let encoded = deep.to_string();
    assert!(
        serde_json::from_str::<Value>(&encoded).is_ok(),
        "precondition: serde_json, the wire's decoder, carries these arguments"
    );
    assert!(
        commission_json::parse(&encoded).is_err(),
        "precondition: Commission's JSON reader refuses them"
    );

    let (mut model, _requests) = Scripted::new(vec![
        calls(vec![call("call_deep", "tests_run", deep)]),
        calls(vec![call(
            "call_flat",
            "tests_run",
            json!({"suite": "unit"}),
        )]),
        prose("DONE"),
    ]);
    let mut sink = VecLoopSink::new();
    let run = loom.run_loop(
        &session(),
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
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "arguments the model wrote that Commission cannot carry end the run with no proposal"
    );
}

/// `governed.rs:26-28`: "Each conversation turn the provider completes is recorded once into the
/// run's session". Two runs at once in one session would number their turns from the same count
/// and the second record would replace the first (`TurnStorage::put`). `loom.run.OpenSession`
/// answers `session-exists` for a session a run holds `Active` (`ess/domains/run.yaml`), so the
/// second run proposes nothing, sends no request and records no turn; the first run's turn is
/// recorded, and once both have ended the session is `Filed`, for a later run to resume
/// (correction 2, C). The first run's model holds its turn until the second run has returned, so
/// the second run meets the session `Active`.
#[test]
fn adversary_w2p2_a_second_run_in_an_active_session_sends_nothing_and_the_session_is_filed() {
    let case = CaseId(CASE.to_owned());
    let loom = Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, PROMPT);
    let handed = frontier(&case, 2, after_tests_actions());
    let commission = commission(&case);
    let held = Arc::new(Held::default());

    let (first, second, second_requests) = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let mut model = Holding::new(
                held.clone(),
                vec![calls(vec![call("call_a", "tests_run", json!({}))])],
            );
            let mut sink = VecLoopSink::new();
            loom.run_loop(
                &session(),
                LoopPorts {
                    model: &mut model,
                    config: config(),
                    sink: &mut sink,
                },
                &commission,
                &handed,
            )
            .outcome
        });
        assert!(
            held.wait(|state| state.entered),
            "the first run never reached its model turn"
        );
        let (mut model, requests) =
            Scripted::new(vec![calls(vec![call("call_b", "tests_run", json!({}))])]);
        let mut sink = VecLoopSink::new();
        let second = loom
            .run_loop(
                &session(),
                LoopPorts {
                    model: &mut model,
                    config: config(),
                    sink: &mut sink,
                },
                &commission,
                &handed,
            )
            .outcome;
        held.release();
        let first = first.join().expect("the first run does not panic");
        let sent = requests.lock().expect("lock").len();
        (first, second, sent)
    });

    assert_eq!(
        second,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "a run in a session another run holds proposes nothing"
    );
    assert_eq!(second_requests, 0, "the second run sent a request");
    let turns = loom.turns();
    assert_eq!(
        turns.len(),
        1,
        "only the first run's turn: {turns:?}; first {first:?}"
    );
    assert_eq!(turns[0].data.session_id, session().session_id);
    assert_eq!(turns[0].data.index, 1);
    let sessions = loom.sessions();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    assert_eq!(
        sessions[0].state,
        SessionState::Filed,
        "the session is filed once both runs ended"
    );
}

// --- the scripted models ------------------------------------------------------------------------

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

/// Whether the holding run is inside its model turn, and whether it may answer it.
#[derive(Default)]
struct Held {
    state: Mutex<HeldState>,
    changed: Condvar,
}

#[derive(Default)]
struct HeldState {
    entered: bool,
    released: bool,
}

impl Held {
    /// Waits, at most ten seconds, until `ready` holds; answers whether it does.
    fn wait(&self, ready: impl Fn(&HeldState) -> bool) -> bool {
        let state = self.state.lock().expect("lock");
        let (state, _) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !ready(state))
            .expect("lock");
        ready(&state)
    }

    fn update(&self, change: impl FnOnce(&mut HeldState)) {
        change(&mut self.state.lock().expect("lock"));
        self.changed.notify_all();
    }

    fn release(&self) {
        self.update(|state| state.released = true);
    }
}

/// A scripted model that, on its first turn, says it is inside it and answers only once released
/// (or after ten seconds).
struct Holding {
    wire: WireId,
    held: Arc<Held>,
    turns: VecDeque<TurnOutcome>,
}

impl Holding {
    fn new(held: Arc<Held>, turns: Vec<TurnOutcome>) -> Self {
        Self {
            wire: WireId::new(responses::WIRE).expect("valid"),
            held,
            turns: turns.into(),
        }
    }
}

impl ModelPort for Holding {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.held.update(|state| state.entered = true);
        self.held.wait(|state| state.released);
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
            "00000000-0000-4000-8000-0000000000e2".to_owned(),
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
    let id = || Uuid("00000000-0000-4000-8000-0000000000f2".to_owned());
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

fn session() -> SessionData {
    SessionData {
        session_id: SessionId(Uuid(SESSION.to_owned())),
        commission_run: CommissionRunId(Uuid(RUN.to_owned())),
        wire: responses::WIRE.to_owned(),
    }
}

fn config() -> LoopConfig {
    LoopConfig::new(MODEL, "INSTRUCTIONS-standing").with_retry_backoff(Duration::from_millis(1))
}
