// generated from loom v1
// model digest ebfc92fb53805a9a16de554fd9d3e6c265d63244389bce4ae4d41cfc30d4b1a2
// contract digest fc0dfef86e7a35fb5a515d27ee479e5ae5cf255204bd26277a1352735120b25c
// do not edit: regenerate with `ess synthesize --layout crate`

//! Metering — `loom.metering`.
//!
//! One run's usage report: how many model calls it made, the input, output and cached input tokens the provider reported for them and their cost in millionths of a US dollar at the run's rate card, in total and for each executor call of the run. A cost no rate card priced is absent, never zero.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

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
