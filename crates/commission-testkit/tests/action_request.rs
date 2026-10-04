//! Acceptance for `story:stale-revision-action-request`: a proposed action becomes an action
//! request bound to the case revision of the frontier it was chosen from, and it is revalidated
//! against the case's current revision and frontier immediately before use.
//!
//! The case moves through the scripted fake governor ([`FakeGovernor`]). Revalidation answers with
//! the outcome of the specification's `RevalidateActionRequest` command, in its declared order:
//! stale, not admitted, needs authority, admitted. A stale request is refused even when its action
//! is admissible at the current revision.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time:
//! a build directory shared between worktrees reuses binaries across them.

use b10x_commission::action_request::{command_input, request, revalidate};
use b10x_commission::admission::admit;
use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionNeedsAuthority, ActionNotAdmitted, ActionRequest, ActionRequestId, ActionRequestStale,
    ActionRequestState, ActionStatus, Admission, CaseId, CompletionDetermination,
    ExecutorOutcomeProposedAction, Frontier, FrontierAction, GovernorError,
    ProposedActionArguments, RevalidateActionRequest, RevalidateActionRequestOutcome, RunId, Unit,
    action_request_state, frontier_state,
};
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor, GovernorCall};
use std::path::PathBuf;
use std::process::Command;

const NS: &str = "commission.responsibility.";

const CASE: &str = "case-action-request";

/// The revision the case is at when the run chose its action.
const N: i64 = 7;

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case() -> CaseId {
    CaseId(CASE.to_owned())
}

fn listed(name: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: Vec::new(),
    }
}

fn blocked(name: &str, reasons: &[&str]) -> FrontierAction {
    FrontierAction {
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
        ..listed(name, ActionStatus::Blocked, None)
    }
}

/// The case at `revision`, with a frontier listing exactly `actions`.
fn at(revision: i64, actions: Vec<FrontierAction>) -> Answer {
    Answer::at(revision).with_items(Vec::new(), Vec::new(), actions)
}

/// The arguments the executor wrote: not null, so the test sees them carried, not defaulted.
fn arguments() -> ProposedActionArguments {
    ProposedActionArguments(
        json::parse(r#"{"branch":"fix-1","squash":true}"#)
            .unwrap_or_else(|error| panic!("arguments fixture is not JSON: {error:?}")),
    )
}

fn proposal(action: &str) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: arguments(),
    }
}

/// A request for `action`, built from the frontier `governor` issues now, as a run builds it from
/// the frontier it was given.
fn request_from(
    governor: &FakeGovernor,
    id: u64,
    action: &str,
) -> ActionRequest<action_request_state::Requested> {
    let frontier = governor
        .frontier(&case())
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    request(
        ActionRequestId(uuid(id)),
        RunId(uuid(0x100)),
        &frontier,
        proposal(action),
    )
}

/// Revalidates `request` against `governor`, which must answer.
fn revalidated<G: Governor>(
    governor: &G,
    request: &ActionRequest<action_request_state::Requested>,
) -> RevalidateActionRequestOutcome {
    revalidate(governor, request)
        .unwrap_or_else(|error| panic!("revalidation failed at the governor: {error:?}"))
}

/// Expectations 1 and 4: a request built at N, revalidated after the case moved to N+1, is refused
/// as stale naming N and N+1, although its action is admissible at N+1. Its arguments are the
/// generated `ProposedActionArguments` the executor wrote.
fn stale_request_is_refused_although_admissible_now() {
    let governor = FakeGovernor::new();
    let merge = || vec![listed("merge", ActionStatus::Admissible, None)];
    governor.script(case(), [at(N, merge()), at(N + 1, merge())]);

    let at_n = request_from(&governor, 1, "merge");
    let data = at_n.data();
    assert_eq!(at_n.state(), ActionRequestState::Requested, "request state");
    assert_eq!(
        data.action_request_id,
        ActionRequestId(uuid(1)),
        "request id"
    );
    assert_eq!(data.run_id, RunId(uuid(0x100)), "run id");
    assert_eq!(data.case_id, case(), "case id");
    assert_eq!(data.expected_case_revision, N, "expected revision");
    assert_eq!(data.action, "merge", "action");
    assert_eq!(data.arguments, arguments(), "arguments");

    // The command input is the whole request, its identity included.
    assert_eq!(
        command_input(&at_n),
        RevalidateActionRequest {
            action_request_id: ActionRequestId(uuid(1)),
            run_id: RunId(uuid(0x100)),
            case_id: case(),
            expected_case_revision: N,
            action: "merge".to_owned(),
            arguments: arguments(),
        },
        "the RevalidateActionRequest input for the request"
    );

    // Expectation 4: the arguments are the generated type, reached through the re-export.
    let name = std::any::type_name_of_val(&data.arguments);
    assert_eq!(
        name, "commission::responsibility::ProposedActionArguments",
        "the request's arguments are not the generated ProposedActionArguments"
    );

    // The case has moved; its frontier at N+1 still admits the action.
    let current = governor
        .frontier(&case())
        .unwrap_or_else(|error| panic!("frontier call failed: {error:?}"));
    assert_eq!(current.data().case_revision, N + 1, "the case did not move");
    assert_eq!(
        admit(&current, "merge"),
        Admission::Admissible(Unit(true)),
        "precondition: the action is admissible at N+1"
    );

    let calls_before = governor.calls().len();
    assert_eq!(
        revalidated(&governor, &at_n),
        RevalidateActionRequestOutcome::Stale {
            error: ActionRequestStale {
                expected_case_revision: N,
                current_case_revision: N + 1,
            },
        },
        "a request made at N, revalidated at N+1"
    );
    assert_eq!(
        governor.calls()[calls_before..],
        [GovernorCall::CurrentRevision(case())],
        "a stale revision is decided from the current revision the governor reports, before any \
         frontier is read"
    );

    // Expectation 2: the same request built at N+1 passes revalidation.
    let at_n1 = request_from(&governor, 2, "merge");
    assert_eq!(
        at_n1.data().expected_case_revision,
        N + 1,
        "expected revision"
    );
    let calls_before = governor.calls().len();
    assert_eq!(
        revalidated(&governor, &at_n1),
        RevalidateActionRequestOutcome::Admitted,
        "the same request built at N+1"
    );
    assert_eq!(
        governor.calls()[calls_before..],
        [
            GovernorCall::CurrentRevision(case()),
            GovernorCall::Frontier(case())
        ],
        "an admitted request is checked against the current revision and the current frontier"
    );
}

/// Expectation 3: a request at the current revision whose action the current frontier does not
/// list is refused as not admitted, naming the action. A blocked action is refused with the
/// frontier's reasons.
fn unlisted_or_blocked_action_is_not_admitted() {
    let governor = FakeGovernor::new();
    governor.script(
        case(),
        [at(
            N,
            vec![
                listed("comment", ActionStatus::Admissible, None),
                blocked("deploy", &["the freeze window is open"]),
            ],
        )],
    );

    let unlisted = request_from(&governor, 3, "merge");
    assert_eq!(
        revalidated(&governor, &unlisted),
        RevalidateActionRequestOutcome::NotAdmitted {
            error: ActionNotAdmitted {
                action: "merge".to_owned(),
                reasons: Vec::new(),
            },
        },
        "an action the current frontier does not list"
    );

    let blocked = request_from(&governor, 4, "deploy");
    assert_eq!(
        revalidated(&governor, &blocked),
        RevalidateActionRequestOutcome::NotAdmitted {
            error: ActionNotAdmitted {
                action: "deploy".to_owned(),
                reasons: vec!["the freeze window is open".to_owned()],
            },
        },
        "an action the current frontier blocks"
    );
}

/// The needs-authority outcome (story § From wave 2026-10-04-w4): an `ApprovalRequired` action is
/// never revalidated as admitted without an authority decision. A stale request is refused as stale
/// before its approval is looked at.
fn approval_required_action_is_not_admitted_without_authority() {
    let governor = FakeGovernor::new();
    let approval = || {
        vec![listed(
            "merge",
            ActionStatus::ApprovalRequired,
            Some("repo.merge"),
        )]
    };
    governor.script(case(), [at(N, approval()), at(N + 1, approval())]);

    let at_n = request_from(&governor, 5, "merge");
    let at_n1 = request_from(&governor, 6, "merge");
    assert_eq!(
        revalidated(&governor, &at_n),
        RevalidateActionRequestOutcome::Stale {
            error: ActionRequestStale {
                expected_case_revision: N,
                current_case_revision: N + 1,
            },
        },
        "a stale request for an ApprovalRequired action"
    );
    assert_eq!(
        revalidated(&governor, &at_n1),
        RevalidateActionRequestOutcome::NeedsAuthority {
            error: ActionNeedsAuthority {
                action: "merge".to_owned(),
                capability: "repo.merge".to_owned(),
            },
        },
        "a current request for an ApprovalRequired action"
    );
}

/// The case moves between the two governor calls of one revalidation: the revision still reads
/// N, but the frontier is issued for N+1. The request is stale against that frontier, although its
/// action is admissible there.
fn frontier_issued_for_a_later_revision_is_stale() {
    let governor = FakeGovernor::new();
    let merge = || vec![listed("merge", ActionStatus::Admissible, None)];
    // Built from the first answer; revalidation reads the second (the revision) and the third
    // (the frontier).
    governor.script(case(), [at(N, merge()), at(N, merge()), at(N + 1, merge())]);

    let at_n = request_from(&governor, 7, "merge");
    assert_eq!(
        revalidated(&governor, &at_n),
        RevalidateActionRequestOutcome::Stale {
            error: ActionRequestStale {
                expected_case_revision: N,
                current_case_revision: N + 1,
            },
        },
        "a frontier issued for N+1 to a request made at N"
    );
}

/// A governor that answers with a frontier for another case, admitting the action there.
struct OtherCaseGovernor(FakeGovernor);

impl Governor for OtherCaseGovernor {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.0.current_revision(case)
    }

    fn frontier(&self, _case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.0.frontier(&CaseId("case-other".to_owned()))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.0.completion(case)
    }
}

/// A frontier issued for another case admits nothing for this one: the request is refused as not
/// admitted, with a reason naming both cases.
fn frontier_for_another_case_is_not_admitted() {
    let fake = FakeGovernor::new();
    let merge = || vec![listed("merge", ActionStatus::Admissible, None)];
    fake.script(case(), [at(N, merge())]);
    fake.script(CaseId("case-other".to_owned()), [at(N, merge())]);

    let at_n = request_from(&fake, 8, "merge");
    match revalidated(&OtherCaseGovernor(fake), &at_n) {
        RevalidateActionRequestOutcome::NotAdmitted { error } => {
            assert_eq!(error.action, "merge", "refused action");
            assert!(
                error.reasons.len() == 1
                    && error.reasons[0].contains("case-other")
                    && error.reasons[0].contains(CASE)
                    && error.reasons[0].contains(&uuid(8).0),
                "the reason does not name both cases: {:?}",
                error.reasons
            );
        }
        other => panic!("a frontier for another case: {other:?}"),
    }
}

/// Expectation 5: the specification declares the action-request command.
fn specification_declares_the_command() {
    let path = root().join("ess/domains/responsibility.yaml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let commands = source
        .split_once("\ncommands:\n")
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("{} declares no commands", path.display()));
    let declared = format!("  - name: {NS}RevalidateActionRequest\n");
    assert!(
        commands.contains(&declared),
        "{} declares no command {NS}RevalidateActionRequest",
        path.display()
    );
}

/// Expectation 6: the conformance suite synthesizes, exit 0, with at least one scenario, among
/// them the revalidation command's.
fn suite_synthesizes() {
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("action_request_revalidation");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .unwrap_or_else(|error| panic!("clear {}: {error}", out_dir.display()));
    }
    std::fs::create_dir_all(&out_dir)
        .unwrap_or_else(|error| panic!("create {}: {error}", out_dir.display()));
    let out = out_dir.join("suite.json");
    let run = Command::new("ess")
        .args(["verify", "conform", "synthesize", "--path"])
        .arg(root().join("ess"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("run `ess verify conform synthesize`: {error}"));
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run.status.success(),
        "`ess verify conform synthesize` failed:\n{report}"
    );

    // The count comes from the summary line, the last one that carries it.
    let summary = report
        .lines()
        .rfind(|line| line.contains(" scenario(s) "))
        .unwrap_or_else(|| panic!("no summary line in:\n{report}"));
    let count: u64 = summary
        .split_whitespace()
        .next()
        .and_then(|first| first.parse().ok())
        .unwrap_or_else(|| panic!("no scenario count in the summary line `{summary}`"));
    assert!(count >= 1, "the suite holds no scenario: `{summary}`");

    let text = std::fs::read_to_string(&out)
        .unwrap_or_else(|error| panic!("read {}: {error}", out.display()));
    let suite = json::parse(&text).unwrap_or_else(|error| panic!("suite is not JSON: {error:?}"));
    let ids: Vec<&str> = match suite.member("scenarios") {
        Some(Value::Object(members)) => members.iter().map(|(id, _)| id.as_str()).collect(),
        other => panic!("suite scenarios: {other:?}"),
    };
    for outcome in ["stale", "not-admitted", "needs-authority", "admitted"] {
        let id = format!("{NS}RevalidateActionRequest/outcome/{outcome}");
        assert!(ids.contains(&id.as_str()), "the suite has no scenario {id}");
    }
}

/// Expectation 7: the specification validates, and in the compiled model `AuthorityDecision`
/// carries `action_request_id` and references exactly one `ActionRequest` through it.
fn authority_decision_references_one_request() {
    let validate = Command::new("ess")
        .args(["specify", "validate", "--path"])
        .arg(root().join("ess"))
        .output()
        .unwrap_or_else(|error| panic!("run `ess specify validate`: {error}"));
    assert!(
        validate.status.success(),
        "`ess specify validate` failed:\n{}{}",
        String::from_utf8_lossy(&validate.stdout),
        String::from_utf8_lossy(&validate.stderr)
    );

    let compile = Command::new("ess")
        .args(["specify", "compile", "--path"])
        .arg(root().join("ess"))
        .args(["--format", "json"])
        .output()
        .unwrap_or_else(|error| panic!("run `ess specify compile`: {error}"));
    assert!(
        compile.status.success(),
        "`ess specify compile` failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let model = json::parse(&String::from_utf8_lossy(&compile.stdout))
        .unwrap_or_else(|error| panic!("compiled model is not JSON: {error:?}"));
    let decision = model
        .member("entities")
        .and_then(|entities| entities.member(&format!("{NS}AuthorityDecision")))
        .unwrap_or_else(|| panic!("the compiled model declares no {NS}AuthorityDecision"));

    let fields: Vec<&str> = match decision.member("fields") {
        Some(Value::Array(fields)) => fields
            .iter()
            .filter_map(|field| match field.member("name") {
                Some(Value::Text(name)) => Some(name.as_str()),
                _ => None,
            })
            .collect(),
        other => panic!("AuthorityDecision fields: {other:?}"),
    };
    assert!(
        fields.contains(&"action_request_id"),
        "AuthorityDecision has no field action_request_id: {fields:?}"
    );

    let text = |value: &str| Some(Value::Text(value.to_owned()));
    let relations = match decision.member("relations") {
        Some(Value::Array(relations)) => relations,
        other => panic!("AuthorityDecision relations: {other:?}"),
    };
    assert!(
        relations.iter().any(|relation| {
            relation.member("kind").cloned() == text("references")
                && relation.member("target").cloned() == text(&format!("{NS}ActionRequest"))
                && relation.member("cardinality").cloned() == text("one")
                && relation.member("via").cloned() == text("action_request_id")
        }),
        "AuthorityDecision does not reference one ActionRequest via action_request_id: \
         {relations:?}"
    );
}

#[test]
fn action_request_revalidation() {
    stale_request_is_refused_although_admissible_now();
    unlisted_or_blocked_action_is_not_admitted();
    approval_required_action_is_not_admitted_without_authority();
    frontier_issued_for_a_later_revision_is_stale();
    frontier_for_another_case_is_not_admitted();
    specification_declares_the_command();
    suite_synthesizes();
    authority_decision_references_one_request();
}
