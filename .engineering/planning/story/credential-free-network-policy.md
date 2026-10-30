---
format: aep.planning-md/3
id: story:credential-free-network-policy
kind: story
status: active
title: A credential-free agent's NetworkPolicy allows DNS and namespace peers only
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T00:48:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T00:48:54Z", actor: "human:timo", revision: 3}
---
## Outcome

A credential-free `LoomAgent` (one declaring `spec.boundary`) gets a NetworkPolicy that allows
egress to DNS and, for each declared upstream, to its namespace on its port, and nothing else: no
host entry and no model-route rule. An egress entry that names a host is refused beside
`spec.boundary`; an upstream that is not a cluster service URL (`http://<service>.<namespace>.svc`
with an optional `.cluster.local` and port) is refused, since its namespace cannot be derived.

## Why

Design D243 § 2 (the pod's NetworkPolicy: DNS plus the credential namespace on the declared
ports, no host entries). The dev cluster's CNI (kindnet with its kube-network-policies
controller) enforces NetworkPolicy (read on 2026-10-30), so the policy is a second layer behind
the upstreams' token admission.

## Acceptance

- `a_credential_free_agent_reaches_dns_and_its_upstream_namespaces_only`.
- `a_host_egress_entry_beside_a_boundary_is_refused`.
- `an_upstream_that_is_no_cluster_service_is_refused`.

## ESS first

No ESS change beyond `story:boundary-container-projected-token`'s types; the refusals are the
controller's.

## Scope

- `crates/loom-operator/src/{admission,render}.rs`, `tests/render.rs`
- AGENTS.md § Operator, CHANGELOG.md
