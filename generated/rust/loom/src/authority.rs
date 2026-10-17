// generated from loom v1
// model digest 748fceb21cf7ee6d4e3507a439a01eec6e7e9190c5bee46f896cc1ad49fa1326
// contract digest 46c4d4d570ca4756a9a8caf1cdfa2d49a60fe40d986d3e54b1dd2ba9a7b18d06
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
