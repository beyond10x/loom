// generated from loom v1
// model digest 36d9f391b47bf4ff392161dfbb07aa4606f2481a4cc3a3d4ac33a45bc10d5732
// contract digest 00b46a8d1edd40edd961dc9dcf2afe82e3d81210aec3a4882dbcfa4b17960ae1
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
