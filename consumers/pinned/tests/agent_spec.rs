//! `story:pinned-consumers-agency-shape`: an agent defined as data, a `loom.agent/1` document,
//! runs through `b10x-loom-sdk` alone, and ends as its spec's budget and stop policy say.
//!
//! The consumer's library parses the document with `loom_sdk::agent::parse`, maps its
//! `session.budget` and `session.stop` onto Commission's types as the agent's `Limits`, and runs
//! one commission on `system-query@1` through `loom_sdk::Runtime`, with `NoEvents` and a memory
//! case store, on a recorded executor answer and a recorded clock reading. A reading is never
//! evidence, so the case never completes: only the spec's limits end the run. No model, network
//! or harness program is reached.

use loom_pinned_consumer::agency::{self, SPEC};
use loom_sdk::agent;
use loom_sdk::commission::model::responsibility::{
    BudgetLimit, EffectOutcome, RunBudgetBreach, RunOutcome, RunOutcomeIdleBound,
    RunOutcomeSuspended, SuspensionReason,
};

/// `SPEC` with `from` replaced by `to`, which it holds exactly once.
fn with(from: &str, to: &str) -> String {
    assert_eq!(
        SPEC.matches(from).count(),
        1,
        "the spec holds `{from}` exactly once:\n{SPEC}"
    );
    SPEC.replace(from, to)
}

fn run(text: &str) -> loom_sdk::LoopEnd {
    let spec = agent::parse(text).unwrap_or_else(|e| panic!("the spec decodes: {e}\n{text}"));
    agency::run(&spec).unwrap_or_else(|e| panic!("the agent runs: {e}"))
}

#[test]
fn an_agent_defined_as_data_runs() {
    // As written: at most 20 steps and 3 idle iterations in a row. Every recorded reading after
    // the first changes nothing, so the idle bound ends the run.
    let end = run(SPEC);
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 3 }),
        "the spec's idle bound ends the run: {end:?}"
    );
    assert!(
        end.effects
            .iter()
            .all(|effect| matches!(effect, EffectOutcome::Performed(_))),
        "every admitted read was performed through the consumer's effect port: {:?}",
        end.effects
    );
    assert!(!end.effects.is_empty(), "the agent read the clock");

    // Another idle bound in the spec, another end.
    let end = run(&with("idle_bound: 3", "idle_bound: 5"));
    assert_eq!(
        end.outcome,
        RunOutcome::IdleBound(RunOutcomeIdleBound { idle_iterations: 5 }),
        "the spec's idle bound of 5 ends the run: {end:?}"
    );

    // The spec's `max_steps` is its run budget's: below the idle bound, the Run is suspended
    // for that limit, two steps charged.
    let end = run(&with("max_steps: 20", "max_steps: 2"));
    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: SuspensionReason::RunBudget(RunBudgetBreach {
                action: None,
                limit: BudgetLimit::MaxSteps,
                max: 2,
                charged: Some(2),
            }),
        }),
        "the spec's max_steps suspends the run: {end:?}"
    );
    assert_eq!(end.steps, 2, "the run took the spec's two steps");
}

#[test]
fn the_spec_declares_the_limits_the_run_is_held_to() {
    // The consumer's spec decodes as a `loom.agent/1` document whose limits are the ones the test
    // above varies.
    let spec = agent::parse(SPEC).unwrap_or_else(|e| panic!("the spec decodes: {e}"));
    let session = spec.session.expect("the spec declares a session");
    assert_eq!(session.budget.max_steps, Some(20));
    assert_eq!(session.stop.idle_bound, Some(3));
    assert!(session.stop.approval_gate);
}
