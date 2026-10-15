// generated from loom v1
// model digest 7e01de49f6648bd42c3487572f5bf305ca3e9c2e76adeb528dd2ffcdffa645c9
// contract digest 339d7d1174a6fc6f5384f70ff5342398aa19ad068e03cd1146e66a670161b962
// do not edit: regenerate with `ess synthesize --layout crate`

//! Workflow — `loom.workflow`.
//!
//! A Loom-side definition of a workflow over one protocol: an ordered list of steps, each either deterministic (one action with literal arguments, proposed without a model) or a judgement (the actions a model may choose among, with instructions). A step may be conditional on a frontier claim, never on a model.
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
