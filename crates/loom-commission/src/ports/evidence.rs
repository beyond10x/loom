//! The observation and evidence ports: two separate ways into the governor.
//!
//! An observation is a raw report — source, subject, time, payload — from a runtime, an
//! integration, a verifier or an executor. Evidence is typed and attributable: kind, subject
//! revision, producer, facts, provenance and the observations it interprets
//! (`docs/contracts/evidence.md`). They are distinct generated types and reach the governor through
//! distinct ports: [`ObservationPort`] and [`EvidencePort`].
//!
//! Nothing in Commission turns an observation, an executor output or a trace into evidence (Atlas
//! ADR 0074). Interpreting observations is an [`EvidenceAdapter`]'s job: a verifier the trusted
//! integration supplies, which Commission's runtime never calls on executor output.
//!
//! Evidence enters only through [`submit_evidence`], which sets the producer the trusted caller
//! supplies, refuses a producer that names nobody and refuses a record that names no observation.
//! [`AttributedEvidence`], the only thing an [`EvidencePort`] accepts, cannot be built any other
//! way.
//!
//! These ports are push-based: the trusted integration submits evidence through [`submit_evidence`]
//! when it has some. The pull-based `EvidenceProvider` of Atlas ADR 0079, which the governor asks
//! for evidence of a kind about a subject at a revision, is a separate contract whose home that ADR
//! still lists as open; [`EvidencePort`] is not it and does not stand in for it.

use crate::model::responsibility::{
    Evidence, EvidenceData, GovernorError, Observation, ObservationData, evidence_state,
    observation_state,
};
use std::fmt;

/// Where observations reach the governor.
pub trait ObservationPort {
    /// Delivers one observation, as reported. It is never evidence.
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError>;
}

/// Where evidence reaches the governor. The governor validates whether it applies.
pub trait EvidencePort {
    /// Delivers one evidence record that [`submit_evidence`] admitted.
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError>;
}

/// Interprets observations into evidence.
///
/// A verifier the trusted integration supplies. Each record it returns names, in
/// `observation_ids`, the observations it read; the producer it writes is replaced by the trusted
/// caller's at [`submit_evidence`].
pub trait EvidenceAdapter {
    /// The evidence records `observations` support; none when they support nothing.
    fn interpret(&self, observations: &[ObservationData]) -> Vec<EvidenceData>;
}

/// Evidence [`submit_evidence`] admitted: its producer is the trusted caller's, which names someone,
/// and it names at least one observation.
pub struct AttributedEvidence(Evidence<evidence_state::Submitted>);

impl fmt::Debug for AttributedEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AttributedEvidence")
            .field(self.0.data())
            .finish()
    }
}

impl AttributedEvidence {
    /// The admitted evidence.
    pub fn evidence(&self) -> &Evidence<evidence_state::Submitted> {
        &self.0
    }

    /// Hands the admitted evidence back.
    pub fn into_evidence(self) -> Evidence<evidence_state::Submitted> {
        self.0
    }
}

/// Why evidence did not reach the governor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// The trusted caller's producer does not name anyone: it breaks the rule at
    /// [`submit_evidence`].
    NoProducer,
    /// The record names no observation; evidence interprets one or more.
    NoObservation,
    /// The governor did not take it.
    Governor(GovernorError),
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProducer => f.write_str("evidence is attributed to no producer"),
            Self::NoObservation => f.write_str("evidence names no observation"),
            Self::Governor(error) => write!(f, "governor refused evidence: {error:?}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

/// Submits `evidence` through `port`, attributed to `producer`.
///
/// `producer` is supplied by the trusted caller beside the payload. Whatever producer the payload
/// names — in its `producer` field, its facts or its provenance — is not read: the field is
/// overwritten and the rest is carried unchanged.
///
/// A `producer` names someone only if it is non-empty, has no leading or trailing whitespace
/// (Unicode `White_Space`) and contains no control (`Cc`) or format (`Cf`) character. A padded
/// producer is refused, never trimmed: the id that arrives is exactly the id the caller named.
///
/// Refused before anything reaches the port, in this order: a `producer` that names no one, with
/// [`EvidenceError::NoProducer`]; a record whose `observation_ids` is empty, with
/// [`EvidenceError::NoObservation`]. A refusal from the port comes back as
/// [`EvidenceError::Governor`].
pub fn submit_evidence<P>(
    port: &P,
    producer: &str,
    evidence: EvidenceData,
) -> Result<(), EvidenceError>
where
    P: EvidencePort + ?Sized,
{
    if !names_someone(producer) {
        return Err(EvidenceError::NoProducer);
    }
    if evidence.observation_ids.is_empty() {
        return Err(EvidenceError::NoObservation);
    }
    let attributed = AttributedEvidence(Evidence::new(EvidenceData {
        producer: producer.to_owned(),
        ..evidence
    }));
    port.receive(attributed).map_err(EvidenceError::Governor)
}

/// Whether `producer` names someone: the rule at [`submit_evidence`].
fn names_someone(producer: &str) -> bool {
    !producer.is_empty()
        && !producer.starts_with(char::is_whitespace)
        && !producer.ends_with(char::is_whitespace)
        && !producer
            .chars()
            .any(|c| c.is_control() || is_format_character(c))
}

/// The characters of Unicode general category `Cf` (format), Unicode 16.0, as inclusive ranges.
///
/// The standard library answers `Cc` ([`char::is_control`]) and `White_Space`
/// ([`char::is_whitespace`]) but not `Cf`, and this workspace takes no dependencies.
const FORMAT_CHARACTERS: [(char, char); 21] = [
    ('\u{00AD}', '\u{00AD}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{180E}', '\u{180E}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0001}', '\u{E0001}'),
    ('\u{E0020}', '\u{E007F}'),
];

/// Whether `c` is a format (`Cf`) character.
fn is_format_character(c: char) -> bool {
    FORMAT_CHARACTERS
        .iter()
        .any(|&(first, last)| (first..=last).contains(&c))
}
