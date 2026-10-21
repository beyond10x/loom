---
format: aep.planning-md/3
id: story:run-budget-tokens-and-deadline
kind: story
status: active
title: A run is bounded by total tokens and a deadline, and may keep the step that passed a limit
relations:
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T18:44:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T18:44:06Z", actor: "human:timo", revision: 3}
---
## Outcome

A host can bound a run by its total tokens and by a wall-clock deadline, and can choose that a step
whose reported usage passed a spending limit is still acted on, the run then stopping before the
next step. A host that sets none of these gets today's behaviour.

## Why

On `9e63c11b`:

- `RunBudget` (`ess/commission/domains/responsibility.yaml:500-513`) bounds input and output
  tokens separately; a host whose limit is the sum (input plus output) cannot state it.
- `max_duration_ms` sums the durations effects and executor calls report (`runtime.rs:2946-2980`,
  `spent_past`). A host whose limit is wall time since the run started, whatever the calls
  report, has no limit to set.
- An executor call whose reported usage takes the run past a spending limit ends the run before its
  outcome is acted on (`runtime.rs:1320-1337`): its proposal is discarded. A host whose own loop
  keeps that step and stops before the next one cannot ask for it.

The discard is documented semantics (fail toward less effect, AGENTS.md § Rules), so it stays the
default and the other choice is opt-in.

## Design

- ESS `RunBudget` gains `max_tokens: Optional<Integer>` (input plus output tokens charged to the
  run, from effects and executor calls, checked with the other spending limits) and
  `deadline: Optional<Timestamp>` (RFC 3339). `BudgetLimit` gains `MaxTokens` and `Deadline`.
- The deadline is read against `LoopContext::now()` before each step: before each executor call and
  before a pending request is invoked again. At or after it, the Run is suspended for `RunBudget`
  with limit `Deadline`, `max` the deadline and `charged` the reading, both in whole seconds since
  the Unix epoch, and the run ends `Suspended` (or `Waiting` while a request is pending, as at any
  budget stop). A deadline that is not RFC 3339 is a `LoopError` before the Run starts; a clock
  reading that is not is taken as past the deadline, with `charged` absent. Commission parses
  RFC 3339 itself and takes no time crate.
- ESS `StopPolicy` (from the stop-policy story) gains `on_breach: BreachStep`, an enum
  `[DiscardStep, KeepStep]`, default `DiscardStep`. `KeepStep`: when an executor call's reported
  usage took the run past a spending limit, the step's outcome is acted on as with no breach (a
  proposal is revalidated, asked about and performed only when admitted), and where the run would
  then go on it is suspended for the breach before the next step. A step whose outcome ends the run
  ends it with that outcome. `KeepStep` performs a proposal after a spending limit was passed; an
  effect's own usage breach is unchanged (that effect is already performed).

## ESS first

`ess/commission/domains/responsibility.yaml` gains the two `RunBudget` fields, the two
`BudgetLimit` variants, `BreachStep` and `StopPolicy.on_breach`; `task commission:generate`
regenerates. Red tests in `crates/loom-commission-testkit/tests/run_and_action_budgets.rs`:
`max_tokens_counts_input_and_output_together`, `a_passed_deadline_stops_the_run_before_the_next_step`,
`keep_step_acts_on_the_step_that_passed_a_spending_limit`.

## Acceptance

- `max_tokens_counts_input_and_output_together`: a run with `max_tokens: 100` whose executor call
  reports 60 in and 50 out is suspended for `RunBudget { limit: MaxTokens, max: 100, charged: 110 }`.
- `a_passed_deadline_stops_the_run_before_the_next_step`: with a clock past the deadline after the
  first step, the executor is called once and the run ends suspended for `Deadline`.
- `a_deadline_that_is_not_rfc_3339_is_refused_before_the_run_starts`.
- `keep_step_acts_on_the_step_that_passed_a_spending_limit`: with `on_breach: KeepStep`, the
  proposal of the step that passed the limit is performed once, and the run then ends suspended
  for the breach with no further executor call.
- `keep_step_still_refuses_an_unadmitted_proposal`: the kept step's proposal of an action the
  provider does not allow is never handed to the effect port.
- With `DiscardStep` (the default), the existing executor-usage tests stay green unchanged.

## Not in scope

- Per-action total tokens or deadlines (`ActionBudget` is unchanged).
- Approval gate and idle bound (stop-policy story), pending answers, completion rules.

## Scope

- `ess/commission/domains/responsibility.yaml`
- `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs`
- `crates/loom-commission-testkit/tests/run_and_action_budgets.rs`, `executor_usage.rs`
- `crates/loom-governor/src/file_suspension.rs` (the new limits round-trip)
- `crates/loom-commission-conformance/src/codec.rs`
- every `RunBudget { .. }` literal in the workspace's tests (`loom-session-executor`, testkit)
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/commission/` (regenerated)
