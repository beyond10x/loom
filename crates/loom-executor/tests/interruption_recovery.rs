// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:interruption-recovery`: a governed run (`Loom::run_loop`) interrupted or
//! stopped at its approval checkpoint, and resumed by session id (`Loom::resume_loop`), with the
//! Commission fake governor moving the case and the Commission fake authority provider granting
//! the merge approval. The model is scripted in this process; nothing is sent over a network.
//!
//! The case is `CHG-1842`. At revision 1 the frontier lists `repository.inspect` (admissible) and
//! `repository.merge` (approval required, `repository.write`); at revision 2 it also lists
//! `tests.run`, so a catalogue projected from revision 2 publishes a tool list no revision-1
//! catalogue has.
//!
//! 1. A run cancelled after a selection, and resumed after the case moved, projects its first
//!    catalogue at the new revision.
//! 2. That resumed run proposes nothing for the selection made before the interruption, and the
//!    revalidation refusal names the stale revision.
//! 3. A run stopped at the merge approval, resumed once the fakes grant the approval with the
//!    frontier unchanged, returns the `ProposedAction` for `repository.merge` without the selector
//!    (the model, and Loom's own) being asked again.
//! 4. A run stopped at the merge approval, resumed after the case moved, projects at the new
//!    revision before it returns anything.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, ProposedActionArguments, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::authority::check_authority;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::harness::governed::{LoopPorts, tool_name};
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    LoopConfig, LoopError, LoopEvent, LoopSink, VecLoopSink,
};
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{
    CatalogueEntry, CatalogueId, CommissionRunId, InterruptSessionOutcome,
    RevalidateSelectionOutcome, SelectionStale, SelectionState, SelectionStrategy, SessionData,
    SessionId, SessionState, TurnId,
};
use b10x_loom_executor::projection::project;
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, EmptyObjectArguments, Loom, SelectorError};
use serde_json::{Value, json};

const CASE: &str = "CHG-1842";
const MODEL: &str = "scripted-model";
const PROMPT: &str = "PROMPT-land-the-change";
const RUN: &str = "00000000-0000-4000-8000-00000000d0ab";
const MERGE: &str = "repository.merge";
const WRITE: &str = "repository.write";

#[test]
fn interruption_recovery() {
    cancelled_after_a_selection_and_resumed_on_a_moved_case();
    suspended_at_the_merge_approval_and_resumed_once_granted();
    suspended_at_the_merge_approval_and_resumed_on_a_moved_case();
}

/// Acceptance 1 and 2. The operator cancels the run when the model's call of `repository_merge`,
/// the model's selection, asks for its decision. The selection is recorded and the cancel takes
/// effect before it is revalidated, so it never leaves Loom: it is in flight.
fn cancelled_after_a_selection_and_resumed_on_a_moved_case() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let selector = Counting::default();
    let loom = Loom::new(&selector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000d001");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![call(
            "call_merge",
            "repository_merge",
            json!({"strategy": "squash"}),
        )]),
        prose("NOTHING-LEFT-TO-PROPOSE"),
    ]);

    let mut interrupting = InterruptAt {
        loom: &loom,
        session: session.session_id.clone(),
        at: "repository_merge",
        interrupted: None,
    };
    let first = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut interrupting,
        },
        &commission,
        &issued(&governor, &case),
    );
    assert!(
        matches!(
            interrupting.interrupted,
            Some(Ok(InterruptSessionOutcome::Interrupted { .. }))
        ),
        "the interrupt was taken: {:?}",
        interrupting.interrupted
    );
    assert_eq!(
        first.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "an interrupted run proposes nothing"
    );
    assert_eq!(
        session_state(&loom, &session.session_id),
        Some(SessionState::Interrupted)
    );
    let selections = loom.selections();
    assert_eq!(selections.len(), 1, "{selections:?}");
    let in_flight = selections[0].data.selection_id.clone();
    assert_eq!(selections[0].data.action, MERGE);
    assert_eq!(selections[0].data.case_revision, 1);
    assert_eq!(
        selections[0].state,
        SelectionState::Selected,
        "the selection is in flight: not revalidated, not proposed"
    );
    assert_eq!(loom.revalidations(), [], "nothing was revalidated");

    // The case moves while the session is interrupted.
    governor.script(case.clone(), [answer(2, moved_actions())]);
    let projected_before = loom.catalogues().len();
    let mut sink = VecLoopSink::new();
    let resumed = loom.resume_loop(
        &session.session_id,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut sink,
        },
        &commission,
        &issued(&governor, &case),
    );

    // 1. The first catalogue the resumed run projects is at the new revision.
    let catalogues = loom.catalogues();
    let first_after = catalogues
        .get(projected_before)
        .unwrap_or_else(|| panic!("the resumed run projected no catalogue: {catalogues:?}"));
    assert_eq!(
        first_after.data.case_revision, 2,
        "1. the resumed run's first catalogue: {first_after:?}"
    );
    let sent = requests.lock().expect("lock").clone();
    assert_eq!(
        sent.len(),
        2,
        "one request before the interruption, one after"
    );
    assert_eq!(
        tool_names(&sent[1]),
        published(2, &moved_actions()),
        "1. the resumed run offers the catalogue of revision 2"
    );

    // 2. No proposal for the selection made before the interruption; its revalidation refusal
    // names the revision it was made at and the current one.
    assert_eq!(
        resumed.outcome,
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
        "2. the resumed run proposes nothing for the pre-interruption selection"
    );
    assert_eq!(
        loom.revalidations(),
        [RevalidateSelectionOutcome::StaleRevision {
            selection_stale: SelectionStale {
                selection_id: in_flight.clone(),
                catalogue_revision: 1,
                case_revision: 2,
            },
        }],
        "2. the refusal names the stale revision"
    );
    let refused = loom
        .selections()
        .into_iter()
        .find(|selection| selection.data.selection_id == in_flight)
        .expect("the selection is still recorded");
    assert_eq!(refused.state, SelectionState::Refused);
    let told = result_of(&sent[1], "call_merge");
    assert!(
        told.contains("case revision 1") && told.contains("revision 2"),
        "2. the model is told the selection is stale, naming both revisions: {told}"
    );
    assert_eq!(selector.asked(), 0, "Loom's own selector is never asked");
    assert_eq!(
        session_state(&loom, &session.session_id),
        Some(SessionState::Filed)
    );
}

/// Acceptance 3. The run stops at the checkpoint of its merge call and proposes it; Commission's
/// authority provider has not granted `repository.write` yet. Once the fake grants it, the run is
/// resumed by session id on the unchanged frontier.
fn suspended_at_the_merge_approval_and_resumed_once_granted() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let selector = Counting::default();
    let loom = Loom::new(&selector, EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000d003");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({"strategy": "squash"}),
    )])]);

    let first = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );
    let merge = merge_proposal("squash");
    assert_eq!(first.outcome, merge, "the run stops at the merge approval");
    let pending = StaticAuthorityProvider::new().answer(
        WRITE,
        AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
            request: "approve the merge of CHG-1842".to_owned(),
        }),
    );
    assert!(
        !check_authority(&pending, &commission, WRITE).allows(),
        "the merge awaits its approval"
    );
    let requests_at_suspension = requests.lock().expect("lock").len();
    let selections_at_suspension = loom.selections();
    let argument_requests_at_suspension = loom.argument_requests();
    assert_eq!(selections_at_suspension.len(), 1);

    // The fakes grant the approval; the frontier is otherwise unchanged.
    let granted = StaticAuthorityProvider::new().answer(WRITE, AuthorityVerdict::Allow(Unit(true)));
    let resumed = loom.resume_loop(
        &session.session_id,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );

    assert_eq!(
        resumed.outcome, merge,
        "3. the resumed run returns the merge proposal"
    );
    assert!(check_authority(&granted, &commission, WRITE).allows());
    assert_eq!(
        requests.lock().expect("lock").len(),
        requests_at_suspension,
        "3. the model, which selects in the governed loop, is not asked again"
    );
    assert_eq!(
        loom.selections(),
        selections_at_suspension,
        "3. no selection is made between the suspension and the return"
    );
    assert_eq!(loom.argument_requests(), argument_requests_at_suspension);
    assert_eq!(selector.asked(), 0, "3. Loom's own selector is never asked");
}

/// Acceptance 4. As 3, but the case moves to revision 2 before the resume. The held merge call was
/// selected at revision 1 and is not proposed; the model chooses again from the catalogue of
/// revision 2, and only that selection is proposed.
fn suspended_at_the_merge_approval_and_resumed_on_a_moved_case() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom =
        Loom::new(Counting::default(), EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000d004");
    let commission = commission(&case);
    let (mut model, requests) = Scripted::new(vec![
        calls(vec![call(
            "call_merge",
            "repository_merge",
            json!({"strategy": "squash"}),
        )]),
        calls(vec![call(
            "call_merge_again",
            "repository_merge",
            json!({"strategy": "rebase"}),
        )]),
    ]);

    let first = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(first.outcome, merge_proposal("squash"));
    let held = loom.selections();
    assert_eq!(held.len(), 1, "{held:?}");

    governor.script(case.clone(), [answer(2, moved_actions())]);
    let projected_before = loom.catalogues().len();
    let resumed = loom.resume_loop(
        &session.session_id,
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );

    let catalogues = loom.catalogues();
    let first_after = catalogues
        .get(projected_before)
        .unwrap_or_else(|| panic!("the resumed run projected no catalogue: {catalogues:?}"));
    assert_eq!(
        first_after.data.case_revision, 2,
        "4. the resumed run projects at the new revision first: {first_after:?}"
    );
    assert_eq!(
        resumed.outcome,
        merge_proposal("rebase"),
        "4. only the selection made on the revision-2 catalogue is proposed"
    );
    let selections = loom.selections();
    assert_eq!(selections.len(), 2, "{selections:?}");
    assert_eq!(selections[0], held[0], "the held selection is not touched");
    assert_eq!(selections[1].data.case_revision, 2);
    assert_eq!(
        selections[1].data.catalogue_id, first_after.data.catalogue_id,
        "4. the proposal was selected from the catalogue projected at resume"
    );
    assert_eq!(
        selections[1].data.strategy,
        SelectionStrategy::ReasoningModel
    );
    assert_eq!(selections[1].state, SelectionState::Admitted);
    let sent = requests.lock().expect("lock").clone();
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert_eq!(tool_names(&sent[1]), published(2, &moved_actions()));
    let told = result_of(&sent[1], "call_merge");
    assert!(
        told.contains("case revision 1") && told.contains("revision 2"),
        "4. the model is told the held call is stale, naming both revisions: {told}"
    );
}

// --- correction round 1: the classes behind the adversary's findings ----------------------------

/// Every id the governed loop numbers is numbered across the runs of one Loom: the catalogues,
/// selections and argument requests of a run and of two resumes after it, the resumes recording no
/// turn before they select, are each recorded under ids no other one has.
#[test]
fn no_catalogue_selection_or_argument_request_id_repeats_across_resumed_runs() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom = Loom::new(Counting::default(), EmptyObjectArguments, PROMPT)
        .with_governor(&governor)
        .with_instance("class-ids");
    let session = session("00000000-0000-4000-8000-00000000d101");
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
        LoopPorts {
            model: &mut model,
            config: config(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(first.outcome, merge_proposal("squash"), "precondition");
    for revision in [2, 3] {
        governor.script(case.clone(), [answer(revision, moved_actions())]);
        let resumed = loom.resume_loop(
            &session.session_id,
            LoopPorts {
                model: &mut model,
                config: config(),
                sink: &mut VecLoopSink::new(),
            },
            &commission,
            &issued(&governor, &case),
        );
        assert!(
            matches!(&resumed.outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == "repository.inspect"),
            "precondition, resume at revision {revision}: {:?}",
            resumed.outcome
        );
    }

    let distinct = |ids: Vec<String>| {
        let count = ids.len();
        let unique: std::collections::BTreeSet<String> = ids.into_iter().collect();
        (count, unique.len())
    };
    let catalogues = loom.catalogues();
    assert_eq!(catalogues.len(), 3, "{catalogues:?}");
    let (count, unique) = distinct(
        catalogues
            .iter()
            .map(|catalogue| catalogue.data.catalogue_id.0.0.clone())
            .collect(),
    );
    assert_eq!(count, unique, "catalogue ids repeat: {catalogues:#?}");
    let selections = loom.selections();
    assert_eq!(selections.len(), 3, "{selections:?}");
    let (count, unique) = distinct(
        selections
            .iter()
            .map(|selection| selection.data.selection_id.0.0.clone())
            .collect(),
    );
    assert_eq!(count, unique, "selection ids repeat: {selections:#?}");
    let requests = loom.argument_requests();
    assert_eq!(requests.len(), 3, "{requests:?}");
    let (count, unique) = distinct(
        requests
            .iter()
            .map(|request| request.data.argument_request_id.0.0.clone())
            .collect(),
    );
    assert_eq!(count, unique, "argument-request ids repeat: {requests:#?}");
}

/// A resume refused as a changed configuration, under a narrowing wider than the stopped run's or
/// for a commission of another case, keeps the checkpoint: the resume after it, as the run was
/// started, returns the held proposal without asking the model.
#[test]
fn a_refused_resume_keeps_the_checkpoint_for_the_next() {
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [answer(1, ready_actions())]);
    let loom =
        Loom::new(Counting::default(), EmptyObjectArguments, PROMPT).with_governor(&governor);
    let session = session("00000000-0000-4000-8000-00000000d102");
    let commission = commission(&case);
    let narrowed = || config().with_admitted(Some(vec![tool("repository_merge")]));
    let (mut model, requests) = Scripted::new(vec![calls(vec![call(
        "call_merge",
        "repository_merge",
        json!({"strategy": "squash"}),
    )])]);
    let first = loom.run_loop(
        &session,
        LoopPorts {
            model: &mut model,
            config: narrowed(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(first.outcome, merge_proposal("squash"), "precondition");

    let other_case = CaseId("CHG-0001".to_owned());
    let other_governor = FakeGovernor::new();
    other_governor.script(other_case.clone(), [answer(1, ready_actions())]);
    for (what, config, commission, frontier) in [
        (
            "a narrowing that also admits repository_inspect",
            config().with_admitted(Some(vec![
                tool("repository_merge"),
                tool("repository_inspect"),
            ])),
            self::commission(&case),
            issued(&governor, &case),
        ),
        (
            "no narrowing",
            config(),
            self::commission(&case),
            issued(&governor, &case),
        ),
        (
            "a commission of another case",
            narrowed(),
            self::commission(&other_case),
            issued(&other_governor, &other_case),
        ),
    ] {
        let refused = loom.resume_loop(
            &session.session_id,
            LoopPorts {
                model: &mut model,
                config,
                sink: &mut VecLoopSink::new(),
            },
            &commission,
            &frontier,
        );
        assert!(
            matches!(refused.run, Some(Err(LoopError::Config(_)))),
            "a resume under {what} fails as a changed configuration: {:?}",
            refused.run
        );
    }

    let resumed = loom.resume_loop(
        &session.session_id,
        LoopPorts {
            model: &mut model,
            config: narrowed(),
            sink: &mut VecLoopSink::new(),
        },
        &commission,
        &issued(&governor, &case),
    );
    assert_eq!(
        resumed.outcome,
        merge_proposal("squash"),
        "the checkpoint outlives the refused resumes: {:?}",
        resumed.run
    );
    assert_eq!(
        requests.lock().expect("lock").len(),
        1,
        "the model is asked once"
    );
}

fn tool(name: &str) -> ToolName {
    ToolName::new(name).expect("valid")
}

// --- the interrupting sink ----------------------------------------------------------------------

/// The caller's sink: interrupts `session` on the approval request for the tool `at`, once.
struct InterruptAt<'l, S, G, V> {
    loom: &'l Loom<S, G, V>,
    session: SessionId,
    at: &'static str,
    interrupted: Option<
        Result<InterruptSessionOutcome, b10x_loom_executor::model::obligation::UnmetObligation>,
    >,
}

impl<S, G, V> LoopSink for InterruptAt<'_, S, G, V> {
    fn emit(&mut self, event: LoopEvent) {
        if self.interrupted.is_none()
            && matches!(&event, LoopEvent::ApprovalRequired { name, .. } if name.as_str() == self.at)
        {
            self.interrupted = Some(self.loom.interrupt(&self.session));
        }
    }
}

// --- the selector Loom is built with ------------------------------------------------------------

/// Loom's own selector, counting how often it is asked. The governed loop selects with the model's
/// call, so it is never asked there.
#[derive(Default)]
struct Counting {
    asked: AtomicUsize,
}

impl Counting {
    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl ActionSelector for Counting {
    fn select(
        &self,
        _context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
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

impl ActionSelector for &Counting {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        (**self).select(context, candidates)
    }

    fn strategy(&self) -> SelectionStrategy {
        (**self).strategy()
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

/// The failed result the conversation carries for `call_id`, as text.
fn result_of(request: &TurnRequest, call_id: &str) -> String {
    request
        .items
        .iter()
        .find_map(|item| match item {
            Item::ToolResult {
                call_id: id,
                output,
                failed: true,
            } if id.as_str() == call_id => Some(output.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no failed result for `{call_id}` in {:?}", request.items))
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

/// Revision 1: the change is ready and its merge needs approval.
fn ready_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action(MERGE, ActionStatus::ApprovalRequired, Some(WRITE)),
    ]
}

/// Revision 2: somebody pushed; the tests may run again and the merge still needs approval.
fn moved_actions() -> Vec<FrontierAction> {
    vec![
        action("repository.inspect", ActionStatus::Admissible, None),
        action("tests.run", ActionStatus::Admissible, None),
        action(MERGE, ActionStatus::ApprovalRequired, Some(WRITE)),
    ]
}

fn answer(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

/// The tool names of the catalogue projected from a frontier at `revision` listing `actions`.
fn published(revision: i64, actions: &[FrontierAction]) -> Vec<String> {
    let id = || Uuid("00000000-0000-4000-8000-0000000000f4".to_owned());
    let frontier = Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000e4".to_owned(),
        )),
        case_id: CaseId(CASE.to_owned()),
        case_revision: revision,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: actions.to_vec(),
    });
    project(&frontier, CatalogueId(id()), TurnId(id()))
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
