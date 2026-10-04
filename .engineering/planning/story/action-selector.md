---
format: aep.planning-md/3
id: story:action-selector
kind: story
status: draft
title: Define ActionSelector over the projected catalogue
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-005
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:frontier-projection
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/src/selection.rs
- confidence: inferred
  path: crates/loom/tests/action_selector.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 7
---
## Outcome

The `ActionSelector` contract of `docs/contracts/loom-action-selection.md`: a selector receives the
selection context and the projected catalogue candidates only, returns one action id and an optional
confidence, and cannot add capability, execute or decide authority (Atlas ADR 0073). Loom refuses any
returned id that is not in the candidate set it handed over, whatever the confidence. The bootstrap
`FirstAdmissibleSelector` stays as the deterministic test selector, rewritten over the projected
catalogue instead of Canon's bootstrap `Frontier` (deleted by Canon's wave 1). Reasoning-model,
Laya, fallback and hierarchical selectors are `epic:fast-selector` (L-007 to L-010).

## Shared surface

Link 3 of the `epic:loom-native-harness` chain over `ess/domains/run.yaml`, `generated/rust/loom/`
and `crates/loom/src/lib.rs`. It depends on `story:frontier-projection`, and
`story:argument-generator` depends on it. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

- `loom.run.Selection.confidence` is `Optional<Decimal>`, declared by `story:ess-hard-gate`
  (resolution (c)); this story does not retype it. Not `Binary64`: `ess verify conform synthesize`
  refuses it ("finite Binary64 is not admitted", probe 2026-10-04, ess 0.52.0). A selector's
  numeric confidence (`crates/loom/src/lib.rs:11`, today `Option<f32>`) is carried in the generated
  `primitives::Decimal`.
- Add the select command with its accepted outcome and its refusal for an action id absent from the
  catalogue; validate with `ess specify validate --path ess`; regenerate with `task generate`;
  `task ess-gate` stays green.

## Domain relations

- `loom.run.Selection -> loom.run.ActionCatalogue`, many-to-one; a selection references the
  catalogue it chose from and does not own it — inferable from `ess/domains/run.yaml`, entity
  `loom.run.Selection`, relation `catalogue` (references, one, via `catalogue_id`).
- The candidates are catalogue entries projected from Commission's `Frontier`
  (`story:frontier-projection`), not Canon `ActionCandidate` values.

## Depends on, outside this store

`commission:story:ess-hard-gate` (the frontier's `FrontierAction`s), through
`story:frontier-projection`.

## Scope

- `crates/loom/src/selection.rs` (new), `crates/loom/src/lib.rs`
- `crates/loom/tests/action_selector.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface)

## Acceptance

The test `selector_cannot_leave_catalogue` in `crates/loom/tests/action_selector.rs` passes. It
hands selectors the projected catalogue and checks:

1. A selector that returns an action id absent from the catalogue at confidence 1.0 is refused, and
   the refusal names that id.
2. After that refusal the fake argument generator has been called zero times.
3. `FirstAdmissibleSelector` returns the first admissible entry of the catalogue.
4. The returned selection is the synthesized `loom.run.Selection`, whose `confidence` is
   `Option<primitives::Decimal>` (`Optional<Decimal>`).

## Source

TASKBOARD L-005; Atlas ADR 0073; `docs/contracts/loom-action-selection.md` safety rules 1 to 3.
