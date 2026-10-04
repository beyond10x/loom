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
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-loop-port
scope:
- confidence: inferred
  path: crates/loom/src/compaction.rs
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/tests/compaction_contract.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 5
---
## Outcome

A Loom compaction contract. When a session crosses its trigger (Harness: 80 % of a declared context
window, freeing to 50 %, otherwise the byte rule `MAX_CONVERSATION_BYTES` / `COMPACTED_TARGET_BYTES`,
`harness-loop/src/lib.rs:885-937` at `798325f0`), the transcript prefix is compacted and the
compaction is priced and recorded on the session. Compaction never carries a stale catalogue
forward: the first request after it carries the catalogue projected from the frontier current at
that request, and nothing the model wrote is promoted to trusted context.

## Shared surface

Behavioural edges: `story:session-transcript-streaming` (the compaction record lives on the filed
session), `story:frontier-projection` (the first request after compaction carries a fresh
projection) and `story:harness-loop-port` (acceptance item 2 reads the tool list of the wired loop's
requests). Depends on `story:run-pipeline-skeleton` for the `compaction` module.

It still edits `loom.run.Session` in `ess/domains/run.yaml` and regenerates `generated/rust/loom/`
itself (the compaction record and usage shapes are not settled for the skeleton), and so does
`story:interruption-recovery`. The two have no behavioural edge between them; the former
ordering-only edge from `story:interruption-recovery` to this story was dropped on 2026-10-04, and
the shared `ess/` and `generated/` paths keep them in separate waves. The whole order is in
`story:agent-executor` § Shared surface.

## ESS first

- **First commit:** add the compaction command and its outcome on `loom.run.Session` in
  `ess/domains/run.yaml`, the outcome carrying the compaction's usage; `ess specify validate --path
  ess` passes; nothing else changes.
- **Red on it:** `task drift` fails, naming the first file of `generated/rust/loom/` that differs
  from the model regenerated from the changed specification.
- **Then:** `task generate`; the test `compaction_contract`; the implementation that makes it pass.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns): compaction rewrites turns the session
  owns — inferable from `ess/domains/run.yaml`, entity `loom.run.Session`.

## Scope

- `crates/loom/src/compaction.rs` (created empty by `story:run-pipeline-skeleton`, filled here),
  `crates/loom/src/lib.rs` (where compaction hooks into the run)
- `crates/loom/tests/compaction_contract.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (its own `Session` declarations)

## Acceptance

The test `compaction_contract` in `crates/loom/tests/compaction_contract.rs` passes. Over a
provider-emulated endpoint with a declared context window, with the Commission fake governor, it
runs a session past the trigger and checks:

1. After compaction the session is at or below 50 % of the declared window.
2. Between the last request before compaction and the first after it, the fake governor moves the
   frontier to a new case revision whose admissible set differs. The tool list of the first request
   after compaction equals the catalogue projected from the new frontier, and differs from the tool
   list of the last request before it.
3. The filed session holds exactly one compaction record, and its usage equals the usage the
   endpoint reported for the compaction request.
4. The instruction text of the first request after compaction equals that of the last request
   before it, byte for byte; the compacted summary appears only as model-authored content.

## Source

TASKBOARD L-012; Atlas ADR 0071 and 0072; Harness `harness-loop/src/lib.rs:885-937` at `798325f0`.
