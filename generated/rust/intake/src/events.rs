// generated from intake v1
// model digest 03ca6d3152cdf9e92b78bf5ba33c285e6da58b46352672d4f2efc355fb35043a
// contract digest ca8ccdc30704d0dff46a99a90c7cb17156bb786caa37ba194a7b9e3f80caed97
// do not edit: regenerate with `ess synthesize --layout crate`

//! Run events — `intake.events`.
//!
//! The machine-readable record of one b10x-loom run, written as one JSON object per line on standard output when the operator asks for it. Every line carries the stream's schema version. The records project what the run already does: the route it took, each model turn, each tool call a model made, the usage a provider reported, each agent session metaharness ran for it, named by its trace-ir/1 digest and summarized, the approvals the run stopped for, and exactly one terminal record, last, with the stop reason and the process exit status. A record adds no authority and is not evidence.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ApprovalEvent — `intake.events.ApprovalEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalEvent {
    /// `actions` — `List<String>`.
    pub actions: Vec<String>,
}

/// RouteEvent — `intake.events.RouteEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEvent {
    /// `protocol` — `String`.
    pub protocol: String,
    /// `accepted` — `Boolean`.
    pub accepted: bool,
}

/// RunEvent — `intake.events.RunEvent`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    /// Tagged `Approval` — `intake.events.ApprovalEvent`.
    Approval(ApprovalEvent),
    /// Tagged `Route` — `intake.events.RouteEvent`.
    Route(RouteEvent),
    /// Tagged `Session` — `intake.events.SessionEvent`.
    Session(SessionEvent),
    /// Tagged `Terminal` — `intake.events.TerminalEvent`.
    Terminal(TerminalEvent),
    /// Tagged `ToolCall` — `intake.events.ToolCallEvent`.
    ToolCall(ToolCallEvent),
    /// Tagged `Turn` — `intake.events.TurnEvent`.
    Turn(TurnEvent),
    /// Tagged `Usage` — `intake.events.UsageEvent`.
    Usage(UsageEvent),
}

/// RunEventLine — `intake.events.RunEventLine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEventLine {
    /// `schema_version` — `intake.events.SchemaVersion`.
    pub schema_version: SchemaVersion,
    /// `event` — `intake.events.RunEvent`.
    pub event: RunEvent,
}

/// SchemaVersion — `intake.events.SchemaVersion`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVersion {
    /// `V1`.
    V1,
}

/// SessionEvent — `intake.events.SessionEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvent {
    /// `transcript_digest` — `Optional<String>`.
    pub transcript_digest: Option<String>,
    /// `source` — `Optional<String>`.
    pub source: Option<String>,
    /// `adapter` — `Optional<String>`.
    pub adapter: Option<String>,
    /// `model` — `Optional<String>`.
    pub model: Option<String>,
    /// `models` — `Optional<List<intake.events.SessionModel>>`.
    pub models: Option<Vec<SessionModel>>,
    /// `turns` — `Optional<Integer>`.
    pub turns: Option<i64>,
    /// `tokens_basis` — `Optional<String>`.
    pub tokens_basis: Option<String>,
    /// `input_tokens` — `Optional<Integer>`.
    pub input_tokens: Option<i64>,
    /// `output_tokens` — `Optional<Integer>`.
    pub output_tokens: Option<i64>,
    /// `cache_read_input_tokens` — `Optional<Integer>`.
    pub cache_read_input_tokens: Option<i64>,
    /// `cache_creation_input_tokens` — `Optional<Integer>`.
    pub cache_creation_input_tokens: Option<i64>,
    /// `cost_usd` — `Optional<Decimal>`.
    pub cost_usd: Option<crate::primitives::Decimal>,
    /// `duration_ms` — `Optional<Integer>`.
    pub duration_ms: Option<i64>,
    /// `duration_api_ms` — `Optional<Integer>`.
    pub duration_api_ms: Option<i64>,
    /// `first_timestamp_ms` — `Optional<Integer>`.
    pub first_timestamp_ms: Option<i64>,
    /// `last_timestamp_ms` — `Optional<Integer>`.
    pub last_timestamp_ms: Option<i64>,
    /// `is_error` — `Optional<Boolean>`.
    pub is_error: Option<bool>,
    /// `stop_reason` — `Optional<String>`.
    pub stop_reason: Option<String>,
    /// `tool_calls` — `Optional<List<intake.events.SessionToolCall>>`.
    pub tool_calls: Option<Vec<SessionToolCall>>,
    /// `unk` — `Optional<Integer>`.
    pub unk: Option<i64>,
    /// `projection_error` — `Optional<String>`.
    pub projection_error: Option<String>,
}

/// SessionModel — `intake.events.SessionModel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionModel {
    /// `name` — `String`.
    pub name: String,
    /// `input_tokens` — `Optional<Integer>`.
    pub input_tokens: Option<i64>,
    /// `output_tokens` — `Optional<Integer>`.
    pub output_tokens: Option<i64>,
    /// `cache_read_input_tokens` — `Optional<Integer>`.
    pub cache_read_input_tokens: Option<i64>,
    /// `cache_creation_input_tokens` — `Optional<Integer>`.
    pub cache_creation_input_tokens: Option<i64>,
    /// `cost_usd` — `Optional<Decimal>`.
    pub cost_usd: Option<crate::primitives::Decimal>,
}

/// SessionToolCall — `intake.events.SessionToolCall`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionToolCall {
    /// `call_id` — `Optional<String>`.
    pub call_id: Option<String>,
    /// `name` — `Optional<String>`.
    pub name: Option<String>,
    /// `answered` — `Boolean`.
    pub answered: bool,
    /// `error` — `Optional<Boolean>`.
    pub error: Option<bool>,
}

/// TerminalEvent — `intake.events.TerminalEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEvent {
    /// `stop_reason` — `Optional<intake.routing.StopReason>`.
    pub stop_reason: Option<crate::routing::StopReason>,
    /// `exit_status` — `Integer`.
    pub exit_status: i64,
    /// `protocol` — `Optional<String>`.
    pub protocol: Option<String>,
    /// `steps` — `Optional<Integer>`.
    pub steps: Option<i64>,
    /// `error` — `Optional<String>`.
    pub error: Option<String>,
    /// `unperformed_actions` — `Optional<List<String>>`.
    pub unperformed_actions: Option<Vec<String>>,
}

/// ToolCallEvent — `intake.events.ToolCallEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallEvent {
    /// `turn` — `Integer`.
    pub turn: i64,
    /// `call_id` — `String`.
    pub call_id: String,
    /// `name` — `String`.
    pub name: String,
    /// `arguments` — `String`.
    pub arguments: String,
}

/// TurnEvent — `intake.events.TurnEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnEvent {
    /// `turn` — `Integer`.
    pub turn: i64,
    /// `phase` — `intake.context.RequestPhase`.
    pub phase: crate::context::RequestPhase,
    /// `model` — `String`.
    pub model: String,
    /// `turn_stop` — `String`.
    pub turn_stop: String,
}

/// UsageEvent — `intake.events.UsageEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageEvent {
    /// `turn` — `Integer`.
    pub turn: i64,
    /// `model` — `String`.
    pub model: String,
    /// `input_tokens` — `Optional<Integer>`.
    pub input_tokens: Option<i64>,
    /// `output_tokens` — `Optional<Integer>`.
    pub output_tokens: Option<i64>,
    /// `cached_input_tokens` — `Optional<Integer>`.
    pub cached_input_tokens: Option<i64>,
    /// `cache_creation_input_tokens` — `Optional<Integer>`.
    pub cache_creation_input_tokens: Option<i64>,
    /// `reasoning_output_tokens` — `Optional<Integer>`.
    pub reasoning_output_tokens: Option<i64>,
    /// `final_usage` — `Boolean`.
    pub final_usage: bool,
}
