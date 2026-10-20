---
format: aep.planning-md/3
id: story:unperformable-names-actions
kind: story
status: active
title: NoPerformableAction names the actions and the open claim no effect performs
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:idle-bound-outcome
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/events.rs
- confidence: cited
  path: crates/loom-cli/tests/run_events.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/unperformable_names.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: docs/contracts/run-events-v1.json
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/intake/domains/events.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: website/docs/reference/commission
- confidence: cited
  path: website/docs/reference/run-events.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

`NoPerformableAction` names the actions the frontier listed that no effect performs, and the open
claim each was offered for. Today it is `NoPerformableAction(Unit)`
(`generated/rust/commission/src/responsibility.rs:563-564`), raised before the executor runs
(`crates/loom-commission/src/runtime.rs` module doc, item 4) or when a proposed action's effect is
not performed (item 8), and says nothing about which.

## Why

takt reports `Unperformable { claim, performer }`, exit 7, and hands the candidate to whoever can
perform it. Without the names it cannot say what to hand over.

## Acceptance

- `crates/loom-commission-testkit/tests/unperformable_names.rs::the_outcome_names_each_unperformed_action_and_its_claim`:
  a frontier listing two actions the port does not perform ends the run naming both, in frontier
  order, each with its open claim.
- `…::a_proposed_unperformed_action_is_named`: the item-8 path names the proposed action.
- The `run --output jsonl` terminal record of a `NoLocalExecutor` stop carries the actions as a new
  optional field: `crates/loom-cli/tests/run_events.rs::a_no_local_executor_stop_names_its_actions`,
  and `task docs-check` accepts the field as not a break.
- `task check` exits 0.

## ESS first

The outcome's payload is declared in `ess/commission/domains/responsibility.yaml`, and the optional
terminal-record field in `ess/intake/domains/events.yaml`, first; the red step is the conformance
scenario synthesized from them.

## Depends on

`story:idle-bound-outcome`, which changes the same `RunOutcome` declaration and files first.

## Not in scope

- Choosing a performer: Loom names what is missing and decides nothing about who serves it.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-intake-slice`, `loom-cli`.

- `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/` (regenerated)
- `ess/intake/domains/events.yaml`, `generated/rust/intake/` (regenerated)
- `crates/loom-commission/src/runtime.rs`
- `crates/loom-commission-testkit/tests/unperformable_names.rs` (new)
- `crates/loom-intake-slice/src/run.rs` (`:908`), `crates/loom-cli/src/events.rs`,
  `crates/loom-cli/tests/run_events.rs`, `docs/contracts/run-events-v1.json` (regenerated)
- `website/docs/reference/commission/`, `website/docs/reference/run-events.md`, `CHANGELOG.md`

Shared with `story:idle-bound-outcome` (before) and `story:pending-effect` (after).
