//! Acceptance for `story:protocol-argument-schema`: generated arguments reach revalidation and a
//! proposal only when they match the argument schema the host's loaded protocol declares for the
//! action (`Loom::with_argument_schemas`); an action whose protocol declares none takes only `{}`;
//! a Loom given no schemas checks nothing. Every check is recorded on its argument request
//! (`loom.run.CheckArguments`).
//!
//! Every run is handed one frontier listing the actions below, all admissible; the selector picks
//! the action a case names and the generator writes the arguments it names. The fake governor
//! answers the frontier at revalidation, so a run that reaches revalidation records it, and one
//! that does not leaves the governor uncalled.

use b10x_loom_commission::model::json::{Value, parse};
use b10x_loom_commission::model::primitives::Uuid as CommissionUuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, PrincipalId,
    Unit, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_executor::arguments::{ArgumentContext, ArgumentGenerator, ArgumentSchemas};
use b10x_loom_executor::model::run::{
    ArgumentRequestState, CatalogueEntry, CheckArgumentsOutcome, SelectionStrategy,
};
use b10x_loom_executor::selection::{Choice, SelectionContext};
use b10x_loom_executor::{ActionSelector, Loom, SelectorError};
use serde_json::json;

const CASE: &str = "CASE-1";
const REVISION: i64 = 4;
/// Declares `{service, environment, limit}`, all required, `limit` an integer.
const INSPECT: &str = "service.inspect";
/// Declares no argument schema: takes only `{}`.
const PING: &str = "service.ping";
/// Declares the planner's schema (`fixtures/argument-schema/planner.yaml`).
const OPEN: &str = "stories.open";
/// Listed by no frontier here.
const OUTSIDE: &str = "service.restart";

/// The planner fixture's protocol, as Canon reads it.
const PLANNER: &str = include_str!("fixtures/argument-schema/planner.yaml");

/// Picks `action` whatever the candidates; an action outside the catalogue is Loom's to refuse.
struct Picks(&'static str);

impl ActionSelector for Picks {
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
        SelectionStrategy::Rule
    }
}

/// Writes the arguments its JSON text spells, whatever the entry.
struct Writes(&'static str);

impl ArgumentGenerator for Writes {
    fn generate(
        &self,
        _context: &ArgumentContext,
        _entry: &CatalogueEntry,
    ) -> Result<Value, String> {
        Ok(parse(self.0).expect("the scripted arguments are JSON"))
    }
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: case(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn listed() -> Vec<FrontierAction> {
    [INSPECT, PING, OPEN]
        .into_iter()
        .map(|action| FrontierAction {
            action: action.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        })
        .collect()
}

fn handed() -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(CommissionUuid(
            "00000000-0000-4000-8000-0000000000f0".to_owned(),
        )),
        case_id: case(),
        case_revision: REVISION,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: listed(),
    })
}

/// The schemas the host hands: `service.inspect`'s and the planner's declared, `service.ping`
/// with none. The planner's is read from its protocol, as a host reads it from the entry it runs.
fn schemas() -> ArgumentSchemas {
    let planner = b10x_canon::model::parse(PLANNER).expect("the planner fixture parses");
    b10x_canon::validate::validate(&planner).expect("the planner fixture validates");
    let open = planner
        .actions
        .iter()
        .find(|(id, _)| id.as_str() == OPEN)
        .and_then(|(_, action)| action.arguments.clone())
        .expect("stories.open declares its arguments");
    ArgumentSchemas::from_iter([
        (
            INSPECT.to_owned(),
            Some(json!({
                "type": "object",
                "properties": {
                    "service": {"type": "string"},
                    "environment": {"type": "string", "enum": ["staging", "prod"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                },
                "required": ["service", "environment", "limit"],
                "additionalProperties": false
            })),
        ),
        (PING.to_owned(), None),
        (OPEN.to_owned(), Some(open)),
    ])
}

/// What one run produced.
struct Ran {
    outcome: ExecutorOutcome,
    requests: Vec<ArgumentRequestState>,
    checks: Vec<CheckArgumentsOutcome>,
    revalidations: usize,
    governor: FakeGovernor,
}

/// One run picking `action` with the arguments `arguments` spells, checked against
/// [`schemas`] when `checked`.
fn run(action: &'static str, arguments: &'static str, checked: bool) -> Ran {
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [Answer::at(REVISION).with_items(Vec::new(), Vec::new(), listed())],
    );
    let (outcome, requests, checks, revalidations) = {
        let loom = Loom::new(Picks(action), Writes(arguments), "inspect the service")
            .with_governor(&governor);
        let loom = if checked {
            loom.with_argument_schemas(schemas())
        } else {
            loom
        };
        let outcome = loom.run(&commission(), &handed());
        (
            outcome,
            loom.argument_requests()
                .iter()
                .map(|request| request.state)
                .collect(),
            loom.argument_checks(),
            loom.revalidations().len(),
        )
    };
    Ran {
        outcome,
        requests,
        checks,
        revalidations,
        governor,
    }
}

/// The proposed arguments, as JSON text.
fn proposed(outcome: &ExecutorOutcome) -> Option<String> {
    match outcome {
        ExecutorOutcome::ProposedAction(proposal) => {
            let mut text = String::new();
            b10x_loom_commission::model::json::push_value(&mut text, &proposal.arguments.0);
            Some(text)
        }
        _ => None,
    }
}

/// The one mismatch a run recorded: whether a schema was declared, the action and the reason.
fn mismatch(ran: &Ran) -> (bool, String, String) {
    match ran.checks.as_slice() {
        [
            CheckArgumentsOutcome::Mismatched {
                arguments_mismatched,
            },
        ]
        | [
            CheckArgumentsOutcome::UndeclaredNotEmpty {
                arguments_mismatched,
            },
        ] => (
            arguments_mismatched.declared,
            arguments_mismatched.action.clone(),
            arguments_mismatched.reason.clone(),
        ),
        other => panic!("not one mismatch: {other:?}"),
    }
}

/// A refused run: nothing proposed, nothing revalidated, the governor never asked, nothing
/// observed or submitted, and the request `Mismatched`.
fn assert_refused(ran: &Ran, label: &str) {
    assert_eq!(
        ran.outcome,
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        "{label}"
    );
    assert_eq!(ran.revalidations, 0, "{label}: revalidated");
    assert!(ran.governor.calls().is_empty(), "{label}: governor asked");
    assert!(ran.governor.observations().is_empty(), "{label}: observed");
    assert!(ran.governor.evidence().is_empty(), "{label}: evidence");
    assert_eq!(
        ran.requests,
        [ArgumentRequestState::Mismatched],
        "{label}: request"
    );
}

#[test]
fn arguments_matching_the_declared_schema_are_revalidated_and_proposed() {
    let ran = run(
        INSPECT,
        r#"{"service": "checkout-api", "environment": "prod", "limit": 20}"#,
        true,
    );

    assert_eq!(
        proposed(&ran.outcome).as_deref(),
        Some(r#"{"service":"checkout-api","environment":"prod","limit":20}"#),
        "{:?}",
        ran.outcome
    );
    assert_eq!(ran.revalidations, 1);
    assert_eq!(ran.requests, [ArgumentRequestState::Matched]);
    assert!(
        matches!(
            ran.checks.as_slice(),
            [CheckArgumentsOutcome::Matched { .. }]
        ),
        "{:?}",
        ran.checks
    );
}

#[test]
fn arguments_not_matching_the_declared_schema_are_never_revalidated_or_proposed() {
    let ran = run(
        INSPECT,
        r#"{"service": "checkout-api", "environment": "prod", "limit": "20"}"#,
        true,
    );

    assert_refused(&ran, "limit as a string");
    let (declared, action, reason) = mismatch(&ran);
    assert!(declared);
    assert_eq!(action, INSPECT);
    assert!(reason.contains(INSPECT), "{reason}");
    assert!(
        reason.contains("/limit") && reason.contains("type"),
        "{reason}"
    );
}

#[test]
fn an_action_with_no_declared_schema_takes_only_the_empty_object() {
    let empty = run(PING, "{}", true);
    assert_eq!(proposed(&empty.outcome).as_deref(), Some("{}"));
    assert_eq!(empty.requests, [ArgumentRequestState::Matched]);

    let ran = run(PING, r#"{"x": 1}"#, true);
    assert_refused(&ran, "{\"x\": 1} for an action with no schema");
    let (declared, action, reason) = mismatch(&ran);
    assert!(!declared);
    assert_eq!(action, PING);
    assert!(reason.contains(PING), "{reason}");
    assert!(reason.contains("declares no argument schema"), "{reason}");
}

#[test]
fn a_loom_without_schemas_checks_no_arguments() {
    let ran = run(PING, r#"{"x": 1}"#, false);

    assert_eq!(proposed(&ran.outcome).as_deref(), Some(r#"{"x":1}"#));
    assert_eq!(ran.requests, [ArgumentRequestState::Requested]);
    assert!(ran.checks.is_empty(), "{:?}", ran.checks);
}

#[test]
fn an_action_outside_the_catalogue_never_reaches_the_check() {
    let ran = run(OUTSIDE, r#"{"x": 1}"#, true);

    assert_eq!(ran.outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    assert!(ran.checks.is_empty(), "{:?}", ran.checks);
    assert!(ran.requests.is_empty(), "{:?}", ran.requests);
}

/// The planner's `case_open` action (`stories`, an array of at least one string): one name
/// passes and is proposed; an empty list, a non-string item and a missing member are each
/// refused before revalidation and before any proposal, so no child is opened and nothing is
/// written.
#[test]
fn a_case_open_action_takes_only_a_non_empty_list_of_story_names() {
    let ran = run(OPEN, r#"{"stories": ["story:one"]}"#, true);
    assert_eq!(
        proposed(&ran.outcome).as_deref(),
        Some(r#"{"stories":["story:one"]}"#)
    );
    assert_eq!(ran.requests, [ArgumentRequestState::Matched]);

    for (label, arguments, pointer, keyword) in [
        (
            "an empty list",
            r#"{"stories": []}"#,
            "/stories",
            "minItems",
        ),
        (
            "a non-string item",
            r#"{"stories": ["story:one", 2]}"#,
            "/stories/1",
            "type",
        ),
        ("a missing member", "{}", "``", "required"),
    ] {
        let ran = run(OPEN, arguments, true);
        assert_refused(&ran, label);
        let (declared, action, reason) = mismatch(&ran);
        assert!(declared, "{label}");
        assert_eq!(action, OPEN, "{label}");
        assert!(
            reason.contains(pointer) && reason.contains(keyword),
            "{label}: {reason}"
        );
    }
}

/// A member named twice is refused before Canon reads the value: the reader Loom checks with keeps
/// one of the two and an effect port may read the other.
#[test]
fn arguments_naming_a_member_twice_are_refused() {
    let ran = run(
        INSPECT,
        r#"{"service": "checkout-api", "environment": "prod", "limit": 20, "limit": "all"}"#,
        true,
    );

    assert_refused(&ran, "limit twice");
    let (_, _, reason) = mismatch(&ran);
    assert!(reason.contains("/limit"), "{reason}");
}
