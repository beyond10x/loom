// generated from loom v1
// model digest 7bd35da34adec64dcc53bfc491e89e8c5b0bce8290af63cbd1ed7a79b0521a60
// contract digest 808c695ab0db97c3e7a51bff1e79a6bb28a1c6884c34dba9b4d4f9ccec9734bf
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

/// HostContext — `loom.authority.HostContext`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostContext {
    /// `verified` — `Json`.
    pub verified: crate::json::Value,
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
