// generated from loom v1
// model digest b69befdd806b9d978ce35adaeb5e7178ae55a9e9cf9cbe285f99ea07bf08b31b
// contract digest c97fcdb6564b6e5dd954e22cdbe2b298eacbda14f2adefb10a318d925744ada0
// do not edit: regenerate with `ess synthesize --layout crate`

//! Metering — `loom.metering`.
//!
//! One run's usage report: how many model calls it made, the input, output and cached input tokens the provider reported for them and their cost in millionths of a US dollar at the run's rate card, in total and for each executor call of the run. A cost no rate card priced is absent, never zero.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionUsage — `loom.metering.ActionUsage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionUsage {
    /// `action` — `String`.
    pub action: String,
    /// `trials` — `Integer`.
    pub trials: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `duration_ms` — `Integer`.
    pub duration_ms: i64,
    /// `cost_micro_usd` — `Optional<Integer>`.
    pub cost_micro_usd: Option<i64>,
}

/// CarriedUsage — `loom.metering.CarriedUsage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedUsage {
    /// `steps` — `List<loom.metering.StepUsage>`.
    pub steps: Vec<StepUsage>,
    /// `effects` — `loom.metering.EffectsUsage`.
    pub effects: EffectsUsage,
}

/// EffectsUsage — `loom.metering.EffectsUsage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectsUsage {
    /// `trials` — `Integer`.
    pub trials: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `duration_ms` — `Integer`.
    pub duration_ms: i64,
    /// `cost_micro_usd` — `Optional<Integer>`.
    pub cost_micro_usd: Option<i64>,
    /// `actions` — `List<loom.metering.ActionUsage>`.
    pub actions: Vec<ActionUsage>,
}

/// RunUsageReport — `loom.metering.RunUsageReport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunUsageReport {
    /// `commission_run` — `loom.run.CommissionRunId`.
    pub commission_run: crate::run::CommissionRunId,
    /// `model_calls` — `Integer`.
    pub model_calls: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `cached_input_tokens` — `Integer`.
    pub cached_input_tokens: i64,
    /// `cost_micro_usd` — `Optional<Integer>`.
    pub cost_micro_usd: Option<i64>,
    /// `steps` — `List<loom.metering.StepUsage>`.
    pub steps: Vec<StepUsage>,
    /// `effects` — `loom.metering.EffectsUsage`.
    pub effects: EffectsUsage,
}

/// StepUsage — `loom.metering.StepUsage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepUsage {
    /// `step` — `Integer`.
    pub step: i64,
    /// `label` — `Optional<String>`.
    pub label: Option<String>,
    /// `model_calls` — `Integer`.
    pub model_calls: i64,
    /// `input_tokens` — `Integer`.
    pub input_tokens: i64,
    /// `output_tokens` — `Integer`.
    pub output_tokens: i64,
    /// `cached_input_tokens` — `Integer`.
    pub cached_input_tokens: i64,
    /// `cost_micro_usd` — `Optional<Integer>`.
    pub cost_micro_usd: Option<i64>,
}
