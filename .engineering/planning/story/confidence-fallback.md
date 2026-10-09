---
format: aep.planning-md/3
id: story:confidence-fallback
kind: story
status: implemented
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
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w1_20261009_confidence_fallback.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w1p2_20261009_confidence_fallback.rs
- confidence: cited
  path: crates/loom-executor/tests/confidence_fallback.rs
- confidence: cited
  path: docs/integrations/laya-fast-selection.md
revision: 18
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T23:16:39Z", actor: "human:timo", revision: 14}
- {from: "proposed", to: "active", at: "2026-10-08T23:16:40Z", actor: "human:timo", revision: 15}
- {from: "active", to: "implemented", at: "2026-10-09T16:30:02Z", actor: "human:timo", revision: 18, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
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

The threshold is an input, not a constant, and Loom ships no default. It is calibrated per
protocol (`decision-blocker:selector-threshold-scope`, option A): the embedding host supplies the
value for the run's protocol when it builds the hybrid selector. Metaharness calibrates values
offline and the host loads its output; Loom does not read Metaharness. The 0.90 and 0.60 in
`docs/integrations/laya-fast-selection.md` are an example only.

A fallback is recorded as two linked selections (`decision-blocker:fallback-selection-record`,
option B). That recording, and the specification change it needs, is
`story:fallback-selection-recording`; this story returns the right choice and records nothing new.

The threshold and the confidence are compared as numbers: `Decimal` is a string newtype whose
derived order is string order (`generated/rust/loom/src/primitives.rs:17-21`), so `0.9` and `0.90`
must compare equal.

A confidence at any level never skips revalidation and never admits an action the candidates did not
contain (ADR 0073 § Security consequence).

## Domain relations

- `loom.run.Selection` → `loom.run.ActionCatalogue`, many-to-one, the selection references the catalogue and does not own it — inferable: `ess/domains/run.yaml`, entity `loom.run.Selection`, relation `catalogue`.
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom-executor/src/selection.rs:115-128`, `chosen`, which refuses an action the catalogue does not list; no ess/1 document declares this relation).

## Acceptance

With scripted fast and stronger selectors, run once for each of two different supplied thresholds, the
hybrid selector returns the fast choice only when its confidence is at or above that threshold, and the
stronger choice when the fast confidence is below it, missing, or the fast selector errs or names an
action outside the candidates. A confidence outside [0, 1], or one that is not a decimal, counts as
missing; a confidence equal to the threshold but written differently (`0.9` and `0.90`) counts as
at the threshold.

## ESS first

No specification change: `loom.run.SelectionStrategy::Hybrid` is declared (`ess/domains/run.yaml:46`).
The red test is the story's own, `crates/loom-executor/tests/confidence_fallback.rs`.

## Not in this story

The lowest band of the example policy (full planner, clarification or no-op below a second threshold).
ADR 0073 settles only "uncertain → reasoning-model selector/planner"; no source chooses among those
three.

## Dependencies

The `ActionSelector` contract (`crates/loom-executor/src/selection.rs:50`, `story:action-selector`,
implemented). Both composed selectors are scripted, so this story does not wait for
`story:reasoning-model-selector` or `story:laya-selector`.

## Source

TASKBOARD L-009; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md` § Confidence policy;
`docs/contracts/loom-action-selection.md` safety rules 3 and 5.

## Carried from story:action-selector (adversary pass 1, 2026-10-04)

`Selection.confidence` is copied from the selector unvalidated (`crates/loom-executor/src/selection.rs:135`). This
story is the first whose selector returns a confidence, so it validates the value at the seam: a
decimal in [0, 1], refused otherwise.

## Delivered

## Delivered

- The carried item above says a confidence outside a decimal in [0, 1] is "refused". As built,
  `select` drops such a confidence and records the selection without one, so it counts as missing
  (the acceptance's rule) and the hybrid falls back; the selection itself is not refused
  (`crates/loom-executor/src/selection.rs`, `Confidence::parse` and `select`).
- `select_action` still copies the confidence unchecked, as `ess/domains/run.yaml` specifies it.
- Adversary pass 1 found leading-zero confidences accepted (fixed 6ca6a2b); pass 2 found nothing.
