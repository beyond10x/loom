---
format: aep.planning-md/3
id: story:pending-answer-ends-run
kind: story
status: draft
title: An effect port's pending answer can end the run waiting on it
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

An effect port can answer `Pending` and end the run at once, `Waiting` on that request, instead of
being invoked again at the next iteration. A port that does not ask for it is invoked again as
today.

## Why

On `9e63c11b`, `crates/loom-commission/src/runtime.rs:1202-1305`: a request the port answered
`Pending` is invoked again at the next iteration, with no wait between, until the port answers
otherwise or a budget ends the run; without a step budget that is up to
`UNBUDGETED_STEP_LIMIT` (64, `runtime.rs:417`, `1057`). The specification says the pause is the
port's (`ess/commission/domains/responsibility.yaml:416-422`). A host whose work cannot finish
while the process waits (a local wait the host resumes later) has no way to end the run there.

## Design

- ESS `EffectOutcomePending` gains `ends_run: Boolean`. `false` is today's answer.
- `true`: once the answer was recorded, charged and observed, the run ends `Waiting`, naming the
  request, with reason `SuspensionReason::Time` carrying the port's `report`, and the Run is
  suspended for that reason through `SuspendRun`. In a fan-out, once every read joined, the run
  ends `Waiting` naming every read answered `Pending` with `ends_run`, in proposal order, and any
  other pending reads beside them. A resumed request answered so ends the same way, its Suspension
  released first.
- The run budget's spending limits are checked first, as after any effect: a breach ends the run
  as today (`Waiting` with the breach's reason).
- How a host continues such a run (a new run that proposes the request again) is the host's.

## ESS first

`ess/commission/domains/responsibility.yaml` gains `EffectOutcomePending.ends_run`;
`task commission:generate` regenerates. Red test:
`crates/loom-commission-testkit/tests/pending_effect.rs`
`a_pending_answer_that_ends_the_run_is_not_invoked_again`.

## Acceptance

- `a_pending_answer_that_ends_the_run_is_not_invoked_again`: the port is invoked once, the run ends
  `Waiting { action_request_ids: [that request], reason: Time(<report>) }`, and the executor is
  called once.
- `a_fan_out_whose_read_ends_the_run_waits_on_every_pending_read`.
- `a_pending_answer_with_ends_run_false_is_invoked_again`: the existing pending tests stay green.
- The answer is recorded in `LoopEnd::effects`, charged one trial and observed once.

## Not in scope

- A wait or timer inside Commission between two invocations.
- Stop policy, run budget limits, completion rules.

## Scope

- `ess/commission/domains/responsibility.yaml`
- `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs`
- `crates/loom-commission-testkit/tests/pending_effect.rs`, `read_fan_out.rs`
- every `EffectOutcomePending { .. }` literal in the workspace
- `crates/loom-commission-conformance/src/codec.rs`
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/commission/` (regenerated)
