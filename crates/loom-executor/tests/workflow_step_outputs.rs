// SPDX-License-Identifier: Apache-2.0

//! `story:dw-typed-step-outputs`: a step declares typed outputs taken from its performed effect's
//! report, and a later deterministic step's arguments name them; the runtime fills them in without
//! a model. A missing or mistyped output ends the call `NeedsHumanJudgment` and proposes nothing.
//!
//! The case is scripted: a governor listing the `software-change@1` actions admissible, moving one
//! revision per performed effect; an effect port answering each action with the report the test
//! scripts for it. The model is scripted and metered (`MeteredModelPort`) under a
//! `MeteredExecutor` sharing one `RunMeter`; no workflow here has a judgement step, so it is never
//! asked. Nothing reaches a network.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value as CommissionValue;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid as CommissionUuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, EffectOutcome, EffectOutcomePerformed,
    Frontier, FrontierAction, FrontierData, FrontierId, GovernorError, Observation, ObservationId,
    PrincipalId, RunId, RunOutcome, Unit, commission_state, frontier_state, observation_state,
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
    ModelPort, StreamSink, TurnOutcome, TurnRequest, WireError, WireId,
};
use b10x_loom_executor::metering::{MeteredExecutor, MeteredModelPort, RunMeter};
use b10x_loom_executor::model::metering::RunUsageReport;
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, SessionData, SessionId};
use b10x_loom_executor::model::workflow::WorkflowDefinition;
use b10x_loom_executor::workflow::{self, DefinitionError, RecordingEffects, WorkflowExecutor};
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};

const CASE: &str = "CASE-OUTPUTS";
const INSPECT: &str = "repository.inspect";
const RUN: &str = "tests.run";
const EDIT: &str = "implementation.edit";

const PROTOCOL_ACTIONS: [&str; 6] = [
    INSPECT,
    "tests.write",
    EDIT,
    RUN,
    "repository.merge",
    "release.rollback",
];

/// The workflow of the acceptance: a read step declaring `call_id` at `/call/id`, then a
/// deterministic step whose arguments name it, once as a member and once inside a list.
const DEFINITION: &str = r#"
name: hand-over
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {path: "calls"}
    outputs:
      - {name: call_id, pointer: /call/id, type: String}
  - name: test
    kind: Deterministic
    action: tests.run
    arguments:
      call: {$step: read, $output: call_id}
      flags: ["--locked", {$step: read, $output: call_id}]
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
    (name == "software-change@1").then(|| PROTOCOL_ACTIONS.map(str::to_owned).to_vec())
}

fn definition(text: &str) -> WorkflowDefinition {
    workflow::load(text, &protocol).unwrap_or_else(|error| panic!("refused: {error}"))
}

/// The scripted case: one revision per performed effect, every action of the protocol but the
/// merge and the rollback listed admissible.
struct Case {
    revision: AtomicI64,
    performed: Mutex<Vec<String>>,
}

impl Case {
    fn new() -> Self {
        Self {
            revision: AtomicI64::new(1),
            performed: Mutex::new(Vec::new()),
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
        let actions = [INSPECT, "tests.write", EDIT, RUN]
            .into_iter()
            .map(|action| FrontierAction {
                action: action.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
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
        Ok(())
    }
}

/// The host's effect port: performs every action on the case and reports, for each, the report
/// the test scripted for it (`null` for an action the test scripted none for).
struct ReportingEffects<'c> {
    case: &'c Case,
    reports: BTreeMap<&'static str, CommissionValue>,
}

impl EffectPort for ReportingEffects<'_> {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let action = request.data().action.as_str();
        self.case
            .performed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(action.to_owned());
        self.case.revision.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: self
                .reports
                .get(action)
                .cloned()
                .unwrap_or(CommissionValue::Null),
            attempt: None,
            audit: None,
        }))
    }
}

/// A model with no script: any call is an error, and every call is counted.
struct NoModel {
    wire: WireId,
    asked: usize,
}

impl ModelPort for NoModel {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        _request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.asked += 1;
        Err(WireError::protocol("this workflow never asks a model"))
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
        session_id: SessionId(Uuid("00000000-0000-4000-8000-00000000d013".to_owned())),
        commission_run: CommissionRunId(Uuid("00000000-0000-4000-8000-00000000d0ac".to_owned())),
        wire: responses::WIRE.to_owned(),
        boundary_refusals: 0,
    }
}

/// What one run of the workflow ended with.
struct Ran {
    end: LoopEnd,
    asked: usize,
    performed: Vec<String>,
    outputs: Vec<(String, String, CommissionValue)>,
    report: RunUsageReport,
}

impl Ran {
    /// Every request the runtime made of a proposal, in order: its action and arguments.
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

    /// The `NeedsHumanJudgment` request the run ended with.
    fn judgment(&self) -> &CommissionValue {
        match &self.end.outcome {
            RunOutcome::NeedsHumanJudgment(needs) => &needs.request.0,
            other => panic!("not NeedsHumanJudgment: {other:?}"),
        }
    }
}

/// Runs `text` on a fresh case whose effect port answers `reports`.
fn run(text: &str, reports: &[(&'static str, CommissionValue)]) -> Ran {
    let case = Case::new();
    let effects = RecordingEffects::new(ReportingEffects {
        case: &case,
        reports: reports.iter().cloned().collect(),
    });
    let loom = Loom::new(FirstAdmissibleSelector, EmptyObjectArguments, "hand over")
        .with_instance("workflow-outputs-test")
        .with_governor(&case);
    let meter = RunMeter::new(None);
    let model = NoModel {
        wire: WireId::new(responses::WIRE).expect("valid"),
        asked: 0,
    };
    let executor = MeteredExecutor::new(
        WorkflowExecutor::new(
            &loom,
            MeteredModelPort::new(model, &meter),
            LoopConfig::new("scripted-model", "INSTRUCTIONS")
                .with_retry_backoff(Duration::from_millis(1)),
            session(),
            definition(text),
            effects.log(),
        )
        .with_meter(&meter),
        &meter,
    );
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let end = run_until_blocked(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));
    let report = meter.report(&end.run_id);
    let executor = executor.into_inner();
    let outputs = executor.outputs();
    let asked = executor.into_model().into_inner().asked;
    Ran {
        end,
        asked,
        performed: case.performed(),
        outputs,
        report,
    }
}

fn object(members: &[(&str, CommissionValue)]) -> CommissionValue {
    CommissionValue::Object(
        members
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

fn text(value: &str) -> CommissionValue {
    CommissionValue::Text(value.to_owned())
}

fn number(spelling: &str) -> CommissionValue {
    CommissionValue::Number(spelling.to_owned())
}

/// `{"call": {"id": <id>}}`, the read's report.
fn call_report(id: CommissionValue) -> CommissionValue {
    object(&[("call", object(&[("id", id)]))])
}

/// The acceptance: the read's report `{"call": {"id": "c-17"}}` declares `call_id: String` at
/// `/call/id`; the next deterministic step proposes arguments holding `"c-17"` wherever they name
/// it; the metered model port saw no call.
#[test]
fn a_later_step_reads_an_earlier_output_without_a_model() {
    let ran = run(DEFINITION, &[(INSPECT, call_report(text("c-17")))]);

    assert_eq!(ran.performed, [INSPECT, RUN]);
    assert_eq!(
        ran.proposed(),
        vec![
            (INSPECT.to_owned(), object(&[("path", text("calls"))])),
            (
                RUN.to_owned(),
                object(&[
                    ("call", text("c-17")),
                    (
                        "flags",
                        CommissionValue::Array(vec![text("--locked"), text("c-17")])
                    ),
                ])
            ),
        ]
    );
    assert_eq!(
        ran.report.model_calls, 0,
        "the metered model port saw no call: {:?}",
        ran.report
    );
    assert_eq!(ran.asked, 0);
    assert_eq!(
        ran.outputs,
        [("read".to_owned(), "call_id".to_owned(), text("c-17"))]
    );
}

/// `/call/id` holding a number, declared `String`: the call after the read ends
/// `NeedsHumanJudgment` naming the step and the output, and nothing is proposed after the read.
#[test]
fn a_mistyped_output_ends_needing_human_judgment() {
    let ran = run(DEFINITION, &[(INSPECT, call_report(number("17")))]);

    let request = ran.judgment();
    assert_eq!(request.member("step"), Some(&text("read")), "{request:?}");
    assert_eq!(
        request.member("output"),
        Some(&text("call_id")),
        "{request:?}"
    );
    assert_eq!(
        ran.proposed()
            .iter()
            .map(|(action, _)| action.as_str())
            .collect::<Vec<_>>(),
        [INSPECT],
        "nothing is proposed after the read"
    );
    assert_eq!(ran.performed, [INSPECT]);
    assert!(
        ran.outputs.is_empty(),
        "no guessed value: {:?}",
        ran.outputs
    );
    assert_eq!(ran.asked, 0);
}

/// A report without `/call/id` ends the call `NeedsHumanJudgment` naming the step and the output,
/// and nothing is proposed after the read.
#[test]
fn an_absent_output_ends_needing_human_judgment() {
    for report in [
        object(&[("call", object(&[]))]),
        object(&[("call", CommissionValue::Array(vec![text("c-17")]))]),
        CommissionValue::Null,
    ] {
        let ran = run(DEFINITION, &[(INSPECT, report.clone())]);

        let request = ran.judgment();
        assert_eq!(request.member("step"), Some(&text("read")), "{report:?}");
        assert_eq!(
            request.member("output"),
            Some(&text("call_id")),
            "{report:?}"
        );
        assert_eq!(ran.performed, [INSPECT], "{report:?}");
        assert_eq!(ran.proposed().len(), 1, "{report:?}");
        assert!(ran.outputs.is_empty(), "{report:?}");
    }
}

/// A step that names an output of a step that was not performed in the run (here skipped because
/// its `when` never holds) ends `NeedsHumanJudgment`, naming the referring step and the output.
#[test]
fn a_reference_to_an_output_of_a_skipped_step_ends_needing_human_judgment() {
    let skipped = DEFINITION.replace(
        "  - name: read\n",
        "  - name: read\n    when: {claim: never.reported, value: 'True'}\n",
    );
    let ran = run(&skipped, &[(INSPECT, call_report(text("c-17")))]);

    let request = ran.judgment();
    assert_eq!(request.member("step"), Some(&text("test")), "{request:?}");
    assert_eq!(
        request.member("output"),
        Some(&text("call_id")),
        "{request:?}"
    );
    assert!(ran.proposed().is_empty(), "{:?}", ran.proposed());
    assert!(ran.performed.is_empty());
}

/// At load, an argument naming an output of a later step, of the step itself, of an unknown step,
/// or an output its step does not declare, is refused naming the referring step; so is a
/// malformed reference, a malformed pointer and an output declared twice.
#[test]
fn a_reference_to_a_later_or_unknown_step_is_refused_at_load() {
    let refused = |text: &str, step: &str, needle: &str| match workflow::load(text, &protocol) {
        Err(DefinitionError::Step { step: named, why }) => {
            assert_eq!(named, step, "{why}");
            assert!(why.contains(needle), "`{needle}` not in: {why}");
        }
        other => panic!("not refused naming `{step}`: {other:?}"),
    };

    let later = r#"
name: hand-over
protocol: software-change@1
steps:
  - name: test
    kind: Deterministic
    action: tests.run
    arguments: {call: {$step: read, $output: call_id}}
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: call_id, pointer: /call/id, type: String}
"#;
    refused(later, "test", "read");

    let itself = DEFINITION.replace(
        "$step: read, $output: call_id}]",
        "$step: test, $output: call_id}]",
    );
    refused(&itself, "test", "test");

    let unknown = DEFINITION.replace("call: {$step: read,", "call: {$step: lookup,");
    refused(&unknown, "test", "lookup");

    let undeclared = DEFINITION.replace(
        "call: {$step: read, $output: call_id}",
        "call: {$step: read, $output: call_name}",
    );
    refused(&undeclared, "test", "call_name");

    let malformed = DEFINITION.replace(
        "call: {$step: read, $output: call_id}",
        "call: {$step: read}",
    );
    refused(&malformed, "test", "$output");

    let pointer = DEFINITION.replace("pointer: /call/id", "pointer: call/id");
    refused(&pointer, "read", "pointer");

    let twice = DEFINITION.replace(
        "      - {name: call_id, pointer: /call/id, type: String}\n",
        "      - {name: call_id, pointer: /call/id, type: String}\n      - {name: call_id, pointer: /id, type: Json}\n",
    );
    refused(&twice, "read", "call_id");

    let untyped = DEFINITION.replace("type: String}", "type: Text}");
    assert!(
        matches!(
            workflow::load(&untyped, &protocol),
            Err(DefinitionError::Shape(_))
        ),
        "an undeclared output type is a shape error"
    );
}

/// Every type, read twice from the same reports: the outputs and the proposals are identical, and
/// each value is the report's value at its pointer.
#[test]
fn outputs_are_identical_on_a_rerun_with_the_same_reports() {
    let typed = r#"
name: typed
protocol: software-change@1
steps:
  - name: read
    kind: Deterministic
    action: repository.inspect
    arguments: {}
    outputs:
      - {name: id, pointer: /call/id, type: String}
      - {name: count, pointer: /count, type: Integer}
      - {name: open, pointer: /open, type: Boolean}
      - {name: whole, pointer: "", type: Json}
      - {name: tags, pointer: /tags, type: StringList}
      - {name: sizes, pointer: /sizes, type: IntegerList}
      - {name: flags, pointer: /flags, type: BooleanList}
      - {name: items, pointer: /items, type: JsonList}
      - {name: first_tag, pointer: /tags/0, type: String}
      - {name: escaped, pointer: /a~1b/c~0d, type: String}
  - name: test
    kind: Deterministic
    action: tests.run
    arguments:
      id: {$step: read, $output: id}
      count: {$step: read, $output: count}
      open: {$step: read, $output: open}
      tags: {$step: read, $output: tags}
      first: {$step: read, $output: first_tag}
      escaped: {$step: read, $output: escaped}
      rest: [{$step: read, $output: sizes}, {$step: read, $output: flags}, {$step: read, $output: items}]
"#;
    let report = object(&[
        ("call", object(&[("id", text("c-17"))])),
        ("count", number("3")),
        ("open", CommissionValue::Bool(true)),
        ("tags", CommissionValue::Array(vec![text("a"), text("b")])),
        (
            "sizes",
            CommissionValue::Array(vec![number("1"), number("-2")]),
        ),
        (
            "flags",
            CommissionValue::Array(vec![CommissionValue::Bool(false)]),
        ),
        (
            "items",
            CommissionValue::Array(vec![CommissionValue::Null, object(&[("k", text("v"))])]),
        ),
        ("a/b", object(&[("c~d", text("escaped"))])),
    ]);
    let first = run(typed, &[(INSPECT, report.clone())]);
    let second = run(typed, &[(INSPECT, report.clone())]);

    assert_eq!(first.performed, [INSPECT, RUN], "{:?}", first.end.outcome);
    assert_eq!(first.outputs, second.outputs);
    assert_eq!(first.proposed(), second.proposed());
    let value = |name: &str| {
        first
            .outputs
            .iter()
            .find(|(step, output, _)| step == "read" && output == name)
            .map(|(_, _, value)| value.clone())
            .unwrap_or_else(|| panic!("no output `{name}`: {:?}", first.outputs))
    };
    assert_eq!(value("id"), text("c-17"));
    assert_eq!(value("count"), number("3"));
    assert_eq!(value("whole"), report);
    assert_eq!(value("first_tag"), text("a"));
    assert_eq!(value("escaped"), text("escaped"));
    assert_eq!(first.outputs.len(), 10);
    assert_eq!(
        first.proposed()[1].1,
        object(&[
            ("id", text("c-17")),
            ("count", number("3")),
            ("open", CommissionValue::Bool(true)),
            ("tags", CommissionValue::Array(vec![text("a"), text("b")])),
            ("first", text("a")),
            ("escaped", text("escaped")),
            (
                "rest",
                CommissionValue::Array(vec![
                    CommissionValue::Array(vec![number("1"), number("-2")]),
                    CommissionValue::Array(vec![CommissionValue::Bool(false)]),
                    CommissionValue::Array(vec![
                        CommissionValue::Null,
                        object(&[("k", text("v"))])
                    ]),
                ])
            ),
        ])
    );
    assert_eq!(first.report.model_calls, 0);
}

/// Each declared type refuses a value of another: a fraction or a string is no `Integer`, a list
/// with one item of another type is no typed list, and `"true"` is no `Boolean`.
#[test]
fn a_value_of_another_type_is_never_taken_as_the_declared_one() {
    let cases: [(&str, CommissionValue); 6] = [
        ("Integer", number("1.5")),
        ("Integer", text("3")),
        ("Boolean", text("true")),
        (
            "StringList",
            CommissionValue::Array(vec![text("a"), number("1")]),
        ),
        ("IntegerList", text("1")),
        ("String", CommissionValue::Null),
    ];
    for (declared, value) in cases {
        let text_of = DEFINITION.replace("type: String}", &format!("type: {declared}}}"));
        let ran = run(&text_of, &[(INSPECT, call_report(value.clone()))]);
        let request = ran.judgment();
        assert_eq!(
            request.member("output"),
            Some(&text("call_id")),
            "{declared} {value:?}"
        );
        assert_eq!(ran.proposed().len(), 1, "{declared} {value:?}");
    }
}
