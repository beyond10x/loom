//! Adversary cases for `story:pinned-consumers-agency-shape`: the consumer's library runs a
//! `loom.agent/1` spec through `loom_sdk::Runtime`, and the run must end where the spec's budget
//! and stop policy say. Each case varies one field the spec declares and asserts the end the
//! spec names, on the consumer's own recorded executor and effect port. No model, network or
//! harness program is reached.

use loom_pinned_consumer::agency::{self, AgentLimits, FixedNow, SPEC};
use loom_sdk::agent;
use loom_sdk::commission::model::primitives::Timestamp;
use loom_sdk::commission::model::responsibility::{
    BreachStep, BudgetLimit, LimitBreachStep, RunBudgetBreach, RunOutcome, RunOutcomeIdleBound,
    RunOutcomeSuspended, SuspensionReason,
};

/// `SPEC` with `from` replaced by `to`, which it holds exactly once.
fn with(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "the spec holds `{from}` exactly once:\n{text}"
    );
    text.replace(from, to)
}

fn parse(text: &str) -> agent::AgentSpec {
    agent::parse(text).unwrap_or_else(|e| panic!("the spec decodes: {e}\n{text}"))
}

/// The instant the consumer's tests run at unless a case names another.
const NOW: &str = "2026-01-01T00:00:00Z";

fn run(text: &str) -> loom_sdk::LoopEnd {
    run_at(text, NOW)
}

/// Runs the spec `text` on a host clock reading `now`.
fn run_at(text: &str, now: &str) -> loom_sdk::LoopEnd {
    agency::run(&parse(text), FixedNow(Timestamp(now.to_owned())))
        .unwrap_or_else(|e| panic!("the agent runs: {e}"))
}

/// A deadline the spec states that is already past on the day the run starts stops the run
/// before any step. The consumer once froze its clock at `2026-01-01T00:00:00Z`, so it read
/// `2026-06-01T00:00:00Z` as still ahead whatever the day, and ran the agent to its idle bound.
/// The clock is the host's now: this case hands it the day the wave that added it ran.
#[test]
fn adversary_a_deadline_already_past_when_the_run_starts_stops_it_before_any_step() {
    let text = with(
        SPEC,
        "max_steps: 20",
        "max_steps: 20, deadline: '2026-06-01T00:00:00Z'",
    );
    let end = run_at(&text, "2026-10-23T00:00:00Z");
    assert!(
        matches!(
            &end.outcome,
            RunOutcome::Suspended(RunOutcomeSuspended {
                reason: SuspensionReason::RunBudget(RunBudgetBreach {
                    limit: BudgetLimit::Deadline,
                    ..
                }),
            })
        ),
        "the spec's deadline has passed, so the run is suspended for it: {:?}",
        end.outcome
    );
    assert_eq!(end.steps, 0, "no step runs past the spec's deadline");
    assert!(
        end.effects.is_empty(),
        "nothing is invoked past the deadline"
    );
}

/// Probe: a deadline before the consumer's own clock stops the run before any step.
#[test]
fn adversary_a_deadline_before_the_consumers_clock_stops_the_run_before_any_step() {
    let text = with(
        SPEC,
        "max_steps: 20",
        "max_steps: 20, deadline: '2025-12-31T23:59:59Z'",
    );
    let end = run(&text);
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::Deadline,
                max: 1_767_225_599,
                charged: Some(1_767_225_600),
            }),
        }),
    );
    assert_eq!(end.steps, 0);
    assert!(end.effects.is_empty());
}

/// Probe: the spec's per-action budget bounds the clock read's trials.
#[test]
fn adversary_a_per_action_trial_budget_suspends_the_run_for_that_action() {
    let text = with(
        SPEC,
        "max_steps: 20",
        "max_steps: 20, per_action: [{action: system.time.read, max_trials: 1}]",
    );
    let end = run(&text);
    match &end.outcome {
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(breach),
        }) => {
            assert_eq!(breach.action.as_deref(), Some("system.time.read"));
            assert_eq!(breach.limit, BudgetLimit::MaxTrials);
            assert_eq!(breach.max, 1);
        }
        other => panic!("the per-action trial budget suspends the run: {other:?}"),
    }
    assert_eq!(
        end.effects.len(),
        1,
        "the read was tried once: {:?}",
        end.effects
    );
}

/// Probe: an idle bound below 1 is read as 1.
#[test]
fn adversary_an_idle_bound_of_zero_is_read_as_one() {
    let end = run(&with(SPEC, "idle_bound: 3", "idle_bound: 0"));
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 1 }),
    );
}

/// Probe: a step budget of zero lets no step run.
#[test]
fn adversary_a_max_steps_of_zero_runs_no_step() {
    let end = run(&with(SPEC, "max_steps: 20", "max_steps: 0"));
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxSteps,
                max: 0,
                charged: Some(0),
            }),
        }),
    );
    assert_eq!(end.steps, 0);
    assert!(end.effects.is_empty());
}

/// Probe: a capability or tool the spec names grants nothing; the run ends as without them.
#[test]
fn adversary_a_spec_naming_tools_and_capabilities_grants_nothing_new() {
    let text = with(
        SPEC,
        "    decisions: ask",
        "    decisions: ask\n    allow: [Bash]\npolicy:\n  capabilities: [repository.merge, system.time.read]",
    );
    let end = run(&text);
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 3 }),
    );
    assert!(
        end.admitted
            .iter()
            .all(|request| request.action == agency::ACTION),
        "only the clock read was admitted: {:?}",
        end.admitted
    );
}

/// Probe, mutant catcher: every stop-policy field reaches Commission's `StopPolicy` as written,
/// `on_breach` and each `breach_by_limit` entry included. The run tests above cannot tell
/// `KeepStep` from `DiscardStep`: the consumer's executor reports no usage and its clock never
/// moves, so no step ever ends past a limit.
#[test]
fn adversary_every_stop_policy_field_maps_as_written() {
    let text = with(
        SPEC,
        "on_breach: DiscardStep",
        "on_breach: KeepStep, breach_by_limit: [{limit: Deadline, step: DiscardStep}, {limit: MaxTokens, step: KeepStep}]",
    );
    let limits = AgentLimits::of(&parse(&text));
    let stop = limits.stop.expect("a session's stop policy");
    assert!(stop.approval_gate);
    assert_eq!(stop.idle_bound, Some(3));
    assert_eq!(stop.on_breach, BreachStep::KeepStep);
    assert_eq!(
        stop.breach_by_limit,
        vec![
            LimitBreachStep {
                limit: BudgetLimit::Deadline,
                step: BreachStep::DiscardStep,
            },
            LimitBreachStep {
                limit: BudgetLimit::MaxTokens,
                step: BreachStep::KeepStep,
            },
        ]
    );
}

/// Probe, mutant catcher: every run-budget field reaches Commission's `RunBudget` as written.
#[test]
fn adversary_every_run_budget_field_maps_as_written() {
    let text = with(
        SPEC,
        "max_steps: 20",
        "max_steps: 20, max_tokens_in: 11, max_tokens_out: 12, max_duration_ms: 13, \
         max_cost_micro_usd: 14, max_tokens: 15, deadline: '2030-01-01T00:00:00Z', \
         per_action: [{action: system.time.read, max_trials: 21, max_tokens_in: 22, \
         max_tokens_out: 23, max_duration_ms: 24, max_cost_micro_usd: 25}]",
    );
    let budget = AgentLimits::of(&parse(&text))
        .budget
        .expect("a session's budget");
    assert_eq!(budget.max_steps, Some(20));
    assert_eq!(budget.max_tokens_in, Some(11));
    assert_eq!(budget.max_tokens_out, Some(12));
    assert_eq!(budget.max_duration_ms, Some(13));
    assert_eq!(budget.max_cost_micro_usd, Some(14));
    assert_eq!(budget.max_tokens, Some(15));
    assert_eq!(
        budget.deadline.map(|d| d.0).as_deref(),
        Some("2030-01-01T00:00:00Z")
    );
    let action = &budget.per_action[0];
    assert_eq!(action.action, "system.time.read");
    assert_eq!(
        (
            action.max_trials,
            action.max_tokens_in,
            action.max_tokens_out,
            action.max_duration_ms,
            action.max_cost_micro_usd
        ),
        (Some(21), Some(22), Some(23), Some(24), Some(25))
    );
}

/// Probe: a spec without a session runs under Commission's own limits, which end it after two
/// idle iterations.
#[test]
fn adversary_a_spec_without_a_session_ends_at_commissions_idle_bound() {
    let text: String = SPEC
        .split_inclusive('\n')
        .take_while(|line| !line.starts_with("session:"))
        .collect();
    let end = run(&text);
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 2 }),
    );
}
