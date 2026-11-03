---
format: aep.planning-md/3
id: story:turn-answers-before-its-budget-ends
kind: story
status: draft
title: A turn with one step left asks for the answer, or ends naming that it ran out before answering
tags:
- from-consumer-report
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
- depends_on: story:turn-records-each-step
revision: 1
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

To be decided when the story is accepted: whether "the last step asks for the answer" is a
declared rule of `loom.agent` (an `ess/` change and a scenario first) or plugin-turn behaviour with
its own red test (no `ess/` change).

## Acceptance

- A turn with one step left offers only the answer (or decline) actions, and a model that answers
  ends the turn with a proposal.
- A turn whose model calls cannot cover another step ends and names that it ran out before
  answering, with both budgets.
- A turn whose reads are refused until one step is left still gets that step for the answer.

## Reproduction

- `crates/loom-plugin/tests/budget_stop_names_arithmetic.rs::a_turn_ended_by_its_step_budget_names_both_budgets`:
  with the default budgets, 8 reads use every step and the turn ends without an answer. Nothing
  reserves the last step.
- `crates/loom-plugin/tests/refused_read_goes_back.rs::a_turn_that_stops_after_a_refused_read_names_the_refusal`:
  3 refused reads use a step budget of 3, and the turn ends without an answer.

## Out of scope

Raising the default budgets.
