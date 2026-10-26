// generated from loom v1
// model digest d9155034aef9f4b4212f9b6a3ffc88316cd8ab2a572b6ef5cecf23eba5d95f3f
// contract digest af385912b2000394d6b4bb57552f401a93a9cf2bea3600aa333fdba397fdf1aa
// do not edit: regenerate with `ess synthesize --layout crate`

//! Models — `loom.models`.
//!
//! The models configuration of a host: the path of an `llm.routes/1` document and, for each role Loom asks a model for (the agent and the classifier), the model that serves it, named by a route or by an endpoint and model id of that document.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ModelRole — `loom.models.ModelRole`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    /// `agent`.
    Agent,
    /// `classifier`.
    Classifier,
}

/// ModelsConfig — `loom.models.ModelsConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsConfig {
    /// `routes` — `String`.
    pub routes: String,
    /// `roles` — `List<loom.models.RoleModel>`.
    pub roles: Vec<RoleModel>,
}

/// RoleModel — `loom.models.RoleModel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleModel {
    /// `role` — `loom.models.ModelRole`.
    pub role: ModelRole,
    /// `model` — `String`.
    pub model: String,
}
