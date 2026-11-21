//! The eight commands of `loom.decision`, each answering the outcome `ess/domains/decision.yaml`
//! declares for it.
//!
//! Every command follows ESS's precedence (`ess/37`): the refusals guarded by the command alone
//! in declaration order; then whether the addressed request exists (`no-request`); then the
//! branches reading what it holds, in declaration order, then `wrong-state`; then acceptance. A
//! guard is read as ESS reads it, three-valued: a comparison with something absent is unknown,
//! and a branch whose guard is unknown decides no outcome ([`DecisionError::Undecided`]). Only an
//! outcome that creates or moves the request writes it, from the state read; every refusal, and
//! `already-recorded`, writes nothing.

use std::cmp::Ordering;
use std::fmt;

use loom::decision::{
    AnswerNotAllowed, Decide, DecideOutcome, DecisionEscalated, DecisionExpired,
    DecisionRequestData, DecisionRequestId, DecisionRequestSnapshot, DecisionRequestState,
    DecisionRequested, DecisionStateConflict, DecisionSuperseded, DecisionTaken, Escalate,
    EscalateOutcome, EscalationSuppressed, Expire, ExpireOutcome, Grant, Holder,
    NoDefaultOnSilence, NoRecordToProduce, NoSuchRequest, NotARequestClass, NotDue, NotEligible,
    OptionOrAnswer, Principal, PrincipalKind, ProducedRecordConflict, ProducedRecordRecorded,
    Recommend, RecommendOutcome, RecommendationRecorded, RecommendationTaken,
    RecommendationWithheld, RecordKeyOpen, RecordProduced, RecordProducedOutcome, Request,
    RequestOutcome, SilenceHasDefault, Strength, Supersede, SupersedeOutcome, TakeRecommendation,
    TakeRecommendationOutcome, TooFewOptions, UnblockKind, UngroundedRecommendation,
};
use loom::primitives::Timestamp;

use crate::instant;
use crate::store::{DecisionStore, StoreConflict, StoreFailure, WriteError};

/// The class `Request` refuses: the escalation row gives a request's holder after `Escalate`.
const ESCALATION: &str = "escalation";

/// A command the domain declares no outcome for: a guard it reaches is unknown (an instant that
/// does not read as RFC 3339, or an option compared while absent), or the outcome would leave the
/// request breaking an invariant. Nothing was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undecided {
    /// The command, as the domain names it.
    pub command: &'static str,
    /// Why no outcome is decided.
    pub reason: String,
}

impl fmt::Display for Undecided {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} decides no outcome: {}", self.command, self.reason)
    }
}

impl std::error::Error for Undecided {}

/// Why a command was not applied. None of these is an outcome of the domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionError {
    /// The store could not read or write.
    Store(StoreFailure),
    /// The request changed in the store between the command's read and its write.
    Conflict(StoreConflict),
    /// The domain declares no outcome for the command over what it read.
    Undecided(Undecided),
}

impl fmt::Display for DecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(failure) => failure.fmt(f),
            Self::Conflict(conflict) => conflict.fmt(f),
            Self::Undecided(undecided) => undecided.fmt(f),
        }
    }
}

impl std::error::Error for DecisionError {}

impl From<StoreFailure> for DecisionError {
    fn from(failure: StoreFailure) -> Self {
        Self::Store(failure)
    }
}

impl From<WriteError> for DecisionError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Conflict(conflict) => Self::Conflict(conflict),
            WriteError::Failure(failure) => Self::Store(failure),
        }
    }
}

/// Kleene `and`: false when any is false, else unknown when any is unknown, else true.
fn all<const N: usize>(truths: [Option<bool>; N]) -> Option<bool> {
    if truths.contains(&Some(false)) {
        Some(false)
    } else if truths.contains(&None) {
        None
    } else {
        Some(true)
    }
}

/// Kleene `or`: true when any is true, else unknown when any is unknown, else false.
fn any(truths: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
    let mut unknown = false;
    for truth in truths {
        match truth {
            Some(true) => return Some(true),
            None => unknown = true,
            Some(false) => {}
        }
    }
    if unknown { None } else { Some(false) }
}

/// Whether a branch's guard holds; an unknown guard decides no outcome.
fn holds(command: &'static str, branch: &str, truth: Option<bool>) -> Result<bool, DecisionError> {
    truth.ok_or_else(|| {
        DecisionError::Undecided(Undecided {
            command,
            reason: format!("the guard of `{branch}` reads something absent or no instant"),
        })
    })
}

/// `left > right` as instants; unknown when either does not read.
fn after(left: &Timestamp, right: &Timestamp) -> Option<bool> {
    instant::compare(left, right).map(|order| order == Ordering::Greater)
}

/// `not defined(expires_at) or expires_at > at`, the not-due guard of `TakeRecommendation` by
/// silence and of `Expire`.
fn not_due(expires_at: Option<&Timestamp>, at: &Timestamp) -> Option<bool> {
    match expires_at {
        None => Some(true),
        Some(expires_at) => after(expires_at, at),
    }
}

/// Writes `data` in `state` over `held`, the request as it was read, unless it breaks an
/// invariant of the entity.
fn write<S: DecisionStore + ?Sized>(
    store: &mut S,
    command: &'static str,
    held: Option<&DecisionRequestSnapshot>,
    state: DecisionRequestState,
    data: DecisionRequestData,
) -> Result<(), DecisionError> {
    if let Some(invariant) = data.broken_invariant() {
        return Err(DecisionError::Undecided(Undecided {
            command,
            reason: format!("the outcome would break the invariant {invariant}"),
        }));
    }
    store.write(held, DecisionRequestSnapshot { state, data })?;
    Ok(())
}

fn conflict(request_id: &DecisionRequestId) -> DecisionStateConflict {
    DecisionStateConflict {
        request_id: request_id.clone(),
    }
}

fn missing(request_id: &DecisionRequestId) -> NoSuchRequest {
    NoSuchRequest {
        request_id: request_id.clone(),
    }
}

/// The kind a principal's name has, the `kind-not-principal` guard of `Decide` and
/// `TakeRecommendation` (decision.yaml): `decider:<kind>` is a decider, `rule:<name>` a rule,
/// `silence` silence, and any other name a person.
fn kind_of(principal: &Principal) -> PrincipalKind {
    let name = principal.0.as_str();
    if name.starts_with("decider:") {
        PrincipalKind::Decider
    } else if name.starts_with("rule:") {
        PrincipalKind::Rule
    } else if name == "silence" {
        PrincipalKind::Silence
    } else {
        PrincipalKind::Person
    }
}

fn active(state: DecisionRequestState) -> bool {
    matches!(
        state,
        DecisionRequestState::Open | DecisionRequestState::Escalated
    )
}

/// `loom.decision.Request`: opens a request, Open with no recommendation.
///
/// `RecordKeyOpen` names the Open or Escalated request the store finds for `record_key`, or,
/// when it finds none, the one `open_for_record` names. A request for the key that another
/// writer opens after that check is refused by the store in the step that writes
/// ([`DecisionStore::write`]); `Request` then answers `RecordKeyOpen` naming it, and nothing is
/// written.
///
/// # Errors
///
/// [`DecisionError`] when the store fails, or holds a request under the id already
/// ([`DecisionError::Conflict`]).
pub fn request<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Request,
) -> Result<RequestOutcome, DecisionError> {
    let request_id = input.request_id.clone();
    if input.decision_class.0 == ESCALATION {
        return Ok(RequestOutcome::EscalationIsNotAClass {
            error: NotARequestClass { request_id },
        });
    }
    if input.options.len() < 2 {
        return Ok(RequestOutcome::TooFewOptions {
            error: TooFewOptions { request_id },
        });
    }
    let open = store
        .open_for_record(&input.record_key)?
        .or(input.open_for_record);
    if let Some(open_for_record) = open {
        return Ok(RequestOutcome::RecordKeyOpen {
            error: RecordKeyOpen {
                request_id,
                open_for_record,
            },
        });
    }
    let requested = DecisionRequested {
        request_id: request_id.clone(),
        case_id: input.case_id.clone(),
        subject_artifact: input.subject_artifact.clone(),
        subject_revision: input.subject_revision.clone(),
    };
    let data = DecisionRequestData {
        request_id,
        case_id: input.case_id,
        question_key: input.question_key,
        record_key: input.record_key,
        unblocks: input.unblocks,
        subject_artifact: input.subject_artifact,
        subject_revision: input.subject_revision,
        subject_path: input.subject_path,
        question: input.question,
        options: input.options,
        autonomy_level: input.autonomy_level,
        decision_class: input.decision_class,
        holder: input.holder,
        eligible: input.eligible,
        addressed_to: input.addressed_to,
        opened_at: input.opened_at,
        expires_at: input.expires_at,
        supersedes: input.supersedes,
        recommended_option: None,
        recommended_answer: None,
        recommendation_strength: Strength::None,
        default_on_silence: false,
        grounds: Vec::new(),
        recommended_by: None,
        grounding_report: None,
        no_recommendation_reason: None,
        escalated_to: None,
        escalation_reason: None,
        decided_option: None,
        decided_answer: None,
        decided_by: None,
        decided_by_kind: None,
        decided_at: None,
        decided_case_revision: None,
        decision_reasons: Vec::new(),
        produced_record: None,
        superseded_by: None,
        superseded_reason: None,
    };
    let request_id = data.request_id.clone();
    match write(
        store,
        "loom.decision.Request",
        None,
        DecisionRequestState::Open,
        data,
    ) {
        Ok(()) => Ok(RequestOutcome::Requested {
            decision_requested: requested,
        }),
        // Another writer opened a request for the record key after the check: the store refused
        // the write in the step that writes and names that request.
        Err(DecisionError::Conflict(StoreConflict {
            record_key_held_by: Some(open_for_record),
            ..
        })) => Ok(RequestOutcome::RecordKeyOpen {
            error: RecordKeyOpen {
                request_id,
                open_for_record,
            },
        }),
        Err(other) => Err(other),
    }
}

/// `loom.decision.Recommend`: records a verified recommendation on an Open request, or, when
/// the grounding check refused it, none and the check's report.
///
/// # Errors
///
/// [`DecisionError`] when the store fails or the request changed since it was read.
pub fn recommend<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Recommend,
) -> Result<RecommendOutcome, DecisionError> {
    let request_id = input.request_id.clone();
    let verified = input.check_verdict == loom::decision::CheckVerdict::Verified;
    if verified && (input.grounds.is_empty() || input.strength == Strength::None) {
        return Ok(RecommendOutcome::Ungrounded {
            error: UngroundedRecommendation { request_id },
        });
    }
    if verified && input.default_on_silence && input.strength != Strength::Strong {
        return Ok(RecommendOutcome::SilenceNeedsStrong {
            error: NoDefaultOnSilence { request_id },
        });
    }
    if verified && input.recommended_option.is_some() == input.recommended_answer.is_some() {
        return Ok(RecommendOutcome::OptionOrAnswer {
            error: OptionOrAnswer { request_id },
        });
    }
    let Some(held) = store.read(&request_id)? else {
        return Ok(RecommendOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    if held.data.unblocks.kind != UnblockKind::Question
        && verified
        && input.recommended_answer.is_some()
    {
        return Ok(RecommendOutcome::AnswerNotAllowed {
            error: AnswerNotAllowed { request_id },
        });
    }
    if held.state != DecisionRequestState::Open {
        return Ok(RecommendOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    let outcome = if verified {
        data.recommended_option = input.recommended_option.clone();
        data.recommended_answer = input.recommended_answer.clone();
        data.recommendation_strength = input.strength;
        data.default_on_silence = input.default_on_silence;
        data.grounds = input.grounds;
        data.recommended_by = Some(input.proposed_by);
        data.grounding_report = Some(input.check_report);
        data.no_recommendation_reason = None;
        RecommendOutcome::Recommended {
            recommendation_recorded: RecommendationRecorded {
                request_id,
                recommended_option: input.recommended_option,
                recommended_answer: input.recommended_answer,
                strength: input.strength,
            },
        }
    } else {
        data.recommended_option = None;
        data.recommended_answer = None;
        data.recommendation_strength = Strength::None;
        data.recommended_by = None;
        data.grounding_report = None;
        data.default_on_silence = false;
        data.grounds = Vec::new();
        data.no_recommendation_reason = Some(input.check_report.clone());
        RecommendOutcome::Withheld {
            recommendation_withheld: RecommendationWithheld {
                request_id,
                reason: input.check_report,
            },
        }
    };
    write(
        store,
        "loom.decision.Recommend",
        Some(&held),
        DecisionRequestState::Open,
        data,
    )?;
    Ok(outcome)
}

/// `loom.decision.Decide`: takes one option, or a free answer on a question request, of an Open
/// or Escalated request, when the stored request lets the principal decide.
///
/// # Errors
///
/// [`DecisionError`] when the store fails, the request changed since it was read, or the
/// `decider-under-rule` guard compares the absent option of an answer.
pub fn decide<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Decide,
) -> Result<DecideOutcome, DecisionError> {
    const COMMAND: &str = "loom.decision.Decide";
    let request_id = input.request_id.clone();
    let not_eligible = || NotEligible {
        request_id: request_id.clone(),
        principal: input.principal.clone(),
    };
    let kind = input.principal_kind;
    if kind != kind_of(&input.principal) {
        return Ok(DecideOutcome::KindNotPrincipal {
            error: not_eligible(),
        });
    }
    if kind == PrincipalKind::Silence {
        return Ok(DecideOutcome::SilenceTakesRecommendation {
            error: not_eligible(),
        });
    }
    if input.option.is_some() == input.answer.is_some() {
        return Ok(DecideOutcome::OptionOrAnswer {
            error: OptionOrAnswer {
                request_id: request_id.clone(),
            },
        });
    }
    let Some(held) = store.read(&request_id)? else {
        return Ok(DecideOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    let stored = &held.data;
    if stored.unblocks.kind != UnblockKind::Question && input.answer.is_some() {
        return Ok(DecideOutcome::AnswerNotAllowed {
            error: AnswerNotAllowed {
                request_id: request_id.clone(),
            },
        });
    }
    if matches!(stored.holder, Holder::Person | Holder::Advised) && kind != PrincipalKind::Person {
        return Ok(DecideOutcome::PersonOnly {
            error: not_eligible(),
        });
    }
    if !stored.eligible.contains(&input.principal) {
        return Ok(DecideOutcome::NotListed {
            error: not_eligible(),
        });
    }
    if kind == PrincipalKind::Decider {
        // `not exists o in options: o.letter == input.option and o.consequence.grant == denied`,
        // three-valued: an absent option or grant compares as unknown.
        let denying = any(stored.options.iter().map(|o| {
            all([
                input.option.as_ref().map(|letter| &o.letter == letter),
                o.consequence.grant.map(|grant| grant == Grant::Denied),
            ])
        }));
        let under_rule = any([
            Some(stored.holder == Holder::Rule),
            Some(stored.holder == Holder::RuleFinal),
            Some(stored.holder == Holder::RuleAndDecider),
            all([
                Some(stored.holder == Holder::RuleAndDeciderFinal),
                denying.map(|denies| !denies),
            ]),
        ]);
        if holds(COMMAND, "decider-under-rule", under_rule)? {
            return Ok(DecideOutcome::DeciderUnderRule {
                error: not_eligible(),
            });
        }
    }
    if kind == PrincipalKind::Rule && !matches!(stored.holder, Holder::Rule | Holder::RuleFinal) {
        return Ok(DecideOutcome::RuleNotHolder {
            error: not_eligible(),
        });
    }
    if !active(held.state) {
        return Ok(DecideOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    data.decided_option = input.option.clone();
    data.decided_answer = input.answer.clone();
    data.decided_by = Some(input.principal.clone());
    data.decided_by_kind = Some(kind);
    data.decided_at = Some(input.decided_at);
    data.decided_case_revision = Some(input.case_revision);
    data.decision_reasons = input.reasons;
    write(
        store,
        COMMAND,
        Some(&held),
        DecisionRequestState::Decided,
        data,
    )?;
    Ok(DecideOutcome::Decided {
        decision_taken: DecisionTaken {
            request_id,
            option: input.option,
            answer: input.answer,
            principal: input.principal,
        },
    })
}

/// `loom.decision.TakeRecommendation`: decides an Open or Escalated request with its strong
/// recommended option or answer, read from the stored request.
///
/// # Errors
///
/// [`DecisionError`] when the store fails, the request changed since it was read, or silence's
/// `decided_at` or the stored `expires_at` does not read as an RFC 3339 instant.
pub fn take_recommendation<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: TakeRecommendation,
) -> Result<TakeRecommendationOutcome, DecisionError> {
    const COMMAND: &str = "loom.decision.TakeRecommendation";
    let request_id = input.request_id.clone();
    let not_eligible = || NotEligible {
        request_id: request_id.clone(),
        principal: input.principal.clone(),
    };
    if input.principal_kind != kind_of(&input.principal) {
        return Ok(TakeRecommendationOutcome::KindNotPrincipal {
            error: not_eligible(),
        });
    }
    let Some(held) = store.read(&request_id)? else {
        return Ok(TakeRecommendationOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    let stored = &held.data;
    let kind = input.principal_kind;
    let silence = kind == PrincipalKind::Silence;
    if matches!(stored.holder, Holder::Person | Holder::Advised)
        && matches!(kind, PrincipalKind::Decider | PrincipalKind::Rule)
    {
        return Ok(TakeRecommendationOutcome::PersonOnly {
            error: not_eligible(),
        });
    }
    if !silence && !stored.eligible.contains(&input.principal) {
        return Ok(TakeRecommendationOutcome::NotListed {
            error: not_eligible(),
        });
    }
    if stored.recommendation_strength != Strength::Strong {
        return Ok(TakeRecommendationOutcome::NotStrong {
            error: not_eligible(),
        });
    }
    if silence && !stored.default_on_silence {
        return Ok(TakeRecommendationOutcome::SilenceWithoutDefault {
            error: NoDefaultOnSilence {
                request_id: request_id.clone(),
            },
        });
    }
    if silence
        && holds(
            COMMAND,
            "silence-not-due",
            not_due(stored.expires_at.as_ref(), &input.decided_at),
        )?
    {
        return Ok(TakeRecommendationOutcome::SilenceNotDue {
            error: NotDue {
                request_id: request_id.clone(),
            },
        });
    }
    if !active(held.state) {
        return Ok(TakeRecommendationOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    data.decided_option = data.recommended_option.clone();
    data.decided_answer = data.recommended_answer.clone();
    data.decided_by = Some(input.principal.clone());
    data.decided_by_kind = Some(kind);
    data.decided_at = Some(input.decided_at);
    data.decided_case_revision = Some(input.case_revision);
    data.decision_reasons = input.reasons;
    write(
        store,
        COMMAND,
        Some(&held),
        DecisionRequestState::Decided,
        data,
    )?;
    Ok(TakeRecommendationOutcome::Taken {
        recommendation_taken: RecommendationTaken {
            request_id,
            principal: input.principal,
        },
    })
}

/// `loom.decision.RecordProduced`: stores the Canon record the runtime wrote for a Decided
/// request, once.
///
/// # Errors
///
/// [`DecisionError`] when the store fails or the request changed since it was read.
pub fn record_produced<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: RecordProduced,
) -> Result<RecordProducedOutcome, DecisionError> {
    let request_id = input.request_id.clone();
    let Some(held) = store.read(&request_id)? else {
        return Ok(RecordProducedOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    let stored = &held.data;
    let recorded = stored
        .produced_record
        .as_ref()
        .map_or("", |record| record.record.as_str());
    if recorded == input.produced.record {
        return Ok(RecordProducedOutcome::AlreadyRecorded);
    }
    if stored.produced_record.is_some() {
        return Ok(RecordProducedOutcome::ProducedConflict {
            error: ProducedRecordConflict { request_id },
        });
    }
    if let Some(decided) = &stored.decided_option
        && !stored
            .options
            .iter()
            .any(|o| &o.letter == decided && o.consequence.produces.is_some())
    {
        return Ok(RecordProducedOutcome::OptionProducesNone {
            error: NoRecordToProduce { request_id },
        });
    }
    if held.state != DecisionRequestState::Decided {
        return Ok(RecordProducedOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    data.produced_record = Some(input.produced.clone());
    write(
        store,
        "loom.decision.RecordProduced",
        Some(&held),
        DecisionRequestState::Decided,
        data,
    )?;
    Ok(RecordProducedOutcome::Recorded {
        produced_record_recorded: ProducedRecordRecorded {
            request_id,
            produced: input.produced,
        },
    })
}

/// `loom.decision.Escalate`: moves an Open or Escalated request to a person, its holder then
/// `person`.
///
/// # Errors
///
/// [`DecisionError`] when the store fails or the request changed since it was read.
pub fn escalate<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Escalate,
) -> Result<EscalateOutcome, DecisionError> {
    let request_id = input.request_id.clone();
    let Some(held) = store.read(&request_id)? else {
        return Ok(EscalateOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    if matches!(
        held.data.holder,
        Holder::DeciderFinal | Holder::RuleFinal | Holder::RuleAndDeciderFinal
    ) {
        return Ok(EscalateOutcome::Suppressed {
            error: EscalationSuppressed { request_id },
        });
    }
    if !active(held.state) {
        return Ok(EscalateOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    data.holder = Holder::Person;
    data.escalated_to = Some(input.escalate_to.clone());
    data.addressed_to = input.escalate_to.clone();
    data.escalation_reason = Some(input.reason);
    write(
        store,
        "loom.decision.Escalate",
        Some(&held),
        DecisionRequestState::Escalated,
        data,
    )?;
    Ok(EscalateOutcome::Escalated {
        decision_escalated: DecisionEscalated {
            request_id,
            escalate_to: input.escalate_to,
        },
    })
}

/// `loom.decision.Supersede`: replaces an Open, Escalated or Decided request by a newer one,
/// keeping what it decided.
///
/// # Errors
///
/// [`DecisionError`] when the store fails or the request changed since it was read.
pub fn supersede<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Supersede,
) -> Result<SupersedeOutcome, DecisionError> {
    let request_id = input.request_id.clone();
    let Some(held) = store.read(&request_id)? else {
        return Ok(SupersedeOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    if !matches!(
        held.state,
        DecisionRequestState::Open
            | DecisionRequestState::Escalated
            | DecisionRequestState::Decided
    ) {
        return Ok(SupersedeOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let mut data = held.data.clone();
    data.superseded_by = Some(input.superseded_by.clone());
    data.superseded_reason = Some(input.reason);
    write(
        store,
        "loom.decision.Supersede",
        Some(&held),
        DecisionRequestState::Superseded,
        data,
    )?;
    Ok(SupersedeOutcome::Superseded {
        decision_superseded: DecisionSuperseded {
            request_id,
            superseded_by: input.superseded_by,
        },
    })
}

/// `loom.decision.Expire`: ends an Open or Escalated request nobody answered by `expires_at`,
/// fired by a host tick at the instant `at`.
///
/// # Errors
///
/// [`DecisionError`] when the store fails, the request changed since it was read, or `at` or
/// the stored `expires_at` does not read as an RFC 3339 instant.
pub fn expire<S: DecisionStore + ?Sized>(
    store: &mut S,
    input: Expire,
) -> Result<ExpireOutcome, DecisionError> {
    const COMMAND: &str = "loom.decision.Expire";
    let request_id = input.request_id.clone();
    let Some(held) = store.read(&request_id)? else {
        return Ok(ExpireOutcome::NoRequest {
            error: missing(&request_id),
        });
    };
    if holds(
        COMMAND,
        "not-due",
        not_due(held.data.expires_at.as_ref(), &input.at),
    )? {
        return Ok(ExpireOutcome::NotDue {
            error: NotDue { request_id },
        });
    }
    if held.data.default_on_silence {
        return Ok(ExpireOutcome::HasDefault {
            error: SilenceHasDefault { request_id },
        });
    }
    if !active(held.state) {
        return Ok(ExpireOutcome::WrongState {
            error: conflict(&request_id),
        });
    }
    let data = held.data.clone();
    write(
        store,
        COMMAND,
        Some(&held),
        DecisionRequestState::Expired,
        data,
    )?;
    Ok(ExpireOutcome::Expired {
        decision_expired: DecisionExpired { request_id },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_valued_and_and_or_read_unknown_as_ess_does() {
        assert_eq!(all([Some(true), None]), None);
        assert_eq!(all([Some(false), None]), Some(false));
        assert_eq!(all([Some(true), Some(true)]), Some(true));
        assert_eq!(any([Some(false), None]), None);
        assert_eq!(any([Some(true), None]), Some(true));
        assert_eq!(any([Some(false), Some(false)]), Some(false));
        assert_eq!(any([]), Some(false));
    }

    /// A store that counts every write it is asked for, and refuses none.
    #[derive(Default)]
    struct Counting {
        inner: crate::store::InMemoryDecisionStore,
        writes: usize,
    }

    impl DecisionStore for Counting {
        fn read(
            &self,
            request_id: &DecisionRequestId,
        ) -> Result<Option<DecisionRequestSnapshot>, StoreFailure> {
            self.inner.read(request_id)
        }

        fn write(
            &mut self,
            read: Option<&DecisionRequestSnapshot>,
            next: DecisionRequestSnapshot,
        ) -> Result<(), WriteError> {
            self.writes += 1;
            self.inner.write(read, next)
        }

        fn list(&self) -> Result<Vec<DecisionRequestSnapshot>, StoreFailure> {
            self.inner.list()
        }

        fn append_record(
            &mut self,
            record: loom::decision::EscalationSuppressedRecord,
        ) -> Result<(), crate::store::AppendError> {
            self.inner.append_record(record)
        }

        fn records(
            &self,
            request_id: &DecisionRequestId,
        ) -> Result<Vec<loom::decision::EscalationSuppressedRecord>, StoreFailure> {
            self.inner.records(request_id)
        }
    }

    fn opened() -> DecisionRequestSnapshot {
        use loom::decision::{
            ActionId, ArtifactId, AutonomyLevel, CaseId, Consequence, DecisionClassId,
            DecisionOption, OptionLetter, Revision, Unblocks,
        };
        let option = |letter: &str, grant| DecisionOption {
            letter: OptionLetter(letter.to_owned()),
            does: format!("Option {letter}."),
            cost: "None.".to_owned(),
            consequence: Consequence {
                admits: Vec::new(),
                blocks: Vec::new(),
                grant: Some(grant),
                answer: None,
                produces: None,
            },
        };
        let mut store = crate::store::InMemoryDecisionStore::new();
        let answer = request(
            &mut store,
            Request {
                request_id: DecisionRequestId("DR-1".to_owned()),
                case_id: CaseId("IDEA-1.billing".to_owned()),
                question_key: "IDEA-1.billing|change.merge|".to_owned(),
                record_key: "change.merge@r9".to_owned(),
                open_for_record: None,
                unblocks: Unblocks {
                    kind: UnblockKind::Approval,
                    action: ActionId("change.merge".to_owned()),
                    capability: None,
                    question_id: None,
                    outcome: None,
                },
                subject_artifact: ArtifactId("change".to_owned()),
                subject_revision: Revision("t0r1".to_owned()),
                subject_path: None,
                question: "Merge?".to_owned(),
                options: vec![option("A", Grant::Granted), option("B", Grant::Denied)],
                autonomy_level: AutonomyLevel::A3,
                decision_class: DecisionClassId("merge".to_owned()),
                holder: Holder::Person,
                eligible: vec![Principal("operator".to_owned())],
                addressed_to: Principal("operator".to_owned()),
                opened_at: Timestamp("2026-11-21T11:00:00Z".to_owned()),
                expires_at: None,
                supersedes: None,
            },
        )
        .expect("applied");
        assert!(
            matches!(answer, RequestOutcome::Requested { .. }),
            "{answer:?}"
        );
        store
            .read(&DecisionRequestId("DR-1".to_owned()))
            .expect("reads")
            .expect("stored")
    }

    /// `write` refuses data that breaks an invariant of the entity before it asks the store:
    /// a default on silence with a weak recommendation (`default_on_silence == false or
    /// recommendation_strength == strong`) is `Undecided`, and the store is asked for no write.
    #[test]
    fn write_refuses_data_that_breaks_an_invariant_and_asks_the_store_nothing() {
        let held = opened();
        let mut store = Counting::default();
        store.inner = crate::store::InMemoryDecisionStore::with_requests(vec![held.clone()]);
        let mut data = held.data.clone();
        data.default_on_silence = true;
        data.recommendation_strength = Strength::Weak;
        assert!(data.broken_invariant().is_some(), "the data breaks one");

        let refused = write(
            &mut store,
            "loom.decision.Recommend",
            Some(&held),
            DecisionRequestState::Open,
            data,
        )
        .expect_err("refused");
        assert!(
            matches!(&refused, DecisionError::Undecided(undecided) if undecided.reason.contains("invariant")),
            "{refused:?}"
        );
        assert_eq!(store.writes, 0, "the store was asked for no write");
        assert_eq!(
            store.inner.list().expect("reads"),
            vec![held],
            "nothing is written"
        );
    }
}
