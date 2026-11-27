// generated from loom v1
// contract digest 93b10ebd872515f89e059d5e64f0981490f97077345b6e02a01f38c191f911d0
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
