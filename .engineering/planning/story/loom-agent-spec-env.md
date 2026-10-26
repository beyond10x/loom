---
format: aep.planning-md/3
id: story:loom-agent-spec-env
kind: story
status: draft
title: A LoomAgent takes plain environment values in spec.env, credential-walked
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
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
revision: 2
---
## Outcome

A `LoomAgent` takes plain environment values in `spec.env` (`name`, `value`), rendered on the
container beside the `secretRef` environment. A value that holds a credential is refused by the
same credential walk as `spec.agent` and `spec.command`; a name that collides with one the
controller sets is refused.

## Why

Trial S5/S6, 2026-10-26: run 7 needed `CARGO_HOME`, `CARGO_TARGET_DIR`, `CARGO_NET_OFFLINE` and
`B10X_GATES_POLICY`, set by a wrapper script because the resource has no plain environment. Gap:
"spec.env for plain environment values on a LoomAgent".

## Acceptance

- ESS first: `loom.resource.LoomAgentSpec` gains `env: List<EnvVar>` and admission refusals
  (credential, reserved name, not a variable name); scenarios red first.
- `spec_env_values_reach_the_container`.
- `a_credential_in_spec_env_is_refused_without_repeating_it`.
- `a_reserved_variable_name_is_refused`.
- `deploy/crd/` regenerated; `task crd-drift` exits 0.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `ess/domains/resource.yaml` (`LoomAgentSpec`) — cited
- `crates/loom-operator/src/render.rs`, `admission.rs` (credential walk) — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` — cited
- AGENTS.md § Resource, README.md (Kubernetes section), CHANGELOG.md — cited
