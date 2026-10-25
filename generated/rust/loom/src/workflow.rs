// generated from loom v1
// model digest 36d9f391b47bf4ff392161dfbb07aa4606f2481a4cc3a3d4ac33a45bc10d5732
// contract digest 00b46a8d1edd40edd961dc9dcf2afe82e3d81210aec3a4882dbcfa4b17960ae1
// do not edit: regenerate with `ess synthesize --layout crate`

//! Workflow — `loom.workflow`.
//!
//! A Loom-side definition of a workflow over one protocol: an ordered list of steps, each either deterministic (one action with literal arguments, proposed without a model) or a judgement (the actions a model may choose among, with instructions). A step may be conditional on a frontier claim, never on a model. A run's progress over a definition may be kept as a record a store writes after every performed step and a later process resumes from.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// DeterministicStep — `loom.workflow.DeterministicStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicStep {
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `Json`.
    pub arguments: crate::json::Value,
}

/// JudgementStep — `loom.workflow.JudgementStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgementStep {
    /// `actions` — `List<String>`.
    pub actions: Vec<String>,
    /// `instructions` — `String`.
    pub instructions: String,
    /// `inputs` — `List<loom.workflow.OutputReference>`.
    pub inputs: Vec<OutputReference>,
}

/// OutputReference — `loom.workflow.OutputReference`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputReference {
    /// `step` — `String`.
    pub step: String,
    /// `output` — `String`.
    pub output: String,
}

/// OutputType — `loom.workflow.OutputType`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    /// `String`.
    String,
    /// `Integer`.
    Integer,
    /// `Boolean`.
    Boolean,
    /// `Json`.
    Json,
    /// `StringList`.
    StringList,
    /// `IntegerList`.
    IntegerList,
    /// `BooleanList`.
    BooleanList,
    /// `JsonList`.
    JsonList,
}

/// RecordedOutput — `loom.workflow.RecordedOutput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedOutput {
    /// `step` — `String`.
    pub step: String,
    /// `output` — `String`.
    pub output: String,
    /// `value` — `Json`.
    pub value: crate::json::Value,
}

/// StepCondition — `loom.workflow.StepCondition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCondition {
    /// `claim` — `String`.
    pub claim: String,
    /// `value` — `loom.workflow.Truth`.
    pub value: Truth,
}

/// StepKind — `loom.workflow.StepKind`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepKind {
    /// Tagged `Deterministic` — `loom.workflow.DeterministicStep`.
    Deterministic(DeterministicStep),
    /// Tagged `Judgement` — `loom.workflow.JudgementStep`.
    Judgement(JudgementStep),
}

/// StepOutput — `loom.workflow.StepOutput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutput {
    /// `name` — `String`.
    pub name: String,
    /// `pointer` — `String`.
    pub pointer: String,
    /// `type` — `loom.workflow.OutputType`.
    pub r#type: OutputType,
}

/// StepRequest — `loom.workflow.StepRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRequest {
    /// `step` — `String`.
    pub step: String,
    /// `action` — `String`.
    pub action: String,
    /// `arguments` — `Json`.
    pub arguments: crate::json::Value,
}

/// Truth — `loom.workflow.Truth`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    /// `True`.
    True,
    /// `False`.
    False,
    /// `Unknown`.
    Unknown,
}

/// UnproducedOutput — `loom.workflow.UnproducedOutput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnproducedOutput {
    /// `step` — `String`.
    pub step: String,
    /// `output` — `String`.
    pub output: String,
    /// `why` — `String`.
    pub why: String,
}

/// WorkflowDefinition — `loom.workflow.WorkflowDefinition`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDefinition {
    /// `name` — `String`.
    pub name: String,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `steps` — `List<loom.workflow.WorkflowStep>`.
    pub steps: Vec<WorkflowStep>,
}

/// WorkflowRunRecord — `loom.workflow.WorkflowRunRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRecord {
    /// `run` — `loom.run.CommissionRunId`.
    pub run: crate::run::CommissionRunId,
    /// `workflow` — `String`.
    pub workflow: String,
    /// `definition_digest` — `String`.
    pub definition_digest: String,
    /// `performed` — `List<loom.workflow.StepRequest>`.
    pub performed: Vec<StepRequest>,
    /// `proposed` — `Optional<loom.workflow.StepRequest>`.
    pub proposed: Option<StepRequest>,
    /// `outputs` — `List<loom.workflow.RecordedOutput>`.
    pub outputs: Vec<RecordedOutput>,
    /// `unproduced` — `Optional<loom.workflow.UnproducedOutput>`.
    pub unproduced: Option<UnproducedOutput>,
    /// `attempted` — `List<String>`.
    pub attempted: Vec<String>,
    /// `usage` — `loom.metering.CarriedUsage`.
    pub usage: crate::metering::CarriedUsage,
}

/// WorkflowStep — `loom.workflow.WorkflowStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowStep {
    /// `name` — `String`.
    pub name: String,
    /// `when` — `Optional<loom.workflow.StepCondition>`.
    pub when: Option<StepCondition>,
    /// `kind` — `loom.workflow.StepKind`.
    pub kind: StepKind,
    /// `outputs` — `List<loom.workflow.StepOutput>`.
    pub outputs: Vec<StepOutput>,
}
