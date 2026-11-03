---
format: aep.planning-md/3
id: story:turn-records-each-step
kind: story
status: active
title: 'A turn records each step: action, effect outcome, refusal reason and the budget left'
tags:
- from-consumer-report
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T09:24:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T09:24:07Z", actor: "human:timo", revision: 3}
---
## Outcome

A turn's record says what each step did, in order: the action selected, the effect port's outcome
(performed, refused, pending, or not invoked), the refusal reason when there was one, and the
step and model-call budget left after it. It is either in the run state the turn records or in one
log line per step. A turn that ends without an answer can then be read back to the step where it
went wrong, from the record alone.

## Why

A downstream adopter retested on 0.55.1 (item D411, 2026-11-03). One of two runs passed, and the
0.55.0 defects (one forced call, a refused read) did not recur. The other run exited 4 after using
16 of its 16 steps and 32 of its 40 model calls, with only 5 reads and no answer. Its cause is
unknown, because the pod log keeps only the last line and the turn's record holds only its reads
and the stop detail. The record does not say which steps were not reads, or what they did. The
adopter suggests two possible causes, both unverified: refused-argument retries using up steps, or
no step being reserved for the answer. The runtime already holds the data in memory: `LoopEnd`'s
`requests` and `effects` (`crates/loom-commission/src/runtime.rs`). The plugin turn passes on only
`reads` and the detail (`crates/loom-plugin/src/turn.rs`, `stopped`).

## ESS first

Decided at acceptance (D412): the steps are a declared member, in `ess/` first. The ESS commit
adds `loom.plugin.StepOutcome` (Performed, Pending, Refused, Failed, NotInvoked), and
`loom.plugin.TurnStep` with the members `action` (optional), `outcome`, `reason` (optional),
`steps_left` and `model_calls_left`. It also adds `steps: List<loom.plugin.TurnStep>` to
`loom.plugin.TurnResult` and to `loom.plugin.RecordLine`. On that commit `task drift` fails, and
`crates/loom-plugin/tests/turn_records_each_step.rs` does not compile. A record line written
before 0.55.2 has no `steps` and is read with none.

## Acceptance

- A turn that ends on its step budget after reads, refused reads and other actions records one
  entry per step: the action, the effect outcome, the refusal reason, and the budget left after it.
- A turn that answers records the same entries, with the last entry being its proposal.
- No entry carries a read's answer text or an argument value beyond what the record already
  holds.

## Reproduction

None yet. No test in loom asserts what a stopped turn records per step.

## Out of scope

Deciding the adopter's run-2 cause. That needs this record first.
