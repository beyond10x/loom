---
format: aep.planning-md/3
id: story:laya-selector
kind: story
status: implemented
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
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: inferred
  path: crates/loom-selector-laya
- confidence: cited
  path: crates/loom-selector-laya/Cargo.toml
- confidence: cited
  path: crates/loom-selector-laya/src/lib.rs
- confidence: cited
  path: crates/loom-selector-laya/tests/laya_selector.rs
- confidence: cited
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: docs/integrations/laya-fast-selection.md
- confidence: cited
  path: website/docs/reference/crates.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T23:16:40Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-08T23:16:40Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-09T16:30:02Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1}}}
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
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom-executor/src/selection.rs:115-128`, `chosen`, which refuses an action the catalogue does not list; no ess/1 document declares this relation).

## Transport

`decision-blocker:laya-transport`, option A: the Laya request and response encoding lives in
`crates/loom-selector-laya` (package `b10x-loom-selector-laya`) and sends through llm's
`b10x-llm-http` `HttpClient::post_json` on a runtime the selector owns; no new third-party crate.
A transport failure, a timeout, a non-2xx status, malformed JSON, nesting past 128 levels or a
probability outside [0, 1] is `SelectorError::Unavailable`.

## Acceptance

Against a local stub that speaks the Laya interface, the selector returns the stub's chosen candidate
with the stub's probability as its confidence, returns a selection error when the stub names an action
outside the candidate set, and neither `cargo tree -p b10x-loom-cli` nor `cargo tree -p
b10x-loom-sdk` lists `b10x-loom-selector-laya`.

## ESS first

No specification change: `loom.run.SelectionStrategy::FastTyped` and `Selection.confidence`
(`ess/domains/run.yaml:46`, `:237`) are declared. The red test is the story's own,
`crates/loom-selector-laya/tests/laya_selector.rs`. The endpoint is adapter configuration, not a
domain noun.

## New-crate obligations

`crates/loom-executor/tests/crate_names.rs` (`EXPECTED` gains the entry), a Cargo `description`
naming no story id, `website/docs/reference/crates.md` regenerated with `task docs-generate`, and
`README.md`, `AGENTS.md` and `CHANGELOG.md` (**Unreleased**) in the same commit (`AGENTS.md`
§ Generated files).

## Not in this story

A run against hosted Laya. It needs an account and a credential, and its value is a measurement, which
belongs to Metaharness.

## Dependencies

The `ActionSelector` contract (`crates/loom-executor/src/selection.rs:50`, `story:action-selector`,
implemented). The implementor reads the published interface at https://laya.tools/laya-for-agents
before writing the stub: request and response schemas, path and method, error signalling.

## Source

TASKBOARD L-008; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md`;
`docs/examples/laya-fast-selection.md`.
