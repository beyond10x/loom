---
format: aep.planning-md/3
id: story:effect-invocation
kind: story
status: active
title: After the recheck, the Commission runtime invokes the effect through the action's binding
refs:
- provider: commission
  reference: story:effect-invocation
relations:
- decomposes: epic:commission-core
- serves: vision:O1
- serves: vision:governed-autonomy
- depends_on: story:ess-055-upgrade
scope:
- confidence: inferred
  path: crates/loom-commission-testkit/src/
- confidence: cited
  path: crates/loom-commission-testkit/tests/effect_invocation.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/
- confidence: cited
  path: crates/loom-commission/src/ports/effect.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission/
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 5, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
- {from: "proposed", to: "active", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 6, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
---
## Outcome

The Commission runtime invokes an admitted action through the action's binding to one Connector
operation, offers an executor only actions that have a binding, and records the one Connector
attempt an invocation produced. This applies the decisions of 2026-10-07 on
`decision-blocker:action-operation-binding` (B), `decision-blocker:invocation-attempt-record` (A),
`decision-blocker:read-action-effect-path` (A) and `decision-blocker:connector-substrate-containment`
(A) to the effect step `story:runtime-merge` already shipped.

What exists (`crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/ports/effect.rs`
at `657c8fa`): the runtime revalidates a request against case revision, frontier and authority, and
only then calls `EffectPort::invoke` once with an `AdmittedRequest`; `EffectPort::performs(action)`
says whether a port performs an action, and an admitted action the port does not perform ends the
run `NoPerformableAction`. The executor is handed the whole frontier.

This story adds:

1. **The binding.** The host's composition declares, per commission, which frontier action binds to
   which Connector operation (`instance_id`, `operation_id`): exactly one operation per bound action,
   at most one binding per action id. Protocol definitions and the Connectors catalog carry no
   binding. Which Connection serves the operation is Connectors' choice and is not modelled here.
2. **Unbound actions that need no authority are never offered.** Before the executor runs, the
   runtime removes from the frontier it hands over every action the effect port does not perform
   (for a Connector-backed port: every action without a binding) **and** that needs no authority,
   so a Loom catalogue never lists one. An action behind an authority gate stays visible even when
   no port performs it (`decision-blocker:gated-unbound-action-visibility`, option B), so a run
   stops at the gate as today; an approved action no port performs ends `NoPerformableAction` at
   invocation. A frontier left with no action still ends `NoPerformableAction`, as today.
3. **One attempt per invoked request.** `EffectOutcome::Performed` names exactly one Connector
   attempt: the reference to the `connectors.mutations.AttemptRecord` the invocation produced. A
   request refused at the recheck is never invoked and names none. The runtime never retries an
   invocation; a retry is a new action request, rechecked.
4. **One path for every action.** Read actions (`repository.inspect`, `logs.search`,
   `metrics.inspect`) take the same recheck and the same invocation as consequential ones; nothing
   distinguishes them.
5. **A Connector-backed effect port behind a port of its own.** `ConnectorEffects` implements
   `EffectPort` from a commission's bindings and invokes through a `ConnectorInvoker` port, one call
   per admitted request. The real Connectors client is not part of this story: Connectors declares
   the Substrate provider (a Substrate daemon is a Connection, decision of 2026-10-07) in its own
   repository first. The testkit supplies a recording invoker.

`b10x-loom-commission` keeps its dependency rule: no executor, Canon or model-provider crate, and no
Connectors crate (the invoker is a port).

## ESS first

Atlas ADR 0080: the first commit changes only `ess/commission/`, a named test is red on it, later
commits make it pass without changing `ess/commission/`.

- **Specification change (first commit, `ess/commission/domains/responsibility.yaml`):** the binding
  noun (an action id, a Connector `instance_id` and `operation_id`) with its relation to the
  commission whose composition declares it (cardinality: many bindings per commission, one per action
  id); the Connector attempt reference on `EffectOutcomePerformed` (exactly one). Model them with
  `ess:specifying`, validate with the newest `ess` (`--strict-requires`). If ESS cannot express the
  reference to a Connectors entity across systems, the reference is an identifier type declared
  here and the limit is reported, not worked around by hand.
- **Red on that commit:** `task commission:drift` fails, because the committed
  `generated/rust/commission/` no longer matches a fresh synthesis.
- **Then:** `task commission:generate`, the runtime filter, `ConnectorEffects`, and the test below.

## Acceptance

The test `effect_invoked_only_through_its_binding` in
`crates/loom-commission-testkit/tests/effect_invocation.rs` passes, with the scripted fake governor,
the static fake authority provider, a recording executor and a recording `ConnectorInvoker`:

1. An admitted request for a bound action is invoked exactly once, through its bound
   (`instance_id`, `operation_id`), after the governor and authority calls (the recording shows the
   order), and its `Performed` outcome names exactly one attempt.
2. The recording executor is never handed an unbound action that needs no authority: with a
   frontier listing one bound and one unbound ungated action, the frontier it receives lists only the
   bound one.
3. A frontier whose only action is unbound and ungated ends `NoPerformableAction`, and nothing is
   invoked.
4. A bound read action (`repository.inspect`) is rechecked and invoked by the same path as a
   consequential one.
5. A stale request, an authority deny and an approval-required each invoke nothing.
6. An unbound action behind an authority gate stays in the frontier the executor receives; the run
   stops at the gate (`ApprovalRequired` or `NeedsAuthority`), and once approved, its invocation
   ends `NoPerformableAction` with nothing invoked. The slice keeps stopping at
   `ApprovalRequired (repository.merge)`: the existing intake-slice and CLI tests that assert it
   still pass unchanged.

`task commission:deps-guard` still passes: `b10x-loom-commission` names no Connectors crate.

## Scope

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`
- `crates/loom-commission/src/runtime.rs` (the frontier filter before the executor)
- `crates/loom-commission/src/ports/effect.rs`, `crates/loom-commission/src/ports/` (new
  `ConnectorInvoker` port and `ConnectorEffects`; inferred placement)
- `crates/loom-commission-testkit/src/` (recording invoker; inferred)
- `crates/loom-commission-testkit/tests/effect_invocation.rs` (new)
- `crates/loom-intake-slice/src/` only if `LocalEffects` must change to keep compiling (inferred)

## Shared surface

Edits `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` and
`crates/loom-commission/src/runtime.rs`, so no other unit that edits any of them shares its wave
(`story:moved-case-outcome`, `story:ess-055-upgrade`).

## Source

Atlas ADR 0082 (Commission invokes effects); Atlas ADR 0080 (specification first); Atlas ADR 0072
(revalidate before every effect; candidates are integrations intersected with admissible actions);
the four decisions recorded on the blockers named in the Outcome.
