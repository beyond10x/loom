//! What the plugin host's tests share: a fixture directory with the fake `connectors` of
//! `loom-connectors`, a recorded classifier model, a scripted model port for turns, a fake plugin,
//! and a scripted executor for the effect port alone. No model, network or credential is reached.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use b10x_loom_plugin::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, DataSource, OperationId, SourceName,
};
use b10x_loom_plugin::{
    Classification, Cursor, Decimal, Host, InboundItem, Intent, ItemId, Objective, Plugin,
    PluginConfig, PluginError, PluginState, Poll, TurnModel,
};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item as ModelItem, Model, Protocol,
    Provenance, StopReason, StreamSink, ToolCall, ToolName, TurnObservation, TurnOutcome,
    TurnRequest,
};
use loom_sdk::commission::model::behaviour::Generated;
use loom_sdk::commission::model::json::{self, Value};
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, Unit, commission_state, frontier_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::executor::AgentExecutor;
use loom_sdk::commission::runtime::LoopEnd;
use loom_sdk::loom::harness::responses;
use loom_sdk::loom::harness::wire as port;
use loom_sdk::{CanonGovernor, EffectPort, LoopContext, MemoryCaseStore, ProtocolCatalog};
use serde_json::json;

pub const ADAPTER: &str = "chat";
pub const CONNECTION: &str = "conn-fixture";
pub const LIST: &str = "channel.history";
pub const SEARCH: &str = "message.search";
pub const GET: &str = "message.get";
pub const MODEL: &str = "recorded-model";

/// The fixtures of `loom-connectors`, reused by path.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../loom-connectors/tests/fixtures/connectors")
}

/// One test's directory: the fake program, its `--state-dir` (`connectors-state`), `HOME`, and a
/// directory for the plugin's own state (`plugin-state`).
pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    pub fn new(suite: &str, test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("loom_plugin")
            .join(suite)
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["connectors-state", "home"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let fake = root.join("connectors");
        fs::copy(fixtures().join("fake-connectors"), &fake).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let fixture = Self { root };
        fixture.answer("describe", LIST, "describe-list.json");
        fixture.answer("describe", SEARCH, "describe-search.json");
        fixture.answer("describe", GET, "describe-get.json");
        fixture.answer("invoke", LIST, "invoke-read.json");
        fixture.answer("invoke", SEARCH, "invoke-search.json");
        fixture.answer("invoke", GET, "invoke-get.json");
        fixture
    }

    pub fn connectors_state(&self) -> PathBuf {
        self.root.join("connectors-state")
    }

    pub fn plugin_state(&self) -> PathBuf {
        self.root.join("plugin-state")
    }

    /// `operations <verb>` of `operation` answers with the fixture `file`.
    pub fn answer(&self, verb: &str, operation: &str, file: &str) {
        fs::copy(
            fixtures().join(file),
            self.connectors_state()
                .join(format!("{verb}.{operation}.json")),
        )
        .unwrap();
    }

    /// `operations <verb>` exits with `status`.
    pub fn exit(&self, verb: &str, status: i32) {
        fs::write(
            self.connectors_state().join(format!("{verb}.exit")),
            status.to_string(),
        )
        .unwrap();
    }

    pub fn cli_config(&self) -> ConnectorsCliConfig {
        ConnectorsCliConfig {
            program: self.root.join("connectors").display().to_string(),
            config: None,
            state_dir: Some(self.connectors_state().display().to_string()),
            timeout_seconds: None,
        }
    }

    /// A configuration with `sources`, threshold 0.5, interval 0 and no roots or checkouts.
    pub fn config(&self, sources: Vec<DataSource>) -> PluginConfig {
        PluginConfig {
            connectors: self.cli_config(),
            sources,
            objectives: vec![
                Objective {
                    name: "answered".to_owned(),
                    weight: Decimal("0.7".to_owned()),
                },
                Objective {
                    name: "fresh".to_owned(),
                    weight: Decimal("0.3".to_owned()),
                },
            ],
            classify_threshold: Some(Decimal("0.5".to_owned())),
            poll_interval_seconds: 0,
            workspace_roots: Vec::new(),
            checkouts: Vec::new(),
        }
    }

    /// The environment the host hands the fake: `HOME` isolated, a plain `PATH`.
    pub fn environment(&self) -> Vec<(String, String)> {
        vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ]
    }

    pub fn host<'a>(&self, config: &'a PluginConfig) -> Host<'a> {
        Host::new(config).with_environment(self.environment())
    }

    /// Every run's argv, one entry per run.
    pub fn argv(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.connectors_state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.strip_suffix('\t')
                    .unwrap_or(line)
                    .split('\t')
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    /// The runs whose verb is `operations <verb>`.
    pub fn runs(&self, verb: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.windows(2)
                    .any(|w| w[0] == "operations" && w[1] == verb)
            })
            .collect()
    }
}

/// The source `chat`: list, search and get.
pub fn chat() -> DataSource {
    DataSource {
        name: SourceName("chat".to_owned()),
        adapter: AdapterAlias(ADAPTER.to_owned()),
        connection: ConnectionId(CONNECTION.to_owned()),
        list: Some(OperationId(LIST.to_owned())),
        search: Some(OperationId(SEARCH.to_owned())),
        get: Some(OperationId(GET.to_owned())),
    }
}

/// The source `chat` declaring list and get only.
pub fn chat_without_search() -> DataSource {
    DataSource {
        search: None,
        ..chat()
    }
}

/// A second source, `wiki`, declaring list only.
pub fn wiki() -> DataSource {
    DataSource {
        name: SourceName("wiki".to_owned()),
        adapter: AdapterAlias(ADAPTER.to_owned()),
        connection: ConnectionId(CONNECTION.to_owned()),
        list: Some(OperationId(LIST.to_owned())),
        search: None,
        get: None,
    }
}

pub fn item(id: &str) -> InboundItem {
    InboundItem {
        id: ItemId(id.to_owned()),
        revision: "1700000000.000100".to_owned(),
        text: "Is the deploy finished?".to_owned(),
        details: loom::json::Value::Object(vec![
            (
                "channel".to_owned(),
                loom::json::Value::Text("C0FIXTURE1".to_owned()),
            ),
            (
                "user".to_owned(),
                loom::json::Value::Text("U0ALICE".to_owned()),
            ),
        ]),
    }
}

pub fn classification(intent: Intent, hints: &[&str], confidence: &str) -> Classification {
    Classification {
        intent,
        hints: hints.iter().map(|hint| (*hint).to_owned()).collect(),
        confidence: Decimal(confidence.to_owned()),
    }
}

// ---- the recorded classifier ---------------------------------------------------------------------

/// A recorded classifier model: answers each turn with one call of the tool the request forces,
/// with the arguments recorded for that tool, and keeps every request it saw.
pub struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    answers: Vec<(String, serde_json::Value)>,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Recorded {
    pub fn answering(answers: Vec<(&str, serde_json::Value)>) -> Self {
        let id = |value: &str| Id::new(value).expect("fixture identifier");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("recorded"),
                endpoint: id("recorded"),
                model: id(MODEL),
                binding_revision: id("rev-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                temperature: false,
                top_p: false,
                reasoning_efforts: Vec::new(),
                context_window: 32_768,
                max_output_tokens: 2_048,
            },
            answers: answers
                .into_iter()
                .map(|(tool, arguments)| (tool.to_owned(), arguments))
                .collect(),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// A classifier answering `classify_item` with `intent`, `hints` and `confidence`.
    pub fn classifying(intent: &str, hints: &[&str], confidence: f64) -> Self {
        Self::answering(vec![(
            "classify_item",
            json!({"intent": intent, "hints": hints, "confidence": confidence}),
        )])
    }

    pub fn requests(&self) -> Vec<TurnRequest> {
        self.seen.lock().unwrap().clone()
    }
}

impl Model for Recorded {
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
        self.seen.lock().unwrap().push(request.clone());
        let tool = request
            .tools
            .first()
            .map(|tool| tool.name.as_str().to_owned())
            .unwrap_or_default();
        let arguments = self
            .answers
            .iter()
            .find(|(name, _)| *name == tool)
            .map(|(_, arguments)| arguments.clone())
            .unwrap_or_else(|| panic!("no recorded answer for the tool `{tool}`"));
        let mut observation = TurnObservation::new(self.provenance.clone());
        observation.final_usage = true;
        let outcome = TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![ModelItem::ToolCall(ToolCall {
                call_id: CallId::new("call_1").expect("call id"),
                name: ToolName::new(&tool).expect("tool name"),
                arguments,
            })],
            observation,
        };
        Box::pin(async move { Ok(outcome) })
    }
}

// ---- the scripted turn model ---------------------------------------------------------------------

/// What a scripted port saw, shared with the test after the port is handed away.
pub type Seen = Arc<Mutex<Vec<port::TurnRequest>>>;

/// A scripted model port of Loom's model loop: each turn answers the next call of its script and
/// records the request. Past the script, it fails the turn.
pub struct Scripted {
    wire: port::WireId,
    script: VecDeque<(String, serde_json::Value)>,
    seen: Seen,
}

impl Scripted {
    pub fn new(script: Vec<(&str, serde_json::Value)>) -> (Self, Seen) {
        let seen = Seen::default();
        (
            Self {
                wire: port::WireId::new(responses::WIRE).expect("valid"),
                script: script
                    .into_iter()
                    .map(|(tool, arguments)| (tool.to_owned(), arguments))
                    .collect(),
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }

    pub fn into_turn_model<'a>(self) -> TurnModel<'a> {
        TurnModel {
            port: Box::new(self),
            model: MODEL.to_owned(),
        }
    }
}

impl port::ModelPort for Scripted {
    fn wire(&self) -> &port::WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &port::TurnRequest,
        _sink: &mut dyn port::StreamSink,
    ) -> Result<port::TurnOutcome, port::WireError> {
        self.seen.lock().unwrap().push(request.clone());
        let (tool, arguments) = self
            .script
            .pop_front()
            .ok_or_else(|| port::WireError::protocol("the script has no further turn"))?;
        Ok(port::TurnOutcome {
            stop_reason: port::StopReason::ToolCalls,
            items: vec![port::Item::ToolCall(port::ToolCall {
                call_id: port::CallId::new(format!("call_{}", self.seen.lock().unwrap().len()))
                    .expect("valid"),
                name: port::ToolName::new(tool).expect("valid"),
                arguments,
            })],
            usage: None,
        })
    }
}

/// Every string anywhere in `value`, one per line.
pub fn strings(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(text) => {
            out.push_str(text);
            out.push('\n');
        }
        serde_json::Value::Array(values) => values.iter().for_each(|value| strings(value, out)),
        serde_json::Value::Object(fields) => fields.values().for_each(|value| strings(value, out)),
        _ => {}
    }
}

/// Every string a turn request carries.
pub fn request_text(request: &port::TurnRequest) -> String {
    let mut text = String::new();
    strings(
        &serde_json::to_value(request).expect("a request encodes"),
        &mut text,
    );
    text
}

// ---- the fake plugin -----------------------------------------------------------------------------

/// A fake plugin: its poll answers `items` and a cursor naming the poll's number, its classifier is
/// recorded, and each turn takes the next script of `turns`. It counts polls and turn models, keeps
/// the objectives each poll was given, and sets `stop` on poll `stop_at`.
pub struct FakePlugin {
    pub items: Vec<InboundItem>,
    pub classifier: Recorded,
    pub turns: Mutex<VecDeque<Vec<(&'static str, serde_json::Value)>>>,
    pub polls: AtomicUsize,
    pub turn_models: AtomicUsize,
    pub objectives: Mutex<Vec<Vec<Objective>>>,
    pub stop_at: Option<(usize, Arc<AtomicBool>)>,
}

impl FakePlugin {
    pub fn new(items: Vec<InboundItem>, classifier: Recorded) -> Self {
        Self {
            items,
            classifier,
            turns: Mutex::new(VecDeque::new()),
            polls: AtomicUsize::new(0),
            turn_models: AtomicUsize::new(0),
            objectives: Mutex::new(Vec::new()),
            stop_at: None,
        }
    }

    pub fn with_turn(self, script: Vec<(&'static str, serde_json::Value)>) -> Self {
        self.turns.lock().unwrap().push_back(script);
        self
    }
}

impl Plugin for FakePlugin {
    fn name(&self) -> &str {
        "fake"
    }

    fn poll(
        &self,
        _host: &Host<'_>,
        _state: &PluginState,
        objectives: &[Objective],
    ) -> Result<Poll, PluginError> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst) + 1;
        self.objectives.lock().unwrap().push(objectives.to_vec());
        if let Some((at, stop)) = &self.stop_at
            && *at == n
        {
            stop.store(true, Ordering::SeqCst);
        }
        Ok(Poll {
            items: self.items.clone(),
            cursors: vec![Cursor {
                name: "C0FIXTURE1".to_owned(),
                value: format!("poll-{n}"),
            }],
        })
    }

    fn classifier(&self) -> Result<&dyn Model, PluginError> {
        Ok(&self.classifier)
    }

    fn turn_model(&self) -> Result<TurnModel<'_>, PluginError> {
        self.turn_models.fetch_add(1, Ordering::SeqCst);
        let script = self
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| PluginError::Turn("the fake has no further turn".to_owned()))?;
        Ok(Scripted::new(script).0.into_turn_model())
    }
}

// ---- the effect port alone -----------------------------------------------------------------------

/// An executor that proposes its script's actions in order, then nothing.
pub struct Proposes {
    script: Mutex<VecDeque<(String, Value)>>,
    pub calls: AtomicUsize,
}

impl Proposes {
    pub fn new(script: Vec<(&str, serde_json::Value)>) -> Self {
        Self {
            script: Mutex::new(
                script
                    .into_iter()
                    .map(|(action, arguments)| {
                        (
                            action.to_owned(),
                            json::parse(&arguments.to_string()).expect("JSON"),
                        )
                    })
                    .collect(),
            ),
            calls: AtomicUsize::new(0),
        }
    }
}

impl AgentExecutor for Proposes {
    fn run(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.script.lock().unwrap().pop_front() {
            Some((action, arguments)) => {
                ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                    action,
                    arguments: ProposedActionArguments(arguments),
                })
            }
            None => ExecutorOutcome::NoUsefulAction(Unit(true)),
        }
    }
}

/// The plugin catalog's governor, with a case on `inbound-answer@1` about item revision `r1`.
pub fn governed() -> (CanonGovernor<MemoryCaseStore>, CaseId) {
    let governor = CanonGovernor::new(MemoryCaseStore::default())
        .with_catalog(&ProtocolCatalog::plugins().expect("the plugin catalog"))
        .expect("the governor admits it");
    let case = governor
        .open(
            "inbound-answer@1",
            [("item".to_owned(), "1700000000.000100".to_owned())].into(),
        )
        .expect("the case opens");
    (governor, case)
}

struct Steps {
    next: u64,
}

impl LoopContext for Steps {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.next += 1;
        ActionRequestId(uuid(0x100 + self.next))
    }
    fn observation_id(&mut self) -> ObservationId {
        self.next += 1;
        ObservationId(uuid(0x200 + self.next))
    }
    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-09T12:00:00Z".to_owned())
    }
    fn step_budget(&self) -> Option<usize> {
        Some(4)
    }
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// One run of Commission's loop on `case`: `executor` proposes, [`PluginAuthority`] decides and
/// `effects` performs.
pub fn run_once<F: EffectPort>(
    governor: &CanonGovernor<MemoryCaseStore>,
    case: &CaseId,
    executor: &Proposes,
    effects: &F,
) -> LoopEnd {
    let runs = AtomicU64::new(0x300);
    let mut store = Generated::new(RunStore::new(move || {
        RunId(uuid(runs.fetch_add(1, Ordering::SeqCst)))
    }));
    let commission = Commission::<commission_state::Assigned>::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: case.clone(),
        principal: PrincipalId("loom-plugin".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    loom_sdk::run_until_blocked(
        governor,
        executor,
        &b10x_loom_plugin::PluginAuthority,
        effects,
        &commission,
        &mut store,
        &mut Steps { next: 0 },
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"))
}
