// generated from loom v1
// contract digest b6247a62e3f402ec8645beaa98d8866f3e25026be7aad2291f3337f509df33f3
// do not edit: regenerate with `ess synthesize --layout crate`

//! Decisions — `loom.decision`.
//!
//! Decision requests: every approval or decision the SDC asks of a person or a decider, with its options, a recommendation and the grounds it rests on. A decision request is opened where a case stops on a decision (driver verdicts awaiting_authority and awaiting_person, docs/driver.md:119-127), is decided by a principal allowed at its autonomy level, and once decided names the Canon record it produced (canon-authority/1, canon-decisions/1 or canon-evidence/1). docs/decisions.md explains it in plain terms.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// ActionId — `loom.decision.ActionId`: a distinct wrapper around `String`.
///
/// A protocol action id, such as roadmap.approve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionId(pub String);

/// ArtifactId — `loom.decision.ArtifactId`: a distinct wrapper around `String`.
///
/// A case artifact id (canon-case/1 artifacts key), such as roadmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactId(pub String);

/// ArtifactRevisionCitation — `loom.decision.ArtifactRevisionCitation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRevisionCitation {
    /// `artifact` — `loom.decision.ArtifactId`.
    pub artifact: ArtifactId,
    /// `revision` — `loom.decision.Revision`.
    pub revision: Revision,
    /// `case` — `Optional<loom.decision.CaseId>`.
    pub case: Option<CaseId>,
    /// `entry` — `Optional<String>`.
    pub entry: Option<String>,
    /// `digest` — `Optional<String>`.
    pub digest: Option<String>,
}

/// AutonomyLevel — `loom.decision.AutonomyLevel`: one of a closed set of names.
///
/// The run's autonomy level, docs/autonomy-levels.md:36-42 and :205-213: A0 Manual, A1 Advised, A2 Internal, A3 To merge, A4 To release, A5 Unattended (eval runs, :41-42). The level does not decide eligibility by itself: with the request's decision class it gives the holder (:241-256, overrides :297-320), and the guards read the holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonomyLevel {
    /// `A0`.
    A0,
    /// `A1`.
    A1,
    /// `A2`.
    A2,
    /// `A3`.
    A3,
    /// `A4`.
    A4,
    /// `A5`.
    A5,
}

/// CapabilityId — `loom.decision.CapabilityId`: a distinct wrapper around `String`.
///
/// A Canon capability an action requires, such as owner.approve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityId(pub String);

/// CaseId — `loom.decision.CaseId`: a distinct wrapper around `String`.
///
/// A Canon case id (canon-case/1 id), such as IDEA-1.direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseId(pub String);

/// CheckVerdict — `loom.decision.CheckVerdict`: one of a closed set of names.
///
/// The grounding check's verdict on a proposed recommendation: verified when it cites at least one ground, every citation resolves and a ground supports the recommended option or free answer; refused otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckVerdict {
    /// `verified`.
    Verified,
    /// `refused`.
    Refused,
}

/// Citation — `loom.decision.Citation`: one of a fixed set of shapes, tagged on the wire by `form`.
///
/// Where a ground is: path:line, a record id, artifact@revision, or one entry of an artifact (goals@<revision>#<entry id>). A program checks that each resolves (tools/decision-grounds.sh; a goals entry through sdc-goals ground).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Citation {
    /// Tagged `artifact_revision` — `loom.decision.ArtifactRevisionCitation`.
    ArtifactRevision(ArtifactRevisionCitation),
    /// Tagged `path_line` — `loom.decision.PathLineCitation`.
    PathLine(PathLineCitation),
    /// Tagged `record_id` — `loom.decision.RecordCitation`.
    RecordId(RecordCitation),
}

/// Consequence — `loom.decision.Consequence`.
///
/// What choosing the option does to the protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consequence {
    /// `admits` — `List<loom.decision.ActionId>`.
    pub admits: Vec<ActionId>,
    /// `blocks` — `List<loom.decision.ActionId>`.
    pub blocks: Vec<ActionId>,
    /// `grant` — `Optional<loom.decision.Grant>`.
    pub grant: Option<Grant>,
    /// `answer` — `Optional<String>`.
    pub answer: Option<String>,
    /// `produces` — `Optional<loom.decision.ProducedDocument>`.
    pub produces: Option<ProducedDocument>,
}

/// DecisionClassId — `loom.decision.DecisionClassId`: a distinct wrapper around `String`.
///
/// A decision class, an identifier validated against the host's classes document (the keys of sdc.decision-classes/1, docs/autonomy-levels.md:244-257 and :398-413, such as merge or code_review); budget_overrun and limit_hit are Loom built-ins. The domain checks no class beyond escalation, which names the row that gives a request's holder after Escalate; Request refuses it as a class of a new request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionClassId(pub String);

/// DecisionOption — `loom.decision.DecisionOption`.
///
/// One option of a decision request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionOption {
    /// `letter` — `loom.decision.OptionLetter`.
    pub letter: OptionLetter,
    /// `does` — `String`.
    pub does: String,
    /// `cost` — `String`.
    pub cost: String,
    /// `consequence` — `loom.decision.Consequence`.
    pub consequence: Consequence,
}

/// The states of `loom.decision.DecisionRequest`, as runtime values.
///
/// Synthesised from the lifecycle, so the two cannot disagree. Which *moves* are legal is not
/// carried here — it is carried by `DecisionRequest<S>`, where an undeclared move does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionRequestState {
    /// `Decided`.
    Decided,
    /// `Escalated`.
    Escalated,
    /// `Expired`.
    Expired,
    /// `Open`.
    Open,
    /// `Superseded`.
    Superseded,
}

/// DecisionRequestId — `loom.decision.DecisionRequestId`: a distinct wrapper around `String`.
///
/// Id of a decision request, chosen by the driver: DR-<case>-<action or question id>-<n>.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequestId(pub String);

/// EscalationSuppressedRecord — `loom.decision.EscalationSuppressedRecord`.
///
/// The record kept beside a request when an escalation was suppressed (the escalation_suppressed event the SDC names in prose, decision.yaml:186 and :759 at 00e3557): which request, its class and subject at revision, the reasons, the decider's best option if any, the cause, and when it was kept. Named apart from the error EscalationSuppressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationSuppressedRecord {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `decision_class` — `loom.decision.DecisionClassId`.
    pub decision_class: DecisionClassId,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
    /// `best` — `Optional<loom.decision.OptionLetter>`.
    pub best: Option<OptionLetter>,
    /// `cause` — `loom.decision.SuppressionCause`.
    pub cause: SuppressionCause,
    /// `kept_at` — `Timestamp`.
    pub kept_at: crate::primitives::Timestamp,
}

/// Grant — `loom.decision.Grant`: one of a closed set of names.
///
/// A canon-authority/1 decision (documents.md:123-132).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// `granted`.
    Granted,
    /// `denied`.
    Denied,
}

/// Ground — `loom.decision.Ground`.
///
/// One ground of a recommendation: its source kind, a citation that resolves, the option it supports (or the free answer) and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ground {
    /// `source` — `loom.decision.GroundSource`.
    pub source: GroundSource,
    /// `citation` — `loom.decision.Citation`.
    pub citation: Citation,
    /// `supports` — `Optional<loom.decision.OptionLetter>`.
    pub supports: Option<OptionLetter>,
    /// `how` — `String`.
    pub how: String,
}

/// GroundSource — `loom.decision.GroundSource`: one of a closed set of names.
///
/// The kind of source a ground cites. idea: the person's own input and its dissection items (work/idea.yaml, work/dissection.yaml), which the decider cites (d1 to d4 in runs/trial-todo-1/decisions.jsonl). goals: one entry of the requester's goals document (sdc.goals/1, docs/requester.md § 2), cited as artifact_revision of artifact goals with its entry id and the entry's digest, displayed goals@<revision>#<entry id>.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundSource {
    /// `idea`.
    Idea,
    /// `vision`.
    Vision,
    /// `requirement`.
    Requirement,
    /// `prior_decision`.
    PriorDecision,
    /// `prd`.
    Prd,
    /// `story`.
    Story,
    /// `spec`.
    Spec,
    /// `policy`.
    Policy,
    /// `evidence_record`.
    EvidenceRecord,
    /// `autonomy_rule`.
    AutonomyRule,
    /// `goals`.
    Goals,
}

/// Holder — `loom.decision.Holder`: one of a closed set of names.
///
/// Who decides this request, the cell of docs/autonomy-levels.md:242-257 for its class at the run's level (holders :233-239). person: P, a person decides. advised: P+, a person decides with the decider's proposal shown as the recommendation (propose only). decider: D→P, the decider takes any option and an escalate goes to a person. decider_final: D*, an escalate is suppressed and the decider's best is final. rule: R and the cells 'R, then P', a rule decides and an escalate goes to a person. rule_final: the cells 'R: end', a rule decides, an escalate is refused and the run ends. rule_and_decider: R∧D→P, the rule's verdict is recorded as a strong recommendation; the decider takes it (TakeRecommendation) or vetoes, and a veto is an Escalate; it never grants against the rule. rule_and_decider_final: R∧D*, the same, except that a veto is a Decide on the denying option, after which the run ends as refused with no explicitly_abandoned record. Under decider_final and rule_and_decider_final a decider result that is missing, failed or withheld by the grounding check decides the denying option, or ends the run where no option denies, and escalation_suppressed records the cause; a rule verdict withheld under rule_and_decider counts as no recommendation and goes to a person, under rule_and_decider_final the run ends. Override order (docs/autonomy-levels.md § 2.3): person < advised < rule_and_decider < rule, decider (not comparable); each non-final holder < its final variant; rule_and_decider_final < rule_final, decider_final. A move down is a lowering; every other move is a raise and needs the class's admitted ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder {
    /// `person`.
    Person,
    /// `advised`.
    Advised,
    /// `decider`.
    Decider,
    /// `decider_final`.
    DeciderFinal,
    /// `rule`.
    Rule,
    /// `rule_final`.
    RuleFinal,
    /// `rule_and_decider`.
    RuleAndDecider,
    /// `rule_and_decider_final`.
    RuleAndDeciderFinal,
}

/// OptionLetter — `loom.decision.OptionLetter`: a distinct wrapper around `String`.
///
/// The letter of an option: A, B, C, in the order presented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionLetter(pub String);

/// OutcomeId — `loom.decision.OutcomeId`: a distinct wrapper around `String`.
///
/// A Canon outcome id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeId(pub String);

/// PathLineCitation — `loom.decision.PathLineCitation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLineCitation {
    /// `path` — `String`.
    pub path: String,
    /// `line` — `Integer`.
    pub line: i64,
    /// `line_end` — `Optional<Integer>`.
    pub line_end: Option<i64>,
    /// `digest` — `Optional<String>`.
    pub digest: Option<String>,
}

/// Principal — `loom.decision.Principal`: a distinct wrapper around `String`.
///
/// Who decides: a person's name (operator), decider:<kind> (decider:model, decider:haiku as an older alias, decider:approve, decider:reject), rule:<name>, or silence for a default taken because nobody answered. Recorded as given, as Canon records a principal (canon-decisions/1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal(pub String);

/// PrincipalKind — `loom.decision.PrincipalKind`: one of a closed set of names.
///
/// person: a named person. decider: decider:<kind>. rule: rule:<name>, a program deciding from evidence (docs/autonomy-levels.md:202-203). silence: the recommended default taken because nobody answered before expires_at (not in autonomy-levels.md; a person-facing addition, A0 to A4 only in practice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    /// `person`.
    Person,
    /// `decider`.
    Decider,
    /// `rule`.
    Rule,
    /// `silence`.
    Silence,
}

/// ProducedDocument — `loom.decision.ProducedDocument`: one of a closed set of names.
///
/// The Canon record a decided request produced. canon_authority: an AuthorityDecision entry bound to the case revision (approval). canon_decisions: an ExplicitDecision entry (outcome). canon_evidence: the approval evidence a manual action produced (context_approval, vision_approval, roadmap_approval). artifact_revision: the new revision of questions.yaml holding the answer (question).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducedDocument {
    /// `canon_authority`.
    CanonAuthority,
    /// `canon_decisions`.
    CanonDecisions,
    /// `canon_evidence`.
    CanonEvidence,
    /// `artifact_revision`.
    ArtifactRevision,
}

/// ProducedRecord — `loom.decision.ProducedRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedRecord {
    /// `document` — `loom.decision.ProducedDocument`.
    pub document: ProducedDocument,
    /// `record` — `String`.
    pub record: String,
    /// `case_revision` — `loom.decision.Revision`.
    pub case_revision: Revision,
    /// `aep_record` — `Optional<String>`.
    pub aep_record: Option<String>,
}

/// ProposerKind — `loom.decision.ProposerKind`: one of a closed set of names.
///
/// Who proposed the recommendation: a deterministic rule, an LLM (accepted only when its citations pass the grounding check), a person, or the reuse of a prior decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposerKind {
    /// `rule`.
    Rule,
    /// `llm`.
    Llm,
    /// `person`.
    Person,
    /// `prior_decision`.
    PriorDecision,
}

/// RecordCitation — `loom.decision.RecordCitation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCitation {
    /// `store` — `loom.decision.RecordStore`.
    pub store: RecordStore,
    /// `record_id` — `String`.
    pub record_id: String,
}

/// RecordStore — `loom.decision.RecordStore`: one of a closed set of names.
///
/// Where a cited record id is looked up: the decision request log, a case's evidence/ (canon-evidence/1 id), authority.json, decisions.json, or an AEP planning store (kind:name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordStore {
    /// `sdc_decisions`.
    SdcDecisions,
    /// `canon_evidence`.
    CanonEvidence,
    /// `canon_authority`.
    CanonAuthority,
    /// `canon_decisions`.
    CanonDecisions,
    /// `aep_artifact`.
    AepArtifact,
}

/// Revision — `loom.decision.Revision`: a distinct wrapper around `String`.
///
/// An artifact or case revision as the case snapshot writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision(pub String);

/// Strength — `loom.decision.Strength`: one of a closed set of names.
///
/// How strongly the grounds support the recommended option. none: the request carries no recommendation (stored before Recommend and after a withheld one; a Recommend sending none is refused). strong: a vision, requirement, PRD, story or spec states it, or a prior decision on the same question and subject stands with its grounds unchanged. moderate: a policy, an autonomy rule or an evidence record supports it by inference. weak: the grounds make it plausible only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    /// `none`.
    None,
    /// `weak`.
    Weak,
    /// `moderate`.
    Moderate,
    /// `strong`.
    Strong,
}

/// SuppressionCause — `loom.decision.SuppressionCause`: one of a closed set of names.
///
/// Why an escalation was suppressed under a final holder (decider_final, rule_final, rule_and_decider_final). escalate: the decider asked to escalate. failure: the decider's result was missing or failed. withheld: the grounding check withheld the decider's recommendation. veto: the decider vetoed under rule_and_decider_final. rule_escalate: the rule asked to escalate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressionCause {
    /// `escalate`.
    Escalate,
    /// `failure`.
    Failure,
    /// `withheld`.
    Withheld,
    /// `veto`.
    Veto,
    /// `rule_escalate`.
    RuleEscalate,
}

/// UnblockKind — `loom.decision.UnblockKind`: one of a closed set of names.
///
/// What the decision unblocks. approval: an action that is approval-required, decided by an authority grant. question: an interview question a writer hands off (questions.answer handoff, docs/driver.md:112-114). outcome: a case outcome that requires an explicit decision (canon-decisions/1); its request has two options, the decision (produces canon_decisions) and 'leave it untaken' (produces none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnblockKind {
    /// `approval`.
    Approval,
    /// `question`.
    Question,
    /// `outcome`.
    Outcome,
}

/// Unblocks — `loom.decision.Unblocks`.
///
/// The case and the protocol action, question or outcome the decision unblocks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unblocks {
    /// `kind` — `loom.decision.UnblockKind`.
    pub kind: UnblockKind,
    /// `action` — `loom.decision.ActionId`.
    pub action: ActionId,
    /// `capability` — `Optional<loom.decision.CapabilityId>`.
    pub capability: Option<CapabilityId>,
    /// `question_id` — `Optional<String>`.
    pub question_id: Option<String>,
    /// `outcome` — `Optional<loom.decision.OutcomeId>`.
    pub outcome: Option<OutcomeId>,
}

/// What DecisionRequest — `loom.decision.DecisionRequest` — holds, apart from where it is in its lifecycle.
///
/// The identity and every declared field. The state is deliberately not one: inside the domain it
/// is carried by the type parameter of [`DecisionRequest<S>`], and at a boundary by [`DecisionRequestSnapshot::state`].
///
/// Every value satisfies `(not (defined(recommended_option)) or not (defined(no_recommendation_reason)))` — checked by [`DecisionRequestData::broken_invariant`].
/// Every value satisfies `(not (defined(recommended_answer)) or not (defined(no_recommendation_reason)))` — checked by [`DecisionRequestData::broken_invariant`].
/// Every value satisfies `(default_on_silence == false or recommendation_strength == strong)` — checked by [`DecisionRequestData::broken_invariant`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequestData {
    /// The identity: `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: CaseId,
    /// `question_key` — `String`.
    pub question_key: String,
    /// `record_key` — `String`.
    pub record_key: String,
    /// `unblocks` — `loom.decision.Unblocks`.
    pub unblocks: Unblocks,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `subject_path` — `Optional<String>`.
    pub subject_path: Option<String>,
    /// `question` — `String`.
    pub question: String,
    /// `options` — `List<loom.decision.DecisionOption>`.
    pub options: Vec<DecisionOption>,
    /// `autonomy_level` — `loom.decision.AutonomyLevel`.
    pub autonomy_level: AutonomyLevel,
    /// `decision_class` — `loom.decision.DecisionClassId`.
    pub decision_class: DecisionClassId,
    /// `holder` — `loom.decision.Holder`.
    pub holder: Holder,
    /// `eligible` — `List<loom.decision.Principal>`.
    pub eligible: Vec<Principal>,
    /// `addressed_to` — `loom.decision.Principal`.
    pub addressed_to: Principal,
    /// `opened_at` — `Timestamp`.
    pub opened_at: crate::primitives::Timestamp,
    /// `expires_at` — `Optional<Timestamp>`.
    pub expires_at: Option<crate::primitives::Timestamp>,
    /// `supersedes` — `Optional<loom.decision.DecisionRequestId>`.
    ///
    /// Carries `superseded_request`: `loom.decision.DecisionRequest` references one `loom.decision.DecisionRequest`.
    pub supersedes: Option<DecisionRequestId>,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `recommendation_strength` — `loom.decision.Strength`.
    pub recommendation_strength: Strength,
    /// `default_on_silence` — `Boolean`.
    pub default_on_silence: bool,
    /// `grounds` — `List<loom.decision.Ground>`.
    pub grounds: Vec<Ground>,
    /// `recommended_by` — `Optional<loom.decision.ProposerKind>`.
    pub recommended_by: Option<ProposerKind>,
    /// `grounding_report` — `Optional<String>`.
    pub grounding_report: Option<String>,
    /// `no_recommendation_reason` — `Optional<String>`.
    pub no_recommendation_reason: Option<String>,
    /// `escalated_to` — `Optional<loom.decision.Principal>`.
    pub escalated_to: Option<Principal>,
    /// `escalation_reason` — `Optional<String>`.
    pub escalation_reason: Option<String>,
    /// `decided_option` — `Optional<loom.decision.OptionLetter>`.
    pub decided_option: Option<OptionLetter>,
    /// `decided_answer` — `Optional<String>`.
    pub decided_answer: Option<String>,
    /// `decided_by` — `Optional<loom.decision.Principal>`.
    pub decided_by: Option<Principal>,
    /// `decided_by_kind` — `Optional<loom.decision.PrincipalKind>`.
    pub decided_by_kind: Option<PrincipalKind>,
    /// `decided_at` — `Optional<Timestamp>`.
    pub decided_at: Option<crate::primitives::Timestamp>,
    /// `decided_case_revision` — `Optional<loom.decision.Revision>`.
    pub decided_case_revision: Option<Revision>,
    /// `decision_reasons` — `List<String>`.
    pub decision_reasons: Vec<String>,
    /// `produced_record` — `Optional<loom.decision.ProducedRecord>`.
    pub produced_record: Option<ProducedRecord>,
    /// `superseded_by` — `Optional<loom.decision.DecisionRequestId>`.
    ///
    /// Carries `superseding_request`: `loom.decision.DecisionRequest` references one `loom.decision.DecisionRequest`.
    pub superseded_by: Option<DecisionRequestId>,
    /// `superseded_reason` — `Optional<String>`.
    pub superseded_reason: Option<String>,
}

impl DecisionRequestData {
    /// The first declared invariant of `loom.decision.DecisionRequest` this value breaks, as the specification declares it,
    /// or `None` when it breaks none.
    ///
    /// An invariant is broken only when it is false of this value. One that reads something
    /// absent — an empty `Optional`, a list position past the end, or `state`, which this
    /// type does not hold — decides nothing, as the conformance interpreter reads it.
    pub fn broken_invariant(&self) -> Option<&'static str> {
        use crate::primitives::invariant as iv;
        if iv::broken(iv::any([iv::not(Some(self.recommended_option.as_ref().map(|v| &v.0).is_some())), iv::not(Some(self.no_recommendation_reason.as_ref().is_some()))])) {
            return Some("(not (defined(recommended_option)) or not (defined(no_recommendation_reason)))");
        }
        if iv::broken(iv::any([iv::not(Some(self.recommended_answer.as_ref().is_some())), iv::not(Some(self.no_recommendation_reason.as_ref().is_some()))])) {
            return Some("(not (defined(recommended_answer)) or not (defined(no_recommendation_reason)))");
        }
        if iv::broken(iv::any([iv::compare(Some(iv::Fact::Bool(self.default_on_silence)), iv::Op::Eq, Some(iv::Fact::Bool(false)), false, true), iv::compare(Some(iv::Fact::text(match self.recommendation_strength { Strength::None => "none", Strength::Weak => "weak", Strength::Moderate => "moderate", Strength::Strong => "strong" })), iv::Op::Eq, Some(iv::Fact::text("strong")), false, true)])) {
            return Some("(default_on_silence == false or recommendation_strength == strong)");
        }
        None
    }
}

/// The states of `loom.decision.DecisionRequest`, at the type level.
///
/// One marker type per declared state, sealed: a state the lifecycle does not declare cannot
/// implement [`Marker`](decision_request_state::Marker), so [`DecisionRequest<S>`](DecisionRequest) can only ever rest in a real state.
pub mod decision_request_state {
    /// Closes [`Marker`] over the declared states.
    mod sealed {
        /// Implemented only by the marker types beside this module.
        pub trait Sealed {}
        impl Sealed for super::Decided {}
        impl Sealed for super::Escalated {}
        impl Sealed for super::Expired {}
        impl Sealed for super::Open {}
        impl Sealed for super::Superseded {}
    }

    /// A declared state of `DecisionRequest`, as a type.
    pub trait Marker: sealed::Sealed {
        /// The same state, as the runtime value.
        const STATE: super::DecisionRequestState;
    }

    /// `Decided`.
    pub struct Decided;

    impl Marker for Decided {
        const STATE: super::DecisionRequestState = super::DecisionRequestState::Decided;
    }

    /// `Escalated`.
    pub struct Escalated;

    impl Marker for Escalated {
        const STATE: super::DecisionRequestState = super::DecisionRequestState::Escalated;
    }

    /// `Expired`. Terminal: an instance may rest here forever.
    pub struct Expired;

    impl Marker for Expired {
        const STATE: super::DecisionRequestState = super::DecisionRequestState::Expired;
    }

    /// `Open`. Where a new instance starts.
    pub struct Open;

    impl Marker for Open {
        const STATE: super::DecisionRequestState = super::DecisionRequestState::Open;
    }

    /// `Superseded`. Terminal: an instance may rest here forever.
    pub struct Superseded;

    impl Marker for Superseded {
        const STATE: super::DecisionRequestState = super::DecisionRequestState::Superseded;
    }
}

/// DecisionRequest — `loom.decision.DecisionRequest` — with its lifecycle state carried by the type.
///
/// The one constructor rests in `Open`, and the only way to change `S` is a method generated from
/// a declared transition. A move the specification does not declare is therefore not an error
/// case: it does not compile. Where the state is data — wire, storage — use [`DecisionRequestSnapshot`]
/// and [`DecisionRequestSnapshot::refine`].
pub struct DecisionRequest<S: decision_request_state::Marker> {
    data: DecisionRequestData,
    state: core::marker::PhantomData<S>,
}

impl<S: decision_request_state::Marker> DecisionRequest<S> {
    /// The state this instance rests in, as the runtime value.
    pub fn state(&self) -> DecisionRequestState {
        S::STATE
    }

    /// What it holds.
    pub fn data(&self) -> &DecisionRequestData {
        &self.data
    }

    /// Hands the data back, giving up the typed state.
    pub fn into_data(self) -> DecisionRequestData {
        self.data
    }
}

impl DecisionRequest<decision_request_state::Open> {
    /// A new instance, resting in `Open` — the only state the lifecycle starts one in.
    pub fn new(data: DecisionRequestData) -> Self {
        Self {
            data,
            state: core::marker::PhantomData,
        }
    }
}

impl DecisionRequest<decision_request_state::Decided> {
    /// `record_produced` — `Decided` → `Decided`. Taken by the `recorded` outcome of `loom.decision.RecordProduced`.
    pub fn record_produced(self) -> DecisionRequest<decision_request_state::Decided> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `supersede` — `Decided` → `Superseded`. Taken by the `superseded` outcome of `loom.decision.Supersede`.
    pub fn supersede(self) -> DecisionRequest<decision_request_state::Superseded> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl DecisionRequest<decision_request_state::Escalated> {
    /// `escalate` — `Escalated` → `Escalated`. Taken by the `escalated` outcome of `loom.decision.Escalate`.
    pub fn escalate(self) -> DecisionRequest<decision_request_state::Escalated> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `decide` — `Escalated` → `Decided`. Taken by the `decided` outcome of `loom.decision.Decide`, the `taken` outcome of `loom.decision.TakeRecommendation`.
    pub fn decide(self) -> DecisionRequest<decision_request_state::Decided> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `supersede` — `Escalated` → `Superseded`. Taken by the `superseded` outcome of `loom.decision.Supersede`.
    pub fn supersede(self) -> DecisionRequest<decision_request_state::Superseded> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `expire` — `Escalated` → `Expired`. Taken by the `expired` outcome of `loom.decision.Expire`.
    pub fn expire(self) -> DecisionRequest<decision_request_state::Expired> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

impl DecisionRequest<decision_request_state::Open> {
    /// `recommend` — `Open` → `Open`. Taken by the `withheld` outcome of `loom.decision.Recommend`, the `recommended` outcome of `loom.decision.Recommend`.
    pub fn recommend(self) -> DecisionRequest<decision_request_state::Open> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `escalate` — `Open` → `Escalated`. Taken by the `escalated` outcome of `loom.decision.Escalate`.
    pub fn escalate(self) -> DecisionRequest<decision_request_state::Escalated> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `decide` — `Open` → `Decided`. Taken by the `decided` outcome of `loom.decision.Decide`, the `taken` outcome of `loom.decision.TakeRecommendation`.
    pub fn decide(self) -> DecisionRequest<decision_request_state::Decided> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `supersede` — `Open` → `Superseded`. Taken by the `superseded` outcome of `loom.decision.Supersede`.
    pub fn supersede(self) -> DecisionRequest<decision_request_state::Superseded> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }

    /// `expire` — `Open` → `Expired`. Taken by the `expired` outcome of `loom.decision.Expire`.
    pub fn expire(self) -> DecisionRequest<decision_request_state::Expired> {
        DecisionRequest {
            data: self.data,
            state: core::marker::PhantomData,
        }
    }
}

/// `loom.decision.DecisionRequest` as it crosses a boundary: the state as a value beside the data.
///
/// Wire and storage know states only at runtime; [`DecisionRequestSnapshot::refine`] is the one door back
/// into the typed lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequestSnapshot {
    /// Where the instance is in its lifecycle.
    pub state: DecisionRequestState,
    /// What it holds.
    pub data: DecisionRequestData,
}

/// An `DecisionRequest` in whichever declared state it was found.
pub enum AnyDecisionRequest {
    /// Resting in `Decided`.
    Decided(DecisionRequest<decision_request_state::Decided>),
    /// Resting in `Escalated`.
    Escalated(DecisionRequest<decision_request_state::Escalated>),
    /// Resting in `Expired`.
    Expired(DecisionRequest<decision_request_state::Expired>),
    /// Resting in `Open`.
    Open(DecisionRequest<decision_request_state::Open>),
    /// Resting in `Superseded`.
    Superseded(DecisionRequest<decision_request_state::Superseded>),
}

impl DecisionRequestSnapshot {
    /// Refines the runtime state into the typed one.
    ///
    /// Total: every declared state has an arm, and an undeclared state cannot reach here because
    /// `DecisionRequestState` cannot spell one.
    pub fn refine(self) -> AnyDecisionRequest {
        match self.state {
            DecisionRequestState::Decided => AnyDecisionRequest::Decided(DecisionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            DecisionRequestState::Escalated => AnyDecisionRequest::Escalated(DecisionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            DecisionRequestState::Expired => AnyDecisionRequest::Expired(DecisionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            DecisionRequestState::Open => AnyDecisionRequest::Open(DecisionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
            DecisionRequestState::Superseded => AnyDecisionRequest::Superseded(DecisionRequest {
                data: self.data,
                state: core::marker::PhantomData,
            }),
        }
    }
}

impl AnyDecisionRequest {
    /// The state, as the runtime value.
    pub fn state(&self) -> DecisionRequestState {
        match self {
            Self::Decided(_) => DecisionRequestState::Decided,
            Self::Escalated(_) => DecisionRequestState::Escalated,
            Self::Expired(_) => DecisionRequestState::Expired,
            Self::Open(_) => DecisionRequestState::Open,
            Self::Superseded(_) => DecisionRequestState::Superseded,
        }
    }

    /// Back to the boundary shape.
    pub fn snapshot(self) -> DecisionRequestSnapshot {
        match self {
            Self::Decided(instance) => DecisionRequestSnapshot {
                state: DecisionRequestState::Decided,
                data: instance.into_data(),
            },
            Self::Escalated(instance) => DecisionRequestSnapshot {
                state: DecisionRequestState::Escalated,
                data: instance.into_data(),
            },
            Self::Expired(instance) => DecisionRequestSnapshot {
                state: DecisionRequestState::Expired,
                data: instance.into_data(),
            },
            Self::Open(instance) => DecisionRequestSnapshot {
                state: DecisionRequestState::Open,
                data: instance.into_data(),
            },
            Self::Superseded(instance) => DecisionRequestSnapshot {
                state: DecisionRequestState::Superseded,
                data: instance.into_data(),
            },
        }
    }
}

/// Decide — the input of `loom.decision.Decide`.
///
/// Take one option of an Open or Escalated request. Decide checks every guard and writes no Canon record. Where the option's consequence names a produces kind, the runtime then writes that Canon record, keyed by its record id (the Canon entry carries no request id; two open requests for the same record id must not exist); RecordProduced stores it as produced_record; the run resumes only once produced_record is set. If the case revision moved between Decide and the Canon write, the write is refused and the request is superseded. A crash after Decide leaves a Decided request without produced_record, and the retry writes the Canon record and records it. An option that produces no record resumes or ends the run directly after Decide. A question request may be decided with a free answer in place of an option (both of the operator's trial-2 answers were 'neither option as asked', docs/requester.md § 1); the answer is written to questions.yaml and produces its new revision (artifact_revision).
///
/// Everything it can result in is [`DecideOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decide {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `option` — `Optional<loom.decision.OptionLetter>`.
    pub option: Option<OptionLetter>,
    /// `answer` — `Optional<String>`.
    pub answer: Option<String>,
    /// `principal` — `loom.decision.Principal`.
    pub principal: Principal,
    /// `principal_kind` — `loom.decision.PrincipalKind`.
    pub principal_kind: PrincipalKind,
    /// `decided_at` — `Timestamp`.
    pub decided_at: crate::primitives::Timestamp,
    /// `case_revision` — `loom.decision.Revision`.
    pub case_revision: Revision,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// Everything `loom.decision.Decide` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecideOutcome {
    /// `kind-not-principal` — when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`.
    ///
    /// principal_kind must be the kind the principal's name has: decider:<kind> is decider, rule:<name> is rule, silence is silence, any other name is person. A command whose kind is not its principal's is refused whatever the holder, before every other kind guard.
    KindNotPrincipal {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `silence-takes-recommendation` — when `principal_kind == silence`.
    ///
    /// Silence never chooses an option: a default on silence is taken through TakeRecommendation.
    SilenceTakesRecommendation {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `option-or-answer` — when `((defined(option) and defined(answer)) or (not (defined(option)) and not (defined(answer))))`.
    ///
    /// A decision names exactly one of an option and a free answer.
    OptionOrAnswer {
        /// Why it was refused: `loom.decision.OptionOrAnswer`.
        error: OptionOrAnswer,
    },
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `answer-not-allowed` — when the existing subject's stored fields satisfy `unblocks.kind != question` and `defined(answer)`.
    ///
    /// An approval or an outcome is decided by one of its options; only a question request takes a free answer.
    AnswerNotAllowed {
        /// Why it was refused: `loom.decision.AnswerNotAllowed`.
        error: AnswerNotAllowed,
    },
    /// `person-only` — when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `principal_kind != person`.
    ///
    /// Holder P or P+ (docs/autonomy-levels.md:233-234): only a person decides; at P+ the decider's proposal is the recommendation (propose only).
    PersonOnly {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `not-listed` — when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))`.
    ///
    /// The principal is not in the request's eligible list.
    NotListed {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `decider-under-rule` — when the existing subject's stored fields satisfy `(holder == rule or holder == rule_final or holder == rule_and_decider or (holder == rule_and_decider_final and not (exists o in options: ((o.letter == input.option and o.consequence.grant == denied)))))` and `principal_kind == decider`.
    ///
    /// Holder R, R: end or R∧D (docs/autonomy-levels.md:237-239): the decider does not choose an option and never grants against the rule. Under R∧D→P (rule_and_decider) it takes the rule's recommendation through TakeRecommendation or vetoes through Escalate. Under R∧D* (rule_and_decider_final) its veto is a Decide on the denying option (consequence.grant denied) and nothing else; the run then ends as refused, with no explicitly_abandoned record (:275-276).
    DeciderUnderRule {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `rule-not-holder` — when the existing subject's stored fields satisfy `not ((holder == rule or holder == rule_final))` and `principal_kind == rule`.
    ///
    /// A rule decides alone only where the holder is R or R: end (docs/autonomy-levels.md:237-238).
    RuleNotHolder {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `decided` — otherwise.
    ///
    /// The request is Decided and has no produced_record yet; RecordProduced adds it after the runtime wrote the Canon record.
    Decided {
        /// The `loom.decision.DecisionTaken` this outcome publishes.
        decision_taken: DecisionTaken,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Open or Escalated.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Escalate — the input of `loom.decision.Escalate`.
///
/// Move an Open or Escalated request to a person. A decider or a rule escalates when it will not decide under D→P, R, or R∧D→P, where an Escalate is also the decider's veto (docs/autonomy-levels.md:235-239, escalation.mode person :385). Under P and P+ an Escalate is a person passing the request on to someone else, also from Escalated. Escalate never opens a request: the escalation row of the class table (:257) is not a request class, it gives the holder of the request after Escalate (P at A2 to A4 in SDC's table).
///
/// Everything it can result in is [`EscalateOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escalate {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `escalate_to` — `loom.decision.Principal`.
    pub escalate_to: Principal,
    /// `reason` — `String`.
    pub reason: String,
}

/// Everything `loom.decision.Escalate` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalateOutcome {
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `suppressed` — when the existing subject's stored fields satisfy `(holder == decider_final or holder == rule_final or holder == rule_and_decider_final)`.
    ///
    /// Holder D*, R: end or R∧D* (A5, docs/autonomy-levels.md:236, :255-256, :385, :454-459): no person is asked. Under D* the decider's best is taken through Decide or TakeRecommendation and the driver records escalation_suppressed; under R: end and on an R∧D* veto the run ends.
    Suppressed {
        /// Why it was refused: `loom.decision.EscalationSuppressed`.
        error: EscalationSuppressed,
    },
    /// `escalated` — otherwise.
    ///
    /// The request waits on the named person and its holder becomes P, the escalation row's cell, so a decider can no longer decide it. A P+ request passed on by a person also becomes P: P and P+ have the same guards, and the recommendation stays.
    Escalated {
        /// The `loom.decision.DecisionEscalated` this outcome publishes.
        decision_escalated: DecisionEscalated,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Open or Escalated.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Expire — the input of `loom.decision.Expire`.
///
/// End a request nobody answered before expires_at, when it has no default on silence. Fired by a host tick that passes the instant as input (at); the command reads no clock.
///
/// Everything it can result in is [`ExpireOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expire {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `at` — `Timestamp`.
    pub at: crate::primitives::Timestamp,
}

/// Everything `loom.decision.Expire` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpireOutcome {
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `not-due` — when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.at as timestamp)`.
    ///
    /// The request has no expires_at, or the tick's instant is before it.
    NotDue {
        /// Why it was refused: `loom.decision.NotDue`.
        error: NotDue,
    },
    /// `has-default` — when the existing subject's stored fields satisfy `default_on_silence == true`.
    ///
    /// Decide it with principal silence instead.
    HasDefault {
        /// Why it was refused: `loom.decision.SilenceHasDefault`.
        error: SilenceHasDefault,
    },
    /// `expired` — otherwise.
    ///
    /// The request is Expired; the case stays stopped.
    Expired {
        /// The `loom.decision.DecisionExpired` this outcome publishes.
        decision_expired: DecisionExpired,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Open or Escalated.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Record a recommendation — the input of `loom.decision.Recommend`.
///
/// Record a recommendation on an open request, after the grounding check (tools/decision-grounds.sh) ran on it. A refused check records no recommendation and the reason. A question request may be recommended a free answer in place of an option (docs/requester.md § 3).
///
/// Everything it can result in is [`RecommendOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommend {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `strength` — `loom.decision.Strength`.
    pub strength: Strength,
    /// `default_on_silence` — `Boolean`.
    pub default_on_silence: bool,
    /// `grounds` — `List<loom.decision.Ground>`.
    pub grounds: Vec<Ground>,
    /// `proposed_by` — `loom.decision.ProposerKind`.
    pub proposed_by: ProposerKind,
    /// `check_verdict` — `loom.decision.CheckVerdict`.
    pub check_verdict: CheckVerdict,
    /// `check_report` — `String`.
    pub check_report: String,
}

/// Everything `loom.decision.Recommend` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendOutcome {
    /// `ungrounded` — when `(check_verdict == verified and (grounds.count < 1 or strength == none))`.
    ///
    /// A verified recommendation without a ground, or with strength none, is a contradiction: nothing was recorded.
    Ungrounded {
        /// Why it was refused: `loom.decision.UngroundedRecommendation`.
        error: UngroundedRecommendation,
    },
    /// `silence-needs-strong` — when `(check_verdict == verified and default_on_silence == true and strength != strong)`.
    ///
    /// Only a strong recommendation may be the default on silence.
    SilenceNeedsStrong {
        /// Why it was refused: `loom.decision.NoDefaultOnSilence`.
        error: NoDefaultOnSilence,
    },
    /// `option-or-answer` — when `(check_verdict == verified and ((defined(recommended_option) and defined(recommended_answer)) or (not (defined(recommended_option)) and not (defined(recommended_answer)))))`.
    ///
    /// A verified recommendation names exactly one of an option and a free answer.
    OptionOrAnswer {
        /// Why it was refused: `loom.decision.OptionOrAnswer`.
        error: OptionOrAnswer,
    },
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `answer-not-allowed` — when the existing subject's stored fields satisfy `unblocks.kind != question` and `(check_verdict == verified and defined(recommended_answer))`.
    ///
    /// Only a question request is recommended a free answer.
    AnswerNotAllowed {
        /// Why it was refused: `loom.decision.AnswerNotAllowed`.
        error: AnswerNotAllowed,
    },
    /// `withheld` — when `check_verdict == refused`.
    ///
    /// The check refused the citations: the request carries no recommendation and says why.
    Withheld {
        /// The `loom.decision.RecommendationWithheld` this outcome publishes.
        recommendation_withheld: RecommendationWithheld,
    },
    /// `recommended` — otherwise.
    ///
    /// The request carries the recommendation and its verified grounds.
    Recommended {
        /// The `loom.decision.RecommendationRecorded` this outcome publishes.
        recommendation_recorded: RecommendationRecorded,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Open.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Record the produced Canon record — the input of `loom.decision.RecordProduced`.
///
/// Store the Canon record the runtime wrote for a Decided request whose decided option produces one (canon-authority/1, canon-decisions/1, canon-evidence/1, or the questions.yaml revision). It runs after Decide or TakeRecommendation and after the Canon write, which is keyed by the record id; the run resumes only once produced_record is set. Sending the record already stored changes nothing; a different record is refused; so is a request whose decided option produces no record.
///
/// Everything it can result in is [`RecordProducedOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordProduced {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `produced` — `loom.decision.ProducedRecord`.
    pub produced: ProducedRecord,
}

/// Everything `loom.decision.RecordProduced` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordProducedOutcome {
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `already-recorded` — when the existing subject's stored fields satisfy `{expr: "coalesce(produced_record.record, "")"} == input.produced.record`.
    ///
    /// A record with the same record id is already stored: record ids key idempotence, so a retry after the record was written changes nothing.
    AlreadyRecorded,
    /// `produced-conflict` — when the existing subject's stored fields satisfy `defined(produced_record)`.
    ///
    /// A different record is already stored: the first one stands.
    ProducedConflict {
        /// Why it was refused: `loom.decision.ProducedRecordConflict`.
        error: ProducedRecordConflict,
    },
    /// `option-produces-none` — when the existing subject's stored fields satisfy `(defined(decided_option) and not (exists o in options: ((o.letter == {fact: decided_option} and defined(o.consequence.produces)))))`.
    ///
    /// The decided option produces no Canon record: the run resumes or ends without RecordProduced.
    OptionProducesNone {
        /// Why it was refused: `loom.decision.NoRecordToProduce`.
        error: NoRecordToProduce,
    },
    /// `recorded` — otherwise.
    ///
    /// The Decided request names the Canon record it produced; the run may resume.
    Recorded {
        /// The `loom.decision.ProducedRecordRecorded` this outcome publishes.
        produced_record_recorded: ProducedRecordRecorded,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Decided.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Request a decision — the input of `loom.decision.Request`.
///
/// Open a decision request where a case stops on a decision: an approval-required action, a handed-off question, or an outcome that needs an explicit decision.
///
/// Everything it can result in is [`RequestOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: CaseId,
    /// `question_key` — `String`.
    pub question_key: String,
    /// `record_key` — `String`.
    pub record_key: String,
    /// `open_for_record` — `Optional<loom.decision.DecisionRequestId>`.
    pub open_for_record: Option<DecisionRequestId>,
    /// `unblocks` — `loom.decision.Unblocks`.
    pub unblocks: Unblocks,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `subject_path` — `Optional<String>`.
    pub subject_path: Option<String>,
    /// `question` — `String`.
    pub question: String,
    /// `options` — `List<loom.decision.DecisionOption>`.
    pub options: Vec<DecisionOption>,
    /// `autonomy_level` — `loom.decision.AutonomyLevel`.
    pub autonomy_level: AutonomyLevel,
    /// `decision_class` — `loom.decision.DecisionClassId`.
    pub decision_class: DecisionClassId,
    /// `holder` — `loom.decision.Holder`.
    pub holder: Holder,
    /// `eligible` — `List<loom.decision.Principal>`.
    pub eligible: Vec<Principal>,
    /// `addressed_to` — `loom.decision.Principal`.
    pub addressed_to: Principal,
    /// `opened_at` — `Timestamp`.
    pub opened_at: crate::primitives::Timestamp,
    /// `expires_at` — `Optional<Timestamp>`.
    pub expires_at: Option<crate::primitives::Timestamp>,
    /// `supersedes` — `Optional<loom.decision.DecisionRequestId>`.
    pub supersedes: Option<DecisionRequestId>,
}

/// Everything `loom.decision.Request` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestOutcome {
    /// `escalation-is-not-a-class` — when `decision_class == escalation`.
    ///
    /// The escalation row of docs/autonomy-levels.md:257 is not a request class: Escalate moves an existing request, and the row gives its holder after Escalate. Nothing was opened.
    EscalationIsNotAClass {
        /// Why it was refused: `loom.decision.NotARequestClass`.
        error: NotARequestClass,
    },
    /// `too-few-options` — when `options.count < 2`.
    ///
    /// Fewer than two options: nothing was opened.
    TooFewOptions {
        /// Why it was refused: `loom.decision.TooFewOptions`.
        error: TooFewOptions,
    },
    /// `record-key-open` — when `defined(open_for_record)`.
    ///
    /// An Open or Escalated request exists for the record key, the one named: nothing was opened (refusal, not supersession).
    RecordKeyOpen {
        /// Why it was refused: `loom.decision.RecordKeyOpen`.
        error: RecordKeyOpen,
    },
    /// `requested` — otherwise.
    ///
    /// The request is Open, with no recommendation yet.
    Requested {
        /// The `loom.decision.DecisionRequested` this outcome publishes.
        decision_requested: DecisionRequested,
    },
}

/// Supersede — the input of `loom.decision.Supersede`.
///
/// Replace a request by a newer one: the subject moved to a new revision, a ground changed, or a person reopened a decision. The newer request names this one in supersedes.
///
/// Everything it can result in is [`SupersedeOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supersede {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `superseded_by` — `loom.decision.DecisionRequestId`.
    pub superseded_by: DecisionRequestId,
    /// `reason` — `String`.
    pub reason: String,
}

/// Everything `loom.decision.Supersede` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupersedeOutcome {
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `superseded` — otherwise.
    ///
    /// The request is Superseded; a decision it held no longer applies. Superseding a Decided request does not revoke the Canon record it produced: Canon's own binding to the subject or case revision decides whether that record still applies.
    Superseded {
        /// The `loom.decision.DecisionSuperseded` this outcome publishes.
        decision_superseded: DecisionSuperseded,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is already Superseded or Expired.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// Take the recommendation — the input of `loom.decision.TakeRecommendation`.
///
/// Decide a request with its recommended option or free answer, which must be strong: a person accepting it, a decider under R∧D (the rule's verdict is the strong recommendation; docs/autonomy-levels.md:239, the merge rule :261-267) or under D→P and D*, or silence when the recommendation is the default on silence. Silence is fired by a host tick that passes the instant as decided_at, at or after expires_at; the command reads no clock. The option or answer is read from the request, never sent. As with Decide, no Canon record is written here: the runtime writes it afterwards and records it through RecordProduced.
///
/// Everything it can result in is [`TakeRecommendationOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeRecommendation {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `principal` — `loom.decision.Principal`.
    pub principal: Principal,
    /// `principal_kind` — `loom.decision.PrincipalKind`.
    pub principal_kind: PrincipalKind,
    /// `decided_at` — `Timestamp`.
    pub decided_at: crate::primitives::Timestamp,
    /// `case_revision` — `loom.decision.Revision`.
    pub case_revision: Revision,
    /// `reasons` — `List<String>`.
    pub reasons: Vec<String>,
}

/// Everything `loom.decision.TakeRecommendation` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeRecommendationOutcome {
    /// `kind-not-principal` — when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`.
    ///
    /// principal_kind must be the kind the principal's name has: decider:<kind> is decider, rule:<name> is rule, silence is silence, any other name is person. A command whose kind is not its principal's is refused whatever the holder, before every other kind guard.
    KindNotPrincipal {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `no-request` — for an identity no record carries.
    ///
    /// No request has this id.
    NoRequest {
        /// Why it was refused: `loom.decision.NoSuchRequest`.
        error: NoSuchRequest,
    },
    /// `person-only` — when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `(principal_kind == decider or principal_kind == rule)`.
    ///
    /// Holder P or P+: only a person decides, a recommendation included.
    PersonOnly {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `not-listed` — when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))` and `principal_kind != silence`.
    ///
    /// A person or decider not in the request's eligible list.
    NotListed {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `not-strong` — when the existing subject's stored fields satisfy `recommendation_strength != strong`.
    ///
    /// Only a strong recommendation is taken through TakeRecommendation, by every principal kind. A person who wants a weak or moderate recommendation chooses its option through Decide. An absent recommendation has strength none.
    NotStrong {
        /// Why it was refused: `loom.decision.NotEligible`.
        error: NotEligible,
    },
    /// `silence-without-default` — when the existing subject's stored fields satisfy `default_on_silence == false` and `principal_kind == silence`.
    ///
    /// Silence takes the recommendation only when it is the default on silence.
    SilenceWithoutDefault {
        /// Why it was refused: `loom.decision.NoDefaultOnSilence`.
        error: NoDefaultOnSilence,
    },
    /// `silence-not-due` — when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.decided_at as timestamp)` and `principal_kind == silence`.
    ///
    /// Silence is fired by a host tick at or after expires_at; the tick's instant is decided_at.
    SilenceNotDue {
        /// Why it was refused: `loom.decision.NotDue`.
        error: NotDue,
    },
    /// `taken` — otherwise.
    ///
    /// The request is Decided with its recommended option or answer and no produced_record yet.
    Taken {
        /// The `loom.decision.RecommendationTaken` this outcome publishes.
        recommendation_taken: RecommendationTaken,
    },
    /// `wrong-state` — from a state no declared move starts in.
    ///
    /// The request is not Open or Escalated.
    WrongState {
        /// Why it was refused: `loom.decision.DecisionStateConflict`.
        error: DecisionStateConflict,
    },
}

/// DecisionEscalated — the event `loom.decision.DecisionEscalated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionEscalated {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `escalate_to` — `loom.decision.Principal`.
    pub escalate_to: Principal,
}

/// DecisionExpired — the event `loom.decision.DecisionExpired`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionExpired {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// DecisionRequested — the event `loom.decision.DecisionRequested`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequested {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: CaseId,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
}

/// DecisionSuperseded — the event `loom.decision.DecisionSuperseded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionSuperseded {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `superseded_by` — `loom.decision.DecisionRequestId`.
    pub superseded_by: DecisionRequestId,
}

/// DecisionTaken — the event `loom.decision.DecisionTaken`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionTaken {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `option` — `Optional<loom.decision.OptionLetter>`.
    pub option: Option<OptionLetter>,
    /// `answer` — `Optional<String>`.
    pub answer: Option<String>,
    /// `principal` — `loom.decision.Principal`.
    pub principal: Principal,
}

/// ProducedRecordRecorded — the event `loom.decision.ProducedRecordRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedRecordRecorded {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `produced` — `loom.decision.ProducedRecord`.
    pub produced: ProducedRecord,
}

/// RecommendationRecorded — the event `loom.decision.RecommendationRecorded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationRecorded {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `strength` — `loom.decision.Strength`.
    pub strength: Strength,
}

/// RecommendationTaken — the event `loom.decision.RecommendationTaken`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationTaken {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `principal` — `loom.decision.Principal`.
    pub principal: Principal,
}

/// RecommendationWithheld — the event `loom.decision.RecommendationWithheld`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendationWithheld {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `reason` — `String`.
    pub reason: String,
}

/// The declared error `loom.decision.AnswerNotAllowed`.
///
/// Only a question request takes a free answer; an approval or an outcome is decided by one of its options. Nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerNotAllowed {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.DecisionStateConflict`.
///
/// The request is not in a state this command acts from; nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionStateConflict {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.EscalationSuppressed`.
///
/// The holder is final (D* or R∧D*): an escalation is suppressed, not recorded as Escalated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationSuppressed {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NoDefaultOnSilence`.
///
/// Silence takes only the recommended option of a request whose recommendation is the default on silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoDefaultOnSilence {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NoRecordToProduce`.
///
/// The decided option's consequence produces no Canon record; nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoRecordToProduce {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NoSuchRequest`.
///
/// No decision request has this id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoSuchRequest {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NotARequestClass`.
///
/// A request cannot be opened in class escalation; Escalate moves an existing request instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotARequestClass {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NotDue`.
///
/// The host tick's instant is before the request's expires_at, or the request has none: silence takes nothing and nothing expires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotDue {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.NotEligible`.
///
/// The principal may not decide this request at its autonomy level; nothing was decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEligible {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `principal` — `loom.decision.Principal`.
    pub principal: Principal,
}

/// The declared error `loom.decision.OptionOrAnswer`.
///
/// A decision or a verified recommendation names exactly one of an option and a free answer; nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionOrAnswer {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.ProducedRecordConflict`.
///
/// The request already names a different produced Canon record; the first one recorded stands and nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedRecordConflict {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.RecordKeyOpen`.
///
/// An Open or Escalated request already exists for the record key: two open requests for one record id must not exist (the SDC's Decide summary; docs/decisions.md:263-264). Nothing was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordKeyOpen {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `open_for_record` — `loom.decision.DecisionRequestId`.
    pub open_for_record: DecisionRequestId,
}

/// The declared error `loom.decision.SilenceHasDefault`.
///
/// The request has a default on silence: it is decided with principal silence, not expired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SilenceHasDefault {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.TooFewOptions`.
///
/// A decision request needs at least two options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooFewOptions {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// The declared error `loom.decision.UngroundedRecommendation`.
///
/// A recommendation the grounding check verified cites no ground; nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UngroundedRecommendation {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
}

/// Decision history of an artifact — one row of the view `loom.decision.ArtifactDecisionHistory`.
///
/// Projects `loom.decision.DecisionRequest` at `read_your_writes` consistency, containing instances where `subject_artifact == param.artifact`.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDecisionHistory {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `question` — `String`.
    pub question: String,
    /// `state` — `loom.decision.DecisionRequest.State`.
    pub state: DecisionRequestState,
    /// `decided_option` — `Optional<loom.decision.OptionLetter>`.
    pub decided_option: Option<OptionLetter>,
    /// `decided_answer` — `Optional<String>`.
    pub decided_answer: Option<String>,
    /// `decided_by` — `Optional<loom.decision.Principal>`.
    pub decided_by: Option<Principal>,
    /// `decided_at` — `Optional<Timestamp>`.
    pub decided_at: Option<crate::primitives::Timestamp>,
    /// `produced_record` — `Optional<loom.decision.ProducedRecord>`.
    pub produced_record: Option<ProducedRecord>,
    /// `supersedes` — `Optional<loom.decision.DecisionRequestId>`.
    pub supersedes: Option<DecisionRequestId>,
    /// `superseded_by` — `Optional<loom.decision.DecisionRequestId>`.
    pub superseded_by: Option<DecisionRequestId>,
}

/// Decision requests — one row of the view `loom.decision.DecisionRequests`.
///
/// Projects `loom.decision.DecisionRequest` at `read_your_writes` consistency.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequests {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `state` — `loom.decision.DecisionRequest.State`.
    pub state: DecisionRequestState,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: CaseId,
    /// `question_key` — `String`.
    pub question_key: String,
    /// `record_key` — `String`.
    pub record_key: String,
    /// `unblocks` — `loom.decision.Unblocks`.
    pub unblocks: Unblocks,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `subject_path` — `Optional<String>`.
    pub subject_path: Option<String>,
    /// `question` — `String`.
    pub question: String,
    /// `options` — `List<loom.decision.DecisionOption>`.
    pub options: Vec<DecisionOption>,
    /// `autonomy_level` — `loom.decision.AutonomyLevel`.
    pub autonomy_level: AutonomyLevel,
    /// `decision_class` — `loom.decision.DecisionClassId`.
    pub decision_class: DecisionClassId,
    /// `holder` — `loom.decision.Holder`.
    pub holder: Holder,
    /// `eligible` — `List<loom.decision.Principal>`.
    pub eligible: Vec<Principal>,
    /// `addressed_to` — `loom.decision.Principal`.
    pub addressed_to: Principal,
    /// `opened_at` — `Timestamp`.
    pub opened_at: crate::primitives::Timestamp,
    /// `expires_at` — `Optional<Timestamp>`.
    pub expires_at: Option<crate::primitives::Timestamp>,
    /// `supersedes` — `Optional<loom.decision.DecisionRequestId>`.
    pub supersedes: Option<DecisionRequestId>,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `recommendation_strength` — `loom.decision.Strength`.
    pub recommendation_strength: Strength,
    /// `default_on_silence` — `Boolean`.
    pub default_on_silence: bool,
    /// `grounds` — `List<loom.decision.Ground>`.
    pub grounds: Vec<Ground>,
    /// `recommended_by` — `Optional<loom.decision.ProposerKind>`.
    pub recommended_by: Option<ProposerKind>,
    /// `grounding_report` — `Optional<String>`.
    pub grounding_report: Option<String>,
    /// `no_recommendation_reason` — `Optional<String>`.
    pub no_recommendation_reason: Option<String>,
    /// `escalated_to` — `Optional<loom.decision.Principal>`.
    pub escalated_to: Option<Principal>,
    /// `escalation_reason` — `Optional<String>`.
    pub escalation_reason: Option<String>,
    /// `decided_option` — `Optional<loom.decision.OptionLetter>`.
    pub decided_option: Option<OptionLetter>,
    /// `decided_answer` — `Optional<String>`.
    pub decided_answer: Option<String>,
    /// `decided_by` — `Optional<loom.decision.Principal>`.
    pub decided_by: Option<Principal>,
    /// `decided_by_kind` — `Optional<loom.decision.PrincipalKind>`.
    pub decided_by_kind: Option<PrincipalKind>,
    /// `decided_at` — `Optional<Timestamp>`.
    pub decided_at: Option<crate::primitives::Timestamp>,
    /// `decided_case_revision` — `Optional<loom.decision.Revision>`.
    pub decided_case_revision: Option<Revision>,
    /// `decision_reasons` — `List<String>`.
    pub decision_reasons: Vec<String>,
    /// `produced_record` — `Optional<loom.decision.ProducedRecord>`.
    pub produced_record: Option<ProducedRecord>,
    /// `superseded_by` — `Optional<loom.decision.DecisionRequestId>`.
    pub superseded_by: Option<DecisionRequestId>,
    /// `superseded_reason` — `Optional<String>`.
    pub superseded_reason: Option<String>,
}

/// Open decisions for a principal — one row of the view `loom.decision.OpenDecisionsForPrincipal`.
///
/// Projects `loom.decision.DecisionRequest` at `read_your_writes` consistency, containing instances where `((state == Open or state == Escalated) and addressed_to == param.principal)`.
/// The specification fully determines every row, so its query is generated over the storage port —
/// see the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDecisionsForPrincipal {
    /// `request_id` — `loom.decision.DecisionRequestId`.
    pub request_id: DecisionRequestId,
    /// `case_id` — `loom.decision.CaseId`.
    pub case_id: CaseId,
    /// `question` — `String`.
    pub question: String,
    /// `subject_artifact` — `loom.decision.ArtifactId`.
    pub subject_artifact: ArtifactId,
    /// `subject_revision` — `loom.decision.Revision`.
    pub subject_revision: Revision,
    /// `recommended_option` — `Optional<loom.decision.OptionLetter>`.
    pub recommended_option: Option<OptionLetter>,
    /// `recommended_answer` — `Optional<String>`.
    pub recommended_answer: Option<String>,
    /// `recommendation_strength` — `loom.decision.Strength`.
    pub recommendation_strength: Strength,
    /// `no_recommendation_reason` — `Optional<String>`.
    pub no_recommendation_reason: Option<String>,
    /// `opened_at` — `Timestamp`.
    pub opened_at: crate::primitives::Timestamp,
    /// `expires_at` — `Optional<Timestamp>`.
    pub expires_at: Option<crate::primitives::Timestamp>,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `loom.decision.Decide` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NotEligible`, which the specification gives no source, in `kind-not-principal`.
    ///
    /// Contract: given `loom.decision.Decide` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `kind-not-principal` when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`, error `loom.decision.NotEligible`; `silence-takes-recommendation` when `principal_kind == silence`, error `loom.decision.NotEligible`; `option-or-answer` when `((defined(option) and defined(answer)) or (not (defined(option)) and not (defined(answer))))`, error `loom.decision.OptionOrAnswer`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `answer-not-allowed` when the existing subject's stored fields satisfy `unblocks.kind != question` and `defined(answer)`, error `loom.decision.AnswerNotAllowed`; `person-only` when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `principal_kind != person`, error `loom.decision.NotEligible`; `not-listed` when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))`, error `loom.decision.NotEligible`; `decider-under-rule` when the existing subject's stored fields satisfy `(holder == rule or holder == rule_final or holder == rule_and_decider or (holder == rule_and_decider_final and not (exists o in options: ((o.letter == input.option and o.consequence.grant == denied)))))` and `principal_kind == decider`, error `loom.decision.NotEligible`; `rule-not-holder` when the existing subject's stored fields satisfy `not ((holder == rule or holder == rule_final))` and `principal_kind == rule`, error `loom.decision.NotEligible`; `decided` otherwise, takes `decide` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionTaken`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait DecideBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Decide`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn decide(&mut self, input: super::Decide) -> Result<super::DecideOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.Escalate` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request`.
    ///
    /// Contract: given `loom.decision.Escalate` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `suppressed` when the existing subject's stored fields satisfy `(holder == decider_final or holder == rule_final or holder == rule_and_decider_final)`, error `loom.decision.EscalationSuppressed`; `escalated` otherwise, takes `escalate` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionEscalated`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait EscalateBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Escalate`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn escalate(&mut self, input: super::Escalate) -> Result<super::EscalateOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.Expire` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request`.
    ///
    /// Contract: given `loom.decision.Expire` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `not-due` when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.at as timestamp)`, error `loom.decision.NotDue`; `has-default` when the existing subject's stored fields satisfy `default_on_silence == true`, error `loom.decision.SilenceHasDefault`; `expired` otherwise, takes `expire` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionExpired`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait ExpireBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Expire`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn expire(&mut self, input: super::Expire) -> Result<super::ExpireOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.Recommend` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.UngroundedRecommendation`, which the specification gives no source, in `ungrounded`.
    ///
    /// Contract: given `loom.decision.Recommend` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `ungrounded` when `(check_verdict == verified and (grounds.count < 1 or strength == none))`, error `loom.decision.UngroundedRecommendation`; `silence-needs-strong` when `(check_verdict == verified and default_on_silence == true and strength != strong)`, error `loom.decision.NoDefaultOnSilence`; `option-or-answer` when `(check_verdict == verified and ((defined(recommended_option) and defined(recommended_answer)) or (not (defined(recommended_option)) and not (defined(recommended_answer)))))`, error `loom.decision.OptionOrAnswer`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `answer-not-allowed` when the existing subject's stored fields satisfy `unblocks.kind != question` and `(check_verdict == verified and defined(recommended_answer))`, error `loom.decision.AnswerNotAllowed`; `withheld` when `check_verdict == refused`, takes `recommend` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationWithheld`; `recommended` otherwise, takes `recommend` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationRecorded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait RecommendBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Recommend`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn recommend(&mut self, input: super::Recommend) -> Result<super::RecommendOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.RecordProduced` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request`.
    ///
    /// Contract: given `loom.decision.RecordProduced` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `already-recorded` when the existing subject's stored fields satisfy `{expr: "coalesce(produced_record.record, "")"} == input.produced.record`, preserves `loom.decision.DecisionRequest`; `produced-conflict` when the existing subject's stored fields satisfy `defined(produced_record)`, error `loom.decision.ProducedRecordConflict`; `option-produces-none` when the existing subject's stored fields satisfy `(defined(decided_option) and not (exists o in options: ((o.letter == {fact: decided_option} and defined(o.consequence.produces)))))`, error `loom.decision.NoRecordToProduce`; `recorded` otherwise, takes `record_produced` of `loom.decision.DecisionRequest`, emits `loom.decision.ProducedRecordRecorded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait RecordProducedBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.RecordProduced`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn record_produced(&mut self, input: super::RecordProduced) -> Result<super::RecordProducedOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.Request` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NotARequestClass`, which the specification gives no source, in `escalation-is-not-a-class`.
    ///
    /// Contract: given `loom.decision.Request` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `escalation-is-not-a-class` when `decision_class == escalation`, error `loom.decision.NotARequestClass`; `too-few-options` when `options.count < 2`, error `loom.decision.TooFewOptions`; `record-key-open` when `defined(open_for_record)`, error `loom.decision.RecordKeyOpen`; `requested` otherwise, creates `loom.decision.DecisionRequest`, emits `loom.decision.DecisionRequested`.
    pub trait RequestBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Request`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn request(&mut self, input: super::Request) -> Result<super::RequestOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.Supersede` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request`.
    ///
    /// Contract: given `loom.decision.Supersede` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `superseded` otherwise, takes `supersede` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionSuperseded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait SupersedeBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.Supersede`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn supersede(&mut self, input: super::Supersede) -> Result<super::SupersedeOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.decision.TakeRecommendation` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.decision.NotEligible`, which the specification gives no source, in `kind-not-principal`.
    ///
    /// Contract: given `loom.decision.TakeRecommendation` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `kind-not-principal` when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`, error `loom.decision.NotEligible`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `person-only` when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `(principal_kind == decider or principal_kind == rule)`, error `loom.decision.NotEligible`; `not-listed` when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))` and `principal_kind != silence`, error `loom.decision.NotEligible`; `not-strong` when the existing subject's stored fields satisfy `recommendation_strength != strong`, error `loom.decision.NotEligible`; `silence-without-default` when the existing subject's stored fields satisfy `default_on_silence == false` and `principal_kind == silence`, error `loom.decision.NoDefaultOnSilence`; `silence-not-due` when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.decided_at as timestamp)` and `principal_kind == silence`, error `loom.decision.NotDue`; `taken` otherwise, takes `decide` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationTaken`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict`.
    pub trait TakeRecommendationBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.decision.TakeRecommendation`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn take_recommendation(&mut self, input: super::TakeRecommendation) -> Result<super::TakeRecommendationOutcome, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.decision.ArtifactDecisionHistory` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait ArtifactDecisionHistoryQuery {
        /// Serves `loom.decision.ArtifactDecisionHistory` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn artifact_decision_history(&self, artifact: crate::decision::ArtifactId) -> Result<Vec<super::ArtifactDecisionHistory>, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.decision.DecisionRequests` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait DecisionRequestsQuery {
        /// Serves `loom.decision.DecisionRequests` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn decision_requests(&self) -> Result<Vec<super::DecisionRequests>, crate::obligation::UnmetObligation>;
    }

    /// The query `loom.decision.OpenDecisionsForPrincipal` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage port. Implement it yourself to replace that query.
    pub trait OpenDecisionsForPrincipalQuery {
        /// Serves `loom.decision.OpenDecisionsForPrincipal` rows at the view's declared consistency.
        ///
        /// `Err` is the typed refusal of a row whose declared type cannot hold its value.
        fn open_decisions_for_principal(&self, principal: crate::decision::Principal) -> Result<Vec<super::OpenDecisionsForPrincipal>, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl DecideBehavior for Unimplemented {
        fn decide(&mut self, _input: super::Decide) -> Result<super::DecideOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Decide" })
        }
    }

    impl EscalateBehavior for Unimplemented {
        fn escalate(&mut self, _input: super::Escalate) -> Result<super::EscalateOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Escalate" })
        }
    }

    impl ExpireBehavior for Unimplemented {
        fn expire(&mut self, _input: super::Expire) -> Result<super::ExpireOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Expire" })
        }
    }

    impl RecommendBehavior for Unimplemented {
        fn recommend(&mut self, _input: super::Recommend) -> Result<super::RecommendOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Recommend" })
        }
    }

    impl RecordProducedBehavior for Unimplemented {
        fn record_produced(&mut self, _input: super::RecordProduced) -> Result<super::RecordProducedOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.RecordProduced" })
        }
    }

    impl RequestBehavior for Unimplemented {
        fn request(&mut self, _input: super::Request) -> Result<super::RequestOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Request" })
        }
    }

    impl SupersedeBehavior for Unimplemented {
        fn supersede(&mut self, _input: super::Supersede) -> Result<super::SupersedeOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.Supersede" })
        }
    }

    impl TakeRecommendationBehavior for Unimplemented {
        fn take_recommendation(&mut self, _input: super::TakeRecommendation) -> Result<super::TakeRecommendationOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.decision.TakeRecommendation" })
        }
    }
}
