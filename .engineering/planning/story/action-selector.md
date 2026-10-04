---
format: aep.planning-md/3
id: story:action-selector
kind: story
status: draft
title: Define ActionSelector over the projected catalogue
refs:
- provider: taskboard
  reference: L-005
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:frontier-projection
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

The `ActionSelector` contract of `docs/contracts/loom-action-selection.md`: a selector receives the
selection context and the projected catalogue candidates only, returns one action id and an optional
confidence, and cannot add capability, execute or decide authority (Atlas ADR 0073). Loom refuses any
returned id that is not in the candidate set it handed over, whatever the confidence. The bootstrap
`FirstAdmissibleSelector` stays as the deterministic test selector. Reasoning-model, Laya, fallback
and hierarchical selectors are `epic:fast-selector` (L-007 to L-010).

## ESS first

- Close the marker on `loom.run.Selection` (`ess/domains/run.yaml:99`, "its ESS scalar type is
  decided in story L-005"): declare `confidence` as `Optional<Binary64>`, inferred from
  `crates/loom/src/lib.rs:11` (`Option<f32>`) and the contract sketch.
- Add the select command with its accepted outcome and its refusal for an action id absent from the
  catalogue; validate with `ess specify validate --path ess`; regenerate the synthesized model.

## Domain relations

- `loom.run.Selection -> loom.run.ActionCatalogue`, many-to-one; a selection references the
  catalogue it chose from and does not own it — inferable from `ess/domains/run.yaml`, entity
  `loom.run.Selection`, relation `catalogue` (references, one, via `catalogue_id`).

## Acceptance

A `b10x-loom` test hands a selector the projected catalogue and shows that a returned action id
absent from it is refused, naming the id, before any argument generation and even at confidence 1.0,
using the synthesized `loom.run.Selection` whose `confidence` is `Optional<Binary64>`.

## Source

TASKBOARD L-005; Atlas ADR 0073; `docs/contracts/loom-action-selection.md` safety rules 1 to 3.
