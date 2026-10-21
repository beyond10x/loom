---
format: aep.planning-md/3
id: story:stop-policy-gate-and-idle-bound
kind: story
status: implemented
title: A host chooses whether an unchanged frontier awaits approval and how many idle steps end a run
relations:
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/stop_policy.rs
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: docs/commission/design/approval-gate.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/commission/system.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T18:44:05Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T18:44:06Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T22:26:37Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":2,"review_outcome":1}}}
---
## Outcome

A host chooses, per run, whether an unchanged frontier that lists actions needing approval ends the
run `AwaitingApproval` (the approval gate) or counts as an idle iteration, and how many idle
iterations in a row end the run `IdleBound`, with or without a step budget. A host that chooses
nothing gets today's behaviour.

## Why

On `9e63c11b`, `crates/loom-commission/src/runtime.rs`:

- The approval gate arms whenever a handed frontier lists an action needing approval that was not
  granted (`1604-1608`) and ends the next iteration on an unchanged frontier `AwaitingApproval`
  (`1307-1315`). A protocol whose frontier always lists such an action (`software.change/1` lists
  `release.rollback`) ends after any step that changed nothing, even when the host wants that step
  judged as no progress.
- The idle bound applies only without a step budget (`1587-1594`) and ends the run at two idle
  iterations (`judge_idle`, `JudgeIdleIteration` `when: idle_iterations >= 2`). A host with a step
  budget that wants a run ended after one idle step cannot ask for it.

Both are documented semantics that the intake slice, plugin turns and other hosts rely on
(`crates/loom-intake-slice/src/run.rs:42`, `894`, `1095`; `crates/loom-plugin/src/turn.rs:22`,
`748`), so the defaults stay and the choice is opt-in.

## Design

- ESS: `commission.responsibility.StopPolicy`, a struct the host supplies and the model never does:
  `approval_gate: Boolean` and `idle_bound: Optional<Integer>`.
- `LoopContext::stop_policy(&self) -> StopPolicy`, a provided method returning the default
  (`approval_gate: true`, `idle_bound: None`), so no existing `LoopContext` changes.
- `approval_gate: false`: item 5 of the runtime never ends a run `AwaitingApproval`. The iteration
  it would have ended goes to the executor, and a step that leaves the frontier unchanged is judged
  as any other step, idle when it admitted no new request. Authority is untouched: a proposal of an
  action that needs approval is still asked about, and one the provider does not allow is never
  handed to the effect port.
- `idle_bound: Some(n)`: `n` idle iterations in a row end the run `IdleBound` whether or not a step
  budget is set; a value below 1 is read as 1, so a host cannot turn the bound off with it.
  `None` keeps the bound at two, applied only without a step budget, as today.
- `JudgeIdleIteration` takes the bound as an input so the generated behaviour decides with the
  host's value; if ESS cannot compare two inputs, stop and report it.
- `docs/commission/design/approval-gate.md` and the runtime module docs say what the policy
  changes.

## ESS first

`ess/commission/domains/responsibility.yaml` gains `StopPolicy` and the `bound` input of
`JudgeIdleIteration`; `task commission:generate` regenerates. The red test is
`crates/loom-commission-testkit/tests/stop_policy.rs`
`an_unchanged_frontier_without_the_gate_is_an_idle_iteration`, failing before the runtime reads the
policy.

## Acceptance

- `an_unchanged_frontier_without_the_gate_is_an_idle_iteration`: with `approval_gate: false` and
  `idle_bound: Some(1)`, a step that changes nothing on a frontier listing an approval-required
  action ends `IdleBound { idle_iterations: 1 }`, not `AwaitingApproval`.
- `an_ungranted_approval_required_action_is_still_refused_without_the_gate`: with
  `approval_gate: false`, an executor that proposes the approval-required action is answered by
  the authority provider, the effect port is never invoked for it, and the run ends
  `NeedsAuthority` (or holds a Suspension under `run_with_suspension`) as with the gate.
- `the_idle_bound_applies_with_a_step_budget_when_set`: with a step budget of 10 and
  `idle_bound: Some(1)`, one idle executor call ends the run `IdleBound { idle_iterations: 1 }`.
- `the_default_stop_policy_keeps_todays_stops`: a `LoopContext` that does not implement
  `stop_policy` ends as before (the existing gate and idle-bound tests stay green unchanged).
- `task commission:conform` passes with the new `JudgeIdleIteration` input.

## Not in scope

- Run budget limits, pending answers and completion rules (separate stories).
- The intake slice, plugin turns and `b10x-loom` keep the default policy.

## Scope

- `ess/commission/domains/responsibility.yaml`
- `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/outcome.rs`
- `crates/loom-commission/src/lib.rs` (re-export)
- `crates/loom-commission-testkit/tests/stop_policy.rs`
- `crates/loom-commission-conformance/` (the new input)
- `docs/commission/design/approval-gate.md`
- `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/commission/` (regenerated)

## Shipped

Merged into wave 2026-10-21-w7 as 65f66ac1. Adversary pass 1 (review-result:w7-adversary) attacked the gate off with a withheld, failing and panicking provider on a fan-out, a pending request, a resume and a kept step: nothing was invoked.
