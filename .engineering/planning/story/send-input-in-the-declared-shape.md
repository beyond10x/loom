---
format: aep.planning-md/3
id: story:send-input-in-the-declared-shape
kind: story
status: proposed
title: A governed write sends its admitted arguments in the shape the operation's described input schema declares
summary: '''ShapeWriteInput in loom.datasource: flat, body or split (every top-level property but body at the top, the rest under body), else input-shape-unknown (an undeclared argument, a required body left empty, a schema or body that is not an object) or input-shape-ambiguous; CLI and relay writes; ConnectorsInvoker out of scope'''
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:connectors-relay-client
- depends_on: story:boundary-admits-declared-http-egress
scope:
- confidence: cited
  path: crates/loom-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-conformance/src/shape.rs
- confidence: cited
  path: crates/loom-connectors/src/cli.rs
- confidence: cited
  path: crates/loom-connectors/src/cli/write.rs
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: cited
  path: crates/loom-connectors/src/relay.rs
- confidence: cited
  path: crates/loom-connectors/src/shape.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_cli_write.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_relay.rs
- confidence: cited
  path: crates/loom-connectors/tests/fixtures/connectors
- confidence: cited
  path: crates/loom-connectors/tests/fixtures/connectors/fake-connectors
- confidence: cited
  path: ess/domains/datasource.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/write-input-shape/
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:03Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
---
## Outcome

A governed write of the plugin path sends its admitted arguments in the shape the operation's
described input schema declares, on both of its transports: `ConnectorsCli::invoke_write` and
`ConnectorsRelay`'s write (`story:connectors-relay-client`). After the describe and before the
invoke, the write decides `loom.datasource.ShapeWriteInput` from the described `input_schema` and
the admitted argument names:

- whether the schema is an object schema (`type: object` with a `properties` object);
- T, every top-level property name other than `body`: the operation's path and query parameters
  and any guard reference the catalog adds beside them (connectors `v0.73.0`
  `adapters/catalog/src/lib.rs:2897-2915`);
- its `body`: absent, open (an object schema with no `properties`), closed (an object schema with
  `properties`, whose names are B), or other (a `body` that is not an object schema);
- whether `body` is in the top-level `required`.

A schema that is not an object schema, and a `body` that is other, are refused
`input-shape-unknown` naming the operation, before any argument is placed. Otherwise each admitted
argument goes, in this order: to the top level when it is in T and not in B; under `body` when it is
in B and not in T, or when the body is open and it is not in T; an argument in both T and B is
refused `input-shape-ambiguous`; one that fits nowhere is refused `input-shape-unknown`; each naming
the operation and the argument. A body the schema requires with no argument under it is refused
`input-shape-unknown` naming the operation and `body`, and no argument, since none is at fault.
The outcome is `flat` (nothing under `body`: the input is the admitted
arguments, as today), `body` (nothing at the top: `{"body": <arguments>}`) or `split`
(`{<top-level arguments>, "body": {<the rest>}}`). A refusal sends no invoke.

The command sees property names only, not their types. A body closed over a nested pointer
(`/a/b`) is rendered by the catalog as a body property `a` holding a closed object (connectors
`v0.73.0` `adapters/catalog/src/closed.rs:503-515`), so `a` is in B and an argument `a` goes to
`body.a` with its value unchanged. Whether that value is the object the schema wants is Connectors'
check: its validation of the whole input (`lib.rs:2059`) refuses a value of another type, and the
write answers as any Connectors refusal of the invoke does. This command does not refuse it.

The model-facing schema of `reply.send` (`crates/loom-plugin/src/turn.rs:800-825`) and the admitted
arguments do not change; `input-not-admitted` still compares the caller's input with the admitted
arguments (`src/cli/write.rs:175-184`, with `admits` at `:319`) before the shape is applied, and the
shape moves arguments without changing a value, so no input other than the admitted arguments can be
sent. The idempotency key derives from the request ids, not the input, and does not change.

Commission's write path, `ConnectorsInvoker` (`crates/loom-connectors/src/lib.rs:244-332`), is not
shaped: it sends `request.data().arguments` as the input (`:272`), which are the arguments of the
protocol action an author binds to the operation against its described input, not a shape Loom
composes; `reply.send`'s flat shape is Loom's own (`turn.rs:806`). Shaping it would change what
Commission admits for a binding, which no part of the request asks for.

## Why

Gap 1, last item, of `.engineering/drafts/e6-request.md`, read rather than inferred. `reply.send`
builds `{channel, text, thread_ts?}` flat (`crates/loom-plugin/src/turn.rs:806`, sent as is,
`crates/loom-plugin/src/effects.rs:719`). Connectors' catalog adapter declares an operation's input
as one top-level property per path or query parameter and per guard reference, `body` beside them
when the operation takes a request body, closed by its `body_keys` or open, and
`additionalProperties: false` at the top (`declare`, connectors `v0.73.0`
`adapters/catalog/src/lib.rs:2831-2941`, the body at `:2877-2896`, guard references at
`:2897-2915`; the same code at the pinned `v0.62.0`, `:2627-2731`, the body at `:2673-2692`). It
checks the closed body on `input.body` (`v0.73.0` `:2055-2057`) and validates the whole input
(`:2059`). Slack's `chat.postMessage` declares `body_keys` `channel`, `text`, `thread_ts`, `mrkdwn`,
`unfurl_links` and no parameter (`adapters/catalog/providers/slack/operations.json:56-61`), so its
input is `{body: {channel, text}}` and Loom's flat input fails validation, through the command line
too: both run the same adapter. Reads are not affected: the model composes a read's input against
the described input schema it is shown (`turn.rs:834`).

## Waits on

- `story:connectors-relay-client`: both writes live in `crates/loom-connectors` (`src/relay.rs`,
  `src/cli/write.rs`), both add a module to `src/lib.rs`, this story adds to its
  `tests/connectors_relay.rs`, both change `Cargo.lock` and AGENTS.md § Connectors.
- `story:boundary-admits-declared-http-egress`: both add scenario lines to `ess/ess-inputs.yaml`
  and regenerate the plan files under `generated/rust/loom/`.

`story:agent-selects-the-connectors-relay` depends on this story: it adds to the same
`ess/ess-inputs.yaml`, the command match in `crates/loom-conformance/src/lib.rs:245-256`,
`crates/loom-connectors/src/lib.rs` (the `Connectors` enum), the fixture
`crates/loom-connectors/tests/fixtures/connectors/fake-connectors` its guards install, and AGENTS.md
§ Connectors.

## ESS first

The first commit changes only `ess/`: `ess/domains/datasource.yaml` gains the enum
`loom.datasource.BodyDeclaration` (`absent`, `open`, `closed`, `other`), the stateless command
`loom.datasource.ShapeWriteInput` (input: `object: Boolean`, `top_level: List<String>`,
`body: BodyDeclaration`, `body_keys: List<String>`, `body_required: Boolean`,
`arguments: List<String>`), its outcomes `flat`, `body` and `split` (each naming the arguments at
the top and under `body`) and the error `WriteInputShapeRefused` (`code` `input-shape-unknown` or
`input-shape-ambiguous`, `argument: Optional<String>`, absent for a refusal of the schema or its
`body` and for a required body left empty);
authored scenarios under `ess/scenarios/write-input-shape/`, listed in `ess/ess-inputs.yaml`:

- `a-closed-body-takes-the-arguments-under-body.yaml` (`body`)
- `a-parameter-and-a-closed-body-split-the-arguments.yaml` (`split`)
- `an-open-body-takes-every-argument-not-at-the-top.yaml` (`body`)
- `arguments-declared-at-the-top-are-sent-flat.yaml` (`flat`)
- `an-argument-declared-at-the-top-and-in-the-body-is-ambiguous.yaml`
- `an-argument-the-schema-does-not-declare-is-unknown.yaml`
- `a-required-body-with-no-argument-under-it-is-unknown.yaml`
- `a-schema-that-is-not-an-object-is-unknown.yaml`
- `a-body-that-is-not-an-object-schema-is-unknown.yaml`

`generated/rust/loom/` changes with it; on that commit `task drift` fails and the scenarios are red.
`task conform` answers them through `loom_connectors::shape_write_input` once implemented.

## Acceptance

Fixtures: describe answers for the fake `connectors` and the fake service, beside
`tests/fixtures/connectors/describe-write.json` and installed as the existing write tests install
it, each a catalog-shaped `input_schema` (`additionalProperties: false`):

- `describe-write-body.json`: Slack's `chat.postMessage` as `v0.73.0` declares it (`body` closed
  over `channel`, `text`, `thread_ts`, `mrkdwn`, `unfurl_links`, no parameter);
- `describe-write-split.json`: a path parameter `channel` and a body closed over `text`,
  `thread_ts`;
- `describe-write-open.json`: no parameter and an open `body`;
- `describe-write-ambiguous.json`: a parameter `text` and a body closed over `text`;
- `describe-write-required-body.json`: a path parameter `channel` and a required body closed over
  `text`, `thread_ts`;
- `describe-write-not-an-object.json`: an `input_schema` of `type: array` that still declares
  `properties` `channel` and `text`, so a skipped object-schema check would place both flat;
- `describe-write-body-not-an-object.json`: a parameter `channel` and a `body` of `type: string`;
- and the flat `describe-write.json` already there.

Cases, each with the arguments `{channel, text}` unless named:

1. `a_write_whose_operation_declares_a_body_sends_the_arguments_under_body`: `body` fixture;
   `{"body":{"channel":…,"text":…}}`.
2. `a_write_with_a_parameter_and_a_body_sends_each_where_declared`: `split` fixture;
   `{"channel":…,"body":{"text":…}}`.
3. `a_write_to_an_open_body_sends_every_argument_under_it`: `open` fixture.
4. `a_write_whose_operation_declares_the_arguments_sends_them_flat`: `describe-write.json`.
5. `an_argument_declared_twice_is_refused_input_shape_ambiguous`: `ambiguous` fixture.
6. `a_write_whose_schema_declares_neither_is_refused_input_shape_unknown`: an argument in neither T
   nor a closed B.
7. `a_required_body_with_no_argument_under_it_is_refused_input_shape_unknown`: `required-body`
   fixture with the arguments `{channel}` only; the same fixture with `{channel, text}` is `split`,
   so the test fails if `body_required` is ignored. The refusal has no `argument` (the member is
   absent) and its message names `body`.
8. `a_schema_that_is_not_an_object_is_refused_input_shape_unknown`: `not-an-object` fixture,
   arguments `{channel, text}`, both declared in its `properties`, so with the check skipped the
   write would be `flat`; the refusal has no `argument`.
9. `a_body_that_is_not_an_object_schema_is_refused_input_shape_unknown`: `body-not-an-object`
   fixture with the arguments `{channel}` only, which would be `flat` if the body were ignored;
   the refusal has no `argument`.

Each refusal sends no invoke and names the operation; cases 5 and 6 also name the argument
(`text`, and the undeclared one), and cases 7 to 9 have none.

- `crates/loom-connectors/tests/connectors_cli_write.rs`: cases 1 to 9 against the fake
  `connectors` (the input asserted is the stdin the fake logs; no invoke logged for a refusal).
- `crates/loom-connectors/tests/connectors_relay.rs`: cases 1 to 9 against the fake service (the
  input asserted is the recorded invoke body's `input`; no invoke recorded for a refusal), and
  `the_shaped_relay_write_sends_no_credential`: no recorded request carries `authorization`,
  `proxy-authorization`, `x-api-key` or `cookie`.
- `the_shape_module_starts_no_process`, in `connectors_relay.rs`: a source scan, as
  `the_relay_module_starts_no_process` (`story:connectors-relay-client`) and
  `no_invoke_ever_carries_an_approval_file` (`tests/connectors_cli.rs:465`) are: `src/shape.rs`,
  read from `CARGO_MANIFEST_DIR`, holds none of `std::process`, `tokio::process`, `Command` and
  `ConnectorsCli`, and no `cli::` path at all (the shaper is a pure function of names, called from
  `src/cli/write.rs` and `src/relay.rs`, never calling into them). `the_relay_module_starts_no_process`
  keeps passing with the shaper called from `src/relay.rs` (`shape::` is not a `cli::` path). A
  program reached through another crate is the behavioural tests' to catch
  (`story:connectors-relay-client` § Acceptance names them).
- `the_shaped_write_keeps_its_idempotency_key`, on both transports, flat, `body` and `split`: the
  key sent equals `cli::idempotency_key` of the admitted request.
- `a_shaped_write_still_refuses_input_other_than_the_admitted_arguments`: input differing from the
  admitted arguments is `input-not-admitted` before any describe.
- Unchanged: `crates/loom-connectors/tests/connectors_invoker.rs`, `support_triage_route.rs` and
  the evidence tests (the evidence reads the admitted arguments, not the wire input).
- Each of the nine scenarios of `ess/scenarios/write-input-shape/` passes under `task conform`;
  `task check` exits 0.

CHANGELOG: `### Fixed`, a governed write sends a catalog operation's body under `body`. No
`**Breaking:**` bullet: no stable path or signature changes, and a write whose schema declares its
arguments flat keeps its input. The chart and the CRD do not change.

## Files

- `ess/domains/datasource.yaml`, `ess/scenarios/write-input-shape/` (new, nine scenarios),
  `ess/ess-inputs.yaml`, `generated/rust/loom/`
- `crates/loom-connectors/src/{cli.rs,cli/write.rs,relay.rs}`, a new `src/shape.rs`,
  `src/lib.rs` (`pub mod shape;` and `pub use shape::shape_write_input;` beside `pub mod cli;`
  and `pub mod evidence;` at `:70-71`)
- `crates/loom-connectors/tests/{connectors_cli_write.rs,connectors_relay.rs}`,
  `tests/fixtures/connectors/fake-connectors` (only if installing the new answers needs it) and the
  seven new describe answers above
- `crates/loom-conformance/src/lib.rs` (the command match), a new module for the command,
  `crates/loom-conformance/Cargo.toml` (`b10x-loom-connectors`, inferred: it depends on
  `b10x-loom-agent` only today), `Cargo.lock`
- AGENTS.md § Connectors, CHANGELOG.md

## Out of scope

Commission's `ConnectorsInvoker` (above). A declared mapping from action arguments to other names
(`mrkdwn`, `unfurl_links` stay unsent). Fixed body values (the catalog adds them itself,
`lib.rs:2083-2085`). Placing an argument below the top of `body`, and checking a value's type: a
body closed over `/a/b` takes the argument `a` at `body.a`, and a value of the wrong type is
Connectors' refusal (above). Reads.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree, `file:line` where it is code) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-connectors` (the governed-write path: `src/cli/write.rs`, the new `src/shape.rs`, the relay write that `story:connectors-relay-client` adds) plus the ESS source `ess/domains/datasource.yaml` — cited
- **Files (ESS first):** `ess/domains/datasource.yaml`, `ess/scenarios/write-input-shape/` (new), `ess/ess-inputs.yaml` (it lists `domains/datasource.yaml` at :8 and the scenarios) — cited
- **Files (connectors):** `crates/loom-connectors/src/cli/write.rs`, `src/cli.rs`, `src/relay.rs` (does not exist in this tree; `story:connectors-relay-client` creates it), a new `src/shape.rs`, `tests/connectors_cli_write.rs`, `tests/connectors_relay.rs` (also new, from the relay story) — cited. `invoke_write` is `src/cli/write.rs:150`. The describe it follows is `src/cli/write.rs:228`, and the invoke input is built at `:249-250`. The shaper goes between them.
- **Files (conformance):** `crates/loom-conformance/src/lib.rs:245-256` (the command match) and `crates/loom-conformance/Cargo.toml` (it lists no `b10x-loom-connectors` today) — cited. A new `crates/loom-conformance/src/shape.rs` — inferred, from the `agent.rs`/`a2a.rs` module-per-domain pattern.
- **Files (module declaration):** `crates/loom-connectors/src/lib.rs` — cited, story Files. It declares only `pub mod cli;` and `pub mod evidence;` (`:70-71`); `loom_connectors::shape_write_input` needs `pub mod shape` and a re-export there.
- **Also likely:** `crates/loom-connectors/tests/fixtures/connectors/fake-connectors` — story names it, so cited, but probably not needed. The fake answers `describe.<operation>.json` else `describe.json` and logs stdin to `stdin.log` (script header), so the new describe answers may need no script change.
- **Symbols:** `ConnectorsCli::invoke_write` (`write.rs:150`), `admits` (`write.rs:319`; its call is `:175`), `idempotency_key` (`write.rs:79`; used `:215`), `SourceEntity.input_schema` (`cli.rs:315-330`, a parsed `loom::json::Value`), `WriteRefusal.code` (a `String`, `write.rs:303-311`), `loom.datasource.ShapeWriteInput`, `BodyDeclaration`, `WriteInputShapeRefused`, `loom_connectors::shape_write_input` — cited
- **Documents:** `AGENTS.md` § Connectors, `CHANGELOG.md` (`### Fixed`) — cited. Nothing under `website/docs/reference/` beyond the generated files.
- **Confidence:** high for the file set, since the story lists it and the tree agrees. Medium for `src/cli.rs`: the story lists it, but the schema is already in `SourceEntity`, so it may need no change.
- **Would collide with:**
  - `story:connectors-relay-client`: `src/relay.rs`, `src/lib.rs`, `tests/connectors_relay.rs`, `Cargo.lock`, AGENTS.md § Connectors. A `depends_on` path orders it: this story depends on it directly.
  - `story:boundary-admits-declared-http-egress`: `ess/ess-inputs.yaml` and `generated/rust/loom/`. A `depends_on` path orders it: this story depends on it directly.
  - `story:agent-selects-the-connectors-relay`: `ess/ess-inputs.yaml`, the `crates/loom-conformance/src/lib.rs` command match, `crates/loom-connectors/src/lib.rs`, the `fake-connectors` fixture, AGENTS.md § Connectors and `generated/rust/loom/`. A `depends_on` path orders it: it depends on this story directly.
  - The two stories this one waits on have no edge between them. They share nothing with each other except `ess/ess-inputs.yaml` and the generated plan files. That is their collision and not this story's.
  - `story:canon-0-9-0-pin` (active, wave 2026-11-21-w2) shares only the house-rule files `Cargo.lock`, `AGENTS.md` and `CHANGELOG.md`. No `depends_on` path orders them. I read only its scope.
- **Safety fact:** `invoke_write` refuses `input-not-admitted` at `write.rs:175`, before the describe at `:228`. The shaper sits after the describe and only moves admitted members. So no input other than the admitted arguments can be sent, and the idempotency key (`:215`) is computed before the shaper and from the request ids only — step 2 (`file:line`), unproven
- **Safety fact:** `ConnectorsInvoker` (`crates/loom-connectors/src/lib.rs:244-332`, input at `:272`) does not call `invoke_write`, so shaping `invoke_write` leaves Commission's path untouched. The only other caller is `crates/loom-plugin/src/effects.rs:727`, which already passes an unshaped `input` and handles `WriteError::Refused` by code string (`:730`) — step 2, unproven
