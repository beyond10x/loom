//! The copy effect port (story:record-copy-between-cases): performs an admitted action bound to a
//! `copy` executor by copying one evidence record from another case of the same tree into the case
//! that performs it. Loom makes the copy; no program runs.
//!
//! [`CopyEffects::new`] checks the document against its protocol in the host's catalog and
//! resolves each `copy` binding's `from` to the registrations it names: the registration of that
//! name, else every registration whose protocol id it is; one that names neither is refused before
//! anything is written. The source and the kind come from the bindings document alone: no request
//! argument, report or program output selects them.
//!
//! A step, in order: the case held under the document's protocol; the cases of its tree
//! (`CanonGovernor::tree`) other than itself held under a registration `from` resolves to, else
//! `NoSourceCase`; among the records those cases hold (`CanonGovernor::case_state`), each that
//! applies, is of the binding's kind and is about its subject's current revision in its case, the
//! latest by its `observed_at` (a record with none is earliest; of two at one instant, the later in
//! tree order, then in the order received), else `NoSourceRecord`; that record's subject declared
//! by the receiving case, else `SubjectUndeclared`. A refused step writes nothing. A receiving case
//! that already holds the copy of that record (a record citing an observation of [`COPY_SOURCE`]
//! that names it) is answered performed and nothing is written. Otherwise the port keeps one
//! observation of [`COPY_SOURCE`] (`loom.bindings.CopiedRecord`) at the record's instant, or takes
//! the one a restart left before the record was written, and submits the copy citing it: the
//! record's kind, subject, subject revision, result and instant, attributed to its producer, with
//! the `upstream_revisions` entries of the artifacts the receiving case declares. The receiving
//! case judges the copy by its own independence map. [`CopyEffects::copy_due`] answers whether a
//! step would write a copy.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_canon::model::Instant as CanonInstant;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::{ObservationPort, submit_evidence};
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_intake_slice::clock::{SystemClock, instant};
use loom::bindings::{
    BindingsDocument, BindingsRefusalReason as Reason, BoundAction, CopiedRecord, CopyRefusal,
    CopyRefusalReason, Executor,
};
use loom_governor::{
    CanonGovernor, CaseState, EvaluationTime, FallibleCaseStore, NoEvaluationTime,
};
use loom_protocols::ProtocolCatalog;
use sha2::{Digest as _, Sha256};

use crate::{Refusal, check, refuse};

/// The `source` of the observation every copy cites; only this port writes it.
pub const COPY_SOURCE: &str = "loom/copy";
/// The kind of the record Loom writes into a parent for each child; never copied.
const CASE_OUTCOME: &str = "case_outcome";

/// The copy effect port of one case (see the module documentation).
pub struct CopyEffects<'a, S, T = NoEvaluationTime> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    document: &'a BindingsDocument,
    /// Per `copy` action, the registrations its `from` names.
    sources: BTreeMap<String, BTreeSet<String>>,
}

impl<S, T> std::fmt::Debug for CopyEffects<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CopyEffects")
            .field("case", &self.case)
            .field("protocol", &self.document.protocol)
            .field("sources", &self.sources)
            .finish_non_exhaustive()
    }
}

/// The source record a step copies.
struct Found {
    case: CaseId,
    record: EvidenceData,
    /// The text of its `observed_at`, when it has one.
    instant: Option<String>,
}

/// What a step does.
enum Plan {
    /// Refused; nothing is written.
    Refused(CopyRefusal),
    /// The receiving case holds the copy already; nothing is written.
    Held { found: Found },
    /// The copy is due: the source, and of its `upstream_revisions` the entries kept and the
    /// artifacts dropped.
    Due {
        found: Found,
        kept: Vec<(String, commission_json::Value)>,
        dropped: Vec<String>,
    },
}

impl<'a, S: FallibleCaseStore, T: EvaluationTime> CopyEffects<'a, S, T> {
    /// The port of `case`, held by `governor`, for `document`, with the host's `catalog`, which
    /// the governor admitted. Nothing is written here.
    ///
    /// # Errors
    /// A [`Refusal`]: `document` refused by [`check`] against its protocol in `catalog`
    /// (`ProtocolMismatch` when the catalog does not hold it); per `copy` binding a `from` that is
    /// neither a registration of `catalog` nor the protocol id of one (`ProtocolMismatch`, at
    /// `executor.from`), and an `evidence` kind `case_outcome` (`KindNotProducible`).
    pub fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        document: &'a BindingsDocument,
        catalog: &ProtocolCatalog,
    ) -> Result<Self, Refusal> {
        let registration = document.protocol.as_str();
        let entry = catalog.get(registration).ok_or_else(|| {
            refuse(
                Reason::ProtocolMismatch,
                None,
                "protocol",
                format!("the host catalog holds no protocol {registration}"),
            )
        })?;
        check(document, registration, &entry.model)?;
        let mut sources = BTreeMap::new();
        for bound in &document.actions {
            if bound.executor.kind != Executor::Copy {
                continue;
            }
            let action = bound.action.as_str();
            let (Some(kind), Some(from)) = (
                bound.executor.evidence.as_deref(),
                bound.executor.from.as_deref(),
            ) else {
                return Err(refuse(
                    Reason::MissingMember,
                    Some(action),
                    "executor",
                    "a copy executor names the evidence kind and the protocol it copies from",
                ));
            };
            if kind == CASE_OUTCOME {
                return Err(refuse(
                    Reason::KindNotProducible,
                    Some(action),
                    "executor.evidence",
                    "case_outcome is written by Loom when it opens and terminates a child case",
                ));
            }
            let registrations: BTreeSet<String> = if catalog.get(from).is_some() {
                BTreeSet::from([from.to_owned()])
            } else {
                catalog
                    .iter()
                    .filter(|candidate| candidate.model.protocol.id.as_str() == from)
                    .map(|candidate| candidate.name().to_owned())
                    .collect()
            };
            if registrations.is_empty() {
                return Err(refuse(
                    Reason::ProtocolMismatch,
                    Some(action),
                    "executor.from",
                    format!(
                        "the host catalog holds no registration {from} and no protocol whose id is {from}"
                    ),
                ));
            }
            sources.insert(action.to_owned(), registrations);
        }
        Ok(Self {
            governor,
            case,
            document,
            sources,
        })
    }

    /// Whether a step of `action` would write a copy: the action is bound to `copy`, the case is
    /// held under the document's protocol, a case of its tree holds a record the step copies, and
    /// the receiving case does not hold its copy yet. A step it would refuse is `false`.
    ///
    /// # Errors
    /// The governor's, when a case cannot be read.
    pub fn copy_due(&self, action: &str) -> Result<bool, EffectError> {
        let Some(binding) = self.binding(action) else {
            return Ok(false);
        };
        if self.governor.protocol(&self.case).map_err(governor)? != self.document.protocol {
            return Ok(false);
        }
        Ok(matches!(self.plan(binding)?, Plan::Due { .. }))
    }

    /// The binding of `action` when it is bound to `copy`.
    fn binding(&self, action: &str) -> Option<&BoundAction> {
        self.document
            .actions
            .iter()
            .find(|bound| bound.action == action && bound.executor.kind == Executor::Copy)
            .filter(|bound| self.sources.contains_key(&bound.action))
    }

    /// What a step of `binding` does, read from the governor; nothing is written.
    fn plan(&self, binding: &BoundAction) -> Result<Plan, EffectError> {
        let action = binding.action.as_str();
        let kind = binding.executor.evidence.as_deref().unwrap_or_default();
        let from = binding.executor.from.as_deref().unwrap_or_default();
        let registrations = &self.sources[action];
        let refused = |reason, detail: String| {
            Ok(Plan::Refused(CopyRefusal {
                reason,
                action: action.to_owned(),
                detail,
            }))
        };
        let tree = self.governor.tree(&self.case).map_err(governor)?;
        let sources: Vec<&CaseId> = tree
            .iter()
            .filter(|member| member.case != self.case && registrations.contains(&member.protocol))
            .map(|member| &member.case)
            .collect();
        if sources.is_empty() {
            return refused(
                CopyRefusalReason::NoSourceCase,
                format!(
                    "no case of the tree of case {} but itself is held under {from}",
                    self.case.0
                ),
            );
        }
        let mut latest: Option<(Option<i64>, Found)> = None;
        for source in sources {
            let state = self.governor.case_state(source).map_err(governor)?;
            for record in current_records(&state, kind) {
                let instant = text(&record.facts, "observed_at").map(str::to_owned);
                let seconds = instant
                    .as_deref()
                    .and_then(|at| CanonInstant::new(at).seconds());
                if latest.as_ref().is_none_or(|(best, _)| seconds >= *best) {
                    latest = Some((
                        seconds,
                        Found {
                            case: source.clone(),
                            record: record.clone(),
                            instant,
                        },
                    ));
                }
            }
        }
        let Some((_, found)) = latest else {
            return refused(
                CopyRefusalReason::NoSourceRecord,
                format!(
                    "no case of the tree of case {} held under {from} holds an applying {kind} record about the current revision of its subject",
                    self.case.0
                ),
            );
        };
        let receiving = self.governor.case_state(&self.case).map_err(governor)?;
        let subject = text(&found.record.facts, "subject").unwrap_or_default();
        if !receiving.artifacts.contains_key(subject) {
            return refused(
                CopyRefusalReason::SubjectUndeclared,
                format!(
                    "the {kind} record {} of case {} is about {subject}, which {} does not declare",
                    found.record.evidence_id.0.0, found.case.0, receiving.protocol
                ),
            );
        }
        let observations = self.governor.try_observations().map_err(governor)?;
        let copies: Vec<&ObservationId> = observations
            .iter()
            .filter(|observation| names_copy_of(observation, &self.case, &found))
            .map(|observation| &observation.observation_id)
            .collect();
        if receiving.evidence.iter().any(|held| {
            held.data
                .observation_ids
                .iter()
                .any(|cited| copies.contains(&cited))
        }) {
            return Ok(Plan::Held { found });
        }
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        if let Some(commission_json::Value::Object(members)) =
            found.record.facts.member("upstream_revisions")
        {
            for (artifact, revision) in members {
                if receiving.artifacts.contains_key(artifact) {
                    kept.push((artifact.clone(), revision.clone()));
                } else {
                    dropped.push(artifact.clone());
                }
            }
        }
        Ok(Plan::Due {
            found,
            kept,
            dropped,
        })
    }

    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, EffectError> {
        let action = request.action.as_str();
        let Some(binding) = self.binding(action) else {
            return Ok(refused(format!("no copy executor is bound to {action}")));
        };
        if request.case_id != self.case {
            return Ok(refused(format!(
                "the request is for case {}, and this port copies into case {}",
                request.case_id.0, self.case.0
            )));
        }
        let held = self.governor.protocol(&self.case).map_err(governor)?;
        if held != self.document.protocol {
            return Ok(refused(format!(
                "case {} is held under {held}, and the bindings document binds {}",
                self.case.0, self.document.protocol
            )));
        }
        let (found, kept, dropped) = match self.plan(binding)? {
            Plan::Refused(refusal) => {
                return Ok(refused(format!(
                    "{:?}: action {}: {}",
                    refusal.reason, refusal.action, refusal.detail
                )));
            }
            Plan::Held { found } => {
                return performed(&serde_json::json!({
                    "copied": null,
                    "held": {
                        "source_case": found.case.0,
                        "source_evidence": found.record.evidence_id.0.0,
                    },
                }));
            }
            Plan::Due {
                found,
                kept,
                dropped,
            } => (found, kept, dropped),
        };
        let copied = CopiedRecord {
            case: self.case.0.clone(),
            action: action.to_owned(),
            source_case: found.case.0.clone(),
            source_evidence: found.record.evidence_id.0.0.clone(),
            dropped_upstream: dropped,
        };
        let payload = copied_json(&copied);
        // The observation a restart left between it and the record, else a new one.
        let observations = self.governor.try_observations().map_err(governor)?;
        let left = observations.iter().find(|observation| {
            names_copy_of(observation, &self.case, &found)
                && found
                    .instant
                    .as_deref()
                    .is_none_or(|at| observation.observed_at.0 == at)
        });
        let observation = if let Some(left) = left {
            left.observation_id.clone()
        } else {
            let observed_at = match &found.instant {
                Some(at) => Timestamp(at.clone()),
                None => instant(&SystemClock).map_err(EffectError::new)?,
            };
            let id = ObservationId(fresh_uuid("copy-observation", &self.case.0));
            self.governor
                .observe(Observation::new(ObservationData {
                    observation_id: id.clone(),
                    source: COPY_SOURCE.to_owned(),
                    subject: action.to_owned(),
                    observed_at,
                    payload: commission_value(&payload)?,
                }))
                .map_err(governor)?;
            id
        };
        let id = EvidenceId(fresh_uuid("copy-evidence", &self.case.0));
        let facts = copy_facts(&found.record.facts, &id, kept);
        let case_revision = self
            .governor
            .current_revision(&self.case)
            .map_err(governor)?;
        submit_evidence(
            self.governor,
            &found.record.producer,
            EvidenceData {
                evidence_id: id,
                case_id: self.case.clone(),
                kind: found.record.kind.clone(),
                subject_revision: case_revision,
                producer: found.record.producer.clone(),
                observation_ids: vec![observation],
                facts: facts.clone(),
                provenance: commission_value(&serde_json::json!({
                    "source": COPY_SOURCE,
                    "source_case": copied.source_case,
                    "source_evidence": copied.source_evidence,
                }))?,
            },
        )
        .map_err(|error| {
            EffectError::new(format!("the copied {} record: {error}", copied.action))
        })?;
        performed(&serde_json::json!({
            "copied": payload,
            "facts": commission_text(&facts),
        }))
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> EffectPort for CopyEffects<'_, S, T> {
    fn performs(&self, action: &str) -> bool {
        self.binding(action).is_some()
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.perform(request.data())
    }
}

/// The records of `state` a copy of `kind` may take, in the order received: each that applies,
/// whose kind and facts' kind are `kind`, whose facts' subject the case holds at the facts'
/// `subject_revision`, and whose `observed_at`, when present, is text.
fn current_records<'s>(
    state: &'s CaseState,
    kind: &'s str,
) -> impl Iterator<Item = &'s EvidenceData> {
    state
        .evidence
        .iter()
        .filter(|held| held.applies)
        .map(|held| &held.data)
        .filter(move |record| record.kind == kind && text(&record.facts, "kind") == Some(kind))
        .filter(|record| {
            matches!(
                (text(&record.facts, "subject"), text(&record.facts, "subject_revision")),
                (Some(subject), Some(revision))
                    if state.artifacts.get(subject).map(String::as_str) == Some(revision)
            )
        })
        .filter(|record| {
            record
                .facts
                .member("observed_at")
                .is_none_or(|at| matches!(at, commission_json::Value::Text(_)))
        })
}

/// Whether `observation` is one of [`COPY_SOURCE`] naming the copy into `case` of `found`.
fn names_copy_of(observation: &ObservationData, case: &CaseId, found: &Found) -> bool {
    observation.source == COPY_SOURCE
        && text(&observation.payload, "case") == Some(case.0.as_str())
        && text(&observation.payload, "source_case") == Some(found.case.0.as_str())
        && text(&observation.payload, "source_evidence")
            == Some(found.record.evidence_id.0.0.as_str())
}

/// The facts of the copy of a record whose facts are `source`: its kind, subject, subject
/// revision, result and instant as they are, the id `copy-<id>`, and `kept` as its
/// `upstream_revisions` when any is kept.
fn copy_facts(
    source: &commission_json::Value,
    id: &EvidenceId,
    kept: Vec<(String, commission_json::Value)>,
) -> commission_json::Value {
    let mut members = vec![
        (
            "format".to_owned(),
            commission_json::Value::Text("canon-evidence/1".to_owned()),
        ),
        (
            "id".to_owned(),
            commission_json::Value::Text(format!("copy-{}", id.0.0)),
        ),
    ];
    for name in [
        "kind",
        "subject",
        "subject_revision",
        "result",
        "observed_at",
    ] {
        if let Some(value) = source.member(name) {
            members.push((name.to_owned(), value.clone()));
        }
    }
    if !kept.is_empty() {
        members.push((
            "upstream_revisions".to_owned(),
            commission_json::Value::Object(kept),
        ));
    }
    commission_json::Value::Object(members)
}

fn copied_json(copied: &CopiedRecord) -> serde_json::Value {
    serde_json::json!({
        "case": copied.case,
        "action": copied.action,
        "source_case": copied.source_case,
        "source_evidence": copied.source_evidence,
        "dropped_upstream": copied.dropped_upstream,
    })
}

/// The text member `name` of the object `value`.
fn text<'v>(value: &'v commission_json::Value, name: &str) -> Option<&'v str> {
    match value.member(name) {
        Some(commission_json::Value::Text(text)) => Some(text),
        _ => None,
    }
}

/// `value` as JSON text.
fn commission_text(value: &commission_json::Value) -> serde_json::Value {
    fn plain(value: &commission_json::Value) -> serde_json::Value {
        match value {
            commission_json::Value::Null => serde_json::Value::Null,
            commission_json::Value::Bool(value) => serde_json::Value::Bool(*value),
            commission_json::Value::Number(spelling) => {
                serde_json::from_str(spelling).unwrap_or(serde_json::Value::Null)
            }
            commission_json::Value::Text(text) => serde_json::Value::String(text.clone()),
            commission_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(plain).collect())
            }
            commission_json::Value::Object(members) => serde_json::Value::Object(
                members
                    .iter()
                    .map(|(name, member)| (name.clone(), plain(member)))
                    .collect(),
            ),
        }
    }
    plain(value)
}

fn performed(report: &serde_json::Value) -> Result<EffectOutcome, EffectError> {
    Ok(EffectOutcome::Performed(EffectOutcomePerformed {
        report: commission_value(report)?,
        attempt: None,
        audit: None,
        usage: None,
    }))
}

fn refused(reason: String) -> EffectOutcome {
    EffectOutcome::Refused(EffectOutcomeRefused {
        reason,
        ends_run: false,
    })
}

fn governor(error: impl std::fmt::Debug) -> EffectError {
    EffectError::new(format!("the governor: {error:?}"))
}

fn commission_value(value: &serde_json::Value) -> Result<commission_json::Value, EffectError> {
    commission_json::parse(&value.to_string())
        .map_err(|error| EffectError::new(format!("a JSON value the port built: {error:?}")))
}

/// A fresh id (version 8, from a digest of the process, the instant and a counter) for a record
/// of `kind` in `case`.
fn fresh_uuid(kind: &str, case: &str) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(0);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let digest = Sha256::digest(
        format!("{kind}\n{case}\n{}\n{nanos}\n{issued}", std::process::id()).as_bytes(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    });
    Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}
