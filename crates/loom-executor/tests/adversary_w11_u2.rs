// SPDX-License-Identifier: Apache-2.0

//! Adversary pass, wave 2026-10-14-w2, U2 (`story:dw-workflow-definition-executor`): attempts to
//! break the governance rules of `AGENTS.md` § Rules for `WorkflowExecutor`. Scripted governor and
//! scripted model only; nothing reaches a network.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictDeny, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, EffectOutcome, EffectOutcomePerformed, Frontier, FrontierAction,
    FrontierData, FrontierId, GovernorError, Observation, ObservationId, PrincipalId, RunId,
    RunOutcome, Unit, commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::LoopConfig;
use b10x_loom_executor::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName, TurnOutcome, TurnRequest,
    WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, SessionData, SessionId};
use b10x_loom_executor::workflow::{self, DefinitionError, RecordingEffects, WorkflowExecutor};
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};
use serde_json::json;

const CASE: &str = "CASE-ADV-W11-U2";
const INSPECT: &str = "repository.inspect";
const WRITE: &str = "tests.write";
const EDIT: &str = "implementation.edit";
const RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";
const PROTOCOL_ACTIONS: [&str; 5] = [INSPECT, WRITE, EDIT, RUN, MERGE];

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

/// A scripted case. Every action in `PROTOCOL_ACTIONS` but the merge is listed admissible unless
/// blocked; `approval` actions are listed approval-required under their own capability. When
/// `block_on_observe` names an action, the first observation the runtime reports blocks it.
struct Case {
    revision: AtomicI64,
    performed: Mutex<Vec<String>>,
    blocked: Mutex<Vec<String>>,
    approval: Vec<&'static str>,
    block_on_observe: Option<&'static str>,
    observed: AtomicBool,
}

impl Case {
    fn new(
        blocked: &[&str],
        approval: Vec<&'static str>,
        block_on_observe: Option<&'static str>,
    ) -> Self {
        Self {
            revision: AtomicI64::new(1),
            performed: Mutex::new(Vec::new()),
            blocked: Mutex::new(blocked.iter().map(|a| (*a).to_owned()).collect()),
            approval,
            block_on_observe,
            observed: AtomicBool::new(false),
        }
    }

    fn performed(&self) -> Vec<String> {
        self.performed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision.load(Ordering::SeqCst))
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision.load(Ordering::SeqCst);
        let blocked = self
            .blocked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let actions = [INSPECT, WRITE, EDIT, RUN]
            .into_iter()
            .map(|action| {
                let is_blocked = blocked.iter().any(|b| b == action);
                let approval = self.approval.contains(&action);
                FrontierAction {
                    action: action.to_owned(),
                    status: if is_blocked {
                        ActionStatus::Blocked
                    } else if approval {
                        ActionStatus::ApprovalRequired
                    } else {
                        ActionStatus::Admissible
                    },
                    capability: approval.then(|| action.to_owned()),
                    reasons: if is_blocked {
                        vec![format!("{action} is blocked by the adversary")]
                    } else {
                        Vec::new()
                    },
                }
            })
            .collect();
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
        if let Some(action) = self.block_on_observe
            && !self.observed.swap(true, Ordering::SeqCst)
        {
            self.blocked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(action.to_owned());
        }
        Ok(())
    }
}

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
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.case
            .performed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.data().action.clone());
        self.case.revision.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: CommissionValue::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

struct Model {
    wire: WireId,
    turns: VecDeque<TurnOutcome>,
    tools: Vec<Vec<String>>,
}

impl ModelPort for Model {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.tools.push(
            request
                .tools
                .iter()
                .map(|tool| tool.name.to_string())
                .collect(),
        );
        self.turns
            .pop_front()
            .ok_or_else(|| WireError::protocol("the script has no further turn"))
    }
}

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
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000a012".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000a0ab".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    }
}

struct Ran {
    end: LoopEnd,
    tools: Vec<Vec<String>>,
    performed: Vec<String>,
}

impl Ran {
    fn proposed(&self) -> Vec<String> {
        self.end
            .requests
            .iter()
            .map(|made| made.request.action.clone())
            .collect()
    }
}

fn run(
    text: &str,
    case: &Case,
    config: LoopConfig,
    authority: &StaticAuthorityProvider,
    turns: Vec<TurnOutcome>,
) -> Ran {
    let effects = RecordingEffects::new(CaseEffects { case });
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "change the code",
    )
    .with_instance("adversary-w11-u2")
    .with_governor(case);
    let definition =
        workflow::load(text, &protocol).unwrap_or_else(|error| panic!("refused: {error}"));
    let executor = WorkflowExecutor::new(
        &loom,
        Model {
            wire: WireId::new(responses::WIRE).expect("valid"),
            turns: turns.into(),
            tools: Vec::new(),
        },
        config,
        session(),
        definition,
        effects.log(),
    );
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let end = run_until_blocked(
        case,
        &executor,
        authority,
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let tools = executor.into_model().tools;
    Ran {
        end,
        tools,
        performed: case.performed(),
    }
}

fn config() -> LoopConfig {
    LoopConfig::new("scripted-model", "INSTRUCTIONS").with_retry_backoff(Duration::from_millis(1))
}

const JUDGE_TWO: &str = r#"
name: change
protocol: software-change@1
steps:
  - name: edit
    kind: Judgement
    actions: [implementation.edit, tests.write]
    instructions: Edit or write a test.
"#;

/// The host narrowed its loop to `implementation_edit` (`LoopConfig::with_admitted`); a judgement
/// step naming `implementation.edit` and `tests.write` must narrow that further, never widen it
/// (`WorkflowExecutor::new` docs: "its actions narrow it"; `AGENTS.md` § Rules: never silently
/// broaden capability).
#[test]
fn adversary_w11_u2_a_judgement_step_never_widens_the_hosts_narrowing() {
    let case = Case::new(&[], Vec::new(), None);
    let narrowed = config().with_admitted(Some(vec![
        ToolName::new("implementation_edit").expect("valid"),
    ]));
    let ran = run(
        JUDGE_TWO,
        &case,
        narrowed,
        &StaticAuthorityProvider::new(),
        vec![
            calls("call_write", "tests_write", json!({})),
            calls("call_edit", "implementation_edit", json!({})),
        ],
    );
    assert!(!ran.tools.is_empty(), "the model was asked");
    for offered in &ran.tools {
        assert_eq!(
            offered,
            &["implementation_edit".to_owned()],
            "the host's narrowing was widened by the judgement step: {:?}",
            ran.tools
        );
    }
    assert!(
        !ran.proposed().iter().any(|action| action == WRITE),
        "an action outside the host's narrowing was proposed: {:?}",
        ran.proposed()
    );
}

/// A judgement step naming an action the frontier blocks offers the model only the step's actions
/// the frontier lists, and a call of the blocked one is never proposed.
#[test]
fn adversary_w11_u2_a_judgement_step_offers_no_action_the_frontier_blocks() {
    let case = Case::new(&[WRITE], Vec::new(), None);
    let ran = run(
        JUDGE_TWO,
        &case,
        config(),
        &StaticAuthorityProvider::new(),
        vec![
            calls("call_write", "tests_write", json!({})),
            calls("call_edit", "implementation_edit", json!({})),
        ],
    );
    assert!(!ran.tools.is_empty(), "the model was asked");
    for offered in &ran.tools {
        assert_eq!(
            offered,
            &["implementation_edit".to_owned()],
            "{:?}",
            ran.tools
        );
    }
    assert!(
        !ran.proposed().iter().any(|action| action == WRITE),
        "{:?}",
        ran.proposed()
    );
    assert!(
        !ran.performed.iter().any(|action| action == WRITE),
        "{:?}",
        ran.performed
    );
}

const DETERMINISTIC_THEN_JUDGE: &str = r#"
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
    instructions: Edit.
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {}
"#;

/// A deterministic proposal is revalidated by Commission like any other: the handed frontier listed
/// `repository.inspect`, the frontier read at revalidation blocks it. Nothing is performed, and the
/// workflow never moves on to a later step: no model is asked and no later action is proposed.
#[test]
fn adversary_w11_u2_a_deterministic_step_refused_at_revalidation_is_never_skipped() {
    let case = Case::new(&[], Vec::new(), Some(INSPECT));
    let ran = run(
        DETERMINISTIC_THEN_JUDGE,
        &case,
        config(),
        &StaticAuthorityProvider::new(),
        vec![calls("call_edit", "implementation_edit", json!({}))],
    );
    assert!(
        ran.performed.is_empty(),
        "nothing is performed: {:?}",
        ran.performed
    );
    assert!(
        ran.tools.is_empty(),
        "no model is asked past the refused step: {:?}",
        ran.tools
    );
    assert!(
        ran.proposed().iter().all(|action| action == INSPECT),
        "only the refused step is proposed: {:?}",
        ran.proposed()
    );
    assert!(
        !ran.proposed().is_empty(),
        "the step was proposed at least once"
    );
    if let RunOutcome::NeedsHumanJudgment(needs) = &ran.end.outcome {
        assert_eq!(
            needs.request.0.member("step"),
            Some(&CommissionValue::Text("inspect".to_owned()))
        );
    }
}

/// A deterministic step whose capability the authority provider denies is never effected and the
/// workflow does not go on to a later step on its own.
#[test]
fn adversary_w11_u2_a_denied_deterministic_step_is_never_effected_nor_skipped() {
    let case = Case::new(&[], vec![INSPECT], None);
    let authority = StaticAuthorityProvider::new().answer(
        INSPECT,
        AuthorityVerdict::Deny(AuthorityVerdictDeny {
            reason: "denied by the adversary".to_owned(),
        }),
    );
    let ran = run(
        DETERMINISTIC_THEN_JUDGE,
        &case,
        config(),
        &authority,
        vec![calls("call_edit", "implementation_edit", json!({}))],
    );
    assert!(ran.performed.is_empty(), "{:?}", ran.performed);
    assert!(
        ran.tools.is_empty(),
        "no model asked past the denied step: {:?}",
        ran.tools
    );
    assert!(
        ran.proposed().iter().all(|action| action == INSPECT),
        "{:?} / {:?}",
        ran.proposed(),
        ran.end.outcome
    );
}

const BASE: &str = r#"
name: change
protocol: software-change@1
steps:
  - name: inspect
    kind: Deterministic
    action: repository.inspect
    arguments: {}
  - name: second
    kind: Deterministic
    action: tests.run
    arguments: {}
"#;

/// Two steps whose names differ only by surrounding whitespace name one step twice: the check
/// trims for emptiness, so it must not tell `inspect` and `inspect ` apart for uniqueness either.
#[test]
fn adversary_w11_u2_a_step_named_twice_up_to_whitespace_is_refused() {
    let text = BASE.replace("name: second", "name: \"inspect \"");
    assert!(
        matches!(
            workflow::load(&text, &protocol),
            Err(DefinitionError::Step { .. })
        ),
        "{:?}",
        workflow::load(&text, &protocol)
    );
}

/// A step carrying a key twice is malformed, not silently resolved to one of its values.
#[test]
fn adversary_w11_u2_a_duplicated_key_is_refused() {
    let text = BASE.replace(
        "action: repository.inspect\n",
        "action: repository.inspect\n    action: tests.run\n",
    );
    assert!(
        matches!(
            workflow::load(&text, &protocol),
            Err(DefinitionError::Shape(_))
        ),
        "{:?}",
        workflow::load(&text, &protocol)
    );
}

/// An empty definition is refused: no steps, an empty document, two documents.
#[test]
fn adversary_w11_u2_empty_and_multi_document_definitions_are_refused() {
    let none = "name: change\nprotocol: software-change@1\nsteps: []\n";
    assert_eq!(
        workflow::load(none, &protocol),
        Err(DefinitionError::NoSteps)
    );
    assert!(matches!(
        workflow::load("", &protocol),
        Err(DefinitionError::Shape(_))
    ));
    let two = format!("---{BASE}---{BASE}");
    assert!(
        matches!(
            workflow::load(&two, &protocol),
            Err(DefinitionError::Shape(_))
        ),
        "{:?}",
        workflow::load(&two, &protocol)
    );
}
