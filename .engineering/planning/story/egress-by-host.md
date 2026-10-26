---
format: aep.planning-md/3
id: story:egress-by-host
kind: story
status: draft
title: A LoomAgent's declared egress is enforced by host, not only by port
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: charts/loom-agents
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: inferred
  path: deploy/crd
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: inferred
  path: generated/rust/loom
revision: 3
---
## Outcome

A `LoomAgent`'s declared egress (`deploy.kubernetes.egress`) is enforced by host name, not only by
port: the pod reaches the hosts it names and no other host on the same port. Where the cluster's
NetworkPolicy cannot express a host (it selects IP blocks and pods), the controller renders an
enforcement that can (an egress proxy sidecar or the CNI's FQDN policy, chosen in the story's
design step), and a resource whose egress cannot be enforced as declared is refused or marked
`Degraded`, never silently widened.

## Why

Trial S5/S6, 2026-10-26: the takt pod's egress was open to any host on the declared ports. Gap:
"Host egress enforced by host, not only by port". AGENTS.md § Rules: "fail toward less authority
... Never silently broaden capability."

## Acceptance

- A design note picks the enforcement (proxy sidecar, CNI FQDN policy, or refusal) with its
  trade-offs, recorded as a decision before code.
- ESS first: the resource records whether egress is enforced by host; scenarios red first.
- `a_request_to_an_undeclared_host_on_a_declared_port_is_blocked` (render-level test of the chosen
  mechanism; a kind run is the manual check).
- `egress_that_cannot_be_enforced_by_host_is_refused_or_degraded`.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`. Confidence low until the mechanism is chosen.

- `crates/loom-operator/src/render.rs` (NetworkPolicy) — cited
- `ess/domains/agent.yaml` (`KubernetesEgress`), `ess/domains/resource.yaml` — cited
- `charts/loom-agents/` (a proxy image or CNI policy RBAC) — inferred
- `generated/rust/loom/`, `deploy/crd/` — inferred
- design § 7 (Security), AGENTS.md § Resource, CHANGELOG.md — cited
