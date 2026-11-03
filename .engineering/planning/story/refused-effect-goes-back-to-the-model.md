---
format: aep.planning-md/3
id: story:refused-effect-goes-back-to-the-model
kind: story
status: implemented
title: A refused effect of an allowed request goes back to the model instead of ending the run at the approval gate
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T06:23:11Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T06:41:45Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A request the authority provider allowed and the effect port refused without `ends_run` passes the
approval gate, as a performed one does: the next iteration runs the executor, and a plugin turn's
next model request carries the refusal (`refused: <reason>` in its earlier-steps block), so the
model can correct the read within the turn's budgets. A refusal still counts idle toward the idle
bound, and the step budget still ends the run. A turn that stops after a refused effect names the
refusal in its detail. A request the provider denied or holds for approval, and a refusal marked
final, end the run as before.

## Why

A downstream adopter's agent on Loom 0.55.0 sent a read whose query member `limit` was a number;
the effect port refused it ("source.read of `logs` has a `query` member `limit` that is not
text"), and the turn ended `stopped`, reads `[]`, detail "the run ended AwaitingApproval(...)"
(item D409, 2026-11-03). Cause, verified in code and in the adopter's state dump: the step was
allowed but not performed, so `granted` was false (`crates/loom-commission/src/runtime.rs:2081`
at 0.55.0), the gate was armed (`:2214`), and the unchanged frontier ended the Run
`AwaitingApproval` (`:1835-1843`) before the model saw the refusal.

## ESS first

No change to `ess/`: `commission.responsibility.StopPolicy` declares the gate and its
`approval_gate` switch, not which step arms it, and its text still holds. The red tests are the
rewritten `crates/loom-commission-testkit/tests/adversary_granted_gate.rs`
(`an_allowed_but_refused_step_goes_back_to_the_executor`) and
`crates/loom-plugin/tests/refused_read_goes_back.rs`, run on the base 00dc7fe8.

## Acceptance

- `adversary_granted_gate.rs::an_allowed_but_refused_step_goes_back_to_the_executor`
- `adversary_granted_gate.rs::an_allowed_step_refused_as_final_ends_the_run`
- `refused_read_goes_back.rs::a_refused_read_goes_back_to_the_model_and_the_turn_goes_on`
- `refused_read_goes_back.rs::a_turn_that_stops_after_a_refused_read_names_the_refusal`
- `docs/commission/design/approval-gate.md` states the rule and lists the case.

## Out of scope

Reading a number in an `http` query as text; a request answered `Pending` and later refused.
