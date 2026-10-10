// generated from loom v1
// model digest 661401844e582bd43becacc2018c2e17e03abd6db5247a31d68efc412baadd64
// contract digest 6085f971d79bcae6071aef5103a63bf925d279df28713c5dd4a816f29b50f01e
// do not edit: regenerate with `ess synthesize --layout crate`

//! Governor — `loom.governor`.
//!
//! The governor's held case as a store writes it: the case id, the protocol that governs it, the case revision, the current revision of every declared artifact, and every evidence record received with whether it applies; and the observations kept beside the cases.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// StoredArtifact — `loom.governor.StoredArtifact`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    /// `artifact` — `String`.
    pub artifact: String,
    /// `revision` — `String`.
    pub revision: String,
}

/// StoredCase — `loom.governor.StoredCase`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCase {
    /// `id` — `loom.governor.StoredCaseId`.
    pub id: StoredCaseId,
    /// `protocol` — `String`.
    pub protocol: String,
    /// `revision` — `Integer`.
    pub revision: i64,
    /// `artifacts` — `List<loom.governor.StoredArtifact>`.
    pub artifacts: Vec<StoredArtifact>,
    /// `evidence` — `List<loom.governor.StoredEvidence>`.
    pub evidence: Vec<StoredEvidence>,
}

/// StoredCaseId — `loom.governor.StoredCaseId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCaseId(pub String);

/// StoredEvidence — `loom.governor.StoredEvidence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvidence {
    /// `data` — `loom.governor.StoredEvidenceData`.
    pub data: StoredEvidenceData,
    /// `applies` — `Boolean`.
    pub applies: bool,
}

/// StoredEvidenceData — `loom.governor.StoredEvidenceData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvidenceData {
    /// `evidence_id` — `Uuid`.
    pub evidence_id: crate::primitives::Uuid,
    /// `case_id` — `loom.governor.StoredCaseId`.
    pub case_id: StoredCaseId,
    /// `kind` — `String`.
    pub kind: String,
    /// `subject_revision` — `Integer`.
    pub subject_revision: i64,
    /// `producer` — `String`.
    pub producer: String,
    /// `observation_ids` — `List<Uuid>`.
    pub observation_ids: Vec<crate::primitives::Uuid>,
    /// `facts` — `Json`.
    pub facts: crate::json::Value,
    /// `provenance` — `Json`.
    pub provenance: crate::json::Value,
}

/// StoredObservation — `loom.governor.StoredObservation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObservation {
    /// `observation_id` — `Uuid`.
    pub observation_id: crate::primitives::Uuid,
    /// `source` — `String`.
    pub source: String,
    /// `subject` — `String`.
    pub subject: String,
    /// `observed_at` — `Timestamp`.
    pub observed_at: crate::primitives::Timestamp,
    /// `payload` — `Json`.
    pub payload: crate::json::Value,
}
