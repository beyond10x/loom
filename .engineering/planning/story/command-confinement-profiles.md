---
format: aep.planning-md/3
id: story:command-confinement-profiles
kind: story
status: draft
title: A Command LoomAgent declares seccomp and AppArmor profiles, allow-listed by the operator configuration
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: charts/loom-agents
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/config.rs
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

A `Command` `LoomAgent` declares its container's seccomp and AppArmor profiles (`RuntimeDefault`,
`Localhost` with a profile name, or `Unconfined`), and the controller renders them; an operator
configuration allow-lists which profiles a resource may ask for, so `Unconfined` is granted by the
cluster's operator, not by the resource alone.

## Why

Trial S5/S6, 2026-10-26: the program inside the pod needs `unshare` (Substrate-style confinement of
its own children), which the default seccomp and AppArmor profiles refuse. The trial patched the
Deployment (`kubectl patch ... seccomp + AppArmor Unconfined`, field manager `autodev-trial`) and its
wrapper waited for `Seccomp: 0`. Gap: "Container seccomp and AppArmor profile fields on the Command
variant (unshare needs Unconfined)".

## Acceptance

- ESS first: `loom.resource` declares the profile fields and an admission refusal for a profile the
  operator configuration does not allow; scenarios red first.
- `a_declared_profile_is_rendered_on_the_container`.
- `an_unconfined_profile_not_allowed_by_the_operator_is_refused`.
- An `AgentServe` resource keeps `RuntimeDefault`.
- `deploy/crd/` regenerated; `task crd-drift` exits 0.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `crates/loom-operator/src/render.rs`, `admission.rs`, `config.rs` — cited
- `ess/domains/resource.yaml` — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/` (values for the allow list) — cited
- AGENTS.md § Resource, design § 7 (Security) and § 9a, CHANGELOG.md — cited
