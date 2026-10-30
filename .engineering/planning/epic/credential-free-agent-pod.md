---
format: aep.planning-md/3
id: epic:credential-free-agent-pod
kind: epic
status: draft
title: A LoomAgent runs credential-free behind a Loom-owned boundary container
summary: The agent container holds no secret; a boundary sidecar with a projected token relays model, Connectors and git
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A `LoomAgent` runs credential-free: its agent container holds no Secret, no token and no key, and
reaches the model, the declared Connectors operations and the declared git repositories only
through a Loom-owned boundary container in its own pod. The boundary holds only a projected
service-account token, audience-bound per upstream and rotated by the kubelet, and adds it to each
upstream call; the processes that hold the provider key, the forge credential and the gates policy
run outside the pod and admit the pod by that token. Any Loom host, not only takt, gets this by
declaring `spec.boundary`.

## Why

Operator direction, 2026-10-30, relayed verbatim: "we want to inject a synthetic tool which is
basically proxying the thing, so the secret lives outside on the connector's side or the tool side
... and the agent is just calling a tool via some proxy, effectively never seeing the secret"; "use
llm-gateway or Substrate, and wire it into Loom so other apps besides takt can use it."

Design D243 (2026-10-30, a credential-free agent pod), decisions A1 (a Loom-owned boundary sidecar,
uid 10002, holding only a projected service-account token with an audience per upstream, 600 s),
B1 (the upstreams admit the pod by that token through TokenReview), C1 (git over smart-HTTP to the
Connectors instance; the boundary gets no shared workspace volume) and E1 (llm-gateway takes
ingress none plus a NetworkPolicy for the first runs).

## Acceptance

- A `LoomAgent` with `spec.boundary` renders a boundary container holding a projected token that
  the agent container cannot read, and is refused beside `secretRef` or `secretFiles`.
- The boundary serves the model, the Connectors wire and a git smart-HTTP relay on loopback, for
  declared operations and repositories only, adding the projected token upstream.
- The pod's NetworkPolicy allows DNS and the declared upstream namespaces on their ports, and no
  host.

## Scope

Loom's part only. The Connectors admission by service-account token, served writes, the git
relay with quarantine on the instance, the GitHub operations and takt's use of the boundary are
other repositories' stories (design D243 § 6).
