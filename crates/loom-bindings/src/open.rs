//! The open effect port (story:child-case-runtime): performs an admitted action bound to an `open`
//! executor by opening its child cases through the governor, and the load check that holds `open`
//! to the protocol's `case_open`.
//!
//! [`check_case_open`] refuses an `open` executor on an action that declares no `case_open`, an
//! action that declares `case_open` bound to any other executor, and an evidence entry of the kind
//! `case_outcome`, which Loom alone writes. [`OpenEffects::new`] loads, for every `open` binding,
//! the child's bindings file, read relative to the directory of the parent's bindings file and
//! never the process's working directory or the workspace, and checks it against the protocol
//! `case_open` names, resolved by its id in the host's catalog; any refusal is the constructor's,
//! before anything is written. A step opens one child per name (`name`, or each entry of the
//! argument `each` names) under that protocol, with the independence of the child's file, and
//! records the binding's `from: effect` entries in the parent, all through
//! `CanonGovernor::open_children`, which writes the parent once.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_canon::model::Protocol;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_intake_slice::clock::{SystemClock, instant};
use loom::bindings::{
    BindingsDocument, BindingsRefusal, BindingsRefusalReason as Reason, BoundAction, Executor,
};
use loom_governor::{
    CanonGovernor, CaseIndependence, ChildError, ChildOpening, EvaluationTime, FallibleCaseStore,
    NoEvaluationTime,
};
use loom_protocols::ProtocolCatalog;
use sha2::{Digest as _, Sha256};

use crate::{ANY_KEY, MAX_DOCUMENT_BYTES, Refusal, check, decode, refuse};

/// The kind of the record Loom writes into a parent for each child; no binding records it.
const CASE_OUTCOME: &str = "case_outcome";
/// The `source` of the observation the port keeps for every step that opens children.
pub const OPEN_SOURCE: &str = "loom/open";
/// The one value of a `from: effect` entry.
const PERFORMED: &str = "performed";

/// Holds `document`'s bindings to `protocol`'s `case_open`: refuses an `open` executor on an
/// action that declares no `case_open`, an action that declares `case_open` bound to another
/// executor (both `MemberNotForExecutor` at `executor.kind`), and an evidence entry of the kind
/// `case_outcome` (`KindNotProducible`), in document order. Run it beside [`check`] when a
/// document is loaded; an action `protocol` does not declare is [`check`]'s to refuse.
///
/// # Errors
/// The first defect found, naming the action and the member.
pub fn check_case_open(document: &BindingsDocument, protocol: &Protocol) -> Result<(), Refusal> {
    for bound in &document.actions {
        let id = bound.action.as_str();
        let Some((_, declared)) = protocol
            .actions
            .iter()
            .find(|(declared, _)| declared.to_string() == id)
        else {
            continue;
        };
        let opens = declared.case_open.is_some();
        let kind = bound.executor.kind;
        if kind == Executor::Open && !opens {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                Some(id),
                "executor.kind",
                format!(
                    "an Open executor performs only an action that declares case_open; {id} declares none"
                ),
            ));
        }
        if opens && kind != Executor::Open {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                Some(id),
                "executor.kind",
                format!(
                    "{id} declares case_open, which only an Open executor performs, not {kind:?}"
                ),
            ));
        }
        if let Some(index) = bound
            .evidence
            .iter()
            .position(|entry| entry.kind == CASE_OUTCOME)
        {
            return Err(refuse(
                Reason::KindNotProducible,
                Some(id),
                format!("evidence[{index}].kind"),
                "case_outcome is written by Loom when it opens and terminates a child case",
            ));
        }
    }
    Ok(())
}

/// What a step of one `open` binding opens: the registration of the child's protocol and the
/// independence its bindings file declares.
#[derive(Debug, Clone)]
struct ChildBinding {
    registration: String,
    independence: CaseIndependence,
}

/// The open effect port of one case (see the module documentation).
pub struct OpenEffects<'a, S, T = NoEvaluationTime> {
    governor: &'a CanonGovernor<S, T>,
    case: CaseId,
    document: &'a BindingsDocument,
    children: BTreeMap<String, ChildBinding>,
}

impl<S, T> std::fmt::Debug for OpenEffects<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenEffects")
            .field("case", &self.case)
            .field("protocol", &self.document.protocol)
            .field("children", &self.children)
            .finish_non_exhaustive()
    }
}

/// `inner`, a refusal of the child's bindings file of `action`, as a refusal of the parent's
/// `executor.bindings` member.
fn of_child_file(action: &str, path: &str, inner: &Refusal) -> Refusal {
    Refusal(BindingsRefusal {
        reason: inner.0.reason,
        action: Some(action.to_owned()),
        member: "executor.bindings".to_owned(),
        detail: format!("the child bindings file {path:?}: {inner}"),
    })
}

impl<'a, S: FallibleCaseStore, T: EvaluationTime> OpenEffects<'a, S, T> {
    /// The port of `case`, held by `governor`, for `document`, read from `bindings_file` (an
    /// absolute path), with the host's `catalog`, which the governor admitted. Nothing is written
    /// here.
    ///
    /// # Errors
    /// A [`Refusal`]: `document` refused by [`check`] or [`check_case_open`] against its protocol
    /// in `catalog` (`ProtocolMismatch` when the catalog does not hold it), a relative
    /// `bindings_file` (`PathNotInWorkspace`), and per `open` binding a `case_open` protocol the
    /// catalog holds no registration of, or more than one (`ProtocolMismatch`), a child bindings
    /// file that cannot be read (`Malformed`) or that [`decode`], [`check`] or [`check_case_open`]
    /// refuses against that protocol, its reason kept, at the parent's `executor.bindings`.
    pub fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        document: &'a BindingsDocument,
        bindings_file: &Path,
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
        check_case_open(document, &entry.model)?;
        let directory = bindings_file
            .parent()
            .filter(|directory| directory.is_absolute())
            .ok_or_else(|| {
                refuse(
                    Reason::PathNotInWorkspace,
                    None,
                    "protocol",
                    "the bindings file is not given as an absolute path, so its directory is unknown",
                )
            })?;
        let mut children = BTreeMap::new();
        for bound in &document.actions {
            if bound.executor.kind != Executor::Open {
                continue;
            }
            let action = bound.action.as_str();
            let Some(opening) = entry
                .model
                .actions
                .iter()
                .find(|(declared, _)| declared.to_string() == action)
                .and_then(|(_, declared)| declared.case_open.as_ref())
            else {
                continue;
            };
            let id = opening.protocol.as_str();
            let candidates: Vec<_> = catalog
                .iter()
                .filter(|candidate| candidate.model.protocol.id.as_str() == id)
                .collect();
            let [child] = candidates.as_slice() else {
                return Err(refuse(
                    Reason::ProtocolMismatch,
                    Some(action),
                    "executor.bindings",
                    format!(
                        "the host catalog holds {} registrations of protocol {id}, which case_open of {action} names; it must hold one",
                        candidates.len()
                    ),
                ));
            };
            let Some(relative) = bound.executor.bindings.as_deref() else {
                return Err(refuse(
                    Reason::MissingMember,
                    Some(action),
                    "executor.bindings",
                    "an Open executor names the child's bindings file",
                ));
            };
            let path = directory.join(relative);
            let text = read_bounded(&path).map_err(|why| {
                refuse(
                    Reason::Malformed,
                    Some(action),
                    "executor.bindings",
                    format!("the child bindings file {relative:?} cannot be read: {why}"),
                )
            })?;
            let child_document =
                decode(&text).map_err(|inner| of_child_file(action, relative, &inner))?;
            check(&child_document, child.name(), &child.model)
                .and_then(|()| check_case_open(&child_document, &child.model))
                .map_err(|inner| of_child_file(action, relative, &inner))?;
            let independence = child_document
                .independence
                .iter()
                .try_fold(CaseIndependence::none(), |independence, declared| {
                    independence.with(
                        declared.producer.as_str(),
                        declared.dimensions.iter().cloned(),
                    )
                })
                .map_err(|error| {
                    refuse(
                        Reason::Malformed,
                        Some(action),
                        "executor.bindings",
                        format!("the child bindings file {relative:?}: {error}"),
                    )
                })?;
            children.insert(
                action.to_owned(),
                ChildBinding {
                    registration: child.name().to_owned(),
                    independence,
                },
            );
        }
        Ok(Self {
            governor,
            case,
            document,
            children,
        })
    }

    /// The binding of `action` when it is bound to `open`.
    fn binding(&self, action: &str) -> Option<&BoundAction> {
        self.document
            .actions
            .iter()
            .find(|bound| bound.action == action && bound.executor.kind == Executor::Open)
    }

    fn perform(&self, request: &ActionRequestData) -> Result<EffectOutcome, EffectError> {
        let action = request.action.as_str();
        let (Some(binding), Some(child)) = (self.binding(action), self.children.get(action)) else {
            return Ok(refused(format!("no open executor is bound to {action}")));
        };
        if request.case_id != self.case {
            return Ok(refused(format!(
                "the request is for case {}, and this port opens children of case {}",
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
        let names = match self.names(binding, request) {
            Ok(names) => names,
            Err(why) => return Ok(refused(format!("{action}: {why}"))),
        };
        let at = instant(&SystemClock).map_err(EffectError::new)?;
        let revisions = self.governor.revisions(&self.case).map_err(governor)?;
        // The port's own observation of the step, which every record it writes cites.
        let observation = ObservationId(fresh_uuid(&self.case.0));
        self.governor
            .observe(Observation::new(ObservationData {
                observation_id: observation.clone(),
                source: OPEN_SOURCE.to_owned(),
                subject: action.to_owned(),
                observed_at: at.clone(),
                payload: commission_value(&serde_json::json!({
                    "case": self.case.0,
                    "action": action,
                    "names": names,
                }))?,
            }))
            .map_err(governor)?;
        let mut records = Vec::with_capacity(binding.evidence.len());
        let mut recorded = Vec::with_capacity(binding.evidence.len());
        for entry in &binding.evidence {
            let row = |key: &str| entry.results.iter().find(|rule| rule.value == key);
            let Some(rule) = row(PERFORMED).or_else(|| row(ANY_KEY)) else {
                return Ok(refused(format!(
                    "ResultNotInTable: action {action}: the {} entry lists no {PERFORMED}",
                    entry.kind
                )));
            };
            let revision_of = |artifact: &str| {
                revisions.get(artifact).cloned().ok_or_else(|| {
                    EffectError::new(format!(
                        "case {} holds no revision for {artifact}",
                        self.case.0
                    ))
                })
            };
            let id = fresh_uuid(&self.case.0);
            let mut facts = serde_json::json!({
                "format": "canon-evidence/1",
                "id": format!("open-{}", id.0),
                "kind": entry.kind,
                "subject": entry.subject,
                "subject_revision": revision_of(&entry.subject)?,
                "observed_at": at.0,
            });
            if let Some(result) = &rule.result {
                facts["result"] = serde_json::Value::String(result.clone());
            }
            if !entry.upstream.is_empty() {
                let mut upstream = serde_json::Map::new();
                for artifact in &entry.upstream {
                    upstream.insert(
                        artifact.clone(),
                        serde_json::Value::String(revision_of(artifact)?),
                    );
                }
                facts["upstream_revisions"] = upstream.into();
            }
            recorded.push(facts.clone());
            records.push(EvidenceData {
                evidence_id: EvidenceId(id),
                case_id: self.case.clone(),
                kind: entry.kind.clone(),
                subject_revision: request.expected_case_revision,
                producer: entry.producer.clone(),
                observation_ids: vec![observation.clone()],
                facts: commission_value(&facts)?,
                provenance: commission_value(&serde_json::json!({"source": OPEN_SOURCE}))?,
            });
        }
        let openings: Vec<ChildOpening> = names
            .into_iter()
            .map(|name| ChildOpening {
                name,
                protocol: child.registration.clone(),
                independence: child.independence.clone(),
            })
            .collect();
        match self.governor.open_children(
            &self.case,
            action,
            request.expected_case_revision,
            &openings,
            records,
            &at.0,
        ) {
            Ok(opened) => Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                report: commission_value(&serde_json::json!({
                    "children": opened.iter().map(|case| case.0.clone()).collect::<Vec<_>>(),
                    "evidence": recorded,
                }))?,
                attempt: None,
                audit: None,
                usage: None,
            })),
            Err(ChildError::StoreUnavailable { problem }) => {
                Err(EffectError::new(format!("the governor: {problem}")))
            }
            Err(refusal) => Ok(refused(format!("{action}: {refusal}"))),
        }
    }

    /// The names of the children a step opens: the executor's `name`, or each entry of the
    /// argument `each` names in the request's arguments (the binding's literal `arguments` when the
    /// request's are `{}`), each a string.
    fn names(
        &self,
        binding: &BoundAction,
        request: &ActionRequestData,
    ) -> Result<Vec<String>, String> {
        if let Some(name) = &binding.executor.name {
            return Ok(vec![name.clone()]);
        }
        let Some(each) = binding.executor.each.as_deref() else {
            return Err("the Open executor names neither name nor each".to_owned());
        };
        let requested = &request.arguments.0;
        let none =
            matches!(requested, commission_json::Value::Object(members) if members.is_empty());
        let entries = if none {
            binding
                .arguments
                .as_ref()
                .and_then(|literal| loom_text_items(literal, each))
        } else {
            commission_text_items(requested, each)
        };
        entries.ok_or_else(|| format!("the argument {each} is not a list of strings"))
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> EffectPort for OpenEffects<'_, S, T> {
    fn performs(&self, action: &str) -> bool {
        self.binding(action).is_some() && self.children.contains_key(action)
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.perform(request.data())
    }
}

/// The strings of the array member `name` of the object `value`.
fn commission_text_items(value: &commission_json::Value, name: &str) -> Option<Vec<String>> {
    match value.member(name)? {
        commission_json::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                commission_json::Value::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// The strings of the array member `name` of the object `value`.
fn loom_text_items(value: &loom::json::Value, name: &str) -> Option<Vec<String>> {
    let loom::json::Value::Object(members) = value else {
        return None;
    };
    match members
        .iter()
        .find(|(member, _)| member == name)
        .map(|(_, item)| item)?
    {
        loom::json::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                loom::json::Value::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// The text of `path`, at most [`MAX_DOCUMENT_BYTES`] of it.
fn read_bounded(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut text = String::new();
    let bound = u64::try_from(MAX_DOCUMENT_BYTES).unwrap_or(u64::MAX);
    file.take(bound.saturating_add(1))
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err(format!("it is larger than {MAX_DOCUMENT_BYTES} bytes"));
    }
    Ok(text)
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
/// of `case`.
fn fresh_uuid(case: &str) -> Uuid {
    static ISSUED: AtomicU64 = AtomicU64::new(0);
    let issued = ISSUED.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let digest = Sha256::digest(
        format!(
            "open-evidence\n{case}\n{}\n{nanos}\n{issued}",
            std::process::id()
        )
        .as_bytes(),
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
