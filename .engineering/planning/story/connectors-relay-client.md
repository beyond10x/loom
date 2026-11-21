---
format: aep.planning-md/3
id: story:connectors-relay-client
kind: story
status: draft
title: ConnectorsRelay reads and performs one governed write over the Connectors wire at a loopback relay URL, holding no credential
summary: GET /v1/describe and POST /v1alpha2/invoke via connectors-client v0.62.0; the CLI's refusals, key and once-only rule kept; placeholder bearer the boundary drops; loopback stub-service tests
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
revision: 1
---
## Outcome

`b10x-loom-connectors` gains `loom_connectors::relay::ConnectorsRelay`, a client of the Connectors
wire at a loopback relay URL, and `loom_connectors::Connectors`, one enum over it and
`ConnectorsCli` with the methods both serve: `sources`, `describe`, `invoke_read`, `read` and
`invoke_write`, each with the signature `ConnectorsCli` has today. The relay client speaks the wire
the boundary's Connectors relay admits (`crates/loom-boundary`, AGENTS.md § Credential boundary):
`GET /v1/describe` and `POST /v1alpha2/invoke`, through `connectors-client` at the tag Loom pins
(`v0.62.0`, `crates/loom-connectors/Cargo.toml:12`), on the invoker's own Tokio runtime pattern
(`ConnectorsInvoker::block_on`, `crates/loom-connectors/src/lib.rs:210-226`).

It keeps every rule `ConnectorsCli` holds today, applied to the describe the service answers:

- A read refuses an operation described with the `mutation` profile, code `write-operation`, and
  sends no invoke; a source with no operation for the kind is `undeclared-kind` with no request.
  A read is sent unkeyed (`Client::invoke_v1alpha2`); its `ReadResult` is the `result` and the
  response's `audit_ref`; a read whose answer carries a `mutation` or an audit record that is not
  `complete` is an error, as `ConnectorsInvoker` reads it (`lib.rs:342-370`).
- A write refuses `action-not-admitted` and `input-not-admitted` before any request (the same
  `admits` comparison, `src/cli/write.rs:319`), `not-a-write` when the describe reports another
  profile than `mutation`, and is sent once, keyed with `cli::idempotency_key` of the admitted
  request (the key the command line sends today, so a resume after a grant keeps it), through
  `Client::invoke_v1alpha2_keyed`. Its `WriteResult` carries Connectors' `mutation.classification`;
  a replay (`mutation.replayed: true`) is `applied` naming the original attempt; an answer lost or
  unreadable after the request left is `Unknown`, never retried; `unsupported` with no request id
  is a `WriteError::Failed` naming the missing standing grant (the remedy text `ConnectorsInvoker`
  gives), and the write is never sent again unkeyed.
- `WriteApproval` is accepted and not sent: the v1alpha2 binding refuses an `approval` member
  (connectors `v0.62.0` `contracts/service/compatibility.md` § 2.1, Refused § 4 members), and the
  host's standing grant is the approval of every keyed write there. The checks on the proof path
  that `invoke_write` makes before any process are made before any request.
- The client holds no credential. `connectors-client` refuses an empty token
  (`Endpoint::with_token`, `crates/connectors-client/src/lib.rs:50-55` at `v0.62.0`), so the
  client sends the fixed text `RELAY_PLACEHOLDER` (`loom-boundary-relay`) as its bearer; the
  boundary drops every `authorization` header and adds its own (`crates/loom-boundary/src/lib.rs:91-96`,
  `:544-569`).
- `ConnectorsRelay::new` takes the URL and refuses, before any request, one that is not
  `http://127.0.0.1:<port>` with no path, user information, query or fragment
  (`RelayUrlRefused`, naming the rule, never the value).

## Why

Gap 1 of the request of session `ai-agent-platform` (`.engineering/drafts/e6-request.md`):
`reply.send` goes through `ConnectorsCli::invoke_write` (`crates/loom-agent/src/serve.rs:18-19`),
which starts the program `runtime.connectors.program` (`crates/loom-connectors/src/cli.rs:442`); the
agent image ships only `b10x-loom` (`deploy/images/loom-agent/Dockerfile:20`), and Connectors' CLI
calls no standalone service (connectors `v0.73.0` `docs/running-services.md:108`). Loom already
speaks the wire in `ConnectorsInvoker`, but only behind Commission's `ConnectorInvoker` port with an
endpoint credential the host resolves (`lib.rs:257-271`); the plugin's effect port and the Slack poll
hold a `ConnectorsCli` by value (`crates/loom-plugin/src/effects.rs:302`,
`crates/loom-plugin-slack/src/poll.rs:143`). No story in the store plans a wire client for the
plugin path: `epic:credential-free-agent-pod` left the client side to takt (its § Scope), and
`story:boundary-mcp-tools` (draft) is MCP tools to a model session, not this.

## ESS first

No new noun: the client answers the declared `loom.datasource.ReadResult`, `ReadRefusal`,
`WriteResult` and `WriteRefusal` (`ess/domains/datasource.yaml`). The first commit changes only
the comments on `ReadRefusal` and `WriteRefusal` in `ess/domains/datasource.yaml`, naming the
relay's own codes (`RelayUrlRefused`, `relay-unreachable`); on it
`crates/loom-connectors/tests/connectors_relay.rs` does not compile. If the implementor finds a code
needs a declared member, that is an ESS change first and the story returns to the coordinator.

## Acceptance

`crates/loom-connectors/tests/connectors_relay.rs`, against a fake Connectors service on a loopback
port that records each request (the precedent is `tests/connectors_invoker.rs`, AGENTS.md
§ Connectors), no network call:

- `a_relay_read_describes_then_invokes_once_unkeyed`: one `GET /v1/describe`, one
  `POST /v1alpha2/invoke` with no `idempotency_key`; the `ReadResult` carries the answer's
  `audit_ref`.
- `a_relay_read_of_a_mutation_operation_sends_no_invoke`: code `write-operation`, zero invokes.
- `a_relay_write_is_keyed_with_the_command_lines_key`: the request's `idempotency_key` equals
  `cli::idempotency_key` of the admitted request.
- `a_relay_write_of_other_input_or_action_sends_nothing`: `input-not-admitted` and
  `action-not-admitted`, zero requests.
- `a_relay_write_described_as_a_read_is_not_a_write`: `not-a-write`, zero invokes.
- `a_lost_relay_write_answer_is_unknown_and_not_resent`: the fake closes after reading the
  invoke; `Unknown`, exactly one invoke.
- `a_keyed_write_refused_unsupported_names_the_standing_grant_and_is_not_resent_unkeyed`.
- `a_replayed_write_is_applied_naming_the_original_attempt`.
- `the_relay_client_sends_no_credential`: every recorded request's `authorization` is exactly
  `Bearer loom-boundary-relay`; a fixture token written by the test appears in no request.
- `a_relay_url_that_is_not_loopback_is_refused_before_any_request`: `http://10.0.0.1:8402`,
  `https://127.0.0.1:8402`, `http://u:p@127.0.0.1:8402`, `http://127.0.0.1:8402/x`,
  `http://127.0.0.1:8402?q`; no connection reaches the fake.
- `the_write_path_starts_no_process`: with `PATH` empty and a `ConnectorsCliConfig.program` that
  does not exist, every test above passes.
- Unchanged: every test of `tests/connectors_cli*.rs` and `tests/connectors_invoker.rs`.
- `task check` exits 0.

No `**Breaking:**` bullet: `ConnectorsCli` keeps its paths and signatures; `Connectors` and
`ConnectorsRelay` are added (`### Added`). The chart and the CRD do not change.

## Files

- `crates/loom-connectors/src/relay.rs` (new), `src/lib.rs` (`Connectors`, `pub mod relay`)
- `crates/loom-connectors/tests/connectors_relay.rs` (new)
- `crates/loom-sdk/src/lib.rs` (re-export), `website/docs/reference/sdk-stability.md` (new paths)
- `ess/domains/datasource.yaml` (comments only)
- AGENTS.md § Connectors, CHANGELOG.md

## Out of scope

Selecting the client in the agent (`story:agent-selects-the-connectors-relay`); the input's shape
(`story:send-input-in-the-declared-shape`); TLS to the relay (loopback only); a token-less
constructor in `connectors-client` (an upstream change; the placeholder stands until one exists).
