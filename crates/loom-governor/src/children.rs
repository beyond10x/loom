//! Child cases (story:child-case-runtime): opening the children of a `case_open` action, the link
//! between parent and child, the parent's one `case_outcome` record per child, the stored
//! termination, the tree of cases and the opening-cycle check at registration.
//!
//! The order of the writes is what keeps a restart safe. Opening writes each child first, holding
//! its `parent` link, then the parent once: the links, the `open` record of every child and the
//! opening action's own records together. A restart between the two leaves a child whose parent
//! does not list it; the next opening of the same name by the same action at the same subject
//! revision takes that child over instead of opening a second. Terminating writes the child's
//! termination first, then replaces the parent's record; a restart between the two is completed by
//! terminating the child again through the same outcome, which replaces the record once more and
//! adds none.

use std::collections::BTreeSet;

use b10x_canon::ir::check_openings;
use b10x_canon::model::ActionId;
use b10x_loom_commission::model::json::Value as Json;
use b10x_loom_commission::model::responsibility::{EvidenceId, FrontierAction};
use loom::governor::StoredCaseId;
use loom::primitives::{Timestamp, Uuid as LoomUuid};

use super::*;

/// The producer of every `case_outcome` record the governor writes into a parent.
pub const CHILD_OUTCOME_PRODUCER: &str = "loom-child-case";

/// The revision a child opens with for an artifact its protocol declares and its parent does not
/// hold; one its parent holds opens at the parent's revision.
pub const UNREVISED: &str = "r0";

/// The `result` of a child's record while the child is open (Canon's `OPEN_OUTCOME`).
const OPEN_RESULT: &str = "open";

/// One child case to open: its name (the last segment of its id), the registered protocol it is
/// opened under (`<name>@<major>`, whose id must be the one `case_open.protocol` names) and the
/// independence its own bindings file declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildOpening {
    /// One plain segment: ASCII letters, digits, `-` and `_`.
    pub name: String,
    /// The registration the child is opened under.
    pub protocol: String,
    /// The dimensions on which each producer is independent of the child's work.
    pub independence: CaseIndependence,
}

/// One case of a tree ([`CanonGovernor::tree`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeCase {
    /// The case.
    pub case: CaseId,
    /// The registered protocol it is held under.
    pub protocol: String,
    /// Its link to its parent; `None` for the root.
    pub parent: Option<CaseLink>,
}

/// Why no child was opened ([`CanonGovernor::open_children`]). Nothing is written, unless the
/// error is the store's: then children already written are taken over by the next opening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildError {
    /// The store failed, or the parent cannot be decided.
    StoreUnavailable { problem: String },
    /// The governor does not hold the parent.
    UnknownCase,
    /// The parent is terminated.
    Terminated { outcome: String },
    /// The parent is no longer at the revision the request was made at.
    Stale { expected: i64, current: i64 },
    /// The parent's protocol declares no `case_open` on `action` (or no such action).
    NoCaseOpen { action: String },
    /// Canon holds `action` blocked for the parent.
    NotAdmitted { action: String },
    /// `protocol` is not the protocol `case_open` names.
    ChildOfAnotherProtocol { protocol: String, case_open: String },
    /// No child was named.
    NoChild,
    /// `name` is not one plain segment.
    InvalidName { name: String },
    /// `name` is given twice.
    RepeatedName { name: String },
    /// A record of `kind` is not one the opening action produces for the parent.
    RecordNotProduced { kind: String },
    /// The instant is not one Canon reads.
    InvalidInstant { instant: String },
    /// The child `name` cannot be opened.
    Open { name: String, error: OpenError },
}

impl fmt::Display for ChildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable { problem } => write!(f, "case store unavailable: {problem}"),
            Self::UnknownCase => f.write_str("the governor does not hold the case"),
            Self::Terminated { outcome } => write!(f, "the case is terminated `{outcome}`"),
            Self::Stale { expected, current } => write!(
                f,
                "the case is at revision {current}, not {expected} as the request was made at"
            ),
            Self::NoCaseOpen { action } => {
                write!(f, "action `{action}` declares no case_open")
            }
            Self::NotAdmitted { action } => write!(f, "action `{action}` is blocked"),
            Self::ChildOfAnotherProtocol {
                protocol,
                case_open,
            } => write!(
                f,
                "protocol `{protocol}` is not `{case_open}`, which case_open names"
            ),
            Self::NoChild => f.write_str("no child case is named"),
            Self::InvalidName { name } => write!(
                f,
                "child name {name:?} is not one segment of letters, digits, `-` and `_`"
            ),
            Self::RepeatedName { name } => write!(f, "child name `{name}` is given twice"),
            Self::RecordNotProduced { kind } => write!(
                f,
                "a `{kind}` record is not one the opening action produces for the case"
            ),
            Self::InvalidInstant { instant } => write!(f, "instant {instant:?} is not RFC 3339"),
            Self::Open { name, error } => write!(f, "child `{name}`: {error}"),
        }
    }
}

impl std::error::Error for ChildError {}

/// Why a case was not terminated ([`CanonGovernor::terminate`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminateError {
    /// The store failed, or the case cannot be decided.
    StoreUnavailable { problem: String },
    /// The governor does not hold the case.
    UnknownCase,
    /// Canon does not judge the termination legitimate: `code` is Canon's refusal code
    /// (`illegitimate-termination`, `undeclared-outcome`, ...). Nothing is written.
    Refused { code: String, message: String },
    /// The case is already terminated through another outcome.
    AlreadyTerminated { outcome: String },
    /// The instant is not one Canon reads.
    InvalidInstant { instant: String },
    /// The case's parent does not hold the case's link, or is not held: the link is no link, and
    /// nothing is written.
    LinkNotHeld { parent: String },
}

impl fmt::Display for TerminateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StoreUnavailable { problem } => write!(f, "case store unavailable: {problem}"),
            Self::UnknownCase => f.write_str("the governor does not hold the case"),
            Self::Refused { code, message } => write!(f, "{code}: {message}"),
            Self::AlreadyTerminated { outcome } => {
                write!(f, "the case is already terminated `{outcome}`")
            }
            Self::InvalidInstant { instant } => write!(f, "instant {instant:?} is not RFC 3339"),
            Self::LinkNotHeld { parent } => {
                write!(f, "parent `{parent}` does not hold the case's link")
            }
        }
    }
}

impl std::error::Error for TerminateError {}

/// Refuses `protocols` when their `case_open` openings form a cycle, as Canon's `check_openings`
/// names it; `name` is the registration (or `catalog`) the refusal is about.
pub(crate) fn no_opening_cycle<'a>(
    name: &str,
    protocols: impl IntoIterator<Item = &'a Ir>,
) -> Result<(), OpenError> {
    check_openings(protocols).map_err(|cycle| OpenError::InvalidProtocol {
        protocol: name.to_owned(),
        problem: format!("{}: {cycle}", cycle.code()),
    })
}

/// `action` as the frontier of `state` lists it: unchanged while the case is open, blocked with
/// the reason `{"terminated":"<outcome>"}` once it is terminated.
pub(crate) fn after_termination(state: &CaseState, mut action: FrontierAction) -> FrontierAction {
    if let Some(termination) = &state.termination {
        action.status = ActionStatus::Blocked;
        let mut reason = String::from("{");
        loom::json::push_text(&mut reason, "terminated");
        reason.push(':');
        loom::json::push_text(&mut reason, &termination.outcome);
        reason.push('}');
        action.reasons.push(reason);
    }
    action
}

/// Whether `data` is a `case_outcome` record, by its kind or its facts.
fn is_case_outcome(data: &EvidenceData) -> bool {
    data.kind == CASE_OUTCOME
        || matches!(data.facts.member("kind"), Some(Json::Text(kind)) if kind == CASE_OUTCOME)
        || data.facts.member("child_case").is_some()
}

/// The child a `case_outcome` record's facts name.
fn child_named(data: &EvidenceData) -> Option<&str> {
    match data.facts.member("child_case") {
        Some(Json::Text(child)) => Some(child),
        _ => None,
    }
}

/// Whether `name` is one plain segment of a child id.
fn plain_segment(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// A UUID (version 8) derived from `input`, as [`frontier_uuid`] derives one.
fn derived_uuid(input: &str) -> String {
    let high = fnv1a(0xcbf2_9ce4_8422_2325, input.as_bytes());
    let low = fnv1a(0x6c62_272e_07bb_0142, input.as_bytes());
    let bits = (u128::from(high) << 64 | u128::from(low)) & !(0xf << 76) & !(0x3 << 62)
        | (0x8 << 76)
        | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn text(value: &str) -> Json {
    Json::Text(value.to_owned())
}

/// The parent's one `case_outcome` record for the child `link` names, with `result`, the instant
/// `observed_at` and the artifact revision `subject_revision`, at the parent's case revision
/// `case_revision`.
fn outcome_record(
    link: &CaseLink,
    case_revision: i64,
    result: &str,
    observed_at: &str,
    subject_revision: &str,
) -> EvidenceData {
    EvidenceData {
        evidence_id: EvidenceId(Uuid(link.outcome_evidence.0.clone())),
        case_id: CaseId(link.parent.0.clone()),
        kind: CASE_OUTCOME.to_owned(),
        subject_revision: case_revision,
        producer: CHILD_OUTCOME_PRODUCER.to_owned(),
        observation_ids: Vec::new(),
        facts: Json::Object(vec![
            ("format".to_owned(), text("canon-evidence/1")),
            (
                "id".to_owned(),
                text(&format!("case-outcome-{}", link.outcome_evidence.0)),
            ),
            ("kind".to_owned(), text(CASE_OUTCOME)),
            ("subject".to_owned(), text(&link.subject)),
            ("subject_revision".to_owned(), text(subject_revision)),
            ("result".to_owned(), text(result)),
            ("child_case".to_owned(), text(&link.child.0)),
            ("observed_at".to_owned(), text(observed_at)),
        ]),
        provenance: Json::Object(vec![
            ("source".to_owned(), text(CHILD_OUTCOME_PRODUCER)),
            ("action".to_owned(), text(&link.action)),
        ]),
    }
}

/// Whether Canon reads `instant`.
fn readable_instant(instant: &str) -> bool {
    CanonInstant::new(instant).seconds().is_some()
}

/// One child the opening resolved: the link, and the state to write when it is new (`None` for a
/// child an earlier, interrupted opening wrote, which is taken over).
struct Resolved {
    link: CaseLink,
    new: Option<CaseState>,
}

impl<S: FallibleCaseStore, T> CanonGovernor<S, T> {
    /// The store the governor holds its cases in.
    pub fn store(&self) -> &S {
        &self.store
    }
}

impl<S: FallibleCaseStore, T: EvaluationTime> CanonGovernor<S, T> {
    /// The case as held, its links and termination included; refused as `frontier` refuses it.
    pub fn case_state(&self, case: &CaseId) -> Result<CaseState, GovernorError> {
        self.held(case)
    }

    /// Refuses `data`, submitted through the evidence port, when it is a `case_outcome` record and
    /// either `held` opened a child or the record names a case the store holds: a child's outcome
    /// is written by the governor alone, when it opens and terminates the child.
    pub(crate) fn refuse_submitted_outcome(
        &self,
        held: &CaseState,
        data: &EvidenceData,
    ) -> Result<(), GovernorError> {
        if !is_case_outcome(data) {
            return Ok(());
        }
        if !held.children.is_empty() || held.parent.is_some() {
            return Err(GovernorError::GovernorUnavailable);
        }
        if let Some(child) = child_named(data)
            && self
                .store
                .get(&CaseId(child.to_owned()))
                .map_err(|_| GovernorError::GovernorUnavailable)?
                .is_some()
        {
            return Err(GovernorError::GovernorUnavailable);
        }
        Ok(())
    }

    /// Opens the child cases `children` of `parent` by performing its action `action`, at the
    /// parent's revision `expected_revision`, at the trusted instant `at`, and records `records`,
    /// the action's own evidence, in the parent.
    ///
    /// The action must declare `case_open` and Canon must not hold it blocked for the parent,
    /// which must be open and at `expected_revision`. Each child is opened under its
    /// [`ChildOpening::protocol`], which must be the protocol `case_open` names, with its
    /// independence, no case input, and for every artifact its protocol declares the parent's
    /// revision, or [`UNREVISED`] when the parent holds none. Its id is `<parent>.<name>`, or
    /// `<parent>.<name>.<n>` with the least `n` from 2 not yet taken by a case or by a record of the
    /// parent. Each child holds its link as `parent`; then the parent, in one write, holds every
    /// link under `children`, one `case_outcome` record per child naming it, about the `case_open`
    /// subject at the parent's revision of it, with result `open` and the instant `at`, and
    /// `records`. A record of another case, of `case_outcome`, or of a kind the action does not
    /// produce, refuses the opening.
    ///
    /// # Errors
    /// [`ChildError`]; nothing is written, unless the store fails between the children and the
    /// parent: the next opening of the same name takes the child written over.
    pub fn open_children(
        &self,
        parent: &CaseId,
        action: &str,
        expected_revision: i64,
        children: &[ChildOpening],
        records: Vec<EvidenceData>,
        at: &str,
    ) -> Result<Vec<CaseId>, ChildError> {
        let unavailable = |problem: String| ChildError::StoreUnavailable { problem };
        let Decided {
            state,
            ir,
            decision,
            ..
        } = self.decided_at(parent).map_err(|error| match error {
            GovernorError::UnknownCase => ChildError::UnknownCase,
            GovernorError::GovernorUnavailable => {
                unavailable("the parent cannot be decided".into())
            }
        })?;
        if let Some(termination) = &state.termination {
            return Err(ChildError::Terminated {
                outcome: termination.outcome.clone(),
            });
        }
        if state.revision != expected_revision {
            return Err(ChildError::Stale {
                expected: expected_revision,
                current: state.revision,
            });
        }
        let no_case_open = || ChildError::NoCaseOpen {
            action: action.to_owned(),
        };
        let declared = ir
            .actions
            .get(&ActionId::new(action))
            .ok_or_else(no_case_open)?;
        let opening = declared.case_open.as_ref().ok_or_else(no_case_open)?;
        let projection =
            project(&ir, &decision).ok_or_else(|| unavailable("no projection".into()))?;
        if projection.actions.iter().any(|projected| {
            projected.action == action && matches!(projected.status, Status::Blocked)
        }) {
            return Err(ChildError::NotAdmitted {
                action: action.to_owned(),
            });
        }
        if children.is_empty() {
            return Err(ChildError::NoChild);
        }
        let mut names = BTreeSet::new();
        for child in children {
            if !plain_segment(&child.name) {
                return Err(ChildError::InvalidName {
                    name: child.name.clone(),
                });
            }
            if !names.insert(child.name.as_str()) {
                return Err(ChildError::RepeatedName {
                    name: child.name.clone(),
                });
            }
        }
        if !readable_instant(at) {
            return Err(ChildError::InvalidInstant {
                instant: at.to_owned(),
            });
        }
        for record in &records {
            if record.case_id != *parent
                || is_case_outcome(record)
                || !declared
                    .may_produce
                    .iter()
                    .any(|kind| kind.as_str() == record.kind)
            {
                return Err(ChildError::RecordNotProduced {
                    kind: record.kind.clone(),
                });
            }
            // The observations it cites, as for a record the evidence port receives.
            if self.check_evidence(record).is_err() {
                return Err(ChildError::RecordNotProduced {
                    kind: record.kind.clone(),
                });
            }
        }
        let subject = opening.subject.as_str().to_owned();
        let subject_revision =
            state.artifacts.get(&subject).cloned().ok_or_else(|| {
                unavailable(format!("the parent holds no revision of `{subject}`"))
            })?;
        // Every child id a record of the parent names: never the id of a new child.
        let named: BTreeSet<String> = state
            .evidence
            .iter()
            .filter(|held| is_case_outcome(&held.data))
            .filter_map(|held| child_named(&held.data).map(str::to_owned))
            .chain(state.children.iter().map(|link| link.child.0.clone()))
            .collect();
        let mut resolved = Vec::with_capacity(children.len());
        for child in children {
            let open_error = |error: OpenError| ChildError::Open {
                name: child.name.clone(),
                error,
            };
            let child_ir = self.protocol_ir(&child.protocol).map_err(open_error)?;
            if child_ir.protocol.id != opening.protocol {
                return Err(ChildError::ChildOfAnotherProtocol {
                    protocol: child.protocol.clone(),
                    case_open: opening.protocol.as_str().to_owned(),
                });
            }
            let revisions: BTreeMap<String, String> = child_ir
                .artifacts
                .keys()
                .map(|artifact| {
                    let artifact = artifact.as_str();
                    let revision = state
                        .artifacts
                        .get(artifact)
                        .cloned()
                        .unwrap_or_else(|| UNREVISED.to_owned());
                    (artifact.to_owned(), revision)
                })
                .collect();
            let child_ir =
                checked(child_ir, &revisions, &CaseInputs::none()).map_err(open_error)?;
            checked_independence(&child_ir, &child.independence).map_err(open_error)?;
            let base = format!("{}.{}", parent.0, child.name);
            let mut n = 1u64;
            let found = loop {
                let id = if n == 1 {
                    base.clone()
                } else {
                    format!("{base}.{n}")
                };
                n += 1;
                if named.contains(&id) {
                    continue;
                }
                let link = CaseLink {
                    parent: StoredCaseId(parent.0.clone()),
                    child: StoredCaseId(id.clone()),
                    action: action.to_owned(),
                    outcome_evidence: LoomUuid(derived_uuid(&format!(
                        "case_outcome\n{}\n{id}",
                        parent.0
                    ))),
                    subject: subject.clone(),
                    subject_revision: subject_revision.clone(),
                };
                let case = CaseId(id);
                match self
                    .store
                    .get(&case)
                    .map_err(|error| unavailable(error.to_string()))?
                {
                    None => {
                        let mut state = opened(
                            &case,
                            &child.protocol,
                            &revisions,
                            &CaseInputs::none(),
                            &child.independence,
                        );
                        state.parent = Some(link.clone());
                        break Resolved {
                            link,
                            new: Some(state),
                        };
                    }
                    // Written by an opening the parent's write did not follow: taken over.
                    Some(held)
                        if held.parent.as_ref() == Some(&link)
                            && held.protocol == child.protocol
                            && held.termination.is_none() =>
                    {
                        break Resolved { link, new: None };
                    }
                    Some(_) => {}
                }
            };
            resolved.push(found);
        }
        for child in &resolved {
            if let Some(state) = &child.new {
                let inserted = self
                    .store
                    .insert(state.clone())
                    .map_err(|error| unavailable(error.to_string()))?;
                if !inserted {
                    return Err(unavailable(format!(
                        "case `{}` was opened meanwhile",
                        child.link.child.0
                    )));
                }
            }
        }
        let time = self
            .time
            .at(parent)
            .map_err(|_| unavailable("the trusted time cannot be read".into()))?;
        self.store
            .update(parent, |held| {
                if held.revision != expected_revision || held.termination.is_some() {
                    return Err(ChildError::Stale {
                        expected: expected_revision,
                        current: held.revision,
                    });
                }
                for child in &resolved {
                    held.children.push(child.link.clone());
                    let data = outcome_record(
                        &child.link,
                        held.revision,
                        OPEN_RESULT,
                        at,
                        &child.link.subject_revision,
                    );
                    let applies = applies(&ir, held, &data, time.as_deref());
                    held.evidence.push(HeldEvidence { data, applies });
                }
                for data in records {
                    let applies = applies(&ir, held, &data, time.as_deref());
                    held.evidence.push(HeldEvidence { data, applies });
                }
                Ok(())
            })
            .map_err(|error| unavailable(error.to_string()))?
            .unwrap_or(Err(ChildError::UnknownCase))?;
        Ok(resolved
            .into_iter()
            .map(|child| CaseId(child.link.child.0))
            .collect())
    }

    /// Terminates `case` through `outcome` at the trusted `instant`, once Canon judges the
    /// termination legitimate for the case as held; and, for a child, replaces its parent's one
    /// `case_outcome` record for it with the outcome, the instant and the revision the outcome was
    /// computed against: the child's own revision of the `case_open` subject when its protocol
    /// declares that artifact, else the parent's revision of it when the child was opened.
    ///
    /// The parent must hold the child's link, or nothing is written. Terminating a terminated case
    /// again through the same outcome writes the child nothing and replaces the parent's record
    /// once more, which completes a termination a restart interrupted; through another outcome it
    /// is refused.
    ///
    /// # Errors
    /// [`TerminateError`]; a refused termination writes nothing.
    pub fn terminate(
        &self,
        case: &CaseId,
        outcome: &str,
        instant: &str,
    ) -> Result<(), TerminateError> {
        let unavailable = |problem: String| TerminateError::StoreUnavailable { problem };
        if !readable_instant(instant) {
            return Err(TerminateError::InvalidInstant {
                instant: instant.to_owned(),
            });
        }
        let (state, ir) = self.held_with_ir(case).map_err(|error| match error {
            GovernorError::UnknownCase => TerminateError::UnknownCase,
            GovernorError::GovernorUnavailable => unavailable("the case cannot be decided".into()),
        })?;
        if let Some(link) = &state.parent {
            self.linked_parent(link)?;
        }
        match &state.termination {
            Some(held) if held.outcome == outcome => {}
            Some(held) => {
                return Err(TerminateError::AlreadyTerminated {
                    outcome: held.outcome.clone(),
                });
            }
            None => {
                let at = self
                    .time
                    .at(case)
                    .map_err(|_| unavailable("the trusted time cannot be read".into()))?;
                judge(&ir, &state, outcome, at.as_deref())?;
                self.store
                    .update(case, |held| {
                        if let Some(termination) = &held.termination {
                            return if termination.outcome == outcome {
                                Ok(())
                            } else {
                                Err(TerminateError::AlreadyTerminated {
                                    outcome: termination.outcome.clone(),
                                })
                            };
                        }
                        judge(&ir, held, outcome, at.as_deref())?;
                        held.termination = Some(CaseTermination {
                            outcome: outcome.to_owned(),
                            instant: Timestamp(instant.to_owned()),
                        });
                        Ok(())
                    })
                    .map_err(|error| unavailable(error.to_string()))?
                    .unwrap_or(Err(TerminateError::UnknownCase))?;
            }
        }
        let Some(link) = state.parent.clone() else {
            return Ok(());
        };
        let terminated = self
            .held(case)
            .map_err(|_| unavailable("the terminated case cannot be read".into()))?;
        let Some(termination) = terminated.termination else {
            return Err(unavailable("the termination is not held".into()));
        };
        let subject_revision = if ir
            .artifacts
            .contains_key(&ArtifactId::new(link.subject.as_str()))
        {
            terminated
                .artifacts
                .get(&link.subject)
                .cloned()
                .unwrap_or_else(|| link.subject_revision.clone())
        } else {
            link.subject_revision.clone()
        };
        let parent = CaseId(link.parent.0.clone());
        let (_, parent_ir) = self
            .held_with_ir(&parent)
            .map_err(|_| unavailable("the parent cannot be decided".into()))?;
        let time = self
            .time
            .at(&parent)
            .map_err(|_| unavailable("the trusted time cannot be read".into()))?;
        self.store
            .update(&parent, |held| {
                if !held.children.contains(&link) {
                    return Err(TerminateError::LinkNotHeld {
                        parent: link.parent.0.clone(),
                    });
                }
                let Some(index) = held
                    .evidence
                    .iter()
                    .position(|record| record.data.evidence_id.0.0 == link.outcome_evidence.0)
                else {
                    return Err(unavailable(format!(
                        "the parent holds no record for child `{}`",
                        link.child.0
                    )));
                };
                let data = outcome_record(
                    &link,
                    held.evidence[index].data.subject_revision,
                    &termination.outcome,
                    &termination.instant.0,
                    &subject_revision,
                );
                let mut without = held.clone();
                without.evidence.remove(index);
                let applies = applies(&parent_ir, &without, &data, time.as_deref());
                held.evidence[index] = HeldEvidence { data, applies };
                Ok(())
            })
            .map_err(|error| unavailable(error.to_string()))?
            .unwrap_or(Err(TerminateError::LinkNotHeld {
                parent: link.parent.0.clone(),
            }))
    }

    /// The parent `link` names, when it holds `link`; otherwise the link is no link.
    fn linked_parent(&self, link: &CaseLink) -> Result<CaseState, TerminateError> {
        let not_held = || TerminateError::LinkNotHeld {
            parent: link.parent.0.clone(),
        };
        let parent = self
            .store
            .get(&CaseId(link.parent.0.clone()))
            .map_err(|error| TerminateError::StoreUnavailable {
                problem: error.to_string(),
            })?
            .ok_or_else(not_held)?;
        if parent.children.contains(link) {
            Ok(parent)
        } else {
            Err(not_held())
        }
    }

    /// Every case of the tree `case` belongs to, the root first and then each case before its
    /// children, in the order they were opened, each with its link to its parent. A link counts
    /// only when both sides hold it: a case whose parent does not list it is the root of its own
    /// tree, listed with no parent.
    ///
    /// # Errors
    /// `UnknownCase` when `case` is not held; `GovernorUnavailable` when the store fails, or a
    /// child a parent lists is not held or holds another link.
    pub fn tree(&self, case: &CaseId) -> Result<Vec<TreeCase>, GovernorError> {
        let get = |case: &CaseId| {
            self.store
                .get(case)
                .map_err(|_| GovernorError::GovernorUnavailable)
        };
        let mut root = get(case)?.ok_or(GovernorError::UnknownCase)?;
        let mut climbed = BTreeSet::from([root.id.0.clone()]);
        while let Some(link) = root.parent.clone() {
            match get(&CaseId(link.parent.0.clone()))? {
                Some(parent)
                    if parent.children.contains(&link) && climbed.insert(parent.id.0.clone()) =>
                {
                    root = parent;
                }
                _ => break,
            }
        }
        let mut listed = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![(root, true)];
        while let Some((state, is_root)) = pending.pop() {
            if !seen.insert(state.id.0.clone()) {
                return Err(GovernorError::GovernorUnavailable);
            }
            for link in state.children.iter().rev() {
                let child = get(&CaseId(link.child.0.clone()))?
                    .ok_or(GovernorError::GovernorUnavailable)?;
                if child.parent.as_ref() != Some(link) {
                    return Err(GovernorError::GovernorUnavailable);
                }
                pending.push((child, false));
            }
            listed.push(TreeCase {
                case: state.id,
                protocol: state.protocol,
                parent: if is_root { None } else { state.parent },
            });
        }
        Ok(listed)
    }
}

/// Canon's judgement of `state` terminated through `outcome`, with the records that apply, at the
/// trusted instant `at` and with its explicit decisions.
fn judge(
    ir: &Ir,
    state: &CaseState,
    outcome: &str,
    at: Option<&str>,
) -> Result<(), TerminateError> {
    let records: Vec<EvidenceRecord> = state
        .evidence
        .iter()
        .filter(|held| held.applies)
        .filter_map(|held| canon_record(&held.data))
        .collect();
    let case = Case {
        termination: Some(OutcomeId::new(outcome)),
        ..snapshot(ir, state)
    };
    decide_case(ir, &case, &records, at, &state.decisions)
        .map(|_| ())
        .map_err(|refusal| TerminateError::Refused {
            code: refusal.code().to_owned(),
            message: refusal.to_string(),
        })
}
