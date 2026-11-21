//! Security review pass 2 of `story:decision-request-entity` (wave 2026-11-21-w7, unit U1):
//! integrity conformance checks of `kind-not-principal` (decision.yaml, `apply.rs` `kind_of`) at
//! the edges of the name form, its precedence, and the kind stored beside a decision.

use b10x_loom_decision::model::{
    ActionId, ArtifactId, AutonomyLevel, CaseId, CheckVerdict, Citation, Consequence, Decide,
    DecideOutcome, DecisionClassId, DecisionOption, DecisionRequestId, DecisionRequestState, Grant,
    Ground, GroundSource, Holder, OptionLetter, PathLineCitation, Principal, PrincipalKind,
    ProducedDocument, ProposerKind, Recommend, RecommendOutcome, Request, RequestOutcome, Revision,
    Strength, TakeRecommendation, TakeRecommendationOutcome, UnblockKind, Unblocks,
};
use b10x_loom_decision::{
    DecisionStore, InMemoryDecisionStore, Timestamp, decide, recommend, request,
    take_recommendation,
};

const DR1: &str = "DR-sec2-1";
const KINDS: [PrincipalKind; 4] = [
    PrincipalKind::Person,
    PrincipalKind::Decider,
    PrincipalKind::Rule,
    PrincipalKind::Silence,
];

/// Names at the edge of the form, each with the kind the ESS text of `kind-not-principal` gives
/// it: a byte prefix `decider:` or `rule:` (case-sensitive, untrimmed), equality with `silence`,
/// and a person otherwise.
const EDGES: [(&str, PrincipalKind); 13] = [
    ("decider:", PrincipalKind::Decider),
    ("decider:x\n", PrincipalKind::Decider),
    ("decider:rule:x", PrincipalKind::Decider),
    ("Decider:x", PrincipalKind::Person),
    (" decider:x", PrincipalKind::Person),
    ("xdecider:x", PrincipalKind::Person),
    ("rule:", PrincipalKind::Rule),
    ("rule:decider:x", PrincipalKind::Rule),
    ("RULE:x", PrincipalKind::Person),
    ("silence ", PrincipalKind::Person),
    ("Silence", PrincipalKind::Person),
    ("", PrincipalKind::Person),
    ("silence", PrincipalKind::Silence),
];

fn id(text: &str) -> DecisionRequestId {
    DecisionRequestId(text.to_owned())
}

fn at(text: &str) -> Timestamp {
    Timestamp(text.to_owned())
}

fn option(name: &str, grant: Grant) -> DecisionOption {
    DecisionOption {
        letter: OptionLetter(name.to_owned()),
        does: format!("Option {name}."),
        cost: "None.".to_owned(),
        consequence: Consequence {
            admits: Vec::new(),
            blocks: Vec::new(),
            grant: Some(grant),
            answer: None,
            produces: Some(ProducedDocument::CanonAuthority),
        },
    }
}

/// A store holding `DR1` Open under `holder`, listing every edge name and `operator`, with a
/// strong recommendation of A that is the default on silence and an `expires_at` long passed.
fn store(holder: Holder) -> InMemoryDecisionStore {
    let mut eligible: Vec<Principal> = EDGES
        .iter()
        .map(|(name, _)| Principal((*name).to_owned()))
        .collect();
    eligible.push(Principal("operator".to_owned()));
    let mut store = InMemoryDecisionStore::new();
    let answer = request(
        &mut store,
        Request {
            request_id: id(DR1),
            case_id: CaseId("IDEA-1.sec2".to_owned()),
            question_key: "IDEA-1.sec2|change.merge|".to_owned(),
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
            holder,
            eligible,
            addressed_to: Principal("operator".to_owned()),
            opened_at: at("2026-11-21T10:00:00Z"),
            expires_at: Some(at("2026-11-21T11:00:00Z")),
            supersedes: None,
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, RequestOutcome::Requested { .. }),
        "{answer:?}"
    );
    let answer = recommend(
        &mut store,
        Recommend {
            request_id: id(DR1),
            recommended_option: Some(OptionLetter("A".to_owned())),
            recommended_answer: None,
            strength: Strength::Strong,
            default_on_silence: true,
            grounds: vec![Ground {
                source: GroundSource::Story,
                citation: Citation::PathLine(PathLineCitation {
                    path: "docs/merge.md".to_owned(),
                    line: 3,
                    line_end: None,
                    digest: None,
                }),
                supports: Some(OptionLetter("A".to_owned())),
                how: "Stated.".to_owned(),
            }],
            proposed_by: ProposerKind::Rule,
            check_verdict: CheckVerdict::Verified,
            check_report: "1 ground".to_owned(),
        },
    )
    .expect("applied");
    assert!(
        matches!(answer, RecommendOutcome::Recommended { .. }),
        "{answer:?}"
    );
    store
}

fn decide_by(request_id: &str, name: &str, kind: PrincipalKind) -> Decide {
    Decide {
        request_id: id(request_id),
        option: Some(OptionLetter("A".to_owned())),
        answer: None,
        principal: Principal(name.to_owned()),
        principal_kind: kind,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: Vec::new(),
    }
}

fn take_by(request_id: &str, name: &str, kind: PrincipalKind) -> TakeRecommendation {
    TakeRecommendation {
        request_id: id(request_id),
        principal: Principal(name.to_owned()),
        principal_kind: kind,
        decided_at: at("2026-11-21T11:30:00Z"),
        case_revision: Revision("r9".to_owned()),
        reasons: Vec::new(),
    }
}

/// The kind the ESS text gives `name`, written from the text, not from `kind_of`.
fn kind_by_text(name: &str) -> PrincipalKind {
    if name.as_bytes().starts_with(b"decider:") {
        PrincipalKind::Decider
    } else if name.as_bytes().starts_with(b"rule:") {
        PrincipalKind::Rule
    } else if name == "silence" {
        PrincipalKind::Silence
    } else {
        PrincipalKind::Person
    }
}

// At each edge of the name form, on both commands and under every holder, exactly the kind the
// ESS text gives the name passes `kind-not-principal`; every other kind is refused there and
// writes nothing.
#[test]
fn security_kind_not_principal_at_the_edges_of_the_name_form_matches_the_ess_text() {
    for (name, expected) in EDGES {
        assert_eq!(
            kind_by_text(name),
            expected,
            "{name:?}: the table and the text agree"
        );
    }
    for holder in [
        Holder::Person,
        Holder::Advised,
        Holder::Decider,
        Holder::DeciderFinal,
        Holder::Rule,
        Holder::RuleFinal,
        Holder::RuleAndDecider,
        Holder::RuleAndDeciderFinal,
    ] {
        for (name, expected) in EDGES {
            for kind in KINDS {
                let mut held = store(holder);
                let before = held.clone();
                let decided = decide(&mut held, decide_by(DR1, name, kind)).expect("applied");
                let refused = matches!(decided, DecideOutcome::KindNotPrincipal { .. });
                assert_eq!(
                    refused,
                    kind != expected,
                    "Decide by {name:?} as {kind:?} under {holder:?}: {decided:?}"
                );
                if refused {
                    assert_eq!(held, before, "a refused Decide wrote");
                }
                let mut held = store(holder);
                let before = held.clone();
                let taken =
                    take_recommendation(&mut held, take_by(DR1, name, kind)).expect("applied");
                let refused = matches!(taken, TakeRecommendationOutcome::KindNotPrincipal { .. });
                assert_eq!(
                    refused,
                    kind != expected,
                    "TakeRecommendation by {name:?} as {kind:?} under {holder:?}: {taken:?}"
                );
                if refused {
                    assert_eq!(held, before, "a refused TakeRecommendation wrote");
                }
            }
        }
    }
}

// `kind-not-principal` is a guard on the command alone, declared first: it answers before the
// request is looked up, before the other input guards, and on a request in any state.
#[test]
fn security_kind_not_principal_comes_before_existence_input_guards_and_state() {
    let mut empty = InMemoryDecisionStore::new();
    let answer = decide(
        &mut empty,
        decide_by(DR1, "decider:x", PrincipalKind::Person),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::KindNotPrincipal { .. }),
        "no request: {answer:?}"
    );
    let mut both = decide_by(DR1, "operator", PrincipalKind::Silence);
    both.answer = Some("Free.".to_owned());
    let answer = decide(&mut empty, both).expect("applied");
    assert!(
        matches!(answer, DecideOutcome::KindNotPrincipal { .. }),
        "before silence and option-or-answer: {answer:?}"
    );
    assert_eq!(empty, InMemoryDecisionStore::new(), "nothing was written");

    let mut held = store(Holder::Person);
    let answer =
        decide(&mut held, decide_by(DR1, "operator", PrincipalKind::Person)).expect("applied");
    assert!(
        matches!(answer, DecideOutcome::Decided { .. }),
        "{answer:?}"
    );
    let before = held.clone();
    let answer = decide(
        &mut held,
        decide_by(DR1, "operator", PrincipalKind::Decider),
    )
    .expect("applied");
    assert!(
        matches!(answer, DecideOutcome::KindNotPrincipal { .. }),
        "Decided, Decide: {answer:?}"
    );
    let answer = take_recommendation(&mut held, take_by(DR1, "rule:x", PrincipalKind::Person))
        .expect("applied");
    assert!(
        matches!(answer, TakeRecommendationOutcome::KindNotPrincipal { .. }),
        "Decided, Take: {answer:?}"
    );
    assert_eq!(held, before, "nothing was written");
}

// After any accepted Decide or TakeRecommendation, the kind stored beside the decision is the
// kind of the stored principal's name: no command leaves `decided_by_kind` disagreeing with
// `decided_by`.
#[test]
fn security_the_stored_decided_by_kind_is_the_kind_of_the_decided_by_name() {
    let mut names: Vec<&str> = EDGES.iter().map(|(name, _)| *name).collect();
    names.push("operator");
    let mut decided = 0_u32;
    for holder in [
        Holder::Person,
        Holder::Advised,
        Holder::Decider,
        Holder::DeciderFinal,
        Holder::Rule,
        Holder::RuleFinal,
        Holder::RuleAndDecider,
        Holder::RuleAndDeciderFinal,
    ] {
        for name in &names {
            for kind in KINDS {
                for take in [false, true] {
                    let mut held = store(holder);
                    if take {
                        take_recommendation(&mut held, take_by(DR1, name, kind)).expect("applied");
                    } else {
                        decide(&mut held, decide_by(DR1, name, kind)).expect("applied");
                    }
                    let snapshot = held.read(&id(DR1)).expect("reads").expect("stored");
                    if snapshot.state == DecisionRequestState::Decided {
                        decided += 1;
                        let by = snapshot
                            .data
                            .decided_by
                            .expect("a Decided request names who");
                        assert_eq!(
                            snapshot.data.decided_by_kind,
                            Some(kind_by_text(&by.0)),
                            "{by:?} stored with kind {:?} under {holder:?}",
                            snapshot.data.decided_by_kind
                        );
                    }
                }
            }
        }
    }
    assert!(decided > 50, "{decided} decisions were reached");
}
