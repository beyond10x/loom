---
format: aep.planning-md/3
id: story:cli-catalog-model-route
kind: story
status: draft
title: b10x-loom --model and --classifier-model take an llm catalog route
relations:
- serves: vision:O3
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: inferred
  path: crates/loom-cli/src/main.rs
revision: 3
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
