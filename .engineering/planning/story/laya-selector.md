---
format: aep.planning-md/3
id: story:laya-selector
kind: story
status: draft
title: Laya selector experiment behind the ActionSelector seam
summary: FastTyped selector adapter for a local or hosted Laya endpoint, in its own crate.
refs:
- provider: taskboard
  reference: L-008
relations:
- decomposes: epic:fast-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-selector-laya
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: inferred
  path: docs/integrations/laya-fast-selection.md
revision: 5
---
## Outcome

A `FastTyped` `ActionSelector` (`loom.run.SelectionStrategy`) that sends a compact selection request —
the goal and the candidate action ids taken from the current frontier — to a Laya endpoint, and maps the
typed choice and its probability onto a `Selection` with that probability as its confidence. The
endpoint is configuration, so one adapter serves local and hosted inference
(`docs/integrations/laya-fast-selection.md` § Locality). The adapter lives in its own crate: Loom
builds and runs with no Laya code, and another typed decision model or a deterministic selector fits
the same seam (`AGENTS.md` § Rules: no semantic dependency on one selector vendor).

This is the experiment: it proves the adapter. It does not make Laya the default and does not measure
Laya against other selectors; that comparison is Metaharness (TASKBOARD I-005).

Rules this story holds:

- Candidate labels come only from the candidate set passed in; an answer naming any other id is a
  selection error (`docs/examples/laya-fast-selection.md`: `release.rollback` is rejected when it is not
  a candidate, whatever its probability).
- The probability is reported as confidence and grants nothing (ADR 0073 § Decision). Thresholding is
  not here; it is `story:confidence-fallback`.
- A transport failure, timeout or malformed answer is a selection error, so the fallback can fail
  toward stronger reasoning (Atlas `docs/design/governed-autonomy/invariants.md` § 7).
- Request and response follow the published Laya interface, read at implementation time
  (https://laya.tools/laya-for-agents). The JSON in `docs/examples/laya-fast-selection.md` is
  illustrative and is not that interface.

## Domain relations

- `loom.run.Selection` → `loom.run.ActionCatalogue`, many-to-one, the selection references the catalogue and does not own it — inferable: `ess/domains/run.yaml`, entity `loom.run.Selection`, relation `catalogue`.
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom/src/lib.rs:79-83`, the `frontier.contains_action` check in `Loom::run`; no ess/1 document declares this relation).

## Acceptance

Against a local stub that speaks the Laya interface, the selector returns the stub's chosen candidate
with the stub's probability as its confidence, returns a selection error when the stub names an action
outside the candidate set, and `cargo tree -p b10x-loom` lists no Laya adapter crate.

## Not in this story

A run against hosted Laya. It needs an account and a credential, and its value is a measurement, which
belongs to Metaharness.

## Dependencies

The `ActionSelector` contract (TASKBOARD L-005, `epic:loom-native-harness`); no story for it existed
when this was drafted.

## Source

TASKBOARD L-008; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md`;
`docs/examples/laya-fast-selection.md`.
