---
format: aep.planning-md/3
id: story:agent-selects-the-connectors-relay
kind: story
status: draft
title: agent serve, run and resume use the Connectors relay when LOOM_BOUNDARY_CONNECTORS_URL is set, and start no connectors program
summary: AdmitConnectorsTransport in loom.agent; poll, source.read and reply.send through Connectors; admission drops the runtime.connectors need under a boundary and refuses both together (Breaking) and undeclared operations
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:connectors-relay-client
revision: 1
---
## Outcome

`b10x-loom agent serve`, `agent run` and `agent resume` read the host setting
`LOOM_BOUNDARY_CONNECTORS_URL` once, first, beside `LOOM_HTTP_CA_FILE`, and decide
`loom.agent.AdmitConnectorsTransport` from it and the spec:

- set (not empty or blank) and `runtime.connectors` absent: `relay`. Every Connectors call of the
  agent goes through `ConnectorsRelay` (`story:connectors-relay-client`) at that URL: the Slack
  poll's `list`, `get` and `search`, a turn's `source.read` over a `connectors` binding, and
  `reply.send`, in a turn and at a resume after a grant. No process is started for any of them.
- unset, empty or blank and `runtime.connectors` present: `cli`, as today.
- set and `runtime.connectors` present: refused `TwoTransports`; neither set nor present: refused
  `NoTransport`; a value `ConnectorsRelay::new` refuses: refused `NotALoopbackUrl`. Each exits 1
  naming the variable and the reason, never the value, before any model is settled.

The plugin host takes the client it is given: `b10x_loom_plugin::Host` builds a
`loom_connectors::Connectors` (`crates/loom-plugin/src/lib.rs:193-204`), `DataSourceEffects` and
the Slack poll hold one (`effects.rs:302`, `crates/loom-plugin-slack/src/poll.rs:143`), and
`PluginConfig.connectors` becomes optional (a host with configured sources and neither a command
line nor a relay refuses to start, `PluginError::Config`). `SpecPlugin::build` no longer requires
`runtime.connectors` when the transport is `relay` (`crates/loom-agent/src/serve.rs:155-159`).

The controller follows: admission no longer refuses an `AgentServe` resource without
`runtime.connectors` when `spec.boundary.connectors` is declared
(`crates/loom-operator/src/admission.rs:1604-1615`); it refuses `runtime.connectors` beside
`spec.boundary.connectors` (`Malformed` at `spec.agent.runtime.connectors`: one transport per pod);
and it refuses a `connectors` binding of `spec.agent` naming a `list`, `get`, `search` or `write`
operation that `spec.boundary.connectors.operations` does not list
(`BoundaryOperationUndeclared` at that member: the relay would answer 403 for it,
`crates/loom-boundary/src/invoke.rs`).

## Why

Gap 1 of `.engineering/drafts/e6-request.md`. The controller gives the agent container
`LOOM_BOUNDARY_CONNECTORS_URL=http://127.0.0.1:8402` (`crates/loom-operator/src/render.rs:252-257`,
`RELAYS`), and no Loom code outside the operator and a contract test reads it (a search of
`crates/` at `39f4da88`: `crates/loom-cli/tests/boundary_contract.rs:125` only). A host setting, not
a spec member, as `LOOM_HTTP_CA_FILE` and `LOOM_A2A_BEARER` are (AGENTS.md § Agent spec): which
relay the pod has is the cluster's.

## Waits on

- `story:connectors-relay-client`: the client and the `Connectors` enum this story selects.

## ESS first

The first commit changes only `ess/`:

- `ess/domains/agent.yaml`: the stateless command `loom.agent.AdmitConnectorsTransport` (input:
  `url` of `loom.agent.TransportSetting` — `Unset`, `Blank`, `Loopback`, `Other` — and
  `runtime_connectors: Boolean`), its outcomes `cli` and `relay`, its error
  `ConnectorsTransportRefused` with `reason` of the enum `ConnectorsTransportRefusalReason`
  (`TwoTransports`, `NoTransport`, `NotALoopbackUrl`); the comment on `loom.agent.RuntimeSpec`
  (`connectors` is not needed under a relay); scenarios under
  `ess/scenarios/agent-connectors-transport/`, listed in `ess-inputs.yaml`.
- `ess/domains/plugin.yaml`: `loom.plugin.PluginConfig.connectors` becomes
  `Optional<loom.datasource.ConnectorsCliConfig>`.
- `ess/domains/resource.yaml`: `AdmissionRefusalReason` gains `BoundaryOperationUndeclared`, and
  `AdmitLoomAgent` its `external:` outcome; the authored scenario
  `ess/scenarios/resource-admission/a-binding-operation-the-boundary-does-not-list-is-refused.yaml`
  and its resource file.

On it `task drift` fails and the new scenarios are red.

## Acceptance

`crates/loom-agent/tests/connectors_relay.rs` (no network call: an in-process
`b10x_loom_boundary` relay bound on `127.0.0.1:0` with a fixture token the test writes under
`CARGO_TARGET_TMPDIR`, in front of a fake Connectors service on a loopback port that records each
request, the pattern of `crates/loom-boundary/tests/relays.rs`; recorded model responses):

- `an_auto_run_sends_its_reply_through_the_relay`: `agent run` of a spec with `writes.mode: auto`
  and no `runtime.connectors` performs one keyed invoke of the channel binding's `write`; the
  record line names the applied send.
- `the_upstream_sees_only_the_boundarys_token`: every request the fake received carries
  `authorization: Bearer <fixture token>`; none carries `loom-boundary-relay`.
- `no_connectors_program_is_started_under_the_relay`: with `PATH` holding only a fixture
  `connectors` that writes a marker file when run, no marker exists after the run.
- `a_source_read_goes_through_the_relay`: a turn's `source.read` over a `connectors` binding is one
  describe and one unkeyed invoke at the fake.
- `an_approved_send_resumes_through_the_relay`: `writes.mode: approval`, `b10x-loom approve`, then
  `agent resume`: one keyed invoke, with the key the held turn would have sent.
- `a_send_over_max_per_hour_or_outside_the_allow_list_reaches_no_relay`: `AdmitSend` refuses
  before any request; the fake received nothing for it.
- `an_operation_the_boundary_does_not_list_is_refused_at_the_relay`: the relay answers 403 and the
  fake received nothing; the turn records the send refused.
- Each scenario of `ess/scenarios/agent-connectors-transport/` has the test of its name in
  `crates/loom-cli/tests/agent_connectors_transport.rs` (exit 1, the variable and reason named, no
  model settled).
- `crates/loom-operator/tests/admission_reasons.rs` names `BoundaryOperationUndeclared`'s member;
  `a_boundary_beside_runtime_connectors_is_refused` and
  `an_agent_serve_resource_with_a_connectors_boundary_needs_no_runtime_connectors` in
  `crates/loom-operator/tests/`.
- Unchanged: every test of `crates/loom-agent/tests/serve.rs` and `crates/loom-cli/tests/agent_run.rs`
  on the fake `connectors`.
- `task conform`, `task crd-drift` (the CRD is expected unchanged: `spec.agent` is the
  `loom.agent/1` document, no member is added) and `task check` exit 0.

CHANGELOG: `### Added` for the relay transport; `**Breaking:**` a `LoomAgent` declaring both
`spec.boundary.connectors` and `spec.agent.runtime.connectors` is refused `Malformed` at
`spec.agent.runtime.connectors` (admitted through 0.57.x). The chart does not change.

## Files

- `ess/domains/agent.yaml`, `ess/domains/plugin.yaml`, `ess/domains/resource.yaml`,
  `ess/scenarios/agent-connectors-transport/` (new), `ess/scenarios/resource-admission/`,
  `ess-inputs.yaml`, `generated/rust/loom/`
- `crates/loom-agent/src/{serve.rs,run.rs}` and a new `src/transport.rs`
- `crates/loom-plugin/src/{lib.rs,effects.rs}`, `crates/loom-plugin-slack/src/poll.rs`
- `crates/loom-cli/src/` (`agent serve`, `run`, `resume` read the variable),
  `crates/loom-cli/tests/agent_connectors_transport.rs` (new)
- `crates/loom-conformance/src/` (answers `AdmitConnectorsTransport`)
- `crates/loom-operator/src/admission.rs`, `crates/loom-operator/tests/`
- AGENTS.md § Agent spec, § Operator, § Connectors; CHANGELOG.md; `website/docs/reference/` agent
  and operator pages

## Out of scope

The input's shape (`story:send-input-in-the-declared-shape`). A relay for a host outside the
cluster. Reading `LOOM_BOUNDARY_GIT_URL` (takt's, not the agent's). `story:boundary-mcp-tools`.
