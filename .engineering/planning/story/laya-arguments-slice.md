---
format: aep.planning-md/3
id: story:laya-arguments-slice
kind: story
status: implemented
title: Laya picks the action, the reasoning model writes only its arguments
summary: TASKBOARD I-004 vertical slice through selection, fallback, argument generation and revalidation.
refs:
- provider: taskboard
  reference: I-004
relations:
- decomposes: epic:fast-selector
- depends_on: story:laya-selector
- depends_on: story:confidence-fallback
- depends_on: story:reasoning-model-selector
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-selector-laya
- confidence: inferred
  path: crates/loom-selector-laya/Cargo.toml
- confidence: inferred
  path: crates/loom-selector-laya/tests/laya_arguments_slice.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T19:23:53Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-10-09T19:23:53Z", actor: "human:timo", revision: 11}
- {from: "active", to: "implemented", at: "2026-10-09T20:04:09Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

The TASKBOARD I-004 vertical slice. Loom, run as Commission's `AgentExecutor` against a scripted
frontier, selects with the Laya selector behind the confidence fallback, whose stronger path is the
reasoning-model selector. It then asks the reasoning model for arguments to the selected action only,
and hands the proposed action to the execution boundary, which revalidates case revision, frontier and
authority before any effect (Atlas ADR 0073 § Decision; `docs/integrations/laya-fast-selection.md`
§ Flow).

The frontier is the example's: `metrics.inspect`, `logs.search`, `release.inspect`
(`docs/examples/laya-fast-selection.md`). The test supplies its own threshold. Three runs, differing
only in what the stub Laya endpoint answers:

1. an in-set action above the threshold: that action is proposed;
2. an in-set action below the threshold: the reasoning-model selector's choice is proposed;
3. `release.rollback`, not in the frontier, at any probability: it is rejected, the run falls back, and
   the reasoning-model selector's choice is proposed.

Not in this story, each because nothing has decided what it needs:

- schema validation of the generated arguments: `decision-blocker:action-argument-schema`;
- selection telemetry, and with it the epic's clause that unauthorized-action attempts at the
  execution boundary stay at 0 in that telemetry: `decision-blocker:selection-telemetry-record`.

So this story meets the epic acceptance except its telemetry clause.

## Domain relations

- `loom.run.Selection` → `loom.run.ActionCatalogue`, many-to-one, the selection references the catalogue and does not own it — inferable: `ess/domains/run.yaml`, entity `loom.run.Selection`, relation `catalogue`.
- `loom.run.ArgumentRequest` → `loom.run.Selection`, many-to-one, the request references the selection —
  inferable: `ess/domains/run.yaml`, entity `loom.run.ArgumentRequest`, relation `selection`.
- A selection names exactly one action of the candidate set it was given — inferable (inferred from `crates/loom/src/lib.rs:79-83`, the `frontier.contains_action` check in `Loom::run`; no ess/1 document declares this relation).

## Acceptance

With Commission's scripted fake governor serving the three-action frontier, a stub Laya endpoint and a
scripted reasoning model, each of the three runs calls the argument generator exactly once and only for
the action finally selected — the stub's choice above the supplied threshold, otherwise the reasoning
selector's — and `release.rollback` never reaches argument generation or the execution boundary.

## Dependencies

`story:laya-selector`, `story:confidence-fallback` and `story:reasoning-model-selector` in this epic.
From `epic:loom-native-harness`: revalidation before execution (TASKBOARD L-004) and the
`ArgumentGenerator` contract (L-006); no story for either existed when this was drafted.

## Source

TASKBOARD I-004; `epic:fast-selector` Acceptance; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md`;
`docs/examples/laya-fast-selection.md`.

## ESS first

No specification change: the story composes types `ess/domains/run.yaml` already declares
(`loom.run.Selection`, `loom.run.ArgumentRequest`, `SelectionStrategy`). Its red test is its own new
test, `crates/loom-selector-laya/tests/laya_arguments_slice.rs`, failing before the slice is wired.

Scoping notes (2026-10-09):

- The test lives in `crates/loom-selector-laya/tests/` with `b10x-loom-commission` and
  `b10x-loom-commission-testkit` as dev-dependencies. A test in `loom-executor` would need a
  dev-dependency on the Laya selector, which `no_product_crate_depends_on_the_selector`
  (`crates/loom-selector-laya/tests/laya_selector.rs`) refuses, because the CLI and the SDK reach
  `loom-executor`.
- The model-backed argument generator (`ModelArguments`, `harness/governed.rs`) is private; the
  test counts calls through its own scripted `ArgumentGenerator`, which is what the acceptance
  checks.
- The membership check the domain relations cite is now `selection::select`
  (`crates/loom-executor/src/selection.rs`); `crates/loom/src/lib.rs` no longer exists.
- `decision-blocker:selection-telemetry-record` is cleared (2026-10-04) and built by
  `story:selection-telemetry`; it is still out of this story.
