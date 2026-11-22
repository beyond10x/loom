// generated from loom v1
// contract digest 4ad6b9940a1b56fdeb9599781b32186cc78703b8456ee8f2c325d2896eb13323
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

/// ArtifactPlacement — `loom.bindings.ArtifactPlacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPlacement {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `path` — `String`.
    pub path: String,
    /// `revision` — `String`.
    pub revision: String,
}

/// ArtifactRevision — `loom.bindings.ArtifactRevision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRevision {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `revision` — `String`.
    pub revision: String,
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

/// BoundStep — `loom.bindings.BoundStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundStep {
    /// `action` — `String`.
    pub action: String,
    /// `outcome` — `loom.bindings.StepOutcome`.
    pub outcome: StepOutcome,
    /// `discarded` — `Optional<loom.bindings.DiscardReason>`.
    pub discarded: Option<DiscardReason>,
    /// `placements` — `List<loom.bindings.ArtifactPlacement>`.
    pub placements: Vec<ArtifactPlacement>,
    /// `moves` — `List<loom.bindings.RevisionMove>`.
    pub moves: Vec<RevisionMove>,
}

/// DiscardReason — `loom.bindings.DiscardReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardReason {
    /// `ChangedByNonWriter`.
    ChangedByNonWriter,
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

/// ProgramEnvironment — `loom.bindings.ProgramEnvironment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramEnvironment {
    /// `path` — `Optional<String>`.
    pub path: Option<String>,
    /// `loom_case` — `String`.
    pub loom_case: String,
    /// `loom_retry_feedback` — `Optional<String>`.
    pub loom_retry_feedback: Option<String>,
}

/// ProgramRefusal — `loom.bindings.ProgramRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramRefusal {
    /// `reason` — `loom.bindings.ProgramRefusalReason`.
    pub reason: ProgramRefusalReason,
    /// `action` — `String`.
    pub action: String,
    /// `detail` — `String`.
    pub detail: String,
}

/// ProgramRefusalReason — `loom.bindings.ProgramRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramRefusalReason {
    /// `ResultNotInTable`.
    ResultNotInTable,
    /// `ReportUnreadable`.
    ReportUnreadable,
    /// `ArgumentsUnreadable`.
    ArgumentsUnreadable,
    /// `RunsSpent`.
    RunsSpent,
    /// `CaseOfAnotherProtocol`.
    CaseOfAnotherProtocol,
    /// `ArtifactUnreadable`.
    ArtifactUnreadable,
}

/// ProgramRun — `loom.bindings.ProgramRun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramRun {
    /// `case` — `String`.
    pub case: String,
    /// `action` — `String`.
    pub action: String,
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `blob_sha256` — `Optional<String>`.
    pub blob_sha256: Option<String>,
    /// `timed_out` — `Boolean`.
    pub timed_out: bool,
    /// `runner_refusal` — `Optional<String>`.
    pub runner_refusal: Option<String>,
    /// `retry_of` — `List<String>`.
    pub retry_of: Vec<String>,
}

/// RecordedEvidence — `loom.bindings.RecordedEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedEvidence {
    /// `kind` — `String`.
    pub kind: String,
    /// `subject` — `String`.
    pub subject: String,
    /// `producer` — `String`.
    pub producer: String,
    /// `result` — `Optional<String>`.
    pub result: Option<String>,
    /// `subject_revision` — `String`.
    pub subject_revision: String,
    /// `observed_at` — `Timestamp`.
    pub observed_at: crate::primitives::Timestamp,
    /// `upstream_revisions` — `List<loom.bindings.ArtifactRevision>`.
    pub upstream_revisions: Vec<ArtifactRevision>,
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

/// RetryFeedback — `loom.bindings.RetryFeedback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryFeedback {
    /// `format` — `String`.
    pub format: String,
    /// `case` — `String`.
    pub case: String,
    /// `action` — `String`.
    pub action: String,
    /// `records` — `List<loom.bindings.RetryRecord>`.
    pub records: Vec<RetryRecord>,
    /// `logs` — `List<loom.bindings.RetryLog>`.
    pub logs: Vec<RetryLog>,
}

/// RetryLog — `loom.bindings.RetryLog`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryLog {
    /// `blob_sha256` — `String`.
    pub blob_sha256: String,
    /// `tail` — `String`.
    pub tail: String,
}

/// RetryRecord — `loom.bindings.RetryRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryRecord {
    /// `evidence_id` — `String`.
    pub evidence_id: String,
    /// `kind` — `String`.
    pub kind: String,
    /// `subject` — `String`.
    pub subject: String,
    /// `subject_revision` — `String`.
    pub subject_revision: String,
    /// `result` — `String`.
    pub result: String,
    /// `producer` — `String`.
    pub producer: String,
    /// `observed_at` — `Timestamp`.
    pub observed_at: crate::primitives::Timestamp,
    /// `action` — `Optional<String>`.
    pub action: Option<String>,
    /// `exit` — `Optional<Integer>`.
    pub exit: Option<i64>,
    /// `blob_sha256` — `Optional<String>`.
    pub blob_sha256: Option<String>,
}

/// RevisionMove — `loom.bindings.RevisionMove`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionMove {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `from` — `String`.
    pub from: String,
    /// `to` — `String`.
    pub to: String,
}

/// RevisionRefusal — `loom.bindings.RevisionRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionRefusal {
    /// `reason` — `loom.bindings.RevisionRefusalReason`.
    pub reason: RevisionRefusalReason,
    /// `artifact` — `String`.
    pub artifact: String,
    /// `path` — `String`.
    pub path: String,
    /// `detail` — `String`.
    pub detail: String,
}

/// RevisionRefusalReason — `loom.bindings.RevisionRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionRefusalReason {
    /// `OutsideWorkspace`.
    OutsideWorkspace,
    /// `TooLarge`.
    TooLarge,
    /// `ChangedWhileRead`.
    ChangedWhileRead,
    /// `Unreadable`.
    Unreadable,
}

/// StepOutcome — `loom.bindings.StepOutcome`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// `Recorded`.
    Recorded,
    /// `Discarded`.
    Discarded,
}

/// ToolAllowList — `loom.bindings.ToolAllowList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAllowList {
    /// `allow` — `List<String>`.
    pub allow: Vec<String>,
}
