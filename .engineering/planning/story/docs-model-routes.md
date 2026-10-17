---
format: aep.planning-md/3
id: story:docs-model-routes
kind: story
status: active
title: The site documents one routes document and the configuration that names Loom's models
relations:
- decomposes: epic:model-routes
- depends_on: story:cli-run-config
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-docs
- confidence: inferred
  path: website/docs/guides/model-routes.md
- confidence: inferred
  path: website/sidebars.js
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T00:43:25Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-17T00:43:25Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
---
## Outcome

The site documents how to name Loom's models: one page, `website/docs/guides/model-routes.md`,
with one `llm.routes/1` document declaring a gateway (bearer, and the anonymous variant),
Anthropic direct (api-key, `env:ANTHROPIC_API_KEY`) and OpenRouter direct (bearer,
chat-completions, `env:OPENROUTER_API_KEY`), the `loom.yaml` beside it, the
`b10x-loom run --config loom.yaml` line, the role overrides, and the SDK calls
`loom_sdk::models::from_config` and `from_routes`. It links llm's guide for the full format.

## Acceptance

- The page exists, is in the sidebar, and `task docs-check` and the site build pass.
- `crates/loom-docs` holds a test, `the_documented_model_routes_parse`, that reads the page and
  - parses its routes document with `llm_models::routes::Routes::parse` and its `loom.yaml` with the
    parser `loom_sdk::models` uses, so the documented files cannot drift from what Loom accepts;
  - asserts the routes document declares an endpoint with `auth.kind: bearer` and one with
    `anonymous` (the gateway), one at `https://api.anthropic.com` and one at
    `https://openrouter.ai`;
  - asserts the page contains `b10x-loom run --config loom.yaml`, `--model`, `--classifier-model`,
    `loom_sdk::models::from_config` and `loom_sdk::models::from_routes`.

## Scope

`website/docs/guides/model-routes.md` (new), `website/sidebars.*`, `crates/loom-docs/` (one test;
dev-dependencies on `b10x-llm-models` 0.6.0 and `b10x-loom-sdk` by path), `Cargo.lock`.

## Ready

After `story:cli-run-config`: the page documents its flag and the SDK calls of
`story:sdk-models-from-config`, and `task docs-check` regenerates `website/docs/reference/cli.md`
from the arguments that story changes.

## ESS first

No new noun; the page documents `loom.models.ModelsConfig`.
