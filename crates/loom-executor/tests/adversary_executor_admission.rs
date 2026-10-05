//! Adversary pass 1 on `story:agent-executor`: Loom's `AgentExecutor` against Commission's own
//! frontier contract.
//!
//! Commission states the invariant (`docs/contracts/frontier.md`, commission `174bf07`): "An
//! executor may not invoke an action absent from the current frontier/admissible set", and sorts a
//! proposal with `b10x_loom_commission::admission::admit`, where any `Blocked` entry refuses, an
//! `ApprovalRequired` entry without a capability refuses, and conflicting capabilities refuse,
//! whatever the order of the entries. These cases drive `Loom::run` with frontiers that contract
//! describes and compare what it returns with what Commission decides for the same frontier.

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, Admission, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, CompletionDetermination, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Frontier, FrontierAction, FrontierClaim, FrontierData, FrontierId, FrontierObligation,
    PrincipalId, RunOutcome, Truth, Unit, commission_state, frontier_state,
};
use b10x_loom_commission::outcome::{Derived, derive};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::model::run::{CatalogueEntry, SelectionStrategy};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{
    ActionSelector, EmptyObjectArguments, FirstAdmissibleSelector, Loom, SelectorError,
};

const MERGE: &str = "repository.merge";

/// A selector that always names one action, as a model may.
///
/// `story:action-selector`: a selector is handed the catalogue projected from the frontier, so an
/// action Commission refuses is not among its candidates, and Loom refuses it as absent from the
/// catalogue before Commission's admission is asked. What these cases assert of `Loom::run` is
/// unchanged.
struct Pick(&'static str);

impl ActionSelector for Pick {
    fn select(
        &self,
        _context: &SelectionContext,
        _candidates: &[CatalogueEntry],
    ) -> Result<Choice, SelectorError> {
        Ok(Choice {
            action: self.0.to_owned(),
            confidence: None,
        })
    }

    fn strategy(&self) -> SelectionStrategy {
        SelectionStrategy::ReasoningModel
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: CaseId("CHG-1842".to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(
    actions: Vec<FrontierAction>,
    obligations: Vec<FrontierObligation>,
) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-000000000003".to_owned(),
        )),
        case_id: CaseId("CHG-1842".to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations,
        actions,
    })
}

fn entry(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: vec![format!("{status:?}")],
    }
}

fn run(selector: Pick, frontier: &Frontier<frontier_state::Issued>) -> ExecutorOutcome {
    Loom::new(selector, EmptyObjectArguments, "land the change").run(&commission(), frontier)
}

fn proposes(outcome: &ExecutorOutcome, action: &str) -> bool {
    matches!(
        outcome,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction { action: proposed, .. })
            if proposed == action
    )
}
/// A selected action the frontier lists only as `Blocked` is outside the admissible set, and
/// Commission refuses it. Loom must not propose it.
#[test]
fn blocked_selection_is_not_proposed() {
    let frontier = frontier(
        vec![
            entry("repository.inspect", ActionStatus::Admissible, None),
            entry(MERGE, ActionStatus::Blocked, None),
        ],
        Vec::new(),
    );
    assert!(
        matches!(admit(&frontier, MERGE), Admission::Refused(_)),
        "precondition: Commission refuses a Blocked merge"
    );

    let outcome = run(Pick(MERGE), &frontier);
    assert!(
        !proposes(&outcome, MERGE),
        "Loom proposed {MERGE}, which the frontier lists as Blocked: {outcome:?}"
    );
}

/// Acceptance 2 with a selector that does not filter `Blocked` out itself: a model asked to land
/// a change names `repository.merge` whenever the frontier lists it. The acceptance test's
/// `MergeSeeking` skips merge while it is `Blocked`, so it never exercises Loom's own guard there.
#[test]
fn merge_seeking_model_never_gets_merge_proposed() {
    let case = CaseId("CHG-1842".to_owned());
    let governor = FakeGovernor::new();
    let listed = |status: ActionStatus, capability: Option<&str>| {
        vec![
            entry("repository.inspect", ActionStatus::Admissible, None),
            entry("repository.edit", ActionStatus::Admissible, None),
            entry("tests.run", ActionStatus::Admissible, None),
            entry(MERGE, status, capability),
        ]
    };
    governor.script(
        case.clone(),
        [
            Answer::at(1).with_items(
                vec![FrontierClaim {
                    claim: "tests.pass".to_owned(),
                    value: Truth::Unknown,
                }],
                Vec::new(),
                listed(ActionStatus::Blocked, None),
            ),
            Answer::at(2).with_items(
                vec![FrontierClaim {
                    claim: "tests.pass".to_owned(),
                    value: Truth::True,
                }],
                Vec::new(),
                listed(ActionStatus::ApprovalRequired, Some("repository.write")),
            ),
        ],
    );
    let commission = commission();
    let loom = Loom::new(Pick(MERGE), EmptyObjectArguments, "land the change");

    let mut outcomes = Vec::new();
    for _ in 0..4 {
        let frontier = governor
            .frontier(&case)
            .unwrap_or_else(|error| panic!("frontier failed: {error:?}"));
        let outcome = loom.run(&commission, &frontier);
        let stop = matches!(outcome, ExecutorOutcome::Suspended(_));
        outcomes.push((admit(&frontier, MERGE), outcome));
        if stop {
            break;
        }
    }
    // Changed for Atlas ADR 0082 (coordinator decision F6, wave 2026-10-04-w8): the executor
    // proposes and Commission asks for authority, so a merge proposal on the frontier where merge
    // is ApprovalRequired is correct. What stays wrong is a merge proposal Commission refuses,
    // here on the frontier where merge is Blocked. Was: no ProposedAction for merge at all.
    let merges: Vec<&(Admission, ExecutorOutcome)> = outcomes
        .iter()
        .filter(|(admission, outcome)| {
            proposes(outcome, MERGE) && matches!(admission, Admission::Refused(_))
        })
        .collect();
    assert!(
        merges.is_empty(),
        "an invocation returned a ProposedAction for {MERGE} that Commission refuses: {merges:?} \
         (all: {outcomes:?})"
    );
}

/// The outcome `admit` implies for Loom selecting `action` on `frontier` (Atlas ADR 0082): a
/// proposal of `action` when Commission admits it or admits it once authorized, and
/// `NoUsefulAction` when Commission refuses it.
fn implied_by_admission(frontier: &Frontier<frontier_state::Issued>, action: &str) -> Implied {
    match admit(frontier, action) {
        Admission::Admissible(_) | Admission::NeedsAuthority(_) => Implied::Proposed,
        Admission::Refused(_) => Implied::NoUsefulAction,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Implied {
    Proposed,
    NoUsefulAction,
}

/// What Loom's outcome is, in the terms of [`Implied`]; `None` for anything else.
fn implied_by_outcome(outcome: &ExecutorOutcome, action: &str) -> Option<Implied> {
    if proposes(outcome, action) {
        Some(Implied::Proposed)
    } else if *outcome == ExecutorOutcome::NoUsefulAction(Unit(true)) {
        Some(Implied::NoUsefulAction)
    } else {
        None
    }
}

/// `entries`, with an admissible sibling after them, so the frontier admits something and the
/// selector runs: Loom's early return for a frontier that admits nothing does not answer for the
/// selection guard.
fn with_sibling(mut entries: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    entries.push(entry("repository.inspect", ActionStatus::Admissible, None));
    frontier(entries, Vec::new())
}

/// One action listed twice, `ApprovalRequired` and `Blocked`: Commission refuses it (the `Blocked`
/// entry decides), so Loom answers `NoUsefulAction`.
///
/// Rewritten for coordinator decision 3 on adversary pass 2 (wave 2026-10-04-w8): under Atlas ADR
/// 0082 Loom never constructs `Suspended(Authority)`, so the pass-1 assertion that it does not ask
/// for authority could no longer fail. The frontier now carries an admissible sibling, and the case
/// asserts that Loom's outcome is what `admit` implies.
#[test]
fn blocked_entry_beside_approval_entry_does_not_ask_for_authority() {
    let frontier = with_sibling(vec![
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
        entry(MERGE, ActionStatus::Blocked, None),
    ]);
    assert_eq!(
        implied_by_admission(&frontier, MERGE),
        Implied::NoUsefulAction,
        "precondition: Commission refuses it"
    );

    let outcome = run(Pick(MERGE), &frontier);
    assert_eq!(
        implied_by_outcome(&outcome, MERGE),
        Some(Implied::NoUsefulAction),
        "Loom's outcome is not what Commission's refusal implies: {outcome:?}"
    );
}

/// Two `ApprovalRequired` entries naming different capabilities: Commission refuses the conflict,
/// whatever the order, so Loom answers `NoUsefulAction` in both orders.
///
/// Rewritten for coordinator decision 3 on adversary pass 2 (wave 2026-10-04-w8): the pass-1
/// assertion compared the capability of a `Suspended(Authority)` Loom no longer constructs (Atlas
/// ADR 0082), so it held whatever Loom did. Each order now carries an admissible sibling and is
/// held to what `admit` implies.
#[test]
fn conflicting_capabilities_do_not_depend_on_order() {
    let a_then_b = with_sibling(vec![
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("release.publish"),
        ),
    ]);
    let b_then_a = with_sibling(vec![
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("release.publish"),
        ),
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]);
    assert_eq!(admit(&a_then_b, MERGE), admit(&b_then_a, MERGE));

    for (order, frontier) in [("a then b", &a_then_b), ("b then a", &b_then_a)] {
        let outcome = run(Pick(MERGE), frontier);
        assert_eq!(
            implied_by_outcome(&outcome, MERGE),
            Some(implied_by_admission(frontier, MERGE)),
            "{order}: Loom's outcome is not what Commission's admission implies: {outcome:?}"
        );
    }
}

/// `ApprovalRequired` with no capability, or a blank one, names nothing to ask for; Commission
/// refuses it, so Loom answers `NoUsefulAction`.
///
/// Rewritten for coordinator decision 3 on adversary pass 2 (wave 2026-10-04-w8): the pass-1
/// assertion that Loom does not suspend for capability `null` or `"  "` could no longer fail under
/// Atlas ADR 0082. Each frontier now carries an admissible sibling and is held to what `admit`
/// implies.
#[test]
fn approval_without_capability_does_not_ask_for_nothing() {
    for capability in [None, Some("  ")] {
        let frontier = with_sibling(vec![entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            capability,
        )]);
        assert_eq!(
            implied_by_admission(&frontier, MERGE),
            Implied::NoUsefulAction,
            "precondition: Commission refuses capability {capability:?}"
        );
        let outcome = run(Pick(MERGE), &frontier);
        assert_eq!(
            implied_by_outcome(&outcome, MERGE),
            Some(Implied::NoUsefulAction),
            "capability {capability:?}: Loom's outcome is not what Commission's refusal implies: \
             {outcome:?}"
        );
    }
}

/// Every frontier of one or two entries for the selected action, over the statuses and
/// capabilities Commission distinguishes, each beside an admissible sibling: Loom's outcome is what
/// `admit` implies. `Admissible` and `NeedsAuthority` are proposed (Atlas ADR 0082); `Refused` is
/// `NoUsefulAction`.
///
/// Rewritten for coordinator decision 3 on adversary pass 2 (wave 2026-10-04-w8): without the
/// sibling every refused frontier was answered by Loom's early return for a frontier that admits
/// nothing, so the selection guard was never exercised, and the `Refused` arm accepted any outcome
/// that was neither a proposal nor an authority suspension.
#[test]
fn loom_agrees_with_commission_admission() {
    let kinds: [(ActionStatus, Option<&str>); 6] = [
        (ActionStatus::Admissible, None),
        (ActionStatus::Blocked, None),
        (ActionStatus::ApprovalRequired, Some("a")),
        (ActionStatus::ApprovalRequired, Some("b")),
        (ActionStatus::ApprovalRequired, None),
        (ActionStatus::ApprovalRequired, Some(" ")),
    ];
    let mut cases: Vec<Vec<FrontierAction>> = kinds
        .iter()
        .map(|(status, cap)| vec![entry(MERGE, *status, *cap)])
        .collect();
    for (s1, c1) in kinds {
        for (s2, c2) in kinds {
            cases.push(vec![entry(MERGE, s1, c1), entry(MERGE, s2, c2)]);
        }
    }

    let mut disagreements = Vec::new();
    for actions in cases {
        let shape: Vec<String> = actions
            .iter()
            .map(|a| format!("{:?}/{:?}", a.status, a.capability))
            .collect();
        let frontier = with_sibling(actions);
        let outcome = run(Pick(MERGE), &frontier);
        let implied = implied_by_admission(&frontier, MERGE);
        if implied_by_outcome(&outcome, MERGE).as_ref() != Some(&implied) {
            disagreements.push(format!(
                "{shape:?}: commission {:?}, implied {implied:?}, loom {outcome:?}",
                admit(&frontier, MERGE)
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} frontier(s) where Loom disagrees with Commission admission:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}

/// A frontier that admits nothing, with an open obligation: Commission derives
/// `NeedsExternalEvidence` naming that obligation when the executor has no useful action. Loom's
/// bootstrap selector instead makes it a `Suspended(ExternalAvailability)`, and `derive` rule 2
/// then ends the run as an outage, hiding the obligation.
#[test]
fn nothing_admissible_is_not_an_external_outage() {
    let frontier = frontier(
        vec![entry(MERGE, ActionStatus::Blocked, None)],
        vec![FrontierObligation {
            obligation: "verify.tests".to_owned(),
            open: true,
        }],
    );
    let expected = derive(
        &CompletionDetermination::Open(Unit(true)),
        &frontier,
        &ExecutorOutcome::NoUsefulAction(Unit(true)),
        None,
    );
    assert!(
        matches!(
            &expected,
            Derived::Ended(RunOutcome::NeedsExternalEvidence(_))
        ),
        "precondition: {expected:?}"
    );

    let outcome = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "land the change",
    )
    .run(&commission(), &frontier);
    let derived = derive(
        &CompletionDetermination::Open(Unit(true)),
        &frontier,
        &outcome,
        None,
    );
    assert_eq!(
        derived, expected,
        "Loom's outcome {outcome:?} ends the run differently from no useful action"
    );
}
