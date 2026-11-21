---
format: aep.planning-md/3
id: story:send-input-in-the-declared-shape
kind: story
status: draft
title: A governed write sends its admitted arguments in the shape the operation's described input schema declares
summary: 'ShapeWriteInput in loom.datasource: flat, or under body for a catalog write with a request body, else input-shape-unknown; both transports'
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:connectors-relay-client
revision: 1
---
## Outcome

A governed write sends its admitted arguments in the shape the operation's described input schema
declares, on both transports (`ConnectorsCli::invoke_write` and `ConnectorsRelay`'s write). After
the describe and before the invoke, the write decides `loom.datasource.ShapeWriteInput` from the
described `input_schema`:

- `flat` when the schema's top-level `properties` declare every admitted argument name: the input
  is the admitted arguments, as today;
- `body` when the top-level `properties` declare `body`, an object schema, and none of the admitted
  argument names: the input is `{"body": <admitted arguments>}`;
- otherwise refused `input-shape-unknown`, naming the operation and which names the schema does
  not declare; no invoke.

The model-facing schema of `reply.send` (`crates/loom-plugin/src/turn.rs:800-825`) and the
admitted arguments do not change; `input-not-admitted` still compares the caller's input with the
admitted arguments (`src/cli/write.rs:319`) before the shape is applied, and the shape is a pure
function of the arguments and the describe, so no input other than the admitted arguments can be
sent. The idempotency key does not change (it derives from the request ids, not the input).

## Why

Gap 1, last item, of `.engineering/drafts/e6-request.md`, read rather than inferred. `reply.send`
builds `{channel, text, thread_ts?}` flat (`crates/loom-plugin/src/turn.rs:806`, sent as is,
`crates/loom-plugin/src/effects.rs:719`). Connectors' catalog adapter puts a write's JSON request
body under the input member `body` and keeps path and query parameters at the top level (connectors
`v0.73.0` `adapters/catalog/src/lib.rs:2877-2896`), checks the closed body on `input.body`
(`:2055-2057`) and validates the whole input against that schema (`:2059`). Slack's
`chat.postMessage` declares `body_keys` `channel`, `text`, `thread_ts`, `mrkdwn`, `unfurl_links`
(`adapters/catalog/providers/slack/operations.json:56-61`), so its input is `{body: {channel,
text}}` and Loom's flat input fails validation. The same holds for every catalog write that has a
request body, through the command line as well: both run the same adapter. Reads are not affected:
the model composes a read's input against the described input schema it is shown
(`turn.rs:834`).

## Waits on

- `story:connectors-relay-client`: both writes live in `crates/loom-connectors`
  (`src/cli/write.rs`, `src/relay.rs`), and the relay's write is that story's.

## ESS first

The first commit changes only `ess/domains/datasource.yaml`: the enum
`loom.datasource.WriteInputShape` (`flat`, `body`), the stateless command
`loom.datasource.ShapeWriteInput` (input: `declares_all_arguments: Boolean`,
`declares_object_body: Boolean`, `declares_any_argument: Boolean`), its outcomes `flat`, `body` and
the error `WriteInputShapeRefused` (`code` `input-shape-unknown`), with authored scenarios under
`ess/scenarios/write-input-shape/` listed in `ess-inputs.yaml`. On it `task drift` fails and the
scenarios are red; `task conform` answers them through `loom_connectors::shape_write_input` once
implemented.

## Acceptance

- `crates/loom-connectors/tests/connectors_cli_write.rs`:
  `a_write_whose_operation_declares_a_body_sends_the_arguments_under_body` (the fake `connectors`
  describes the schema of Slack's `chat.postMessage` at `v0.73.0`; the stdin it logs is
  `{"body":{"channel":…,"text":…}}`), `a_write_whose_operation_declares_the_arguments_sends_them_flat`,
  `a_write_whose_schema_declares_neither_is_refused_input_shape_unknown` (no invoke logged).
- `crates/loom-connectors/tests/connectors_relay.rs`: the same three against the fake service.
- `a_shaped_write_still_refuses_input_other_than_the_admitted_arguments`: input differing from the
  admitted arguments is `input-not-admitted` before any describe.
- `the_shaped_write_keeps_its_idempotency_key`.
- `crates/loom-connectors/tests/support_triage_route.rs` and `evidence.rs` tests unchanged (the
  evidence reads the admitted arguments, not the wire input).
- Each scenario of `ess/scenarios/write-input-shape/` passes under `task conform`; `task check`
  exits 0.

CHANGELOG: `### Fixed`, a governed write sends a catalog operation's body under `body`. No
`**Breaking:**` bullet: no stable path or signature changes, and a write that worked keeps its flat
input. The chart and the CRD do not change.

## Files

- `ess/domains/datasource.yaml`, `ess/scenarios/write-input-shape/` (new), `ess-inputs.yaml`,
  `generated/rust/loom/`
- `crates/loom-connectors/src/{cli.rs,cli/write.rs,relay.rs}`, a new `src/shape.rs`
- `crates/loom-connectors/tests/{connectors_cli_write.rs,connectors_relay.rs}`,
  `tests/fixtures/connectors/fake-connectors` (a describe answer with a body schema)
- `crates/loom-conformance/src/`
- AGENTS.md § Connectors, CHANGELOG.md

## Out of scope

A declared mapping from action arguments to other names (`mrkdwn`, `unfurl_links` stay unsent);
fixed body values (the catalog adds them itself, `lib.rs:2083-2085`); reads.
