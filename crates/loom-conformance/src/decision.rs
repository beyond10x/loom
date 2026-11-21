//! The `loom.decision` domain, answered by `b10x-loom-decision`.
//!
//! The eight commands are applied by `b10x_loom_decision` to the scenario's
//! `InMemoryDecisionStore`, and the three views are its view functions over that store. This
//! module decodes the suite's input into the generated types, publishes the event each outcome
//! carries and encodes each answer back; it decides no outcome. A command the crate answers with
//! a `DecisionError` (no outcome decided, or a store refusal) is reported as reaching no declared
//! outcome. An absent `Optional` member is encoded as null.

use std::collections::BTreeMap;

use b10x_loom_decision::model::{
    ActionId, ArtifactDecisionHistory, ArtifactId, ArtifactRevisionCitation, AutonomyLevel,
    CapabilityId, CaseId, CheckVerdict, Citation, Consequence, Decide, DecideOutcome,
    DecisionClassId, DecisionOption, DecisionRequestId, DecisionRequestState, DecisionRequests,
    Escalate, EscalateOutcome, Expire, ExpireOutcome, Grant, Ground, GroundSource, Holder,
    OpenDecisionsForPrincipal, OptionLetter, OutcomeId, PathLineCitation, Principal, PrincipalKind,
    ProducedDocument, ProducedRecord, ProposerKind, Recommend, RecommendOutcome, RecordCitation,
    RecordProduced, RecordProducedOutcome, RecordStore, Request, RequestOutcome, Revision,
    Strength, Supersede, SupersedeOutcome, TakeRecommendation, TakeRecommendationOutcome,
    UnblockKind, Unblocks,
};
use b10x_loom_decision::{InMemoryDecisionStore, Timestamp};
use ess_conformance::target::{ObservedEvent, SemanticCommandResult, ViewRow};
use ess_primitives::ids::CorrelationId;
use ess_primitives::node::Node;

use crate::codec::{self, Input};
use crate::{Live, error, event, took};

pub(crate) const REQUEST: &str = "loom.decision.Request";
pub(crate) const RECOMMEND: &str = "loom.decision.Recommend";
pub(crate) const DECIDE: &str = "loom.decision.Decide";
pub(crate) const TAKE_RECOMMENDATION: &str = "loom.decision.TakeRecommendation";
pub(crate) const RECORD_PRODUCED: &str = "loom.decision.RecordProduced";
pub(crate) const ESCALATE: &str = "loom.decision.Escalate";
pub(crate) const SUPERSEDE: &str = "loom.decision.Supersede";
pub(crate) const EXPIRE: &str = "loom.decision.Expire";
pub(crate) const OPEN_DECISIONS_FOR_PRINCIPAL: &str = "loom.decision.OpenDecisionsForPrincipal";
pub(crate) const ARTIFACT_DECISION_HISTORY: &str = "loom.decision.ArtifactDecisionHistory";
pub(crate) const DECISION_REQUESTS: &str = "loom.decision.DecisionRequests";

const DECISION_REQUESTED: &str = "loom.decision.DecisionRequested";
const RECOMMENDATION_RECORDED: &str = "loom.decision.RecommendationRecorded";
const RECOMMENDATION_WITHHELD: &str = "loom.decision.RecommendationWithheld";
const DECISION_TAKEN: &str = "loom.decision.DecisionTaken";
const RECOMMENDATION_TAKEN: &str = "loom.decision.RecommendationTaken";
const PRODUCED_RECORD_RECORDED: &str = "loom.decision.ProducedRecordRecorded";
const DECISION_ESCALATED: &str = "loom.decision.DecisionEscalated";
const DECISION_SUPERSEDED: &str = "loom.decision.DecisionSuperseded";
const DECISION_EXPIRED: &str = "loom.decision.DecisionExpired";

const TOO_FEW_OPTIONS: &str = "loom.decision.TooFewOptions";
const NOT_A_REQUEST_CLASS: &str = "loom.decision.NotARequestClass";
const RECORD_KEY_OPEN: &str = "loom.decision.RecordKeyOpen";
const UNGROUNDED_RECOMMENDATION: &str = "loom.decision.UngroundedRecommendation";
const OPTION_OR_ANSWER: &str = "loom.decision.OptionOrAnswer";
const ANSWER_NOT_ALLOWED: &str = "loom.decision.AnswerNotAllowed";
const NOT_ELIGIBLE: &str = "loom.decision.NotEligible";
const NO_DEFAULT_ON_SILENCE: &str = "loom.decision.NoDefaultOnSilence";
const SILENCE_HAS_DEFAULT: &str = "loom.decision.SilenceHasDefault";
const NOT_DUE: &str = "loom.decision.NotDue";
const ESCALATION_SUPPRESSED: &str = "loom.decision.EscalationSuppressed";
const PRODUCED_RECORD_CONFLICT: &str = "loom.decision.ProducedRecordConflict";
const NO_RECORD_TO_PRODUCE: &str = "loom.decision.NoRecordToProduce";
const DECISION_STATE_CONFLICT: &str = "loom.decision.DecisionStateConflict";
const NO_SUCH_REQUEST: &str = "loom.decision.NoSuchRequest";

/// A closed set of declared names, both ways.
macro_rules! names {
    ($decode:ident, $encode:ident, $ty:ty, { $($name:literal => $variant:expr),* $(,)? }) => {
        fn $decode(text: &str) -> Option<$ty> {
            match text {
                $($name => Some($variant),)*
                _ => None,
            }
        }

        fn $encode(value: $ty) -> Node {
            codec::text_node(match value {
                $(v if v == $variant => $name,)*
                _ => unreachable!("every variant is named"),
            })
        }
    };
}

names!(unblock_kind, unblock_kind_node, UnblockKind, {
    "approval" => UnblockKind::Approval,
    "question" => UnblockKind::Question,
    "outcome" => UnblockKind::Outcome,
});
names!(grant, grant_node, Grant, {
    "granted" => Grant::Granted,
    "denied" => Grant::Denied,
});
names!(produced_document, produced_document_node, ProducedDocument, {
    "canon_authority" => ProducedDocument::CanonAuthority,
    "canon_decisions" => ProducedDocument::CanonDecisions,
    "canon_evidence" => ProducedDocument::CanonEvidence,
    "artifact_revision" => ProducedDocument::ArtifactRevision,
});
names!(strength, strength_node, Strength, {
    "none" => Strength::None,
    "weak" => Strength::Weak,
    "moderate" => Strength::Moderate,
    "strong" => Strength::Strong,
});
names!(ground_source, ground_source_node, GroundSource, {
    "idea" => GroundSource::Idea,
    "vision" => GroundSource::Vision,
    "requirement" => GroundSource::Requirement,
    "prior_decision" => GroundSource::PriorDecision,
    "prd" => GroundSource::Prd,
    "story" => GroundSource::Story,
    "spec" => GroundSource::Spec,
    "policy" => GroundSource::Policy,
    "evidence_record" => GroundSource::EvidenceRecord,
    "autonomy_rule" => GroundSource::AutonomyRule,
    "goals" => GroundSource::Goals,
});
names!(record_store, record_store_node, RecordStore, {
    "sdc_decisions" => RecordStore::SdcDecisions,
    "canon_evidence" => RecordStore::CanonEvidence,
    "canon_authority" => RecordStore::CanonAuthority,
    "canon_decisions" => RecordStore::CanonDecisions,
    "aep_artifact" => RecordStore::AepArtifact,
});
names!(autonomy_level, autonomy_level_node, AutonomyLevel, {
    "A0" => AutonomyLevel::A0,
    "A1" => AutonomyLevel::A1,
    "A2" => AutonomyLevel::A2,
    "A3" => AutonomyLevel::A3,
    "A4" => AutonomyLevel::A4,
    "A5" => AutonomyLevel::A5,
});
names!(holder, holder_node, Holder, {
    "person" => Holder::Person,
    "advised" => Holder::Advised,
    "decider" => Holder::Decider,
    "decider_final" => Holder::DeciderFinal,
    "rule" => Holder::Rule,
    "rule_final" => Holder::RuleFinal,
    "rule_and_decider" => Holder::RuleAndDecider,
    "rule_and_decider_final" => Holder::RuleAndDeciderFinal,
});
names!(principal_kind, principal_kind_node, PrincipalKind, {
    "person" => PrincipalKind::Person,
    "decider" => PrincipalKind::Decider,
    "rule" => PrincipalKind::Rule,
    "silence" => PrincipalKind::Silence,
});
names!(proposer_kind, proposer_kind_node, ProposerKind, {
    "rule" => ProposerKind::Rule,
    "llm" => ProposerKind::Llm,
    "person" => ProposerKind::Person,
    "prior_decision" => ProposerKind::PriorDecision,
});
names!(check_verdict, _check_verdict_node, CheckVerdict, {
    "verified" => CheckVerdict::Verified,
    "refused" => CheckVerdict::Refused,
});
names!(_state, state_node, DecisionRequestState, {
    "Open" => DecisionRequestState::Open,
    "Escalated" => DecisionRequestState::Escalated,
    "Decided" => DecisionRequestState::Decided,
    "Superseded" => DecisionRequestState::Superseded,
    "Expired" => DecisionRequestState::Expired,
});

type Members = BTreeMap<String, Node>;

/// The text member `name`.
fn text(members: &Members, name: &str) -> Option<String> {
    members.get(name)?.as_text().map(ToOwned::to_owned)
}

/// The `Optional<String>` member `name` (or of a newtype of one).
fn optional_text(members: &Members, name: &str) -> Option<Option<String>> {
    match members.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Text(text)) => Some(Some(text.clone())),
        Some(_) => None,
    }
}

/// The `List<String>` member `name` (or of a newtype of `String`).
fn texts(members: &Members, name: &str) -> Option<Vec<String>> {
    members
        .get(name)?
        .as_seq()?
        .iter()
        .map(|item| item.as_text().map(ToOwned::to_owned))
        .collect()
}

/// The enum member `name`, by `decode`.
fn named<T>(members: &Members, name: &str, decode: fn(&str) -> Option<T>) -> Option<T> {
    decode(members.get(name)?.as_text()?)
}

/// The `Optional` enum member `name`, by `decode`.
fn optional_named<T>(
    members: &Members,
    name: &str,
    decode: fn(&str) -> Option<T>,
) -> Option<Option<T>> {
    match members.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(node) => decode(node.as_text()?).map(Some),
    }
}

fn optional_integer(members: &Members, name: &str) -> Option<Option<i64>> {
    match members.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Number(number)) => number.as_i64().map(Some),
        Some(_) => None,
    }
}

fn map<'a>(members: &'a Members, name: &str) -> Option<&'a Members> {
    members.get(name)?.as_map()
}

fn request_id(input: &Input) -> Option<DecisionRequestId> {
    text(input, "request_id").map(DecisionRequestId)
}

fn principal(input: &Input, name: &str) -> Option<Principal> {
    text(input, name).map(Principal)
}

fn instant(input: &Input, name: &str) -> Option<Timestamp> {
    text(input, name).map(Timestamp)
}

fn letter(members: &Members, name: &str) -> Option<Option<OptionLetter>> {
    optional_text(members, name).map(|text| text.map(OptionLetter))
}

fn unblocks(members: &Members) -> Option<Unblocks> {
    Some(Unblocks {
        kind: named(members, "kind", unblock_kind)?,
        action: ActionId(text(members, "action")?),
        capability: optional_text(members, "capability")?.map(CapabilityId),
        question_id: optional_text(members, "question_id")?,
        outcome: optional_text(members, "outcome")?.map(OutcomeId),
    })
}

fn consequence(members: &Members) -> Option<Consequence> {
    Some(Consequence {
        admits: texts(members, "admits")?
            .into_iter()
            .map(ActionId)
            .collect(),
        blocks: texts(members, "blocks")?
            .into_iter()
            .map(ActionId)
            .collect(),
        grant: optional_named(members, "grant", grant)?,
        answer: optional_text(members, "answer")?,
        produces: optional_named(members, "produces", produced_document)?,
    })
}

fn options(input: &Input) -> Option<Vec<DecisionOption>> {
    input
        .get("options")?
        .as_seq()?
        .iter()
        .map(|item| {
            let members = item.as_map()?;
            Some(DecisionOption {
                letter: OptionLetter(text(members, "letter")?),
                does: text(members, "does")?,
                cost: text(members, "cost")?,
                consequence: consequence(map(members, "consequence")?)?,
            })
        })
        .collect()
}

fn citation(members: &Members) -> Option<Citation> {
    let value = map(members, "value")?;
    Some(match text(members, "form")?.as_str() {
        "path_line" => Citation::PathLine(PathLineCitation {
            path: text(value, "path")?,
            line: optional_integer(value, "line")??,
            line_end: optional_integer(value, "line_end")?,
            digest: optional_text(value, "digest")?,
        }),
        "record_id" => Citation::RecordId(RecordCitation {
            store: named(value, "store", record_store)?,
            record_id: text(value, "record_id")?,
        }),
        "artifact_revision" => Citation::ArtifactRevision(ArtifactRevisionCitation {
            artifact: ArtifactId(text(value, "artifact")?),
            revision: Revision(text(value, "revision")?),
            case: optional_text(value, "case")?.map(CaseId),
            entry: optional_text(value, "entry")?,
            digest: optional_text(value, "digest")?,
        }),
        _ => return None,
    })
}

fn grounds(input: &Input) -> Option<Vec<Ground>> {
    input
        .get("grounds")?
        .as_seq()?
        .iter()
        .map(|item| {
            let members = item.as_map()?;
            Some(Ground {
                source: named(members, "source", ground_source)?,
                citation: citation(map(members, "citation")?)?,
                supports: letter(members, "supports")?,
                how: text(members, "how")?,
            })
        })
        .collect()
}

fn produced(members: &Members) -> Option<ProducedRecord> {
    Some(ProducedRecord {
        document: named(members, "document", produced_document)?,
        record: text(members, "record")?,
        case_revision: Revision(text(members, "case_revision")?),
        aep_record: optional_text(members, "aep_record")?,
    })
}

fn optional<T>(value: Option<&T>, encode: impl FnOnce(&T) -> Node) -> Node {
    value.map_or(Node::Null, encode)
}

fn text_of(text: &str) -> Node {
    codec::text_node(text)
}

fn texts_node<'a>(texts: impl IntoIterator<Item = &'a str>) -> Node {
    Node::Seq(texts.into_iter().map(text_of).collect())
}

fn object<const N: usize>(members: [(&str, Node); N]) -> Node {
    Node::Map(
        members
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn unblocks_node(unblocks: &Unblocks) -> Node {
    object([
        ("kind", unblock_kind_node(unblocks.kind)),
        ("action", text_of(&unblocks.action.0)),
        (
            "capability",
            optional(unblocks.capability.as_ref(), |c| text_of(&c.0)),
        ),
        (
            "question_id",
            optional(unblocks.question_id.as_ref(), |q| text_of(q)),
        ),
        (
            "outcome",
            optional(unblocks.outcome.as_ref(), |o| text_of(&o.0)),
        ),
    ])
}

fn options_node(options: &[DecisionOption]) -> Node {
    Node::Seq(
        options
            .iter()
            .map(|option| {
                let consequence = &option.consequence;
                object([
                    ("letter", text_of(&option.letter.0)),
                    ("does", text_of(&option.does)),
                    ("cost", text_of(&option.cost)),
                    (
                        "consequence",
                        object([
                            (
                                "admits",
                                texts_node(consequence.admits.iter().map(|a| a.0.as_str())),
                            ),
                            (
                                "blocks",
                                texts_node(consequence.blocks.iter().map(|a| a.0.as_str())),
                            ),
                            (
                                "grant",
                                optional(consequence.grant.as_ref(), |g| grant_node(*g)),
                            ),
                            (
                                "answer",
                                optional(consequence.answer.as_ref(), |a| text_of(a)),
                            ),
                            (
                                "produces",
                                optional(consequence.produces.as_ref(), |p| {
                                    produced_document_node(*p)
                                }),
                            ),
                        ]),
                    ),
                ])
            })
            .collect(),
    )
}

fn citation_node(citation: &Citation) -> Node {
    let (form, value) = match citation {
        Citation::PathLine(cited) => (
            "path_line",
            object([
                ("path", text_of(&cited.path)),
                ("line", codec::number(cited.line)),
                (
                    "line_end",
                    optional(cited.line_end.as_ref(), |l| codec::number(*l)),
                ),
                ("digest", optional(cited.digest.as_ref(), |d| text_of(d))),
            ]),
        ),
        Citation::RecordId(cited) => (
            "record_id",
            object([
                ("store", record_store_node(cited.store)),
                ("record_id", text_of(&cited.record_id)),
            ]),
        ),
        Citation::ArtifactRevision(cited) => (
            "artifact_revision",
            object([
                ("artifact", text_of(&cited.artifact.0)),
                ("revision", text_of(&cited.revision.0)),
                ("case", optional(cited.case.as_ref(), |c| text_of(&c.0))),
                ("entry", optional(cited.entry.as_ref(), |e| text_of(e))),
                ("digest", optional(cited.digest.as_ref(), |d| text_of(d))),
            ]),
        ),
    };
    object([("form", text_of(form)), ("value", value)])
}

fn grounds_node(grounds: &[Ground]) -> Node {
    Node::Seq(
        grounds
            .iter()
            .map(|ground| {
                object([
                    ("source", ground_source_node(ground.source)),
                    ("citation", citation_node(&ground.citation)),
                    (
                        "supports",
                        optional(ground.supports.as_ref(), |s| text_of(&s.0)),
                    ),
                    ("how", text_of(&ground.how)),
                ])
            })
            .collect(),
    )
}

/// A produced record as the suite writes it whole: `aep_record` is left out when absent, as an
/// authored scenario's `produced: {document, record, case_revision}` leaves it out.
fn produced_node(produced: &ProducedRecord) -> Node {
    let mut node = object([
        ("document", produced_document_node(produced.document)),
        ("record", text_of(&produced.record)),
        ("case_revision", text_of(&produced.case_revision.0)),
    ]);
    if let (Node::Map(members), Some(aep_record)) = (&mut node, produced.aep_record.as_ref()) {
        members.insert("aep_record".to_owned(), text_of(aep_record));
    }
    node
}

fn id_node(id: &DecisionRequestId) -> Node {
    text_of(&id.0)
}

fn letter_node(letter: Option<&OptionLetter>) -> Node {
    optional(letter, |l| text_of(&l.0))
}

fn answer_node(answer: Option<&String>) -> Node {
    optional(answer, |a| text_of(a))
}

fn instant_node(at: Option<&Timestamp>) -> Node {
    optional(at, |t| text_of(&t.0))
}

/// `outcome` of `command`, refused with the error `name` naming the request.
fn refused(
    command: &str,
    outcome: &str,
    name: &str,
    id: &DecisionRequestId,
) -> SemanticCommandResult {
    took(command, outcome).with_error(error(name).with("request_id", id_node(id)))
}

/// `outcome` of `command`, refused `NotEligible` naming the request and the principal.
fn not_eligible(
    command: &str,
    outcome: &str,
    id: &DecisionRequestId,
    who: &Principal,
) -> SemanticCommandResult {
    took(command, outcome).with_error(
        error(NOT_ELIGIBLE)
            .with("request_id", id_node(id))
            .with("principal", text_of(&who.0)),
    )
}

fn store(live: &mut Live) -> &mut InMemoryDecisionStore {
    &mut live.decisions
}

fn accepted(
    live: &mut Live,
    command: &str,
    outcome: &str,
    published: ObservedEvent,
    correlation: &CorrelationId,
) -> SemanticCommandResult {
    let published = live.publish(published, correlation);
    took(command, outcome).emitting(published)
}

pub(crate) fn request(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Request {
        request_id: request_id(input)?,
        case_id: CaseId(text(input, "case_id")?),
        question_key: text(input, "question_key")?,
        record_key: text(input, "record_key")?,
        open_for_record: optional_text(input, "open_for_record")?.map(DecisionRequestId),
        unblocks: unblocks(map(input, "unblocks")?)?,
        subject_artifact: ArtifactId(text(input, "subject_artifact")?),
        subject_revision: Revision(text(input, "subject_revision")?),
        subject_path: optional_text(input, "subject_path")?,
        question: text(input, "question")?,
        options: options(input)?,
        autonomy_level: named(input, "autonomy_level", autonomy_level)?,
        decision_class: DecisionClassId(text(input, "decision_class")?),
        holder: named(input, "holder", holder)?,
        eligible: texts(input, "eligible")?
            .into_iter()
            .map(Principal)
            .collect(),
        addressed_to: principal(input, "addressed_to")?,
        opened_at: instant(input, "opened_at")?,
        expires_at: optional_text(input, "expires_at")?.map(Timestamp),
        supersedes: optional_text(input, "supersedes")?.map(DecisionRequestId),
    };
    Some(
        match b10x_loom_decision::request(store(live), command).ok()? {
            RequestOutcome::EscalationIsNotAClass { error } => refused(
                REQUEST,
                "escalation-is-not-a-class",
                NOT_A_REQUEST_CLASS,
                &error.request_id,
            ),
            RequestOutcome::TooFewOptions { error } => refused(
                REQUEST,
                "too-few-options",
                TOO_FEW_OPTIONS,
                &error.request_id,
            ),
            RequestOutcome::RecordKeyOpen { error: open } => took(REQUEST, "record-key-open")
                .with_error(
                    error(RECORD_KEY_OPEN)
                        .with("request_id", id_node(&open.request_id))
                        .with("open_for_record", id_node(&open.open_for_record)),
                ),
            RequestOutcome::Requested { decision_requested } => accepted(
                live,
                REQUEST,
                "requested",
                event(DECISION_REQUESTED)
                    .with("request_id", id_node(&decision_requested.request_id))
                    .with("case_id", text_of(&decision_requested.case_id.0))
                    .with(
                        "subject_artifact",
                        text_of(&decision_requested.subject_artifact.0),
                    )
                    .with(
                        "subject_revision",
                        text_of(&decision_requested.subject_revision.0),
                    ),
                correlation,
            ),
        },
    )
}

pub(crate) fn recommend(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Recommend {
        request_id: request_id(input)?,
        recommended_option: letter(input, "recommended_option")?,
        recommended_answer: optional_text(input, "recommended_answer")?,
        strength: named(input, "strength", strength)?,
        default_on_silence: codec::boolean(input, "default_on_silence")?,
        grounds: grounds(input)?,
        proposed_by: named(input, "proposed_by", proposer_kind)?,
        check_verdict: named(input, "check_verdict", check_verdict)?,
        check_report: text(input, "check_report")?,
    };
    Some(
        match b10x_loom_decision::recommend(store(live), command).ok()? {
            RecommendOutcome::Ungrounded { error } => refused(
                RECOMMEND,
                "ungrounded",
                UNGROUNDED_RECOMMENDATION,
                &error.request_id,
            ),
            RecommendOutcome::SilenceNeedsStrong { error } => refused(
                RECOMMEND,
                "silence-needs-strong",
                NO_DEFAULT_ON_SILENCE,
                &error.request_id,
            ),
            RecommendOutcome::OptionOrAnswer { error } => refused(
                RECOMMEND,
                "option-or-answer",
                OPTION_OR_ANSWER,
                &error.request_id,
            ),
            RecommendOutcome::NoRequest { error } => {
                refused(RECOMMEND, "no-request", NO_SUCH_REQUEST, &error.request_id)
            }
            RecommendOutcome::AnswerNotAllowed { error } => refused(
                RECOMMEND,
                "answer-not-allowed",
                ANSWER_NOT_ALLOWED,
                &error.request_id,
            ),
            RecommendOutcome::WrongState { error } => refused(
                RECOMMEND,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            RecommendOutcome::Withheld {
                recommendation_withheld: withheld,
            } => accepted(
                live,
                RECOMMEND,
                "withheld",
                event(RECOMMENDATION_WITHHELD)
                    .with("request_id", id_node(&withheld.request_id))
                    .with("reason", text_of(&withheld.reason)),
                correlation,
            ),
            RecommendOutcome::Recommended {
                recommendation_recorded: recorded,
            } => accepted(
                live,
                RECOMMEND,
                "recommended",
                event(RECOMMENDATION_RECORDED)
                    .with("request_id", id_node(&recorded.request_id))
                    .with(
                        "recommended_option",
                        letter_node(recorded.recommended_option.as_ref()),
                    )
                    .with(
                        "recommended_answer",
                        answer_node(recorded.recommended_answer.as_ref()),
                    )
                    .with("strength", strength_node(recorded.strength)),
                correlation,
            ),
        },
    )
}

pub(crate) fn decide(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Decide {
        request_id: request_id(input)?,
        option: letter(input, "option")?,
        answer: optional_text(input, "answer")?,
        principal: principal(input, "principal")?,
        principal_kind: named(input, "principal_kind", principal_kind)?,
        decided_at: instant(input, "decided_at")?,
        case_revision: Revision(text(input, "case_revision")?),
        reasons: texts(input, "reasons")?,
    };
    Some(
        match b10x_loom_decision::decide(store(live), command).ok()? {
            DecideOutcome::KindNotPrincipal { error } => not_eligible(
                DECIDE,
                "kind-not-principal",
                &error.request_id,
                &error.principal,
            ),
            DecideOutcome::SilenceTakesRecommendation { error } => not_eligible(
                DECIDE,
                "silence-takes-recommendation",
                &error.request_id,
                &error.principal,
            ),
            DecideOutcome::OptionOrAnswer { error } => refused(
                DECIDE,
                "option-or-answer",
                OPTION_OR_ANSWER,
                &error.request_id,
            ),
            DecideOutcome::NoRequest { error } => {
                refused(DECIDE, "no-request", NO_SUCH_REQUEST, &error.request_id)
            }
            DecideOutcome::AnswerNotAllowed { error } => refused(
                DECIDE,
                "answer-not-allowed",
                ANSWER_NOT_ALLOWED,
                &error.request_id,
            ),
            DecideOutcome::PersonOnly { error } => {
                not_eligible(DECIDE, "person-only", &error.request_id, &error.principal)
            }
            DecideOutcome::NotListed { error } => {
                not_eligible(DECIDE, "not-listed", &error.request_id, &error.principal)
            }
            DecideOutcome::DeciderUnderRule { error } => not_eligible(
                DECIDE,
                "decider-under-rule",
                &error.request_id,
                &error.principal,
            ),
            DecideOutcome::RuleNotHolder { error } => not_eligible(
                DECIDE,
                "rule-not-holder",
                &error.request_id,
                &error.principal,
            ),
            DecideOutcome::WrongState { error } => refused(
                DECIDE,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            DecideOutcome::Decided { decision_taken } => accepted(
                live,
                DECIDE,
                "decided",
                event(DECISION_TAKEN)
                    .with("request_id", id_node(&decision_taken.request_id))
                    .with("option", letter_node(decision_taken.option.as_ref()))
                    .with("answer", answer_node(decision_taken.answer.as_ref()))
                    .with("principal", text_of(&decision_taken.principal.0)),
                correlation,
            ),
        },
    )
}

pub(crate) fn take_recommendation(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = TakeRecommendation {
        request_id: request_id(input)?,
        principal: principal(input, "principal")?,
        principal_kind: named(input, "principal_kind", principal_kind)?,
        decided_at: instant(input, "decided_at")?,
        case_revision: Revision(text(input, "case_revision")?),
        reasons: texts(input, "reasons")?,
    };
    let command_name = TAKE_RECOMMENDATION;
    Some(
        match b10x_loom_decision::take_recommendation(store(live), command).ok()? {
            TakeRecommendationOutcome::KindNotPrincipal { error } => not_eligible(
                command_name,
                "kind-not-principal",
                &error.request_id,
                &error.principal,
            ),
            TakeRecommendationOutcome::NoRequest { error } => refused(
                command_name,
                "no-request",
                NO_SUCH_REQUEST,
                &error.request_id,
            ),
            TakeRecommendationOutcome::PersonOnly { error } => not_eligible(
                command_name,
                "person-only",
                &error.request_id,
                &error.principal,
            ),
            TakeRecommendationOutcome::NotListed { error } => not_eligible(
                command_name,
                "not-listed",
                &error.request_id,
                &error.principal,
            ),
            TakeRecommendationOutcome::NotStrong { error } => not_eligible(
                command_name,
                "not-strong",
                &error.request_id,
                &error.principal,
            ),
            TakeRecommendationOutcome::SilenceWithoutDefault { error } => refused(
                command_name,
                "silence-without-default",
                NO_DEFAULT_ON_SILENCE,
                &error.request_id,
            ),
            TakeRecommendationOutcome::SilenceNotDue { error } => {
                refused(command_name, "silence-not-due", NOT_DUE, &error.request_id)
            }
            TakeRecommendationOutcome::WrongState { error } => refused(
                command_name,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            TakeRecommendationOutcome::Taken {
                recommendation_taken: taken,
            } => accepted(
                live,
                command_name,
                "taken",
                event(RECOMMENDATION_TAKEN)
                    .with("request_id", id_node(&taken.request_id))
                    .with("principal", text_of(&taken.principal.0)),
                correlation,
            ),
        },
    )
}

pub(crate) fn record_produced(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = RecordProduced {
        request_id: request_id(input)?,
        produced: produced(map(input, "produced")?)?,
    };
    Some(
        match b10x_loom_decision::record_produced(store(live), command).ok()? {
            RecordProducedOutcome::NoRequest { error } => refused(
                RECORD_PRODUCED,
                "no-request",
                NO_SUCH_REQUEST,
                &error.request_id,
            ),
            RecordProducedOutcome::AlreadyRecorded => took(RECORD_PRODUCED, "already-recorded"),
            RecordProducedOutcome::ProducedConflict { error } => refused(
                RECORD_PRODUCED,
                "produced-conflict",
                PRODUCED_RECORD_CONFLICT,
                &error.request_id,
            ),
            RecordProducedOutcome::OptionProducesNone { error } => refused(
                RECORD_PRODUCED,
                "option-produces-none",
                NO_RECORD_TO_PRODUCE,
                &error.request_id,
            ),
            RecordProducedOutcome::WrongState { error } => refused(
                RECORD_PRODUCED,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            RecordProducedOutcome::Recorded {
                produced_record_recorded: recorded,
            } => accepted(
                live,
                RECORD_PRODUCED,
                "recorded",
                event(PRODUCED_RECORD_RECORDED)
                    .with("request_id", id_node(&recorded.request_id))
                    .with("produced", produced_node(&recorded.produced)),
                correlation,
            ),
        },
    )
}

pub(crate) fn escalate(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Escalate {
        request_id: request_id(input)?,
        escalate_to: principal(input, "escalate_to")?,
        reason: text(input, "reason")?,
    };
    Some(
        match b10x_loom_decision::escalate(store(live), command).ok()? {
            EscalateOutcome::NoRequest { error } => {
                refused(ESCALATE, "no-request", NO_SUCH_REQUEST, &error.request_id)
            }
            EscalateOutcome::Suppressed { error } => refused(
                ESCALATE,
                "suppressed",
                ESCALATION_SUPPRESSED,
                &error.request_id,
            ),
            EscalateOutcome::WrongState { error } => refused(
                ESCALATE,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            EscalateOutcome::Escalated {
                decision_escalated: escalated,
            } => accepted(
                live,
                ESCALATE,
                "escalated",
                event(DECISION_ESCALATED)
                    .with("request_id", id_node(&escalated.request_id))
                    .with("escalate_to", text_of(&escalated.escalate_to.0)),
                correlation,
            ),
        },
    )
}

pub(crate) fn supersede(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Supersede {
        request_id: request_id(input)?,
        superseded_by: text(input, "superseded_by").map(DecisionRequestId)?,
        reason: text(input, "reason")?,
    };
    Some(
        match b10x_loom_decision::supersede(store(live), command).ok()? {
            SupersedeOutcome::NoRequest { error } => {
                refused(SUPERSEDE, "no-request", NO_SUCH_REQUEST, &error.request_id)
            }
            SupersedeOutcome::WrongState { error } => refused(
                SUPERSEDE,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            SupersedeOutcome::Superseded {
                decision_superseded: superseded,
            } => accepted(
                live,
                SUPERSEDE,
                "superseded",
                event(DECISION_SUPERSEDED)
                    .with("request_id", id_node(&superseded.request_id))
                    .with("superseded_by", id_node(&superseded.superseded_by)),
                correlation,
            ),
        },
    )
}

pub(crate) fn expire(
    live: &mut Live,
    input: &Input,
    correlation: &CorrelationId,
) -> Option<SemanticCommandResult> {
    let command = Expire {
        request_id: request_id(input)?,
        at: instant(input, "at")?,
    };
    Some(
        match b10x_loom_decision::expire(store(live), command).ok()? {
            ExpireOutcome::NoRequest { error } => {
                refused(EXPIRE, "no-request", NO_SUCH_REQUEST, &error.request_id)
            }
            ExpireOutcome::NotDue { error } => {
                refused(EXPIRE, "not-due", NOT_DUE, &error.request_id)
            }
            ExpireOutcome::HasDefault { error } => refused(
                EXPIRE,
                "has-default",
                SILENCE_HAS_DEFAULT,
                &error.request_id,
            ),
            ExpireOutcome::WrongState { error } => refused(
                EXPIRE,
                "wrong-state",
                DECISION_STATE_CONFLICT,
                &error.request_id,
            ),
            ExpireOutcome::Expired { decision_expired } => accepted(
                live,
                EXPIRE,
                "expired",
                event(DECISION_EXPIRED).with("request_id", id_node(&decision_expired.request_id)),
                correlation,
            ),
        },
    )
}

fn row<const N: usize>(members: [(&str, Node); N]) -> ViewRow {
    members
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

fn open_row(held: &OpenDecisionsForPrincipal) -> ViewRow {
    row([
        ("request_id", id_node(&held.request_id)),
        ("case_id", text_of(&held.case_id.0)),
        ("question", text_of(&held.question)),
        ("subject_artifact", text_of(&held.subject_artifact.0)),
        ("subject_revision", text_of(&held.subject_revision.0)),
        (
            "recommended_option",
            letter_node(held.recommended_option.as_ref()),
        ),
        (
            "recommended_answer",
            answer_node(held.recommended_answer.as_ref()),
        ),
        (
            "recommendation_strength",
            strength_node(held.recommendation_strength),
        ),
        (
            "no_recommendation_reason",
            answer_node(held.no_recommendation_reason.as_ref()),
        ),
        ("opened_at", instant_node(Some(&held.opened_at))),
        ("expires_at", instant_node(held.expires_at.as_ref())),
    ])
}

fn history_row(held: &ArtifactDecisionHistory) -> ViewRow {
    row([
        ("request_id", id_node(&held.request_id)),
        ("subject_revision", text_of(&held.subject_revision.0)),
        ("question", text_of(&held.question)),
        ("state", state_node(held.state)),
        ("decided_option", letter_node(held.decided_option.as_ref())),
        ("decided_answer", answer_node(held.decided_answer.as_ref())),
        (
            "decided_by",
            optional(held.decided_by.as_ref(), |p| text_of(&p.0)),
        ),
        ("decided_at", instant_node(held.decided_at.as_ref())),
        (
            "produced_record",
            optional(held.produced_record.as_ref(), produced_node),
        ),
        ("supersedes", optional(held.supersedes.as_ref(), id_node)),
        (
            "superseded_by",
            optional(held.superseded_by.as_ref(), id_node),
        ),
    ])
}

fn requests_row(held: &DecisionRequests) -> ViewRow {
    row([
        ("request_id", id_node(&held.request_id)),
        ("state", state_node(held.state)),
        ("case_id", text_of(&held.case_id.0)),
        ("question_key", text_of(&held.question_key)),
        ("record_key", text_of(&held.record_key)),
        ("unblocks", unblocks_node(&held.unblocks)),
        ("subject_artifact", text_of(&held.subject_artifact.0)),
        ("subject_revision", text_of(&held.subject_revision.0)),
        ("subject_path", answer_node(held.subject_path.as_ref())),
        ("question", text_of(&held.question)),
        ("options", options_node(&held.options)),
        ("autonomy_level", autonomy_level_node(held.autonomy_level)),
        ("decision_class", text_of(&held.decision_class.0)),
        ("holder", holder_node(held.holder)),
        (
            "eligible",
            texts_node(held.eligible.iter().map(|p| p.0.as_str())),
        ),
        ("addressed_to", text_of(&held.addressed_to.0)),
        ("opened_at", instant_node(Some(&held.opened_at))),
        ("expires_at", instant_node(held.expires_at.as_ref())),
        ("supersedes", optional(held.supersedes.as_ref(), id_node)),
        (
            "recommended_option",
            letter_node(held.recommended_option.as_ref()),
        ),
        (
            "recommended_answer",
            answer_node(held.recommended_answer.as_ref()),
        ),
        (
            "recommendation_strength",
            strength_node(held.recommendation_strength),
        ),
        ("default_on_silence", Node::Bool(held.default_on_silence)),
        ("grounds", grounds_node(&held.grounds)),
        (
            "recommended_by",
            optional(held.recommended_by.as_ref(), |k| proposer_kind_node(*k)),
        ),
        (
            "grounding_report",
            answer_node(held.grounding_report.as_ref()),
        ),
        (
            "no_recommendation_reason",
            answer_node(held.no_recommendation_reason.as_ref()),
        ),
        (
            "escalated_to",
            optional(held.escalated_to.as_ref(), |p| text_of(&p.0)),
        ),
        (
            "escalation_reason",
            answer_node(held.escalation_reason.as_ref()),
        ),
        ("decided_option", letter_node(held.decided_option.as_ref())),
        ("decided_answer", answer_node(held.decided_answer.as_ref())),
        (
            "decided_by",
            optional(held.decided_by.as_ref(), |p| text_of(&p.0)),
        ),
        (
            "decided_by_kind",
            optional(held.decided_by_kind.as_ref(), |k| principal_kind_node(*k)),
        ),
        ("decided_at", instant_node(held.decided_at.as_ref())),
        (
            "decided_case_revision",
            optional(held.decided_case_revision.as_ref(), |r| text_of(&r.0)),
        ),
        (
            "decision_reasons",
            texts_node(held.decision_reasons.iter().map(String::as_str)),
        ),
        (
            "produced_record",
            optional(held.produced_record.as_ref(), produced_node),
        ),
        (
            "superseded_by",
            optional(held.superseded_by.as_ref(), id_node),
        ),
        (
            "superseded_reason",
            answer_node(held.superseded_reason.as_ref()),
        ),
    ])
}

/// The rows of the view `view` with its `params`, or `None` when a parameter does not decode or
/// the store refuses.
pub(crate) fn rows(live: &Live, view: &str, params: &Input) -> Option<Vec<ViewRow>> {
    let store = &live.decisions;
    Some(match view {
        OPEN_DECISIONS_FOR_PRINCIPAL => b10x_loom_decision::open_decisions_for_principal(
            store,
            &Principal(text(params, "principal")?),
        )
        .ok()?
        .iter()
        .map(open_row)
        .collect(),
        ARTIFACT_DECISION_HISTORY => b10x_loom_decision::artifact_decision_history(
            store,
            &ArtifactId(text(params, "artifact")?),
        )
        .ok()?
        .iter()
        .map(history_row)
        .collect(),
        _ => b10x_loom_decision::decision_requests(store)
            .ok()?
            .iter()
            .map(requests_row)
            .collect(),
    })
}
