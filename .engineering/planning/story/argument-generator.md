---
format: aep.planning-md/3
id: story:argument-generator
kind: story
status: draft
title: Define ArgumentGenerator for the selected action only
refs:
- provider: taskboard
  reference: L-006
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:action-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

The `ArgumentGenerator` contract: arguments are generated for the selected action only (Atlas ADR
0073, step 1). The generator is handed that one action, never the rest of the catalogue, and returns
a JSON value rather than a string; the argument request is recorded against the selection it serves.

## Scope

Validation of generated arguments against the action schema (ADR 0073 step 2) is not in this story:
where an action schema comes from is held by `decision-blocker:action-argument-schema`.

## ESS first

Add the argument request command on `loom.run.ArgumentRequest` with its outcome; validate with
`ess specify validate --path ess`; regenerate the synthesized model.

## Domain relations

- `loom.run.ArgumentRequest -> loom.run.Selection`, references one; a request serves one selection
  and does not own it — inferable from `ess/domains/run.yaml`, entity `loom.run.ArgumentRequest`,
  relation `selection` (references, one, via `selection_id`).

## Acceptance

A `b10x-loom` test shows the argument generator is handed exactly the selected action and none of
the other catalogue entries, and that its JSON output reaches the returned `ProposedAction` through
a synthesized `loom.run.ArgumentRequest` referencing that selection.

## Source

TASKBOARD L-006; Atlas ADR 0073; `docs/contracts/loom-action-selection.md`;
`docs/examples/laya-fast-selection.md`.
