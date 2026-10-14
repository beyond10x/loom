// generated from loom v1
// model digest 47ee2e732f06c5c48e29f632cf2bbcae0c5b6ae5fb056f25ab3fa3aa80618b7d
// contract digest ebda3a653d1aac83a8c0a66081d3878fa4dfe2da527b9d567406f6923d8bd6df
// do not edit: regenerate with `ess synthesize --layout crate`

//! Governor — `loom.governor`.
//!
//! The governor's held case as a store writes it: the case id, the protocol that governs it, the case revision, the current revision of every declared artifact, the value of every case input the protocol declares, and every evidence record received with whether it applies; and the observations kept beside the cases.
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
    /// `inputs` — `List<loom.governor.StoredInput>`.
    pub inputs: Vec<StoredInput>,
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

/// StoredInput — `loom.governor.StoredInput`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredInput {
    /// `input` — `String`.
    pub input: String,
    /// `value` — `String`.
    pub value: String,
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
