//! `b10x-loom-decision` applies the eight `loom.decision` commands as `ess/domains/decision.yaml`
//! declares them, over its `DecisionStore` port (`story:decision-request-entity`).
//!
//! Each test states one Acceptance line of the story. A refusal never reaches the store: every test
//! that expects one compares the whole in-memory store before and after it, and
//! [`no_sequence_of_commands_breaks_an_invariant_or_writes_on_a_refusal`] drives random sequences
//! of all eight commands over one request and checks both after every step.

use std::path::{Path, PathBuf};

use b10x_loom_decision::model::{
    ArtifactId, AutonomyLevel, CaseId, CheckVerdict, Citation, Consequence, Decide, DecideOutcome,
    DecisionClassId, DecisionOption, DecisionRequestId, DecisionRequestSnapshot,
    DecisionRequestState, Escalate, EscalateOutcome, EscalationSuppressedRecord, Expire,
    ExpireOutcome, Grant, Ground, GroundSource, Holder, NoSuchRequest, OptionLetter,
    PathLineCitation, Principal, PrincipalKind, ProducedDocument, ProducedRecord, ProposerKind,
    Recommend, RecommendOutcome, RecordKeyOpen, RecordProduced, RecordProducedOutcome, Request,
    RequestOutcome, Revision, Strength, Supersede, SupersedeOutcome, SuppressionCause,
    TakeRecommendation, TakeRecommendationOutcome, UnblockKind, Unblocks,
};
use b10x_loom_decision::{
    AppendError, DecisionError, DecisionStore, InMemoryDecisionStore, StoreConflict, Timestamp,
    WriteError, artifact_decision_history, decide, decision_requests, escalate, expire,
    open_decisions_for_principal, recommend, record_produced, request, supersede,
    take_recommendation,
};

const DR1: &str = "DR-IDEA-1.billing-change.merge-1";
const DR2: &str = "DR-IDEA-1.billing-change.merge-2";
const KEY: &str = "change.merge@r9";

fn id(text: &str) -> DecisionRequestId {
    DecisionRequestId(text.to_owned())
}

fn principal(text: &str) -> Principal {
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

/// An approval of `change.merge`: A grants, B denies, both produce an authority entry.
fn approval(request_id: &str, holder: Holder, eligible: &[&str], addressed_to: &str) -> Request {
    Request {
        request_id: id(request_id),
        case_id: CaseId("IDEA-1.billing".to_owned()),
        question_key: "IDEA-1.billing|change.merge|".to_owned(),
        record_key: KEY.to_owned(),
        open_for_record: None,
        unblocks: Unblocks {
            kind: UnblockKind::Approval,
            action: b10x_loom_decision::model::ActionId("change.merge".to_owned()),
            capability: Some(b10x_loom_decision::model::CapabilityId(
                "change.merge".to_owned(),
            )),
            question_id: None,
            outcome: None,
        },
        subject_artifact: ArtifactId("change".to_owned()),
        subject_revision: Revision("t0r1".to_owned()),
        subject_path: None,
        question: "Merge the change at revision t0r1?".to_owned(),
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
        ],
        autonomy_level: AutonomyLevel::A3,
        decision_class: DecisionClassId("merge".to_owned()),
        holder,
        eligible: eligible.iter().map(|name| principal(name)).collect(),
        addressed_to: principal(addressed_to),
        opened_at: at("2026-11-21T11:00:00Z"),
        expires_at: None,
        supersedes: None,
    }
}

fn person_approval(request_id: &str) -> Request {
    approval(request_id, Holder::Person, &["operator"], "operator")
}

fn decide_input(request_id: &str, option: &str, who: &str, kind: PrincipalKind) -> Decide {
    Decide {
        request_id: id(request_id),
        option: Some(letter(option)),
        answer: None,
        principal: principal(who),
        principal_kind: kind,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: vec!["Because.".to_owned()],
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
        how: "The story states the merge.".to_owned(),
    }
}

fn recommend_input(
    request_id: &str,
    strength: Strength,
    default_on_silence: bool,
    grounds: Vec<Ground>,
    verdict: CheckVerdict,
) -> Recommend {
    Recommend {
        request_id: id(request_id),
        recommended_option: Some(letter("A")),
        recommended_answer: None,
        strength,
        default_on_silence,
        grounds,
        proposed_by: ProposerKind::Rule,
        check_verdict: verdict,
        check_report: "1 ground, 1 resolves".to_owned(),
    }
}

fn produced(record: &str) -> ProducedRecord {
    ProducedRecord {
        document: ProducedDocument::CanonAuthority,
        record: record.to_owned(),
        case_revision: Revision("r9".to_owned()),
        aep_record: None,
    }
}

fn stored(store: &InMemoryDecisionStore, request_id: &str) -> DecisionRequestSnapshot {
    store
        .read(&id(request_id))
        .expect("the in-memory store reads")
        .unwrap_or_else(|| panic!("{request_id} is stored"))
}

fn opened(store: &mut InMemoryDecisionStore, input: Request) {
    let answer = request(store, input).expect("the request is applied");
    assert!(
        matches!(answer, RequestOutcome::Requested { .. }),
        "{answer:?}"
    );
}

#[test]
fn one_option_and_the_escalation_class_are_refused_and_store_nothing() {
    let mut store = InMemoryDecisionStore::new();

    let mut one_option = person_approval(DR1);
    one_option.options.truncate(1);
    let answer = request(&mut store, one_option).expect("applied");
    assert!(
        matches!(&answer, RequestOutcome::TooFewOptions { error } if error.request_id == id(DR1)),
        "{answer:?}"
    );

    let mut escalation = person_approval(DR1);
    escalation.decision_class = DecisionClassId("escalation".to_owned());
    let answer = request(&mut store, escalation).expect("applied");
    assert!(
        matches!(&answer, RequestOutcome::EscalationIsNotAClass { error } if error.request_id == id(DR1)),
        "{answer:?}"
    );

    assert_eq!(store, InMemoryDecisionStore::new(), "nothing was stored");
}

#[test]
fn an_open_request_for_the_record_key_refuses_another_until_it_is_decided() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let before = store.clone();

    let mut named = person_approval(DR2);
    named.open_for_record = Some(id(DR1));
    let answer = request(&mut store, named).expect("applied");
    assert_eq!(
        answer,
        RequestOutcome::RecordKeyOpen {
            error: RecordKeyOpen {
                request_id: id(DR2),
                open_for_record: id(DR1),
            },
        }
    );
    assert_eq!(store, before, "the store is unchanged");

    // Two calls in a row: the second names no open request, and the store's own answer refuses it.
    let answer = request(&mut store, person_approval(DR2)).expect("applied");
    assert!(
        matches!(&answer, RequestOutcome::RecordKeyOpen { error } if error.open_for_record == id(DR1)),
        "{answer:?}"
    );
    assert_eq!(store, before, "the store is unchanged");

    // After a reopen of the store from what it held, the rule still holds.
    let mut reopened = InMemoryDecisionStore::with_requests(store.list().expect("lists"));
    let answer = request(&mut reopened, person_approval(DR2)).expect("applied");
    assert!(
        matches!(&answer, RequestOutcome::RecordKeyOpen { error } if error.open_for_record == id(DR1)),
        "{answer:?}"
    );

    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );
    assert_eq!(store.open_for_record(KEY).expect("reads"), None);
    opened(&mut store, person_approval(DR2));
    assert_eq!(stored(&store, DR2).state, DecisionRequestState::Open);
}

#[test]
fn a_stored_request_carries_its_keys_and_a_decided_one_its_case_revision() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let held = stored(&store, DR1);
    assert_eq!(held.data.question_key, "IDEA-1.billing|change.merge|");
    assert_eq!(held.data.record_key, KEY);
    assert_eq!(held.data.decided_case_revision, None);
    assert_eq!(held.data.grounds, Vec::new(), "Request stores grounds []");
    assert_eq!(
        held.data.decision_reasons,
        Vec::<String>::new(),
        "Request stores decision_reasons []"
    );

    decide(
        &mut store,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    let held = stored(&store, DR1);
    assert_eq!(held.state, DecisionRequestState::Decided);
    assert_eq!(
        held.data.decided_case_revision,
        Some(Revision("r9".to_owned()))
    );
    assert_eq!(held.data.decided_option, Some(letter("A")));
    assert_eq!(held.data.decided_by, Some(principal("operator")));
}

#[test]
fn a_second_write_from_the_same_read_is_refused_and_the_first_is_kept() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let read = stored(&store, DR1);

    let mut first = read.clone();
    first.data.question = "first".to_owned();
    let mut second = read.clone();
    second.data.question = "second".to_owned();

    store.write(Some(&read), first.clone()).expect("kept");
    let refused = store.write(Some(&read), second).expect_err("refused");
    match refused {
        WriteError::Conflict(StoreConflict {
            request_id,
            read: held_read,
            found,
            record_key_held_by,
        }) => {
            assert_eq!(
                record_key_held_by, None,
                "a stale read names no record-key holder"
            );
            assert_eq!(request_id, id(DR1));
            assert_eq!(held_read.as_deref(), Some(&read));
            assert_eq!(found.as_deref(), Some(&first));
        }
        other @ WriteError::Failure(_) => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(stored(&store, DR1), first, "the store holds the first");

    // A new request whose id was stored since the read is refused the same way.
    let refused = store.write(None, first.clone()).expect_err("refused");
    assert!(
        matches!(&refused, WriteError::Conflict(conflict) if conflict.read.is_none() && conflict.found.is_some()),
        "{refused:?}"
    );
}

/// The port's own rule, with no command: a write that would leave two Open or Escalated requests
/// with one `record_key` is a conflict, and nothing is written.
#[test]
fn the_store_refuses_a_second_open_request_for_a_record_key() {
    let mut scratch = InMemoryDecisionStore::new();
    opened(&mut scratch, person_approval(DR1));
    let first = stored(&scratch, DR1);
    assert_eq!(first.state, DecisionRequestState::Open);
    let mut second = first.clone();
    second.data.request_id = id(DR2);

    let mut store = InMemoryDecisionStore::new();
    store.write(None, first.clone()).expect("the first is kept");
    let refused = store.write(None, second.clone());
    assert!(
        matches!(&refused, Err(WriteError::Conflict(conflict)) if conflict.request_id == id(DR2)),
        "a second open request for {KEY} was written: {refused:?}"
    );
    assert_eq!(
        store.list().expect("reads"),
        vec![first.clone()],
        "nothing is written"
    );

    let mut escalated = second.clone();
    escalated.state = DecisionRequestState::Escalated;
    assert!(
        matches!(store.write(None, escalated), Err(WriteError::Conflict(_))),
        "an Escalated request for the key counts as open too"
    );

    // Another key, or the first request no longer open, leaves the write free.
    let mut other_key = second.clone();
    other_key.data.record_key = "change.merge@r10".to_owned();
    store.write(None, other_key).expect("another key is kept");
    let mut expired = first.clone();
    expired.state = DecisionRequestState::Expired;
    store
        .write(Some(&first), expired)
        .expect("the first expires");
    let mut third = first.clone();
    third.data.request_id = id("DR-IDEA-1.billing-change.merge-3");
    store
        .write(None, third)
        .expect("a new open request for the key once the first is closed");
}

#[test]
fn recommend_withheld_clears_the_recommendation_and_verified_ones_are_checked() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let answer = recommend(
        &mut store,
        recommend_input(
            DR1,
            Strength::Strong,
            false,
            vec![ground()],
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Recommended { .. }),
        "{answer:?}"
    );
    assert_eq!(
        stored(&store, DR1).data.recommended_option,
        Some(letter("A"))
    );

    let mut withheld = recommend_input(
        DR1,
        Strength::None,
        false,
        Vec::new(),
        CheckVerdict::Refused,
    );
    withheld.check_report = "citation 1 does not resolve".to_owned();
    let answer = recommend(&mut store, withheld).expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Withheld { .. }),
        "{answer:?}"
    );
    let held = stored(&store, DR1).data;
    assert_eq!(held.recommended_option, None);
    assert_eq!(held.recommended_answer, None);
    assert_eq!(held.recommended_by, None);
    assert_eq!(held.grounding_report, None);
    assert_eq!(held.recommendation_strength, Strength::None);
    assert!(!held.default_on_silence);
    assert_eq!(
        held.grounds,
        Vec::new(),
        "a withheld Recommend stores grounds []"
    );
    assert_eq!(
        held.no_recommendation_reason.as_deref(),
        Some("citation 1 does not resolve")
    );

    let before = store.clone();
    let answer = recommend(
        &mut store,
        recommend_input(
            DR1,
            Strength::Strong,
            false,
            Vec::new(),
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Ungrounded { .. }),
        "{answer:?}"
    );
    let answer = recommend(
        &mut store,
        recommend_input(
            DR1,
            Strength::None,
            false,
            vec![ground()],
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Ungrounded { .. }),
        "{answer:?}"
    );
    let answer = recommend(
        &mut store,
        recommend_input(
            DR1,
            Strength::Moderate,
            true,
            vec![ground()],
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::SilenceNeedsStrong { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before, "no refusal reached the store");
}

#[test]
fn under_rule_and_decider_final_a_decider_takes_only_the_denying_option() {
    let mut store = InMemoryDecisionStore::new();
    opened(
        &mut store,
        approval(
            DR1,
            Holder::RuleAndDeciderFinal,
            &["decider:x"],
            "decider:x",
        ),
    );
    let before = store.clone();

    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "decider:x", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(&answer, DecideOutcome::DeciderUnderRule { error } if error.principal == principal("decider:x")),
        "{answer:?}"
    );
    assert_eq!(store, before, "a refused grant stores nothing");

    let answer = decide(
        &mut store,
        decide_input(DR1, "B", "decider:x", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );
    assert_eq!(stored(&store, DR1).data.decided_option, Some(letter("B")));
}

#[test]
fn an_escalate_hands_the_request_to_a_person_and_a_decider_can_no_longer_decide() {
    let mut store = InMemoryDecisionStore::new();
    opened(
        &mut store,
        approval(
            DR1,
            Holder::Decider,
            &["decider:x", "timo", "ana"],
            "decider:x",
        ),
    );

    let answer = escalate(
        &mut store,
        Escalate {
            request_id: id(DR1),
            escalate_to: principal("timo"),
            reason: "The refund path changed.".to_owned(),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, EscalateOutcome::Escalated { .. }),
        "{answer:?}"
    );
    let held = stored(&store, DR1);
    assert_eq!(held.state, DecisionRequestState::Escalated);
    assert_eq!(held.data.holder, Holder::Person);
    assert_eq!(held.data.addressed_to, principal("timo"));

    let before = store.clone();
    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "decider:x", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::PersonOnly { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before);

    escalate(
        &mut store,
        Escalate {
            request_id: id(DR1),
            escalate_to: principal("ana"),
            reason: "Ana owns billing.".to_owned(),
        },
    )
    .expect("applied");
    let held = stored(&store, DR1);
    assert_eq!(held.state, DecisionRequestState::Escalated);
    assert_eq!(held.data.addressed_to, principal("ana"));
    assert_eq!(held.data.escalated_to, Some(principal("ana")));

    let mut store = InMemoryDecisionStore::new();
    opened(
        &mut store,
        approval(DR1, Holder::RuleFinal, &["rule:merge"], "rule:merge"),
    );
    let before = store.clone();
    let answer = escalate(
        &mut store,
        Escalate {
            request_id: id(DR1),
            escalate_to: principal("timo"),
            reason: "No.".to_owned(),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, EscalateOutcome::Suppressed { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before, "a suppressed escalation changes nothing");
}

#[test]
fn a_produced_record_is_recorded_once_and_another_is_refused() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    decide(
        &mut store,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    let held = stored(&store, DR1);
    assert_eq!(held.data.produced_record, None);
    assert_eq!(
        held.data.decided_case_revision,
        Some(Revision("r9".to_owned()))
    );

    let answer = record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced(KEY),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, RecordProducedOutcome::Recorded { .. }),
        "{answer:?}"
    );
    assert_eq!(
        stored(&store, DR1).data.produced_record,
        Some(produced(KEY))
    );

    let before = store.clone();
    let answer = record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced(KEY),
        },
    )
    .expect("applied");
    assert_eq!(answer, RecordProducedOutcome::AlreadyRecorded);
    assert_eq!(
        format!("{store:?}"),
        format!("{before:?}"),
        "no byte changed"
    );

    let answer = record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced("change.merge@r10"),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, RecordProducedOutcome::ProducedConflict { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before, "the first record stands");
}

#[test]
fn leaving_an_outcome_untaken_produces_no_record() {
    let mut store = InMemoryDecisionStore::new();
    let mut outcome = person_approval(DR1);
    outcome.unblocks.kind = UnblockKind::Outcome;
    outcome.options = vec![
        option("A", None, Some(ProducedDocument::CanonDecisions)),
        option("B", None, None),
    ];
    opened(&mut store, outcome);
    let answer = decide(
        &mut store,
        decide_input(DR1, "B", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );

    let before = store.clone();
    let answer = record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced("merge/explicit@r9"),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, RecordProducedOutcome::OptionProducesNone { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before);
}

fn take_by_silence(request_id: &str, instant: &str) -> TakeRecommendation {
    TakeRecommendation {
        request_id: id(request_id),
        principal: principal("silence"),
        principal_kind: PrincipalKind::Silence,
        decided_at: at(instant),
        case_revision: Revision("r9".to_owned()),
        reasons: Vec::new(),
    }
}

#[test]
fn silence_takes_nothing_before_expires_at_or_without_one() {
    for expires_at in [Some("2026-11-21T12:00:00Z"), None] {
        let mut store = InMemoryDecisionStore::new();
        let mut input = person_approval(DR1);
        input.expires_at = expires_at.map(at);
        opened(&mut store, input);
        recommend(
            &mut store,
            recommend_input(
                DR1,
                Strength::Strong,
                true,
                vec![ground()],
                CheckVerdict::Verified,
            ),
        )
        .expect("applied");
        let before = store.clone();

        let instants: &[&str] = match expires_at {
            Some(_) => &["2026-11-21T11:59:00Z"],
            None => &["2026-11-21T11:59:00Z", "2030-01-01T00:00:00Z"],
        };
        for instant in instants {
            let answer =
                take_recommendation(&mut store, take_by_silence(DR1, instant)).expect("applied");
            assert!(
                matches!(answer, TakeRecommendationOutcome::SilenceNotDue { .. }),
                "{expires_at:?} at {instant}: {answer:?}"
            );
            assert_eq!(store, before);
        }
    }
}

#[test]
fn expire_answers_only_at_or_after_expires_at() {
    let mut store = InMemoryDecisionStore::new();
    let mut input = person_approval(DR1);
    input.expires_at = Some(at("2026-11-21T12:00:00Z"));
    opened(&mut store, input);
    let before = store.clone();

    let answer = expire(
        &mut store,
        Expire {
            request_id: id(DR1),
            at: at("2026-11-21T11:59:00Z"),
        },
    )
    .expect("applied");
    assert!(matches!(answer, ExpireOutcome::NotDue { .. }), "{answer:?}");
    assert_eq!(store, before);

    let answer = expire(
        &mut store,
        Expire {
            request_id: id(DR1),
            at: at("2026-11-21T12:00:00Z"),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, ExpireOutcome::Expired { .. }),
        "{answer:?}"
    );
    assert_eq!(stored(&store, DR1).state, DecisionRequestState::Expired);

    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let before = store.clone();
    for instant in ["2026-11-21T12:00:00Z", "2030-01-01T00:00:00Z"] {
        let answer = expire(
            &mut store,
            Expire {
                request_id: id(DR1),
                at: at(instant),
            },
        )
        .expect("applied");
        assert!(matches!(answer, ExpireOutcome::NotDue { .. }), "{answer:?}");
    }
    assert_eq!(store, before);
}

#[test]
fn an_instant_with_an_offset_is_compared_as_the_instant_it_names() {
    let mut store = InMemoryDecisionStore::new();
    let mut input = person_approval(DR1);
    input.expires_at = Some(at("2026-11-21T12:00:00Z"));
    opened(&mut store, input);
    // 12:30+01:00 is 11:30Z, before expires_at, though its text sorts after it.
    let answer = expire(
        &mut store,
        Expire {
            request_id: id(DR1),
            at: at("2026-11-21T12:30:00+01:00"),
        },
    )
    .expect("applied");
    assert!(matches!(answer, ExpireOutcome::NotDue { .. }), "{answer:?}");

    let before = store.clone();
    let refused = expire(
        &mut store,
        Expire {
            request_id: id(DR1),
            at: at("noon"),
        },
    )
    .expect_err("an instant that is not RFC 3339 decides no outcome");
    assert!(
        matches!(refused, DecisionError::Undecided(_)),
        "{refused:?}"
    );
    assert_eq!(store, before);
}

#[test]
fn superseding_a_decided_request_keeps_its_decision_and_record() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    decide(
        &mut store,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced(KEY),
        },
    )
    .expect("applied");
    let answer = supersede(
        &mut store,
        Supersede {
            request_id: id(DR1),
            superseded_by: id(DR2),
            reason: "The case moved.".to_owned(),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, SupersedeOutcome::Superseded { .. }),
        "{answer:?}"
    );
    let held = stored(&store, DR1);
    assert_eq!(held.state, DecisionRequestState::Superseded);
    assert_eq!(held.data.decided_option, Some(letter("A")));
    assert_eq!(held.data.produced_record, Some(produced(KEY)));
    assert_eq!(held.data.superseded_by, Some(id(DR2)));
}

#[test]
fn a_principal_the_stored_request_does_not_list_cannot_decide() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    let before = store.clone();
    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "mallory", PrincipalKind::Person),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::NotListed { .. }),
        "{answer:?}"
    );
    assert_eq!(store, before, "a refused Decide writes nothing");
    assert_eq!(stored(&store, DR1).state, DecisionRequestState::Open);
}

fn take_by(request_id: &str, who: &str, kind: PrincipalKind) -> TakeRecommendation {
    TakeRecommendation {
        request_id: id(request_id),
        principal: principal(who),
        principal_kind: kind,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: vec!["Because.".to_owned()],
    }
}

/// A store with `DR1` Open under `holder`, listing a person, `decider:model`, `rule:merge` and
/// silence, with a strong recommendation of A that is the default on silence.
fn kind_store(holder: Holder) -> InMemoryDecisionStore {
    let mut store = InMemoryDecisionStore::new();
    let mut input = approval(
        DR1,
        holder,
        &["operator", "decider:model", "rule:merge", "silence"],
        "operator",
    );
    input.expires_at = Some(at("2026-11-21T11:00:00Z"));
    opened(&mut store, input);
    let answer = recommend(
        &mut store,
        recommend_input(
            DR1,
            Strength::Strong,
            true,
            vec![ground()],
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Recommended { .. }),
        "{answer:?}"
    );
    store
}

/// The kind a command claims must be the kind its principal's name has (decision.yaml
/// `kind-not-principal`, security review F1): each disagreement is refused on both commands,
/// whatever the holder, and nothing is written.
#[test]
fn a_principal_kind_its_name_does_not_have_is_refused_on_both_commands() {
    let disagreements = [
        ("operator", PrincipalKind::Decider),
        ("operator", PrincipalKind::Rule),
        ("silence", PrincipalKind::Person),
        ("decider:model", PrincipalKind::Person),
        ("rule:merge", PrincipalKind::Decider),
        ("operator", PrincipalKind::Silence),
        ("decider:model", PrincipalKind::Rule),
    ];
    for holder in [Holder::Person, Holder::Decider, Holder::RuleAndDecider] {
        for (who, kind) in disagreements {
            let mut store = kind_store(holder);
            let before = store.clone();
            let answer = decide(&mut store, decide_input(DR1, "A", who, kind)).expect("applied");
            assert!(
                matches!(answer, DecideOutcome::KindNotPrincipal { .. }),
                "Decide by {who} as {kind:?} under {holder:?}: {answer:?}"
            );
            let answer = take_recommendation(&mut store, take_by(DR1, who, kind)).expect("applied");
            assert!(
                matches!(answer, TakeRecommendationOutcome::KindNotPrincipal { .. }),
                "TakeRecommendation by {who} as {kind:?} under {holder:?}: {answer:?}"
            );
            assert_eq!(store, before, "nothing is written");
        }
    }
    // The guard reads the command alone: it refuses before the request is looked up.
    let mut empty = InMemoryDecisionStore::new();
    let answer = take_recommendation(&mut empty, take_by(DR1, "operator", PrincipalKind::Decider))
        .expect("applied");
    assert!(
        matches!(answer, TakeRecommendationOutcome::KindNotPrincipal { .. }),
        "{answer:?}"
    );
}

/// A truthful `decider:model` with kind `decider` is decided or refused as before the guard.
#[test]
fn a_truthful_decider_model_is_decided_or_refused_as_before() {
    let mut store = kind_store(Holder::Decider);
    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "decider:model", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );
    assert_eq!(
        stored(&store, DR1).data.decided_by_kind,
        Some(PrincipalKind::Decider)
    );

    let mut store = kind_store(Holder::RuleAndDecider);
    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "decider:model", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::DeciderUnderRule { .. }),
        "{answer:?}"
    );
    let answer = take_recommendation(
        &mut store,
        take_by(DR1, "decider:model", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, TakeRecommendationOutcome::Taken { .. }),
        "{answer:?}"
    );

    let mut store = kind_store(Holder::Person);
    let answer = decide(
        &mut store,
        decide_input(DR1, "A", "decider:model", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::PersonOnly { .. }),
        "{answer:?}"
    );
    let answer = take_recommendation(&mut store, take_by(DR1, "silence", PrincipalKind::Silence))
        .expect("applied");
    assert!(
        matches!(answer, TakeRecommendationOutcome::Taken { .. }),
        "{answer:?}"
    );
}

#[test]
fn the_decided_case_revision_is_stored_once_and_no_later_command_changes_it() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    decide(
        &mut store,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");
    let r9 = Some(Revision("r9".to_owned()));
    assert_eq!(stored(&store, DR1).data.decided_case_revision, r9);

    let mut again = decide_input(DR1, "B", "operator", PrincipalKind::Person);
    again.case_revision = Revision("r10".to_owned());
    let answer = decide(&mut store, again).expect("applied");
    assert!(
        matches!(answer, DecideOutcome::WrongState { .. }),
        "{answer:?}"
    );
    record_produced(
        &mut store,
        RecordProduced {
            request_id: id(DR1),
            produced: produced(KEY),
        },
    )
    .expect("applied");
    assert_eq!(stored(&store, DR1).data.decided_case_revision, r9);
    supersede(
        &mut store,
        Supersede {
            request_id: id(DR1),
            superseded_by: id(DR2),
            reason: "The case moved.".to_owned(),
        },
    )
    .expect("applied");
    assert_eq!(stored(&store, DR1).data.decided_case_revision, r9);
}

#[test]
fn a_decided_expired_or_superseded_request_accepts_no_further_outcome() {
    let mut decided = InMemoryDecisionStore::new();
    opened(&mut decided, person_approval(DR1));
    recommend(
        &mut decided,
        recommend_input(
            DR1,
            Strength::Strong,
            false,
            vec![ground()],
            CheckVerdict::Verified,
        ),
    )
    .expect("applied");
    decide(
        &mut decided,
        decide_input(DR1, "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");

    let mut expired = InMemoryDecisionStore::new();
    let mut input = person_approval(DR1);
    input.expires_at = Some(at("2026-11-21T12:00:00Z"));
    opened(&mut expired, input);
    expire(
        &mut expired,
        Expire {
            request_id: id(DR1),
            at: at("2026-11-21T12:00:00Z"),
        },
    )
    .expect("applied");

    let mut superseded = InMemoryDecisionStore::new();
    opened(&mut superseded, person_approval(DR1));
    supersede(
        &mut superseded,
        Supersede {
            request_id: id(DR1),
            superseded_by: id(DR2),
            reason: "Asked again.".to_owned(),
        },
    )
    .expect("applied");

    for (name, mut store) in [
        ("Decided", decided),
        ("Expired", expired),
        ("Superseded", superseded),
    ] {
        let before = store.clone();
        let answer = decide(
            &mut store,
            decide_input(DR1, "B", "operator", PrincipalKind::Person),
        )
        .expect("applied");
        assert!(
            matches!(answer, DecideOutcome::WrongState { .. }),
            "{name}: {answer:?}"
        );
        let answer = take_recommendation(
            &mut store,
            TakeRecommendation {
                request_id: id(DR1),
                principal: principal("operator"),
                principal_kind: PrincipalKind::Person,
                decided_at: at("2026-11-21T12:10:00Z"),
                case_revision: Revision("r9".to_owned()),
                reasons: Vec::new(),
            },
        )
        .expect("applied");
        assert!(
            matches!(
                answer,
                TakeRecommendationOutcome::WrongState { .. }
                    | TakeRecommendationOutcome::NotStrong { .. }
            ),
            "{name}: {answer:?}"
        );
        let answer = escalate(
            &mut store,
            Escalate {
                request_id: id(DR1),
                escalate_to: principal("timo"),
                reason: "Again.".to_owned(),
            },
        )
        .expect("applied");
        assert!(
            matches!(answer, EscalateOutcome::WrongState { .. }),
            "{name}: {answer:?}"
        );
        assert_eq!(store, before, "{name}: nothing changed");
    }
}

#[test]
fn records_beside_a_request_are_read_back_in_the_order_appended() {
    let mut store = InMemoryDecisionStore::new();
    opened(&mut store, person_approval(DR1));
    opened(&mut store, {
        let mut other = person_approval(DR2);
        other.record_key = "change.merge@r10".to_owned();
        other
    });
    let record = |cause, kept_at: &str| EscalationSuppressedRecord {
        request_id: id(DR1),
        decision_class: DecisionClassId("merge".to_owned()),
        subject_artifact: ArtifactId("change".to_owned()),
        subject_revision: Revision("t0r1".to_owned()),
        reasons: vec!["Suppressed.".to_owned()],
        best: Some(letter("B")),
        cause,
        kept_at: at(kept_at),
    };
    let first = record(SuppressionCause::Escalate, "2026-11-21T12:00:00Z");
    let second = record(SuppressionCause::Failure, "2026-11-21T12:01:00Z");
    store.append_record(first.clone()).expect("appended");
    store.append_record(second.clone()).expect("appended");
    assert_eq!(store.records(&id(DR1)).expect("reads"), vec![first, second]);
    assert_eq!(store.records(&id(DR2)).expect("reads"), Vec::new());

    let mut unknown = record(SuppressionCause::Veto, "2026-11-21T12:02:00Z");
    unknown.request_id = id("DR-unknown");
    let refused = store.append_record(unknown).expect_err("refused");
    assert_eq!(
        refused,
        AppendError::NoSuchRequest(NoSuchRequest {
            request_id: id("DR-unknown"),
        })
    );
}

#[test]
fn open_decisions_for_a_principal_lists_its_open_and_escalated_requests_only() {
    let mut store = InMemoryDecisionStore::new();
    let mut keyed = |request_id: &str, key: &str, addressed_to: &str| {
        let mut input = approval(
            request_id,
            Holder::Person,
            &["operator", "ana"],
            addressed_to,
        );
        input.record_key = key.to_owned();
        opened(&mut store, input);
    };
    keyed("DR-open", "k1", "operator");
    keyed("DR-escalated", "k2", "ana");
    keyed("DR-decided", "k3", "operator");
    keyed("DR-other", "k4", "ana");
    escalate(
        &mut store,
        Escalate {
            request_id: id("DR-escalated"),
            escalate_to: principal("operator"),
            reason: "Operator decides.".to_owned(),
        },
    )
    .expect("applied");
    decide(
        &mut store,
        decide_input("DR-decided", "A", "operator", PrincipalKind::Person),
    )
    .expect("applied");

    let listed: Vec<String> = open_decisions_for_principal(&store, &principal("operator"))
        .expect("reads")
        .into_iter()
        .map(|row| row.request_id.0)
        .collect();
    assert_eq!(listed, ["DR-open", "DR-escalated"]);

    let history =
        artifact_decision_history(&store, &ArtifactId("change".to_owned())).expect("reads");
    assert_eq!(history.len(), 4);
    let all = decision_requests(&store).expect("reads");
    assert_eq!(all.len(), 4);
    assert_eq!(
        all[2].decided_case_revision,
        Some(Revision("r9".to_owned()))
    );
    assert_eq!(all[0].record_key, "k1");
}

/// A deterministic generator, so a failing sequence is reproduced by its seed.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).expect("fits")]
    }

    fn flip(&mut self) -> bool {
        self.below(2) == 0
    }
}

const STRENGTHS: [Strength; 4] = [
    Strength::None,
    Strength::Weak,
    Strength::Moderate,
    Strength::Strong,
];
const KINDS: [PrincipalKind; 4] = [
    PrincipalKind::Person,
    PrincipalKind::Decider,
    PrincipalKind::Rule,
    PrincipalKind::Silence,
];
const WHO: [&str; 4] = ["operator", "decider:x", "rule:merge", "silence"];
const INSTANTS: [&str; 3] = [
    "2026-11-21T11:59:00Z",
    "2026-11-21T12:00:00Z",
    "2026-11-21T12:01:00Z",
];

/// Applies one random command to `DR1` and answers whether its outcome was a refusal.
fn random_step(store: &mut InMemoryDecisionStore, draw: &mut Draw) -> bool {
    let option = |draw: &mut Draw| {
        if draw.flip() {
            Some(letter(draw.pick(&["A", "B", "C"])))
        } else {
            None
        }
    };
    let answer = |draw: &mut Draw| {
        if draw.flip() {
            Some("Free answer.".to_owned())
        } else {
            None
        }
    };
    match draw.below(8) {
        0 => {
            let mut input = approval(
                DR1,
                Holder::Person,
                &["operator", "decider:x", "rule:merge"],
                "operator",
            );
            input.open_for_record = if draw.flip() { Some(id(DR2)) } else { None };
            // A Request for an id the store already holds in a state with no open record key
            // writes from no read, and the store refuses it as a conflict: nothing is written.
            match request(store, input) {
                Ok(RequestOutcome::Requested { .. }) => false,
                Ok(_) | Err(DecisionError::Conflict(_)) => true,
                Err(other) => panic!("applied: {other:?}"),
            }
        }
        1 => {
            let verdict = if draw.flip() {
                CheckVerdict::Verified
            } else {
                CheckVerdict::Refused
            };
            let input = Recommend {
                request_id: id(DR1),
                recommended_option: option(draw),
                recommended_answer: answer(draw),
                strength: *draw.pick(&STRENGTHS),
                default_on_silence: draw.flip(),
                grounds: if draw.flip() {
                    vec![ground()]
                } else {
                    Vec::new()
                },
                proposed_by: ProposerKind::Llm,
                check_verdict: verdict,
                check_report: "report".to_owned(),
            };
            !matches!(
                recommend(store, input).expect("applied"),
                RecommendOutcome::Recommended { .. } | RecommendOutcome::Withheld { .. }
            )
        }
        2 => {
            let input = Decide {
                request_id: id(DR1),
                option: option(draw),
                answer: answer(draw),
                principal: principal(draw.pick(&WHO)),
                principal_kind: *draw.pick(&KINDS),
                decided_at: at(draw.pick(&INSTANTS)),
                case_revision: Revision("r9".to_owned()),
                reasons: Vec::new(),
            };
            !matches!(
                decide(store, input).expect("applied"),
                DecideOutcome::Decided { .. }
            )
        }
        3 => {
            let input = TakeRecommendation {
                request_id: id(DR1),
                principal: principal(draw.pick(&WHO)),
                principal_kind: *draw.pick(&KINDS),
                decided_at: at(draw.pick(&INSTANTS)),
                case_revision: Revision("r9".to_owned()),
                reasons: Vec::new(),
            };
            !matches!(
                take_recommendation(store, input).expect("applied"),
                TakeRecommendationOutcome::Taken { .. }
            )
        }
        4 => {
            let input = RecordProduced {
                request_id: id(DR1),
                produced: produced(draw.pick(&[KEY, "change.merge@r10"])),
            };
            !matches!(
                record_produced(store, input).expect("applied"),
                RecordProducedOutcome::Recorded { .. } | RecordProducedOutcome::AlreadyRecorded
            )
        }
        5 => {
            let input = Escalate {
                request_id: id(DR1),
                escalate_to: principal(draw.pick(&WHO)),
                reason: "reason".to_owned(),
            };
            !matches!(
                escalate(store, input).expect("applied"),
                EscalateOutcome::Escalated { .. }
            )
        }
        6 => {
            let input = Supersede {
                request_id: id(DR1),
                superseded_by: id(DR2),
                reason: "reason".to_owned(),
            };
            !matches!(
                supersede(store, input).expect("applied"),
                SupersedeOutcome::Superseded { .. }
            )
        }
        _ => {
            let input = Expire {
                request_id: id(DR1),
                at: at(draw.pick(&INSTANTS)),
            };
            !matches!(
                expire(store, input).expect("applied"),
                ExpireOutcome::Expired { .. }
            )
        }
    }
}

#[test]
fn no_sequence_of_commands_breaks_an_invariant_or_writes_on_a_refusal() {
    let mut refusals = 0_u32;
    let mut accepted = 0_u32;
    for seed in 1..=1000_u64 {
        let mut draw = Draw(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut store = InMemoryDecisionStore::new();
        let mut input = approval(
            DR1,
            Holder::Person,
            &["operator", "decider:x", "rule:merge"],
            "operator",
        );
        input.holder = *draw.pick(&[
            Holder::Person,
            Holder::Advised,
            Holder::Decider,
            Holder::DeciderFinal,
            Holder::Rule,
            Holder::RuleFinal,
            Holder::RuleAndDecider,
            Holder::RuleAndDeciderFinal,
        ]);
        input.expires_at = draw.flip().then(|| at("2026-11-21T12:00:00Z"));
        opened(&mut store, input);
        for step in 0..40 {
            let before = store.clone();
            let refused = random_step(&mut store, &mut draw);
            if refused {
                refusals += 1;
                assert_eq!(store, before, "seed {seed}, step {step}: a refusal wrote");
            } else {
                accepted += 1;
            }
            let held = stored(&store, DR1).data;
            assert_eq!(
                held.broken_invariant(),
                None,
                "seed {seed}, step {step}: {held:?}"
            );
            assert!(
                held.recommended_option.is_none() || held.no_recommendation_reason.is_none(),
                "seed {seed}, step {step}: decision.yaml:275"
            );
            assert!(
                !held.default_on_silence || held.recommendation_strength == Strength::Strong,
                "seed {seed}, step {step}: decision.yaml:277"
            );
        }
    }
    assert!(
        refusals > 1000 && accepted > 1000,
        "{refusals} refused, {accepted} accepted"
    );
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = repository().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The declaration of type `name` in `ess/domains/decision.yaml`: its lines up to the next
/// declaration.
fn declaration(domain: &str, name: &str) -> Vec<String> {
    let start = format!("  - name: {name}");
    let mut lines = domain.lines().skip_while(|line| *line != start);
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("ess/domains/decision.yaml declares {name}"));
    std::iter::once(first)
        .chain(lines.take_while(|line| !line.starts_with("  - name: ") && !line.is_empty()))
        .map(ToOwned::to_owned)
        .collect()
}

fn variants(declaration: &[String]) -> Vec<String> {
    let line = declaration
        .iter()
        .find_map(|line| line.trim().strip_prefix("variants: "))
        .unwrap_or_else(|| panic!("{declaration:?} lists its variants on one line"));
    line.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|variant| variant.trim().to_owned())
        .collect()
}

#[test]
fn strength_and_autonomy_level_keep_the_sdc_order_at_ess_37() {
    let domain = read("ess/domains/decision.yaml");
    // `~/beyond10x/sdc/ess/domains/decision.yaml:110` and `:172` at 00e3557.
    for (name, expected) in [
        (
            "loom.decision.Strength",
            &["none", "weak", "moderate", "strong"][..],
        ),
        (
            "loom.decision.AutonomyLevel",
            &["A0", "A1", "A2", "A3", "A4", "A5"][..],
        ),
    ] {
        let declared = declaration(&domain, name);
        assert!(
            declared.iter().any(|line| line.trim() == "ordered: true"),
            "{name} carries ordered: true: {declared:?}"
        );
        assert_eq!(variants(&declared), expected, "{name}'s variants in order");
    }
    let format = read("ess/system.yaml").lines().find_map(|line| {
        line.strip_prefix("format: ")
            .map(str::trim)
            .map(ToOwned::to_owned)
    });
    assert_eq!(format.as_deref(), Some("ess/37"));
}

#[test]
fn the_manifest_names_no_commission_canon_executor_model_or_mandate_crate() {
    let manifest = read("crates/loom-decision/Cargo.toml");
    let mut section = String::new();
    let mut dependencies = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.to_owned();
        } else if section.contains("dependencies")
            && let Some((name, _)) = line.split_once('=')
        {
            dependencies.push(name.trim().to_owned());
        }
    }
    assert_eq!(dependencies, ["loom"], "the generated loom crate only");
    for name in &dependencies {
        for forbidden in ["commission", "canon", "executor", "llm", "mandate"] {
            assert!(!name.contains(forbidden), "{name} names {forbidden}");
        }
    }
}

#[test]
fn no_source_of_the_crate_reads_a_file_or_runs_a_program() {
    let dir = repository().join("crates/loom-decision/src");
    for entry in std::fs::read_dir(&dir).expect("src is a directory") {
        let path = entry.expect("an entry").path();
        let source = std::fs::read_to_string(&path).expect("a source file");
        for forbidden in [
            "std::fs",
            "std::process",
            "std::net",
            "Command::new",
            "std::env",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} uses {forbidden}: no command text is a path or a program",
                path.display()
            );
        }
    }
}
