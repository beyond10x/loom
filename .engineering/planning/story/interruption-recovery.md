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
- depends_on: story:compaction-contract
scope:
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/src/recovery.rs
- confidence: inferred
  path: crates/loom/tests/interruption_recovery.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 4
---
## Outcome

A Loom run can be cancelled at any point and recovered from its filed session, and a run suspended
at an approval resumes from its checkpoint before the exact effect (the Harness approval checkpoint,
`harness-loop/src/approval.rs`, and `LoopStop::Cancelled`, at `798325f0`). Recovery trusts nothing
that was in flight: it re-projects from the frontier current at resume and revalidates
(`story:selection-revalidation`) before returning any `ProposedAction`, so a selection made before
the interruption cannot be proposed against a moved case.

## Shared surface

Link 9 of the `epic:loom-native-harness` chain over `ess/domains/run.yaml`, `generated/rust/loom/`
and `crates/loom/src/lib.rs`. It depends on `story:compaction-contract`, and
`story:loom-ess-conformance` depends on it. Both this story and the two before it edit
`loom.run.Session`. The whole order is in `story:agent-executor` § Shared surface. It also depends on
`story:session-transcript-streaming` and `story:selection-revalidation` directly.

## ESS first

Add the interrupt and resume commands and their outcomes on `loom.run.Session`; validate with
`ess specify validate --path ess`; regenerate with `task generate`.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns) and `loom.run.Selection ->
  loom.run.ActionCatalogue` (relation `catalogue`) for the revision a pre-interruption selection was
  made at — inferable from `ess/domains/run.yaml`.

## Depends on, outside this store

`commission:story:run-outcomes` (M-007) for the Run's suspend and resume, and
`commission:story:authority-provider-port` (M-005) for the fake that grants the approval.

## Scope

- `crates/loom/src/recovery.rs` (new), `crates/loom/src/lib.rs`
- `crates/loom/tests/interruption_recovery.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface)

## Acceptance

The test `interruption_recovery` in `crates/loom/tests/interruption_recovery.rs` passes. With the
Commission fakes, it checks:

1. A run cancelled after a selection, and resumed by session id after the fake governor advanced
   the case revision, projects its first catalogue at the new revision.
2. That resumed run returns no `ProposedAction` for the pre-interruption selection; the
   revalidation refusal names the stale revision.
3. A run suspended at the merge approval, resumed by session id after the fakes grant the approval
   with the frontier otherwise unchanged, returns a `ProposedAction` for `repository.merge`, and the
   selector is not called again between the suspension and that return.
4. A run suspended at the merge approval, resumed after the fake governor advanced the case
   revision, re-projects at the new revision before returning anything.

## Source

TASKBOARD L-013; Atlas ADR 0071 and 0072; Harness `harness-loop/src/approval.rs` at `798325f0`.
