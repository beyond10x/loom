// generated from loom v1
// contract digest 128d7cf7dfa9562a0c2dc4225132e8a37a44efa4a569353eb7c53ed0316f8c67
// do not edit: regenerate with `ess synthesize --layout crate`

//! Governor — `loom.governor`.
//!
//! The governor's held case as a store writes it: the case id, the protocol that governs it, the case revision, the current revision of every declared artifact, the value of every case input the protocol declares, the independence dimensions the host declared for each evidence producer, every evidence record received with whether it applies, every explicit decision recorded for it, the links to the case that opened it and to the cases it opened, and how it terminated; and the observations kept beside the cases.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// CaseLink — `loom.governor.CaseLink`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseLink {
    /// `parent` — `loom.governor.StoredCaseId`.
    pub parent: StoredCaseId,
    /// `child` — `loom.governor.StoredCaseId`.
    pub child: StoredCaseId,
    /// `name` — `String`.
    pub name: String,
    /// `action` — `String`.
    pub action: String,
    /// `outcome_evidence` — `Uuid`.
    pub outcome_evidence: crate::primitives::Uuid,
    /// `subject` — `String`.
    pub subject: String,
    /// `subject_revision` — `String`.
    pub subject_revision: String,
}

/// CaseTermination — `loom.governor.CaseTermination`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseTermination {
    /// `outcome` — `String`.
    pub outcome: String,
    /// `instant` — `Timestamp`.
    pub instant: crate::primitives::Timestamp,
}

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
    /// `parent` — `Optional<loom.governor.CaseLink>`.
    pub parent: Option<CaseLink>,
    /// `children` — `Optional<List<loom.governor.CaseLink>>`.
    pub children: Option<Vec<CaseLink>>,
    /// `termination` — `Optional<loom.governor.CaseTermination>`.
    pub termination: Option<CaseTermination>,
}

/// StoredCaseId — `loom.governor.StoredCaseId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCaseId(pub String);

/// StoredDecisionRequest — `loom.governor.StoredDecisionRequest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDecisionRequest {
    /// `position` — `Integer`.
    pub position: i64,
    /// `state` — `loom.decision.DecisionRequest.State`.
    pub state: crate::decision::DecisionRequestState,
    /// `request` — `loom.governor.StoredDecisionRequestData`.
    pub request: StoredDecisionRequestData,
    /// `records` — `List<loom.decision.EscalationSuppressedRecord>`.
    pub records: Vec<crate::decision::EscalationSuppressedRecord>,
}

/// StoredDecisionRequestData — `loom.governor.StoredDecisionRequestData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDecisionRequestData {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: crate::decision::DecisionRequestId,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: crate::decision::CaseId,
    /// `question_key` — `String`.
    pub question_key: String,
    /// `record_key` — `String`.
    pub record_key: String,
    /// `unblocks` — `loom.decision.Unblocks`.
    pub unblocks: crate::decision::Unblocks,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: crate::decision::ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: crate::decision::Revision,
    /// `subject_path` — `Optional<String>`.
    pub subject_path: Option<String>,
    /// `question` — `String`.
    pub question: String,
    /// `options` — `List<loom.decision.DecisionOption>`.
    pub options: Vec<crate::decision::DecisionOption>,
    /// `autonomy_level` — `loom.decision.AutonomyLevel`.
    pub autonomy_level: crate::decision::AutonomyLevel,
    /// `decision_class` — `loom.decision.DecisionClassId`.
    pub decision_class: crate::decision::DecisionClassId,
    /// `holder` — `loom.decision.Holder`.
    pub holder: crate::decision::Holder,
    /// `eligible` — `List<loom.decision.Principal>`.
    pub eligible: Vec<crate::decision::Principal>,
    /// `addressed_to` — `loom.decision.Principal`.
    pub addressed_to: crate::decision::Principal,
    /// `opened_at` — `Timestamp`.
    pub opened_at: crate::primitives::Timestamp,
    /// `expires_at` — `Optional<Timestamp>`.
    pub expires_at: Option<crate::primitives::Timestamp>,
    /// `supersedes` — `Optional<loom.decision.DecisionRequestId>`.
    pub supersedes: Option<crate::decision::DecisionRequestId>,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<crate::decision::OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `recommendation_strength` — `loom.decision.Strength`.
    pub recommendation_strength: crate::decision::Strength,
    /// `default_on_silence` — `Boolean`.
    pub default_on_silence: bool,
    /// `grounds` — `List<loom.decision.Ground>`.
    pub grounds: Vec<crate::decision::Ground>,
    /// `recommended_by` — `Optional<loom.decision.ProposerKind>`.
    pub recommended_by: Option<crate::decision::ProposerKind>,
    /// `grounding_report` — `Optional<String>`.
    pub grounding_report: Option<String>,
    /// `no_recommendation_reason` — `Optional<String>`.
    pub no_recommendation_reason: Option<String>,
    /// `escalated_to` — `Optional<loom.decision.Principal>`.
    pub escalated_to: Option<crate::decision::Principal>,
    /// `escalation_reason` — `Optional<String>`.
    pub escalation_reason: Option<String>,
    /// `decided_option` — `Optional<loom.decision.OptionLetter>`.
    pub decided_option: Option<crate::decision::OptionLetter>,
    /// `decided_answer` — `Optional<String>`.
    pub decided_answer: Option<String>,
    /// `decided_by` — `Optional<loom.decision.Principal>`.
    pub decided_by: Option<crate::decision::Principal>,
    /// `decided_by_kind` — `Optional<loom.decision.PrincipalKind>`.
    pub decided_by_kind: Option<crate::decision::PrincipalKind>,
    /// `decided_at` — `Optional<Timestamp>`.
    pub decided_at: Option<crate::primitives::Timestamp>,
    /// `decided_case_revision` — `Optional<loom.decision.Revision>`.
    pub decided_case_revision: Option<crate::decision::Revision>,
    /// `decision_reasons` — `List<String>`.
    pub decision_reasons: Vec<String>,
    /// `produced_record` — `Optional<loom.decision.ProducedRecord>`.
    pub produced_record: Option<crate::decision::ProducedRecord>,
    /// `superseded_by` — `Optional<loom.decision.DecisionRequestId>`.
    pub superseded_by: Option<crate::decision::DecisionRequestId>,
    /// `superseded_reason` — `Optional<String>`.
    pub superseded_reason: Option<String>,
}

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
