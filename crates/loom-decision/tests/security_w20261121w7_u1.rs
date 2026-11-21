//! Security review of `story:decision-request-entity` (wave 2026-11-21-w7, unit U1): integrity
//! conformance checks of `b10x-loom-decision` against its own contract, written by the security
//! reviewer. Each guard refusal of each command is applied over a store that counts writes; each
//! closed state is given every command; the principal fields of each command are tried against
//! the stored request; and the record-key rule is tried across the two port calls it rests on.

use std::cell::RefCell;

use b10x_loom_decision::model::{
    ActionId, ArtifactId, AutonomyLevel, CapabilityId, CaseId, CheckVerdict, Citation, Consequence,
    Decide, DecideOutcome, DecisionClassId, DecisionOption, DecisionRequestId,
    DecisionRequestSnapshot, DecisionRequestState, Escalate, EscalateOutcome,
    EscalationSuppressedRecord, Expire, ExpireOutcome, Grant, Ground, GroundSource, Holder,
    OptionLetter, PathLineCitation, Principal, PrincipalKind, ProducedDocument, ProducedRecord,
    ProposerKind, Recommend, RecommendOutcome, RecordProduced, RecordProducedOutcome, Request,
    RequestOutcome, Revision, Strength, Supersede, SupersedeOutcome, TakeRecommendation,
    TakeRecommendationOutcome, UnblockKind, Unblocks,
};
use b10x_loom_decision::{
    AppendError, DecisionError, DecisionStore, InMemoryDecisionStore, StoreFailure, Timestamp,
    WriteError, decide, escalate, expire, recommend, record_produced, request, supersede,
    take_recommendation,
};

const DR1: &str = "DR-sec-1";
const DR2: &str = "DR-sec-2";
const NONE: &str = "DR-sec-absent";
const KEY: &str = "change.merge@r9";

fn id(text: &str) -> DecisionRequestId {
    DecisionRequestId(text.to_owned())
}

fn who(text: &str) -> Principal {
    Principal(text.to_owned())
}

fn letter(text: &str) -> OptionLetter {
    OptionLetter(text.to_owned())
}

fn at(text: &str) -> Timestamp {
    Timestamp(text.to_owned())
}

fn option(name: &str, grant: Option<Grant>, produces: Option<ProducedDocument>) -> DecisionOption {
    DecisionOption {
        letter: letter(name),
        does: format!("Option {name}."),
        cost: "None.".to_owned(),
        consequence: Consequence {
            admits: Vec::new(),
            blocks: Vec::new(),
            grant,
            answer: None,
            produces,
        },
    }
}

/// An approval: A grants and produces an entry, B denies and produces an entry, C produces none.
fn approval(request_id: &str, holder: Holder, eligible: &[&str]) -> Request {
    Request {
        request_id: id(request_id),
        case_id: CaseId("IDEA-1.sec".to_owned()),
        question_key: "IDEA-1.sec|change.merge|".to_owned(),
        record_key: KEY.to_owned(),
        open_for_record: None,
        unblocks: Unblocks {
            kind: UnblockKind::Approval,
            action: ActionId("change.merge".to_owned()),
            capability: Some(CapabilityId("change.merge".to_owned())),
            question_id: None,
            outcome: None,
        },
        subject_artifact: ArtifactId("change".to_owned()),
        subject_revision: Revision("t0r1".to_owned()),
        subject_path: None,
        question: "Merge?".to_owned(),
        options: vec![
            option(
                "A",
                Some(Grant::Granted),
                Some(ProducedDocument::CanonAuthority),
            ),
            option(
                "B",
                Some(Grant::Denied),
                Some(ProducedDocument::CanonAuthority),
            ),
            option("C", None, None),
        ],
        autonomy_level: AutonomyLevel::A3,
        decision_class: DecisionClassId("merge".to_owned()),
        holder,
        eligible: eligible.iter().map(|name| who(name)).collect(),
        addressed_to: who(eligible.first().copied().unwrap_or("operator")),
        opened_at: at("2026-11-21T11:00:00Z"),
        expires_at: Some(at("2026-11-21T12:00:00Z")),
        supersedes: None,
    }
}

fn decide_by(request_id: &str, option: &str, principal: &str, kind: PrincipalKind) -> Decide {
    Decide {
        request_id: id(request_id),
        option: Some(letter(option)),
        answer: None,
        principal: who(principal),
        principal_kind: kind,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: Vec::new(),
    }
}

fn take_by(
    request_id: &str,
    principal: &str,
    kind: PrincipalKind,
    instant: &str,
) -> TakeRecommendation {
    TakeRecommendation {
        request_id: id(request_id),
        principal: who(principal),
        principal_kind: kind,
        decided_at: at(instant),
        case_revision: Revision("r9".to_owned()),
        reasons: Vec::new(),
    }
}

fn ground() -> Ground {
    Ground {
        source: GroundSource::Story,
        citation: Citation::PathLine(PathLineCitation {
            path: "docs/merge.md".to_owned(),
            line: 3,
            line_end: None,
            digest: None,
        }),
        supports: Some(letter("A")),
        how: "Stated.".to_owned(),
    }
}

fn recommendation(request_id: &str, strength: Strength, default_on_silence: bool) -> Recommend {
    Recommend {
        request_id: id(request_id),
        recommended_option: Some(letter("A")),
        recommended_answer: None,
        strength,
        default_on_silence,
        grounds: vec![ground()],
        proposed_by: ProposerKind::Rule,
        check_verdict: CheckVerdict::Verified,
        check_report: "1 ground".to_owned(),
    }
}

fn withheld(request_id: &str) -> Recommend {
    Recommend {
        request_id: id(request_id),
        recommended_option: None,
        recommended_answer: None,
        strength: Strength::None,
        default_on_silence: false,
        grounds: Vec::new(),
        proposed_by: ProposerKind::Llm,
        check_verdict: CheckVerdict::Refused,
        check_report: "0 grounds".to_owned(),
    }
}

fn produced(record: &str) -> RecordProduced {
    RecordProduced {
        request_id: id(DR1),
        produced: ProducedRecord {
            document: ProducedDocument::CanonAuthority,
            record: record.to_owned(),
            case_revision: Revision("r9".to_owned()),
            aep_record: None,
        },
    }
}

fn escalation(request_id: &str, to: &str) -> Escalate {
    Escalate {
        request_id: id(request_id),
        escalate_to: who(to),
        reason: "Over to a person.".to_owned(),
    }
}

fn superseding(request_id: &str) -> Supersede {
    Supersede {
        request_id: id(request_id),
        superseded_by: id(DR2),
        reason: "Asked again.".to_owned(),
    }
}

fn expiry(request_id: &str, instant: &str) -> Expire {
    Expire {
        request_id: id(request_id),
        at: at(instant),
    }
}

/// An in-memory store that counts every write the commands attempt.
#[derive(Debug, Default)]
struct Counting {
    inner: InMemoryDecisionStore,
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

    fn open_for_record(&self, record_key: &str) -> Result<Option<DecisionRequestId>, StoreFailure> {
        self.inner.open_for_record(record_key)
    }

    fn append_record(&mut self, record: EscalationSuppressedRecord) -> Result<(), AppendError> {
        self.inner.append_record(record)
    }

    fn records(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Vec<EscalationSuppressedRecord>, StoreFailure> {
        self.inner.records(request_id)
    }
}

/// A store holding `DR1`, opened by `Request` with `holder` and `eligible`.
fn holding(holder: Holder, eligible: &[&str]) -> Counting {
    let mut store = Counting::default();
    let answer = request(&mut store, approval(DR1, holder, eligible)).expect("applied");
    assert!(
        matches!(answer, RequestOutcome::Requested { .. }),
        "{answer:?}"
    );
    store
}

fn accepted<T: std::fmt::Debug>(
    label: &str,
    answer: Result<T, DecisionError>,
    ok: impl FnOnce(&T) -> bool,
) {
    let answer = answer.unwrap_or_else(|error| panic!("{label}: applied: {error}"));
    assert!(
        ok(&answer),
        "{label}: setup expected acceptance, got {answer:?}"
    );
}

/// Applies a command expected to be refused: it answers the refusal `expected` names, attempts
/// no write, and leaves the store byte for byte as it was.
fn refused<T: std::fmt::Debug>(
    store: &mut Counting,
    label: &str,
    apply: impl FnOnce(&mut Counting) -> Result<T, DecisionError>,
    expected: impl FnOnce(&T) -> bool,
) {
    let before = store.inner.clone();
    store.writes = 0;
    let answer = apply(store).unwrap_or_else(|error| panic!("{label}: applied: {error}"));
    assert!(expected(&answer), "{label}: {answer:?}");
    assert_eq!(store.writes, 0, "{label}: a refusal attempted a write");
    assert_eq!(store.inner, before, "{label}: a refusal changed the store");
}

/// Applies a command expected to decide no outcome or to conflict: an error, and the store as it
/// was.
fn errs<T: std::fmt::Debug>(
    store: &mut Counting,
    label: &str,
    apply: impl FnOnce(&mut Counting) -> Result<T, DecisionError>,
    expected: impl FnOnce(&DecisionError) -> bool,
) {
    let before = store.inner.clone();
    let answer = apply(store);
    match &answer {
        Err(error) => assert!(expected(error), "{label}: {error:?}"),
        Ok(outcome) => panic!("{label}: expected an error, got {outcome:?}"),
    }
    assert_eq!(store.inner, before, "{label}: the store changed");
}

fn state(store: &Counting, request_id: &str) -> DecisionRequestState {
    store
        .read(&id(request_id))
        .expect("reads")
        .unwrap_or_else(|| panic!("{request_id} is stored"))
        .state
}

// Invariants 1 and 2: every guard of `ess/domains/decision.yaml` answers its refusal, and none
// reaches the store.
#[test]
fn security_every_guard_refusal_of_every_command_attempts_no_write() {
    use DecideOutcome as D;
    use ExpireOutcome as X;
    use RecommendOutcome as M;
    use RecordProducedOutcome as P;
    use RequestOutcome as R;
    use TakeRecommendationOutcome as T;

    // Request.
    let mut empty = Counting::default();
    refused(
        &mut empty,
        "Request escalation-is-not-a-class",
        |s| {
            let mut input = approval(DR1, Holder::Person, &["operator"]);
            input.decision_class = DecisionClassId("escalation".to_owned());
            request(s, input)
        },
        |a| matches!(a, R::EscalationIsNotAClass { .. }),
    );
    refused(
        &mut empty,
        "Request too-few-options",
        |s| {
            let mut input = approval(DR1, Holder::Person, &["operator"]);
            input.options.truncate(1);
            request(s, input)
        },
        |a| matches!(a, R::TooFewOptions { .. }),
    );
    refused(
        &mut empty,
        "Request record-key-open (named)",
        |s| {
            let mut input = approval(DR2, Holder::Person, &["operator"]);
            input.open_for_record = Some(id(DR1));
            request(s, input)
        },
        |a| matches!(a, R::RecordKeyOpen { error } if error.open_for_record == id(DR1)),
    );
    let mut open = holding(Holder::Person, &["operator"]);
    refused(
        &mut open,
        "Request record-key-open (stored)",
        |s| request(s, approval(DR2, Holder::Person, &["operator"])),
        |a| matches!(a, R::RecordKeyOpen { error } if error.open_for_record == id(DR1)),
    );

    // Recommend.
    refused(
        &mut open,
        "Recommend ungrounded",
        |s| {
            let mut input = recommendation(DR1, Strength::Strong, false);
            input.grounds.clear();
            recommend(s, input)
        },
        |a| matches!(a, M::Ungrounded { .. }),
    );
    refused(
        &mut open,
        "Recommend ungrounded (strength none)",
        |s| recommend(s, recommendation(DR1, Strength::None, false)),
        |a| matches!(a, M::Ungrounded { .. }),
    );
    refused(
        &mut open,
        "Recommend silence-needs-strong",
        |s| recommend(s, recommendation(DR1, Strength::Moderate, true)),
        |a| matches!(a, M::SilenceNeedsStrong { .. }),
    );
    refused(
        &mut open,
        "Recommend option-or-answer (both)",
        |s| {
            let mut input = recommendation(DR1, Strength::Strong, false);
            input.recommended_answer = Some("Free.".to_owned());
            recommend(s, input)
        },
        |a| matches!(a, M::OptionOrAnswer { .. }),
    );
    refused(
        &mut open,
        "Recommend option-or-answer (neither)",
        |s| {
            let mut input = recommendation(DR1, Strength::Strong, false);
            input.recommended_option = None;
            recommend(s, input)
        },
        |a| matches!(a, M::OptionOrAnswer { .. }),
    );
    refused(
        &mut open,
        "Recommend no-request",
        |s| recommend(s, recommendation(NONE, Strength::Strong, false)),
        |a| matches!(a, M::NoRequest { .. }),
    );
    refused(
        &mut open,
        "Recommend answer-not-allowed",
        |s| {
            let mut input = recommendation(DR1, Strength::Strong, false);
            input.recommended_option = None;
            input.recommended_answer = Some("Free.".to_owned());
            recommend(s, input)
        },
        |a| matches!(a, M::AnswerNotAllowed { .. }),
    );

    // Decide.
    refused(
        &mut open,
        "Decide silence-takes-recommendation",
        |s| decide(s, decide_by(DR1, "A", "operator", PrincipalKind::Silence)),
        |a| matches!(a, D::SilenceTakesRecommendation { .. }),
    );
    refused(
        &mut open,
        "Decide option-or-answer (neither)",
        |s| {
            let mut input = decide_by(DR1, "A", "operator", PrincipalKind::Person);
            input.option = None;
            decide(s, input)
        },
        |a| matches!(a, D::OptionOrAnswer { .. }),
    );
    refused(
        &mut open,
        "Decide option-or-answer (both)",
        |s| {
            let mut input = decide_by(DR1, "A", "operator", PrincipalKind::Person);
            input.answer = Some("Free.".to_owned());
            decide(s, input)
        },
        |a| matches!(a, D::OptionOrAnswer { .. }),
    );
    refused(
        &mut open,
        "Decide no-request",
        |s| decide(s, decide_by(NONE, "A", "operator", PrincipalKind::Person)),
        |a| matches!(a, D::NoRequest { .. }),
    );
    refused(
        &mut open,
        "Decide answer-not-allowed",
        |s| {
            let mut input = decide_by(DR1, "A", "operator", PrincipalKind::Person);
            input.option = None;
            input.answer = Some("Free.".to_owned());
            decide(s, input)
        },
        |a| matches!(a, D::AnswerNotAllowed { .. }),
    );
    refused(
        &mut open,
        "Decide person-only (person)",
        |s| decide(s, decide_by(DR1, "A", "operator", PrincipalKind::Decider)),
        |a| matches!(a, D::PersonOnly { .. }),
    );
    refused(
        &mut open,
        "Decide not-listed",
        |s| decide(s, decide_by(DR1, "A", "mallory", PrincipalKind::Person)),
        |a| matches!(a, D::NotListed { .. }),
    );
    let mut advised = holding(Holder::Advised, &["operator", "decider:x"]);
    refused(
        &mut advised,
        "Decide person-only (advised)",
        |s| decide(s, decide_by(DR1, "A", "decider:x", PrincipalKind::Decider)),
        |a| matches!(a, D::PersonOnly { .. }),
    );
    for holder in [Holder::Rule, Holder::RuleFinal, Holder::RuleAndDecider] {
        let mut ruled = holding(holder, &["operator", "decider:x", "rule:merge"]);
        refused(
            &mut ruled,
            &format!("Decide decider-under-rule ({holder:?})"),
            |s| decide(s, decide_by(DR1, "B", "decider:x", PrincipalKind::Decider)),
            |a| matches!(a, D::DeciderUnderRule { .. }),
        );
    }
    let mut final_rule = holding(Holder::RuleAndDeciderFinal, &["operator", "decider:x"]);
    refused(
        &mut final_rule,
        "Decide decider-under-rule (granting under R∧D*)",
        |s| decide(s, decide_by(DR1, "A", "decider:x", PrincipalKind::Decider)),
        |a| matches!(a, D::DeciderUnderRule { .. }),
    );
    for holder in [
        Holder::Decider,
        Holder::DeciderFinal,
        Holder::RuleAndDecider,
        Holder::RuleAndDeciderFinal,
    ] {
        let mut not_rule = holding(holder, &["operator", "rule:merge"]);
        refused(
            &mut not_rule,
            &format!("Decide rule-not-holder ({holder:?})"),
            |s| decide(s, decide_by(DR1, "A", "rule:merge", PrincipalKind::Rule)),
            |a| matches!(a, D::RuleNotHolder { .. }),
        );
    }

    // TakeRecommendation.
    refused(
        &mut open,
        "TakeRecommendation no-request",
        |s| {
            take_recommendation(
                s,
                take_by(
                    NONE,
                    "operator",
                    PrincipalKind::Person,
                    "2026-11-21T11:30:00Z",
                ),
            )
        },
        |a| matches!(a, T::NoRequest { .. }),
    );
    for kind in [PrincipalKind::Decider, PrincipalKind::Rule] {
        refused(
            &mut advised,
            &format!("TakeRecommendation person-only ({kind:?})"),
            |s| take_recommendation(s, take_by(DR1, "decider:x", kind, "2026-11-21T11:30:00Z")),
            |a| matches!(a, T::PersonOnly { .. }),
        );
    }
    refused(
        &mut open,
        "TakeRecommendation not-listed",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "mallory",
                    PrincipalKind::Person,
                    "2026-11-21T11:30:00Z",
                ),
            )
        },
        |a| matches!(a, T::NotListed { .. }),
    );
    refused(
        &mut open,
        "TakeRecommendation not-strong (none)",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "operator",
                    PrincipalKind::Person,
                    "2026-11-21T11:30:00Z",
                ),
            )
        },
        |a| matches!(a, T::NotStrong { .. }),
    );
    let mut moderate = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend moderate",
        recommend(
            &mut moderate,
            recommendation(DR1, Strength::Moderate, false),
        ),
        |a| matches!(a, M::Recommended { .. }),
    );
    refused(
        &mut moderate,
        "TakeRecommendation not-strong (moderate)",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "operator",
                    PrincipalKind::Person,
                    "2026-11-21T11:30:00Z",
                ),
            )
        },
        |a| matches!(a, T::NotStrong { .. }),
    );
    let mut strong = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend strong",
        recommend(&mut strong, recommendation(DR1, Strength::Strong, false)),
        |a| matches!(a, M::Recommended { .. }),
    );
    refused(
        &mut strong,
        "TakeRecommendation silence-without-default",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "silence",
                    PrincipalKind::Silence,
                    "2026-11-21T12:30:00Z",
                ),
            )
        },
        |a| matches!(a, T::SilenceWithoutDefault { .. }),
    );
    let mut silent = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend strong default",
        recommend(&mut silent, recommendation(DR1, Strength::Strong, true)),
        |a| matches!(a, M::Recommended { .. }),
    );
    refused(
        &mut silent,
        "TakeRecommendation silence-not-due (before)",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "silence",
                    PrincipalKind::Silence,
                    "2026-11-21T11:59:59Z",
                ),
            )
        },
        |a| matches!(a, T::SilenceNotDue { .. }),
    );
    errs(
        &mut silent,
        "TakeRecommendation silence at no instant",
        |s| {
            take_recommendation(
                s,
                take_by(DR1, "silence", PrincipalKind::Silence, "tomorrow"),
            )
        },
        |e| matches!(e, DecisionError::Undecided(_)),
    );

    // Expire.
    refused(
        &mut open,
        "Expire no-request",
        |s| expire(s, expiry(NONE, "2026-11-21T12:30:00Z")),
        |a| matches!(a, X::NoRequest { .. }),
    );
    refused(
        &mut open,
        "Expire not-due",
        |s| expire(s, expiry(DR1, "2026-11-21T11:59:59Z")),
        |a| matches!(a, X::NotDue { .. }),
    );
    errs(
        &mut open,
        "Expire at no instant",
        |s| expire(s, expiry(DR1, "")),
        |e| matches!(e, DecisionError::Undecided(_)),
    );
    refused(
        &mut silent,
        "Expire has-default",
        |s| expire(s, expiry(DR1, "2026-11-21T12:30:00Z")),
        |a| matches!(a, X::HasDefault { .. }),
    );
    let mut timeless = Counting::default();
    let mut input = approval(DR1, Holder::Person, &["operator"]);
    input.expires_at = None;
    accepted(
        "Request without expires_at",
        request(&mut timeless, input),
        |a| matches!(a, R::Requested { .. }),
    );
    refused(
        &mut timeless,
        "Expire not-due (no expires_at)",
        |s| expire(s, expiry(DR1, "2099-01-01T00:00:00Z")),
        |a| matches!(a, X::NotDue { .. }),
    );

    // Escalate.
    refused(
        &mut open,
        "Escalate no-request",
        |s| escalate(s, escalation(NONE, "timo")),
        |a| matches!(a, EscalateOutcome::NoRequest { .. }),
    );
    for holder in [
        Holder::DeciderFinal,
        Holder::RuleFinal,
        Holder::RuleAndDeciderFinal,
    ] {
        let mut suppressed = holding(holder, &["operator", "decider:x"]);
        refused(
            &mut suppressed,
            &format!("Escalate suppressed ({holder:?})"),
            |s| escalate(s, escalation(DR1, "timo")),
            |a| matches!(a, EscalateOutcome::Suppressed { .. }),
        );
    }

    // Supersede.
    refused(
        &mut open,
        "Supersede no-request",
        |s| supersede(s, superseding(NONE)),
        |a| matches!(a, SupersedeOutcome::NoRequest { .. }),
    );

    // RecordProduced, and the wrong-state of each command.
    refused(
        &mut open,
        "RecordProduced no-request",
        |s| {
            let mut input = produced(KEY);
            input.request_id = id(NONE);
            record_produced(s, input)
        },
        |a| matches!(a, P::NoRequest { .. }),
    );
    refused(
        &mut open,
        "RecordProduced wrong-state (Open)",
        |s| record_produced(s, produced(KEY)),
        |a| matches!(a, P::WrongState { .. }),
    );
    let mut untaken = holding(Holder::Person, &["operator"]);
    accepted(
        "Decide C",
        decide(
            &mut untaken,
            decide_by(DR1, "C", "operator", PrincipalKind::Person),
        ),
        |a| matches!(a, D::Decided { .. }),
    );
    refused(
        &mut untaken,
        "RecordProduced option-produces-none",
        |s| record_produced(s, produced(KEY)),
        |a| matches!(a, P::OptionProducesNone { .. }),
    );
    let mut decided = holding(Holder::Person, &["operator"]);
    accepted(
        "Decide A",
        decide(
            &mut decided,
            decide_by(DR1, "A", "operator", PrincipalKind::Person),
        ),
        |a| matches!(a, D::Decided { .. }),
    );
    accepted(
        "RecordProduced",
        record_produced(&mut decided, produced(KEY)),
        |a| matches!(a, P::Recorded { .. }),
    );
    refused(
        &mut decided,
        "RecordProduced already-recorded",
        |s| record_produced(s, produced(KEY)),
        |a| matches!(a, P::AlreadyRecorded),
    );
    refused(
        &mut decided,
        "RecordProduced produced-conflict",
        |s| record_produced(s, produced("change.merge@r10")),
        |a| matches!(a, P::ProducedConflict { .. }),
    );
    refused(
        &mut decided,
        "Recommend wrong-state",
        |s| recommend(s, withheld(DR1)),
        |a| matches!(a, M::WrongState { .. }),
    );
    refused(
        &mut decided,
        "Decide wrong-state",
        |s| decide(s, decide_by(DR1, "B", "operator", PrincipalKind::Person)),
        |a| matches!(a, D::WrongState { .. }),
    );
    refused(
        &mut decided,
        "Escalate wrong-state",
        |s| escalate(s, escalation(DR1, "timo")),
        |a| matches!(a, EscalateOutcome::WrongState { .. }),
    );
    refused(
        &mut decided,
        "Expire wrong-state",
        |s| expire(s, expiry(DR1, "2026-11-21T12:30:00Z")),
        |a| matches!(a, X::WrongState { .. }),
    );
    accepted(
        "TakeRecommendation strong",
        take_recommendation(
            &mut strong,
            take_by(
                DR1,
                "operator",
                PrincipalKind::Person,
                "2026-11-21T11:30:00Z",
            ),
        ),
        |a| matches!(a, T::Taken { .. }),
    );
    refused(
        &mut strong,
        "TakeRecommendation wrong-state",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "operator",
                    PrincipalKind::Person,
                    "2026-11-21T11:31:00Z",
                ),
            )
        },
        |a| matches!(a, T::WrongState { .. }),
    );
    accepted(
        "Supersede",
        supersede(&mut decided, superseding(DR1)),
        |a| matches!(a, SupersedeOutcome::Superseded { .. }),
    );
    refused(
        &mut decided,
        "Supersede wrong-state",
        |s| supersede(s, superseding(DR1)),
        |a| matches!(a, SupersedeOutcome::WrongState { .. }),
    );
}

// Invariant 5: a Decided, Expired or Superseded request accepts no further outcome from any of
// the eight commands, except the two the lifecycle declares from Decided (record_produced,
// supersede); none reopens it.
#[test]
fn security_no_command_changes_a_decided_expired_or_superseded_request() {
    // Decided, with a strong recommendation and an expires_at already passed.
    let mut decided = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend",
        recommend(&mut decided, recommendation(DR1, Strength::Strong, false)),
        |a| matches!(a, RecommendOutcome::Recommended { .. }),
    );
    accepted(
        "Decide",
        decide(
            &mut decided,
            decide_by(DR1, "A", "operator", PrincipalKind::Person),
        ),
        |a| matches!(a, DecideOutcome::Decided { .. }),
    );

    // Expired, with a strong recommendation that is no default on silence.
    let mut expired = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend",
        recommend(&mut expired, recommendation(DR1, Strength::Strong, false)),
        |a| matches!(a, RecommendOutcome::Recommended { .. }),
    );
    accepted(
        "Expire",
        expire(&mut expired, expiry(DR1, "2026-11-21T12:00:00Z")),
        |a| matches!(a, ExpireOutcome::Expired { .. }),
    );

    // Superseded after it was Decided and its record recorded.
    let mut superseded = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend",
        recommend(
            &mut superseded,
            recommendation(DR1, Strength::Strong, false),
        ),
        |a| matches!(a, RecommendOutcome::Recommended { .. }),
    );
    accepted(
        "Decide",
        decide(
            &mut superseded,
            decide_by(DR1, "A", "operator", PrincipalKind::Person),
        ),
        |a| matches!(a, DecideOutcome::Decided { .. }),
    );
    accepted(
        "RecordProduced",
        record_produced(&mut superseded, produced(KEY)),
        |a| matches!(a, RecordProducedOutcome::Recorded { .. }),
    );
    accepted(
        "Supersede",
        supersede(&mut superseded, superseding(DR1)),
        |a| matches!(a, SupersedeOutcome::Superseded { .. }),
    );

    // Superseded while Open.
    let mut withdrawn = holding(Holder::Person, &["operator"]);
    accepted(
        "Supersede",
        supersede(&mut withdrawn, superseding(DR1)),
        |a| matches!(a, SupersedeOutcome::Superseded { .. }),
    );

    for (name, store, closed) in [
        ("Decided", &mut decided, DecisionRequestState::Decided),
        ("Expired", &mut expired, DecisionRequestState::Expired),
        (
            "Superseded",
            &mut superseded,
            DecisionRequestState::Superseded,
        ),
        (
            "Superseded while Open",
            &mut withdrawn,
            DecisionRequestState::Superseded,
        ),
    ] {
        errs(
            store,
            &format!("{name}: Request under the same id"),
            |s| {
                let mut input = approval(DR1, Holder::Person, &["operator"]);
                input.record_key = "other@r1".to_owned();
                request(s, input)
            },
            |e| matches!(e, DecisionError::Conflict(_)),
        );
        refused(
            store,
            &format!("{name}: Recommend withheld"),
            |s| recommend(s, withheld(DR1)),
            |a| matches!(a, RecommendOutcome::WrongState { .. }),
        );
        refused(
            store,
            &format!("{name}: Recommend verified"),
            |s| recommend(s, recommendation(DR1, Strength::Strong, false)),
            |a| matches!(a, RecommendOutcome::WrongState { .. }),
        );
        refused(
            store,
            &format!("{name}: Decide"),
            |s| decide(s, decide_by(DR1, "B", "operator", PrincipalKind::Person)),
            |a| matches!(a, DecideOutcome::WrongState { .. }),
        );
        if name != "Superseded while Open" {
            refused(
                store,
                &format!("{name}: TakeRecommendation"),
                |s| {
                    take_recommendation(
                        s,
                        take_by(
                            DR1,
                            "operator",
                            PrincipalKind::Person,
                            "2026-11-21T12:30:00Z",
                        ),
                    )
                },
                |a| matches!(a, TakeRecommendationOutcome::WrongState { .. }),
            );
        }
        refused(
            store,
            &format!("{name}: Escalate"),
            |s| escalate(s, escalation(DR1, "timo")),
            |a| matches!(a, EscalateOutcome::WrongState { .. }),
        );
        refused(
            store,
            &format!("{name}: Expire"),
            |s| expire(s, expiry(DR1, "2026-11-21T12:30:00Z")),
            |a| matches!(a, ExpireOutcome::WrongState { .. }),
        );
        match closed {
            DecisionRequestState::Decided => {}
            _ => refused(
                store,
                &format!("{name}: Supersede"),
                |s| supersede(s, superseding(DR1)),
                |a| matches!(a, SupersedeOutcome::WrongState { .. }),
            ),
        }
        match name {
            "Superseded" => {
                refused(
                    store,
                    &format!("{name}: RecordProduced again"),
                    |s| record_produced(s, produced(KEY)),
                    |a| matches!(a, RecordProducedOutcome::AlreadyRecorded),
                );
                refused(
                    store,
                    &format!("{name}: RecordProduced another"),
                    |s| record_produced(s, produced("change.merge@r10")),
                    |a| matches!(a, RecordProducedOutcome::ProducedConflict { .. }),
                );
            }
            "Decided" => {}
            _ => refused(
                store,
                &format!("{name}: RecordProduced"),
                |s| record_produced(s, produced(KEY)),
                |a| matches!(a, RecordProducedOutcome::WrongState { .. }),
            ),
        }
        assert_eq!(state(store, DR1), closed, "{name}: the state held");
        assert_eq!(
            store.open_for_record(KEY).expect("reads"),
            None,
            "{name}: nothing reopened"
        );
    }
}

// Invariant 3: a principal named by a command does not widen who may decide; the stored eligible
// list decides.
#[test]
fn security_a_principal_named_by_a_command_is_held_to_the_stored_list() {
    let mut store = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend",
        recommend(&mut store, recommendation(DR1, Strength::Strong, false)),
        |a| matches!(a, RecommendOutcome::Recommended { .. }),
    );
    // Escalate addresses a principal the list does not hold: it moves the inbox, not the list.
    accepted(
        "Escalate to mallory",
        escalate(&mut store, escalation(DR1, "mallory")),
        |a| matches!(a, EscalateOutcome::Escalated { .. }),
    );
    let held = store.read(&id(DR1)).expect("reads").expect("stored");
    assert_eq!(
        held.data.eligible,
        vec![who("operator")],
        "Escalate leaves the eligible list"
    );
    refused(
        &mut store,
        "Decide by the addressed principal",
        |s| decide(s, decide_by(DR1, "A", "mallory", PrincipalKind::Person)),
        |a| matches!(a, DecideOutcome::NotListed { .. }),
    );
    refused(
        &mut store,
        "TakeRecommendation by the addressed principal",
        |s| {
            take_recommendation(
                s,
                take_by(
                    DR1,
                    "mallory",
                    PrincipalKind::Person,
                    "2026-11-21T11:30:00Z",
                ),
            )
        },
        |a| matches!(a, TakeRecommendationOutcome::NotListed { .. }),
    );
    for near in ["Operator", "operator ", " operator", ""] {
        refused(
            &mut store,
            &format!("Decide by {near:?}"),
            |s| decide(s, decide_by(DR1, "A", near, PrincipalKind::Person)),
            |a| matches!(a, DecideOutcome::NotListed { .. }),
        );
    }
}

// Invariant 3: the principal's kind is read from the command (`principal_kind`), not from the
// stored request or the principal. A listed decider that names itself a person decides a request
// whose holder lets a decider choose no option (`decider-under-rule`, decision.yaml:631-643).
#[test]
fn security_a_listed_decider_naming_itself_a_person_cannot_decide_under_a_rule() {
    let mut store = holding(Holder::RuleAndDecider, &["operator", "decider:haiku"]);
    let before = store.inner.clone();
    let answer = decide(
        &mut store,
        decide_by(DR1, "A", "decider:haiku", PrincipalKind::Person),
    )
    .expect("applied");
    assert!(
        !matches!(answer, DecideOutcome::Decided { .. }),
        "decider:haiku, a decider by its name, decided a rule_and_decider request by naming its \
         kind person: {answer:?}; stored {:?}",
        store.read(&id(DR1)).expect("reads").map(|held| (
            held.state,
            held.data.decided_by,
            held.data.decided_by_kind
        ))
    );
    assert_eq!(store.inner, before, "nothing was written");
}

/// A store whose record-key lookup and write are two calls, as any port's are: between them
/// another writer's `Request` for the same record key lands.
struct Racing {
    inner: RefCell<InMemoryDecisionStore>,
    competitor: RefCell<Option<Request>>,
}

impl DecisionStore for Racing {
    fn read(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Option<DecisionRequestSnapshot>, StoreFailure> {
        self.inner.borrow().read(request_id)
    }

    fn write(
        &mut self,
        read: Option<&DecisionRequestSnapshot>,
        next: DecisionRequestSnapshot,
    ) -> Result<(), WriteError> {
        self.inner.get_mut().write(read, next)
    }

    fn list(&self) -> Result<Vec<DecisionRequestSnapshot>, StoreFailure> {
        self.inner.borrow().list()
    }

    fn open_for_record(&self, record_key: &str) -> Result<Option<DecisionRequestId>, StoreFailure> {
        let answer = self.inner.borrow().open_for_record(record_key)?;
        if let Some(competing) = self.competitor.borrow_mut().take() {
            let landed = request(&mut *self.inner.borrow_mut(), competing).expect("applied");
            assert!(
                matches!(landed, RequestOutcome::Requested { .. }),
                "{landed:?}"
            );
        }
        Ok(answer)
    }

    fn append_record(&mut self, record: EscalationSuppressedRecord) -> Result<(), AppendError> {
        self.inner.get_mut().append_record(record)
    }

    fn records(
        &self,
        request_id: &DecisionRequestId,
    ) -> Result<Vec<EscalationSuppressedRecord>, StoreFailure> {
        self.inner.borrow().records(request_id)
    }
}

// Invariant 4: one Open or Escalated request per record key across the check and the write. The
// port's write is conditional on the request id only, so a second request for the key written
// between `open_for_record` and `write` is not refused.
#[test]
fn security_one_open_request_per_record_key_holds_across_the_check_and_the_write() {
    let mut store = Racing {
        inner: RefCell::new(InMemoryDecisionStore::new()),
        competitor: RefCell::new(Some(approval(DR2, Holder::Person, &["operator"]))),
    };
    let answer = request(&mut store, approval(DR1, Holder::Person, &["operator"]));
    let open: Vec<String> = store
        .list()
        .expect("lists")
        .into_iter()
        .filter(|held| {
            held.data.record_key == KEY
                && matches!(
                    held.state,
                    DecisionRequestState::Open | DecisionRequestState::Escalated
                )
        })
        .map(|held| held.data.request_id.0)
        .collect();
    assert!(
        open.len() <= 1,
        "two open requests for record key {KEY}: {open:?} (the second Request answered {answer:?})"
    );
}

// Invariant 6: the decided case revision is the one TakeRecommendation gives, kept as given (an
// empty one too: the domain has no guard on it), and no later command changes it.
#[test]
fn security_the_case_revision_taken_with_the_recommendation_is_kept() {
    let mut store = holding(Holder::Person, &["operator"]);
    accepted(
        "Recommend",
        recommend(&mut store, recommendation(DR1, Strength::Strong, false)),
        |a| matches!(a, RecommendOutcome::Recommended { .. }),
    );
    let mut take = take_by(
        DR1,
        "operator",
        PrincipalKind::Person,
        "2026-11-21T11:30:00Z",
    );
    take.case_revision = Revision(String::new());
    accepted(
        "TakeRecommendation",
        take_recommendation(&mut store, take),
        |a| matches!(a, TakeRecommendationOutcome::Taken { .. }),
    );
    let kept = |store: &Counting| {
        store
            .read(&id(DR1))
            .expect("reads")
            .expect("stored")
            .data
            .decided_case_revision
    };
    assert_eq!(kept(&store), Some(Revision(String::new())), "kept as given");
    let mut again = decide_by(DR1, "B", "operator", PrincipalKind::Person);
    again.case_revision = Revision("r10".to_owned());
    refused(
        &mut store,
        "Decide again",
        |s| decide(s, again),
        |a| matches!(a, DecideOutcome::WrongState { .. }),
    );
    accepted(
        "RecordProduced",
        record_produced(&mut store, produced(KEY)),
        |a| matches!(a, RecordProducedOutcome::Recorded { .. }),
    );
    assert_eq!(
        kept(&store),
        Some(Revision(String::new())),
        "RecordProduced keeps it"
    );
    accepted("Supersede", supersede(&mut store, superseding(DR1)), |a| {
        matches!(a, SupersedeOutcome::Superseded { .. })
    });
    assert_eq!(
        kept(&store),
        Some(Revision(String::new())),
        "Supersede keeps it"
    );
}
