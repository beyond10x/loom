// generated from loom v1
// model digest 866bbd9d47246b4227f3631ebb34d83e265512af496b4fe99b0098c8cc298c10
// contract digest 08c02cd39830806f4c6eaa95dfea4ecf631c548ae35d9b4e43d6ab0db1b725ec
// do not edit: regenerate with `ess synthesize --layout crate`

//! Evaluation — `loom.evaluation`.
//!
//! One stateless evaluation: a protocol named from the host's catalog, a canon-case/1 case snapshot, the canon-evidence/1 records that go with it and an optional trusted evaluation time go in; Canon's decision comes out, or a refusal naming the input it could not use. It decides and never acts, reads no clock and calls no network or model.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionStatus — `loom.evaluation.ActionStatus`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    /// `Admissible`.
    Admissible,
    /// `ApprovalRequired`.
    ApprovalRequired,
    /// `Blocked`.
    Blocked,
}

/// CaseSnapshot — `loom.evaluation.CaseSnapshot`: a distinct wrapper around `Json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSnapshot(pub crate::json::Value);

/// ClaimValue — `loom.evaluation.ClaimValue`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimValue {
    /// `True`.
    True,
    /// `False`.
    False,
    /// `Unknown`.
    Unknown,
}

/// DecidedAction — `loom.evaluation.DecidedAction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecidedAction {
    /// `action` — `String`.
    pub action: String,
    /// `status` — `loom.evaluation.ActionStatus`.
    pub status: ActionStatus,
    /// `requires` — `List<String>`.
    pub requires: Vec<String>,
    /// `reasons` — `List<Json>`.
    pub reasons: Vec<crate::json::Value>,
}

/// DecidedClaim — `loom.evaluation.DecidedClaim`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecidedClaim {
    /// `claim` — `String`.
    pub claim: String,
    /// `value` — `loom.evaluation.ClaimValue`.
    pub value: ClaimValue,
}

/// DecidedObligation — `loom.evaluation.DecidedObligation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecidedObligation {
    /// `obligation` — `String`.
    pub obligation: String,
    /// `open` — `Boolean`.
    pub open: bool,
}

/// EvaluationDecision — `loom.evaluation.EvaluationDecision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationDecision {
    /// `protocol` — `loom.evaluation.ProtocolName`.
    pub protocol: ProtocolName,
    /// `case` — `String`.
    pub case: String,
    /// `actions` — `List<loom.evaluation.DecidedAction>`.
    pub actions: Vec<DecidedAction>,
    /// `claims` — `List<loom.evaluation.DecidedClaim>`.
    pub claims: Vec<DecidedClaim>,
    /// `obligations` — `List<loom.evaluation.DecidedObligation>`.
    pub obligations: Vec<DecidedObligation>,
    /// `outcome` — `Optional<String>`.
    pub outcome: Option<String>,
    /// `canon` — `Json`.
    pub canon: crate::json::Value,
}

/// EvaluationInput — `loom.evaluation.EvaluationInput`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationInput {
    /// `Request`.
    Request,
    /// `Protocol`.
    Protocol,
    /// `Snapshot`.
    Snapshot,
    /// `Evidence`.
    Evidence,
    /// `Time`.
    Time,
}

/// EvaluationRefusal — `loom.evaluation.EvaluationRefusal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRefusal {
    /// `input` — `loom.evaluation.EvaluationInput`.
    pub input: EvaluationInput,
    /// `evidence_index` — `Optional<Integer>`.
    pub evidence_index: Option<i64>,
    /// `code` — `String`.
    pub code: String,
    /// `message` — `String`.
    pub message: String,
}

/// EvaluationRequest — `loom.evaluation.EvaluationRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRequest {
    /// `protocol` — `loom.evaluation.ProtocolName`.
    pub protocol: ProtocolName,
    /// `snapshot` — `loom.evaluation.CaseSnapshot`.
    pub snapshot: CaseSnapshot,
    /// `evidence` — `List<loom.evaluation.EvidenceRecord>`.
    pub evidence: Vec<EvidenceRecord>,
    /// `at` — `Optional<Timestamp>`.
    pub at: Option<crate::primitives::Timestamp>,
}

/// EvidenceRecord — `loom.evaluation.EvidenceRecord`: a distinct wrapper around `Json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord(pub crate::json::Value);

/// ProtocolName — `loom.evaluation.ProtocolName`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolName(pub String);
