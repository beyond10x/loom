#![forbid(unsafe_code)]

//! Evaluates a case's protocol with Canon behind the governor and evidence ports Commission
//! defines.
//!
//! [`CanonGovernor`] implements Commission's [`Governor`], [`EvidencePort`] and [`ObservationPort`]
//! over Canon. A case is opened on a built-in or host-registered protocol, named `<name>@<major>`
//! (`software-change@1`), with the caller's revision of every artifact the protocol declares and
//! the value of every case input it declares ([`CaseInputs`], [`CanonGovernor::open`]). Its only
//! other writes are a new artifact revision ([`CanonGovernor::update_revision`]), evidence and
//! observations.
//!
//! # Case inputs
//!
//! A case input is configuration the case fixes when it opens (`software.change/1` declares
//! `risk`, `change_kind` and `affects_security_boundary`). The host supplies the values, as
//! trusted context; the governor checks them against the protocol before anything is stored: an
//! input the protocol does not declare ([`OpenError::UndeclaredInput`]), a value its input does
//! not list ([`OpenError::UnlistedInputValue`]) and a declared input left out
//! ([`OpenError::MissingInput`]) are each refused naming the input. A protocol that declares no
//! input opens with none. One input given twice never replaces the first: [`CaseInputs::with`]
//! and [`CaseInputs::try_from_pairs`] refuse it ([`OpenError::RepeatedInput`]). The inputs are
//! held with the case and given to Canon with every evaluation; they never change.
//!
//! Every read of a held case checks its inputs again, as at opening: a case whose stored inputs
//! do not satisfy its protocol (a declared input missing, an empty list included, an undeclared
//! one, an unlisted value) is never decided and its inputs are never reported. The governor
//! answers `GovernorUnavailable` for it, as for any store failure, and [`FileCaseStore`] refuses
//! to read such a case of a built-in protocol, naming the case and the input.
//!
//! # Independence
//!
//! A protocol may declare independence dimensions (`independence_dimensions`) and an evidence
//! match may demand that its records' producer be independent of the case's work on some of them
//! (`independent_on`). Who is independent is the host's to say, as trusted context, never an
//! evidence record's or a model's: [`CanonGovernor::open_with_independence`] takes, for each
//! producer, the dimensions it is independent on ([`CaseIndependence`]), and the governor puts
//! them in the case's `canon-case/1` snapshot. A producer that is not an identifier
//! ([`OpenError::InvalidProducer`]) or a dimension the protocol does not declare
//! ([`OpenError::UndeclaredDimension`]) is refused naming it, before anything is stored. A case
//! opened with [`CanonGovernor::open`] declares no producer independent, so a claim whose every
//! match demands independence is never decided by a record. The declarations are held with the
//! case and never change; [`FileCaseStore`] keeps them in `StoredCase.independence`, and reads a
//! case stored without them (as Loom 0.23.0 and earlier wrote every case) as declaring none,
//! which is how those versions evaluated it. Every read of a held case checks them again, as at
//! opening.
//!
//! # Evaluation
//!
//! Every `frontier` and `completion` call evaluates afresh: the protocol is taken from the built-in
//! registry or registered by [`CanonGovernor::with_protocol`], compiled with Canon and evaluated with
//! `b10x_canon::eval::evaluate_with` over the case's `canon-case/1` snapshot and the evidence that
//! applies. A trusted host may supply [`EvaluationTime`] for freshness; the governor reads no clock
//! itself and supplies no authority decision. It never invents authority, so an action
//! that requires a capability is at best `approval-required`; Commission asks its authority
//! provider for it.
//!
//! The explicit decisions it supplies (`canon-decisions/1`) are the ones the host recorded on the
//! case with [`CanonGovernor::decide`], each taken at the case revision current when it was
//! recorded, so an outcome that requires a decision (`requires: decision: <name>`) is legitimate
//! while the case is at that revision and blocked again once it moves. Canon judges each decision
//! before it is recorded, and one it refuses (an outcome the protocol does not declare, or one that
//! does not require a decision of that name) is refused ([`DecisionError::Refused`]) and nothing
//! is kept. Who may take a decision is not the governor's to judge: it grants no authority with
//! one.
//!
//! The frontier lists every action the protocol declares, with Canon's status, the capability the
//! action requires (when it requires one) and, unless it is admissible, Canon's reasons, each
//! written as the compact JSON Canon gives it. It also lists every claim's value and every
//! obligation, open exactly when Canon reports it `open`: one Canon holds `discharged` or
//! `not_applicable` (the case's inputs do not select it) is not open. It is issued for the case's current
//! revision, under a frontier id derived from the case, that revision and Canon's decision bytes.
//!
//! `completion` reports `Complete` with an outcome only when exactly one declared outcome is
//! `legitimate` in Canon's decision, and `Open` otherwise: never an outcome Canon holds blocked,
//! and never a choice between several.
//!
//! # Evidence
//!
//! Evidence arrives through Commission's `submit_evidence`. Its `facts` carry one
//! `canon-evidence/1` record of the evidence's `kind`, handed to Canon's `evidence_from_value` as
//! the value it is, never as text: a string reaches Canon exactly as it arrived, a number is read
//! from its spelling as Canon's YAML reader reads a number, and an object with a member named twice
//! carries no record. The record's `producer` is the trusted one `submit_evidence` attributed the
//! evidence to (`EvidenceData.producer`), never one its facts name: a producer in the facts is
//! removed before Canon reads the record, so it never decides whether the record applies, and a
//! trusted producer that is not a Canon identifier (it holds whitespace) is given as none, so its
//! record satisfies no demand for independence. The host's facts (`target`, the route target a
//! routing record names) are removed the same way: Canon refuses a field `canon-evidence/1` does
//! not declare, and they are kept with the evidence, never read. Its `observed_at` is read from the
//! facts, as the host wrote it; the governor reads no clock. It is taken only as the instant of the
//! observation the record cites: a record citing an observation this governor never received (with
//! or without an `observed_at`), or one of whose `observed_at` members is not the text of each
//! cited observation's `observed_at`, is refused (`GovernorUnavailable`, Commission's error naming
//! no record) and nothing of it is kept; [`CanonGovernor::check_evidence`] gives the
//! [`EvidenceRefusal`] naming the record. Every record received for a held case is kept, in the
//! order received ([`CanonGovernor::evidence`]); it applies only when its facts read as such a
//! record and Canon evaluates the case with it and the records that already apply without refusal
//! (a declared kind and subject, an id used once). Whether it applies is decided once, when it
//! arrives, so a record that does not apply never stops a later evaluation. Which artifact revision
//! a record is about is Canon's to judge: a record bound to a superseded revision of its subject is
//! excluded by Canon's revision binding.
//!
//! Observations are kept apart, in the order received ([`CanonGovernor::observations`]), and are
//! never evidence (Atlas ADR 0074).
//!
//! # Stateless evaluation
//!
//! [`evaluate`] is the same evaluation for a caller that keeps its own case record: it takes an
//! [`EvaluationRequest`](model::evaluation::EvaluationRequest) (`ess/domains/evaluation.yaml`)
//! naming a protocol of the host's [`ProtocolCatalog`], a `canon-case/1` snapshot, the
//! `canon-evidence/1` records and an optional trusted time, and returns Canon's decision, projected
//! as the frontier and completion are, with the whole `canon-decision/1` document beside it. It
//! holds nothing, names no Commission type, and refuses an input it cannot use naming that input:
//! the protocol, the snapshot, the evidence record by its position, the time, or the explicit
//! decisions the request carries (`canon-decisions/1`, judged by Canon as for the governor). A readable record
//! that does not apply to the case is set aside, as the governor sets it aside. Unlike the
//! governor, which sets aside an unreadable record and the later of two records with one id and
//! still decides, `evaluate` refuses both. The protocol comes only from the catalog, never from
//! the caller.
//!
//! # Dependencies
//!
//! Canon is the library `b10x-canon-engineering` uses, tag `0.6.0`, pinned by `Cargo.lock` to
//! commit `efcfd04`, so the protocol model the ELS registry returns is the one this crate
//! compiles. Commission is the path dependency `b10x-loom-commission`, built from this
//! repository's own tree, and `b10x-canon-engineering` is pinned to tag `0.9.0` of
//! beyond10x/engineering-protocols.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use loom_protocols::ProtocolCatalog;

/// The generated `loom` model: [`evaluate`] reads and returns its `loom.evaluation` types.
pub use loom as model;

use loom::evaluation::{
    ActionStatus as DecidedStatus, ClaimValue, DecidedAction, DecidedClaim, DecidedObligation,
    EvaluationDecision, EvaluationInput, EvaluationRefusal, EvaluationRequest, ExplicitDecision,
};

use b10x_canon::eval::{
    Refusal, Supplied, case_from_value, evaluate_with, evidence_from_value, render,
};
use b10x_canon::ir::{Ir, compile};
use b10x_canon::model::{
    ArtifactId, CASE_FORMAT, Case, CaseArtifact, Decision, Declarations, EvidenceRecord,
    IndependenceDimensionId, InputId, InputValue, Principal, Revision, Truth as CanonTruth,
    is_identifier,
};
use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, CompletionDeterminationComplete, EvidenceData,
    Frontier, FrontierAction, FrontierClaim, FrontierData, FrontierId, FrontierObligation,
    GovernorError, Observation, ObservationData, Truth, Unit, frontier_state, observation_state,
};
use b10x_loom_commission::ports::evidence::{AttributedEvidence, EvidencePort, ObservationPort};
use b10x_loom_commission::ports::governor::Governor;

mod file_run_store;
mod file_store;
mod file_suspension;
mod locked_dir;

pub use file_run_store::{FileWorkflowRunStore, LONGEST_RUN_ID};
pub use file_store::{FileCaseStore, FileStoreError};
pub use file_suspension::{FileSuspensionStore, LONGEST_COMMISSION_ID};

/// One case as the governor holds it. A durable store writes it as the `loom.governor` domain's
/// `StoredCase` (`ess/domains/governor.yaml`), as [`FileCaseStore`] does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseState {
    /// The case id.
    pub id: CaseId,
    /// The registered protocol that governs it, `<name>@<major>`.
    pub protocol: String,
    /// The case revision: 1 when opened, one more with each artifact revision recorded.
    pub revision: i64,
    /// The current revision of every artifact the protocol declares.
    pub artifacts: BTreeMap<String, String>,
    /// The value of every case input the protocol declares, as the case opened with them.
    pub inputs: CaseInputs,
    /// The independence dimensions the host declared for each evidence producer when the case
    /// opened (see the crate documentation, "Independence").
    pub independence: CaseIndependence,
    /// Every evidence record received for the case, in the order received.
    pub evidence: Vec<HeldEvidence>,
    /// Every explicit decision recorded for the case ([`CanonGovernor::decide`]), in the order
    /// recorded, each at the Canon case revision (`r<revision>`) it was taken at.
    pub decisions: Vec<ExplicitDecision>,
}

/// The value of each case input a case opens with, keyed by input id (`risk` → `standard`): the
/// host's trusted context, never a model's. [`CanonGovernor::open`] checks them against the
/// inputs the protocol declares; a protocol that declares none opens with
/// [`CaseInputs::none`]. Giving an input a second value is refused
/// ([`OpenError::RepeatedInput`]), never a replacement of the first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaseInputs {
    values: BTreeMap<String, String>,
}

impl CaseInputs {
    /// No input: what a case of a protocol that declares none opens with.
    pub fn none() -> Self {
        Self::default()
    }

    /// These inputs and `input` with `value`.
    ///
    /// # Errors
    /// [`OpenError::RepeatedInput`] when these inputs already give `input` a value.
    pub fn with(
        mut self,
        input: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, OpenError> {
        let input = input.into();
        if self.values.contains_key(&input) {
            return Err(OpenError::RepeatedInput { input });
        }
        self.values.insert(input, value.into());
        Ok(self)
    }

    /// The inputs `pairs` give, each input once.
    ///
    /// # Errors
    /// [`OpenError::RepeatedInput`] naming the first input given a second time.
    pub fn try_from_pairs<I: Into<String>, V: Into<String>>(
        pairs: impl IntoIterator<Item = (I, V)>,
    ) -> Result<Self, OpenError> {
        pairs
            .into_iter()
            .try_fold(Self::none(), |inputs, (input, value)| {
                inputs.with(input, value)
            })
    }

    /// The value given for `input`, if any.
    pub fn get(&self, input: &str) -> Option<&str> {
        self.values.get(input).map(String::as_str)
    }

    /// Every input and its value, in input order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(input, value)| (input.as_str(), value.as_str()))
    }

    /// Whether no input is given.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// How many inputs are given.
    pub fn len(&self) -> usize {
        self.values.len()
    }
}

/// The independence dimensions on which each evidence producer is independent of a case's work,
/// keyed by producer (`reviewer-run-2` → `different_agent_run`): the host's trusted context, never
/// an evidence record's or a model's. [`CanonGovernor::open_with_independence`] checks them
/// against the dimensions the protocol declares; a case opened with [`CaseIndependence::none`]
/// declares no producer independent. Declaring a producer twice, or one dimension twice for it, is
/// refused ([`OpenError::RepeatedProducer`], [`OpenError::RepeatedDimension`]), never a
/// replacement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaseIndependence {
    producers: BTreeMap<String, BTreeSet<String>>,
}

impl CaseIndependence {
    /// No producer declared independent.
    pub fn none() -> Self {
        Self::default()
    }

    /// These declarations and `producer` independent on each of `dimensions`.
    ///
    /// # Errors
    /// [`OpenError::RepeatedProducer`] when these declarations already list `producer`, and
    /// [`OpenError::RepeatedDimension`] when `dimensions` names one dimension twice.
    pub fn with<D: Into<String>>(
        mut self,
        producer: impl Into<String>,
        dimensions: impl IntoIterator<Item = D>,
    ) -> Result<Self, OpenError> {
        let producer = producer.into();
        if self.producers.contains_key(&producer) {
            return Err(OpenError::RepeatedProducer { producer });
        }
        let mut listed = BTreeSet::new();
        for dimension in dimensions {
            let dimension = dimension.into();
            if listed.contains(&dimension) {
                return Err(OpenError::RepeatedDimension {
                    producer,
                    dimension,
                });
            }
            listed.insert(dimension);
        }
        self.producers.insert(producer, listed);
        Ok(self)
    }

    /// The dimensions declared for `producer`, in dimension order; `None` when it is not listed.
    pub fn get(&self, producer: &str) -> Option<impl Iterator<Item = &str>> {
        self.producers
            .get(producer)
            .map(|dimensions| dimensions.iter().map(String::as_str))
    }

    /// Every producer listed, in producer order, with its dimensions in dimension order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, impl Iterator<Item = &str>)> {
        self.producers.iter().map(|(producer, dimensions)| {
            (producer.as_str(), dimensions.iter().map(String::as_str))
        })
    }

    /// Whether no producer is listed.
    pub fn is_empty(&self) -> bool {
        self.producers.is_empty()
    }

    /// How many producers are listed.
    pub fn len(&self) -> usize {
        self.producers.len()
    }
}

/// An evidence record as received, and whether it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEvidence {
    /// The record as `submit_evidence` admitted it.
    pub data: EvidenceData,
    /// Whether it applies: its facts read as a `canon-evidence/1` record that Canon evaluates.
    pub applies: bool,
}

/// Infallible compatibility port. Durable adapters use [`FallibleCaseStore`] instead.
pub trait CaseStore {
    /// Holds `state` under its id; `false`, holding nothing new, when that id is already held.
    ///
    /// `false` means exactly that the id is already held, and nothing else. A store that cannot
    /// write must not return `false`: [`CanonGovernor::open`] takes `false` as "try the next id"
    /// and would never stop. Fallible implementations must use [`FallibleCaseStore`].
    fn insert(&self, state: CaseState) -> bool;

    /// A copy of the case's state, or `None` when it is not held.
    fn get(&self, case: &CaseId) -> Option<CaseState>;

    /// Applies `change` to the held case's state, with no other change in between; `None` when it
    /// is not held.
    fn update<T>(&self, case: &CaseId, change: impl FnOnce(&mut CaseState) -> T) -> Option<T>;

    /// Keeps one observation.
    fn observe(&self, observation: ObservationData);

    /// Every observation kept, in the order received.
    fn observations(&self) -> Vec<ObservationData>;
}

/// A host-owned store whose failures are explicit. Writes must be atomic: an error must leave
/// the previous state intact, including when the update callback already ran. No callback may
/// perform external effects. Persistence implementations own their transaction and recovery.
pub trait FallibleCaseStore {
    type Error: fmt::Display;
    fn insert(&self, state: CaseState) -> Result<bool, Self::Error>;
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, Self::Error>;
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error>;
    fn observe(&self, observation: ObservationData) -> Result<(), Self::Error>;
    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error>;
}

impl<S: CaseStore> FallibleCaseStore for S {
    type Error = std::convert::Infallible;
    fn insert(&self, state: CaseState) -> Result<bool, Self::Error> {
        Ok(CaseStore::insert(self, state))
    }
    fn get(&self, case: &CaseId) -> Result<Option<CaseState>, Self::Error> {
        Ok(CaseStore::get(self, case))
    }
    fn update<T>(
        &self,
        case: &CaseId,
        change: impl FnOnce(&mut CaseState) -> T,
    ) -> Result<Option<T>, Self::Error> {
        Ok(CaseStore::update(self, case, change))
    }
    fn observe(&self, observation: ObservationData) -> Result<(), Self::Error> {
        CaseStore::observe(self, observation);
        Ok(())
    }
    fn observations(&self) -> Result<Vec<ObservationData>, Self::Error> {
        Ok(CaseStore::observations(self))
    }
}

/// Trusted evaluation time, refreshed for every evaluation. This is a host port, never model
/// context. It grants no authority and contributes no evidence or explicit decisions.
pub trait EvaluationTime {
    fn at(&self, case: &CaseId) -> Result<Option<String>, GovernorError>;
}

/// Preserve the default governor's clock-free behavior.
#[derive(Debug, Default)]
pub struct NoEvaluationTime;
impl EvaluationTime for NoEvaluationTime {
    fn at(&self, _: &CaseId) -> Result<Option<String>, GovernorError> {
        Ok(None)
    }
}
impl<F> EvaluationTime for F
where
    F: Fn(&CaseId) -> Result<Option<String>, GovernorError>,
{
    fn at(&self, case: &CaseId) -> Result<Option<String>, GovernorError> {
        self(case)
    }
}

/// A [`CaseStore`] in memory, behind one lock.
#[derive(Debug, Default)]
pub struct MemoryCaseStore {
    inner: Mutex<Memory>,
}

#[derive(Debug, Default)]
struct Memory {
    cases: BTreeMap<String, CaseState>,
    observations: Vec<ObservationData>,
}

impl MemoryCaseStore {
    fn lock(&self) -> MutexGuard<'_, Memory> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl CaseStore for MemoryCaseStore {
    fn insert(&self, state: CaseState) -> bool {
        let mut memory = self.lock();
        if memory.cases.contains_key(&state.id.0) {
            return false;
        }
        memory.cases.insert(state.id.0.clone(), state);
        true
    }

    fn get(&self, case: &CaseId) -> Option<CaseState> {
        self.lock().cases.get(&case.0).cloned()
    }

    fn update<T>(&self, case: &CaseId, change: impl FnOnce(&mut CaseState) -> T) -> Option<T> {
        self.lock().cases.get_mut(&case.0).map(change)
    }

    fn observe(&self, observation: ObservationData) {
        self.lock().observations.push(observation);
    }

    fn observations(&self) -> Vec<ObservationData> {
        self.lock().observations.clone()
    }
}

/// Why a case was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// A host store failed; this is not an id collision and must not be retried as one.
    StoreUnavailable { problem: String },
    /// No host registration or built-in matches `protocol`.
    UnknownProtocol { protocol: String },
    /// The protocol does not parse, validate, compile or fit Commission's frontier.
    InvalidProtocol { protocol: String, problem: String },
    /// The protocol declares `artifact`, and the revisions give none for it.
    MissingRevision { artifact: String },
    /// The revisions name `artifact`, which the protocol does not declare.
    UndeclaredArtifact { artifact: String },
    /// The revision given for `artifact` is not an identifier.
    InvalidRevision { artifact: String },
    /// The inputs give `input`, which the protocol does not declare.
    UndeclaredInput { input: String },
    /// The inputs give `input` the value `value`, which the input does not list.
    UnlistedInputValue { input: String, value: String },
    /// The protocol declares `input`, and the inputs give no value for it.
    MissingInput { input: String },
    /// The inputs give `input` a second value.
    RepeatedInput { input: String },
    /// The independence declarations name `producer`, which is not an identifier.
    InvalidProducer { producer: String },
    /// The independence declarations list `producer` on `dimension`, which the protocol does not
    /// declare.
    UndeclaredDimension { producer: String, dimension: String },
    /// The independence declarations list `producer` a second time.
    RepeatedProducer { producer: String },
    /// The independence declarations list `producer` on `dimension` a second time.
    RepeatedDimension { producer: String, dimension: String },
    /// The case id is not an identifier.
    InvalidCaseId { case: String },
    /// A case with this id is already held.
    CaseExists { case: String },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable { problem } => write!(f, "case store unavailable: {problem}"),
            Self::UnknownProtocol { protocol } => {
                write!(f, "no registered protocol `{protocol}`")
            }
            Self::InvalidProtocol { protocol, problem } => {
                write!(f, "protocol `{protocol}` does not compile: {problem}")
            }
            Self::MissingRevision { artifact } => {
                write!(f, "no revision is given for declared artifact `{artifact}`")
            }
            Self::UndeclaredArtifact { artifact } => {
                write!(f, "artifact `{artifact}` is not declared by the protocol")
            }
            Self::InvalidRevision { artifact } => {
                write!(
                    f,
                    "the revision of artifact `{artifact}` is not an identifier"
                )
            }
            Self::UndeclaredInput { input } => {
                write!(f, "case input `{input}` is not declared by the protocol")
            }
            Self::UnlistedInputValue { input, value } => write!(
                f,
                "case input `{input}` does not list the value `{value}` among its values"
            ),
            Self::MissingInput { input } => {
                write!(f, "no value is given for declared case input `{input}`")
            }
            Self::RepeatedInput { input } => {
                write!(f, "case input `{input}` is given more than one value")
            }
            Self::InvalidProducer { producer } => {
                write!(f, "producer `{producer}` is not an identifier")
            }
            Self::UndeclaredDimension {
                producer,
                dimension,
            } => write!(
                f,
                "producer `{producer}` is declared independent on dimension `{dimension}`, which the protocol does not declare"
            ),
            Self::RepeatedProducer { producer } => {
                write!(
                    f,
                    "producer `{producer}` is declared independent more than once"
                )
            }
            Self::RepeatedDimension {
                producer,
                dimension,
            } => write!(
                f,
                "producer `{producer}` is declared independent on dimension `{dimension}` more than once"
            ),
            Self::InvalidCaseId { case } => write!(f, "case id `{case}` is not an identifier"),
            Self::CaseExists { case } => write!(f, "case `{case}` is already held"),
        }
    }
}

impl std::error::Error for OpenError {}

/// Why an artifact revision was not recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// The store could not record the revision.
    StoreUnavailable { problem: String },
    /// The governor does not hold the case.
    UnknownCase,
    /// The case's protocol does not declare `artifact`.
    UndeclaredArtifact { artifact: String },
    /// The revision is not an identifier.
    InvalidRevision { artifact: String },
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable { problem } => write!(f, "case store unavailable: {problem}"),
            Self::UnknownCase => f.write_str("the governor does not hold the case"),
            Self::UndeclaredArtifact { artifact } => {
                write!(f, "artifact `{artifact}` is not declared by the protocol")
            }
            Self::InvalidRevision { artifact } => {
                write!(
                    f,
                    "the revision of artifact `{artifact}` is not an identifier"
                )
            }
        }
    }
}

impl std::error::Error for UpdateError {}

/// Why an explicit decision was not recorded ([`CanonGovernor::decide`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionError {
    /// The store could not record the decision.
    StoreUnavailable { problem: String },
    /// The governor does not hold the case.
    UnknownCase,
    /// Canon refuses the decision for the case: `code` is Canon's refusal code
    /// (`undeclared-outcome`, `undeclared-decision`, `invalid-identifier`, ...).
    Refused { code: String, message: String },
}

impl fmt::Display for DecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable { problem } => write!(f, "case store unavailable: {problem}"),
            Self::UnknownCase => f.write_str("the governor does not hold the case"),
            Self::Refused { code, message } => write!(f, "{code}: {message}"),
        }
    }
}

impl std::error::Error for DecisionError {}

/// Why the governor refuses an evidence record's `observed_at` ([`CanonGovernor::check_evidence`]),
/// naming the record by its evidence id. A refused record is not kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRefusal {
    /// The record cites `observation`, which this governor never received.
    ObservationNotReceived {
        evidence: String,
        observation: String,
    },
    /// The record's `observed_at` is not the instant of `observation`, which it cites.
    ObservedAtDiffers {
        evidence: String,
        observation: String,
        observed_at: String,
        observation_at: String,
    },
    /// The observations received could not be read, so the record was not checked.
    Unavailable { evidence: String },
}

impl fmt::Display for EvidenceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObservationNotReceived {
                evidence,
                observation,
            } => write!(
                f,
                "evidence `{evidence}` cites observation `{observation}`, which the governor \
                 never received"
            ),
            Self::ObservedAtDiffers {
                evidence,
                observation,
                observed_at,
                observation_at,
            } => write!(
                f,
                "evidence `{evidence}` is observed at `{observed_at}`, not at `{observation_at}`, \
                 the instant of observation `{observation}` it cites"
            ),
            Self::Unavailable { evidence } => write!(
                f,
                "evidence `{evidence}` was not checked: the observations received cannot be read"
            ),
        }
    }
}

impl std::error::Error for EvidenceRefusal {}

/// A governor that decides with Canon, over cases kept in `S`.
#[derive(Debug, Default)]
pub struct CanonGovernor<S, T = NoEvaluationTime> {
    store: S,
    protocols: BTreeMap<String, Ir>,
    catalog_definitions: Option<BTreeMap<String, String>>,
    time: T,
}

impl<S: FallibleCaseStore> CanonGovernor<S> {
    /// A governor over `store`.
    pub fn new(store: S) -> Self {
        Self {
            store,
            protocols: BTreeMap::new(),
            catalog_definitions: None,
            time: NoEvaluationTime,
        }
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> CanonGovernor<S, T> {
    /// Admit exactly the immutable host catalog, compiling every definition with Canon.
    ///
    /// This replaces implicit bundled lookup with the catalog's exact membership. Admission
    /// must precede any explicit protocol registrations; it cannot silently replace definitions
    /// already admitted by another host path. Once bound, the catalog cannot be extended.
    pub fn with_catalog(mut self, catalog: &ProtocolCatalog) -> Result<Self, OpenError> {
        if self.catalog_definitions.is_some() || !self.protocols.is_empty() {
            return Err(OpenError::InvalidProtocol {
                protocol: "catalog".into(),
                problem: "a catalog or host protocol is already registered".into(),
            });
        }
        let mut protocols = BTreeMap::new();
        for entry in catalog.iter() {
            let name = entry.name();
            let ir = compile(&entry.model).map_err(|problems| OpenError::InvalidProtocol {
                protocol: name.to_owned(),
                problem: problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            })?;
            representable(name, &ir)?;
            protocols.insert(name.to_owned(), ir);
        }
        self.protocols = protocols;
        self.catalog_definitions = Some(
            catalog
                .iter()
                .map(|entry| (entry.name().to_owned(), entry.definition.sha256.clone()))
                .collect(),
        );
        Ok(self)
    }

    /// Check that routing and artifact initialization use the exact admitted catalog.
    ///
    /// Legacy governors without explicit catalog admission fail this check; their existing
    /// protocol registration and case-opening methods remain available unchanged.
    pub fn validate_catalog(&self, catalog: &ProtocolCatalog) -> Result<(), OpenError> {
        let definitions: BTreeMap<_, _> = catalog
            .iter()
            .map(|entry| (entry.name().to_owned(), entry.definition.sha256.clone()))
            .collect();
        if self.catalog_definitions.as_ref() != Some(&definitions) {
            return Err(OpenError::InvalidProtocol {
                protocol: "catalog".into(),
                problem: "the supplied catalog differs from the governor's admitted definitions"
                    .into(),
            });
        }
        Ok(())
    }

    /// Parse and register a host-admitted protocol with the governor's pinned Canon. This keeps
    /// embedders independent of Canon crate identity; all registration checks still apply.
    pub fn with_protocol_yaml(self, name: &str, yaml: &str) -> Result<Self, OpenError> {
        let model = b10x_canon::model::parse(yaml).map_err(|error| OpenError::InvalidProtocol {
            protocol: name.to_owned(),
            problem: error.to_string(),
        })?;
        self.with_protocol(name, &model)
    }

    /// Register a protocol admitted by the trusted host. Canon validates and compiles the model;
    /// registration is not protocol-adoption authority. Models must never call this method.
    /// Existing registrations and built-ins cannot be replaced, even before a case is opened.
    /// Hosts restoring a persistent store must restore the same admitted protocol definitions
    /// under the same names; the store does not persist or authenticate protocol definitions.
    pub fn with_protocol(
        mut self,
        name: &str,
        model: &b10x_canon::model::Protocol,
    ) -> Result<Self, OpenError> {
        let invalid = |problem: String| OpenError::InvalidProtocol {
            protocol: name.to_owned(),
            problem,
        };
        if self.catalog_definitions.is_some() {
            return Err(invalid("the admitted catalog is immutable".into()));
        }
        if name.is_empty()
            || name.trim() != name
            || name.chars().any(char::is_control)
            || self.protocols.contains_key(name)
            || !matches!(protocol_ir(name), Err(OpenError::UnknownProtocol { .. }))
        {
            return Err(invalid(
                "protocol name is empty or already registered".into(),
            ));
        }
        let ir = compile(model).map_err(|problems| {
            invalid(
                problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        representable(name, &ir)?;
        self.protocols.insert(name.to_owned(), ir);
        Ok(self)
    }

    /// Use a trusted host callback for Canon freshness checks; no clock is read by the governor.
    pub fn with_evaluation_time<U: EvaluationTime>(self, time: U) -> CanonGovernor<S, U> {
        CanonGovernor {
            store: self.store,
            protocols: self.protocols,
            catalog_definitions: self.catalog_definitions,
            time,
        }
    }

    fn protocol_ir(&self, protocol: &str) -> Result<Ir, OpenError> {
        match self.protocols.get(protocol) {
            Some(ir) => Ok(ir.clone()),
            None if self.catalog_definitions.is_some() => Err(OpenError::UnknownProtocol {
                protocol: protocol.to_owned(),
            }),
            None => {
                let ir = protocol_ir(protocol)?;
                representable(protocol, &ir)?;
                Ok(ir)
            }
        }
    }

    /// Opens a case on registered `protocol` (`<name>@<major>`) with `revisions`, the
    /// revision of every artifact the protocol declares, and `inputs`, the value of every case
    /// input it declares, under the first free id `case-<n>`. Nothing is stored when either is
    /// refused (see the crate documentation, "Case inputs"). No producer is declared
    /// independent; [`CanonGovernor::open_with_independence`] declares some.
    pub fn open(
        &self,
        protocol: &str,
        revisions: BTreeMap<String, String>,
        inputs: CaseInputs,
    ) -> Result<CaseId, OpenError> {
        self.open_with_independence(protocol, revisions, inputs, CaseIndependence::none())
    }

    /// Opens a case as [`CanonGovernor::open`] does, with `independence`, the dimensions on which
    /// the host declares each evidence producer independent of the case's work. Nothing is
    /// stored when they are refused (see the crate documentation, "Independence").
    pub fn open_with_independence(
        &self,
        protocol: &str,
        revisions: BTreeMap<String, String>,
        inputs: CaseInputs,
        independence: CaseIndependence,
    ) -> Result<CaseId, OpenError> {
        let ir = checked(self.protocol_ir(protocol)?, &revisions, &inputs)?;
        checked_independence(&ir, &independence)?;
        let mut n = 1u64;
        loop {
            let case = CaseId(format!("case-{n}"));
            if self
                .store
                .insert(opened(&case, protocol, &revisions, &inputs, &independence))
                .map_err(|e| OpenError::StoreUnavailable {
                    problem: e.to_string(),
                })?
            {
                return Ok(case);
            }
            n += 1;
        }
    }

    /// Opens a case as [`CanonGovernor::open`] does, under the id `case`.
    pub fn open_case(
        &self,
        case: CaseId,
        protocol: &str,
        revisions: BTreeMap<String, String>,
        inputs: CaseInputs,
    ) -> Result<(), OpenError> {
        self.open_case_with_independence(
            case,
            protocol,
            revisions,
            inputs,
            CaseIndependence::none(),
        )
    }

    /// Opens a case as [`CanonGovernor::open_with_independence`] does, under the id `case`.
    pub fn open_case_with_independence(
        &self,
        case: CaseId,
        protocol: &str,
        revisions: BTreeMap<String, String>,
        inputs: CaseInputs,
        independence: CaseIndependence,
    ) -> Result<(), OpenError> {
        if !is_identifier(&case.0) {
            return Err(OpenError::InvalidCaseId { case: case.0 });
        }
        let ir = checked(self.protocol_ir(protocol)?, &revisions, &inputs)?;
        checked_independence(&ir, &independence)?;
        if self
            .store
            .insert(opened(&case, protocol, &revisions, &inputs, &independence))
            .map_err(|e| OpenError::StoreUnavailable {
                problem: e.to_string(),
            })?
        {
            Ok(())
        } else {
            Err(OpenError::CaseExists { case: case.0 })
        }
    }

    /// Records `revision` as the current revision of `artifact` and raises the case revision by
    /// one; returns the new case revision. When `artifact` already holds `revision` nothing
    /// changes, and the current case revision is returned.
    pub fn update_revision(
        &self,
        case: &CaseId,
        artifact: &str,
        revision: &str,
    ) -> Result<i64, UpdateError> {
        if !is_identifier(revision) {
            return Err(UpdateError::InvalidRevision {
                artifact: artifact.to_owned(),
            });
        }
        self.store
            .update(case, |state| {
                let current = state.artifacts.get_mut(artifact).ok_or_else(|| {
                    UpdateError::UndeclaredArtifact {
                        artifact: artifact.to_owned(),
                    }
                })?;
                if current.as_str() == revision {
                    return Ok(state.revision);
                }
                revision.clone_into(current);
                state.revision += 1;
                Ok(state.revision)
            })
            .map_err(|e| UpdateError::StoreUnavailable {
                problem: e.to_string(),
            })?
            .unwrap_or(Err(UpdateError::UnknownCase))
    }

    /// Records the explicit decision `decision`, taken for `outcome` by `principal`, on the case at
    /// its current revision, so later `frontier` and `completion` calls evaluate with it until the
    /// case moves. Recording a decision already held at that revision changes nothing. Canon judges
    /// it first: one it refuses for the case is [`DecisionError::Refused`] and nothing is kept.
    pub fn decide(
        &self,
        case: &CaseId,
        outcome: &str,
        decision: &str,
        principal: &str,
    ) -> Result<(), DecisionError> {
        let (_, ir) = self.held_with_ir(case).map_err(|error| match error {
            GovernorError::UnknownCase => DecisionError::UnknownCase,
            _ => DecisionError::StoreUnavailable {
                problem: "the held case cannot be read under its protocol".into(),
            },
        })?;
        self.store
            .update(case, |state| {
                let taken = ExplicitDecision {
                    decision: decision.to_owned(),
                    outcome: outcome.to_owned(),
                    principal: principal.to_owned(),
                    case_revision: format!("r{}", state.revision),
                };
                if state.decisions.contains(&taken) {
                    return Ok(());
                }
                let mut decisions = state.decisions.clone();
                decisions.push(taken);
                decide_case(&ir, &snapshot(&ir, state), &[], None, &decisions).map_err(
                    |refusal| DecisionError::Refused {
                        code: refusal.code().to_owned(),
                        message: refusal.to_string(),
                    },
                )?;
                state.decisions = decisions;
                Ok(())
            })
            .map_err(|e| DecisionError::StoreUnavailable {
                problem: e.to_string(),
            })?
            .unwrap_or(Err(DecisionError::UnknownCase))
    }

    /// The current revision of every artifact of the case.
    pub fn revisions(&self, case: &CaseId) -> Result<BTreeMap<String, String>, GovernorError> {
        Ok(self.held(case)?.artifacts)
    }

    /// The value of every case input the case opened with; `GovernorUnavailable` when the
    /// held inputs do not satisfy the case's protocol (see the crate documentation, "Case
    /// inputs").
    pub fn inputs(&self, case: &CaseId) -> Result<CaseInputs, GovernorError> {
        Ok(self.held(case)?.inputs)
    }

    /// The independence the host declared for each evidence producer when the case opened;
    /// `GovernorUnavailable` when the held declarations do not satisfy the case's protocol (see
    /// the crate documentation, "Independence").
    pub fn independence(&self, case: &CaseId) -> Result<CaseIndependence, GovernorError> {
        Ok(self.held(case)?.independence)
    }

    /// The registered protocol, `<name>@<major>`, the case is held under.
    pub fn protocol(&self, case: &CaseId) -> Result<String, GovernorError> {
        Ok(self.held(case)?.protocol)
    }

    /// Every evidence record received for the case, in the order received.
    pub fn evidence(&self, case: &CaseId) -> Result<Vec<EvidenceData>, GovernorError> {
        Ok(self
            .held(case)?
            .evidence
            .into_iter()
            .map(|held| held.data)
            .collect())
    }

    /// Every observation received, in the order received.
    pub fn try_observations(&self) -> Result<Vec<ObservationData>, GovernorError> {
        self.store
            .observations()
            .map_err(|_| GovernorError::GovernorUnavailable)
    }

    /// The `canon-decision/1` document Canon wrote for the case as held now, its explanation
    /// included: the decision `frontier` renders into its frontier id and `completion` reads its
    /// outcome from, at the case's current revision. It is one evaluation, made as theirs is (see
    /// the crate documentation, "Evaluation"), and the document is the generated JSON value
    /// [`evaluate`] returns as `canon`, equal to it for the same protocol, snapshot, evidence,
    /// explicit decisions and time.
    ///
    /// # Errors
    /// `UnknownCase` when the case is not held, and `GovernorUnavailable` when it cannot be decided
    /// (its store fails, its protocol is not registered, its held inputs or independence
    /// declarations do not satisfy the protocol, the trusted time fails or Canon refuses), as
    /// `frontier` refuses it.
    pub fn decision_document(&self, case: &CaseId) -> Result<loom::json::Value, GovernorError> {
        let (_, _, decision) = self.decided(case)?;
        loom::json::parse(&render(&decision)).map_err(|_| GovernorError::GovernorUnavailable)
    }

    /// The held case, after checking its inputs against its protocol as at opening.
    fn held(&self, case: &CaseId) -> Result<CaseState, GovernorError> {
        self.held_with_ir(case).map(|(state, _)| state)
    }

    /// The held case and its compiled protocol; `GovernorUnavailable` when the store fails, the
    /// protocol is not registered or the case's inputs do not satisfy it.
    fn held_with_ir(&self, case: &CaseId) -> Result<(CaseState, Ir), GovernorError> {
        let state = self
            .store
            .get(case)
            .map_err(|_| GovernorError::GovernorUnavailable)?
            .ok_or(GovernorError::UnknownCase)?;
        let ir = self
            .protocol_ir(&state.protocol)
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        checked_inputs(&ir, &state.inputs).map_err(|_| GovernorError::GovernorUnavailable)?;
        checked_independence(&ir, &state.independence)
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        Ok((state, ir))
    }

    /// Canon's decision for the case as held now, and the compiled protocol it was made under.
    fn decided(&self, case: &CaseId) -> Result<(CaseState, Ir, Decision), GovernorError> {
        let (state, ir) = self.held_with_ir(case)?;
        let records: Vec<EvidenceRecord> = state
            .evidence
            .iter()
            .filter(|held| held.applies)
            .filter_map(|held| canon_record(&held.data))
            .collect();
        let at = self
            .time
            .at(case)
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        let decision = decide_case(
            &ir,
            &snapshot(&ir, &state),
            &records,
            at.as_deref(),
            &state.decisions,
        )
        .map_err(|_| GovernorError::GovernorUnavailable)?;
        Ok((state, ir, decision))
    }
}

impl<S: CaseStore, T> CanonGovernor<S, T> {
    /// Infallible compatibility accessor. Fallible stores use [`Self::try_observations`].
    pub fn observations(&self) -> Vec<ObservationData> {
        CaseStore::observations(&self.store)
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> Governor for CanonGovernor<S, T> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.held(case)?.revision)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let (state, ir, decision) = self.decided(case)?;
        let projection = project(&ir, &decision).ok_or(GovernorError::GovernorUnavailable)?;
        let claims = projection
            .claims
            .into_iter()
            .map(|(claim, value)| FrontierClaim {
                claim,
                value: match value {
                    CanonTruth::True => Truth::True,
                    CanonTruth::False => Truth::False,
                    CanonTruth::Unknown => Truth::Unknown,
                },
            })
            .collect();
        let obligations = projection
            .obligations
            .into_iter()
            .map(|(obligation, open)| FrontierObligation { obligation, open })
            .collect();
        let actions = projection
            .actions
            .into_iter()
            .map(|action| FrontierAction {
                action: action.action,
                status: match action.status {
                    Status::Admissible => ActionStatus::Admissible,
                    Status::ApprovalRequired => ActionStatus::ApprovalRequired,
                    Status::Blocked => ActionStatus::Blocked,
                },
                capability: action.requires.into_iter().next(),
                reasons: action.reasons,
            })
            .collect();
        let rendered = render(&decision);
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(frontier_uuid(&state, &rendered)),
            case_id: state.id,
            case_revision: state.revision,
            claims,
            obligations,
            actions,
        }))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        let (_, _, decision) = self.decided(case)?;
        Ok(match completed(&decision) {
            Some(outcome) => {
                CompletionDetermination::Complete(CompletionDeterminationComplete { outcome })
            }
            None => CompletionDetermination::Open(Unit(true)),
        })
    }
}

impl<S: FallibleCaseStore, T> CanonGovernor<S, T> {
    /// Whether the governor takes `data`: every observation the record cites was received by this
    /// governor ([`CanonGovernor::try_observations`]), whether or not its facts carry an
    /// `observed_at`, and every `observed_at` member of its facts is the text of each cited
    /// observation's `observed_at`. A record that fails either is refused by
    /// [`EvidencePort::receive`], which keeps nothing of it; this names the record and why (see
    /// the module documentation, "Evidence").
    ///
    /// # Errors
    /// [`EvidenceRefusal`], naming the record.
    pub fn check_evidence(&self, data: &EvidenceData) -> Result<(), EvidenceRefusal> {
        let evidence = data.evidence_id.0.0.clone();
        // Every member named `observed_at`, so a second one with another instant is checked too.
        let observed_at: Vec<&json::Value> = match &data.facts {
            json::Value::Object(members) => members
                .iter()
                .filter(|(name, _)| name == "observed_at")
                .map(|(_, value)| value)
                .collect(),
            _ => Vec::new(),
        };
        if data.observation_ids.is_empty() {
            return Ok(());
        }
        let received = self
            .store
            .observations()
            .map_err(|_| EvidenceRefusal::Unavailable {
                evidence: evidence.clone(),
            })?;
        for cited in &data.observation_ids {
            let Some(observation) = received
                .iter()
                .find(|observation| &observation.observation_id == cited)
            else {
                return Err(EvidenceRefusal::ObservationNotReceived {
                    evidence,
                    observation: cited.0.0.clone(),
                });
            };
            for value in &observed_at {
                match value {
                    json::Value::Text(at) if *at == observation.observed_at.0 => {}
                    other => {
                        return Err(EvidenceRefusal::ObservedAtDiffers {
                            evidence,
                            observation: cited.0.0.clone(),
                            observed_at: match other {
                                json::Value::Text(at) => at.clone(),
                                _ => "a value that is not text".to_owned(),
                            },
                            observation_at: observation.observed_at.0.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> EvidencePort for CanonGovernor<S, T> {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        let data = evidence.into_evidence().into_data();
        let (_, ir) = self.held_with_ir(&data.case_id)?;
        // Commission's `GovernorError` names no record; `check_evidence` names it for the host.
        self.check_evidence(&data)
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        let case = data.case_id.clone();
        let at = self
            .time
            .at(&case)
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        self.store
            .update(&case, |state| {
                let applies = applies(&ir, state, &data, at.as_deref());
                state.evidence.push(HeldEvidence { data, applies });
            })
            .map_err(|_| GovernorError::GovernorUnavailable)?
            .ok_or(GovernorError::UnknownCase)
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> ObservationPort for CanonGovernor<S, T> {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.store
            .observe(observation.into_data())
            .map_err(|_| GovernorError::GovernorUnavailable)
    }
}

/// The compiled protocol, after checking `revisions` and `inputs` against what it declares.
fn checked(
    ir: Ir,
    revisions: &BTreeMap<String, String>,
    inputs: &CaseInputs,
) -> Result<Ir, OpenError> {
    let ir = checked_revisions(ir, revisions)?;
    checked_inputs(&ir, inputs)?;
    Ok(ir)
}

/// Checks `inputs` against the case inputs `ir` declares, as Canon checks a case's: each input
/// given, in input order, is declared and has a value its input lists; then each declared input,
/// in input order, is given.
fn checked_inputs(ir: &Ir, inputs: &CaseInputs) -> Result<(), OpenError> {
    for (input, value) in inputs.iter() {
        let Some(declared) = ir.inputs.get(&InputId::new(input)) else {
            return Err(OpenError::UndeclaredInput {
                input: input.to_owned(),
            });
        };
        if !declared
            .values
            .iter()
            .any(|listed| listed.as_str() == value)
        {
            return Err(OpenError::UnlistedInputValue {
                input: input.to_owned(),
                value: value.to_owned(),
            });
        }
    }
    if let Some(missing) = ir
        .inputs
        .keys()
        .find(|input| inputs.get(input.as_str()).is_none())
    {
        return Err(OpenError::MissingInput {
            input: missing.as_str().to_owned(),
        });
    }
    Ok(())
}

/// Checks `independence` against the dimensions `ir` declares, as Canon checks a case's: each
/// producer, in producer order, is an identifier, and each dimension listed for it, in dimension
/// order, is one the protocol declares.
fn checked_independence(ir: &Ir, independence: &CaseIndependence) -> Result<(), OpenError> {
    for (producer, mut dimensions) in independence.iter() {
        if !is_identifier(producer) {
            return Err(OpenError::InvalidProducer {
                producer: producer.to_owned(),
            });
        }
        if let Some(dimension) = dimensions.find(|dimension| {
            !ir.independence_dimensions
                .contains_key(&IndependenceDimensionId::new(*dimension))
        }) {
            return Err(OpenError::UndeclaredDimension {
                producer: producer.to_owned(),
                dimension: dimension.to_owned(),
            });
        }
    }
    Ok(())
}

/// Checks `inputs` against the case inputs the ELS built-in `protocol` declares, when it names
/// one; a protocol that is not built in is the registering governor's to check.
pub(crate) fn checked_builtin_inputs(protocol: &str, inputs: &CaseInputs) -> Result<(), OpenError> {
    match protocol_ir(protocol) {
        Ok(ir) => checked_inputs(&ir, inputs),
        Err(OpenError::UnknownProtocol { .. }) => Ok(()),
        Err(other) => Err(other),
    }
}

/// Checks `independence` against the dimensions the ELS built-in `protocol` declares, when it
/// names one; a protocol that is not built in is the registering governor's to check.
pub(crate) fn checked_builtin_independence(
    protocol: &str,
    independence: &CaseIndependence,
) -> Result<(), OpenError> {
    match protocol_ir(protocol) {
        Ok(ir) => checked_independence(&ir, independence),
        Err(OpenError::UnknownProtocol { .. }) => Ok(()),
        Err(other) => Err(other),
    }
}

/// The compiled protocol, after checking `revisions` against the artifacts it declares.
fn checked_revisions(ir: Ir, revisions: &BTreeMap<String, String>) -> Result<Ir, OpenError> {
    if let Some(artifact) = ir
        .artifacts
        .keys()
        .find(|artifact| !revisions.contains_key(artifact.as_str()))
    {
        return Err(OpenError::MissingRevision {
            artifact: artifact.as_str().to_owned(),
        });
    }
    for (artifact, revision) in revisions {
        if !ir
            .artifacts
            .contains_key(&ArtifactId::new(artifact.as_str()))
        {
            return Err(OpenError::UndeclaredArtifact {
                artifact: artifact.clone(),
            });
        }
        if !is_identifier(revision) {
            return Err(OpenError::InvalidRevision {
                artifact: artifact.clone(),
            });
        }
    }
    Ok(ir)
}

fn representable(protocol: &str, ir: &Ir) -> Result<(), OpenError> {
    if ir.actions.values().any(|action| action.requires.len() > 1) {
        return Err(OpenError::InvalidProtocol {
            protocol: protocol.into(),
            problem: "Commission frontier supports at most one capability per action".into(),
        });
    }
    Ok(())
}

/// A case just opened: revision 1, no evidence.
fn opened(
    case: &CaseId,
    protocol: &str,
    revisions: &BTreeMap<String, String>,
    inputs: &CaseInputs,
    independence: &CaseIndependence,
) -> CaseState {
    CaseState {
        id: case.clone(),
        protocol: protocol.to_owned(),
        revision: 1,
        artifacts: revisions.clone(),
        inputs: inputs.clone(),
        independence: independence.clone(),
        evidence: Vec::new(),
        decisions: Vec::new(),
    }
}

/// The ELS built-in `protocol`, `<name>@<major>`, compiled by Canon.
fn protocol_ir(protocol: &str) -> Result<Ir, OpenError> {
    let unknown = || OpenError::UnknownProtocol {
        protocol: protocol.to_owned(),
    };
    let (name, major) = protocol.split_once('@').ok_or_else(unknown)?;
    let major: u32 = major
        .parse()
        .ok()
        .filter(|parsed: &u32| parsed.to_string() == major)
        .ok_or_else(unknown)?;
    let builtin = canon_engineering::registry::get(name, major).map_err(|error| match error {
        canon_engineering::registry::Error::UnknownName { .. }
        | canon_engineering::registry::Error::UnknownMajor { .. } => unknown(),
        other => OpenError::InvalidProtocol {
            protocol: protocol.to_owned(),
            problem: other.to_string(),
        },
    })?;
    compile(&builtin.model).map_err(|problems| OpenError::InvalidProtocol {
        protocol: protocol.to_owned(),
        problem: problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "),
    })
}

/// The case's `canon-case/1` snapshot.
fn snapshot(ir: &Ir, state: &CaseState) -> Case {
    Case {
        format: CASE_FORMAT.to_owned(),
        id: b10x_canon::model::CaseId::new(state.id.0.as_str()),
        protocol: ir.protocol.id.clone(),
        artifacts: Declarations::new(
            state
                .artifacts
                .iter()
                .map(|(artifact, revision)| {
                    (
                        ArtifactId::new(artifact.as_str()),
                        CaseArtifact {
                            revision: Revision::new(revision.as_str()),
                        },
                    )
                })
                .collect(),
        ),
        termination: None,
        revision: Some(Revision::new(format!("r{}", state.revision))),
        inputs: Declarations::new(
            state
                .inputs
                .iter()
                .map(|(input, value)| (InputId::new(input), InputValue::new(value)))
                .collect(),
        ),
        // Who is independent of the case's work is the host's declaration, made when the case
        // opened; a producer it does not list satisfies no demand for independence.
        independence: Declarations::new(
            state
                .independence
                .iter()
                .map(|(producer, dimensions)| {
                    (
                        Principal::new(producer),
                        dimensions.map(IndependenceDimensionId::new).collect(),
                    )
                })
                .collect(),
        ),
    }
}

/// The members of a record's facts that are the host's and not `canon-evidence/1` fields, removed
/// before Canon reads the record: `target`, the queue or owner a routing record names
/// (`story:routing-record-names-target`).
const HOST_FACTS: &[&str] = &["target"];

/// The `canon-evidence/1` record `data`'s facts carry, when they carry one of its kind, naming
/// the trusted producer `submit_evidence` attributed it to. A producer the facts name is never
/// read: its `producer` member is removed before Canon reads the record, so whatever it holds (a
/// null, a number, an object) never decides whether the record applies. A trusted producer Canon
/// would refuse as an identifier (one holding whitespace) is named as no producer, which
/// satisfies no demand for independence. The host's facts (`HOST_FACTS`) are removed too: they
/// are not fields of the record, which Canon would refuse.
fn canon_record(data: &EvidenceData) -> Option<EvidenceRecord> {
    let mut value = canon_value(&data.facts)?;
    if let serde_yaml_ng::Value::Mapping(members) = &mut value {
        members.remove("producer");
        for name in HOST_FACTS {
            members.remove(*name);
        }
    }
    let mut record = evidence_from_value(&value).ok()?;
    record.producer = is_identifier(&data.producer).then(|| Principal::new(data.producer.as_str()));
    (record.kind.as_str() == data.kind).then_some(record)
}

/// `value` as Canon's value type, built from it directly with no text in between. A number is read
/// from its spelling as Canon's YAML reader reads one; `None` when a number's spelling does not
/// read as one, or an object names a member twice (which Canon's reader refuses).
fn canon_value<V: JsonTree>(value: &V) -> Option<serde_yaml_ng::Value> {
    use serde_yaml_ng::{Mapping, Number, Value as Yaml};
    Some(match value.node() {
        Node::Null => Yaml::Null,
        Node::Bool(boolean) => Yaml::Bool(boolean),
        Node::Number(spelling) => Yaml::Number(spelling.parse::<Number>().ok()?),
        Node::Text(text) => Yaml::String(text.to_owned()),
        Node::Array(items) => Yaml::Sequence(items.iter().map(canon_value).collect::<Option<_>>()?),
        Node::Object(members) => {
            let mut mapping = Mapping::with_capacity(members.len());
            for (name, member) in members {
                let previous = mapping.insert(Yaml::String(name.clone()), canon_value(member)?);
                if previous.is_some() {
                    return None;
                }
            }
            Yaml::Mapping(mapping)
        }
    })
}

/// One JSON value seen through its shape, so Commission's and Loom's generated JSON values reach
/// Canon by the one reader, [`canon_value`].
enum Node<'a, V> {
    Null,
    Bool(bool),
    Number(&'a str),
    Text(&'a str),
    Array(&'a [V]),
    Object(&'a [(String, V)]),
}

trait JsonTree: Sized {
    fn node(&self) -> Node<'_, Self>;
}

impl JsonTree for json::Value {
    fn node(&self) -> Node<'_, Self> {
        match self {
            Self::Null => Node::Null,
            Self::Bool(boolean) => Node::Bool(*boolean),
            Self::Number(spelling) => Node::Number(spelling),
            Self::Text(text) => Node::Text(text),
            Self::Array(items) => Node::Array(items),
            Self::Object(members) => Node::Object(members),
        }
    }
}

impl JsonTree for loom::json::Value {
    fn node(&self) -> Node<'_, Self> {
        match self {
            Self::Null => Node::Null,
            Self::Bool(boolean) => Node::Bool(*boolean),
            Self::Number(spelling) => Node::Number(spelling),
            Self::Text(text) => Node::Text(text),
            Self::Array(items) => Node::Array(items),
            Self::Object(members) => Node::Object(members),
        }
    }
}

/// Whether `data` applies to the case: its facts carry a record of its kind, and Canon evaluates
/// the case with it and the records that already apply.
fn applies(ir: &Ir, state: &CaseState, data: &EvidenceData, at: Option<&str>) -> bool {
    let Some(record) = canon_record(data) else {
        return false;
    };
    let mut records: Vec<EvidenceRecord> = state
        .evidence
        .iter()
        .filter(|held| held.applies)
        .filter_map(|held| canon_record(&held.data))
        .collect();
    records.push(record);
    decide_case(ir, &snapshot(ir, state), &records, at, &state.decisions).is_ok()
}

/// Canon's decision for `case` under `ir` from `records`, at the trusted instant `at` when one is
/// given, with the explicit `decisions` (none supplied when empty) and no authority decision. Every
/// evaluation the governor makes goes through here: the held case's ([`CanonGovernor`]'s
/// `decided`), an arriving record's ([`applies`]), a decision being recorded
/// ([`CanonGovernor::decide`]) and a caller's snapshot ([`evaluate`]).
fn decide_case(
    ir: &Ir,
    case: &Case,
    records: &[EvidenceRecord],
    at: Option<&str>,
    decisions: &[ExplicitDecision],
) -> Result<Decision, Refusal> {
    let decisions = (!decisions.is_empty()).then(|| canon_decisions(decisions));
    evaluate_with(
        ir,
        case,
        records,
        Supplied {
            at,
            decisions: decisions.as_deref(),
            ..Supplied::default()
        },
    )
}

/// `decisions` as a `canon-decisions/1` document: a JSON list, which Canon's YAML reader reads as
/// the values it holds, every name a string whatever it spells.
fn canon_decisions(decisions: &[ExplicitDecision]) -> String {
    let mut out = String::from("[");
    for (index, entry) in decisions.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let members = [
            ("decision", &entry.decision),
            ("outcome", &entry.outcome),
            ("principal", &entry.principal),
            ("case_revision", &entry.case_revision),
        ];
        out.push('{');
        for (position, (name, value)) in members.into_iter().enumerate() {
            if position > 0 {
                out.push(',');
            }
            loom::json::push_text(&mut out, name);
            out.push(':');
            loom::json::push_text(&mut out, value);
        }
        out.push('}');
    }
    out.push(']');
    out
}

/// Canon's status of one action.
#[derive(Debug, Clone, Copy)]
enum Status {
    Admissible,
    ApprovalRequired,
    Blocked,
}

/// One declared action as the decision gives it: its status, every capability the protocol says
/// it requires, and Canon's reasons, each as the compact JSON Canon gives it.
struct ProjectedAction {
    action: String,
    status: Status,
    requires: Vec<String>,
    reasons: Vec<String>,
}

/// Canon's decision as the governor reports it: every claim's value, every obligation and whether
/// it is open, and every declared action in the protocol's order.
struct Projection {
    claims: Vec<(String, CanonTruth)>,
    obligations: Vec<(String, bool)>,
    actions: Vec<ProjectedAction>,
}

/// `decision` projected as the frontier reports it; `None` when it gives a declared action no
/// status, or one the governor does not know.
fn project(ir: &Ir, decision: &Decision) -> Option<Projection> {
    let claims = decision
        .claims
        .iter()
        .map(|(claim, entry)| (claim.as_str().to_owned(), entry.value))
        .collect();
    let obligations = decision
        .obligations
        .as_ref()
        .and_then(|section| section.as_array())
        .into_iter()
        .flatten()
        .map(|entry| {
            (
                entry["id"].as_str().unwrap_or_default().to_owned(),
                // Open only as Canon reports it: `discharged` and `not_applicable` are not open.
                entry["status"] == "open",
            )
        })
        .collect();
    let section = decision.actions.as_ref().and_then(|s| s.as_object());
    let mut actions = Vec::with_capacity(ir.actions.len());
    for (id, declared) in &ir.actions {
        let entry = section?.get(id.as_str())?;
        let status = match entry["status"].as_str() {
            Some("admissible") => Status::Admissible,
            Some("approval-required") => Status::ApprovalRequired,
            Some("blocked") => Status::Blocked,
            _ => return None,
        };
        actions.push(ProjectedAction {
            action: id.as_str().to_owned(),
            status,
            requires: declared
                .requires
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            reasons: entry["reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .map(ToString::to_string)
                .collect(),
        });
    }
    Some(Projection {
        claims,
        obligations,
        actions,
    })
}

/// The case's outcome when exactly one declared outcome is `legitimate` in `decision`: never an
/// outcome Canon holds blocked, and never a choice between several.
fn completed(decision: &Decision) -> Option<String> {
    let legitimate: Vec<&String> = decision
        .outcomes
        .as_ref()
        .and_then(|section| section.as_object())
        .into_iter()
        .flatten()
        .filter(|(_, entry)| entry["status"] == "legitimate")
        .map(|(outcome, _)| outcome)
        .collect();
    match legitimate.as_slice() {
        [outcome] => Some((*outcome).clone()),
        _ => None,
    }
}

/// Canon's decision for a case the caller keeps: `request` names a protocol of the host's
/// `catalog` (`<name>@<major>`), a `canon-case/1` snapshot, the `canon-evidence/1` records and an
/// optional trusted instant, and the explicit `canon-decisions/1` decisions taken for the case.
/// Nothing is held, no clock is read and no authority decision is supplied; the decision is
/// reported as [`CanonGovernor`] reports its frontier and completion. A decision applies only at
/// the snapshot's revision: one taken at another revision is superseded, and a termination only
/// it would make legitimate is refused, as Canon refuses it.
///
/// A record that reads as a `canon-evidence/1` record but that Canon refuses for this case (a kind
/// or subject the protocol does not declare, for instance) is set aside, as `CanonGovernor` sets it
/// aside when it arrives, and the decision is made from the rest; the decision does not list it.
///
/// Two inputs differ from `CanonGovernor`: a record that is not a readable `canon-evidence/1`
/// record, and a record repeating an earlier record's id. The governor sets either aside (for a
/// repeated id it keeps the first record) and still decides; `evaluate` refuses the request,
/// naming the record (`duplicate-identifier` and the later position for a repeated id).
///
/// A refusal names the input it is about: an unknown or uncompilable protocol; a snapshot that is
/// not a `canon-case/1` document, that Canon refuses for the protocol, or whose termination the
/// records do not make legitimate; a record that is not a readable `canon-evidence/1` record, or
/// that repeats an earlier record's id (by its position in `evidence`, the later one); a time
/// Canon cannot read; or decisions Canon refuses (an entry that is not an identifier, one given
/// twice, one for an undeclared outcome or for an outcome that does not require it). The time,
/// the decisions and the records are judged before the decision is made, once each.
pub fn evaluate(
    catalog: &ProtocolCatalog,
    request: &EvaluationRequest,
) -> Result<EvaluationDecision, EvaluationRefusal> {
    let protocol = request.protocol.0.as_str();
    let entry = catalog.get(protocol).ok_or_else(|| {
        refused(
            EvaluationInput::Protocol,
            None,
            "unknown-protocol",
            format!("the host catalog holds no protocol `{protocol}`"),
        )
    })?;
    let ir = compile(&entry.model).map_err(|problems| {
        refused(
            EvaluationInput::Protocol,
            None,
            "invalid-protocol",
            format!(
                "protocol `{protocol}` does not compile: {}",
                problems
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )
    })?;
    let case = canon_value(&request.snapshot.0)
        .ok_or_else(|| malformed(EvaluationInput::Snapshot, None))
        .and_then(|value| {
            case_from_value(&value)
                .map_err(|refusal| canon_refused(EvaluationInput::Snapshot, None, &refusal))
        })?;
    let records = request
        .evidence
        .iter()
        .enumerate()
        .map(|(index, record)| {
            canon_value(&record.0)
                .ok_or_else(|| malformed(EvaluationInput::Evidence, Some(index)))
                .and_then(|value| {
                    evidence_from_value(&value).map_err(|refusal| {
                        canon_refused(EvaluationInput::Evidence, Some(index), &refusal)
                    })
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        if !seen.insert(record.id.as_str()) {
            return Err(refused(
                EvaluationInput::Evidence,
                Some(index),
                "duplicate-identifier",
                format!("evidence `{}` is given more than once", record.id.as_str()),
            ));
        }
    }
    let at = request.at.as_ref().map(|at| at.0.as_str());
    // The snapshot and then the time, each judged before any record and on the snapshot without its
    // termination, whose legitimacy only the records can establish.
    let open = Case {
        termination: None,
        ..case.clone()
    };
    let decisions = request.decisions.as_slice();
    decide_case(&ir, &open, &[], None, &[])
        .map_err(|refusal| canon_refused(EvaluationInput::Snapshot, None, &refusal))?;
    decide_case(&ir, &open, &[], at, &[])
        .map_err(|refusal| canon_refused(EvaluationInput::Time, None, &refusal))?;
    decide_case(&ir, &open, &[], at, decisions)
        .map_err(|refusal| canon_refused(EvaluationInput::Decisions, None, &refusal))?;
    let decision = match decide_case(&ir, &case, &records, at, decisions) {
        Ok(decision) => decision,
        Err(_) => {
            // As `CanonGovernor` does on arrival, a record Canon refuses for this case is set
            // aside; what Canon still refuses with the rest is the snapshot's termination.
            let applying: Vec<EvidenceRecord> = records
                .iter()
                .filter(|record| {
                    decide_case(&ir, &open, std::slice::from_ref(record), at, decisions).is_ok()
                })
                .cloned()
                .collect();
            decide_case(&ir, &case, &applying, at, decisions)
                .map_err(|refusal| canon_refused(EvaluationInput::Snapshot, None, &refusal))?
        }
    };
    let unrepresentable = |problem: &str| {
        refused(
            EvaluationInput::Protocol,
            None,
            "unrepresentable-decision",
            format!("Canon's decision under protocol `{protocol}` {problem}"),
        )
    };
    let projection = project(&ir, &decision)
        .ok_or_else(|| unrepresentable("does not give every declared action a known status"))?;
    let as_json = |text: &str| {
        loom::json::parse(text).map_err(|_| unrepresentable("does not read as Loom's JSON"))
    };
    let actions = projection
        .actions
        .into_iter()
        .map(|action| {
            Ok(DecidedAction {
                action: action.action,
                status: match action.status {
                    Status::Admissible => DecidedStatus::Admissible,
                    Status::ApprovalRequired => DecidedStatus::ApprovalRequired,
                    Status::Blocked => DecidedStatus::Blocked,
                },
                requires: action.requires,
                reasons: action
                    .reasons
                    .iter()
                    .map(|reason| as_json(reason))
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(EvaluationDecision {
        protocol: request.protocol.clone(),
        case: decision.case.as_str().to_owned(),
        actions,
        claims: projection
            .claims
            .into_iter()
            .map(|(claim, value)| DecidedClaim {
                claim,
                value: match value {
                    CanonTruth::True => ClaimValue::True,
                    CanonTruth::False => ClaimValue::False,
                    CanonTruth::Unknown => ClaimValue::Unknown,
                },
            })
            .collect(),
        obligations: projection
            .obligations
            .into_iter()
            .map(|(obligation, open)| DecidedObligation { obligation, open })
            .collect(),
        outcome: completed(&decision),
        canon: as_json(&render(&decision))?,
    })
}

fn refused(
    input: EvaluationInput,
    index: Option<usize>,
    code: &str,
    message: String,
) -> EvaluationRefusal {
    EvaluationRefusal {
        input,
        evidence_index: index.map(|index| i64::try_from(index).unwrap_or(i64::MAX)),
        code: code.to_owned(),
        message,
    }
}

fn canon_refused(
    input: EvaluationInput,
    index: Option<usize>,
    refusal: &Refusal,
) -> EvaluationRefusal {
    refused(input, index, refusal.code(), refusal.to_string())
}

/// A JSON value Canon's reader cannot take: an object naming a member twice, or a number whose
/// spelling Canon's YAML reader does not read as one.
fn malformed(input: EvaluationInput, index: Option<usize>) -> EvaluationRefusal {
    refused(
        input,
        index,
        "malformed",
        "the value names an object member twice or spells a number Canon cannot read".into(),
    )
}

/// A UUID (version 8) derived from the case, its revision and Canon's decision bytes.
fn frontier_uuid(state: &CaseState, decision: &str) -> Uuid {
    let input = format!("{}\n{}\n{decision}", state.id.0, state.revision);
    let high = fnv1a(0xcbf2_9ce4_8422_2325, input.as_bytes());
    let low = fnv1a(0x6c62_272e_07bb_0142, input.as_bytes());
    let bits = (u128::from(high) << 64 | u128::from(low)) & !(0xf << 76) & !(0x3 << 62)
        | (0x8 << 76)
        | (0x2 << 62);
    let hex = format!("{bits:032x}");
    Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// 64-bit FNV-1a of `bytes` from `basis`.
fn fnv1a(basis: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(basis, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
