#![forbid(unsafe_code)]

//! Evaluates a case's protocol with Canon behind the governor and evidence ports Commission
//! defines.
//!
//! [`CanonGovernor`] implements Commission's [`Governor`], [`EvidencePort`] and [`ObservationPort`]
//! over Canon. A case is opened on an ELS built-in protocol, named `<name>@<major>`
//! (`software-change@1`), with the caller's revision of every artifact the protocol declares
//! ([`CanonGovernor::open`]). Its only other writes are a new artifact revision
//! ([`CanonGovernor::update_revision`]), evidence and observations.
//!
//! # Evaluation
//!
//! Every `frontier` and `completion` call evaluates afresh: the protocol is taken from the ELS
//! registry, compiled with `b10x_canon::ir::compile` and evaluated with
//! `b10x_canon::eval::evaluate_with` over the case's `canon-case/1` snapshot and the evidence that
//! applies. Nothing else is supplied: no evaluation instant (the governor reads no clock), no
//! explicit decision and no authority decision. The governor never invents authority, so an action
//! that requires a capability is at best `approval-required`; Commission asks its authority
//! provider for it.
//!
//! The frontier lists every action the protocol declares, with Canon's status, the capability the
//! action requires (when it requires one) and, unless it is admissible, Canon's reasons, each
//! written as the compact JSON Canon gives it. It also lists every claim's value and every
//! obligation, open unless Canon holds it discharged. It is issued for the case's current
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
//! carries no record. Every record received for a held case is
//! kept, in the order received ([`CanonGovernor::evidence`]); it applies only when its facts read
//! as such a record and Canon evaluates the case with it and the records that already apply
//! without refusal (a declared kind and subject, an id used once). Whether it applies is decided
//! once, when it arrives, so a record that does not apply never stops a later evaluation. Which
//! artifact revision a record is about is Canon's to judge: a record bound to a superseded
//! revision of its subject is excluded by Canon's revision binding.
//!
//! Observations are kept apart, in the order received ([`CanonGovernor::observations`]), and are
//! never evidence (Atlas ADR 0074).
//!
//! # Dependencies
//!
//! Canon is the library `b10x-canon-engineering` uses, `branch = "main"`, pinned by `Cargo.lock`
//! to commit `d2e09ae` (`66c8d4b` plus a documentation merge), so the protocol model the ELS
//! registry returns is the one this crate compiles. Commission is pinned to `e61e4f0`, and
//! `b10x-canon-engineering` to tag `0.1.0` of beyond10x/engineering-protocols.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use b10x_canon::eval::{Supplied, evaluate_with, evidence_from_value};
use b10x_canon::ir::{Ir, compile};
use b10x_canon::model::{
    ArtifactId, CASE_FORMAT, Case, CaseArtifact, Decision, Declarations, EvidenceRecord, Revision,
    Truth as CanonTruth, is_identifier,
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

/// One case as the governor holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseState {
    /// The case id.
    pub id: CaseId,
    /// The ELS built-in that governs it, `<name>@<major>`.
    pub protocol: String,
    /// The case revision: 1 when opened, one more with each artifact revision recorded.
    pub revision: i64,
    /// The current revision of every artifact the protocol declares.
    pub artifacts: BTreeMap<String, String>,
    /// Every evidence record received for the case, in the order received.
    pub evidence: Vec<HeldEvidence>,
}

/// An evidence record as received, and whether it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEvidence {
    /// The record as `submit_evidence` admitted it.
    pub data: EvidenceData,
    /// Whether it applies: its facts read as a `canon-evidence/1` record that Canon evaluates.
    pub applies: bool,
}

/// Where the governor keeps cases and observations. In memory for now ([`MemoryCaseStore`]).
pub trait CaseStore {
    /// Holds `state` under its id; `false`, holding nothing new, when that id is already held.
    ///
    /// `false` means exactly that the id is already held, and nothing else. A store that cannot
    /// write must not return `false`: [`CanonGovernor::open`] takes `false` as "try the next id"
    /// and would never stop. The trait has no error channel yet.
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
    /// The ELS registry holds no built-in `protocol`, or it is not `<name>@<major>`.
    UnknownProtocol { protocol: String },
    /// The built-in does not parse, validate or compile with Canon.
    InvalidProtocol { protocol: String, problem: String },
    /// The protocol declares `artifact`, and the revisions give none for it.
    MissingRevision { artifact: String },
    /// The revisions name `artifact`, which the protocol does not declare.
    UndeclaredArtifact { artifact: String },
    /// The revision given for `artifact` is not an identifier.
    InvalidRevision { artifact: String },
    /// The case id is not an identifier.
    InvalidCaseId { case: String },
    /// A case with this id is already held.
    CaseExists { case: String },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProtocol { protocol } => {
                write!(f, "the ELS registry holds no protocol `{protocol}`")
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
            Self::InvalidCaseId { case } => write!(f, "case id `{case}` is not an identifier"),
            Self::CaseExists { case } => write!(f, "case `{case}` is already held"),
        }
    }
}

impl std::error::Error for OpenError {}

/// Why an artifact revision was not recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
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

/// A governor that decides with Canon, over cases kept in `S`.
#[derive(Debug, Default)]
pub struct CanonGovernor<S> {
    store: S,
}

impl<S: CaseStore> CanonGovernor<S> {
    /// A governor over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Opens a case on the ELS built-in `protocol` (`<name>@<major>`) with `revisions`, the
    /// revision of every artifact the protocol declares, under the first free id `case-<n>`.
    pub fn open(
        &self,
        protocol: &str,
        revisions: BTreeMap<String, String>,
    ) -> Result<CaseId, OpenError> {
        checked(protocol, &revisions)?;
        let mut n = 1u64;
        loop {
            let case = CaseId(format!("case-{n}"));
            if self.store.insert(opened(&case, protocol, &revisions)) {
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
    ) -> Result<(), OpenError> {
        if !is_identifier(&case.0) {
            return Err(OpenError::InvalidCaseId { case: case.0 });
        }
        checked(protocol, &revisions)?;
        if self.store.insert(opened(&case, protocol, &revisions)) {
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
            .unwrap_or(Err(UpdateError::UnknownCase))
    }

    /// The current revision of every artifact of the case.
    pub fn revisions(&self, case: &CaseId) -> Result<BTreeMap<String, String>, GovernorError> {
        Ok(self.held(case)?.artifacts)
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
    pub fn observations(&self) -> Vec<ObservationData> {
        self.store.observations()
    }

    fn held(&self, case: &CaseId) -> Result<CaseState, GovernorError> {
        self.store.get(case).ok_or(GovernorError::UnknownCase)
    }

    /// Canon's decision for the case as held now, and the compiled protocol it was made under.
    fn decide(&self, case: &CaseId) -> Result<(CaseState, Ir, Decision), GovernorError> {
        let state = self.held(case)?;
        let ir = protocol_ir(&state.protocol).map_err(|_| GovernorError::GovernorUnavailable)?;
        let records: Vec<EvidenceRecord> = state
            .evidence
            .iter()
            .filter(|held| held.applies)
            .filter_map(|held| canon_record(&held.data))
            .collect();
        let decision = evaluate_with(&ir, &snapshot(&ir, &state), &records, Supplied::default())
            .map_err(|_| GovernorError::GovernorUnavailable)?;
        Ok((state, ir, decision))
    }
}

impl<S: CaseStore> Governor for CanonGovernor<S> {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.held(case)?.revision)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let (state, ir, decision) = self.decide(case)?;
        let claims = decision
            .claims
            .iter()
            .map(|(claim, entry)| FrontierClaim {
                claim: claim.as_str().to_owned(),
                value: match entry.value {
                    CanonTruth::True => Truth::True,
                    CanonTruth::False => Truth::False,
                    CanonTruth::Unknown => Truth::Unknown,
                },
            })
            .collect();
        let obligations = decision
            .obligations
            .as_ref()
            .and_then(|section| section.as_array())
            .into_iter()
            .flatten()
            .map(|entry| FrontierObligation {
                obligation: entry["id"].as_str().unwrap_or_default().to_owned(),
                open: entry["status"] != "discharged",
            })
            .collect();
        let section = decision.actions.as_ref().and_then(|s| s.as_object());
        let mut actions = Vec::with_capacity(ir.actions.len());
        for (id, declared) in &ir.actions {
            let entry = section
                .and_then(|section| section.get(id.as_str()))
                .ok_or(GovernorError::GovernorUnavailable)?;
            let status = match entry["status"].as_str() {
                Some("admissible") => ActionStatus::Admissible,
                Some("approval-required") => ActionStatus::ApprovalRequired,
                Some("blocked") => ActionStatus::Blocked,
                _ => return Err(GovernorError::GovernorUnavailable),
            };
            let reasons = entry["reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .map(ToString::to_string)
                .collect();
            actions.push(FrontierAction {
                action: id.as_str().to_owned(),
                status,
                capability: declared
                    .requires
                    .first()
                    .map(|capability| capability.as_str().to_owned()),
                reasons,
            });
        }
        let rendered = b10x_canon::eval::render(&decision);
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
        let (_, _, decision) = self.decide(case)?;
        let legitimate: Vec<&String> = decision
            .outcomes
            .as_ref()
            .and_then(|section| section.as_object())
            .into_iter()
            .flatten()
            .filter(|(_, entry)| entry["status"] == "legitimate")
            .map(|(outcome, _)| outcome)
            .collect();
        Ok(match legitimate.as_slice() {
            [outcome] => CompletionDetermination::Complete(CompletionDeterminationComplete {
                outcome: (*outcome).clone(),
            }),
            _ => CompletionDetermination::Open(Unit(true)),
        })
    }
}

impl<S: CaseStore> EvidencePort for CanonGovernor<S> {
    fn receive(&self, evidence: AttributedEvidence) -> Result<(), GovernorError> {
        let data = evidence.into_evidence().into_data();
        let state = self.held(&data.case_id)?;
        let ir = protocol_ir(&state.protocol).map_err(|_| GovernorError::GovernorUnavailable)?;
        let case = data.case_id.clone();
        self.store
            .update(&case, |state| {
                let applies = applies(&ir, state, &data);
                state.evidence.push(HeldEvidence { data, applies });
            })
            .ok_or(GovernorError::UnknownCase)
    }
}

impl<S: CaseStore> ObservationPort for CanonGovernor<S> {
    fn observe(
        &self,
        observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        self.store.observe(observation.into_data());
        Ok(())
    }
}

/// The compiled protocol, after checking `revisions` against what it declares.
fn checked(protocol: &str, revisions: &BTreeMap<String, String>) -> Result<Ir, OpenError> {
    let ir = protocol_ir(protocol)?;
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

/// A case just opened: revision 1, no evidence.
fn opened(case: &CaseId, protocol: &str, revisions: &BTreeMap<String, String>) -> CaseState {
    CaseState {
        id: case.clone(),
        protocol: protocol.to_owned(),
        revision: 1,
        artifacts: revisions.clone(),
        evidence: Vec::new(),
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
    }
}

/// The `canon-evidence/1` record `data`'s facts carry, when they carry one of its kind.
fn canon_record(data: &EvidenceData) -> Option<EvidenceRecord> {
    let record = evidence_from_value(&canon_value(&data.facts)?).ok()?;
    (record.kind.as_str() == data.kind).then_some(record)
}

/// `value` as Canon's value type, built from it directly with no text in between. A number is read
/// from its spelling as Canon's YAML reader reads one; `None` when a number's spelling does not
/// read as one, or an object names a member twice (which Canon's reader refuses).
fn canon_value(value: &json::Value) -> Option<serde_yaml_ng::Value> {
    use serde_yaml_ng::{Mapping, Number, Value as Yaml};
    Some(match value {
        json::Value::Null => Yaml::Null,
        json::Value::Bool(boolean) => Yaml::Bool(*boolean),
        json::Value::Number(spelling) => Yaml::Number(spelling.parse::<Number>().ok()?),
        json::Value::Text(text) => Yaml::String(text.clone()),
        json::Value::Array(items) => {
            Yaml::Sequence(items.iter().map(canon_value).collect::<Option<_>>()?)
        }
        json::Value::Object(members) => {
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

/// Whether `data` applies to the case: its facts carry a record of its kind, and Canon evaluates
/// the case with it and the records that already apply.
fn applies(ir: &Ir, state: &CaseState, data: &EvidenceData) -> bool {
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
    evaluate_with(ir, &snapshot(ir, state), &records, Supplied::default()).is_ok()
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
