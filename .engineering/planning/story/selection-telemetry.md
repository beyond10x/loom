---
format: aep.planning-md/3
id: story:selection-telemetry
kind: story
status: active
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
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 12}
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
