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
//! member `each` names) under that protocol, with the independence of the child's file, and
//! records the binding's `from: effect` entries in the parent, all through
//! `CanonGovernor::open_children`, which writes the parent once.
//!
//! The object `each` reads (story:open-children-from-arguments-from) is the request's arguments
//! when they are not `{}`, else the binding's literal `arguments`, else the one JSON object its
//! `arguments_from` program prints, run as a `program` binding's is ([`run_arguments_from`]); an
//! action that declares an `arguments` schema holds the object to it with Canon's
//! `check_arguments`. The names are checked before anything is written; then the program's output
//! is kept as a blob and the names as one observation of [`OPEN_SOURCE`] (`loom.bindings.ChildNames`),
//! which a step cut before its write takes again at the same case revision instead of running the
//! program twice. A refusal names the member, the entry's index, the JSON pointer and the keyword,
//! never the text the program printed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_canon::model::Protocol;
use b10x_canon::validate::{ArgumentsRefused, check_arguments};
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, CaseId, Commission, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, EvidenceData, EvidenceId, Observation, ObservationData, ObservationId,
    commission_state,
};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_intake_slice::bindings::{DEFAULT_TIMEOUT_SECONDS, admit_bound_program_runner};
use b10x_loom_intake_slice::clock::{SystemClock, instant};
use b10x_loom_intake_slice::confinement::{ConfinementError, TestRunner};
use loom::bindings::{
    BindingsDocument, BindingsRefusal, BindingsRefusalReason as Reason, BoundAction,
    ChildNameSource, Executor, OpenRefusal, OpenRefusalReason, ProgramRoot,
};
use loom_governor::{
    CanonGovernor, CaseIndependence, ChildError, ChildOpening, EvaluationTime, FallibleCaseStore,
    NoEvaluationTime, plain_child_name,
};
use loom_protocols::ProtocolCatalog;
use sha2::{Digest as _, Sha256};

use crate::arguments::{NotPrinted, case_names, run_arguments_from};
use crate::program::{commission_text, keep_blob, loom_text};
use crate::resolve::beside;
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
    /// The `arguments` schema each `open` action of the protocol declares.
    schemas: BTreeMap<String, serde_json::Value>,
    workspace: PathBuf,
    store: PathBuf,
    /// The admitted runner, when a binding of the document has `arguments_from`.
    runner: Option<Arc<dyn TestRunner>>,
}

impl<S, T> std::fmt::Debug for OpenEffects<'_, S, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenEffects")
            .field("case", &self.case)
            .field("protocol", &self.document.protocol)
            .field("children", &self.children)
            .field("workspace", &self.workspace)
            .field("store", &self.store)
            .field("runner", &self.runner.as_ref().map(|runner| runner.label()))
            .finish_non_exhaustive()
    }
}

/// Why [`OpenEffects::new`] built no port.
#[derive(Debug)]
pub enum OpenPortError {
    /// The document, or a child's bindings file, is refused.
    Refused(Refusal),
    /// The runner an `arguments_from` program needs is refused: by the admission of a host runner
    /// for bound programs (none is `NoRunner`), or by its own `preflight_child` on the workspace.
    Runner(ConfinementError),
}

impl std::fmt::Display for OpenPortError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(refusal) => write!(out, "{refusal}"),
            Self::Runner(error) => write!(out, "the runner: {error}"),
        }
    }
}

impl std::error::Error for OpenPortError {}

impl From<Refusal> for OpenPortError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// The names of one step and where they come from.
#[derive(Debug, Clone)]
struct Listed {
    names: Vec<String>,
    source: ChildNameSource,
    /// The SHA-256 of the `arguments_from` program's output.
    blob: Option<String>,
    /// That output, still to be kept; `None` once kept.
    bytes: Option<Vec<u8>>,
    /// The observation of a list a cut step kept, taken again, and its instant, which the records
    /// citing it carry.
    observation: Option<(ObservationId, Timestamp)>,
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
    /// absolute path), with the host's `catalog`, which the governor admitted. An `arguments_from`
    /// program starts in `workspace` through `runner`, and its output is kept under `store`.
    /// Nothing starts and nothing is written here.
    ///
    /// # Errors
    /// [`OpenPortError::Refused`]: `document` refused by [`check`] or [`check_case_open`] against
    /// its protocol in `catalog` (`ProtocolMismatch` when the catalog does not hold it), a relative
    /// `bindings_file` (`PathNotInWorkspace`), and per `open` binding a `case_open` protocol the
    /// catalog holds no registration of, or more than one (`ProtocolMismatch`), a child bindings
    /// file that cannot be read (`Malformed`) or that [`decode`], [`check`] or [`check_case_open`]
    /// refuses against that protocol, its reason kept, at the parent's `executor.bindings`, and
    /// `arguments_from` on an action whose protocol declares no `arguments` schema
    /// (`ArgumentsUndeclared`). [`OpenPortError::Runner`]: a binding has `arguments_from` and
    /// `runner` is refused by the admission of a host runner for bound programs or by its own
    /// `preflight_child` on `workspace`; a document with none needs no runner.
    // The parent's document and file, the catalog, and the three a program binding's port takes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        governor: &'a CanonGovernor<S, T>,
        case: CaseId,
        document: &'a BindingsDocument,
        bindings_file: &Path,
        catalog: &ProtocolCatalog,
        workspace: &Path,
        store: &Path,
        runner: Option<Arc<dyn TestRunner>>,
    ) -> Result<Self, OpenPortError> {
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
        let mut schemas = BTreeMap::new();
        for bound in &document.actions {
            if bound.executor.kind != Executor::Open {
                continue;
            }
            let action = bound.action.as_str();
            let Some(declared) = entry
                .model
                .actions
                .iter()
                .find(|(declared, _)| declared.to_string() == action)
                .map(|(_, declared)| declared)
            else {
                continue;
            };
            let Some(opening) = declared.case_open.as_ref() else {
                continue;
            };
            match &declared.arguments {
                Some(schema) => {
                    schemas.insert(action.to_owned(), schema.clone());
                }
                // Canon gives an action without a schema `{}` alone: no printed object fits it.
                None if bound.arguments_from.is_some() => {
                    return Err(refuse(
                        Reason::ArgumentsUndeclared,
                        Some(action),
                        "arguments_from",
                        format!(
                            "{action} declares no arguments schema, so it takes only {{}} and no object an arguments_from program prints"
                        ),
                    )
                    .into());
                }
                None => {}
            }
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
                )
                .into());
            };
            let Some(relative) = bound.executor.bindings.as_deref() else {
                return Err(refuse(
                    Reason::MissingMember,
                    Some(action),
                    "executor.bindings",
                    "an Open executor names the child's bindings file",
                )
                .into());
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
        let runs_programs = document
            .actions
            .iter()
            .any(|bound| bound.executor.kind == Executor::Open && bound.arguments_from.is_some());
        let runner = if runs_programs {
            let runner =
                admit_bound_program_runner(runner.as_ref()).map_err(OpenPortError::Runner)?;
            runner
                .preflight_child(workspace)
                .map_err(OpenPortError::Runner)?;
            Some(runner)
        } else {
            None
        };
        Ok(Self {
            governor,
            case,
            document,
            children,
            schemas,
            workspace: workspace.to_owned(),
            store: store.to_owned(),
            runner,
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
        // Every refusal up to here and in `names` opens no child and writes nothing.
        let listed = match self.names(binding, request)? {
            Ok(listed) => listed,
            Err(outcome) => return Ok(outcome),
        };
        // A record holds the instant of the observation it cites: a list taken again keeps its own.
        let at = match &listed.observation {
            Some((_, observed_at)) => observed_at.clone(),
            None => instant(&SystemClock).map_err(EffectError::new)?,
        };
        let revisions = self.governor.revisions(&self.case).map_err(governor)?;
        if let (Some(digest), Some(bytes)) = (&listed.blob, &listed.bytes) {
            keep_blob(&self.store, digest, bytes).map_err(|error| {
                EffectError::new(format!(
                    "the output of the arguments_from program of {action} was not kept, nothing was opened: {error}"
                ))
            })?;
        }
        // The port's own observation of the step, which every record it writes cites: the one a
        // cut step kept at this case revision, or a new one.
        let observation = match &listed.observation {
            Some((kept, _)) => kept.clone(),
            None => {
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
                            "case_revision": request.expected_case_revision,
                            "source": source_text(listed.source),
                            "names": listed.names,
                            "blob_sha256": listed.blob,
                        }))?,
                    }))
                    .map_err(governor)?;
                observation
            }
        };
        let names = listed.names.clone();
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
                    "names": listed.names,
                    "source": source_text(listed.source),
                    "blob_sha256": listed.blob,
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

    /// The names of the children a step opens and where they come from: the executor's `name`, or
    /// each entry of the member `each` names of the request's arguments when they are not `{}`, else
    /// of the binding's literal `arguments`, else of the object its `arguments_from` program
    /// printed, or of the list a cut step kept at the request's case revision. `Ok(Err)` is the
    /// step's refusal, with nothing started or written.
    fn names(
        &self,
        binding: &BoundAction,
        request: &ActionRequestData,
    ) -> Result<Result<Listed, EffectOutcome>, EffectError> {
        let action = binding.action.as_str();
        if let Some(name) = &binding.executor.name {
            return Ok(Ok(Listed {
                names: vec![name.clone()],
                source: ChildNameSource::Name,
                blob: None,
                bytes: None,
                observation: None,
            }));
        }
        let Some(each) = binding.executor.each.as_deref() else {
            return Ok(Err(refused(format!(
                "{action}: the Open executor names neither name nor each"
            ))));
        };
        let requested = &request.arguments.0;
        let none =
            matches!(requested, commission_json::Value::Object(members) if members.is_empty());
        let (object, source, printed) = if !none {
            (
                json_of(&commission_text(requested))?,
                ChildNameSource::Request,
                None,
            )
        } else if let Some(literal) = &binding.arguments {
            (
                json_of(&loom_text(literal))?,
                ChildNameSource::Literal,
                None,
            )
        } else if let Some(argv) = binding.arguments_from.as_deref() {
            if let Some(kept) = self.kept(action, request.expected_case_revision)? {
                return Ok(Ok(kept));
            }
            match self.printed(binding, argv, requested)? {
                Ok(printed) => (
                    printed.object.clone(),
                    ChildNameSource::ArgumentsFrom,
                    Some(printed),
                ),
                Err(outcome) => return Ok(Err(outcome)),
            }
        } else {
            (
                serde_json::Value::Object(serde_json::Map::new()),
                ChildNameSource::Request,
                None,
            )
        };
        if let Some(schema) = self.schemas.get(action)
            && let Err(refused) = check_arguments(Some(schema), &object)
        {
            return Ok(Err(open_refusal(
                OpenRefusalReason::ArgumentsMismatched,
                action,
                mismatch(&refused),
            )));
        }
        let names = match each_names(&object, each) {
            Ok(names) => names,
            Err((reason, detail)) => return Ok(Err(open_refusal(reason, action, detail))),
        };
        Ok(Ok(Listed {
            names,
            source,
            blob: printed.as_ref().map(|printed| printed.sha256.clone()),
            bytes: printed.map(|printed| printed.bytes),
            observation: None,
        }))
    }

    /// The object the `arguments_from` program `argv` of `binding` prints for `requested`, run once
    /// under the binding's timeout, else 60 s; `Ok(Err)` is the step's refusal.
    fn printed(
        &self,
        binding: &BoundAction,
        argv: &[String],
        requested: &commission_json::Value,
    ) -> Result<Result<crate::arguments::Printed, EffectOutcome>, EffectError> {
        let action = binding.action.as_str();
        let program = argv
            .first()
            .map_or("the arguments_from program", String::as_str);
        let Some(runner) = &self.runner else {
            return Ok(Err(refused(format!(
                "{action}: {program} was not started: the port was built with no runner"
            ))));
        };
        if self.document.program_root == Some(ProgramRoot::Bindings) && beside(program) {
            return Ok(Err(refused(format!(
                "{action}: {program} was not started: arguments_from[0] names a file beside the bindings file, and the document was not resolved with resolve_programs"
            ))));
        }
        let seconds = binding.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        let timeout = Duration::from_secs(u64::try_from(seconds).unwrap_or(1));
        if Instant::now().checked_add(timeout).is_none() {
            return Ok(Err(refused(format!(
                "{action}: {program} was not started: timeout_seconds {seconds} is past what this host's clock can hold"
            ))));
        }
        let names = match case_names(self.governor, &self.case)? {
            Ok(names) => names,
            Err(why) => {
                return Ok(Err(refused(format!(
                    "{action}: {program} was not started: {why}"
                ))));
            }
        };
        Ok(
            match run_arguments_from(
                argv,
                &commission_text(requested),
                timeout,
                &self.workspace,
                runner.as_ref(),
                &self.case,
                &names,
            ) {
                Ok(printed) => Ok(printed),
                Err(NotPrinted::NotStarted(why)) => Err(refused(format!("{action}: {why}"))),
                Err(NotPrinted::Unreadable(why)) => Err(open_refusal(
                    OpenRefusalReason::ArgumentsUnreadable,
                    action,
                    why,
                )),
            },
        )
    }

    /// The list a step of `action` kept at `case_revision` from its `arguments_from` program: the
    /// first observation of [`OPEN_SOURCE`] naming this case, the action, source `ArgumentsFrom`
    /// and that revision, whose names are a list of strings.
    fn kept(&self, action: &str, case_revision: i64) -> Result<Option<Listed>, EffectError> {
        let observations = self
            .governor
            .try_observations()
            .map_err(|error| EffectError::new(format!("the observations: {error:?}")))?;
        for observation in observations {
            if observation.source != OPEN_SOURCE {
                continue;
            }
            let Ok(payload) = json_of(&commission_text(&observation.payload)) else {
                continue;
            };
            let fits = payload["case"] == self.case.0.as_str()
                && payload["action"] == action
                && payload["source"] == source_text(ChildNameSource::ArgumentsFrom)
                && payload["case_revision"] == case_revision;
            if !fits {
                continue;
            }
            let Some(names) = payload["names"].as_array().and_then(|names| {
                names
                    .iter()
                    .map(|name| name.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            }) else {
                continue;
            };
            return Ok(Some(Listed {
                names,
                source: ChildNameSource::ArgumentsFrom,
                blob: payload["blob_sha256"].as_str().map(str::to_owned),
                bytes: None,
                observation: Some((observation.observation_id, observation.observed_at)),
            }));
        }
        Ok(None)
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

/// The names in the member `each` of `object`, checked in order: a list (`EachNotAList`) of
/// strings (`EachItemNotText`) with at least one entry (`NoChildNamed`), each one plain segment
/// (`ChildNameInvalid`) given once (`ChildNameRepeated`). A refusal names the member and the
/// entry's index, never an entry's text.
fn each_names(
    object: &serde_json::Value,
    each: &str,
) -> Result<Vec<String>, (OpenRefusalReason, String)> {
    let Some(items) = object.get(each).and_then(serde_json::Value::as_array) else {
        return Err((
            OpenRefusalReason::EachNotAList,
            format!("the member {each} is missing or not a list"),
        ));
    };
    let mut names = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(name) = item.as_str() else {
            return Err((
                OpenRefusalReason::EachItemNotText,
                format!("entry {index} of {each} is not a string"),
            ));
        };
        names.push(name);
    }
    if names.is_empty() {
        return Err((
            OpenRefusalReason::NoChildNamed,
            format!("{each} names no child"),
        ));
    }
    let mut seen = BTreeSet::new();
    for (index, name) in names.iter().enumerate() {
        if !plain_child_name(name) {
            return Err((
                OpenRefusalReason::ChildNameInvalid,
                format!(
                    "entry {index} of {each} is not one plain segment of ASCII letters, digits, - and _"
                ),
            ));
        }
        if !seen.insert(*name) {
            return Err((
                OpenRefusalReason::ChildNameRepeated,
                format!("entry {index} of {each} repeats an earlier entry"),
            ));
        }
    }
    Ok(names.into_iter().map(str::to_owned).collect())
}

/// What `check_arguments` refused, by JSON pointer and keyword alone: its reasons may quote a
/// value of the object.
fn mismatch(refused: &ArgumentsRefused) -> String {
    match refused {
        ArgumentsRefused::Schema(offences) => {
            let pointers: Vec<String> = offences
                .iter()
                .map(|(pointer, _)| format!("`{pointer}`"))
                .collect();
            format!(
                "the action's arguments schema is outside Canon's dialect at {}",
                pointers.join(", ")
            )
        }
        ArgumentsRefused::Arguments(refusals) => {
            let each: Vec<String> = refusals
                .iter()
                .map(|refusal| {
                    format!(
                        "the arguments at {:?} fail {}",
                        refusal.pointer, refusal.keyword
                    )
                })
                .collect();
            each.join("; ")
        }
    }
}

/// The text of `source` in the `loom/open` observation and the step's report.
fn source_text(source: ChildNameSource) -> &'static str {
    match source {
        ChildNameSource::Name => "Name",
        ChildNameSource::Request => "Request",
        ChildNameSource::Literal => "Literal",
        ChildNameSource::ArgumentsFrom => "ArgumentsFrom",
    }
}

/// `text`, JSON the port wrote itself, read back.
fn json_of(text: &str) -> Result<serde_json::Value, EffectError> {
    serde_json::from_str(text)
        .map_err(|error| EffectError::new(format!("a JSON value the port read: {error}")))
}

/// A refused step of `action` for `reason`, as `{reason:?}: action {action}: {detail}`.
fn open_refusal(
    reason: OpenRefusalReason,
    action: &str,
    detail: impl Into<String>,
) -> EffectOutcome {
    let refusal = OpenRefusal {
        reason,
        action: action.to_owned(),
        detail: detail.into(),
    };
    refused(format!(
        "{:?}: action {}: {}",
        refusal.reason, refusal.action, refusal.detail
    ))
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
