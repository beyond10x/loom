---
format: aep.planning-md/3
id: story:boundary-admits-declared-http-egress
kind: story
status: draft
title: A credential-free LoomAgent may reach an in-cluster http source it declares, through a namespace egress peer the operator allows
summary: Namespace entry admitted beside spec.boundary only outside upstream namespaces, with a selector, bound to a GET http binding with no bearer and listed in allowedBoundaryHttpEgress; chart changes
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:agent-selects-the-connectors-relay
revision: 1
---
## Outcome

A `LoomAgent` with `spec.boundary` may reach an in-cluster `http` source it declares. Admission
step 5 (`crates/loom-operator/src/admission.rs:1361`) admits a `deploy.kubernetes.egress` entry of
`spec.agent` that names a namespace beside `spec.boundary` only when every one of these holds, and
otherwise refuses it at the entry:

1. its `namespace` is the namespace of no declared upstream (`BoundaryEgressNamespace` at its
   `namespace`, the rule and detail of today for that case);
2. its `selector` is not empty (`BoundaryEgressNamespace` at its `selector`: an empty selector opens
   every pod of the namespace);
3. an `http` binding of `spec.agent` with `method: GET` and no `bearer_env` has a `base_url` whose
   host is a cluster service in that namespace (`<service>.<namespace>.svc` or
   `<service>.<namespace>.svc.cluster.local`, `admission::cluster_service`)
   (`BoundaryEgressUnbound` at the entry);
4. an entry of the controller configuration's new `allowedBoundaryHttpEgress` list,
   `{namespace, selector, port}` (`config::AllowedHttpEgress`, unknown members refused; empty, the
   default, admits none), gives exactly its namespace, its label set and its port
   (`BoundaryEgressNotAllowed` at the entry).

An entry naming a host stays refused (`BoundaryEgressHost`). The NetworkPolicy of such a resource
opens DNS, each upstream as today, and each admitted entry as a peer of its namespace and pod
selector on its TCP port, as it renders an entry without a boundary
(`crates/loom-operator/src/render.rs:1179-1206`, today skipped beside a boundary at `:1180-1184`).
Nothing is added to the boundary container, the agent container or the relays. A resource without
`spec.boundary`, and one with a boundary and no namespace entry, renders byte for byte as before.

## Why

Gap 2 of `.engineering/drafts/e6-request.md`: admission refuses every namespace egress entry beside
a boundary (`admission.rs:1473-1480`, rule text `:65-69` and `:1359-1360`), and the boundary
relays `model`, `connectors` and `git` only (`render.rs:245-263`), so an agent that reads Loki,
Prometheus and a health endpoint over `http` bindings loses them once it gets a boundary.

What the refusal stops: commit `85c92a0f` (2026-10-30, wave D243-L) added it to fix "the adversary
finding that a namespace entry opened an upstream's namespace on further ports"; its text is "a
resource with spec.boundary reaches only DNS and its boundary's upstreams". The upstreams admit the
pod by its projected token (design D243 decision B1, as `epic:credential-free-agent-pod` and
`approval-record:wave-d243-l` quote it); a destination that admits by network position alone is
what the rule keeps closed. Design D243 itself is not in this repository or the workspace (searched
2026-11-21); its § 2 is read only as the stories quote it: "DNS plus the credential namespace on
the declared ports, no host entries" (`story:credential-free-network-policy`).

The two options asked, compared:

- (a), this story. Cost: admission, render, one configuration list and the chart; no relay, no
  CRD member. Lets through: from any process in the pod, any method and path on the selected pods'
  port (a NetworkPolicy is layer 4 and pod-wide), so a source's write API on the same port as its
  reads (Loki's push, say) is reachable; GET-only and the allow-listed paths are held in process by
  `HttpGet` (`loom.agent.AdmitHttpRead`) only. Conditions 1 and 4 keep the adversary fix and put
  each opening under the cluster operator, as `allowedBoundaryUpstreams` does for upstreams.
- (b), a fourth relay. Cost: `spec.boundary.http` in `ess/domains/resource.yaml` and the CRD, flags
  and a listener in `crates/loom-boundary`, render, admission, the chart, and the agent rewriting an
  `http` binding's `base_url` to the relay. Lets through the same: the relay adds no token, so the
  upstream cannot tell the relay from the agent, and the NetworkPolicy that lets the boundary reach
  the source lets the agent container reach it on the same port (the policy selects pods, not
  containers). Its method and path filter is one the agent container can go around, and duplicates
  `HttpGet`'s for a Loom agent.

The documents point to (a)'s shape: every D243 relay exists to add a credential the agent lacks
(A1, B1), and the network rule is written in namespaces and ports. (b) would be a relay with no
credential that a pod-wide policy makes optional. So the code and the documents settle it, and the
residual exposure of (a) is the cluster operator's to accept per entry (`## Open` of the plan).

## Waits on

- `story:agent-selects-the-connectors-relay`: both edit admission step 5 in
  `crates/loom-operator/src/admission.rs`, `AdmissionRefusalReason` and `AdmitLoomAgent` in
  `ess/domains/resource.yaml`, and `ess/scenarios/resource-admission/config.yaml`.

## ESS first

The first commit changes only `ess/`: `ess/domains/resource.yaml` — `AdmissionRefusalReason` gains
`BoundaryEgressUnbound` and `BoundaryEgressNotAllowed`, `AdmitLoomAgent` an `external:` outcome for
each, and the comment above the enum (`:412-413`) the new rule;
`ess/scenarios/resource-admission/config.yaml` gains `allowedBoundaryHttpEgress`; authored scenarios
`a-boundary-beside-an-unbound-egress-namespace-is-refused.yaml`,
`a-boundary-beside-an-egress-namespace-the-configuration-does-not-give-is-refused.yaml`,
`a-boundary-beside-an-egress-in-an-upstream-namespace-is-refused.yaml` and
`a-boundary-beside-an-allowed-http-egress-is-admitted.yaml`, with their resource files; the existing
`a-boundary-beside-an-egress-namespace-is-refused.yaml` keeps its reason. On it `task conform` is red.

## Acceptance

`crates/loom-operator/tests/render.rs` and `admission_reasons.rs`, no cluster:

- `a_declared_http_egress_beside_a_boundary_is_admitted_and_rendered`: the NetworkPolicy holds DNS,
  the upstream rules of today and exactly one more rule, the entry's namespace and pod selector on
  its port.
- `an_egress_in_an_upstream_namespace_beside_a_boundary_is_refused`: an entry naming the model
  upstream's namespace with a selector and an allowed configuration entry is
  `BoundaryEgressNamespace`.
- `an_egress_with_an_empty_selector_beside_a_boundary_is_refused`.
- `an_egress_no_get_binding_names_is_refused`: no `http` binding in that namespace, a `POST`
  binding, and a GET binding with `bearer_env`, each `BoundaryEgressUnbound`.
- `an_egress_the_configuration_does_not_give_is_refused`: another port, another label set, an empty
  `allowedBoundaryHttpEgress`, each `BoundaryEgressNotAllowed`.
- `a_host_egress_beside_a_boundary_is_still_refused`.
- `the_boundary_container_and_the_agent_container_are_unchanged_by_an_http_egress`: the pod spec
  but the NetworkPolicy equals the same resource's without the entry.
- The golden files `tests/golden/*.yaml` are unchanged; a new golden
  `tests/golden/credential-free-http-egress.yaml` is blessed and reviewed.
- `crates/loom-operator/tests/chart.rs`: the chart renders `allowedBoundaryHttpEgress` from values
  into `config.yaml`, empty by default.
- `task conform`, `task crd-drift` (the CRD is expected unchanged), `task chart-check` and
  `task check` exit 0.

CHANGELOG: `### Changed`, a namespace egress entry beside `spec.boundary` is admitted under
`allowedBoundaryHttpEgress`. No `**Breaking:**` bullet: every resource admitted today is admitted
and renders the same, and the default list admits nothing new. The chart changes (`values.yaml`,
`templates/configmap.yaml`); the CRD does not.

## Files

- `ess/domains/resource.yaml`, `ess/scenarios/resource-admission/` (config and four scenarios with
  resource files), `generated/rust/loom/`
- `crates/loom-operator/src/{admission.rs,render.rs,config.rs}`, `tests/render.rs`,
  `tests/admission_reasons.rs`, `tests/chart.rs`, `tests/golden/credential-free-http-egress.yaml`
- `crates/loom-conformance/src/admission.rs` (only if the config reader there needs the member)
- `charts/loom-agents/values.yaml`, `charts/loom-agents/templates/configmap.yaml`, `Taskfile.yml`
  (`chart-check` overrides)
- AGENTS.md § Operator and § Resource, CHANGELOG.md, `website/` operator pages

## Out of scope

An `http` relay in the boundary (option (b), above). A credential on an `http` binding beside a
boundary. An egress entry naming a host. Layer-7 policy (method or path in the NetworkPolicy).
`story:credential-free-workspace-isolation` (draft, another epic), which also edits
`admission.rs`: no edge; the second to land rebases.
