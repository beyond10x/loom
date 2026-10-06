// SPDX-License-Identifier: Apache-2.0

//! Adversary cases for `story:interruption-recovery` (wave 2026-10-06-w4, pass 2), against
//! `Loom::interrupt` and `Loom::resume_loop` at b00e69f.
//!
//! The case is the one `tests/interruption_recovery.rs` uses: `CHG-1842`, whose frontier at
//! revision 1 lists `repository.inspect` (admissible) and `repository.merge` (approval required)
//! and at revision 2 also `tests.run`. Every model is scripted in this process; nothing is sent
//! over a network. Two cases use a second thread, as `Loom::interrupt` documents ("Callable from
//! the run's own sink and from another thread"); channels order the two threads, and every wait is
//! bounded.
//!
//! - An interrupted run whose session is resumed before the interrupted run has ended: the
//!   interrupt is acknowledged (`Interrupted`), yet the interrupted run proposes its merge, and
//!   when it ends late it files the session the resumed run holds, so the resumed run cannot be
//!   interrupted.
//! - A Loom without a governor revalidates a held selection in flight against nothing, so it
//!   proposes it on a case that moved, while the same Loom refuses an admitted held selection on
//!   the same move.
//! - A resumed run interrupted when its held call asks for approval still revalidates the
//!   selection in flight.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction,
    FrontierData, FrontierId, PrincipalId, ProposedActionArguments, Unit, commission_state,
    frontier_state,
};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::harness::governed::LoopPorts;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{LoopConfig, LoopEvent, LoopSink, VecLoopSink};
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
/// The longest one thread waits for the other; a wait that runs out lets the run carry on.
const WAIT: Duration = Duration::from_secs(20);

/// Run A calls `repository_merge`. The operator interrupts session S while run A is at the merge
/// call's approval, before the call is put to the pipeline, and `Loom::interrupt` answers
/// `Interrupted`. The operator then resumes S (`Loom::resume_loop`, "from `Filed` or
/// `Interrupted`"), and that run answers and ends. Only then does run A's thread get past the
/// approval request.
///
/// Run A was interrupted before its call reached the pipeline, so it proposes nothing:
/// `recovery.rs` "the run's loop is cancelled at its next check ... The run ends `NoUsefulAction`";
/// `ess/domains/run.yaml` `InterruptSession`: "nothing the run had in flight is proposed".
#[test]
fn an_interrupted_run_proposes_nothing_when_its_session_is_resumed_before_it_ends() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000b201");
    let commission = commission(&case);
    let frontier = issued(&governor, &case);
    let (loom, session, commission, frontier) = (&loom, &session, &commission, &frontier);

    let (at_gate, reached_gate) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (interrupted, resumed, first) = thread::scope(|scope| {
        let run_a = scope.spawn(move || {
            let (mut model, _) = Scripted::new(vec![calls(vec![call(
                "call_merge",
                "repository_merge",
                json!({"strategy": "squash"}),
            )])]);
            let mut sink = HoldAtApproval {
                tool: "repository_merge",
                reached: Some(at_gate),
                release: released,
            };
            loom.run_loop(
                session,
                ports(&mut model, config(), &mut sink),
                commission,
                frontier,
            )
        });
        reached_gate
            .recv_timeout(WAIT)
            .expect("run A reaches the approval of its merge call");
        let interrupted = loom.interrupt(&session.session_id);
        let (mut model, _) = Scripted::new(vec![prose("RESUMED-RUN-ANSWERS")]);
        let resumed = loom.resume_loop(
            &session.session_id,
            ports(&mut model, config(), &mut VecLoopSink::new()),
            commission,
            frontier,
        );
        let _ = release.send(());
        (
            interrupted,
            resumed,
            run_a.join().expect("run A does not panic"),
        )
    });

    assert!(
        matches!(interrupted, Ok(InterruptSessionOutcome::Interrupted { .. })),
        "precondition: run A's session was interrupted: {interrupted:?}"
    );
    assert_eq!(
        first.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "run A was interrupted before its merge call reached the pipeline, so it proposes nothing; \
         its session was resumed by another run before run A ended (that run answered {:?})",
        resumed.outcome
    );
}

/// Run A is blocked in its model read when the operator interrupts session S (`Loom::interrupt`
/// cancels the loop, not the read: the cancel it makes is never handed to the model port). The
/// operator resumes S at once: run C claims it. While run C is in its first model turn, run A's
/// read returns and run A ends. Run C still holds S, so S is `Active`, and the operator's interrupt
/// of S reaches run C, whose loop stops at its next check before its call reaches the pipeline
/// (`governed.rs`: "One run holds a session at a time"; `Loom::interrupt`: "Interrupts the run
/// holding `session`").
#[test]
fn a_run_resumed_while_the_interrupted_run_still_reads_its_model_can_be_interrupted() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000b202");
    let commission = commission(&case);
    let frontier = issued(&governor, &case);
    let (loom, session, commission, frontier) = (&loom, &session, &commission, &frontier);

    let (entered, in_read) = mpsc::channel();
    let (release_a, a_released) = mpsc::channel();
    let (a_done, a_ended) = mpsc::channel();
    let (interrupted, seen, first) = thread::scope(|scope| {
        let run_a = scope.spawn(move || {
            let mut model = BlockedRead {
                wire: wire(),
                entered: Some(entered),
                release: a_released,
            };
            let run = loom.run_loop(
                session,
                ports(&mut model, config(), &mut VecLoopSink::new()),
                commission,
                frontier,
            );
            let _ = a_done.send(());
            run
        });
        in_read
            .recv_timeout(WAIT)
            .expect("run A is in its model read");
        let interrupted = loom.interrupt(&session.session_id);
        let mut model = Observer {
            wire: wire(),
            loom,
            session: session.session_id.clone(),
            release_a: release_a.clone(),
            a_ended,
            seen: None,
        };
        let _ = loom.resume_loop(
            &session.session_id,
            ports(&mut model, config(), &mut VecLoopSink::new()),
            commission,
            frontier,
        );
        let _ = release_a.send(());
        let first = run_a.join().expect("run A does not panic");
        (interrupted, model.seen, first)
    });

    assert!(
        matches!(interrupted, Ok(InterruptSessionOutcome::Interrupted { .. })),
        "precondition: run A's session was interrupted: {interrupted:?}"
    );
    // A resume refused while the interrupted run is still running never asks its model; then there
    // is no run C to observe, and only run A's outcome below is asserted.
    if let Some(seen) = seen {
        assert!(
            matches!(seen.interrupt, Ok(InterruptSessionOutcome::Interrupted { .. })),
            "run C holds session S, so the operator's interrupt of S reaches it; S was {:?} after \
             the interrupted run A ended, and the interrupt answered {:?}",
            seen.state,
            seen.interrupt
        );
        assert_eq!(seen.state, Some(SessionState::Active), "run C holds S");
        assert_eq!(
            loom.selections(),
            [],
            "run C was cancelled at its next check, before its call reached the pipeline"
        );
    }
    assert_eq!(
        first.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "run A was interrupted, so it proposes nothing however it stopped"
    );
}

/// A Loom without a governor. Session B stops at its merge proposal, admitted; session A is
/// interrupted at its merge call's approval, so its merge selection is in flight (`Selected`).
/// Both are resumed on the frontier of revision 2, which still lists the merge. The admitted held
/// merge is refused as stale (selected at revision 1). The in-flight held merge, which was never
/// revalidated at all, is refused too: `recovery.rs` "a selection in flight is revalidated against
/// the case's current frontier"; `ess/domains/run.yaml` `ResumeSession`: "revalidates what the
/// interrupted run had in flight, before anything is proposed"; `AGENTS.md` § Rules: "Revalidate
/// every selected action against case revision".
#[test]
fn a_governorless_resume_never_proposes_an_in_flight_selection_on_a_moved_case() {
    let case = CaseId(CASE.to_owned());
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT);
    let commission = commission(&case);
    let at_revision_1 = || handed(1, "00000000-0000-4000-8000-0000000000e1", ready_actions());
    let at_revision_2 = || handed(2, "00000000-0000-4000-8000-0000000000e2", moved_actions());

    // Session B: the merge is proposed, then the case moves.
    let admitted = session("00000000-0000-4000-8000-00000000b2a3");
    let (mut model_b, _) = Scripted::new(vec![
        calls(vec![call(
            "call_merge",
            "repository_merge",
            json!({"strategy": "squash"}),
        )]),
        prose("NOTHING-LEFT-TO-PROPOSE"),
    ]);
    let proposed = loom.run_loop(
        &admitted,
        ports(&mut model_b, config(), &mut VecLoopSink::new()),
        &commission,
        &at_revision_1(),
    );
    assert_eq!(proposed.outcome, merge_proposal("squash"), "precondition");
    let refused = loom.resume_loop(
        &admitted.session_id,
        ports(&mut model_b, config(), &mut VecLoopSink::new()),
        &commission,
        &at_revision_2(),
    );
    assert_eq!(
        refused.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        "precondition: the admitted held merge is refused as stale on revision 2 ({:?})",
        refused.run
    );

    // Session A: the merge is selected when the run is interrupted, then the case moves.
    let in_flight = session("00000000-0000-4000-8000-00000000b2b3");
    let (mut model_a, _) = Scripted::new(vec![
        calls(vec![call(
            "call_merge",
            "repository_merge",
            json!({"strategy": "squash"}),
        )]),
        prose("NOTHING-LEFT-TO-PROPOSE"),
    ]);
    let mut at_merge = InterruptAt {
        loom: &loom,
        session: in_flight.session_id.clone(),
        tool: "repository_merge",
        interrupted: None,
    };
    let first = loom.run_loop(
        &in_flight,
        ports(&mut model_a, config(), &mut at_merge),
        &commission,
        &at_revision_1(),
    );
    assert!(
        matches!(
            at_merge.interrupted,
            Some(Ok(InterruptSessionOutcome::Interrupted { .. }))
        ),
        "precondition: {:?}",
        at_merge.interrupted
    );
    assert_eq!(first.outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    // Session A's selection is the last one made. Without a governor nothing moves a selection out
    // of `Selected`, session B's included.
    let selected = loom
        .selections()
        .pop()
        .expect("precondition: the interrupted run's merge selection is recorded");
    assert_eq!(
        (selected.data.action.as_str(), selected.data.case_revision),
        (MERGE, 1),
        "precondition"
    );

    let resumed = loom.resume_loop(
        &in_flight.session_id,
        ports(&mut model_a, config(), &mut VecLoopSink::new()),
        &commission,
        &at_revision_2(),
    );
    assert_ne!(
        resumed.outcome,
        merge_proposal("squash"),
        "the in-flight merge, selected at revision 1 and never revalidated, is proposed on \
         revision 2, where the same Loom refused the admitted held merge as stale"
    );
}

/// A run is interrupted at its merge call's approval: the selection is in flight. The session is
/// resumed, and the resumed run is interrupted in turn when its held merge call asks for approval,
/// before Loom answers it. The selection stays in flight, unrevalidated, for the next resume:
/// `recovery.rs` "A call that reached the selection pipeline before the loop saw the cancel is
/// selected and recorded, and stopped before it is revalidated: the selection stays `Selected`";
/// `Proposer::decide`: "A run interrupted while the call was being selected ends at that
/// checkpoint before revalidation".
#[test]
fn a_resumed_run_interrupted_at_its_held_call_leaves_the_selection_unrevalidated() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(FirstEntry, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000b204");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({"strategy": "squash"}),
    )])]);

    let mut at_merge = InterruptAt {
        loom: &loom,
        session: session.session_id.clone(),
        tool: "repository_merge",
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
        "precondition: {:?}",
        at_merge.interrupted
    );
    assert_eq!(first.outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));

    let mut at_held_merge = InterruptAt {
        loom: &loom,
        session: session.session_id.clone(),
        tool: "repository_merge",
        interrupted: None,
    };
    let resumed = loom.resume_loop(
        &session.session_id,
        ports(&mut model, config(), &mut at_held_merge),
        &commission,
        &issued(&governor, &case),
    );
    assert!(
        matches!(
            at_held_merge.interrupted,
            Some(Ok(InterruptSessionOutcome::Interrupted { .. }))
        ),
        "precondition: the resumed run is interrupted at its held call: {:?}",
        at_held_merge.interrupted
    );
    assert_eq!(
        resumed.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "precondition"
    );
    assert_eq!(requests.lock().expect("lock").len(), 1, "precondition");

    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "precondition: {selections:?}");
    assert_eq!(
        (selections[0].state, loom.revalidations().len()),
        (SelectionState::Selected, 0),
        "the resumed run was interrupted before Loom answered its held call, so the selection in \
         flight is not revalidated: {:?}",
        loom.revalidations()
    );
}

// --- the sinks ----------------------------------------------------------------------------------

/// Holds the run at the approval request of `tool` until released: the thread running it is
/// somewhere the operator's other thread can act before the run goes on.
struct HoldAtApproval {
    tool: &'static str,
    reached: Option<Sender<()>>,
    release: Receiver<()>,
}

impl LoopSink for HoldAtApproval {
    fn emit(&mut self, event: LoopEvent) {
        if let LoopEvent::ApprovalRequired { name, .. } = &event
            && name.as_str() == self.tool
            && let Some(reached) = self.reached.take()
        {
            let _ = reached.send(());
            let _ = self.release.recv_timeout(WAIT);
        }
    }
}

/// Interrupts `session` at the approval request of `tool`, once.
struct InterruptAt<'l, S, G, V> {
    loom: &'l Loom<S, G, V>,
    session: SessionId,
    tool: &'static str,
    interrupted: Option<Result<InterruptSessionOutcome, UnmetObligation>>,
}

impl<S, G, V> LoopSink for InterruptAt<'_, S, G, V> {
    fn emit(&mut self, event: LoopEvent) {
        if self.interrupted.is_none()
            && matches!(&event, LoopEvent::ApprovalRequired { name, .. } if name.as_str() == self.tool)
        {
            self.interrupted = Some(self.loom.interrupt(&self.session));
        }
    }
}

// --- the models ---------------------------------------------------------------------------------

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
            wire: wire(),
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

/// Run A's model: its first read blocks until released, then answers in prose.
struct BlockedRead {
    wire: WireId,
    entered: Option<Sender<()>>,
    release: Receiver<()>,
}

impl ModelPort for BlockedRead {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let Some(entered) = self.entered.take() else {
            return Err(WireError::protocol("run A has no further turn"));
        };
        let _ = entered.send(());
        let _ = self.release.recv_timeout(WAIT);
        Ok(prose("RUN-A-LATE-ANSWER"))
    }
}

/// What run C saw of its session once run A had ended, and what the operator's interrupt of it
/// answered.
struct Seen {
    state: Option<SessionState>,
    interrupt: Result<InterruptSessionOutcome, UnmetObligation>,
}

/// Run C's model: on its first turn it lets run A's read return, waits for run A to end, looks at
/// the session and interrupts it, then calls `repository_inspect`.
struct Observer<'l, S, G, V> {
    wire: WireId,
    loom: &'l Loom<S, G, V>,
    session: SessionId,
    release_a: Sender<()>,
    a_ended: Receiver<()>,
    seen: Option<Seen>,
}

impl<S, G, V> ModelPort for Observer<'_, S, G, V> {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        if self.seen.is_some() {
            return Ok(prose("RUN-C-DONE"));
        }
        let _ = self.release_a.send(());
        let _ = self.a_ended.recv_timeout(WAIT);
        self.seen = Some(Seen {
            state: session_state(self.loom, &self.session),
            interrupt: self.loom.interrupt(&self.session),
        });
        Ok(calls(vec![call(
            "call_inspect_c",
            "repository_inspect",
            json!({}),
        )]))
    }
}

fn wire() -> WireId {
    WireId::new(responses::WIRE).expect("valid")
}

fn ports<'p>(
    model: &'p mut dyn ModelPort,
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

fn prose(text: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant(text)],
        usage: None,
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

/// The frontier a run is handed, for a Loom with no governor to ask.
fn handed(revision: i64, id: &str, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(id.to_owned())),
        case_id: CaseId(CASE.to_owned()),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
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
