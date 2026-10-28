// generated from loom v1
// model digest 66a02a0baf5fbe8890612e69df450544c958c9f01799dc162cea2a954a89a58c
// contract digest 70e66c4d4dd22d9a004633e14def9f35e93b84c0782021c37bc41550f5e18b6a
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
