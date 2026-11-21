---
format: aep.planning-md/3
id: epic:boundary-agent-reaches-connectors-and-http-sources
kind: epic
status: proposed
title: A LoomAgent behind spec.boundary does its Connectors work through the relay and reads declared in-cluster http sources, with no credential in its agent container
summary: Wire client to the Connectors relay, selected by LOOM_BOUNDARY_CONNECTORS_URL; writes in the operation's declared input shape; operator-allowed namespace egress for credential-free GET http bindings
tags:
- ai-agent-platform
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:02Z", actor: "agent:sdc", revision: 5, decided_on: {"recorded":{"review_outcome":9}}, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
---
## Outcome

A `LoomAgent` behind `spec.boundary` does its Connectors work and reads its in-cluster `http`
sources with no credential in its agent container. With `spec.boundary.connectors`, `b10x-loom
agent serve`, `agent run` and `agent resume` speak the Connectors wire themselves to the boundary's
relay at `LOOM_BOUNDARY_CONNECTORS_URL`, for the Slack poll, `source.read` over a `connectors`
binding and `reply.send`, with no `connectors` program in the image and no token in the agent
container; the write keeps today's policy (`AdmitSend`, the admitted-arguments bound, the
idempotency key, one send never retried), and a spec that one relay cannot serve (a second
transport, several Connectors instances) is refused at start. A write of the plugin path sends its
arguments in the shape the operation's described input schema declares, so a catalog write such as
Slack's `chat.postMessage` receives `{body: {channel, text}}`. A credential-free agent may reach an
in-cluster `http` source it declares for credential-free reads, through a NetworkPolicy peer the
cluster operator allows, never in the namespace of an upstream the operator allows.

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
`expirationSeconds` 600 (`:722-742`); `automountServiceAccountToken` is false. The agent container
gets `LOOM_BOUNDARY_MODEL_URL`, `LOOM_BOUNDARY_CONNECTORS_URL` and `LOOM_BOUNDARY_GIT_URL` for the
declared upstreams; the model relay reaches the agent through `routes.yaml`, whose default route
names `http://127.0.0.1:8401/v1` (AGENTS.md § Operator). The NetworkPolicy opens DNS and each
upstream's namespace on one port.

The boundary came from `epic:credential-free-agent-pod` (design D243; wave D243-L,
`approval-record:wave-d243-l`). It built the relays, the container and the NetworkPolicy, and
scoped the client side out: "takt's use of the boundary [is] other repositories' stories" (its
§ Scope), and "the host (takt), not the model, calls the forge" (`story:boundary-mcp-tools`, draft,
deferred). So the Connectors client in Loom's own agent was neither planned nor refused, and no
story covers either gap. `http` sources are not mentioned in D243 as the stories quote it.

## Stories

1. `story:boundary-admits-declared-http-egress` (gap 2)
2. `story:connectors-relay-client`
3. `story:send-input-in-the-declared-shape`
4. `story:agent-selects-the-connectors-relay`

Gap 2 does not wait on gap 1: `story:boundary-admits-declared-http-egress` depends on no story of
the set, and `story:connectors-relay-client` shares no file with it but `CHANGELOG.md` and other
sections of `AGENTS.md`, so the two may start together. Every other pair that lands on one file is
ordered by a `depends_on` edge naming the file in both bodies:

- `story:send-input-in-the-declared-shape` depends on `story:connectors-relay-client`
  (`crates/loom-connectors/src/{lib.rs,relay.rs,cli/write.rs}`, `tests/connectors_relay.rs`,
  `Cargo.lock`, AGENTS.md § Connectors) and on `story:boundary-admits-declared-http-egress`
  (`ess/ess-inputs.yaml`, the plan files under `generated/rust/loom/`).
- `story:agent-selects-the-connectors-relay` depends on `story:connectors-relay-client` (the client
  it selects, `crates/loom-connectors/src/lib.rs`), on `story:send-input-in-the-declared-shape`
  (`ess/ess-inputs.yaml`, the command match in `crates/loom-conformance/src/lib.rs`,
  `crates/loom-connectors/src/lib.rs`, the fixtures under
  `crates/loom-connectors/tests/fixtures/connectors/`, AGENTS.md § Connectors; and its end-to-end
  test needs the shaped write) and on `story:boundary-admits-declared-http-egress`
  (`crates/loom-operator/src/admission.rs`, `tests/render.rs`, `tests/adversary_d243_u1.rs`,
  `ess/scenarios/resource-admission/`, `ess/ess-inputs.yaml`).

Gap 1 is closed for the consumer when story 4 lands, on stories 2 and 3; gap 2 by story 1.
`CHANGELOG.md` and distinct sections of `AGENTS.md` are not ordered: the second story to land
rebases. So does the second of `story:boundary-admits-declared-http-egress` and four draft stories
of other epics that edit the same admission files with no edge
(`story:credential-free-workspace-isolation`, `story:egress-by-host`,
`story:secret-files-several-secrets`, `story:command-home-survives-pod-replacement`; that story's
§ Waits on names the files).

## Acceptance

- `story:agent-selects-the-connectors-relay` owns the end-to-end check,
  `an_auto_run_sends_its_reply_through_the_relay_in_the_declared_shape` in
  `crates/loom-agent/tests/connectors_relay.rs`: an `agent run` with `writes.mode: auto`,
  `spec.boundary.connectors` and no `runtime.connectors`, against an in-process boundary relay and a
  fake Connectors service on loopback whose describe answers Slack's `chat.postMessage` schema at
  connectors `v0.73.0`, performs one keyed invoke whose input is `{body: {channel, text}}`; every
  request the fake sees carries the boundary's fixture token; no `connectors` program starts.
- The relay client sends no credential of its own (`story:connectors-relay-client`,
  `the_relay_client_sends_no_credential`, with no boundary in front, since a boundary replaces the
  agent's credential headers), refuses any Connectors instance but the one it was built for before
  any request, and answers each stage of a write (describe, then invoke) with one result.
- The Slack poll and `agent serve` under the relay start no `connectors` program
  (`story:agent-selects-the-connectors-relay`).
- A write the send policy refuses (window full, channel outside the allow list) reaches no upstream,
  and one the relay refuses (an operation the boundary does not list) is recorded refused.
- A credential-free resource with a namespace egress entry that a credential-free `http` binding
  names by namespace and an `allowedBoundaryHttpEgress` entry matches (the same namespace, port
  and selector, the selector compared as a whole set) is admitted and its NetworkPolicy opens
  that peer only, with no credential, variable or volume added to either container; the same entry
  in an allowed upstream's namespace, with an empty selector, unbound or not allowed is refused; a
  host entry stays refused (`story:boundary-admits-declared-http-egress`).
- `task conform`, `task crd-drift`, `task chart-check` and `task check` exit 0.

One `**Breaking:**` bullet in the epic: a `LoomAgent` declaring both `spec.boundary.connectors` and
`spec.agent.runtime.connectors` is refused (`story:agent-selects-the-connectors-relay`). The CRD
does not change; the chart changes in `story:boundary-admits-declared-http-egress`
(`allowedBoundaryHttpEgress`, empty by default).

## Not in scope

- An `http` relay in the boundary (gap 2's option (b); `story:boundary-admits-declared-http-egress`
  says why).
- Several Connectors instances behind one boundary: refused at start (`SeveralInstances`), and by
  the relay client before any request (`relay-other-instance`).
- Shaping Commission's `ConnectorsInvoker` write path (`story:send-input-in-the-declared-shape`
  says why).
- TLS from the boundary to an upstream (refused today; a later story of the D243 epic).
- MCP tools to a model session (`story:boundary-mcp-tools`).
- The Connectors host's configuration: `service-account-token` admission and `standing_grants`
  (connectors `contracts/service/compatibility.md` § 2.1) are the consumer's deployment.
- llm-gateway's service-account token on its model ingress (the requester's; see
  `e6-llm-gateway-note.md`).
- A live run against a cluster, a Connectors host or Slack; a qualification record says what one is.
