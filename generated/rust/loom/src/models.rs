// generated from loom v1
// model digest b774beb53c36941f484cfdcfb927b46287a34f934f9b43c3d7e49306ebe94f34
// contract digest 7b9385bd4805d85156d0039e947a53ea05aeda4435f2bebd60af8072091657ce
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
