// generated from loom v1
// model digest c609715c66a9f4b846295d1ce2efb0b0fc2930d04eddbe2614231f390eca6514
// contract digest 80154eb04deb81857ab390917c869e0ebf32a4d9a025fd65e81597c4092bbbe1
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
}
