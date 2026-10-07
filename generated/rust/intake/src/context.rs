// generated from intake v1
// model digest 8158f2c6f08736511dded8320855abb57ef13c8b3b492ffef5ed2fd4bb05d87d
// contract digest 6edfc386e180a0eea5e77b3cac17ef9f12f629f4d81c0918f6db81cbb71e4848
// do not edit: regenerate with `ess synthesize --layout crate`

//! context — `intake.context`.
//!
//! Run-local working context owned by the CLI briefing for a single intent. Legacy remains the default; Bounded is opt-in. Intent and instructions are preserved exactly. Working state is derived only from typed execution reports, never parsed from model or artifact text. A test result records its tested revision: passing on revision A never validates later revision B. Compact history events are archived from the first action, with bulk contents retained in the existing result store. The separate archive admits at most 4096 events and 16777216 UTF-8 bytes. Capacity refusal stops subsequent model requests and preserves already completed effects. Bounded context retires old events in batches when a serialized agent request exceeds 49152 bytes or a 64-entry tail would discard history, aiming for 32768 bytes. Every serialized bounded request, including schemas and escaped lookup responses, must fit 65536 bytes; mandatory content that cannot fit is an explicit refusal. A stable prompt prefix changes only at a checkpoint, with changing state and candidates afterward. Both selection and argument generation may list and read archived history using existing result-reference selection semantics, at most eight lookups per stage and always within the request ceiling. Retrieval omits whole records when necessary and never truncates structured data. No summarization model calls are made. Context reports contain measurements only, no source payloads; absent provider counters remain unknown. This contract adds no persisted entity, interactive correction, durable session memory, training export, or governed-loop compaction behavior.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ContextPolicy — `intake.context.ContextPolicy`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPolicy {
    /// `Legacy`.
    Legacy,
    /// `Bounded`.
    Bounded,
}

/// ContextReport — `intake.context.ContextReport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextReport {
    /// `policy` — `intake.context.ContextPolicy`.
    pub policy: ContextPolicy,
    /// `requests` — `List<intake.context.ContextRequestMetric>`.
    pub requests: Vec<ContextRequestMetric>,
    /// `checkpoints` — `Integer`.
    pub checkpoints: i64,
    /// `retrievals` — `Integer`.
    pub retrievals: i64,
    /// `model_calls` — `Integer`.
    pub model_calls: i64,
    /// `elapsed_ms` — `Integer`.
    pub elapsed_ms: i64,
}

/// ContextRequestMetric — `intake.context.ContextRequestMetric`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRequestMetric {
    /// `phase` — `intake.context.RequestPhase`.
    pub phase: RequestPhase,
    /// `request_bytes` — `Integer`.
    pub request_bytes: i64,
    /// `elapsed_ms` — `Integer`.
    pub elapsed_ms: i64,
    /// `final_usage` — `Boolean`.
    pub final_usage: bool,
    /// `input_tokens` — `Optional<Integer>`.
    pub input_tokens: Option<i64>,
    /// `cache_read_tokens` — `Optional<Integer>`.
    pub cache_read_tokens: Option<i64>,
    /// `cache_write_tokens` — `Optional<Integer>`.
    pub cache_write_tokens: Option<i64>,
    /// `output_tokens` — `Optional<Integer>`.
    pub output_tokens: Option<i64>,
}

/// HistoryEvent — `intake.context.HistoryEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEvent {
    /// `sequence` — `Integer`.
    pub sequence: i64,
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `String`.
    pub arguments: String,
    /// `report` — `String`.
    pub report: String,
    /// `artifacts` — `List<intake.results.StoredResult>`.
    pub artifacts: Vec<crate::results::StoredResult>,
}

/// RefusalState — `intake.context.RefusalState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalState {
    /// `action` — `String`.
    pub action: String,
    /// `reason` — `String`.
    pub reason: String,
}

/// RequestPhase — `intake.context.RequestPhase`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPhase {
    /// `Classification`.
    Classification,
    /// `Selection`.
    Selection,
    /// `Arguments`.
    Arguments,
}

/// TestState — `intake.context.TestState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestState {
    /// `exit_code` — `Optional<Integer>`.
    pub exit_code: Option<i64>,
    /// `timed_out` — `Boolean`.
    pub timed_out: bool,
    /// `tested_revision` — `Optional<String>`.
    pub tested_revision: Option<String>,
}

/// WorkingState — `intake.context.WorkingState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingState {
    /// `latest_revision` — `Optional<String>`.
    pub latest_revision: Option<String>,
    /// `latest_test` — `Optional<intake.context.TestState>`.
    pub latest_test: Option<TestState>,
    /// `latest_refusal` — `Optional<intake.context.RefusalState>`.
    pub latest_refusal: Option<RefusalState>,
    /// `recent_artifacts` — `List<intake.results.StoredResult>`.
    pub recent_artifacts: Vec<crate::results::StoredResult>,
}
