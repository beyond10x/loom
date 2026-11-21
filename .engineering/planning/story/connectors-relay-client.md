---
format: aep.planning-md/3
id: story:connectors-relay-client
kind: story
status: implemented
title: ConnectorsRelay reads and performs one governed write over the Connectors wire at a loopback relay URL, holding no credential
summary: GET /v1/describe and POST /v1alpha2/invoke on its own HTTP client over connectors-core v0.62.0 types, no authorization header, for one adapter and connection pair; one URL rule (loopback_url); one result per stage of a write; a write's answer classified from the typed InvokeResponse; the CLI's refusals, key and once-only rule kept; every relay and Connectors answer tested on a loopback fake with real v1alpha2 bodies
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-connectors/Cargo.toml
- confidence: cited
  path: crates/loom-connectors/src/cli.rs
- confidence: cited
  path: crates/loom-connectors/src/cli/write.rs
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: cited
  path: crates/loom-connectors/src/relay.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_relay.rs
- confidence: cited
  path: crates/loom-connectors/tests/security_w20261121w5_u2.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 21
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:03Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T09:18:22Z", actor: "agent:sdc", revision: 15, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w5"}
- {from: "active", to: "implemented", at: "2026-11-21T10:50:27Z", actor: "agent:sdc", revision: 21, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w5"}
---
## Outcome

`b10x-loom-connectors` gains `loom_connectors::relay::ConnectorsRelay`, a client of the Connectors
wire at a loopback relay URL, for one Connectors instance, with the read and write methods
`ConnectorsCli` has, under the same signatures: `sources`, `describe`, `invoke_read`, `read` and
`invoke_write`. It speaks exactly what the boundary's Connectors relay admits
(`crates/loom-boundary`, AGENTS.md § Credential boundary): `GET /v1/describe` (the legacy bare
Descriptor, which Connectors keeps for describe, `contracts/service/compatibility.md` § 2.1 Routes,
`:49` at both `v0.62.0` and `v0.73.0`) and `POST /v1alpha2/invoke`. Requests and answers are
encoded and decoded with `connectors-core`'s public types at the pinned `v0.62.0`
(`v1alpha2::InvokeRequest`, `KeyedInvokeRequest`, `InvokeResponse::decode` and `decode_keyed`,
`Descriptor`) on a `reqwest` client of its own (redirects off, no proxy, connect bound 5 s and
request bound 20 s, as `connectors-client`'s), run on a Tokio runtime the client owns, as
`ConnectorsInvoker` does (`crates/loom-connectors/src/lib.rs:210-226`). `ConnectorsRelay` is
`Clone` and shares one runtime by reference count (an `Arc`, shut down in the background when the
last clone drops, as `ConnectorsInvoker`'s `Drop` does at `lib.rs:228-235`); `over` shares it. It sends no
`authorization` header: the boundary adds its token (`crates/loom-boundary/src/lib.rs:544-569`).

Not `connectors-client` itself, for two reasons read in it at `v0.62.0`: it refuses an empty token
and always sends one (`Endpoint::with_token`, `crates/connectors-client/src/lib.rs:50-55`), and it
turns every answer that is not a Connectors envelope into a `Failure` it raised itself, with no
status (`Failure::local` and `from_error`, `:285-300`), so it cannot tell a refusal the boundary
made locally, when nothing was sent, from `upstream_failed`, when a write may have been. The answer
is read under `connectors-client`'s own correlation rule (`read_v1alpha2`, `:319-330`): the request
id echoed (or null only on an error without `mutation`) and the HTTP status the one the error code
maps to; any other answer is not a valid Connectors answer.

One instance per client. `ConnectorsRelay::new(url, adapter, connection, sources)` builds a client
for the one `adapter` and `connection` pair behind the relay (a boundary has one Connectors
upstream, and the v1alpha2 invoke refuses a `connection` member, § 2.1). The URL rule is one
public function that takes no pair, `relay::loopback_url(value: &str) -> Result<RelayUrl,
RelayUrlRefused>`: it refuses a value that is not `http://127.0.0.1:<port>` with no path, user
information, query or fragment (`RelayUrlRefused`, naming the rule, never the value). `new` calls
it first, and `story:agent-selects-the-connectors-relay`'s `src/transport.rs` calls it to classify
the variable. Each method refuses, before any request, a
source or operation naming another pair, with code `relay-other-instance`, naming the source or
operation and the pair the client was built for: `sources` when a configured source names another
pair, `describe` of another adapter, `invoke_read` and `read` of another pair (`CliError::Refused`),
and `invoke_write` of a `WriteOperation` naming another `adapter` or `connection`
(`WriteError::Refused`). `over(sources)` gives the same client (URL, pair, HTTP client and
runtime) over other sources, as `Host::connectors_over` builds a narrowed client
(`crates/loom-plugin/src/lib.rs:198`); a source of another pair stays refused. `story:agent-selects-the-connectors-relay`'s
`SeveralInstances` surfaces this refusal for an agent spec.

Rules kept from `ConnectorsCli`, by reusing its code (visibility widened, not copied):
`mod write;` (`src/cli.rs:59`) becomes `pub(crate) mod write;`, and `admits`
(`src/cli/write.rs:319`), `Described` (`src/cli.rs:609`) and `text` (`:716`) become
`pub(crate)`; `WRITE_PROFILES` (`src/cli.rs:92`), `idempotency_key`, `CliError` and `WriteError`
(`src/cli.rs:61`, `:103`) are already `pub`. No signature changes and nothing becomes public.
`classified` (`src/cli/write.rs:366-407`) is not reused: it reads the command line's projected
answer (`ok` and `result`, or `error.data`, `:367-375`), and a v1alpha2 `InvokeResponse` has no
`ok`, carries `request_id`, `mutation` and `source_audit` at its top level, and an `error` of only
`code`, `message` and `retry_after_seconds` (connectors `v0.62.0`
`crates/connectors-core/src/v1alpha2.rs:489-503`, `:226-236`); passed in as it arrives, every
relay write would classify `Unknown`.

A relay write's answer is classified from the typed response, in `src/relay.rs`: the function
`write_result(response: &InvokeResponse, key: String) -> WriteResult` reads the decoded
`InvokeResponse` (`decode_keyed`). `classification` is the `MutationObservation`'s
(`v1alpha2.rs:192-201`): `EffectKnowledge::NotAttempted`, `Refused`, `Applied` and `Unknown` become
the `MutationClassification` of the same name, and no `mutation` is `Unknown`. `request_id` is the
response's, or the mutation's `original_request_id` for a replay (`replayed: true`). `body` is
`result` on success. `audit_ref` is the response's top-level `audit_ref`. `source_audit` is always
absent at `v0.62.0`: both rule sets of this binding refuse it (`first_binding_rules` and
`keyed_binding_rules`, "source_audit is not answered on this binding", `v1alpha2.rs:630-631`,
`:691-692`), so the `WriteResult`'s `source_audit` is always `None`. A newer host that sends one
is not answering this binding as `v0.62.0` defines it: `decode_keyed` (`:717-728`) refuses the
body as `upstream_protocol`, and the client answers it as "any other answer" (a write `Unknown`,
not retried; a read, decoded with `decode`, `CliError::Failed` `upstream-protocol`). `code`
is `error.code` as Connectors spells it on the wire (the serde name, as `code_name` reads it at
`src/lib.rs:487-492`; `ErrorCode` is a snake-case enum, `v1alpha2.rs:36-39`) on an error. Values cross from
`serde_json` to `loom::json` as JSON text.

- A read refuses an operation the describe gives the `mutation` profile, code `write-operation`,
  with no invoke; a source with no operation for the kind is `undeclared-kind` with no request at
  all. A read is sent with no `idempotency_key`. Its `ReadResult` is `result` and the response's
  `audit_ref`; a successful read whose answer carries a `mutation`, or whose audit record is not
  `complete` with a non-empty `audit_ref`, is `CliError::Failed`, as `ConnectorsInvoker` reads one
  (`lib.rs:342-370`).
- A write refuses `action-not-admitted` and `input-not-admitted` before any request (the
  comparison `ConnectorsCli::invoke_write` makes with `admits`, `src/cli/write.rs:175-184`), the
  proof path checks `invoke_write` makes before any process (`approval-missing`,
  `approval-not-a-path`, `approval-not-absolute`) before any request, and `not-a-write` when the
  describe gives another profile than `mutation`. It is sent once, keyed with
  `cli::idempotency_key` of the admitted request (the key the command line sends today, so a resume
  after a grant keeps it). The `WriteApproval` is not sent: the v1alpha2 binding refuses an
  `approval` member (§ 2.1, Refused § 4 members), and the host's standing grant is the approval of
  every keyed write there.

What every answer becomes, per request. A read and a write each send at most two requests: the
describe, then the invoke. A boundary answer is a non-2xx whose body is exactly the boundary's
`{"error", "message"}` with one of its fixed codes (`crates/loom-boundary/src/lib.rs:646-656`;
codes `route.rs:38-60`, `lib.rs:385-414`, `operation_refused` at `:472-476`). Codes `not_relayed`,
`path_refused`, `target_refused`, `method_refused`, `operation_refused`, `body_too_large`,
`body_unreadable`, `token_unavailable`, `upstream_unreachable` and `upstream_timeout` are answered
before anything is sent upstream; `upstream_failed` is answered after the request may have left.

| Answer | A read, at its describe or its invoke | A write, at its describe (no invoke follows) | A write, at its invoke |
|---|---|---|---|
| The relay cannot be connected to (nothing sent) | `CliError::Failed` starting `relay-unavailable:` | `WriteError::Failed` starting `relay-unavailable:` | `WriteError::Refused`, code `relay-unavailable` |
| Boundary 400, 403 (`operation_refused`), 404, 405, 413 | `CliError::Refused`, code `relay-refused`, naming the boundary's code | `WriteError::Refused`, code `relay-refused` | `WriteError::Refused`, code `relay-refused` |
| Boundary 503 `token_unavailable`, 502 `upstream_unreachable`, 504 `upstream_timeout` | `CliError::Failed` starting `relay-unavailable:` | `WriteError::Failed` starting `relay-unavailable:` | `WriteError::Refused`, code `relay-unavailable` |
| Boundary 502 `upstream_failed` | `CliError::Failed` starting `relay-unavailable:` | `WriteError::Failed` starting `relay-unavailable:` (a describe changes nothing) | `WriteResult` `Unknown`, not retried |
| A valid Connectors error (401 `unauthorized` from its token review, 403 `approval_required`, 404, 409 `idempotency_conflict`, 429, 5xx) | `CliError::Refused` with Connectors' code, not retried; at a read's invoke `approval_required` and `idempotency_conflict` are unreachable at `v0.62.0`: the unkeyed binding's `first_binding_rules` refuses every code but the 13 base ones and `outcome_unknown` (`v1alpha2.rs:648-651`), so `decode` reads such a body as the last row | `WriteError::Refused` with Connectors' code | `mutation.classification` when present (`refused`, `not_attempted`), else `Unknown`, each with Connectors' code, as `write_result` reads the typed response (above); `unsupported` with `request_id: null` is `WriteError::Failed` naming the missing standing grant (`ConnectorsInvoker`'s remedy text), never resent unkeyed |
| Success | the describe: the invoke follows; the invoke: `ReadResult` (rules above) | the invoke follows, or `not-a-write` | `WriteResult` with the classification; a replay (`replayed: true`) is `applied` with the original request's id as `request_id` |
| Any other answer, an answer lost or unreadable, a timeout after the request left | `CliError::Failed` (`upstream-protocol`) | `WriteError::Failed` (`upstream-protocol`) | `WriteResult` `Unknown`, not retried |

A write's describe column follows `WriteError`'s own split (`src/cli/write.rs:52-59`): a refusal
by the boundary or by Connectors is `Refused`, a describe that could not be made or read is
`Failed`. Its invoke column is `Refused` for every answer that proves nothing was sent upstream,
and a `WriteResult` once it may have been.

## Why

Gap 1 of the request of session `ai-agent-platform` (`.engineering/drafts/e6-request.md`):
`reply.send` goes through `ConnectorsCli::invoke_write` (`crates/loom-agent/src/serve.rs:18-19`),
which starts the program `runtime.connectors.program` (`crates/loom-connectors/src/cli.rs:442`); the
agent image ships only `b10x-loom` (`deploy/images/loom-agent/Dockerfile:20`), and Connectors' CLI
calls no standalone service (connectors `v0.73.0` `docs/running-services.md:108`). Loom speaks the
wire already in `ConnectorsInvoker`, but only behind Commission's `ConnectorInvoker` port with an
endpoint credential the host resolves (`lib.rs:257-271`). No story in the store plans a wire client
for the plugin path: `epic:credential-free-agent-pod` left the client side to takt (its § Scope),
and `story:boundary-mcp-tools` (draft) is MCP tools to a model session.

## Waits on

Nothing in this epic. `story:send-input-in-the-declared-shape` depends on it (`src/lib.rs`,
`src/relay.rs`, `src/cli/write.rs`, `tests/connectors_relay.rs`, `Cargo.lock`, AGENTS.md
§ Connectors), and so does `story:agent-selects-the-connectors-relay` (it selects this client and
uses `over`; `src/lib.rs`, AGENTS.md § Connectors).

## ESS first

No ESS change: the client answers the declared `loom.datasource.ReadResult`, `ReadRefusal`,
`WriteResult` and `WriteRefusal` (`ess/domains/datasource.yaml`) without a new member; its codes are
values of their `code: String`, as `story:connectors-invoker-keyed-writes` added `ConnectorsInvoker`'s
keyed answers without a declaration. `generated/` does not change. The red run is
`crates/loom-connectors/tests/connectors_relay.rs` on a commit that adds only it.

## Acceptance

`crates/loom-connectors/tests/connectors_relay.rs`, against a fake service on a loopback port that
records each request and answers each request as the test scripts it, by method and path (the
precedent is `tests/connectors_invoker.rs`, AGENTS.md § Connectors), no network call. Every test
names the request its scripted answer is for: "at the describe" (`GET /v1/describe`) or "at the
invoke" (`POST /v1alpha2/invoke`, after a describe answered with the operation's descriptor).
Every Connectors answer the fake writes at the invoke is a real v1alpha2 body: an `InvokeResponse`
built with `connectors-core` `v0.62.0`, echoing the request's id, and written with the encoder of
the binding the client reads it under (connectors `v0.62.0` `crates/connectors-core/src/v1alpha2.rs`):

- a read's answer, which the client reads with `decode`, is written with `encode` (`:674`, which
  applies `section5_rules` and `first_binding_rules`, `:629-651`). Those rules hold every read
  answer the tests script: a success, an incomplete audit, a mutation on a read, and the base-code
  errors `unauthorized`, `not_found`, `rate_limited`, `unavailable` and `internal`;
- a write's answer, which the client reads with `decode_keyed`, is written with `encode_keyed`
  (`:732-741`, `keyed_binding_rules`, `:690-712`). Only it builds a replay (`replayed: true`,
  refused by `first_binding_rules` at `:637-639`) and the keyed codes `approval_required` and
  `idempotency_conflict` (`KEYED_BINDING_EXTENDED`, `:745-749`). Every write answer of the tests is
  written with it: applied, not_attempted, refused, unknown (with `outcome_unknown`, as
  `section5_rules` requires at `:611-614`), `unauthorized`, `approval_required`,
  `idempotency_conflict`, a 5xx, `unsupported` with a null request id, and the replay, which carries
  `attempt` beside `original_request_id` (`section5_rules` refuses one without the other, `:605-610`).

The answers built by hand, not by an encoder, are only those whose subject is a body no encoder
writes: the uncorrelated answer (a request id the request did not send), a body with
`source_audit` (refused by both rule sets, `:630-631`, `:691-692`), the lost and unreadable
answers, and the boundary's own bodies. So each row of the table is driven by the body Connectors
sends at `v0.62.0`, not by the command line's projection.

- `a_relay_write_applied_carries_its_classification_audit_and_body`: at the invoke, a success with
  `mutation.classification: applied`, a top-level `audit_ref` with `audit_status: complete`, no
  `source_audit`, and a `result`: the `WriteResult` is `Applied`, with the response's `request_id`,
  `audit_ref`, `source_audit` `None`, and the `result` as `body`.
- `a_relay_write_whose_mutation_is_not_attempted_or_unknown_says_so`: at the invoke, an error body
  with `mutation.classification: not_attempted` and a base code, then one with `unknown` and
  `outcome_unknown`: `NotAttempted`, then `Unknown`, each with Connectors' code.
- `an_answer_carrying_a_source_audit_is_an_upstream_protocol_answer`: at the invoke, a body built
  by hand as an applied success plus a `source_audit`: the write is `Unknown`, not retried (exactly
  one invoke), and the read is `CliError::Failed` `upstream-protocol`, since `decode_keyed` and
  `decode` refuse the member at `v0.62.0`.

- `a_relay_read_describes_then_invokes_once_unkeyed`: one `GET /v1/describe`, one
  `POST /v1alpha2/invoke` with no `idempotency_key`; the `ReadResult` carries the answer's
  `audit_ref`.
- `a_read_of_a_kind_the_source_does_not_declare_sends_no_request`: `undeclared-kind`, zero requests.
- `a_relay_read_of_a_mutation_operation_sends_no_invoke`: `write-operation`, zero invokes.
- `a_read_answer_with_a_mutation_or_an_incomplete_audit_is_failed`: at the invoke; each
  `CliError::Failed`.
- `a_relay_write_is_keyed_with_the_command_lines_key`: the `idempotency_key` equals
  `cli::idempotency_key` of the admitted request.
- `a_relay_write_of_other_input_or_action_sends_nothing`: `input-not-admitted` and
  `action-not-admitted`, zero requests.
- `a_relay_write_with_a_relative_proof_path_sends_nothing`: `approval-not-absolute`, zero requests.
- `a_relay_write_sends_no_approval_member`: the invoke body holds no `approval`.
- `a_relay_write_described_as_a_read_is_not_a_write`: `not-a-write`, zero invokes.
- `a_call_naming_another_instance_sends_nothing`: `sources` of a client configured with a source
  naming another `connection`, `sources` of `over` a source of another adapter, `invoke_read` of
  another connection, `read` of a source of another
  adapter, `describe` of another adapter, and `invoke_write` of a
  `WriteOperation` of another connection: each `relay-other-instance` (`CliError::Refused`, or
  `WriteError::Refused` for the write), zero requests.
- `a_relay_that_cannot_be_reached_is_unavailable`: no listener on the port. The read is
  `CliError::Failed` starting `relay-unavailable:`; the write, whose describe is its first request,
  is `WriteError::Failed` starting `relay-unavailable:`.
- `a_write_whose_relay_goes_away_after_the_describe_is_refused_unavailable`: the fake closes its
  listener before it writes the describe's answer, and writes that answer with
  `connection: close` (the precedent is `tests/connectors_invoker.rs:276`), so the invoke meets no
  listener and no kept-alive connection; the write is `WriteError::Refused` `relay-unavailable`,
  the same for a read is `CliError::Failed` starting `relay-unavailable:`, and the fake recorded
  no invoke for either.
- `a_boundary_refusal_at_the_describe_is_relay_refused_and_no_invoke_is_sent`: at the describe, the
  boundary's 404 `not_relayed`, then 405 `method_refused`, each for a read and for a write: the
  read is `CliError::Refused` `relay-refused`, the write `WriteError::Refused` `relay-refused`;
  exactly one request each.
- `a_boundary_refusal_at_the_invoke_is_relay_refused_and_nothing_is_resent`: at the invoke, the
  boundary's 403 `operation_refused`, then 413 `body_too_large`, then 400 `body_unreadable`, each
  for a read and for a write:
  `relay-refused` as above; exactly one describe and one invoke each.
- `a_boundary_unavailable_answer_at_the_describe_is_relay_unavailable`: at the describe, 503
  `token_unavailable`, 502 `upstream_unreachable`, 504 `upstream_timeout` and 502
  `upstream_failed`: the read is `CliError::Failed` and the write `WriteError::Failed`, each
  starting `relay-unavailable:`; no invoke.
- `a_boundary_unavailable_answer_at_the_invoke_is_relay_unavailable`: at the invoke, 503
  `token_unavailable`, 502 `upstream_unreachable`, 504 `upstream_timeout`: the read is
  `CliError::Failed` starting `relay-unavailable:`, the write `WriteError::Refused`
  `relay-unavailable`; exactly one invoke.
- `an_upstream_failed_write_is_unknown_and_not_resent`: at the invoke, 502 `upstream_failed`;
  `Unknown`, exactly one invoke; the same answer to a read is `CliError::Failed` starting
  `relay-unavailable:`.
- `a_connectors_error_at_the_describe_is_refused_with_its_code`: at the describe, a legacy
  Connectors error (401 `unauthorized`); the read is `CliError::Refused` and the write
  `WriteError::Refused`, each with that code; no invoke.
- `a_connectors_unauthorized_answer_at_the_invoke_is_refused_for_a_read_and_unknown_for_a_write`:
  at the invoke, a valid v1alpha2 401 `unauthorized` envelope, request id null, no `mutation`.
- `a_connectors_error_with_a_refused_mutation_is_refused`: at the invoke, 403 `approval_required`
  is `Unknown` with its code; a 4xx with `mutation.classification: refused` is `Refused`.
- `a_connectors_5xx_at_the_invoke_is_refused_for_a_read_and_unknown_for_a_write`.
- `a_lost_relay_write_answer_is_unknown_and_not_resent`: the fake closes after reading the invoke;
  `Unknown`, exactly one invoke. It also stands for the row's "a timeout after the request left",
  which has the same result in every column. A test of the timeout itself would wait out the
  20 s request bound, so there is none.
- `an_unreadable_describe_is_an_upstream_protocol_failure`: at the describe, a 200 that is not a
  Descriptor; the read is `CliError::Failed` and the write `WriteError::Failed`, each
  `upstream-protocol`; no invoke.
- `an_uncorrelated_answer_is_not_a_connectors_answer`: at the invoke, another request id echoed;
  the read is `Failed`, the write `Unknown`.
- `a_keyed_write_refused_unsupported_names_the_standing_grant_and_is_not_resent_unkeyed`: at the
  invoke.
- `a_replayed_write_is_applied_naming_the_original_attempt`: at the invoke, a success with
  `mutation` `{classification: applied, replayed: true, attempt: {instance, id}, original_request_id:
  <id>}`, written with `encode_keyed`: `Applied`, `request_id` that id.
- `the_relay_client_sends_no_credential`: no recorded request carries `authorization`,
  `proxy-authorization`, `x-api-key` or `cookie`; a fixture token written by the test appears in no
  request. This is the one test of the epic that fails for a client that sends a credential itself:
  behind a boundary, the boundary drops those headers and adds its own token
  (`crates/loom-boundary/src/lib.rs:551-567`), so no test downstream of it can see one.
- `a_relay_url_that_is_not_loopback_is_refused_before_any_request`: `http://10.0.0.1:8402`,
  `https://127.0.0.1:8402`, `http://u:p@127.0.0.1:8402`, `http://127.0.0.1:8402/x`,
  `http://127.0.0.1:8402?q`, `http://127.0.0.1:8402#f`: `relay::loopback_url` and
  `ConnectorsRelay::new` each refuse every one with the same `RelayUrlRefused`, and accept
  `http://127.0.0.1:8402`; no connection reaches the fake.
- `the_relay_module_starts_no_process`: a source scan, as `no_invoke_ever_carries_an_approval_file`
  scans `src/cli.rs` (`tests/connectors_cli.rs:465`): `src/relay.rs`, read from
  `CARGO_MANIFEST_DIR`, holds none of `std::process`, `tokio::process`, `Command` and
  `ConnectorsCli`, and every name it reaches through a `cli::` path (in a `use` group or inline)
  is one of `CliError`, `WriteError`, `idempotency_key`, `WRITE_PROFILES`, `SourceEntities` (the
  type `sources` returns, `src/cli.rs:99`, `:165`), `Described`, `text` and `write::admits`; any
  other `cli::` name fails the test. Those eight start no program
  (`admits` with `same` and `sorted` at `src/cli/write.rs:319-348`; the struct at
  `src/cli.rs:609-613`; `text` at `:716-721`; the rest are types, a constant and a hash). The scan
  reads one file, so it cannot see a program started through another crate or a module it does not
  name. The tests that catch that are behavioural, in `story:agent-selects-the-connectors-relay`:
  `no_connectors_program_is_started_under_the_relay`, `the_poll_under_the_relay_starts_no_process`,
  `agent_serve_polls_and_sends_through_the_relay` and `the_enum_dispatches_to_the_client_it_holds`
  (no marker written), and here every test that asserts the fake's recorded requests.
- Unchanged: every test of `tests/connectors_cli*.rs` and `tests/connectors_invoker.rs`.
- `task check` exits 0.

CHANGELOG: `### Added`, `ConnectorsRelay`. No `**Breaking:**` bullet: `ConnectorsCli` keeps its
paths and signatures. The chart and the CRD do not change.

## As built, wave 2026-11-21-w5

Where the unit's commits (`29e5d199..cbf96e1e`) differ from the text above, the commits are what
holds:

- `Described` and `text` (`src/cli.rs:609`, `:716`) stay private: the relay client does not use
  them. `mod write` and `admits` are `pub(crate)` as written.
- In `src/lib.rs`, beside `pub mod relay`: `code_name` is `pub(crate)` (`:500`), and the
  standing-grant remedy text moved, unchanged, into `pub(crate) fn standing_grant_remedy`
  (`:487-497`), which the invoker and the relay both call. Nothing became public outside
  `loom_connectors::relay`.
- `ConnectorsRelay::new` returns `Result<Self, RelayUrlRefused>` (`src/relay.rs:228-233`). When
  the runtime or the HTTP client cannot start, every call answers `relay-unavailable` and sends
  nothing.
- Four files the list below does not name: `crates/loom-sdk/tests/host_ports.rs` (four `use`
  lines, so the stability page's path check covers the four relay paths),
  `website/data/status.json` (one entry), `README.md` (the crate's row) and
  `crates/loom-connectors/tests/security_w20261121w5_u2.rs` (14 cases of the security review,
  `review-result:connectors-relay-client-security-1`: the URL rule over 66 spellings, no
  redirect followed, no proxy and no credential variable read, the pair check on all five
  methods, 13 answers that are neither the boundary's nor Connectors', the 4 MiB response limit,
  the connect bound, clones and nested runtimes).
- Decided by the implementor where the text is silent, read by the security review: the request
  id is `loom-relay-<32 hex>` and is neither a secret nor an idempotency key; an operation the
  descriptor does not list is `Refused` with `not_found` after one describe and no invoke; a
  read's error that carries a `mutation` is `Failed` with `upstream-protocol`; `WriteResult.code`
  is absent when the answer did not come from Connectors; the descriptor's `instance` and
  `adapter` are not compared with the pair (the boundary's operation allowlist is the guard for
  a write).
- A write answered `token_unavailable`, `upstream_unreachable` or `upstream_timeout` is reported
  as not sent. That rests on the boundary sending those answers before or at connecting
  upstream (`crates/loom-boundary/src/lib.rs:430-431`, `:595-602`), read in the code and not
  held by a test of this crate.

## Files

- `crates/loom-connectors/src/relay.rs` (new), `src/lib.rs` (`pub mod relay`)
- `crates/loom-connectors/src/cli.rs` (`pub(crate) mod write`; `Described` and `text`
  `pub(crate)`), `crates/loom-connectors/src/cli/write.rs` (`admits` `pub(crate)`)
- `crates/loom-connectors/Cargo.toml`: `reqwest = { version = "0.12", default-features = false }`.
  The workspace root declares no `reqwest`; the one declaration is
  `crates/loom-agent/Cargo.toml:22` (`version = "0.12", default-features = false, features =
  ["rustls-tls-webpki-roots"]`). No TLS feature, since the relay is plain `http` on loopback, and
  no `json` or `blocking` feature, since bodies are encoded by `connectors-core` and the client
  runs on its own runtime. In the workspace build, Cargo unifies features, so the one
  `reqwest 0.12.28` in `Cargo.lock` keeps loom-agent's TLS feature.
- `Cargo.lock`
- `crates/loom-connectors/tests/connectors_relay.rs` (new)
- `crates/loom-sdk/src/lib.rs`: no code change. `pub use b10x_loom_connectors as connectors;`
  (`:55`) already exposes `loom_sdk::connectors::relay`; only the `connectors` row of the doc table
  (`:14`) gains `connectors::relay::ConnectorsRelay`.
- `website/docs/reference/sdk-stability.md` (the new path)
- AGENTS.md § Connectors, CHANGELOG.md

## Out of scope

The `Connectors` enum over both clients and selecting one in the agent
(`story:agent-selects-the-connectors-relay`); the input's shape
(`story:send-input-in-the-declared-shape`); TLS to the relay (loopback only); several Connectors
instances behind one relay (one boundary has one Connectors upstream; this client refuses any
other pair, and the agent refuses a spec needing more, `SeveralInstances`). A contract test against
a Connectors host newer than `v0.62.0` is not in this story, and the pin stays at `v0.62.0` (the
requester's deployed host version is unverified, `.engineering/drafts/e6-requester-answer-1.md`).

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w5 from the unit's commits (`git diff --stat 29e5d199 cbf96e1e`: 15 files, 4494 insertions, 12 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-connectors` (package `b10x-loom-connectors`) — cited, 9 of the 15 changed files are under it (manifest, `src/lib.rs`, `src/cli.rs`, `src/cli/write.rs`, new `src/relay.rs`, two new test files)
- **Files (manifests):** `crates/loom-connectors/Cargo.toml` (1 line: `reqwest = { version = "0.12", default-features = false }`) — cited
- **Files:** `Cargo.lock` — cited, 1 line: `reqwest 0.12.28` under `b10x-loom-connectors`
- **Files (source):** `crates/loom-connectors/src/relay.rs` (new, 1299 lines: `ConnectorsRelay`, the URL rule, the request and answer handling over `connectors-core` v0.62.0 types), `crates/loom-connectors/src/lib.rs` (29 lines changed, 21 added and 8 removed: `pub mod relay`, a module-doc paragraph on the relay client, the standing-grant remedy text moved out of `ConnectorsInvoker`'s refusal into `standing_grant_remedy`, `code_name` made `pub(crate)`), `crates/loom-connectors/src/cli.rs` (1 line: `mod write;` becomes `pub(crate) mod write;`), `crates/loom-connectors/src/cli/write.rs` (1 line: `admits` becomes `pub(crate)`), `crates/loom-sdk/src/lib.rs` (1 line, a doc-comment table row: the `connectors` row names `connectors::relay::ConnectorsRelay`; no code change) — cited
- **Files (new tests):** `crates/loom-connectors/tests/connectors_relay.rs` (1708 lines, 34 cases, the story's acceptance; the last commit holds its unreachable port in a bound, never-listening `TcpSocket` for the whole case) and `crates/loom-connectors/tests/security_w20261121w5_u2.rs` (1429 lines, 14 cases, the security review's) — cited
- **Files (existing tests):** `crates/loom-sdk/tests/host_ports.rs` (4 lines: `use` lines for `ConnectorsRelay`, `RelayUrl`, `RelayUrlRefused` and `loopback_url` in the `connectors` module) — cited
- **Files (documents):** `AGENTS.md` (24 lines: the relay client's paragraph in § Connectors), `CHANGELOG.md` (1 line, `### Added`), `README.md` (1 line changed: the `b10x-loom-connectors` crate row names the relay client), `website/docs/reference/sdk-stability.md` (1 line: the table row for the four relay paths), `website/data/status.json` (1 line: the entry "Connectors through the credential boundary's relay") — cited
- **Symbols:** public items added in `loom_connectors::relay`: `ConnectorsRelay` (`Clone`, `Debug`) with `new(url, adapter, connection, sources) -> Result<Self, RelayUrlRefused>`, `over`, `sources`, `describe`, `invoke_read`, `read`, `invoke_write`; `RelayUrl` (`port()`, `Display`); `RelayUrlRefused` (`RULE`, `Display`, `Error`); `loopback_url(&str) -> Result<RelayUrl, RelayUrlRefused>` — cited. Crate-private: `cli::write` (module, was private), `cli::write::admits`, `standing_grant_remedy`, `code_name` (was private). Private in `relay.rs`: `write_result`, `exchange`, `legacy_code_name`. No public symbol changed or removed; `Described` and `text` in `src/cli.rs` stay private (`cli.rs:609`, `:716`)
- **Not predicted and touched:** `crates/loom-connectors/tests/security_w20261121w5_u2.rs`, `crates/loom-sdk/tests/host_ports.rs`, `website/data/status.json`, `README.md`; `AGENTS.md`, `CHANGELOG.md` and `Cargo.lock` are named by the story's Files but not by its typed scope or Scope section
- **Predicted and not touched:** none; all eight typed scope paths are in the diff
- **Review:** `review-result:connectors-relay-client-security-1` — green at `69793e4d` plus the untracked test file (14 cases added, none red, 149 to 163 executed); all eight invariants hold; one note, `unreachable_url` in `tests/connectors_relay.rs` could hand a closed port to another process, fixed in `cbf96e1e`; two facts not settled (the boundary's pre-connect answers walked through code only, the connect-bound mutation reasoned and not run)
