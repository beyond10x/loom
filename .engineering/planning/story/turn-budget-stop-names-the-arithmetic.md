---
format: aep.planning-md/3
id: story:turn-budget-stop-names-the-arithmetic
kind: story
status: implemented
title: A turn that ends on its budgets names which one and the two model calls a step
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T06:41:46Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A plugin or agent turn that ends on its budgets without an answer says which budget ended it and
the arithmetic between them: each step asks the model twice, a selection and its arguments. A
turn the step budget ends is recorded "the turn used its N steps (step_budget) and M of its B
model calls (model_budget) without an answer, after R reads; each step asks the model twice, a
selection and its arguments". A turn the model budget ends first is recorded "the turn used its B
model calls (model_budget) after S of its N steps (step_budget) without an answer; each step asks
the model twice, a selection and its arguments, so B model calls allow B/2 steps". Either ends
with "; the last effect was refused: <reason>" when the last effect was refused.

## Why

A downstream adopter's agent with `step_budget: 8` and `model_budget: 16` performed 8 reads and
was recorded "the turn used its 16 model calls" (item D409, 2026-11-03). The step budget ended the
Run, and the detail named the model budget only, because the turn matched `asked >= model_budget`
first (`crates/loom-plugin/src/turn.rs:522` at 0.55.0). The smaller of the two changes the item
offers is taken: a clear detail, no new validation of a spec, since 8 and 16 are consistent
budgets and a guard would refuse nothing here.

## ESS first

No change to `ess/`: `loom.agent` declares the two budgets; the record's `detail` text is not
declared. The red test is `crates/loom-plugin/tests/budget_stop_names_arithmetic.rs`, run on the
base 00dc7fe8.

## Acceptance

- `budget_stop_names_arithmetic.rs::a_turn_ended_by_its_step_budget_names_both_budgets`
- `budget_stop_names_arithmetic.rs::a_turn_ended_by_its_model_budget_names_the_steps_it_allows`
- `crates/loom-cli/tests/agent_run.rs::budgets_hold` and the model-budget cases of
  `crates/loom-plugin/tests/adversary_2.rs` and `crates/loom-cli/tests/adversary_w20261102w2_a.rs`
  hold the new detail.

## Out of scope

A spec check that refuses `model_budget < 2 * step_budget`.
