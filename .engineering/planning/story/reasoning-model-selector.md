---
format: aep.planning-md/3
id: story:reasoning-model-selector
kind: story
status: active
title: Reasoning-model selector chooses one action from the given candidates
summary: ActionSelector backed by the reasoning model; the stronger path fast selection falls back to.
refs:
- provider: taskboard
  reference: L-007
relations:
- decomposes: epic:fast-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom-executor
- confidence: inferred
  path: crates/loom-executor/src/harness/governed.rs
- confidence: inferred
  path: crates/loom-executor/src/harness/wire/port.rs
- confidence: inferred
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: inferred
  path: crates/loom-executor/tests/reasoning_model_selector.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 10}
---
## Outcome

Loom has an `ActionSelector` whose strategy is `ReasoningModel` (`ess/domains/run.yaml`,
`loom.run.SelectionStrategy`). It presents the reasoning model with exactly the candidate action ids it
was given, as a fixed choice, and turns the answer into a `Selection`. It is the stronger path the
confidence fallback delegates to (Atlas ADR 0073 § Fallback) and the baseline Metaharness compares fast
selectors against.

Rules this story holds:

- The model chooses only among the candidates passed in; an answer naming any other id is a selection
  error, never a selection (ADR 0073 § Decision; `docs/contracts/loom-action-selection.md` safety rules
  1 and 2).
- The selector does not generate arguments, execute, or decide authority (ADR 0073 § Decision).
- The model is reached through the provider-neutral model client ported from Harness; the selector
  names no model vendor (`AGENTS.md` § Rules).

## Domain relations

- `loom.run.Selection` → `loom.run.ActionCatalogue`, many-to-one, the selection references the catalogue and does not own it — inferable: `ess/domains/run.yaml`, entity `loom.run.Selection`, relation `catalogue`.
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom-executor/src/selection.rs:115-128`, `chosen`, which refuses an action the catalogue does not list, reached through `selection::select` at `crates/loom-executor/src/lib.rs:254` and `:475`; no ess/1 document declares this relation).

## Acceptance

With a scripted model client and a three-candidate set, the reasoning-model selector returns the
candidate the script names, and returns a selection error rather than a selection when the script
names an action outside the set.

## Dependencies

Met on `main` at 5e3d0cb: the `ActionSelector` contract (`crates/loom-executor/src/selection.rs:50`,
`story:action-selector`, implemented) and the provider-neutral model client `ModelPort`
(`crates/loom-executor/src/harness/wire/port.rs:93`, `story:harness-crate-port`, implemented).

## ESS first

No specification change: `loom.run.SelectionStrategy::ReasoningModel` (`ess/domains/run.yaml:44-46`)
and the out-of-set refusal (`loom.run.ActionNotInCatalogue`, `run.yaml:303`; `SelectAction` outcome
`not-in-catalogue`) are declared. The red test is the story's own,
`crates/loom-executor/tests/reasoning_model_selector.rs`.

## Implementation notes (scoping, 2026-10-09)

- `ActionSelector::select` takes `&self` and `ModelPort::turn` takes `&mut self`; hold the port
  behind interior mutability rather than changing the trait.
- `crates/loom-executor/src/harness/governed.rs:1022-1043` (`ModelSelection`) already records the
  governed loop's model tool call as a `ReasoningModel` selection; reuse or leave it, never
  duplicate it.
- The fixed choice is a forced tool call (`ToolChoice`, `harness/wire/turn.rs:125-168`) over the
  candidate ids only.

## Source

TASKBOARD L-007; Atlas ADR 0073; `docs/contracts/loom-action-selection.md`; `docs/design/loom-design.md`
§ Action selection strategies.
