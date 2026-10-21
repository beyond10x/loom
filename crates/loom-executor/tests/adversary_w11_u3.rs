// SPDX-License-Identifier: Apache-2.0

//! Adversarial cases for `story:dw-typed-step-outputs`: a step's typed outputs, taken from its
//! performed effect's report, bound into a later deterministic step's arguments without a model.
//!
//! Attacked here: type confusion (an integer with a fraction, a boolean spelt as a string, a list
//! of mixed items, `null`); RFC 6901 pointer edges (`~0`/`~1` and their order, the empty pointer,
//! an array index, a leading-zero index, `-`); a reference deep inside the arguments; a resolved
//! value shaped like a reference; a judgement step's arguments shaped like a reference; outputs of
//! a step whose first effect was refused; two steps with the same request; a log carried over from
//! an earlier executor; a resolved request still revalidated and gated by Commission; a step after
//! a resolved one still reached; determinism across reruns. The governor, the effect port and the
//! executor the host supplies for judgement steps (`story:judgement-step-any-executor`) are
//! scripted; nothing reaches a network.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    AuthorityVerdictApprovalRequired, CaseId, Commission, CommissionData, CommissionId,
    CompletionDetermination, CompletionDeterminationComplete, EffectOutcome,
    EffectOutcomePerformed, EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Frontier, FrontierAction, FrontierData, FrontierId, GovernorError, Observation, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, RunOutcome, Unit, commission_state,
    frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, LoopEnd, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::model::workflow::WorkflowDefinition;
use b10x_loom_executor::workflow::{
    self, DefinitionError, Judgement, RecordingEffects, WorkflowExecutor,
};
use serde_json::json;

const CASE: &str = "CASE-ADV-OUTPUTS";
const INSPECT: &str = "repository.inspect";
const WRITE: &str = "tests.write";
const EDIT: &str = "implementation.edit";
const RUN: &str = "tests.run";
const MERGE: &str = "repository.merge";

const PROTOCOL_ACTIONS: [&str; 6] = [INSPECT, WRITE, EDIT, RUN, MERGE, "release.rollback"];

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
    workflow::load(text, &protocol).unwrap_or_else(|error| panic!("refused: {error}\n{text}"))
}

/// The scripted case: one revision per performed effect; inspect, write, edit and run listed
/// admissible unless blocked; the merge listed, approval required, once `repository.inspect` was performed, and only when `merge_listed`.
struct Case {
    revision: AtomicI64,
    performed: Mutex<Vec<String>>,
    blocked: Vec<&'static str>,
    complete_after: Option<&'static str>,
    merge_listed: bool,
}

impl Case {
    fn new(blocked: Vec<&'static str>) -> Self {
        Self {
            revision: AtomicI64::new(1),
            performed: Mutex::new(Vec::new()),
            blocked,
            complete_after: None,
            merge_listed: false,
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
        if self.merge_listed && self.performed().iter().any(|done| done == INSPECT) {
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
        if let Some(action) = self.complete_after
            && self.performed().iter().any(|done| done == action)
        {
            return Ok(CompletionDetermination::Complete(
                CompletionDeterminationComplete {
                    outcome: "done".to_owned(),
                },
            ));
        }
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

/// One scripted answer of the effect port.
#[derive(Clone)]
enum Reply {
    Performed(CommissionValue),
    Refused,
}

/// The host's effect port: answers each invocation of an action with the next scripted reply for
/// it, `Performed` with a `null` report once the script for it is spent.
struct ScriptedEffects<'c> {
    case: &'c Case,
    script: Mutex<BTreeMap<&'static str, VecDeque<Reply>>>,
}

impl<'c> ScriptedEffects<'c> {
    fn new(case: &'c Case, script: Vec<(&'static str, Vec<Reply>)>) -> Self {
        Self {
            case,
            script: Mutex::new(
                script
                    .into_iter()
                    .map(|(action, replies)| (action, replies.into()))
                    .collect(),
            ),
        }
    }
}

impl EffectPort for ScriptedEffects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.clone();
        let reply = self
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(action.as_str())
            .and_then(VecDeque::pop_front)
            .unwrap_or(Reply::Performed(CommissionValue::Null));
        match reply {
            Reply::Refused => Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!("{action} refused by the test"),
                ends_run: false,
            })),
            Reply::Performed(report) => {
                self.case
                    .performed
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(action);
                self.case.revision.fetch_add(1, Ordering::SeqCst);
                Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                    report,
                    attempt: None,
                    audit: None,
                    usage: None,
                }))
            }
        }
    }
}

/// The host's executor of judgement steps, counting its calls: each proposes the next scripted
/// action and arguments, as an agent the workflow knows nothing of writes them.
struct Host {
    script: Mutex<VecDeque<(&'static str, serde_json::Value)>>,
    asked: AtomicU64,
}

/// The host's executor built for one judgement.
struct HostStep<'h> {
    host: &'h Host,
}

impl AgentExecutor for HostStep<'_> {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.host.asked.fetch_add(1, Ordering::SeqCst);
        match self
            .host
            .script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
        {
            Some((action, arguments)) => {
                ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                    action: action.to_owned(),
                    arguments: ProposedActionArguments(cv(arguments)),
                })
            }
            None => ExecutorOutcome::NoUsefulAction(Unit(true)),
        }
    }
}

/// The host's executor proposes `action` with `arguments`.
fn proposes(
    action: &'static str,
    arguments: serde_json::Value,
) -> (&'static str, serde_json::Value) {
    (action, arguments)
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

/// What one run ended with.
struct Ran {
    end: LoopEnd,
    /// The calls of the host's executor.
    asked: u64,
    performed: Vec<String>,
    outputs: Vec<(String, String, CommissionValue)>,
}

impl Ran {
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

    fn actions(&self) -> Vec<String> {
        self.proposed()
            .into_iter()
            .map(|(action, _)| action)
            .collect()
    }

    fn judgment(&self) -> &CommissionValue {
        match &self.end.outcome {
            RunOutcome::NeedsHumanJudgment(needs) => &needs.request.0,
            other => panic!(
                "not NeedsHumanJudgment: {other:?}; proposed {:?}",
                self.proposed()
            ),
        }
    }
}

/// Runs `text` once on `case` through `effects`, its judgement steps on the host's executor
/// proposing `proposals` in order.
fn run_on(
    case: &Case,
    effects: &RecordingEffects<ScriptedEffects<'_>>,
    text: &str,
    proposals: Vec<(&'static str, serde_json::Value)>,
) -> Ran {
    let host = Host {
        script: Mutex::new(proposals.into()),
        asked: AtomicU64::new(0),
    };
    let executor = WorkflowExecutor::with_executor(
        |_: &Judgement| HostStep { host: &host },
        definition(text),
        effects.log(),
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
        case,
        &executor,
        &authority,
        effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let outputs = executor.outputs();
    drop(executor);
    Ran {
        end,
        asked: host.asked.load(Ordering::SeqCst),
        performed: case.performed(),
        outputs,
    }
}

/// Runs `text` on a fresh case blocking `blocked`, whose effect port answers `script`.
fn run(
    text: &str,
    blocked: Vec<&'static str>,
    script: Vec<(&'static str, Vec<Reply>)>,
    proposals: Vec<(&'static str, serde_json::Value)>,
) -> Ran {
    let case = Case::new(blocked);
    let effects = RecordingEffects::new(ScriptedEffects::new(&case, script));
    run_on(&case, &effects, text, proposals)
}

/// A serde_json value as Commission's JSON, numbers in serde_json's spelling.
fn cv(value: serde_json::Value) -> CommissionValue {
    match value {
        serde_json::Value::Null => CommissionValue::Null,
        serde_json::Value::Bool(flag) => CommissionValue::Bool(flag),
        serde_json::Value::Number(number) => CommissionValue::Number(number.to_string()),
        serde_json::Value::String(text) => CommissionValue::Text(text),
        serde_json::Value::Array(items) => {
            CommissionValue::Array(items.into_iter().map(cv).collect())
        }
        serde_json::Value::Object(members) => CommissionValue::Object(
            members
                .into_iter()
                .map(|(key, value)| (key, cv(value)))
                .collect(),
        ),
    }
}

fn text(value: &str) -> CommissionValue {
    CommissionValue::Text(value.to_owned())
}

fn number(spelling: &str) -> CommissionValue {
    CommissionValue::Number(spelling.to_owned())
}

fn performed(report: CommissionValue) -> Vec<Reply> {
    vec![Reply::Performed(report)]
}

/// A two-step workflow: `read` (inspect) declares `value` at `pointer` with `kind`, and `use`
/// (tests.run) proposes `{got: <value>}`.
fn hand_over(pointer: &str, kind: &str) -> String {
    format!(
        r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {{path: "calls"}}
    outputs:
      - {{name: value, pointer: '{pointer}', type: {kind}}}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments: {{got: {{$step: read, $output: value}}}}
"#
    )
}

/// `report` as `read`'s report, `pointer` and `kind` declared: the value `use` proposes.
fn resolved(pointer: &str, kind: &str, report: CommissionValue) -> CommissionValue {
    let ran = run(
        &hand_over(pointer, kind),
        Vec::new(),
        vec![(INSPECT, performed(report.clone()))],
        Vec::new(),
    );
    let proposed = ran.proposed();
    assert_eq!(
        ran.actions(),
        [INSPECT, RUN],
        "`{pointer}` as {kind} on {report:?}: {:?}",
        ran.end.outcome
    );
    assert_eq!(ran.asked, 0);
    proposed[1]
        .1
        .member("got")
        .cloned()
        .unwrap_or_else(|| panic!("no `got`: {proposed:?}"))
}

/// `report` as `read`'s report, `pointer` and `kind` declared, is refused: either at load, or the
/// call after the read ends `NeedsHumanJudgment` naming the output, and nothing is proposed after
/// the read.
fn proposes_nothing_after_the_read(pointer: &str, kind: &str, report: CommissionValue) {
    let text = hand_over(pointer, kind);
    if workflow::load(&text, &protocol).is_err() {
        return;
    }
    let ran = run(
        &text,
        Vec::new(),
        vec![(INSPECT, performed(report.clone()))],
        Vec::new(),
    );
    assert_eq!(
        ran.actions(),
        [INSPECT],
        "`{pointer}` as {kind} on {report:?} proposed after the read: {:?}",
        ran.proposed()
    );
    let request = ran.judgment();
    assert_eq!(
        request.member("output"),
        Some(&text_value("value")),
        "{request:?}"
    );
    assert!(
        ran.outputs.is_empty(),
        "no guessed value: {:?}",
        ran.outputs
    );
}

fn text_value(value: &str) -> CommissionValue {
    text(value)
}

// ---- type confusion ---------------------------------------------------------------------------

/// A number with a fraction is not an `Integer`; nor is `1.0`'s neighbour `2.5` in a list.
#[test]
fn an_integer_with_a_fraction_proposes_nothing() {
    proposes_nothing_after_the_read("/n", "Integer", cv(json!({"n": 1.5})));
    proposes_nothing_after_the_read("/n", "IntegerList", cv(json!({"n": [1, 2.5]})));
}

/// A whole number, negative included, is an `Integer` and keeps its spelling.
#[test]
fn a_whole_number_is_an_integer_and_keeps_its_spelling() {
    assert_eq!(
        resolved("/n", "Integer", cv(json!({"n": 42}))),
        number("42")
    );
    assert_eq!(
        resolved("/n", "Integer", cv(json!({"n": -3}))),
        number("-3")
    );
}

/// A string spelling a boolean or a number is not a `Boolean` or an `Integer`; a boolean is not a
/// `String`.
#[test]
fn a_string_spelling_another_type_is_not_that_type() {
    proposes_nothing_after_the_read("/b", "Boolean", cv(json!({"b": "true"})));
    proposes_nothing_after_the_read("/n", "Integer", cv(json!({"n": "17"})));
    proposes_nothing_after_the_read("/s", "String", cv(json!({"s": true})));
    assert_eq!(
        resolved("/b", "Boolean", cv(json!({"b": false}))),
        CommissionValue::Bool(false)
    );
}

/// A list holding an item of another type is not a list of that type; a scalar is not a list;
/// an empty list is a list of any type; a `JsonList` takes mixed items.
#[test]
fn a_list_holds_only_items_of_its_type() {
    proposes_nothing_after_the_read("/l", "StringList", cv(json!({"l": ["a", 1]})));
    proposes_nothing_after_the_read("/l", "BooleanList", cv(json!({"l": [true, "false"]})));
    proposes_nothing_after_the_read("/l", "StringList", cv(json!({"l": "a"})));
    proposes_nothing_after_the_read("/l", "StringList", cv(json!({"l": ["a", null]})));
    assert_eq!(
        resolved("/l", "StringList", cv(json!({"l": []}))),
        CommissionValue::Array(Vec::new())
    );
    assert_eq!(
        resolved(
            "/l",
            "JsonList",
            cv(json!({"l": [1, "a", null, {"k": true}]}))
        ),
        cv(json!([1, "a", null, {"k": true}]))
    );
    proposes_nothing_after_the_read("/l", "JsonList", cv(json!({"l": {"0": 1}})));
}

/// `null` is no `String`, `Integer` or `Boolean`; a member holding `null` is present, so a `Json`
/// output carries it.
#[test]
fn null_is_present_but_typed_only_as_json() {
    proposes_nothing_after_the_read("/s", "String", cv(json!({"s": null})));
    proposes_nothing_after_the_read("/s", "Integer", cv(json!({"s": null})));
    proposes_nothing_after_the_read("/s", "Boolean", cv(json!({"s": null})));
    assert_eq!(
        resolved("/s", "Json", cv(json!({"s": null}))),
        CommissionValue::Null
    );
}

// ---- JSON pointers (RFC 6901) -----------------------------------------------------------------

/// `~1` is `/` and `~0` is `~`, and `~01` is the key `~1`, not `/`: `~1` is unescaped before `~0`.
#[test]
fn pointer_escapes_are_unescaped_in_order() {
    let report = cv(json!({"a/b": "slash", "m~n": "tilde", "~1": "tilde-one", "/": "wrong"}));
    assert_eq!(resolved("/a~1b", "String", report.clone()), text("slash"));
    assert_eq!(resolved("/m~0n", "String", report.clone()), text("tilde"));
    assert_eq!(resolved("/~01", "String", report), text("tilde-one"));
}

/// The empty pointer names the whole report.
#[test]
fn the_empty_pointer_is_the_whole_report() {
    let report = cv(json!({"call": {"id": "c-17"}, "n": 2}));
    assert_eq!(resolved("", "Json", report.clone()), report);
    assert_eq!(resolved("", "String", text("whole")), text("whole"));
}

/// `/` names the member with the empty key, not the whole report.
#[test]
fn a_lone_slash_is_the_empty_key() {
    let report = cv(json!({"": "empty-key", "other": 1}));
    assert_eq!(resolved("/", "String", report), text("empty-key"));
}

/// An array index selects its item; an index with a leading zero, `-`, an index past the end
/// and a negative index select nothing.
#[test]
fn array_indices_follow_rfc_6901() {
    let report = cv(json!({"items": ["zero", "one", "two"]}));
    assert_eq!(resolved("/items/1", "String", report.clone()), text("one"));
    assert_eq!(resolved("/items/0", "String", report.clone()), text("zero"));
    proposes_nothing_after_the_read("/items/01", "String", report.clone());
    proposes_nothing_after_the_read("/items/-", "String", report.clone());
    proposes_nothing_after_the_read("/items/3", "String", report.clone());
    proposes_nothing_after_the_read("/items/-1", "String", report.clone());
    proposes_nothing_after_the_read("/items/+1", "String", report);
}

/// An object member whose key is a number is a member, not an index; an index into an object
/// selects nothing unless the key is spelt so.
#[test]
fn a_numeric_key_is_a_member_of_an_object() {
    let report = cv(json!({"items": {"1": "member", "01": "padded"}}));
    assert_eq!(
        resolved("/items/1", "String", report.clone()),
        text("member")
    );
    assert_eq!(resolved("/items/01", "String", report), text("padded"));
}

/// A pointer with an escape that is neither `~0` nor `~1` is malformed.
#[test]
fn a_malformed_escape_proposes_nothing() {
    let report = cv(json!({"a~2": "x", "a": "y", "a~": "z"}));
    proposes_nothing_after_the_read("/a~2", "String", report.clone());
    proposes_nothing_after_the_read("/a~", "String", report);
}

// ---- where a reference sits -------------------------------------------------------------------

/// A reference four levels deep, inside lists and objects, is resolved where it sits; the members
/// around it are kept as written.
#[test]
fn a_reference_deep_inside_the_arguments_is_resolved_in_place() {
    let deep = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: id, pointer: /id, type: String}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments:
      a:
        - keep
        - b:
            - c: {$step: read, $output: id}
              d: 1
"#;
    let ran = run(
        deep,
        Vec::new(),
        vec![(INSPECT, performed(cv(json!({"id": "deep-id"}))))],
        Vec::new(),
    );
    assert_eq!(ran.actions(), [INSPECT, RUN], "{:?}", ran.end.outcome);
    assert_eq!(
        ran.proposed()[1].1,
        CommissionValue::Object(vec![(
            "a".to_owned(),
            CommissionValue::Array(vec![
                text("keep"),
                CommissionValue::Object(vec![(
                    "b".to_owned(),
                    CommissionValue::Array(vec![CommissionValue::Object(vec![
                        ("c".to_owned(), text("deep-id")),
                        ("d".to_owned(), number("1")),
                    ])])
                )])
            ])
        )])
    );
}

/// A resolved value shaped like a reference is data: it is not resolved again, so a report cannot
/// make one output stand for another.
#[test]
fn a_resolved_value_shaped_like_a_reference_is_not_resolved_again() {
    let shaped = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: blob, pointer: /blob, type: Json}
      - {name: secret, pointer: /secret, type: String}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments: {got: {$step: read, $output: blob}}
"#;
    let ran = run(
        shaped,
        Vec::new(),
        vec![(
            INSPECT,
            performed(cv(
                json!({"blob": {"$step": "read", "$output": "secret"}, "secret": "s3cr3t"}),
            )),
        )],
        Vec::new(),
    );
    assert_eq!(ran.actions(), [INSPECT, RUN], "{:?}", ran.end.outcome);
    assert_eq!(
        ran.proposed()[1].1.member("got"),
        Some(&cv(json!({"$step": "read", "$output": "secret"}))),
        "{:?}",
        ran.proposed()
    );
}

/// A judgement step's arguments shaped like a reference are its executor's, carried as written:
/// only a deterministic step's own arguments are resolved, so an executor (a model, or the host's
/// agent) cannot read an output through one.
#[test]
fn a_models_arguments_shaped_like_a_reference_are_not_resolved() {
    let judged = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: id, pointer: /id, type: String}
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Edit.
"#;
    let ran = run(
        judged,
        Vec::new(),
        vec![(INSPECT, performed(cv(json!({"id": "c-17"}))))],
        vec![proposes(
            EDIT,
            json!({"ref": {"$step": "read", "$output": "id"}}),
        )],
    );
    let proposed = ran.proposed();
    assert_eq!(ran.actions(), [INSPECT, EDIT], "{:?}", ran.end.outcome);
    assert_eq!(
        proposed[1].1.member("ref"),
        Some(&cv(json!({"$step": "read", "$output": "id"}))),
        "{proposed:?}"
    );
}

/// An output a judgement step declares is read from its effect's report, never from the
/// arguments its executor wrote; or a judgement step's outputs are refused at load (they are out
/// of the story's scope).
#[test]
fn a_judgement_steps_output_never_carries_the_models_value() {
    let judged = r#"
name: adversary
protocol: software-change@1
steps:
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Edit.
    outputs:
      - {name: path, pointer: /path, type: String}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments: {got: {$step: edit, $output: path}}
"#;
    if workflow::load(judged, &protocol).is_err() {
        return;
    }
    let ran = run(
        judged,
        Vec::new(),
        vec![(EDIT, performed(cv(json!({"path": "from-report"}))))],
        vec![proposes(EDIT, json!({"path": "from-model"}))],
    );
    assert_eq!(ran.actions(), [EDIT, RUN], "{:?}", ran.end.outcome);
    assert_eq!(
        ran.proposed()[1].1.member("got"),
        Some(&text("from-report")),
        "{:?}",
        ran.proposed()
    );
}

// ---- which effect an output comes from --------------------------------------------------------

/// A step whose first effect was refused takes its outputs from the effect performed on the
/// retry, and declares them once.
#[test]
fn outputs_come_from_the_performed_attempt_after_a_refusal() {
    let ran = run(
        &hand_over("/id", "String"),
        Vec::new(),
        vec![(
            INSPECT,
            vec![
                Reply::Refused,
                Reply::Performed(cv(json!({"id": "second"}))),
            ],
        )],
        Vec::new(),
    );
    assert_eq!(
        ran.actions(),
        [INSPECT, INSPECT, RUN],
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(ran.proposed()[2].1.member("got"), Some(&text("second")));
    assert_eq!(
        ran.outputs,
        [("read".to_owned(), "value".to_owned(), text("second"))]
    );
}

/// Two steps making the same request each take their outputs from their own performed effect.
#[test]
fn two_steps_with_the_same_request_each_read_their_own_report() {
    let twice = r#"
name: adversary
protocol: software-change@1
steps:
  - name: first
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "calls"}
    outputs:
      - {name: n, pointer: /n, type: Integer}
  - name: second
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "calls"}
    outputs:
      - {name: n, pointer: /n, type: Integer}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments: {a: {$step: first, $output: n}, b: {$step: second, $output: n}}
"#;
    let ran = run(
        twice,
        Vec::new(),
        vec![(
            INSPECT,
            vec![
                Reply::Performed(cv(json!({"n": 1}))),
                Reply::Performed(cv(json!({"n": 2}))),
            ],
        )],
        Vec::new(),
    );
    assert_eq!(
        ran.actions(),
        [INSPECT, INSPECT, RUN],
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(
        ran.proposed()[2].1,
        CommissionValue::Object(vec![
            ("a".to_owned(), number("1")),
            ("b".to_owned(), number("2")),
        ])
    );
}

/// A host that hands a new executor the log of an earlier run: an output the earlier executor's
/// step produced is not this run's, so a step whose source step this executor never performed
/// proposes nothing.
#[test]
fn an_output_from_an_earlier_executors_run_is_not_this_runs() {
    let case = Case::new(Vec::new());
    let effects = RecordingEffects::new(ScriptedEffects::new(
        &case,
        vec![(INSPECT, performed(cv(json!({"id": "earlier"}))))],
    ));
    let first = run_on(&case, &effects, &hand_over("/id", "String"), Vec::new());
    assert_eq!(first.actions(), [INSPECT, RUN], "{:?}", first.end.outcome);

    let skipped = hand_over("/id", "String").replace(
        "  - name: read\n",
        "  - name: read\n    when: {claim: never.reported, value: 'True'}\n",
    );
    let second = run_on(&case, &effects, &skipped, Vec::new());
    assert!(
        second.proposed().is_empty(),
        "an earlier run's output reached this run: {:?}",
        second.proposed()
    );
    assert_eq!(second.judgment().member("output"), Some(&text("value")));
    assert!(second.outputs.is_empty(), "{:?}", second.outputs);
}

// ---- Commission still decides -----------------------------------------------------------------

/// A step after a resolved step is reached: the resolved proposal is the one the executor waits to
/// see performed, so the workflow advances past it.
#[test]
fn the_step_after_a_resolved_step_is_reached() {
    let three = format!(
        "{}  - name: after\n    kind: Deterministic\n    action: tests.write\n    arguments: {{x: 1}}\n",
        hand_over("/id", "String")
    );
    let ran = run(
        &three,
        Vec::new(),
        vec![(INSPECT, performed(cv(json!({"id": "c-9"}))))],
        Vec::new(),
    );
    assert_eq!(
        ran.actions(),
        [INSPECT, RUN, WRITE],
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(ran.performed, [INSPECT, RUN, WRITE]);
}

/// A resolved request needing approval still stops at Commission's authority gate, and the
/// request Commission revalidated carries the resolved value.
#[test]
fn a_resolved_request_is_still_gated_by_authority() {
    let gated = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: branch, pointer: /branch, type: String}
  - name: merge
    kind: Deterministic
    action: repository.merge
    arguments: {branch: {$step: read, $output: branch}}
"#;
    let mut case = Case::new(Vec::new());
    case.merge_listed = true;
    let effects = RecordingEffects::new(ScriptedEffects::new(
        &case,
        vec![(INSPECT, performed(cv(json!({"branch": "feature/x"}))))],
    ));
    let ran = run_on(&case, &effects, gated, Vec::new());
    assert!(
        matches!(&ran.end.outcome, RunOutcome::NeedsAuthority(needs) if needs.request == "merge-approval"),
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(ran.performed, [INSPECT], "the merge was not performed");
    assert_eq!(
        ran.proposed()
            .last()
            .map(|(_, arguments)| arguments.clone()),
        Some(cv(json!({"branch": "feature/x"})))
    );
}

/// A resolved step whose action the frontier blocks proposes nothing: resolving an output grants
/// no admission.
#[test]
fn a_resolved_step_the_frontier_blocks_proposes_nothing() {
    let ran = run(
        &hand_over("/id", "String"),
        vec![RUN],
        vec![(INSPECT, performed(cv(json!({"id": "c-1"}))))],
        Vec::new(),
    );
    assert_eq!(ran.actions(), [INSPECT], "{:?}", ran.proposed());
    assert_eq!(ran.judgment().member("step"), Some(&text("use")));
    assert_eq!(ran.performed, [INSPECT]);
}

// ---- references that cannot be satisfied at load ----------------------------------------------

/// A step naming its own output, directly or by a two-step cycle, is refused at load.
#[test]
fn a_reference_cycle_is_refused_at_load() {
    let itself = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {self: {$step: read, $output: id}}
    outputs:
      - {name: id, pointer: /id, type: String}
"#;
    assert!(
        matches!(workflow::load(itself, &protocol), Err(DefinitionError::Step { step, .. }) if step == "read"),
        "{:?}",
        workflow::load(itself, &protocol)
    );
    let cycle = r#"
name: adversary
protocol: software-change@1
steps:
  - name: a
    kind: Deterministic
    action: repository.inspect
    arguments: {x: {$step: b, $output: y}}
    outputs:
      - {name: x, pointer: /x, type: String}
  - name: b
    kind: Deterministic
    action: tests.run
    arguments: {y: {$step: a, $output: x}}
    outputs:
      - {name: y, pointer: /y, type: String}
"#;
    assert!(
        matches!(workflow::load(cycle, &protocol), Err(DefinitionError::Step { step, .. }) if step == "a"),
        "{:?}",
        workflow::load(cycle, &protocol)
    );
}

/// A judgement step's arguments are the model's, so it has none to resolve; a reference naming a
/// judgement step's undeclared output is refused at load like any other.
#[test]
fn a_reference_to_an_undeclared_output_of_any_step_kind_is_refused() {
    let undeclared = r#"
name: adversary
protocol: software-change@1
steps:
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Edit.
  - name: use
    kind: Deterministic
    action: tests.run
    arguments: {got: {$step: edit, $output: path}}
"#;
    assert!(
        matches!(workflow::load(undeclared, &protocol), Err(DefinitionError::Step { step, .. }) if step == "use"),
        "{:?}",
        workflow::load(undeclared, &protocol)
    );
}

// ---- determinism ------------------------------------------------------------------------------

/// Two runs with the same reports propose the same arguments and hold the same outputs, in the
/// order the step declares them, whatever order the report holds them in.
#[test]
fn outputs_are_identical_and_in_declared_order_across_reruns() {
    let many = r#"
name: adversary
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: zeta, pointer: /z, type: Integer}
      - {name: alpha, pointer: /a, type: StringList}
      - {name: mid, pointer: /m, type: Json}
  - name: use
    kind: Deterministic
    action: tests.run
    arguments:
      - {$step: read, $output: mid}
      - {$step: read, $output: alpha}
      - {$step: read, $output: zeta}
"#;
    let report = || {
        CommissionValue::Object(vec![
            ("a".to_owned(), cv(json!(["x", "y"]))),
            ("m".to_owned(), cv(json!({"k": [1, 2]}))),
            ("z".to_owned(), number("7")),
        ])
    };
    let once = || {
        run(
            many,
            Vec::new(),
            vec![(INSPECT, performed(report()))],
            Vec::new(),
        )
    };
    let first = once();
    let second = once();
    assert_eq!(first.proposed(), second.proposed());
    assert_eq!(first.outputs, second.outputs);
    assert_eq!(
        first.outputs,
        [
            ("read".to_owned(), "zeta".to_owned(), number("7")),
            ("read".to_owned(), "alpha".to_owned(), cv(json!(["x", "y"]))),
            (
                "read".to_owned(),
                "mid".to_owned(),
                cv(json!({"k": [1, 2]}))
            ),
        ]
    );
    assert_eq!(
        first.proposed()[1].1,
        CommissionValue::Array(vec![
            cv(json!({"k": [1, 2]})),
            cv(json!(["x", "y"])),
            number("7"),
        ])
    );
}

// ---- when an output is taken ------------------------------------------------------------------

/// The story's design: `RecordingEffects` extracts and type-checks a step's outputs when its
/// effect is `Performed`, and keeps them for the run. A run the governor completes right after the
/// last step was performed calls the executor no more, and that step's outputs are still the
/// run's.
#[test]
fn the_last_steps_outputs_are_kept_when_the_governor_completes_the_case() {
    let mut case = Case::new(Vec::new());
    case.complete_after = Some(RUN);
    let last = r#"
name: adversary
protocol: software-change@1
steps:
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {}
    outputs:
      - {name: passed, pointer: /passed, type: Boolean}
"#;
    let effects = RecordingEffects::new(ScriptedEffects::new(
        &case,
        vec![(RUN, performed(cv(json!({"passed": true}))))],
    ));
    let ran = run_on(&case, &effects, last, Vec::new());
    assert!(
        matches!(&ran.end.outcome, RunOutcome::Completed(_)),
        "{:?}",
        ran.end.outcome
    );
    assert_eq!(ran.performed, [RUN]);
    assert_eq!(
        ran.outputs,
        [(
            "test".to_owned(),
            "passed".to_owned(),
            CommissionValue::Bool(true)
        )],
        "the performed step's outputs were never taken"
    );
}
