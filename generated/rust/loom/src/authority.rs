// generated from loom v1
// model digest 9ae2c9527af772aefdac8e5f0ed3e249fa61378afce19d33e99b5df30cab38c3
// contract digest ffc963a189f2a35f008d4a5896fbac5abf5a6e082000da4c37354932f6d557f3
// do not edit: regenerate with `ess synthesize --layout crate`

//! Authority — `loom.authority`.
//!
//! One authority decision taken by Mandate for Commission: the context the host supplies in the commission's authority context, and how each outcome of Mandate's check, and each failure to ask it, maps to Commission's answer. A failure grants nothing.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// Answer — `loom.authority.Answer`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// `Allow`.
    Allow,
    /// `ApprovalRequired`.
    ApprovalRequired,
    /// `Deny`.
    Deny,
    /// `Undecided`.
    Undecided,
}

/// CaseResource — `loom.authority.CaseResource`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseResource {
    /// `Case`.
    Case,
}

/// HostContext — `loom.authority.HostContext`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostContext {
    /// `verified` — `Json`.
    pub verified: crate::json::Value,
}

/// MandateHost — `loom.authority.MandateHost`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandateHost {
    /// `verified` — `Json`.
    pub verified: crate::json::Value,
    /// `approver` — `Uuid`.
    pub approver: crate::primitives::Uuid,
    /// `approver_policy` — `Uuid`.
    pub approver_policy: crate::primitives::Uuid,
    /// `relation` — `String`.
    pub relation: String,
    /// `capabilities` — `List<String>`.
    pub capabilities: Vec<String>,
    /// `approval_required` — `List<String>`.
    pub approval_required: Vec<String>,
    /// `approval_seconds` — `Integer`.
    pub approval_seconds: i64,
}

/// Outcome — `loom.authority.Outcome`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// `Allowed`.
    Allowed,
    /// `ApprovalRequired`.
    ApprovalRequired,
    /// `Denied`.
    Denied,
    /// `InvalidCredential`.
    InvalidCredential,
    /// `TenantMismatch`.
    TenantMismatch,
    /// `AudienceMismatch`.
    AudienceMismatch,
    /// `StaleEpoch`.
    StaleEpoch,
    /// `Unavailable`.
    Unavailable,
    /// `NoMandateContext`.
    NoMandateContext,
    /// `NoMandateResource`.
    NoMandateResource,
    /// `Contradictory`.
    Contradictory,
}
