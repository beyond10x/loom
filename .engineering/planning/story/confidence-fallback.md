---
format: aep.planning-md/3
id: story:confidence-fallback
kind: story
status: draft
title: Low-confidence fast selections fall back to the stronger selector
summary: Hybrid selector gated by a caller-supplied threshold; no built-in default.
refs:
- provider: taskboard
  reference: L-009
relations:
- decomposes: epic:fast-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: inferred
  path: crates/loom-executor/tests
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: inferred
  path: docs/design/loom-design.md
- confidence: inferred
  path: docs/integrations/laya-fast-selection.md
- confidence: inferred
  path: ess/domains/run.yaml
- confidence: inferred
  path: generated/rust/loom/src/primitives.rs
revision: 10
---
## Outcome

A `Hybrid` selector (`loom.run.SelectionStrategy`) composes a fast selector and a stronger selector. It
returns the fast selection only when the fast selector named an action from the candidate set with a
confidence at or above a threshold the caller supplies; otherwise it returns the stronger selector's
answer (Atlas ADR 0073 § Fallback; Atlas `docs/design/governed-autonomy/invariants.md` § 7 and § 8).

It falls back when:

- the confidence is below the threshold;
- the fast selector reported no confidence (fail toward more explicit uncertainty, invariants § 7);
- the fast selector errs, including naming an action outside the candidates; that selection is rejected
  and never returned.

The threshold is an input, not a constant, and Loom ships no default. The 0.90 and 0.60 in
`docs/integrations/laya-fast-selection.md` are an example; thresholds are calibrated per domain and
measured by Metaharness. What a threshold is calibrated per, and who owns the value, is open:
`decision-blocker:selector-threshold-scope`. How a fallback is recorded is open too:
`decision-blocker:fallback-selection-record`. Neither answer changes what this story returns.

A confidence at any level never skips revalidation and never admits an action the candidates did not
contain (ADR 0073 § Security consequence).

## Domain relations

- `loom.run.Selection` → `loom.run.ActionCatalogue`, many-to-one, the selection references the catalogue and does not own it — inferable: `ess/domains/run.yaml`, entity `loom.run.Selection`, relation `catalogue`.
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom/src/lib.rs:79-83`, the `frontier.contains_action` check in `Loom::run`; no ess/1 document declares this relation).

## Acceptance

With scripted fast and stronger selectors, run once for each of two different supplied thresholds, the
hybrid selector returns the fast choice only when its confidence is at or above that threshold, and the
stronger choice when the fast confidence is below it, missing, or the fast selector errs or names an
action outside the candidates.

## Not in this story

The lowest band of the example policy (full planner, clarification or no-op below a second threshold).
ADR 0073 settles only "uncertain → reasoning-model selector/planner"; no source chooses among those
three.

## Dependencies

The `ActionSelector` contract (TASKBOARD L-005, `epic:loom-native-harness`); no story for it existed
when this was drafted. Both composed selectors are scripted, so this story does not wait for
`story:reasoning-model-selector` or `story:laya-selector`.

## Source

TASKBOARD L-009; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md` § Confidence policy;
`docs/contracts/loom-action-selection.md` safety rules 3 and 5.

## Carried from story:action-selector (adversary pass 1, 2026-10-04)

`Selection.confidence` is copied from the selector unvalidated (`crates/loom/src/selection.rs`). This
story is the first whose selector returns a confidence, so it validates the value at the seam: a
decimal in [0, 1], refused otherwise.
