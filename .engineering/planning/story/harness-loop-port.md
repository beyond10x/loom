---
format: aep.planning-md/3
id: story:harness-loop-port
kind: story
status: draft
title: Wire the ported Harness loop to Loom's projection, selection and revalidation
summary: The ported loop's tool list is the projected catalogue and every tool call goes through selection, arguments and revalidation; the port itself is story:harness-crate-port.
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:harness-module-map
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-crate-port
scope:
- confidence: inferred
  path: crates/loom/src/harness/
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/tests/harness_loop_port.rs
revision: 7
---
## Outcome

The Harness loop ported by `story:harness-crate-port` calls Loom's own pieces at Harness's seams:
the catalogue from `story:frontier-projection` is what each request's tool list carries (through
the `TurnEnvironmentProvider` seam, `harness-loop/src/environment.rs:47` at `798325f0`), and a
model's tool call goes through selection, argument generation and revalidation
(`story:action-selector`, `story:argument-generator`, `story:selection-revalidation`) before it
becomes a `ProposedAction`.

On 2026-10-04 the porting half of this story (the five crates, the provider-wire fixtures, the
licence, Harness unchanged; former acceptance items 1, 5 and 6) moved to
`story:harness-crate-port`, so the port no longer waits for the selection chain. This story keeps
the wiring and its acceptance test, `ported_loop_round_trip`.

## Shared surface

Depends on `story:harness-crate-port` (the ported loop), `story:selection-revalidation` (the
pipeline a tool call goes through; it reaches `story:frontier-projection`,
`story:action-selector` and `story:argument-generator` through it), `story:run-pipeline-skeleton`
and `story:harness-module-map`. These are behavioural edges: acceptance items 1 to 3 below exercise
the projection, the selector, the generator and the refusal path. It edits the ported
`crates/loom/src/harness/` tree and the executor pipeline in `crates/loom/src/lib.rs`.
`story:compaction-contract` and `story:interruption-recovery` depend on it.

## ESS first

- **Specification change:** none — wiring adds no domain noun; it uses
  `loom.run.ProjectCatalogue`, `SelectAction`, `RequestArguments` and `RevalidateSelection`, declared
  by `story:run-pipeline-skeleton`. If wiring meets a domain noun, the story stops and reports it.
- **Red test:** the first commit adds `ported_loop_round_trip` in
  `crates/loom/tests/harness_loop_port.rs`; it fails on that commit because the ported loop's tool
  list is still the `ToolPort` attached at startup, not the projected catalogue (item 1), and a tool
  call does not reach the selector (item 2).

## Scope

- `crates/loom/src/harness/` (the `TurnEnvironmentProvider` and `ToolPort` implementations that
  feed the projected catalogue and route tool calls into the pipeline)
- `crates/loom/src/lib.rs` (the executor pipeline)
- `crates/loom/tests/harness_loop_port.rs` (new)

## Constraints

Read-only on `beyond10x/harness`.

## Acceptance

The test `ported_loop_round_trip` in `crates/loom/tests/harness_loop_port.rs` passes and checks,
over a provider-emulated endpoint, with the Commission fake governor serving the software-change
frontier:

1. The tool list of every request equals the catalogue projected from the frontier current when
   the request was assembled.
2. A scripted model reply that calls a catalogue action makes Loom return a `ProposedAction` for
   that action, and the model's tool call reached the selector and the argument generator exactly
   once each.
3. A scripted model reply that calls a name outside the catalogue is answered to the model as a
   refusal naming that name, and no `ProposedAction` is returned.

## Source

TASKBOARD L-002 (the executor carries Harness's loop); `epic:loom-native-harness` § Outcome; Atlas
ADR 0071 and 0072; `docs/design/harness-map.md` § Seams a port reuses; finding 2 of
`review-result:loom-native-harness-scope-r1`; the split of 2026-10-04 into
`story:harness-crate-port`.
