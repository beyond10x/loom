---
format: aep.planning-md/3
id: epic:boundary-agent-reaches-connectors-and-http-sources
kind: epic
status: draft
title: A LoomAgent behind spec.boundary does its Connectors work through the relay and reads declared in-cluster http sources, with no credential in its agent container
summary: Wire client to the Connectors relay, selected by LOOM_BOUNDARY_CONNECTORS_URL; writes in the operation's declared input shape; operator-allowed namespace egress for credential-free GET http bindings
tags:
- ai-agent-platform
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A `LoomAgent` behind `spec.boundary` does its Connectors work and reads its in-cluster `http`
sources with no credential in its agent container. With `spec.boundary.connectors`, `b10x-loom
agent serve`, `agent run` and `agent resume` speak the Connectors wire themselves to the boundary's
relay at `LOOM_BOUNDARY_CONNECTORS_URL`, for the Slack poll, `source.read` over a `connectors`
binding and `reply.send`, with no `connectors` program in the image and no token in the agent
container; the write keeps today's policy (`AdmitSend`, the admitted-arguments bound, the
idempotency key, one send never retried). A governed write sends its arguments in the shape the
operation's described input schema declares, so a catalog write such as Slack's
`chat.postMessage` receives `{body: {channel, text}}`. A credential-free agent may reach an
in-cluster `http` source it declares for credential-free GET reads, through a NetworkPolicy peer
the cluster operator allows, never in a boundary upstream's namespace.

## Why

Request of session `ai-agent-platform`, 2026-11-21 (`.engineering/drafts/e6-request.md`), read at
Loom tag `0.57.2` (tag object `5e54e37`, commit `3cb82b7c`); this tree is `39f4da88`, and
`git diff --stat 0.57.2 39f4da88 -- crates/loom-operator crates/loom-agent crates/loom-connectors
crates/loom-plugin deploy charts` shows only `crates/loom-operator/src/main.rs` and
`tests/startup.rs` (startup, not boundary code). The consumer is the platform's chart: an hourly
operator agent that posts one report to one Slack channel through a Connectors catalog instance and
reads logs, metrics and a health endpoint in-cluster. The operator's grant of priority reached
session `loom` relayed, not first-hand.

What the boundary renders today (`crates/loom-operator/src/render.rs`, AGENTS.md § Operator and
§ Credential boundary): a native sidecar `boundary` (init container, `restartPolicy: Always`,
user 10002, default image, `b10x-loom agent boundary`) with one relay per declared upstream on
loopback, model 8401, Connectors 8402, git 8403 (`RELAYS`, `:245-263`). Each relay authenticates
upstream with `Authorization: Bearer <projected service-account token>`, read again on every request
from `/var/run/loom/boundary/tokens/<kind>` (`BOUNDARY_TOKENS_DIR`, `:217`), a projected volume
mounted into the boundary alone, one token per upstream with the declared audience and
`expirationSeconds` 600 (`:719-741`); `automountServiceAccountToken` is false. The agent container
gets `LOOM_BOUNDARY_MODEL_URL`, `LOOM_BOUNDARY_CONNECTORS_URL` and `LOOM_BOUNDARY_GIT_URL` for the
declared upstreams; the model relay reaches the agent through `routes.yaml`, whose default route
names `http://127.0.0.1:8401/v1` (AGENTS.md § Operator). The NetworkPolicy opens DNS and each
upstream's namespace on one port.

The boundary came from `epic:credential-free-agent-pod` (design D243; wave D243-L,
`approval-record:wave-d243-l`). It built the relays, the container and the NetworkPolicy, and
scoped the client side out: "takt's use of the boundary [is] other repositories' stories" (its
§ Scope), and "the host (takt), not the model, calls the forge" (`story:boundary-mcp-tools`, draft,
deferred). So the Connectors client in Loom's own agent was neither planned nor refused: the epic
built for takt's `Command` pod, and no story covers either gap (the store's stories naming the
boundary are listed in the plan's `## Verification`). `http` sources are not mentioned in D243 as the
stories quote it.

## Stories

1. `story:connectors-relay-client`
2. `story:agent-selects-the-connectors-relay`
3. `story:send-input-in-the-declared-shape`
4. `story:boundary-admits-declared-http-egress`

Story 1 comes first: stories 2 and 3 use its client. Story 2 depends on 1; story 3 depends on 1
(both edit `crates/loom-connectors`; 3 shapes the write of both transports). Story 4 depends on 2,
because both edit admission step 5 in `crates/loom-operator/src/admission.rs`, the enum
`AdmissionRefusalReason` and `AdmitLoomAgent` in `ess/domains/resource.yaml` and
`ess/scenarios/resource-admission/config.yaml`; it needs nothing of 2's behaviour. Stories 2 and 3
share no file but `AGENTS.md` and `CHANGELOG.md` and may run in one wave. Gap 1 is closed for the
consumer by 1, 2 and 3 together; gap 2 by 4.

## Acceptance

- An `agent run` with `writes.mode: auto`, `spec.boundary.connectors` and no `runtime.connectors`,
  against an in-process boundary relay and a fake Connectors service on loopback, performs one keyed
  invoke whose input is `{body: {channel, text}}` for an operation that declares a body; the fake
  sees only the boundary's fixture token; no `connectors` program starts
  (`crates/loom-agent/tests/connectors_relay.rs`).
- A write the send policy refuses (window full, channel outside the allow list, an operation the
  boundary does not list) reaches no upstream.
- A credential-free resource with a namespace egress entry that a credential-free GET `http`
  binding names and `allowedBoundaryHttpEgress` gives is admitted and its NetworkPolicy opens that
  peer only; the same entry in an upstream's namespace, with an empty selector, unbound or not
  allowed is refused; a host entry stays refused.
- `task conform`, `task crd-drift`, `task chart-check` and `task check` exit 0.

One `**Breaking:**` bullet in the epic: a `LoomAgent` declaring both `spec.boundary.connectors` and
`spec.agent.runtime.connectors` is refused (story 2). The CRD does not change; the chart changes
in story 4 (`allowedBoundaryHttpEgress`, empty by default).

## Not in scope

- An `http` relay in the boundary (gap 2's option (b); story 4 says why).
- A token-less constructor in `connectors-client`; the relay client sends a fixed placeholder bearer
  the boundary drops.
- TLS from the boundary to an upstream (refused today; a later story of the D243 epic).
- MCP tools to a model session (`story:boundary-mcp-tools`).
- The Connectors host's configuration: `service-account-token` admission and `standing_grants`
  (connectors `contracts/service/compatibility.md` § 2.1) are the consumer's deployment.
- llm-gateway's service-account token on its model ingress (the requester's; see
  `e6-llm-gateway-note.md`).
- A live run against a cluster, a Connectors host or Slack; a qualification record says what one is.
