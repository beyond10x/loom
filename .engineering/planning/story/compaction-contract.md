---
format: aep.planning-md/3
id: story:compaction-contract
kind: story
status: draft
title: Define the compaction contract
refs:
- provider: taskboard
  reference: L-012
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:session-transcript-streaming
- depends_on: story:frontier-projection
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A Loom compaction contract. When a session crosses its trigger (Harness: 80 % of a declared context
window, freeing to 50 %, otherwise the byte rule `MAX_CONVERSATION_BYTES` / `COMPACTED_TARGET_BYTES`,
`harness-loop/src/lib.rs:885-937` at `798325f0`), the transcript prefix is compacted and the
compaction is priced and recorded. Compaction never carries a stale catalogue forward: the first
request after it carries the catalogue projected from the current frontier, and nothing the model
wrote is promoted to trusted context.

## ESS first

Add the compaction command and its outcome on `loom.run.Session`; validate with
`ess specify validate --path ess`; regenerate the synthesized model.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns): compaction rewrites turns the session
  owns — inferable from `ess/domains/run.yaml`, entity `loom.run.Session`.

## Acceptance

A `b10x-loom` test runs a session past the declared context-window trigger and shows it compacted to
at or below the free target, with the tool list of the first request after compaction equal to the
catalogue projected from the current frontier.

## Source

TASKBOARD L-012; Atlas ADR 0071 and 0072; Harness `harness-loop/src/lib.rs:885-937` at `798325f0`.
