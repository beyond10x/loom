// generated from loom v1
// model digest bd0680f8ac3534518a439d54a62dfd2a90bf5c733881ebeb4328aed4f2b64019
// contract digest 686d3e50c2ac373e8881a3960e68cf840d8e87f71ccb441f5efdc3580e3c3fa1
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
