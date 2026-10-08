---
format: aep.planning-md/3
id: story:cli-catalog-model-route
kind: story
status: implemented
title: b10x-loom --model and --classifier-model take an llm catalog route
relations:
- serves: vision:O3
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
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/src/model_catalog.rs
- confidence: cited
  path: crates/loom-cli/src/regular_file.rs
- confidence: cited
  path: crates/loom-cli/tests/catalog_route.rs
- confidence: cited
  path: crates/loom-cli/tests/catalog_route_adversary.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/catalog-route
- confidence: cited
  path: crates/loom-executor/Cargo.toml
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/guides/run-an-intent.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 24
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T09:20:19Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-08T09:20:19Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-08T09:48:06Z", actor: "human:timo", revision: 24, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

`b10x-loom run` takes an llm catalog file and a route alias for `--model` and
`--classifier-model`, not only a Codex model name. Today both flags go through `codex_model`
(`crates/loom-cli/src/main.rs`, the classifier at the intent preparation and the agent before
`run_prepared_intent`), so a model served through an llm catalog route (for example a
self-hosted endpoint) cannot drive a Loom run. llm's qualification of a self-hosted model used
from Loom waits on this.

## Acceptance

With a catalog file and a route alias, `b10x-loom run` builds the classifier and the agent model
through llm's catalog-to-`Model` function; a plain Codex model name still works as before; an
unknown alias or an unreadable catalog stops before any model call with an error naming the alias
or the file. Tests use recorded responses and a fixture catalog; no network call.

## ESS first

Command-line configuration only. If the run request in `ess/` names the model, the catalog route is
declared there first; otherwise record why no ESS change is needed before implementing.

## Upstream

Waits on llm's catalog-to-`Model` function (`upstream-blocker:llm-catalog-model-port`); depend on
the llm release that ships it.
