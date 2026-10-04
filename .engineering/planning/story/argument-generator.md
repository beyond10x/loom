---
format: aep.planning-md/3
id: story:argument-generator
kind: story
status: draft
title: Define ArgumentGenerator for the selected action only
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-006
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:action-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
scope:
- confidence: inferred
  path: crates/loom/src/arguments.rs
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/tests/argument_generator.rs
revision: 8
---
## Outcome

The `ArgumentGenerator` contract: arguments are generated for the selected action only (Atlas ADR
0073, step 1). The generator is handed that one catalogue entry, never the rest of the catalogue,
and returns a JSON value rather than a string; the argument request is recorded against the
selection it serves. The value reaches the `ProposedAction` as Commission's generated
`ProposedActionArguments` (commission `story:agent-executor-port`, M-004). The bootstrap
`ArgumentGenerator::generate(&ActionCandidate, …) -> String` (`crates/loom/src/lib.rs:18-20`) is
replaced: `ActionCandidate` is Canon's bootstrap type, deleted by Canon's wave 1.

## Scope

- Validation of generated arguments against the action schema (ADR 0073 step 2) is not in this
  story: where an action schema comes from is held by `decision-blocker:action-argument-schema`.
- Files: `crates/loom/src/arguments.rs` (created empty by `story:run-pipeline-skeleton`, filled
  here), `crates/loom/src/lib.rs` (the executor pipeline),
  `crates/loom/tests/argument_generator.rs` (new). Not `ess/` or `generated/`.

## Shared surface

Depends on `story:action-selector` for behaviour: the generator is handed the catalogue entry the
catalogue-based selection names, which that story introduces into the executor pipeline. Depends on
`story:run-pipeline-skeleton` for the argument request command and the `arguments` module. It edits
the executor pipeline in `crates/loom/src/lib.rs`, as `story:action-selector`,
`story:selection-revalidation` and `story:harness-loop-port` do, so it is kept apart from them.
`story:selection-revalidation` depends on it. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

- **Declarations relied on** (landed by `story:run-pipeline-skeleton`): `loom.run.RequestArguments`
  and its `requested` outcome creating `loom.run.ArgumentRequest`. This story does not change
  `ess/`.
- **Red test:** the first commit adds `arguments_for_selected_action_only` in
  `crates/loom/tests/argument_generator.rs`; it fails on that commit because the generator seam
  still takes a `FrontierAction` and returns `{}` with no `ArgumentRequest` recorded. The
  implementation commit makes it pass.

## Domain relations

- `loom.run.ArgumentRequest -> loom.run.Selection`, references one; a request serves one selection
  and does not own it — inferable from `ess/domains/run.yaml`, entity `loom.run.ArgumentRequest`,
  relation `selection` (references, one, via `selection_id`).

## Depends on, outside this store

`commission:story:agent-executor-port` (M-004) for `ProposedActionArguments`;
`commission:story:ess-hard-gate` (the frontier's `FrontierAction`s), through
`story:frontier-projection`.

## Acceptance

The test `arguments_for_selected_action_only` in `crates/loom/tests/argument_generator.rs` passes.
With a three-entry catalogue and a selector that picks the second entry, it checks:

1. The fake argument generator was handed exactly one catalogue entry, the selected one.
2. None of the other two catalogue entries' action ids appears in what the generator was handed.
3. The JSON value the generator returned equals the `ProposedActionArguments` of the returned
   `ProposedAction`.
4. One synthesized `loom.run.ArgumentRequest` was recorded, and its `selection_id` is the id of
   that selection.

## Source

TASKBOARD L-006; Atlas ADR 0073; `docs/contracts/loom-action-selection.md`;
`docs/examples/laya-fast-selection.md`.
