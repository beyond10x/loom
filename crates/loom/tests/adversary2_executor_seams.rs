//! Adversary pass 2 on `story:agent-executor`: the paths of `Loom::run` no test observes.
//!
//! Each case asserts a literal outcome, never one built by the code under test. The pass-1 cases
//! `blocked_entry_beside_approval_entry_does_not_ask_for_authority`,
//! `conflicting_capabilities_do_not_depend_on_order` and
//! `approval_without_capability_does_not_ask_for_nothing` assert only that Loom does not return
//! `Suspended(Authority)`, which Loom no longer constructs (Atlas ADR 0082), so they pass whatever
//! Loom proposes. `refused_selections_are_no_useful_action` restates them on the outcome itself.

use b10x_commission::admission::admit;
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeSuspended, Frontier, FrontierAction,
    FrontierData, FrontierId, PrincipalId, ProposedActionArguments, SuspensionReason, Unit,
    commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_loom::{ActionSelector, ArgumentGenerator, EmptyObjectArguments, Loom, SelectorError};

const MERGE: &str = "repository.merge";

/// A selector that names one action, or fails with one message.
struct Scripted(Result<&'static str, &'static str>);

impl ActionSelector for Scripted {
    fn select(
        &self,
        _frontier: &Frontier<frontier_state::Issued>,
        _prompt: &str,
    ) -> Result<String, SelectorError> {
        self.0
            .map(str::to_owned)
            .map_err(|error| SelectorError::Unavailable(error.to_owned()))
    }
}

/// A generator whose arguments say which frontier entry it was given, as one that reads the
/// entry's status or capability to shape its arguments would.
struct EchoEntry;

impl ArgumentGenerator for EchoEntry {
    fn generate(
        &self,
        action: &FrontierAction,
        _prompt: &str,
    ) -> Result<ProposedActionArguments, String> {
        Ok(ProposedActionArguments(Value::Object(vec![
            (
                "status".to_owned(),
                Value::Text(format!("{:?}", action.status)),
            ),
            (
                "capability".to_owned(),
                action
                    .capability
                    .as_ref()
                    .map_or(Value::Null, |c| Value::Text(c.clone())),
            ),
        ])))
    }
}

/// A generator that fails.
struct Failing(&'static str);

impl ArgumentGenerator for Failing {
    fn generate(
        &self,
        _action: &FrontierAction,
        _prompt: &str,
    ) -> Result<ProposedActionArguments, String> {
        Err(self.0.to_owned())
    }
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        case_id: CaseId("CHG-1842".to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn frontier(actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-000000000003".to_owned())),
        case_id: CaseId("CHG-1842".to_owned()),
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions,
    })
}

fn entry(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

/// Whether some text anywhere in `value` contains `needle`.
fn carries_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::Text(text) => text.contains(needle),
        Value::Array(items) => items.iter().any(|item| carries_text(item, needle)),
        Value::Object(members) => members.iter().any(|(_, item)| carries_text(item, needle)),
        _ => false,
    }
}

fn external_availability(outcome: &ExecutorOutcome) -> Option<&Value> {
    match outcome {
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::ExternalAvailability(detail),
        }) => Some(detail),
        _ => None,
    }
}

/// The three pass-1 frontiers Commission refuses, asserted on the outcome itself: Loom answers each
/// with `NoUsefulAction`, the literal value, and never with a proposal.
#[test]
fn refused_selections_are_no_useful_action() {
    let refused: Vec<(&str, Vec<FrontierAction>)> = vec![
        (
            "Blocked beside ApprovalRequired",
            vec![
                entry(
                    MERGE,
                    ActionStatus::ApprovalRequired,
                    Some("repository.write"),
                ),
                entry(MERGE, ActionStatus::Blocked, None),
            ],
        ),
        (
            "conflicting capabilities, a then b",
            vec![
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
            ],
        ),
        (
            "conflicting capabilities, b then a",
            vec![
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
            ],
        ),
        (
            "ApprovalRequired with no capability",
            vec![entry(MERGE, ActionStatus::ApprovalRequired, None)],
        ),
        (
            "ApprovalRequired with a blank capability",
            vec![entry(MERGE, ActionStatus::ApprovalRequired, Some("  "))],
        ),
    ];
    let mut wrong = Vec::new();
    for (shape, mut actions) in refused {
        // An admissible sibling, so the frontier admits something and Loom's early return for a
        // frontier that admits nothing does not answer for the selection guard. Without it every
        // pass-1 frontier above is decided before the selector is even called.
        actions.push(entry("repository.inspect", ActionStatus::Admissible, None));
        let frontier = frontier(actions);
        assert!(
            matches!(admit(&frontier, MERGE), Admission::Refused(_)),
            "precondition: Commission refuses {shape}"
        );
        let outcome = Loom::new(Scripted(Ok(MERGE)), EmptyObjectArguments, "land the change")
            .run(&commission(), &frontier);
        if outcome != ExecutorOutcome::NoUsefulAction(Unit(true)) {
            wrong.push(format!("{shape}: {outcome:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "Loom did not answer a refused selection with NoUsefulAction:\n{}",
        wrong.join("\n")
    );
}

/// One action listed `Admissible` and `ApprovalRequired`: Commission's answer is `NeedsAuthority`
/// whatever the order (`admission.rs` rule 4). Loom hands its argument generator the first entry
/// listed, so what it proposes depends on the order of the frontier's entries.
#[test]
fn generator_is_given_the_same_entry_whatever_the_order() {
    let admissible_first = frontier(vec![
        entry(MERGE, ActionStatus::Admissible, None),
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
    ]);
    let approval_first = frontier(vec![
        entry(
            MERGE,
            ActionStatus::ApprovalRequired,
            Some("repository.write"),
        ),
        entry(MERGE, ActionStatus::Admissible, None),
    ]);
    assert_eq!(
        admit(&admissible_first, MERGE),
        admit(&approval_first, MERGE),
        "precondition: Commission's admission does not depend on the order"
    );

    let loom = Loom::new(Scripted(Ok(MERGE)), EchoEntry, "land the change");
    let first = loom.run(&commission(), &admissible_first);
    let second = loom.run(&commission(), &approval_first);
    assert_eq!(
        first, second,
        "the proposal for {MERGE} depends on the order of its frontier entries"
    );
}

/// The run's doc: a selector failure is `Suspended(ExternalAvailability)` carrying its message.
/// The unit test `a_selector_outage_keeps_its_message` compares with `outage(...)`, the function
/// under test, so it passes whatever `outage` keeps.
#[test]
fn a_selector_outage_carries_its_message_text() {
    let outcome = Loom::new(
        Scripted(Err("model endpoint unreachable")),
        EmptyObjectArguments,
        "land the change",
    )
    .run(
        &commission(),
        &frontier(vec![entry(
            "metrics.inspect",
            ActionStatus::Admissible,
            None,
        )]),
    );
    let detail = external_availability(&outcome)
        .unwrap_or_else(|| panic!("not Suspended(ExternalAvailability): {outcome:?}"));
    assert!(
        carries_text(detail, "model endpoint unreachable"),
        "the outage does not carry the selector's message: {outcome:?}"
    );
}

/// The run's doc: a failure of the argument generator is `Suspended(ExternalAvailability)`
/// carrying its message. No other case drives that path.
#[test]
fn an_argument_generator_failure_is_an_outage_with_its_message() {
    let outcome = Loom::new(
        Scripted(Ok("metrics.inspect")),
        Failing("argument schema unavailable"),
        "land the change",
    )
    .run(
        &commission(),
        &frontier(vec![entry(
            "metrics.inspect",
            ActionStatus::Admissible,
            None,
        )]),
    );
    let detail = external_availability(&outcome)
        .unwrap_or_else(|| panic!("not Suspended(ExternalAvailability): {outcome:?}"));
    assert!(
        carries_text(detail, "argument schema unavailable"),
        "the outage does not carry the generator's message: {outcome:?}"
    );
}

/// The run's doc lists "a frontier that admits nothing" first among the `NoUsefulAction` cases,
/// before any selector failure. With a selector that is down, that frontier is still
/// `NoUsefulAction`: there is nothing to select. Every other case of a frontier that admits
/// nothing uses a selector that answers `NOTHING_ADMISSIBLE` itself, so they pass without the
/// check.
#[test]
fn a_frontier_that_admits_nothing_is_no_useful_action_even_when_the_selector_is_down() {
    let outcome = Loom::new(
        Scripted(Err("model endpoint unreachable")),
        EmptyObjectArguments,
        "land the change",
    )
    .run(
        &commission(),
        &frontier(vec![entry(MERGE, ActionStatus::Blocked, None)]),
    );
    assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
}
