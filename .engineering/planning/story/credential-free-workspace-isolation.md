---
format: aep.planning-md/3
id: story:credential-free-workspace-isolation
kind: story
status: draft
title: A credential-free agent cannot reach what its policy closes or a Secret-bearing agent's claim
relations:
- derived_from: epic:credential-free-agent-pod
revision: 1
---
## Outcome

A credential-free `LoomAgent` cannot be pointed at what its NetworkPolicy closes or at state a
Secret-bearing agent leaves behind: `spec.routes` endpoints beside a boundary must be the loopback
model relay, and a workspace claim a Secret-bearing resource mounts is not admitted for a
credential-free one.

## Why

Wave 2026-10-30-w1 reviews: the u1 fix round left "routes given in `spec.routes` that point at an
endpoint the pod cannot reach beside a boundary are still admitted"; the u1 adversary pass noted
that a credential-free resource may mount read-write the workspace claim a Secret-bearing resource
mounts (INFEASIBLE today: not shown that such a run leaves credentials there).

## Acceptance

- `routes_beside_a_boundary_name_only_the_model_relay`.
- `a_claim_a_secret_bearing_agent_mounts_is_refused_for_a_credential_free_one` (or a decision that
  the claim allowlist already carries that intent).

## Scope

- `crates/loom-operator/src/admission.rs`, `tests/render.rs`, AGENTS.md § Operator.
