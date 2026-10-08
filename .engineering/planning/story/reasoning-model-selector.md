---
format: aep.planning-md/3
id: story:reasoning-model-selector
kind: story
status: draft
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
  path: crates/loom-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/src/selection.rs
- confidence: inferred
  path: crates/loom-executor/tests/reasoning_model_selector.rs
revision: 5
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
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom/src/lib.rs:79-83`, the `frontier.contains_action` check in `Loom::run`; no ess/1 document declares this relation).

## Acceptance

With a scripted model client and a three-candidate set, the reasoning-model selector returns the
candidate the script names, and returns a selection error rather than a selection when the script
names an action outside the set.

## Dependencies

The `ActionSelector` contract (TASKBOARD L-005) and the model client ported from Harness, both in
`epic:loom-native-harness`. No story for either existed when this was drafted; until one does, the
edge is the epic-level `epic:fast-selector depends_on epic:loom-native-harness`.

## Source

TASKBOARD L-007; Atlas ADR 0073; `docs/contracts/loom-action-selection.md`; `docs/design/loom-design.md`
§ Action selection strategies.
