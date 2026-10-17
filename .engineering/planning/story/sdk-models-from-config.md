---
format: aep.planning-md/3
id: story:sdk-models-from-config
kind: story
status: implemented
title: The SDK loads a host's models from a configuration or a routes document
relations:
- decomposes: epic:model-routes
- depends_on: story:llm-routes-pin
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-sdk/Cargo.toml
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: inferred
  path: crates/loom-sdk/src/models.rs
- confidence: inferred
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: inferred
  path: crates/loom-sdk/tests/models.rs
- confidence: inferred
  path: generated/rust/loom
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T00:43:24Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-17T00:43:25Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-10-17T04:33:36Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

An embedding host loads its models from a configuration file through the SDK alone:
`loom_sdk::models::from_config(path)` reads a `loom.models.ModelsConfig` (YAML) and returns the
model of each declared role; `loom_sdk::models::from_routes(path, name)` returns the model one
name of an `llm.routes/1` document serves. Both return `llm_core::Model` built by
`llm_models::routes::Routes::port`, with credentials resolved by `Routes::resolvers` from the
document's `secret:` references (`env:`, and `file:` with `~/` against `HOME`); Loom never reads
a secret itself and nothing is read or sent before the first turn.

## Acceptance

In `crates/loom-sdk/tests/models.rs`:
- `a_config_names_the_model_of_each_role`: a config with `agent` and `classifier` returns two
  models whose identity is the route each role names.
- `a_routes_name_resolves_to_a_model`: `from_routes(path, "sonnet")` returns the route's model;
  an `<endpoint>/<model-id>` name of a declared endpoint does too.
- `a_refusal_names_the_file_and_the_name`: an undeclared role, a role listed twice, an unknown
  field, a missing or invalid routes document and a name the document does not declare are each
  refused with an error naming the file, and the role or name.
- `a_missing_secret_is_refused_at_the_first_turn_and_never_read_early`: a route whose `env:`
  variable is unset builds, and its first turn is refused `unauthorized`; no network call is made.
- `crates/loom-sdk/Cargo.toml` depends on `b10x-llm-models` 0.6.0 with its `environment` and `file`
  features, and `Cargo.lock` gains no second version of any `b10x-llm-*` crate.
- `loom_sdk::models::parse_config(text)` returns the generated `ModelsConfig` (re-exported as
  `loom_sdk::models::ModelsConfig`), the parser `from_config` uses; `crates/loom-sdk/Cargo.toml`
  depends on the generated `loom` crate (`generated/rust/loom`) by path for it.
- The routes path is relative to the configuration file's directory.
- The SDK stability page lists `loom_sdk::models::{from_config, from_routes, parse_config, ModelsConfig}` and
  `crates/loom-sdk/tests/host_ports.rs` resolves them.

## Scope

`crates/loom-sdk/src/models.rs` (new), `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/Cargo.toml`, `Cargo.lock`,
`crates/loom-sdk/tests/models.rs`, `crates/loom-sdk/tests/host_ports.rs`,
`website/docs/reference/sdk-stability.md`, `CHANGELOG.md`.

## Ready

After `story:llm-routes-pin` (the `loom.models` types, generated, and llm 0.6.0).

## ESS first

The configuration is `loom.models.ModelsConfig`, declared by `story:llm-routes-pin`; this story
hand-writes no type that shadows it (`task no-hand-model`).
