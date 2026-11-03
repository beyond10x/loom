---
format: aep.planning-md/3
id: story:turn-answers-before-its-budget-ends
kind: story
status: implemented
title: A turn with one step left asks for the answer, or ends naming that it ran out before answering
tags:
- from-consumer-report
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
- depends_on: story:turn-records-each-step
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T09:24:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T09:24:07Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T10:00:54Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

When one step, or the model calls one step needs, is left in a turn, the turn asks the model for
its answer and does not offer another read. If the turn cannot do that, it ends and names that it
ran out before answering. A turn never spends its last step on a read and then stops without an
answer and without saying so.

## Why

A downstream adopter retested on 0.55.1 (item D411, 2026-11-03). One of two runs exited 4 after
using 16 of 16 steps and 32 of 40 model calls, with 5 reads and no answer. Nothing in the turn
reserves a step for the answer: the catalogue offers reads until the step budget ends the Run.
Since 0.55.1, a read the port refuses also comes back to the model and takes a step
(story:refused-effect-goes-back-to-the-model). Whether either of these caused that run is
unverified, because the run's per-step record does not exist yet
(story:turn-records-each-step).

## ESS first

Decided at acceptance (D412): no change to `ess/`. Which actions a step offers is the plugin
turn's narrowing of the frontier, and `loom.agent` declares the two budgets but not that
narrowing. The red test is `crates/loom-plugin/tests/answer_before_budget_ends.rs`, run on the
base. The 0.55.1 tests whose model read on its last step after a performed read now expect that
step to be offered only the answer: `budget_stop_names_arithmetic.rs` uses refused reads, the case
in which a budget still ends a turn, and `turn.rs::a_turn_that_only_reads_stops_at_its_budget`
stops after 7 reads. `refused_read_goes_back.rs` is unchanged: its reads are all refused.

## Acceptance

- A turn that has performed a read and has one step left offers only the answer (or decline)
  actions, and a model that answers ends the turn with a proposal.
- A turn that has performed a read and whose model calls cover only one more step offers that
  step only the answer too.
- A turn whose later reads are refused until one step is left still gets that step for the
  answer, once one read was performed.
- A model that selects none of the answers on that step ends the turn `stopped`, its detail
  naming that it ran out before answering, the answers offered and both budgets.
- A turn that has performed no read keeps its reads to the end: `inbound-answer@1` completes a
  proposal only from a source read, so offering only the answer would end the turn on a
  proposal that completes nothing. It ends naming the budget and the last refusal, as in 0.55.1.
  (Decided in implementation, D412: the literal reading "a turn whose reads are all refused gets
  the answer step" would offer a proposal the protocol cannot complete.)

## Reproduction

- `crates/loom-plugin/tests/budget_stop_names_arithmetic.rs::a_turn_ended_by_its_step_budget_names_both_budgets`:
  with the default budgets, 8 reads use every step and the turn ends without an answer. Nothing
  reserves the last step.
- `crates/loom-plugin/tests/refused_read_goes_back.rs::a_turn_that_stops_after_a_refused_read_names_the_refusal`:
  3 refused reads use a step budget of 3, and the turn ends without an answer.

## Out of scope

Raising the default budgets.
