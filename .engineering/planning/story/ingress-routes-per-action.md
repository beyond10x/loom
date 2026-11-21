---
format: aep.planning-md/3
id: story:ingress-routes-per-action
kind: story
status: draft
title: An action selects named ingress routes the engine declares, and Substrate serves them to its process
summary: Lifts the load-time IngressUnserved refusal on a confinement that serves network_ingress; held by upstream-blocker:substrate-ingress
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-daemon
revision: 3
---
## Outcome

An action can select named ingress routes that the engine configuration declares, and Substrate
serves each to the action's process as an inbound endpoint. The engine configuration's `ingress`
section declares each route once (a name, a port, TCP); a binding's `ingress` selects routes by name
and never states a port. When the opened confinement serves the family `network_ingress`, read
through the mapping of `story:confinement-level-required`, Loom hands the selected names to
Substrate with the start, on both launch paths: in process through the request its runner hands the
`linux` adapter, and under `Daemon` through the SDK's start; it records in the step's confinement
record the endpoint Substrate reports for each. The load-time refusal `IngressUnserved` of
`story:named-egress-per-action` then applies only to a confinement that does not serve that
family. A new member of Loom's recorded projection of the request is written only when routes are
selected, so a run without ingress keeps its `profile_digest`.

The story's first commit is the `## ESS first` change and nothing else; its second commit moves
Loom's Substrate pin to the release that clears its blocker: `substrate-host`, `substrate-wire` and
`b10x-substrate-sdk` in `crates/loom-intake-slice/Cargo.toml` (the SDK line is there since
`story:substrate-daemon-runner`), and the version and revision that
`crates/loom-intake-slice/tests/host_git_hardening.rs:549-552` asserts.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "egress and ingress allow-lists
(host, port, direction)". `upstream-blocker:substrate-ingress`: at Substrate 0.21.0
`ingress_routes` is reserved and any value is refused
(`crates/substrate-host/src/confinement/profile.rs:93-96`), no adapter serves `network_ingress`
(`linux.rs:29`), and the SDK has no ingress member. `story:named-egress-per-action` declares
`ingress` in a binding and refuses it at load; this story serves it. Session `substrate` planned
`story:inbound-endpoint-for-a-confined-exec` and `story:confinement-profile-ingress-routes`
(drafts, 2026-11-21). Loom's `ConfinementSpec` has `egress` and no ingress member
(`ess/intake/domains/confinement.yaml:132-147`), and `AppliedConfinement` holds `network` as a list
of `AppliedAperture` and nothing for an endpoint (`:53-71`).

## Waits on

- `upstream-blocker:substrate-ingress`: a Substrate release in which an adapter serves a declared
  inbound endpoint for a run.
- `story:tree-daemon`: the last unheld story of the chain. Through it,
  `story:named-egress-per-action` (`ingress` in `BoundAction`, `IngressUnserved`),
  `story:confinement-level-required` (the mapping) and `story:substrate-daemon-runner` (`Daemon`,
  the SDK dependency) come first. No edge orders this story against
  `story:executable-descendants-held`, `story:confinement-level-on-oci-and-pod` or
  `story:substrate-daemon-on-another-host`, each held by another blocker; they share the launch path
  in `crates/loom-intake-slice/src/confinement.rs` and `ess/domains/engine.yaml`, and their order on
  them is settled by scope when this story's blocker clears.

## ESS first

`ess/domains/engine.yaml`: `IngressSection` (name, port), `EngineConfiguration` gains optional
`ingress`, `EngineRefusal` gains `IngressUndeclared`, and `StepConfinement` gains `ingress` (name
and the reported endpoint). `ess/intake/domains/confinement.yaml`: `ConfinementSpec` gains optional
`ingress` (route names), `AppliedConfinement` gains optional `ingress` (name and endpoint), and the
recorded `ConfinementProfile` gains optional `ingress`. On that commit `task drift` and
`task intake-drift` fail and `crates/loom-tree/tests/action_ingress.rs` does not compile.

## Acceptance

Under the in-process `linux` adapter and under `Daemon` with a managed daemon, both of the
Substrate release that clears the blocker and serving `network_ingress`:

- `Cargo.lock` holds one copy of each Substrate crate, at that release, and
  `tests/host_git_hardening.rs` asserts that release.
- A listener program whose action selects a declared route is reachable from the host at the
  endpoint the step's confinement record names.
- A listener program whose action selects no route is not reachable from the host.
- A binding that selects an undeclared route is refused at `tree open` `IngressUndeclared`.
- A run with no `ingress` has the same `profile_digest` as before this story.

Under the in-process `linux` adapter of Substrate 0.21.0 (a test against the mapping with that
release's families), which serves no `network_ingress`, a binding with `ingress` is refused at load
`IngressUnserved`; under the `local` adapter of the new release the same.

- `task check` exits 0.

## Out of scope

UDP. Rate bounds (`rate_bounds`). Ingress on `oci` and `pod`.
