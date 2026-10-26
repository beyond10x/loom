---
format: aep.planning-md/3
id: story:command-opaque-config-files
kind: story
status: implemented
title: A Command LoomAgent takes opaque config files instead of a mandatory loom.agent/1 spec.agent
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: charts/loom-agents/crds
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: deploy/crd
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: generated/rust/loom
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T15:49:10Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T15:49:10Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-26T20:55:52Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A `Command` `LoomAgent` takes its configuration as opaque files (`spec.configFiles`: a file name and
its text, written verbatim under `/config`) instead of a mandatory `loom.agent/1` `spec.agent`. The
controller walks each file for a credential and refuses one, as it walks `spec.agent` today, and
decodes nothing else. `spec.agent` stays required for an `AgentServe` resource.

## Why

Trial S5/S6, 2026-10-26: takt reads its own run configuration (`.autodev/endpoint.toml`, a
`run.toml`), not a `loom.agent/1` document, so the required `spec.agent` carried a placeholder for
the `Command` variant. Gap: "Command variant takes opaque config files instead of a mandatory
loom.agent/1 spec.agent". The takt side of the same contract is takt's ("One config contract with
loom: read /config/agent.yaml or a mounted run.toml").

## Acceptance

- ESS first: `LoomAgentSpec.agent` becomes optional for the `Command` variant, `configFiles` is
  declared, and admission refuses an `AgentServe` resource without `agent` and a file name that is
  not a plain file name; scenarios red first.
- `config_files_are_written_verbatim_under_config`.
- `a_credential_in_a_config_file_is_refused_without_repeating_it`.
- `an_agent_serve_resource_without_spec_agent_is_refused`.
- `deploy/crd/` regenerated (`agent` no longer required); `task crd-drift` exits 0.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `ess/domains/resource.yaml` (`LoomAgentSpec`, `AgentVariant`) — cited
- `crates/loom-operator/src/render.rs` (the `/config` ConfigMap), `admission.rs` — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` — cited
- AGENTS.md § Resource, design § 9a (the `/config` contract), CHANGELOG.md — cited
