---
format: aep.planning-md/3
id: story:agent-selects-the-connectors-relay
kind: story
status: proposed
title: agent serve, run and resume use the Connectors relay when LOOM_BOUNDARY_CONNECTORS_URL is set, and start no connectors program
summary: Connectors enum carried by Host::with_relay and SpecPlugin::host to run_plugin_on, run_item, Answering and resume; AdmitConnectorsTransport in loom.agent (TwoTransports, NoTransport, NotALoopbackUrl, SeveralInstances); poll, source.read and reply.send through the relay with the epic's end-to-end check; a down relay is Unavailable in the effect port and the Slack poll; admission drops the runtime.connectors need under a boundary and refuses both together (Breaking)
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:connectors-relay-client
- depends_on: story:send-input-in-the-declared-shape
- depends_on: story:boundary-admits-declared-http-egress
scope:
- confidence: cited
  path: crates/loom-agent/Cargo.toml
- confidence: cited
  path: crates/loom-agent/src/run.rs
- confidence: cited
  path: crates/loom-agent/src/serve.rs
- confidence: cited
  path: crates/loom-agent/src/serve/a2a.rs
- confidence: cited
  path: crates/loom-agent/src/transport.rs
- confidence: cited
  path: crates/loom-agent/tests/connectors_relay.rs
- confidence: cited
  path: crates/loom-cli/src/a2a.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/agent_connectors_transport.rs
- confidence: cited
  path: crates/loom-cli/tests/agent_serve.rs
- confidence: cited
  path: crates/loom-conformance/src/agent.rs
- confidence: cited
  path: crates/loom-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/tests/adversary_d243_u1.rs
- confidence: cited
  path: crates/loom-operator/tests/render.rs
- confidence: cited
  path: crates/loom-plugin-slack/src/config.rs
- confidence: cited
  path: crates/loom-plugin-slack/src/poll.rs
- confidence: cited
  path: crates/loom-plugin-slack/tests/poll_relay.rs
- confidence: cited
  path: crates/loom-plugin-slack/tests/support/mod.rs
- confidence: cited
  path: crates/loom-plugin/src/effects.rs
- confidence: cited
  path: crates/loom-plugin/src/lib.rs
- confidence: cited
  path: crates/loom-plugin/tests/connectors_enum.rs
- confidence: cited
  path: crates/loom-plugin/tests/held_record.rs
- confidence: cited
  path: crates/loom-plugin/tests/support/mod.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: ess/domains/plugin.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/agent-connectors-transport/
- confidence: cited
  path: ess/scenarios/resource-admission/
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:03Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
---
## Outcome

`loom_connectors::Connectors` is one enum over `ConnectorsCli` and `ConnectorsRelay`
(`story:connectors-relay-client`) with the methods both serve, `sources`, `describe`,
`invoke_read`, `read` and `invoke_write`, each dispatching to the client it holds, and a third
value, `Unset`, for a host given neither: each of its methods answers `CliError::Failed` (or
`WriteError::Failed`) starting `no-connectors-transport:`, with no request and no process.
`DataSourceEffects` and the Slack poll hold a `Connectors` (`effects.rs:302,321`,
`crates/loom-plugin-slack/src/poll.rs:143`), and `PluginConfig.connectors` becomes optional.

How the relay reaches every host. Each of `run_plugin` (`crates/loom-plugin/src/lib.rs:361-368`),
`Answering::new` (`crates/loom-agent/src/serve/a2a.rs:318-320`) and the host `agent resume` hands
`SpecPlugin::resume` (`crates/loom-cli/src/main.rs:492`) builds its `Host` from `PluginConfig`
alone, which holds no relay URL. The carrier is the `Host` and the `SpecPlugin` that builds it:

- `Host` (`lib.rs:155-158`) gains `with_relay(relay: ConnectorsRelay)`, beside `with_environment`
  (`:173`). `connectors()` and `connectors_over(sources)` (`:193-204`) return a `Connectors`: the
  relay over those sources (`ConnectorsRelay::over`) when one was given; otherwise
  `Connectors::Cli` of `config.connectors` when it is set; otherwise `Connectors::Unset`. Their
  callers (`turn.rs:346`, `poll.rs:29`, `serve.rs:476` and the tests that pass `host.connectors()`
  to `DataSourceEffects::new`) compile unchanged. `run_plugin_on` and `run_item` refuse,
  `PluginError::Config`, before any hook, a host that is `Unset` while its configuration names a
  source.
- `SpecPlugin` holds the transport it was built with (`b10x_loom_agent::transport::Transport`,
  `Cli` or `Relay(ConnectorsRelay)`), and `SpecPlugin::host(&self) -> Host<'_>` returns
  `Host::new(&self.config)`, with `.with_relay(relay.clone())` under `Relay`.
  `SpecPlugin::new(spec, state)` and `for_run(spec, state)` keep their signatures and build `Cli`
  (every existing caller unchanged); `new_over(spec, state, transport)` and
  `for_run_over(spec, state, transport)` take the selected one.
- The call sites take `plugin.host()`: `agent serve` calls `run_plugin_on(&plugin, &plugin.host(),
  state, &stop)` in place of `run_plugin(&plugin, plugin.config(), …)` (`main.rs:320`); `agent run`
  (`main.rs:459`) and `agent resume` (`main.rs:492`) replace `Host::new(plugin.config())` with
  `plugin.host()`; `Answering::new` becomes `Self::with_host(plugin, plugin.host())` (`a2a.rs:319`),
  so `crates/loom-cli/src/a2a.rs:149` is unchanged. `SpecPlugin::resume(&self, host, …)`
  (`serve.rs:424`) keeps its signature and uses the host it is handed.
- `agent serve`'s A2A listener builds a plugin of its own, `SpecPlugin::new(agent.spec,
  agent.state)` (`crates/loom-cli/src/a2a.rs:136`), which under the relay, with no
  `runtime.connectors`, would fail at `serve.rs:155-159` and stop the host. It takes the same
  carrier: `a2a::Agent` (`a2a.rs:90-102`) gains `transport: Transport`, which `main.rs` fills with
  a clone of the selected transport where it builds the two `a2a::Agent` values (`main.rs:296`,
  `:310`; `Transport` is `Clone`, since `ConnectorsRelay` is), and `a2a.rs:136` calls
  `SpecPlugin::new_over(agent.spec, agent.state, agent.transport)`. Its `Answering::new` then
  takes `plugin.host()` as above, so under the relay it builds no `Cli`, and with zero
  `connectors` bindings its host is `Unset`. This is the smallest seam: the listener keeps building
  its own plugin (its classifier and turn models are the listener's, `a2a.rs:136-140`), and only
  the transport is passed in.
- `agent resume` on an `Unset` host: the host is `Unset` only for a spec with zero `connectors`
  bindings, which has no channel and so holds no send. `SpecPlugin::resume` answers
  `ResumeFailure::NotHeld` at `read_held` (`serve.rs:427`) before it touches the host. A held
  send there (none can be written) would meet `WriteError::Failed` `no-connectors-transport:`
  and never be sent.
- The Slack plugin's own host (`crates/loom-plugin-slack/src/lib.rs:141`) stays on the command
  line: its document still requires `connectors` (`config.rs:342`), now built as `Some(…)`
  (`config.rs:341`).

Connectors down. The effect port reads a read that failed `relay-unavailable:` as Connectors down,
`PluginError::Unavailable`, as it reads a timed-out program: a constant
`RELAY_UNAVAILABLE = "relay-unavailable:"` beside `NOT_GRANTED` and `TIMED_OUT` (`effects.rs:84-89`)
and an arm beside the `TIMED_OUT` one in the read's match (`effects.rs:462-479`). The Slack poll has
its own mapping, `failed` (`crates/loom-plugin-slack/src/poll.rs:260-269`), which gains the same arm
(`CliError::Failed` starting `RELAY_UNAVAILABLE` → `PluginError::Unavailable`), importing the
constant beside the other two (`poll.rs:3`), so a poll whose relay is down is unavailable, not a
poll failure.

`b10x-loom agent serve`, `agent run` and `agent resume` read the host setting
`LOOM_BOUNDARY_CONNECTORS_URL` once and decide `loom.agent.AdmitConnectorsTransport` from it and
the spec, before any model is settled:

- `agent serve` and `agent run`: right after `LOOM_HTTP_CA_FILE` (`main.rs:262-264`, `:443-445`),
  before `served_models` (`:266`, `:446`).
- `agent resume`, which reads no `LOOM_HTTP_CA_FILE` and settles no model (`main.rs:470-500`):
  right after the spec is read (`served_spec`, `:471`), before the state directory is checked
  (`existing_state`, `:479`), the plugin is built (`:482`) or the state directory is held
  (`StateDir::open`, `:488`). "Before any model is settled" means, for resume, before anything of
  the state directory is touched.

The decision:

- set (not empty or blank), `runtime.connectors` absent and the spec's `connectors` bindings naming
  at most one `adapter` and `connection` pair: `relay`. Every Connectors call of the agent goes
  through `ConnectorsRelay` at that URL, built for that pair: the Slack poll's `list`, `get` and
  `search`, a turn's `source.read` over a `connectors` binding, and `reply.send`, in a turn and at a
  resume after a grant. No process is started for any of them. With zero `connectors` bindings
  (`instances: 0`, which `AdmitConnectorsTransport` already answers `relay`) there is no pair and
  no client is built: `Transport::Relay` holds `Option<ConnectorsRelay>`, `None` here, and
  `SpecPlugin::host()` adds no relay, so the host's `connectors()` is `Unset`. No hook reaches it:
  such a spec names no Connectors source, and its channel binding, if any, must be a `connectors`
  binding (`crates/loom-agent/src/serve.rs:827-840`), so a spec with a channel has a pair.
- unset, empty or blank and `runtime.connectors` present: `cli`, as today.
- refused, each exiting 1 naming the variable and the reason, never the value: set and
  `runtime.connectors` present, `TwoTransports`; neither, `NoTransport`; a value
  `relay::loopback_url` refuses, `NotALoopbackUrl`; set, and the bindings naming two or more
  `adapter` and `connection` pairs, `SeveralInstances`.

`TransportSetting::Loopback` is exactly a value `relay::loopback_url` admits
(`http://127.0.0.1:<port>` with no path, user information, query or fragment;
`story:connectors-relay-client`); every other value that is not blank is `Other`.
`src/transport.rs` classifies the value by calling `relay::loopback_url`, which takes no pair, and
reading its `RelayUrlRefused` as `Other`; `ConnectorsRelay::new` calls the same function. So the
ESS input, the selection and the client hold one rule, and classifying needs no binding.

`SeveralInstances` surfaces the client's own refusal. `ConnectorsRelay` serves one pair and refuses
any other with `relay-other-instance` before any request (`story:connectors-relay-client`); the
agent finds the second pair in the spec before building the client and exits 1 with
`SeveralInstances`, naming the bindings of each pair and the client's rule (`relay-other-instance`:
a relay client serves one adapter and connection), so the refusal an SDK host meets at a call is
the one the agent reports at start. Why one pair: `ConnectorsCli` addresses an instance per
binding, `operations describe --adapter <alias>` naming the `connector` and `instance` the alias
selects and `invoke <connector>/<instance> --connection <c>` (AGENTS.md § Connectors; `src/cli.rs`),
so two bindings may reach two instances or two credentials. A boundary has one Connectors upstream
(`render.rs:252-257`, one `--connectors-upstream`), `GET /v1/describe` names one `instance`
(`connectors-core` `v0.62.0` `Descriptor`, `lib.rs:103-110`), and the v1alpha2 invoke refuses a
`connection` member (connectors `contracts/service/compatibility.md` § 2.1).

`SpecPlugin::build` no longer requires `runtime.connectors` when the transport is `relay`
(`crates/loom-agent/src/serve.rs:155-159`). The controller follows: admission no longer refuses an
`AgentServe` resource without `runtime.connectors` when `spec.boundary.connectors` is declared
(`crates/loom-operator/src/admission.rs:1604-1615`), and it refuses `runtime.connectors` beside
`spec.boundary.connectors` (`Malformed` at `spec.agent.runtime.connectors`: one transport per pod).
That refusal sits in step 5 (`fn boundary`, called at `admission.rs:1692`), after its egress loop,
so every egress and upstream refusal of step 5, and `NoModelRoute` in step 4, still answers first.

## Why

Gap 1 of `.engineering/drafts/e6-request.md`. The controller gives the agent container
`LOOM_BOUNDARY_CONNECTORS_URL=http://127.0.0.1:8402` (`crates/loom-operator/src/render.rs:252-257`,
`RELAYS`), and no Loom code outside the operator and a contract test reads it (a search of `crates/`
at `39f4da88`: `crates/loom-cli/tests/boundary_contract.rs:125` only). A host setting, not a spec
member, as `LOOM_HTTP_CA_FILE` and `LOOM_A2A_BEARER` are (AGENTS.md § Agent spec): which relay the
pod has is the cluster's.

## Waits on

- `story:connectors-relay-client`: the client this story selects, its one-pair rule and `over`;
  AGENTS.md § Connectors.
- `story:send-input-in-the-declared-shape`: the relay write this story's tests send is shaped there;
  both add to `ess/ess-inputs.yaml`, the command match in
  `crates/loom-conformance/src/lib.rs:245-256` and `crates/loom-connectors/src/lib.rs`; this story
  reads its fixture `describe-write-body.json`, and its unchanged guards install the fixture
  `crates/loom-connectors/tests/fixtures/connectors/fake-connectors` (read at
  `crates/loom-cli/tests/agent_run.rs:488`, `agent_serve.rs:596`, `plugin.rs:370`,
  `crates/loom-plugin/tests/support/mod.rs:84`), which that story may edit; AGENTS.md § Connectors.
- `story:boundary-admits-declared-http-egress`: both edit `crates/loom-operator/src/admission.rs`,
  `crates/loom-operator/tests/render.rs`, `tests/adversary_d243_u1.rs`,
  `ess/scenarios/resource-admission/` and `ess/ess-inputs.yaml`; this story's operator assertions
  start from the ones that story leaves.

## ESS first

The first commit changes only `ess/`:

- `ess/domains/agent.yaml`: the stateless command `loom.agent.AdmitConnectorsTransport` (input:
  `url` of `loom.agent.TransportSetting` — `Unset`, `Blank`, `Loopback`, `Other`, `Loopback` as
  defined above —, `runtime_connectors: Boolean` and `instances: Integer`, the distinct `adapter`
  and `connection` pairs of the spec's `connectors` bindings), its outcomes `cli` and `relay`, its
  error `ConnectorsTransportRefused` with `reason` of the enum `ConnectorsTransportRefusalReason`
  (`TwoTransports`, `NoTransport`, `NotALoopbackUrl`, `SeveralInstances`); the comment on
  `loom.agent.RuntimeSpec` (`connectors` is not needed under a relay); scenarios under
  `ess/scenarios/agent-connectors-transport/`, listed in `ess/ess-inputs.yaml`:
  - `a-loopback-url-without-runtime-connectors-selects-the-relay.yaml` (`relay`)
  - `a-loopback-url-with-no-connectors-binding-selects-the-relay.yaml` (`relay`, `instances: 0`)
  - `no-url-with-runtime-connectors-selects-the-command-line.yaml` (`cli`)
  - `a-blank-url-with-runtime-connectors-selects-the-command-line.yaml` (`cli`)
  - `a-url-beside-runtime-connectors-is-refused.yaml` (`TwoTransports`)
  - `neither-url-nor-runtime-connectors-is-refused.yaml` (`NoTransport`)
  - `a-url-that-is-not-loopback-is-refused.yaml` (`NotALoopbackUrl`)
  - `bindings-of-two-instances-under-a-url-are-refused.yaml` (`SeveralInstances`)
- `ess/domains/plugin.yaml`: `loom.plugin.PluginConfig.connectors` becomes
  `Optional<loom.datasource.ConnectorsCliConfig>`.
- `ess/scenarios/resource-admission/a-boundary-beside-runtime-connectors-is-refused.yaml` and its
  resource file `resources/boundary-beside-runtime-connectors.yaml` (`Malformed` at
  `spec.agent.runtime.connectors`), listed in `ess/ess-inputs.yaml`.

`generated/rust/loom/` changes with it; on that commit `task drift` fails and the new scenarios are
red. No `AdmissionRefusalReason` is added.

## Acceptance

No test in this story starts a model or reaches the network. "The marker fixture" is a fixture
`connectors` that writes a marker file when run, on a `PATH` of its own: in-process tests give it to
the host as `Host::with_environment([("PATH", <marker directory>)])`, never a process-wide
`set_var`; binary tests set the child's environment. With no `runtime.connectors` there is no
configured program, so the marker guards against a fallback to a `connectors` found on `PATH`.

`crates/loom-agent/tests/connectors_relay.rs` (an in-process `b10x_loom_boundary` relay bound on
`127.0.0.1:0` with a fixture token the test writes under `CARGO_TARGET_TMPDIR`, in front of a fake
Connectors service on a loopback port that records each request, the pattern of
`crates/loom-boundary/tests/relays.rs`; recorded model responses). The fake service's describe
answers the channel binding's `write` operation with
`crates/loom-connectors/tests/fixtures/connectors/describe-write-body.json` (Slack's
`chat.postMessage` at connectors `v0.73.0`: a `body` closed over `channel`, `text`, `thread_ts`,
`mrkdwn`, `unfurl_links`, no parameter, `additionalProperties: false`;
`story:send-input-in-the-declared-shape` adds it), and its reads with the schemas of
`describe-{list,get,search}.json` in the same directory.

- `an_auto_run_sends_its_reply_through_the_relay_in_the_declared_shape`: `agent run` of a spec with
  `writes.mode: auto` and no `runtime.connectors` performs exactly one keyed invoke of the channel
  binding's `write`, whose `input` is `{"body": {"channel": <the first allowed channel>, "text":
  <the proposal>}}`; the record line names the applied send. This is the epic's end-to-end check.
- `every_request_reaches_the_upstream_through_the_boundary`: every request the fake received
  carries `authorization: Bearer <fixture token>`, the boundary's, so each went through the relay.
  It cannot fail for a client that sends a credential of its own: the boundary drops the agent's
  `authorization`, `proxy-authorization`, `x-api-key` and `cookie` and adds its token
  (`crates/loom-boundary/src/lib.rs:551-567`). The test that can is `story:connectors-relay-client`'s
  `the_relay_client_sends_no_credential`, which records the client's requests with no boundary in
  front.
- `no_connectors_program_is_started_under_the_relay`: with the marker fixture, no marker exists
  after the run.
- `a_source_read_goes_through_the_relay`: a turn's `source.read` over a `connectors` binding is one
  describe and one unkeyed invoke at the fake, and no marker.
- `an_approved_send_resumes_through_the_relay`: `writes.mode: approval`, `b10x-loom approve`, then
  `agent resume`: one keyed invoke, with the key the held turn would have sent; no marker.
- `a_send_over_max_per_hour_or_outside_the_allow_list_reaches_no_relay`: `AdmitSend` refuses
  before any request; the fake received nothing for it.
- `an_operation_the_boundary_does_not_list_is_refused_at_the_relay`: the relay answers 403
  `operation_refused` at the invoke, the fake received no invoke, and the turn records the send
  refused (`relay-refused`).
- `a_relay_that_cannot_be_reached_makes_a_read_unavailable`: no listener; the turn ends
  `PluginError::Unavailable`.

`crates/loom-plugin-slack/tests/poll_relay.rs` (new; the fake service as above, answering the
poll's `list`, `get` and `search` with the answers `tests/poll.rs` uses):
`the_poll_lists_gets_and_searches_through_the_relay` (one describe and one invoke per read, items as
`tests/poll.rs` expects), `the_poll_under_the_relay_starts_no_process` (the marker fixture; no
marker) and `a_poll_whose_relay_is_down_is_unavailable` (no listener on the relay port: the poll is
`PluginError::Unavailable`, not `PluginError::Poll`).

`crates/loom-cli/tests/agent_serve.rs`: `agent_serve_polls_and_sends_through_the_relay` (the binary,
`--once`, `LOOM_BOUNDARY_CONNECTORS_URL` naming the relay, `writes.mode: approval`; the poll's reads
reach the fake, the held send is written, no marker exists), and two A2A tests, each on the
pattern of an existing one, with a fake Connectors service on the loopback URL, no
`runtime.connectors`, the marker fixture on the child's `PATH` and recorded model responses from
the test's model server (`serve`, `agent_serve.rs:842`):

- `agent_serve_answers_a2a_through_the_relay` (as `agent_serve_answers_a2a_beside_the_poll`,
  `:1063`, with the URL set): stderr holds `A2A listening on 127.0.0.1:`, an A2A message whose
  recorded turn calls `source.read` over the connectors binding is answered, the fake received one
  `GET /v1/describe` and one `POST /v1alpha2/invoke` for it, and no marker exists.
- `agent_serve_answers_a2a_with_no_connectors_binding_under_the_url` (as
  `agent_serve_answers_a2a_without_a_channel`, `:1280`, with its `docs` binding an `http` binding on
  loopback and the URL set, so the spec has zero `connectors` bindings and selects `relay` with no
  client): the host starts (stderr holds `A2A listening on 127.0.0.1:`), an A2A message is
  answered, the fake Connectors service received no connection, and no marker exists.

`crates/loom-agent/tests/connectors_relay.rs` also holds
`a_spec_plugin_over_no_relay_hands_an_unset_host`: `SpecPlugin::new_over(spec, state,
Transport::Relay(None))` for a spec with zero `connectors` bindings and no `runtime.connectors`
builds, and its `host().connectors()` is `Connectors::Unset`.

`crates/loom-cli/tests/agent_connectors_transport.rs` (new): each scenario of
`ess/scenarios/agent-connectors-transport/` has the test of its name, which runs the binary once
for each of `agent serve`, `agent run` and `agent resume`. Each run is set up so that it fails at
the stage after the transport: for `serve` and `run`, `--catalog` names a file that does not exist;
for `resume`, `--state` names a directory that does not exist. A fake service listens on the
loopback URL when the scenario sets one, and the marker fixture is on the child's `PATH`. What each
run asserts:

- a refusal scenario (`TwoTransports`, `NoTransport`, `NotALoopbackUrl`, `SeveralInstances`): exit
  1, and the message names `LOOM_BOUNDARY_CONNECTORS_URL` and the reason, not the value, and not the
  catalog or the state directory (so the transport answered first). The `SeveralInstances` run
  (two bindings of two adapters) also names both bindings and `relay-other-instance`;
- a scenario that selects (`a-loopback-url-without-runtime-connectors-selects-the-relay`,
  `a-loopback-url-with-no-connectors-binding-selects-the-relay`,
  `no-url-with-runtime-connectors-selects-the-command-line`,
  `a-blank-url-with-runtime-connectors-selects-the-command-line`): exit 1 with the catalog's
  message (`serve`, `run`) or the state directory's (`resume`), which names neither the variable
  nor a transport reason, so selection passed. The blank scenario runs twice, with the value `""`
  and with `"  "`: a blank value read as set would be `TwoTransports` here;
- every run: the fake received no connection and no marker exists, since nothing past the
  transport ran.

What each selection then does is checked where it runs: the relay path by
`agent_serve_polls_and_sends_through_the_relay` and `tests/connectors_relay.rs`, the command line
by the unchanged `agent_serve.rs`, `agent_run.rs` and `plugin.rs` tests on the fake `connectors`.

`crates/loom-plugin/tests/connectors_enum.rs` (new): `the_enum_dispatches_to_the_client_it_holds`
(a `Connectors::Cli` read logs an argv at the fake `connectors`; a `Connectors::Relay` read reaches
the fake service and starts no process), `a_host_given_a_relay_hands_it_to_every_hook`
(`Host::new(&config).with_relay(relay)` with `config.connectors` unset: `connectors()` and
`connectors_over(..)` are `Connectors::Relay`), and, for `Unset`:

- `unset_answers_no_connectors_transport_with_no_request_and_no_process`: on
  `Host::new(&config).with_environment([("PATH", <marker directory>)])` with `config.connectors`
  unset and no relay, `connectors()` is `Connectors::Unset`, and its `sources` (one configured
  source), `describe`, `invoke_read`, `read` and `invoke_write` each answer `CliError::Failed` (or
  `WriteError::Failed`) starting `no-connectors-transport:`; a fake service listening on a loopback
  port recorded no connection, and no marker exists.
- `run_plugin_on_refuses_an_unset_host`: `run_plugin_on` (the entry `agent serve` takes,
  `main.rs:320`) on that host, its configuration naming a source, is `PluginError::Config`, and the
  test plugin's hooks were called zero times.
- `run_item_refuses_an_unset_host`: the same for `run_item` (the entry of `agent run`,
  `main.rs:459-462`).

Operator, in `crates/loom-operator/tests/render.rs`: `a_boundary_beside_runtime_connectors_is_refused`
(`Malformed` at `spec.agent.runtime.connectors`) and
`an_agent_serve_resource_with_a_connectors_boundary_needs_no_runtime_connectors` (rendered).
Then the existing tests:

- Each `render.rs` test that reaches a Connectors boundary through `credential_free_serve` or
  `credential_free_serve_routed` (listed below) either renders through a call that panics on any
  refusal or asserts its own refusal with `refusal` (`render.rs:136-141`, which panics on a render);
  `render.rs` holds no test that returns early on a refusal (a search for
  `Refused(_)) => return|continue` at `61fd500b` finds only `adversary_d243_u1.rs:294`, `:329`,
  `:403`). Each passes with the helpers changed.
- `adversary_d243_u1.rs:282`, `an_egress_namespace_entry_does_not_open_an_upstream_namespace_on_another_port`:
  after `story:boundary-admits-declared-http-egress` it asserts the refusal is
  `BoundaryEgressNamespace` at that entry's `namespace`; this story leaves that assertion and it
  passes (the egress refusal comes before the both-transports one in step 5).
- `adversary_d243_u1.rs:319`, `default_routes_beside_a_boundary_without_a_model_relay_name_a_reachable_endpoint`:
  today returns on any refusal (`:329`). It now asserts the refusal is `NoModelRoute` at
  `spec.boundary.model` (step 4, `admission.rs:1655-1665`, which a Connectors and git boundary with
  no model and `routes: {}` meets), so it fails if the resource is refused for another reason, the
  both-transports one included. It cannot reach `Ok(rendered)`: that resource is refused by design.
- `adversary_d243_u1.rs:372`, `every_admitted_boundary_is_a_command_line_the_relay_accepts`, builds
  `Command` resources (`command`, `:97`), which have no `spec.agent`; it is unchanged.
- Unchanged: every test of `crates/loom-agent/tests/serve.rs`, `crates/loom-cli/tests/agent_run.rs`,
  `agent_serve.rs` and `plugin.rs`. The test bodies of `crates/loom-plugin-slack/tests/poll.rs`,
  `crates/loom-plugin/tests/held_record.rs` and the tests built on `crates/loom-plugin/tests/support`
  are unchanged; their `PluginConfig` literals (Files) set `connectors: Some(…)`.
- `task conform`, `task crd-drift` (the CRD is expected unchanged: `spec.agent` is the `loom.agent/1`
  document, no member is added) and `task check` exit 0.

CHANGELOG: `### Added`, `Connectors` and the relay transport; `**Breaking:**` a `LoomAgent`
declaring both `spec.boundary.connectors` and `spec.agent.runtime.connectors` is refused `Malformed`
at `spec.agent.runtime.connectors` (admitted through 0.57.x). The chart does not change.

## What the refusal makes fail, and what each becomes

- `examples/agents/operator.yaml:40-42` (`runtime.connectors`): unchanged. It declares no boundary,
  so the refusal does not apply; the golden `tests/golden/platform-operator.yaml` built from it is
  unchanged.
- `crates/loom-operator/tests/render.rs`: `credential_free_agent()` (`:1834-1857`) wraps that
  example and keeps its `runtime.connectors`; its line-count assertion
  (`agent.lines().count() + 4 == example().lines().count()`, `:1848-1852`) is unchanged, because
  the line is dropped after it returns. `credential_free_serve` and `credential_free_serve_routed`
  (`:1860-1868`) put a boundary after it, with `connectors` in the tests at `:1035-1043`,
  `:1967-1970`, `:2248-2250`, `:2294-2306`, `:2341` (`a_boundary_upstream_the_controller_does_not_allow_is_refused`,
  `boundary(false, true, false)`), `:2437-2439`, `:2791`, `:2814`, `:3104` and `:3126`. Both
  helpers drop the example's `runtime.connectors` line when `rest` declares
  `spec.boundary.connectors`, so each resource is admitted, or refused for its own reason, as
  before; with a boundary without `connectors` the line stays (an `AgentServe` resource still needs
  a command line there). `a_namespace_egress_entry_beside_a_boundary_is_refused` (`:2586`) wraps
  `credential_free_agent()` directly; its refusals are egress refusals of step 5, which come before
  the both-transports refusal, so it is as `story:boundary-admits-declared-http-egress` leaves it.
- `crates/loom-operator/tests/adversary_d243_u1.rs`: its own `credential_free_agent` (`:55-68`) is
  called only at `:284` and `:321`, both beside a boundary with `connectors`; it drops the
  `runtime.connectors` line always.
- `tests/golden/credential-free.yaml`: unchanged. It is a `Command` resource with no `spec.agent`
  (`credential_free_command`, `render.rs:1822-1831`); its line 22, `connectors.toml: ''`, is
  `spec.connectorsConfig`, not `runtime.connectors`.
- `tests/golden/credential-free-http-egress.yaml` (`story:boundary-admits-declared-http-egress`):
  unchanged. Its resource has a model-only boundary and keeps `runtime.connectors`, so the helper
  change does not apply, and its test passes as that story leaves it.

## Files

- `ess/domains/agent.yaml`, `ess/domains/plugin.yaml`, `ess/scenarios/agent-connectors-transport/`
  (new, the eight scenarios above), `ess/scenarios/resource-admission/` (one scenario and its
  resource file), `ess/ess-inputs.yaml`, `generated/rust/loom/`
- `crates/loom-connectors/src/lib.rs` (`Connectors`), `crates/loom-sdk/src/lib.rs` (the doc table
  row), `website/docs/reference/sdk-stability.md`
- `crates/loom-agent/src/{serve.rs,run.rs,serve/a2a.rs}` and a new `src/transport.rs`;
  `crates/loom-agent/tests/connectors_relay.rs` (new); `crates/loom-agent/Cargo.toml`:
  dev-dependencies `b10x-loom-boundary = { path = "../loom-boundary" }` and what the
  `crates/loom-boundary/tests/relays.rs` pattern uses there (`hyper` with `server` and `http1`,
  `hyper-util` with `tokio`, `http-body-util`, `tokio` with `macros`, `net`, `rt-multi-thread`),
  no cycle (`crates/loom-boundary/Cargo.toml` depends on no `b10x-loom` crate)
- `crates/loom-plugin/src/{lib.rs,effects.rs}`, `crates/loom-plugin/tests/connectors_enum.rs` (new),
  `crates/loom-plugin/tests/held_record.rs` (`:14`) and `tests/support/mod.rs` (`:134`)
- `crates/loom-plugin-slack/src/{poll.rs,config.rs}` (`config.rs:341`),
  `crates/loom-plugin-slack/tests/poll_relay.rs` (new), `crates/loom-plugin-slack/tests/support/mod.rs`
  (`:161`)
- `crates/loom-agent/src/serve.rs:185` (its `PluginConfig` literal; listed above)
- `crates/loom-cli/src/main.rs` (`agent serve`, `run`, `resume` read the variable and take
  `plugin.host()`; the two `a2a::Agent` values get the transport), `crates/loom-cli/src/a2a.rs`
  (`Agent.transport`; `SpecPlugin::new_over` at `:136`), `crates/loom-cli/src/lib.rs` (the help text of the three commands: host
  settings and exit-1 causes, `:423-485`), `crates/loom-cli/tests/agent_connectors_transport.rs`
  (new), `crates/loom-cli/tests/agent_serve.rs`
- `crates/loom-conformance/src/lib.rs` (the command match) and its agent module
- `crates/loom-connectors/tests/fixtures/connectors/` (read: `fake-connectors`, `describe-*.json`)
- `crates/loom-operator/src/admission.rs`, `crates/loom-operator/tests/render.rs` (helpers and the
  two new tests), `tests/adversary_d243_u1.rs`
- `website/docs/reference/cli.md` (the `agent serve`, `run` and `resume` sections, beside
  `LOOM_HTTP_CA_FILE` at `:308` and the exit-1 lists at `:328`, `:387`),
  `docs/design/loom-agent-crd-deployment.md:327` (the rule that `agent serve` and the controller
  refuse a `runtime` without `connectors`, reversed under a Connectors boundary),
  `website/docs/reference/operator-cli.md` only if it states that rule; AGENTS.md § Agent spec,
  § Operator, § Connectors; CHANGELOG.md

## Out of scope

The input's shape (`story:send-input-in-the-declared-shape`). A relay for a host outside the
cluster. Several Connectors instances behind one boundary. Reading `LOOM_BOUNDARY_GIT_URL` (takt's,
not the agent's). The relay for the Slack plugin's own binary. An admission refusal of a binding
operation the boundary does not list: the relay's 403 refuses it at run time
(`an_operation_the_boundary_does_not_list_is_refused_at_the_relay`). A write whose relay is down is
not `PluginError::Unavailable`: it is refused or failed as `story:connectors-relay-client` answers
it. `story:boundary-mcp-tools`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-agent` (new `src/transport.rs`, `src/serve.rs:150-159`), `crates/loom-connectors/src/lib.rs` (the `Connectors` enum), `crates/loom-plugin` and `crates/loom-cli` (selection and wiring) — cited, the story's Files section. No single crate dominates: the code spans five crates and the operator.
- **Files, ESS:** `ess/domains/agent.yaml` (new command, `RuntimeSpec` comment at `:365-371`), `ess/domains/plugin.yaml:106-107`, `ess/scenarios/agent-connectors-transport/` (new), `ess/scenarios/resource-admission/` (one scenario and resource), `ess/ess-inputs.yaml` — cited
- **Files, agent:** `crates/loom-agent/src/serve.rs:155-159`, `crates/loom-agent/src/run.rs`, `crates/loom-agent/src/transport.rs` (new), `crates/loom-agent/tests/connectors_relay.rs` (new) — cited
- **Files, plugin:** `crates/loom-plugin/src/lib.rs:193-204`, `crates/loom-plugin/src/effects.rs:302,321,461-478`, `crates/loom-plugin/tests/connectors_enum.rs` (new), `crates/loom-plugin-slack/src/poll.rs:143`, `crates/loom-plugin-slack/tests/poll_relay.rs` (new) — cited
- **Files, CLI and conformance:** `crates/loom-cli/tests/agent_connectors_transport.rs` (new), `crates/loom-cli/tests/agent_serve.rs`, `crates/loom-conformance/src/lib.rs:245-256`, `crates/loom-conformance/src/agent.rs` — cited
- **Files, SDK:** `crates/loom-connectors/src/lib.rs`, `crates/loom-sdk/src/lib.rs`, `website/docs/reference/sdk-stability.md` — cited
- **Files, operator:** `crates/loom-operator/src/admission.rs:1604-1615`, `crates/loom-operator/tests/render.rs` (helpers `:1834-1868`), `crates/loom-operator/tests/adversary_d243_u1.rs` (helper `:55-68`) — cited; the two new admission tests are in `tests/render.rs` (story Files)
- **Symbols:** `Connectors`, `ConnectorsCli`, `ConnectorsRelay` (`story:connectors-relay-client`), `loom.agent.AdmitConnectorsTransport`, `TransportSetting`, `ConnectorsTransportRefusalReason`, `PluginConfig.connectors`, `Host::connectors` and `connectors_over`, `DataSourceEffects.connectors`, `SpecPlugin::build`, `LOOM_BOUNDARY_CONNECTORS_URL` — cited
- **Files, CLI:** `crates/loom-cli/src/main.rs` — cited, story Files. `LOOM_HTTP_CA_FILE` is read in `agent_serve` (`:263`) and `agent_run` (`:444`). `agent_resume_command` (`:470`) reads no environment and settles no model.
- **Files, CLI help:** `crates/loom-cli/src/lib.rs:423-485` — cited, story Files. The help text lists the host settings and the exit-1 causes.
- **Files, hosts:** `crates/loom-agent/src/serve/a2a.rs:319` (`Answering::new` builds `Host::new(&plugin.config)`) — cited, story Files; `crates/loom-plugin-slack/src/lib.rs:141` (`run_plugin(self, &self.config.plugin, …)`) — cited as unchanged (it stays on the command line).
- **Files, `PluginConfig` fallout:** `crates/loom-agent/src/serve.rs:185`, `crates/loom-plugin-slack/src/config.rs:341-358` (requires `connectors`), `crates/loom-plugin-slack/tests/support/mod.rs:161`, `crates/loom-plugin/tests/held_record.rs:14`, `crates/loom-plugin/tests/support/mod.rs:134` — cited, story Files. Each builds a `PluginConfig`, so the `Optional` change makes them fail to compile until they set `Some(…)`.
- **Files, build:** `crates/loom-agent/Cargo.toml` — cited, story Files. `loom-agent` has no dev-dependency on `b10x-loom-boundary`, and the in-process relay test needs one.
- **Documents:** AGENTS.md § Agent spec, § Operator, § Connectors (house rule); `website/docs/reference/cli.md`; `docs/design/loom-agent-crd-deployment.md:327` — cited, story Files. There is no agent page under `website/docs/reference/`.
- **Confidence:** medium — the story names nearly every file and line, and I confirmed most of them. The `Host`/`run_plugin` plumbing, the CLI file and the `PluginConfig` fallout are inferred.
- **Would collide with**, with the `depends_on` order read from the graph:
  - `story:boundary-admits-declared-http-egress` on `admission.rs`, `tests/render.rs`, `resource-admission/` and `ess/ess-inputs.yaml`. Ordered: `depends_on` edge.
  - `story:send-input-in-the-declared-shape` on `ess/ess-inputs.yaml`, `loom-conformance/src/lib.rs`, the `fake-connectors` fixture and `loom-connectors/src/lib.rs`. Ordered: `depends_on` edge.
  - `story:connectors-relay-client` on `loom-sdk/src/lib.rs`, `sdk-stability.md` and `loom-connectors/src/lib.rs`. Ordered: `depends_on` edge.
  - `story:credential-free-workspace-isolation` on `admission.rs` and `tests/render.rs`. No path either way. Its body says "no edge; the second to land rebases".
  - `story:command-home-survives-pod-replacement` and `story:secret-files-several-secrets` on `admission.rs`. No path either way.
  - `story:agent-project-kubernetes`, `story:agent-spec-admits-takt-member`, `story:agent-spec-constraints-in-crd` (proposed) and `story:egress-by-host` on `ess/domains/agent.yaml`. No path either way. The first one only depends on the delivered `agent-serve`.
  - `story:tree-run-command` (proposed) and `story:revalidation-membership-conformance` on `loom-conformance/src/lib.rs`. No path either way. `tree-run-command` touches it only if `tree.yaml` declares a command.
  - `story:tree-run-command`, `story:autonomy-grant-table`, `story:decision-request-entity`, `story:engine-configuration-document`, `story:engineering-protocols-vendor-command` and `story:hash-chained-run-log` on `ess/ess-inputs.yaml`. No path either way. These are line appends.
  - `story:child-launches-through-confinement-adapters` (`loom-operator/tests`) and `story:protocol-argument-schema` (`loom-plugin/src/effects.rs`). No path either way. Both are incidental mentions.
  - `story:dw-incident-workflow` on the `fake-connectors` fixture. Its `depends_on` chain (`dw-read-fanout`, `dw-typed-step-outputs`, `dw-bugfix-baseline`) does not reach this story.
  - `story:agent-project-kubernetes`, `story:tree-run-command`, `story:secrets-subcommand` and `story:agent-node-run-records` name `crates/loom-cli/src/{main,agent}.rs`. I did not check which functions they edit. No path either way.
  - The `sdk-stability.md` re-export line is shared with about ten unordered stories; none is substantive.
- **Safety fact:** the CRD does not change because `spec.agent` is the `loom.agent/1` document and no member is added — story § Acceptance, unproven (level 1). The reverse risk is a compile break: `PluginConfig` is built in three places that the story's Files section omits (see defect 2).
- **Contradicted by code:** see defects 1-6.
