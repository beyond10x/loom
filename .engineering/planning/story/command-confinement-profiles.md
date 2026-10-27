---
format: aep.planning-md/3
id: story:command-confinement-profiles
kind: story
status: implemented
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
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T13:03:24Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-27T13:03:24Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-27T13:18:02Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
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

Scenarios, each a Rust test in `crates/loom-operator/tests/command_confinement_profiles.rs` (the
CRD shape in `crates/loom-resource/tests/crd.rs`):

- `a_declared_profile_is_rendered_on_the_container`: the program container, not the init
  containers, carries the declared seccomp and AppArmor profile.
- `an_unconfined_profile_not_allowed_by_the_operator_is_refused`: `ProfileNotAllowed`, with the
  configuration absent and with `allowUnconfinedProfiles: false`.
- `an_unconfined_profile_the_operator_allows_is_rendered`.
- `a_localhost_profile_outside_the_operators_list_is_refused`: the seccomp and AppArmor lists are
  separate.
- `a_profile_names_its_localhost_profile_exactly_when_it_is_localhost`: `ProfileInvalid` for a
  `Localhost` without a name, a name on another kind, an absolute, `..` or empty name, an unknown
  type.
- `an_agent_serve_resource_keeps_runtime_default`: a profile on an `AgentServe` resource is
  `Malformed`.
- `a_command_resource_without_a_profile_renders_as_before` and
  `the_configuration_without_the_new_members_denies_what_it_did_not_list`: additive, today's
  behaviour unchanged.
- `the_confinement_profiles_are_a_closed_type_with_an_optional_localhost_name` (CRD).
- `task drift` and `task crd-drift` exit 0; `task chart-check` renders the new values.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `crates/loom-operator/src/render.rs`, `admission.rs`, `config.rs` — cited
- `ess/domains/resource.yaml` — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/` (values for the allow list) — cited
- AGENTS.md § Resource, design § 7 (Security) and § 9a, CHANGELOG.md — cited

## Also reported (S7)

The orchestrator, 2026-10-26, from the takt one-hour pod trial (S7): "still: no seccomp or
AppArmor field (a probe under the default profile got EPERM on `unshare`; the trial needed a hand
patch to Unconfined)". Trial record: "probe pod under RuntimeDefault, `unshare -U -m true` =
EPERM".

## Decisions

Orchestrator decision D94, 2026-10-27 (Timo: "Speed up your work ... Be faster."), taken inside
the item:

- D1. `Unconfined` is refused unless the controller's configuration sets `allowUnconfinedProfiles:
  true` (default `false`, chart value of the same name): the cluster's operator grants it, never
  the resource alone.
- D2. A `Localhost` profile is admitted only when its name is in `allowedSeccompLocalhostProfiles`
  or `allowedAppArmorLocalhostProfiles` (separate lists; empty admits none), as Secrets and claims
  are allow-listed.
- D3. The profiles go on the program container's `securityContext`; the init containers and the
  pod keep `RuntimeDefault`. Only `Command` declares them; on `AgentServe` they are `Malformed`.
- D4. Additive: a resource that declares none renders as before. The two new `Config` and spec
  members are listed as breaking for a struct-literal consumer, as the earlier ones were.
- D5. The refusals stay in the operator's admission, held by Rust tests, as `ClaimNotAllowed` is.
