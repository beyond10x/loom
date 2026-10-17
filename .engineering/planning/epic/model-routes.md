---
format: aep.planning-md/3
id: epic:model-routes
kind: epic
status: active
title: A host names its models by role, from an llm routes document
relations:
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T00:43:20Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-17T00:43:20Z", actor: "human:timo", revision: 3}
---
## Outcome

A host names its models the way it names any other role: one configuration file names an llm
`llm.routes/1` document and, per role, the route that serves it. `b10x-loom run --config loom.yaml`
runs on those routes, with credentials resolved from the routes document's `secret:` references
through llm's resolvers, and the SDK offers the same loader to an embedding host
(`loom_sdk::models`), returning `llm_core::Model`.

## Why

`run --catalog` takes an `llm.catalog/1` file, but the command line supplies no credential
resolver (`crates/loom-cli/src/model_catalog.rs:111`, `no_resolver`), so every route whose
account needs a credential is refused: a host can reach only anonymous endpoints, or Codex. llm
0.6.0 ships `llm.routes/1` (`llm_models::routes::Routes`) with `secret:` references
(`env:`, `file:`, `keychain:`) and `Routes::resolvers`, which closes that gap without Loom reading
a secret itself.

## The shape (decided)

```yaml
# loom.yaml
routes: routes.yaml            # an llm.routes/1 document, relative to this file
roles:
  - {role: agent, model: sonnet}
  - {role: classifier, model: or-haiku}
```

- `run --config loom.yaml`; `--model` and `--classifier-model` still override one role each.
- Library: `loom_sdk::models::from_config(path)` and `from_routes(path, name)`, returning
  `llm_core::Model`.
- Docs: one routes document showing a gateway (bearer or anonymous), Anthropic direct and
  OpenRouter direct.
- llm pin 0.4.0 -> 0.6.0.

## Stories

- `story:llm-routes-pin`: llm 0.6.0 and the `loom.models` ESS domain.
- `story:sdk-models-from-config`: the loader in the SDK.
- `story:cli-run-config`: `b10x-loom run --config`.
- `story:docs-model-routes`: the documented routes document and configuration.

## Not in scope

`plugin run` keeps `--catalog`; giving it `--config` is a later story. A credential Loom reads
itself; a route that falls back to a second target.
