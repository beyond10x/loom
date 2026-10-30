---
format: aep.planning-md/3
id: approval-record:wave-d243-l
kind: approval-record
status: approved
title: 'Wave D243-L accepted under the orchestrator''s grant: boundary container, loopback relays, credential-free NetworkPolicy'
tags:
- non-interactive
relations:
- decides: story:boundary-container-projected-token
- decides: story:boundary-loopback-relays
- decides: story:credential-free-network-policy
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T00:48:52Z", actor: "human:timo", revision: 2}
---
# Wave D243-L: a credential-free agent pod (boundary container, loopback relays, NetworkPolicy)

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved the wave under the grant in the workspace instructions (class C: a wave, its commits,
one bot pull request, its merge and one release), 2026-10-30, relaying the operator's direction
verbatim: "we want to inject a synthetic tool which is basically proxying the thing, so the secret
lives outside on the connector's side or the tool side ... and the agent is just calling a tool
via some proxy, effectively never seeing the secret"; "use llm-gateway or Substrate, and wire it
into Loom so other apps besides takt can use it."

The orchestrator's decisions on design D243 (2026-10-30), applied as given:

- "A1: a Loom-owned boundary sidecar, uid 10002, holding only a projected service-account token,
  with audience per upstream, 600 s."
- "B1: the upstreams admit the pod by that token through TokenReview."
- "C1: git goes over smart-HTTP to the connectors instance. The boundary gets no shared workspace
  volume."
- "E1: llm-gateway takes ingress none plus a NetworkPolicy for the first runs."
- Units L1, L2 and L3 as filed; "L4 (MCP tools to the model) is not in this wave."

| unit | story | surface | review |
|---|---|---|---|
| s0 | ESS first for `story:boundary-container-projected-token` | `ess/domains/resource.yaml` | none (spec only) |
| u1 | `story:boundary-container-projected-token` and `story:credential-free-network-policy` | `crates/loom-operator`, generated, CRD, chart | adversary, security-reviewer |
| u2 | `story:boundary-loopback-relays` | `crates/loom-boundary` (new), `crates/loom-cli` | adversary, security-reviewer |

L1 and L3 land on the same two files (`admission.rs`, `render.rs`), so one implementor carries
both; u2 is disjoint and runs beside it.

## Decisions taken inside the item

- The boundary reads its own projected service-account token file. AGENTS.md § Never ("Never read
  a credential file in Loom code or tests") is narrowed to exactly that file, which the kubelet
  writes for the pod; tests read a fixture token they write themselves. Reason: A1 is that read.
- The boundary is a Kubernetes native sidecar (an init container with `restartPolicy: Always`), so
  a `Command` resource's Job still completes when its program exits.
- The boundary runs from the controller's default image as `b10x-loom agent boundary`, as the
  `stage-files` and `prepare-dirs` init containers do: no second image to build.
- Upstreams are plain `http://` cluster service URLs for this wave (the gateway and the Connectors
  host serve plain TCP today, design D243 § 1 row 3); an `https` upstream is refused. In-cluster
  TLS is a later story.
- The controller's configuration lists the upstreams a resource may name
  (`allowedBoundaryUpstreams`, empty admits none), as `allowedEgressHosts` lists hosts.
- ESS stays at 0.77.0, the newest release on 2026-10-30.

## Cluster read (step 1 of the item, read-only)

The dev cluster (kind v0.33.0, Kubernetes v1.37.0) runs kindnet `kindest/kindnetd:v20260820-69b56db7`,
whose log reads `"Starting controller" name="kube-network-policies"` and `"Policy engine is
ready."` and syncs nftables rules: NetworkPolicy is enforced by kindnet's embedded
kube-network-policies controller. No packet was sent to prove a deny; nothing in the cluster was
changed.
