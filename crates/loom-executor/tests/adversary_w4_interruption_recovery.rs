// SPDX-License-Identifier: Apache-2.0

//! Adversary cases for `story:interruption-recovery` (wave 2026-10-06-w4, pass 1), against
//! `Loom::interrupt` and `Loom::resume_loop`.
//!
//! The case and the fakes are the ones `tests/interruption_recovery.rs` uses: `CHG-1842`, whose
//! frontier at revision 1 lists `repository.inspect` (admissible) and `repository.merge` (approval
//! required) and at revisions 2 and 3 also `tests.run`. The model is scripted in this process;
//! nothing is sent over a network.
//!
//! - Selection identity across the runs of one Loom: a resumed run that selects before it records
//!   a turn projects its catalogue under the id an earlier resumed run used, and its selection
//!   numbers restart at 0, so a later run gives a selection id an earlier run gave.
//! - A held checkpoint survives a resume that ends before it reaches the held call: a governor
//!   outage at the resume's first projection, or an interrupt of the resumed run.
//! - The held call is matched by call id alone: a call of another tool reusing its id resolves it,
//!   so a resume narrowed away from `repository.merge` proposes the merge.
//! - A resume under a configuration whose narrowing differs is not refused, though
//!   `Loom::resume_loop` says a checkpoint resumed under another configuration fails.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    PrincipalId, ProposedActionArguments, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::harness::governed::LoopPorts;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    LoopConfig, LoopError, LoopEvent, LoopSink, VecLoopSink,
};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::obligation::UnmetObligation;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueEntry, CommissionRunId, InterruptSessionOutcome, SelectionState, SelectionStrategy,
    SessionData, SessionId, SessionState,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};
use serde_json::{Value, json};

const CASE: &str = "CHG-1842";
const MODEL: &str = "scripted-model";
const PROMPT: &str = "PROMPT-land-the-change";
const RUN: &str = "00000000-0000-4000-8000-00000000d0ab";
const MERGE: &str = "repository.merge";
const INSPECT: &str = "repository.inspect";

/// A model turn calls `repository_merge` and `repository_inspect` side by side. The merge is
/// proposed and the run stops at its checkpoint with the inspect call still to run. The case moves
/// to revision 2 and the session is resumed: the held merge is stale and denied, and the inspect
/// call is selected and proposed — before the resumed run records any turn. The case moves to
/// revision 3 and the session is resumed again: the held inspect call is stale, the model calls
/// `repository_inspect` again, and that is a third selection.
///
/// Three selections were made, so three are recorded under three ids (`lib.rs`: "Within one
/// namespace, two runs, two scopes or two kinds get different ids"; `prepare`: "Every selection
/// made is recorded"). The second resume projects under the catalogue id the first resume used
/// (both are turn index 2: the first recorded no turn), and its selections are numbered from 0
/// again, so its selection is given the id of the first resume's.
#[test]
fn a_second_resume_never_gives_a_selection_id_an_earlier_run_gave() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT)
        .with_governor(&governor)
        .with_instance("adversary-w4");
    let session = session("00000000-0000-4000-8000-00000000a401");
    let commission = commission(&case);
    let (mut model, _requests) = Scripted::new(vec![
        calls(vec![
            call(
                "call_merge",
                "repository_merge",
                json!({"strategy": "squash"}),
            ),
            call("call_inspect", "repository_inspect", json!({})),
        ]),
        calls(vec![call(
            "call_inspect_again",
            "repository_inspect",
            json!({}),
        )]),
    ]);

    let first = loom.run_loop(
        &session,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        first.outcome,
        merge_proposal("squash"),
        "precondition: run 1"
    );

    governor.script(case.clone(), [answer(2, moved_actions())]);
    let second = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        second.outcome,
        inspect_proposal(),
        "precondition: the first resume proposes the inspect call the checkpoint still held"
    );

    governor.script(case.clone(), [answer(3, moved_actions())]);
    let third = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        third.outcome,
        inspect_proposal(),
        "precondition: the second resume proposes the model's new inspect call"
    );

    let selections = loom.selections();
    let ids: BTreeSet<String> = selections
        .iter()
        .map(|selection| selection.data.selection_id.0.0.clone())
        .collect();
    assert_eq!(
        (selections.len(), ids.len()),
        (3, 3),
        "three selections were made (merge at revision 1, inspect at 2, inspect at 3), so three \
         are recorded under three ids: {selections:#?}"
    );
}

/// A run stops at the merge approval and proposes it. The governor is unavailable once, at the
/// first resume's projection; the second resume finds it answering again, with the frontier
/// unchanged. The checkpoint is still Loom's to resume from, so the second resume returns the
/// merge proposal without asking the model again (acceptance 3; `Proposer::resolve`: "an outage
/// of the governor ends the run at the checkpoint, still held").
#[test]
fn a_governor_outage_at_resume_keeps_the_checkpoint_for_the_next_resume() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000a402");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({"strategy": "squash"}),
    )])]);

    let first = loom.run_loop(
        &session,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        first.outcome,
        merge_proposal("squash"),
        "precondition: run 1"
    );
    let asked_at_suspension = requests.lock().expect("lock").len();

    let handed = issued(&governor, &case);
    governor.script(
        case.clone(),
        [Answer::unavailable(), answer(1, ready_actions())],
    );
    let outage = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &handed,
    );
    assert!(
        matches!(outage.outcome, ExecutorOutcome::Suspended(_)),
        "precondition: the first resume meets the outage: {:?}",
        outage.outcome
    );

    let resumed = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        resumed.outcome,
        merge_proposal("squash"),
        "the checkpoint outlives a governor outage at resume: the next resume returns the merge \
         proposal (run: {:?})",
        resumed.run
    );
    assert_eq!(
        requests.lock().expect("lock").len(),
        asked_at_suspension,
        "the model is not asked again between the suspension and the return"
    );
}

/// A run is interrupted while its merge call is selected: the selection is in flight. The session
/// is resumed, and the resumed run is interrupted in turn while it projects its first catalogue,
/// before it reaches the held call. Resumed again on the unchanged frontier, the session continues
/// from the checkpoint: the in-flight selection is revalidated and proposed, and the model is not
/// asked again (`ess/domains/run.yaml` `ResumeSession`: an interrupted session's resume
/// "revalidates what the interrupted run had in flight, before anything is proposed";
/// `governed_run`: an interrupted run's "checkpoint is held for the run that resumes the session").
#[test]
fn a_resumed_run_interrupted_before_its_gate_keeps_the_checkpoint() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000a403");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({"strategy": "squash"}),
    )])]);

    let mut at_merge = InterruptOn {
        loom: &loom,
        session: session.session_id.clone(),
        when: |event| matches!(event, LoopEvent::ApprovalRequired { name, .. } if name.as_str() == "repository_merge"),
        interrupted: None,
    };
    let first = loom.run_loop(
        &session,
        ports(&mut model, config(), &mut at_merge),
        &commission,
        &issued(&governor, &case),
    );
    assert!(
        matches!(
            at_merge.interrupted,
            Some(Ok(InterruptSessionOutcome::Interrupted { .. }))
        ),
        "precondition: run 1 is interrupted: {:?}",
        at_merge.interrupted
    );
    assert_eq!(first.outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "precondition: {selections:?}");
    assert_eq!(
        selections[0].state,
        SelectionState::Selected,
        "precondition"
    );
    let in_flight = selections[0].data.selection_id.clone();
    let asked_at_interruption = requests.lock().expect("lock").len();

    let mut at_projection = InterruptOn {
        loom: &loom,
        session: session.session_id.clone(),
        when: |event| matches!(event, LoopEvent::InventoryChanged { .. }),
        interrupted: None,
    };
    let interrupted_again = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut at_projection),
        &commission,
        &issued(&governor, &case),
    );
    assert!(
        matches!(
            at_projection.interrupted,
            Some(Ok(InterruptSessionOutcome::Interrupted { .. }))
        ),
        "precondition: the first resume is interrupted: {:?}",
        at_projection.interrupted
    );
    assert_eq!(
        interrupted_again.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true))
    );
    assert_eq!(
        session_state(&loom, &session.session_id),
        Some(SessionState::Interrupted)
    );

    let resumed = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    let held = loom
        .selections()
        .into_iter()
        .find(|selection| selection.data.selection_id == in_flight)
        .expect("the in-flight selection is recorded");
    assert_eq!(
        (resumed.outcome, held.state),
        (merge_proposal("squash"), SelectionState::Admitted),
        "the selection run 1 had in flight is revalidated and proposed by the resume after the \
         second interrupt (run: {:?})",
        resumed.run
    );
    assert_eq!(
        requests.lock().expect("lock").len(),
        asked_at_interruption,
        "the model is not asked again"
    );
}

/// A run stops at the merge approval. The operator resumes the session narrowed to
/// `repository_inspect`, so the resumed run does not publish `repository_merge` and the loop
/// refuses the held merge call before Loom is asked about it. The model then calls
/// `repository_inspect` under the call id of the held merge call. That call is an inspect call: a
/// resume narrowed away from the merge never proposes the merge (AGENTS.md § Rules: the model never
/// supplies authority; never silently broaden capability).
#[test]
fn a_resume_narrowed_away_from_the_merge_never_proposes_it_for_a_call_reusing_its_id() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000a404");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![call(
            "call_merge",
            "repository_merge",
            json!({"strategy": "squash"}),
        )]),
        calls(vec![call("call_merge", "repository_inspect", json!({}))]),
    ]);

    let first = loom.run_loop(
        &session,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        first.outcome,
        merge_proposal("squash"),
        "precondition: run 1"
    );

    let narrowed = config().with_admitted(Some(vec![
        ToolName::new("repository_inspect").expect("valid"),
    ]));
    let resumed = loom.resume_loop(
        &session.session_id,
        ports(&mut model, narrowed, &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    let sent = requests.lock().expect("lock").clone();
    assert!(
        sent.len() == 2 && !tool_names(&sent[1]).contains(&"repository_merge".to_owned()),
        "precondition: the resumed run asked the model once, without publishing the merge: {:?}",
        sent.iter().map(tool_names).collect::<Vec<_>>()
    );
    assert!(
        !matches!(
            &resumed.outcome,
            ExecutorOutcome::ProposedAction(proposed) if proposed.action == MERGE
        ),
        "a resume narrowed to repository_inspect proposed {:?} for the model's call of \
         repository_inspect that reused the held merge call's id",
        resumed.outcome
    );
}

/// A run narrowed to `repository_inspect` stops at the checkpoint of its inspect call. The session
/// is resumed under the same configuration without the narrowing. `Loom::resume_loop`: "`ports.config`
/// must be the configuration the run was started with: a checkpoint resumed under another one
/// fails, the session is filed as failed, and no checkpoint is held after it".
#[test]
fn a_resume_under_another_narrowing_fails_as_a_changed_configuration() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000a405");
    let commission = commission(&case);
    let (mut model, _requests) = Scripted::new(vec![calls(vec![call(
        "call_inspect",
        "repository_inspect",
        json!({}),
    )])]);
    let narrowed = || {
        config().with_admitted(Some(vec![
            ToolName::new("repository_inspect").expect("valid"),
        ]))
    };

    let first = loom.run_loop(
        &session,
        ports(&mut model, narrowed(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(first.outcome, inspect_proposal(), "precondition: run 1");

    let resumed = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut VecLoopSink::new()),
        &commission,
        &issued(&governor, &case),
    );
    assert!(
        matches!(resumed.run, Some(Err(LoopError::Config(_)))),
        "a checkpoint resumed under a configuration with another narrowing fails; it answered \
         {:?} with {:?}",
        resumed.outcome,
        resumed
            .run
            .as_ref()
            .map(|run| run.as_ref().map(|outcome| &outcome.stop))
    );
}

// --- the interrupting sink ----------------------------------------------------------------------

/// The caller's sink: interrupts `session` on the first event `when` picks.
struct InterruptOn<'l, S, G, V> {
    loom: &'l Loom<S, G, V>,
    session: SessionId,
    when: fn(&LoopEvent) -> bool,
    interrupted: Option<Result<InterruptSessionOutcome, UnmetObligation>>,
}

impl<S, G, V> LoopSink for InterruptOn<'_, S, G, V> {
    fn emit(&mut self, event: LoopEvent) {
        if self.interrupted.is_none() && (self.when)(&event) {
            self.interrupted = Some(self.loom.interrupt(&self.session));
        }
    }
}

// --- Loom's own selector, never asked in a governed run -----------------------------------------

struct FirstEntry;

impl ActionSelector for FirstEntry {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        candidates
            .first()
            .map(|entry| Choice {
                action: entry.action.clone(),
                confidence: None,
            })
            .ok_or(SelectorError::NothingAdmissible)
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::Rule
    }
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

fn ports<'p>(
    model: &'p mut Scripted,
    config: LoopConfig,
    sink: &'p mut dyn LoopSink,
) -> LoopPorts<'p> {
    LoopPorts {
        model,
        config,
        sink,
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

fn ready_actions() -> Vec<FrontierAction> {
    vec![
        action(INSPECT, ActionStatus::Admissible, None),
        action(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]
}

fn moved_actions() -> Vec<FrontierAction> {
    vec![
        action(INSPECT, ActionStatus::Admissible, None),
        action("tests.run", ActionStatus::Admissible, None),
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

fn issued(governor: &FakeGovernor, case: &CaseId) -> Frontier<frontier_state::Issued> {
    governor
        .frontier(case)
        .unwrap_or_else(|error| panic!("frontier for {} failed: {error:?}", case.0))
}

fn merge_proposal(strategy: &str) -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: MERGE.to_owned(),
        arguments: ProposedActionArguments(CommissionValue::Object(vec![(
            "strategy".to_owned(),
            CommissionValue::Text(strategy.to_owned()),
        )])),
    })
}

fn inspect_proposal() -> ExecutorOutcome {
    ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action: INSPECT.to_owned(),
        arguments: ProposedActionArguments(CommissionValue::Object(Vec::new())),
    })
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

fn session(id: &str) -> SessionData {
    SessionData {
        session_id: SessionId(Uuid(id.to_owned())),
        commission_run: CommissionRunId(Uuid(RUN.to_owned())),
        wire: responses::WIRE.to_owned(),
    }
}

fn session_state<S, G, V>(loom: &Loom<S, G, V>, session: &SessionId) -> Option<SessionState> {
    loom.sessions()
        .into_iter()
        .find(|held| &held.data.session_id == session)
        .map(|held| held.state)
}

fn config() -> LoopConfig {
    LoopConfig::new(MODEL, "INSTRUCTIONS-standing").with_retry_backoff(Duration::from_millis(1))
}
