// generated from loom v1
// model digest 748fceb21cf7ee6d4e3507a439a01eec6e7e9190c5bee46f896cc1ad49fa1326
// contract digest 46c4d4d570ca4756a9a8caf1cdfa2d49a60fe40d986d3e54b1dd2ba9a7b18d06
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
