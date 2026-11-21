// generated from loom v1
// contract digest d6b5712bb63bf64af1efc3cc5f1bb05481ffc2b09a55f22685830eff5096b3a5
// do not edit: regenerate with `ess synthesize --layout crate`

//! Bindings — `loom.bindings`.
//!
//! The `loom.bindings/1` document: per action of one registered protocol its executor (a program, an agent, a person, the opening of child cases or the copy of a record from another case), the table that turns its result into evidence, the tools an agent may be offered, its budget and timeout, the records that send it back to work and the text shown when a person continues; and the refusals a document earns at decoding and at load against the compiled protocol.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionBudget — `loom.bindings.ActionBudget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBudget {
    /// `max_runs` — `Integer`.
    pub max_runs: i64,
    /// `max_turns` — `Optional<Integer>`.
    pub max_turns: Option<i64>,
    /// `max_tokens` — `Optional<Integer>`.
    pub max_tokens: Option<i64>,
    /// `max_budget_micro_usd` — `Optional<Integer>`.
    pub max_budget_micro_usd: Option<i64>,
}

/// ArtifactPath — `loom.bindings.ArtifactPath`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPath {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `path` — `String`.
    pub path: String,
}

/// BindingsDocument — `loom.bindings.BindingsDocument`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingsDocument {
    /// `format` — `String`.
    pub format: String,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `artifacts` — `List<loom.bindings.ArtifactPath>`.
    pub artifacts: Vec<ArtifactPath>,
    /// `independence` — `List<loom.bindings.ProducerIndependence>`.
    pub independence: Vec<ProducerIndependence>,
    /// `actions` — `List<loom.bindings.BoundAction>`.
    pub actions: Vec<BoundAction>,
}

/// BindingsRefusal — `loom.bindings.BindingsRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingsRefusal {
    /// `reason` — `loom.bindings.BindingsRefusalReason`.
    pub reason: BindingsRefusalReason,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `member` — `String`.
    pub member: String,
    /// `detail` — `String`.
    pub detail: String,
}

/// BindingsRefusalReason — `loom.bindings.BindingsRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingsRefusalReason {
    /// `Malformed`.
    Malformed,
    /// `UndeclaredMember`.
    UndeclaredMember,
    /// `MissingMember`.
    MissingMember,
    /// `UnsupportedFormat`.
    UnsupportedFormat,
    /// `InvalidArgv`.
    InvalidArgv,
    /// `InvalidProtocol`.
    InvalidProtocol,
    /// `InvalidEvidence`.
    InvalidEvidence,
    /// `InvalidBudget`.
    InvalidBudget,
    /// `InvalidToolName`.
    InvalidToolName,
    /// `InvalidEffectResult`.
    InvalidEffectResult,
    /// `ProtocolMismatch`.
    ProtocolMismatch,
    /// `UnknownAction`.
    UnknownAction,
    /// `DuplicateAction`.
    DuplicateAction,
    /// `KindNotProducible`.
    KindNotProducible,
    /// `UndeclaredArtifact`.
    UndeclaredArtifact,
    /// `UndeclaredEvidenceKind`.
    UndeclaredEvidenceKind,
    /// `MemberNotForExecutor`.
    MemberNotForExecutor,
    /// `TimeoutOutOfRange`.
    TimeoutOutOfRange,
    /// `PathNotInWorkspace`.
    PathNotInWorkspace,
    /// `UndeclaredDimension`.
    UndeclaredDimension,
}

/// BoundAction — `loom.bindings.BoundAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundAction {
    /// `action` — `String`.
    pub action: String,
    /// `executor` — `loom.bindings.ExecutorSpec`.
    pub executor: ExecutorSpec,
    /// `arguments` — `Optional<Json>`.
    pub arguments: Option<crate::json::Value>,
    /// `arguments_from` — `Optional<List<String>>`.
    pub arguments_from: Option<Vec<String>>,
    /// `evidence` — `List<loom.bindings.EvidenceEntry>`.
    pub evidence: Vec<EvidenceEntry>,
    /// `tools` — `Optional<loom.bindings.ToolAllowList>`.
    pub tools: Option<ToolAllowList>,
    /// `budget` — `loom.bindings.ActionBudget`.
    pub budget: ActionBudget,
    /// `timeout_seconds` — `Optional<Integer>`.
    pub timeout_seconds: Option<i64>,
    /// `retry_when` — `List<loom.bindings.RetryCondition>`.
    pub retry_when: Vec<RetryCondition>,
    /// `handoff` — `Optional<String>`.
    pub handoff: Option<String>,
}

/// EvidenceEntry — `loom.bindings.EvidenceEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEntry {
    /// `kind` — `String`.
    pub kind: String,
    /// `subject` — `String`.
    pub subject: String,
    /// `producer` — `String`.
    pub producer: String,
    /// `from` — `loom.bindings.ResultSource`.
    pub from: ResultSource,
    /// `pointer` — `Optional<String>`.
    pub pointer: Option<String>,
    /// `results` — `List<loom.bindings.ResultRule>`.
    pub results: Vec<ResultRule>,
    /// `upstream` — `List<String>`.
    pub upstream: Vec<String>,
}

/// Executor — `loom.bindings.Executor`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executor {
    /// `Program`.
    Program,
    /// `Agent`.
    Agent,
    /// `Person`.
    Person,
    /// `Open`.
    Open,
    /// `Copy`.
    Copy,
}

/// ExecutorSpec — `loom.bindings.ExecutorSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorSpec {
    /// `kind` — `loom.bindings.Executor`.
    pub kind: Executor,
    /// `argv` — `Optional<List<String>>`.
    pub argv: Option<Vec<String>>,
    /// `adapter` — `Optional<String>`.
    pub adapter: Option<String>,
    /// `model` — `Optional<String>`.
    pub model: Option<String>,
    /// `inputs` — `Optional<List<String>>`.
    pub inputs: Option<Vec<String>>,
    /// `outputs` — `Optional<List<String>>`.
    pub outputs: Option<Vec<String>>,
    /// `principal` — `Optional<String>`.
    pub principal: Option<String>,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `each` — `Optional<String>`.
    pub each: Option<String>,
    /// `bindings` — `Optional<String>`.
    pub bindings: Option<String>,
    /// `evidence` — `Optional<String>`.
    pub evidence: Option<String>,
    /// `from` — `Optional<String>`.
    pub from: Option<String>,
}

/// ProducerIndependence — `loom.bindings.ProducerIndependence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerIndependence {
    /// `producer` — `String`.
    pub producer: String,
    /// `dimensions` — `List<String>`.
    pub dimensions: Vec<String>,
}

/// ResultRule — `loom.bindings.ResultRule`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRule {
    /// `value` — `String`.
    pub value: String,
    /// `result` — `Optional<String>`.
    pub result: Option<String>,
}

/// ResultSource — `loom.bindings.ResultSource`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSource {
    /// `Exit`.
    Exit,
    /// `Effect`.
    Effect,
    /// `Report`.
    Report,
}

/// RetryCondition — `loom.bindings.RetryCondition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryCondition {
    /// `kind` — `String`.
    pub kind: String,
    /// `results` — `List<String>`.
    pub results: Vec<String>,
}

/// ToolAllowList — `loom.bindings.ToolAllowList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAllowList {
    /// `allow` — `List<String>`.
    pub allow: Vec<String>,
}
