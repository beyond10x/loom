---
format: aep.planning-md/3
id: story:interruption-recovery
kind: story
status: draft
title: Define interruption and recovery
refs:
- provider: taskboard
  reference: L-013
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:session-transcript-streaming
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A Loom run can be cancelled at any point and recovered from its filed session, and a run suspended
at an approval resumes from its checkpoint before the exact effect (the Harness approval checkpoint,
`harness-loop/src/approval.rs`, and `LoopStop::Cancelled`, at `798325f0`). Recovery trusts nothing
that was in flight: it re-projects from the frontier current at resume and revalidates before any
effect, so a selection made before the interruption cannot execute against a moved case.

## ESS first

Add the interrupt and resume commands and their outcomes on `loom.run.Session`; validate with
`ess specify validate --path ess`; regenerate the synthesized model.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns) and `loom.run.Selection ->
  loom.run.ActionCatalogue` (relation `catalogue`) for the revision a pre-interruption selection was
  made at — inferable from `ess/domains/run.yaml`.

## Acceptance

A `b10x-loom` test cancels a run after a selection and resumes it after the fake governor has
advanced the case revision, and shows the resumed run re-projects from the new frontier and never
hands the stale selection to the execution adapter.

## Source

TASKBOARD L-013; Atlas ADR 0071 and 0072; Harness `harness-loop/src/approval.rs` at `798325f0`.
