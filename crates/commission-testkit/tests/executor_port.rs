//! Acceptance for `story:agent-executor-port`: an `AgentExecutor` returns one generated
//! `ExecutorOutcome` for a commission and its current frontier, and `b10x-commission` depends on
//! no model-provider crate and not on Loom or Canon.
//!
//! `task deps-guard` runs this file. Source paths are read when the test runs
//! (`CARGO_MANIFEST_DIR`), never baked in at build time: a build directory shared between
//! worktrees reuses binaries across them.

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData, CommissionId,
    ExecutorOutcome, ExecutorOutcomeNeedsHumanJudgment, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, Frontier, FrontierData, FrontierId, HumanDecisionRequest,
    PrincipalId, ProposedActionArguments, SuspensionReason, Unit, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission_testkit::fake_executor::{ExecutorCall, ScriptedExecutor};
use std::path::PathBuf;
use std::process::Command;

/// Crates the guard refuses whatever the deny list says: Loom depends on Commission, never the
/// reverse (Atlas ADR 0075), and Canon left with `story:port-skeleton`.
const ALWAYS_REFUSED: [&str; 2] = ["b10x-loom", "b10x-canon"];

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// The crate names in `model-provider-deny.txt`, in file order, one per non-blank line.
fn deny_list() -> Vec<String> {
    let path = root().join("model-provider-deny.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Everything the guard refuses: the deny list, then Loom and Canon.
fn refused() -> Vec<String> {
    let mut refused = deny_list();
    refused.extend(ALWAYS_REFUSED.iter().map(|name| (*name).to_owned()));
    refused
}

/// The guard's matcher: each refused crate a `cargo tree --prefix none` listing names, in
/// `refused` order. A line names the crate its first word spells exactly, so `foo-extras` is not
/// `foo`.
fn guard_violations(listing: &str, refused: &[String]) -> Vec<String> {
    let named: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    refused
        .iter()
        .filter(|name| named.contains(&name.as_str()))
        .cloned()
        .collect()
}

/// The real `cargo tree -p b10x-commission -e normal --all-features --target all --prefix none`
/// listing for this tree: every optional feature and every target, so a refused crate behind a
/// feature or a `cfg(target)` table is listed too.
fn real_listing() -> String {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root())
        .args([
            "tree",
            "--locked",
            "-p",
            "b10x-commission",
            "-e",
            "normal",
            "--all-features",
            "--target",
            "all",
            "--prefix",
            "none",
        ])
        .output()
        .unwrap_or_else(|error| panic!("run `cargo tree`: {error}"));
    assert!(
        out.status.success(),
        "`cargo tree` failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn uuid(n: u8) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-0000000000{n:02x}"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn commission() -> Commission<commission_state::Assigned> {
    commission_numbered(1)
}

fn commission_numbered(n: u8) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(n)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId("case-1".to_owned()),
        principal: PrincipalId("principal-1".to_owned()),
        authority_context: AuthorityContext(Value::Object(Vec::new())),
    })
}

/// A frontier with empty item lists: `story:frontier-admission` may change the item types, and
/// this test neither builds nor reads an item.
fn frontier() -> Frontier<frontier_state::Issued> {
    frontier_numbered(3)
}

fn frontier_numbered(n: u8) -> Frontier<frontier_state::Issued> {
    Frontier::new(FrontierData {
        frontier_id: FrontierId(uuid(n)),
        case_id: CaseId("case-1".to_owned()),
        case_revision: 7,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: Vec::new(),
    })
}

fn human_request() -> HumanDecisionRequest {
    HumanDecisionRequest(Value::Object(vec![(
        "question".to_owned(),
        text("approve the refund?"),
    )]))
}

/// The variant's name. Exhaustive with no wildcard: a sixth variant does not compile until this
/// test says what it is.
fn variant(outcome: &ExecutorOutcome) -> &'static str {
    match outcome {
        ExecutorOutcome::ProposedAction(_) => "ProposedAction",
        ExecutorOutcome::NeedsHumanJudgment(_) => "NeedsHumanJudgment",
        ExecutorOutcome::Suspended(_) => "Suspended",
        ExecutorOutcome::NoUsefulAction(_) => "NoUsefulAction",
        ExecutorOutcome::CompletedLocalReasoning(_) => "CompletedLocalReasoning",
    }
}

/// Calls the executor through the port, never through the fake's own inherent methods.
fn call<E: AgentExecutor>(executor: &E) -> ExecutorOutcome {
    executor.run(&commission(), &frontier())
}

#[test]
fn executor_port_contract() {
    // 1. Each of the five generated variants comes back through `AgentExecutor`, payload unchanged.
    let arguments = ProposedActionArguments(Value::Object(vec![
        ("amount".to_owned(), Value::Number("10.50".to_owned())),
        ("currency".to_owned(), text("EUR")),
    ]));
    let five = vec![
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: "refund".to_owned(),
            arguments: arguments.clone(),
        }),
        ExecutorOutcome::NeedsHumanJudgment(ExecutorOutcomeNeedsHumanJudgment {
            request: human_request(),
        }),
        ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::Budget(text("tokens exhausted")),
        }),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
    ];
    let mut names: Vec<&str> = five.iter().map(variant).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 5, "the script must cover all five variants");

    let executor = ScriptedExecutor::new(five.clone());
    for expected in &five {
        let returned = call(&executor);
        assert_eq!(
            &returned,
            expected,
            "the fake executor changed a {} outcome",
            variant(expected)
        );
    }

    // 2. `ProposedAction` carries generated `ProposedActionArguments`; `Suspended` carries a
    //    generated `SuspensionReason`, every one of its seven variants intact.
    let proposed = call(&ScriptedExecutor::new([five[0].clone()]));
    let ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
        action,
        arguments: carried,
    }) = proposed
    else {
        panic!("expected ProposedAction, found {}", variant(&proposed));
    };
    let carried: ProposedActionArguments = carried;
    assert_eq!(action, "refund");
    assert_eq!(carried, arguments);

    let reasons = vec![
        SuspensionReason::Authority(text("approval pending")),
        SuspensionReason::Budget(text("tokens exhausted")),
        SuspensionReason::Dependency(vec![CaseId("case-2".to_owned())]),
        SuspensionReason::Evidence(vec!["delivery receipt".to_owned()]),
        SuspensionReason::ExternalAvailability(text("payments api down")),
        SuspensionReason::Human(human_request()),
        SuspensionReason::Time(text("2026-10-05T09:00:00Z")),
    ];
    let suspended = ScriptedExecutor::new(
        reasons
            .iter()
            .cloned()
            .map(|reason| ExecutorOutcome::Suspended(ExecutorOutcomeSuspended { reason })),
    );
    for expected in &reasons {
        let returned = call(&suspended);
        let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended { reason }) = returned else {
            panic!("expected Suspended, found {}", variant(&returned));
        };
        let reason: SuspensionReason = reason;
        assert_eq!(&reason, expected);
    }

    // 5 (signature). `AgentExecutor::run` takes the generated `Frontier`, named here through
    //    `b10x-commission`'s `model` re-export; a hand-written frontier type would not coerce.
    let _: fn(
        &ScriptedExecutor,
        &Commission<commission_state::Assigned>,
        &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome = <ScriptedExecutor as AgentExecutor>::run;
    let frontier_type = std::any::type_name::<Frontier<frontier_state::Issued>>();
    assert!(
        frontier_type.starts_with("commission::responsibility::Frontier<"),
        "{frontier_type} is not the generated Frontier"
    );

    // 3 and 5 (dependencies). The real listing names b10x-commission itself (so the command ran
    //    on this package), and neither Loom, Canon nor any crate on the deny list.
    let refused = refused();
    let listing = real_listing();
    assert!(
        listing
            .lines()
            .any(|line| line.split_whitespace().next() == Some("b10x-commission")),
        "`cargo tree` did not print b10x-commission:\n{listing}"
    );
    let found = guard_violations(&listing, &refused);
    assert!(
        found.is_empty(),
        "b10x-commission depends on refused crates {found:?}:\n{listing}"
    );
    assert!(
        refused.iter().any(|name| name == "b10x-canon"),
        "the guard does not refuse b10x-canon"
    );

    // 4. The matcher is shown to fire: a canned listing naming Loom, and one naming the first
    //    crate on the deny list, each yield a violation naming that crate. A clean canned listing
    //    yields none, and a name that only starts like a refused crate is not that crate.
    let deny = deny_list();
    let first = deny
        .first()
        .unwrap_or_else(|| panic!("model-provider-deny.txt names no crate"));
    let clean = "b10x-commission v0.0.0 (/repo/crates/commission)\ncommission v1.0.0 (/repo/generated/rust/commission)\n";
    let with_loom = format!("{clean}b10x-loom v0.0.0\n");
    let with_provider = format!("{clean}{first} v0.1.0\n");
    assert_eq!(
        guard_violations(&with_loom, &refused),
        vec!["b10x-loom".to_owned()]
    );
    assert_eq!(
        guard_violations(&with_provider, &refused),
        vec![first.clone()]
    );
    assert!(guard_violations(clean, &refused).is_empty());
    let lookalike = format!("{clean}{first}-extras v0.1.0\n");
    assert!(guard_violations(&lookalike, &refused).is_empty());
}

/// The fake logs each call's commission id and frontier id, in call order, for the runtime loop
/// (`story:local-runtime-loop`) to assert what it ran the executor on.
#[test]
fn scripted_executor_logs_each_call() {
    let executor = ScriptedExecutor::new([
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::NoUsefulAction(Unit(true)),
        ExecutorOutcome::CompletedLocalReasoning(Unit(true)),
    ]);
    assert_eq!(executor.calls(), Vec::<ExecutorCall>::new());

    let runs = [(1, 3), (1, 4), (5, 4)];
    for (commission, frontier) in runs {
        executor.run(
            &commission_numbered(commission),
            &frontier_numbered(frontier),
        );
    }
    let expected: Vec<ExecutorCall> = runs
        .iter()
        .map(|(commission, frontier)| ExecutorCall {
            commission_id: CommissionId(uuid(*commission)),
            frontier_id: FrontierId(uuid(*frontier)),
        })
        .collect();
    assert_eq!(executor.calls(), expected);
}
