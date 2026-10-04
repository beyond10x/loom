---
format: aep.planning-md/3
id: story:effect-invocation
kind: story
status: draft
title: After the recheck, the Commission runtime invokes the effect through the action's binding
refs:
- provider: commission
  reference: story:effect-invocation
relations:
- decomposes: epic:commission-core
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 1
---
> Re-filed from `beyond10x/commission` `story:effect-invocation` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

After the recheck, the Commission runtime invokes the effect of a selected action through the
action's binding. Atlas ADR 0082 (operator decision of 2026-10-04, option A): executors (Loom, a
human tool, a workflow executor, test fakes) only return a `ProposedAction`; Commission makes the
invocation. This answers loom `decision-blocker:effect-invocation-owner` (cleared there).

For an action request the run has turned a `ProposedAction` into, and immediately before the effect,
the runtime:

1. revalidates the request against the current case revision and frontier
   (`story:stale-revision-action-request`);
2. asks authority at the moment of the call (`story:authority-provider-port`); a deny, an
   approval-required or a provider failure stops it;
3. only then invokes through the action's binding, Connectors inside Substrate, once.

An action with no binding is refused by name and nothing is invoked. A refusal at any step invokes
nothing. The executor never sees the binding, the Connection or the result of the invocation.

This replaces the reason `story:local-runtime-loop` gives for executing no effect ("the sources put
execution bindings in Loom"); that story still executes none, and this one adds the step after it.

## Blocked

Not ready to schedule. Open decisions stop it, and no answer is written here:

- `decision-blocker:action-operation-binding`: who declares action -> Connector operation, and how
  many (moved here from loom `decision-blocker:action-operation-binding`, ADR 0082 § Open).
- `decision-blocker:read-action-effect-path`: whether read actions take this same path
  (ADR 0082 § Open).
- `decision-blocker:invocation-attempt-record`: whether Commission records the Connector attempt an
  invocation produced, and how many one request may have.
- loom `decision-blocker:connector-substrate-containment`: whether Substrate is the Connector
  provider or the confinement the invocation runs in. Open, filed in the loom store; not duplicated
  here.

## ESS first

Atlas ADR 0080: the first commit changes only the specification, a named test is red on it, later
commits make it pass.

- **Specification change (first commit, `ess/domains/responsibility.yaml` only):** a command that
  invokes the effect of a revalidated action request, with outcomes invoked, refused as stale,
  refused as not admitted, refused by authority and refused as unbound; and the port vocabulary for
  the binding the runtime invokes through. Its relations are not written until the blockers above
  clear: the Commission ESS gate admits no open question, so this commit cannot be made yet.
- **Red on that commit:** `drift_passes_on_the_committed_tree`
  (`crates/commission-xtask/tests/checks.rs`, `task drift`) fails because the committed
  `generated/rust/commission/` no longer matches a fresh synthesis of `ess/`.
- **Then:** `task generate`, the invocation step and the test `effect_invoked_only_after_recheck`.

## Domain relations

- Frontier action -> Connector operation (the binding): UNSETTLED,
  `decision-blocker:action-operation-binding`.
- Action request -> Connector attempt (`connectors.mutations.AttemptRecord`): UNSETTLED,
  `decision-blocker:invocation-attempt-record`.
- Connector attempt <-> `substrate.operations.AcceptedOperation`: UNSETTLED, loom
  `decision-blocker:connector-substrate-containment`.
- Read action -> effect path: UNSETTLED, `decision-blocker:read-action-effect-path`.

## Shared surface

Depends on `story:stale-revision-action-request` (the request and its revalidation),
`story:authority-provider-port` (the authority recheck) and `story:local-runtime-loop` (the loop the
step joins, in `crates/commission/src/runtime.rs`, which that story fills). It edits
`ess/domains/responsibility.yaml` and `generated/rust/commission/`, so it cannot share a wave with
another story that does.

## Scope

- `crates/commission/src/runtime.rs` (the step after revalidation)
- `crates/commission/src/ports/effect.rs`, `crates/commission-testkit/src/fake_effect.rs` (new;
  not created by `story:port-skeleton`)
- `crates/commission-testkit/tests/effect_invocation.rs` (new)
- `ess/domains/responsibility.yaml`, `generated/rust/commission/`

## Acceptance

The test `effect_invoked_only_after_recheck` in
`crates/commission-testkit/tests/effect_invocation.rs` passes, with the scripted fake governor, the
static fake authority provider and a recording fake binding:

1. A request at the current revision, admitted by the frontier and allowed by authority, is invoked
   exactly once through its bound operation, and the recording shows the governor and authority
   calls before the invocation.
2. The same request after the fake governor moves the case from N to N+1 is refused as stale,
   naming N and N+1, and nothing is invoked.
3. Authority deny, approval-required and provider failure each invoke nothing.
4. An action with no binding is refused as unbound, naming the action, and nothing is invoked.

## Source

Atlas ADR 0082; Atlas ADR 0080; Atlas ADR 0072 (revalidate before every effect);
`docs/contracts/commission-executor.md` (executor outcomes end at `ProposedAction`).
