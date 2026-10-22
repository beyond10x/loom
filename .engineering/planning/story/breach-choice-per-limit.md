---
format: aep.planning-md/3
id: story:breach-choice-per-limit
kind: story
status: active
title: A stop policy chooses what becomes of a breaching step for each limit
summary: per-limit DiscardStep or KeepStep beside on_breach (takt gap C)
relations:
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T00:08:43Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T00:08:43Z", actor: "human:timo", revision: 4}
---
## Outcome

A host chooses, for each limit of its run budget, whether an executor step that breached that limit
is discarded or kept. A host that names no limit gets `on_breach` for every limit, as on 0.32.0.

## Why

On `a238395b` (0.32.0) `StopPolicy.on_breach` (`ess/commission/domains/responsibility.yaml`,
`runtime.rs:1490-1517`) is one choice for every limit. takt keeps a step that crossed its token
limit and stops before the next (`crates/takt/tests/loop_gaps.rs:286-299` in takt) and discards a
harness step that returned at or after its hour deadline (`crates/takt/src/run.rs:10366-10420`).
One `on_breach` cannot do both: takt's `dependency-blocker:loom-runtime-stops`, gap C.

Also on 0.32.0: once a spending breach is kept (`KeepStep`), the deadline is not read again after
the call (`kept_breach.is_none()`, `runtime.rs:1508`), so a call past both is never judged against
the deadline. With one choice for every limit that did not matter; with a choice per limit it does.

## Design

- ESS `StopPolicy` gains `breach_by_limit: List<commission.responsibility.LimitBreachStep>`;
  `LimitBreachStep` is `{ limit: BudgetLimit, step: BreachStep }`. A limit listed takes its own
  `step`; a limit not listed takes `on_breach`. Empty, the default, is 0.32.0's behaviour.
- A limit listed twice is a `LoopError` before the Run starts, naming the limit. `MaxSteps` and
  `MaxTrials` are count limits that stop before the step and are never an executor step's breach;
  listing them is allowed and has no effect (documented).
- Once an executor call was observed, the runtime collects every run limit the call breached: the
  spending limits in the order `spent_past` checks them, then `Deadline`. The step is discarded
  when any breached limit's choice is `DiscardStep`; the Run is then suspended for the first
  breached limit whose choice is `DiscardStep`. When every breached limit's choice is `KeepStep`
  the step is acted on and the Run suspended before the next step for the first breached limit.
- The deadline is read after the call whether or not a spending breach was kept.

## ESS first

`ess/commission/domains/responsibility.yaml` gains `LimitBreachStep` and
`StopPolicy.breach_by_limit`; `task commission:generate` regenerates. Red tests in
`crates/loom-commission-testkit/tests/stop_policy.rs`.

## Acceptance

- `a_kept_token_breach_and_a_discarded_deadline_breach_each_take_their_own_choice`: with
  `breach_by_limit: [{MaxTokens, KeepStep}, {Deadline, DiscardStep}]` and `on_breach: DiscardStep`,
  a call past the token limit only is performed once and the run then ends suspended for
  `MaxTokens` with no further call; a call that ended past the deadline only performs nothing and
  the run ends suspended for `Deadline`.
- `a_step_past_a_kept_and_a_discarded_limit_is_discarded`: a call past both the token limit
  (`KeepStep`) and the deadline (`DiscardStep`) performs nothing and the run ends suspended for
  `Deadline`.
- `a_limit_listed_twice_is_refused_before_the_run_starts`.
- `a_limit_not_listed_takes_on_breach`: `on_breach: KeepStep` with only `{Deadline, DiscardStep}`
  listed keeps a step past `MaxTokensIn`.
- The existing `on_breach` tests in `stop_policy.rs`, `run_and_action_budgets.rs` and
  `executor_usage.rs` stay green with `breach_by_limit` empty.

## Not in scope

- Action budgets (`ActionBudget`): an effect's own breach is already performed under either choice.
- The host stop (`story:host-stop-before-step`).

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/lib.rs`
- `crates/loom-commission-testkit/tests/stop_policy.rs`
- every `StopPolicy { .. }` literal in the workspace (`rg 'StopPolicy \{'`)
- `crates/loom-commission-conformance/src/codec.rs` if it encodes `StopPolicy`
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/commission/` (regenerated)
