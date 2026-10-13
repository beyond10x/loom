// generated from intake v1
// model digest 19ac3d8d1db2341a963fcfb8d85843ba6e1da6d764dc7e98dd4d43eed5077883
// contract digest 09dd5b3cae539d5e002453702e5eec93d09e4697eecb6cd2f7a112ab5e12c81c
// do not edit: regenerate with `ess synthesize --layout crate`

//! Run events — `intake.events`.
//!
//! The machine-readable record of one b10x-loom run, written as one JSON object per line on standard output when the operator asks for it. Every line carries the stream's schema version. The records project what the run already does: the route it took, each model turn, each tool call a model made, the usage a provider reported, the approvals the run stopped for, and exactly one terminal record, last, with the stop reason and the process exit status. A record adds no authority and is not evidence.
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
