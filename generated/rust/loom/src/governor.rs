// generated from loom v1
// model digest dab0e9b0b131719fb6d54b29aa318e3ed2ca817df946678486108672bd614274
// contract digest 077f4e4e6548832cdab0185ef7b7f4e65f130d090c84f28346747b75d9e33239
// do not edit: regenerate with `ess synthesize --layout crate`

//! Governor — `loom.governor`.
//!
//! The governor's held case as a store writes it: the case id, the protocol that governs it, the case revision, the current revision of every declared artifact, the value of every case input the protocol declares, the independence dimensions the host declared for each evidence producer, every evidence record received with whether it applies, and every explicit decision recorded for it; and the observations kept beside the cases.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// CompletionRule — `loom.governor.CompletionRule`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionRule {
    /// `ExactlyOne`.
    ExactlyOne,
    /// `FirstDeclared`.
    FirstDeclared,
}

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
    /// `independence` — `Optional<List<loom.governor.StoredIndependence>>`.
    pub independence: Option<Vec<StoredIndependence>>,
    /// `evidence` — `List<loom.governor.StoredEvidence>`.
    pub evidence: Vec<StoredEvidence>,
    /// `decisions` — `Optional<List<loom.evaluation.ExplicitDecision>>`.
    pub decisions: Option<Vec<crate::evaluation::ExplicitDecision>>,
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

/// StoredIndependence — `loom.governor.StoredIndependence`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredIndependence {
    /// `producer` — `String`.
    pub producer: String,
    /// `dimensions` — `List<String>`.
    pub dimensions: Vec<String>,
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
