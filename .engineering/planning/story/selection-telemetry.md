---
format: aep.planning-md/3
id: story:selection-telemetry
kind: story
status: implemented
title: Record selection telemetry for Metaharness
refs:
- provider: taskboard
  reference: L-009
relations:
- decomposes: epic:fast-selector
- depends_on: story:confidence-fallback
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:fallback-selection-recording
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-executor/src/harness/governed.rs
- confidence: inferred
  path: crates/loom-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/src/selection.rs
- confidence: inferred
  path: crates/loom-executor/src/session.rs
- confidence: inferred
  path: crates/loom-executor/tests/selection_telemetry.rs
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/src/run.rs
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 12}
- {from: "active", to: "implemented", at: "2026-10-10T04:21:19Z", actor: "human:timo", revision: 14, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Loom records one `SelectionRecord` per selection and a per-session count of boundary refusals, as
decided on decision-blocker:selection-telemetry-record (2026-10-04). ESS first: `ess/domains/run.yaml`
gains `SelectionRecord` (owned by `Selection`, cardinality one; fields strategy, candidate_count,
chosen_action, confidence `Optional<Decimal>`, fell_back_to `Optional<SelectionStrategy>`,
latency_ms `Integer`, input_tokens `Integer`, output_tokens `Integer`) and `Session.boundary_refusals`
`Integer`; the model is regenerated, and the ESS hard gate (ADR 0076) stays green. The records are
written into Loom's session record for Metaharness; they are never evidence (ADR 0074).

## Acceptance

Named test `selection_telemetry_is_recorded`: (1) a run with three selections through the fake
selector writes three `SelectionRecord`s whose chosen actions, strategies and candidate counts equal
the selections; (2) a below-threshold selection records `fell_back_to: ReasoningModel`; (3) an
out-of-frontier proposal refused at the boundary increments `boundary_refusals` by one and adds no
`SelectionRecord`; (4) `ess_gate` passes.

## Source

Build pack `projects/loom/TASKS.md` lines 13-20 (Atlas `docs/design/governed-autonomy/projects/loom-TASKS.md`);
`docs/contracts/loom-action-selection.md` safety rule 7.

## Settled at implementation (2026-10-10)

- `fell_back_to` is on the record of the overruled fast selection and names the replacement's
  strategy; the replacement's record has none.
- A boundary refusal is a selection refused after it was made, by admission or by revalidation; a
  model call outside the catalogue makes no selection and is not counted. Only a run with a
  session (`run_loop`) counts; the plain `AgentExecutor` run has no session.
- In a governed loop the selection is the model's tool call: the first selection of a turn carries
  that turn's latency and token usage, later selections of the same turn record 0, so totals are
  not double-counted.
- Acceptance (3) means the refusal writes no further record; the refused selection keeps the record
  written when it was made.
- The records live on the `Loom` and in `Loom::sessions()`; the filed session file keeps its
  format, and a resume compares `SessionData` without `boundary_refusals`.
- ESS writes the count with `sets: {increment: 1}` on a self-transition; `confidence` is not
  published in the `SelectionRecords` view (a Decimal the suite cannot replay, ESS-SYNTH-001).
