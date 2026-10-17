---
format: aep.planning-md/3
id: story:llm-routes-pin
kind: story
status: active
title: Loom links llm 0.6.0 and declares its models configuration in ESS
relations:
- decomposes: epic:model-routes
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: inferred
  path: crates/loom-executor/Cargo.toml
- confidence: inferred
  path: crates/loom-intake-router/Cargo.toml
- confidence: inferred
  path: crates/loom-intake-slice/Cargo.toml
- confidence: inferred
  path: crates/loom-plugin-slack/Cargo.toml
- confidence: inferred
  path: crates/loom-plugin/Cargo.toml
- confidence: inferred
  path: crates/loom-sdk/Cargo.toml
- confidence: inferred
  path: crates/loom-selector-laya/Cargo.toml
- confidence: inferred
  path: ess/domains/models.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: generated/rust/loom
- confidence: inferred
  path: website/data/ess
- confidence: inferred
  path: website/docs/reference/ess
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T00:43:24Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-17T00:43:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
---
## Outcome

Every Loom crate links llm 0.6.0, and the configuration that names a host's models is declared in
ESS: the domain `loom.models` (`ess/domains/models.yaml`) declares `loom.models.ModelsConfig`
(`routes`: the path of an `llm.routes/1` document, relative to the configuration file; `roles`: a
list of `loom.models.RoleModel`, each a `loom.models.ModelRole` — `agent` or `classifier` — and the
`model` that serves it, a route name or an `<endpoint>/<model-id>` name of that document).

## Acceptance

- Every `b10x-llm-*` dependency in every `Cargo.toml` names tag `0.6.0`, and `Cargo.lock` holds one
  version of each `b10x-llm-*` crate (`grep` over the manifests and `cargo tree -d` show no 0.4.0).
- `generated/rust/loom` holds the three declared types: `pub struct ModelsConfig` (fields `routes`,
  `roles`), `pub struct RoleModel` (fields `role`, `model`) and `pub enum ModelRole` with the
  variants for `agent` and `classifier` (`grep` over `generated/rust/loom/src`), and
  `ess specify compile --path ess --format json` lists `loom.models.ModelsConfig`.
- `ess specify validate --path ess --strict-requires` exits 0 with `ess/domains/models.yaml` listed
  in `ess/ess-inputs.yaml`; `generated/rust/loom` is regenerated and `task drift` passes.
- The package gates of every touched crate pass: `cargo clippy -p <crate> --all-targets -- -D
  warnings` and `cargo test -p <crate>`.
- `CHANGELOG.md` § Unreleased names the pin and says `Breaking: none.` for it (llm 0.5.0 and 0.6.0
  name no breaking change in the crates Loom uses).

## Scope

All `crates/*/Cargo.toml` that name `b10x-llm-*`, `Cargo.lock`, `consumers/pinned/Cargo.lock` if it
resolves llm, `ess/domains/models.yaml`, `ess/ess-inputs.yaml`, `generated/rust/loom/`,
`website/docs/reference/ess/` and `website/data/ess/` (regenerated reference pages), `CHANGELOG.md`. Only the call sites the bump forces to change.

## Not in scope

Any use of `llm_models::routes` (the next stories).

## ESS first

This story is the ESS change: `ess/domains/models.yaml` first, in its own commit, then the pin.
