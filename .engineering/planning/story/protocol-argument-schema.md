---
format: aep.planning-md/3
id: story:protocol-argument-schema
kind: story
status: draft
title: Loom refuses generated arguments that do not match the argument schema its loaded protocol declares
relations:
- decomposes: epic:fast-selector
revision: 1
---
# story:protocol-argument-schema

Title: Generated arguments are refused unless they match the argument schema the loaded protocol declares

Relations: decomposes `epic:fast-selector`; serves `vision:governed-autonomy`, `vision:O1`,
`vision:O3`; depends on `story:laya-arguments-slice` (implemented). Records the contract of
`architecture-decision-record:action-argument-schema-from-protocol`; clears
`decision-blocker:action-argument-schema`.

Revisions cited: Loom at `9bd0fa6`; Canon 0.4.0 at `7c2cec1`, as `Cargo.lock:146-148` pins it.

## Outcome

An action's arguments reach revalidation and a proposal only when they match the argument schema
its protocol declares. Loom reads the schema from the protocol the host loaded, publishes it as the
action's tool schema in the governed loop, and validates the generated arguments against its own
copy after generation and before revalidation. An action whose protocol declares no schema takes
only `{}`. The model never supplies a schema.

## Why

Atlas ADR 0073 step 2 asks for it, and `epic:fast-selector`'s I-004 slice left it out
(`story:laya-arguments-slice`, "Not in this story"). Today no argument is validated by Loom
(`crates/loom-executor/src/arguments.rs:10-11`), and every frontier action is published to the
model as `{"type": "object"}` (`crates/loom-executor/src/harness/governed.rs:745-761`), so an
action accepts whatever object a model writes until a host's effect port inspects it.

## Design

- `loom-protocols`: `ProtocolEntry::argument_schema(&self, action) -> ArgumentSchema`, beside
  `ProtocolEntry::actions` (`crates/loom-protocols/src/lib.rs:58-66`), reading the field Canon
  adds to its source `Action` (`crates/canon/src/model/mod.rs:184-197` today has none).
  `ArgumentSchema` is `Declared(Value)` or `Undeclared`.
- `ProtocolCatalog::add_yaml` (`crates/loom-protocols/src/lib.rs:161-198`) refuses, after Canon's
  validation and compilation (`lib.rs:181-184`), a protocol any of whose declared schemas is not an
  object schema or does not compile in `jsonschema::validator_for`, naming the protocol and the
  action, as `OutputSchema::named` does for an answer schema
  (`crates/loom-executor/src/harness/turn_loop/answer.rs:86-105`).
- `loom-executor`: a new `arguments::ArgumentSchemas`, a host-supplied lookup from action id to
  `Option<Value>` (none: the protocol declares no schema), compiled once into validators when it is
  built; `Loom::with_argument_schemas(schemas)`. A Loom built without one treats every action as
  declaring none. The executor keeps no dependency on `loom-protocols`, as `workflow::load` takes
  the protocol's actions (`crates/loom-executor/src/workflow.rs:119-125`, `AGENTS.md:498-501`).
- The check runs in `Loom::prepare` right after `arguments.generate(&context, entry)`
  (`crates/loom-executor/src/lib.rs:681`) and before `Prepared` is returned to `Loom::finish`,
  which revalidates and proposes (`lib.rs:698-710`). It is recorded on the argument request
  (`loom.run.CheckArguments`, below). A declared schema: the validator's bounded diagnostic on a
  mismatch. No schema: anything but exactly `{}` is a mismatch saying the protocol declares no
  argument schema for the action.
- On a mismatch, `AgentExecutor::run` returns `NoUsefulAction` (arguments are what the generator
  wrote, not an outage); the governed loop answers the model's call with a refused tool result
  carrying the diagnostic, as non-object arguments are
  (`crates/loom-executor/src/harness/turn_loop/tests.rs:4265`), and nothing is proposed. The
  selection is not revalidated.
- `governed::spec` (`crates/loom-executor/src/harness/governed.rs:749-770`) publishes the declared
  schema as `input_schema`; for an action with none it publishes
  `{"type": "object", "maxProperties": 0}`. `CatalogueTools` looks the schema up through the same
  `ArgumentSchemas` the check uses, so what is published and what is enforced cannot differ.
- Loom's own protocols declare schemas once Canon's field exists: `protocols/inbound-answer/1.yaml`
  for `source.read` (`{source, kind, input}`, the shape `crates/loom-plugin/src/effects.rs:6-8`
  reads) and `reply.propose`; `protocols/system-query/1.yaml`'s `system.time.read` declares none
  and so takes `{}`. The plugin host (`crates/loom-plugin/src/turn.rs:392-393`) and the CLI host
  (`crates/loom-intake-slice/src/run.rs:593-602`) pass `with_argument_schemas` built from the
  entry they run.
- Canon's dialect is used as Canon declares it; Loom pins no draft of its own. The input is
  already bounded: 64 KiB per tool call (`crates/loom-executor/src/harness/wire/bound.rs:12`) and
  128 levels of nesting (`crates/loom-executor/tests/json_depth.rs:1-25`).

## Domain relations

- `loom.run.ArgumentRequest` -> `loom.run.Selection`, many-to-one, references — inferable:
  `ess/domains/run.yaml:442-459`, relation `selection`. Unchanged.
- Argument check -> argument request: at most one check per request, recorded as the request's
  move out of `Requested` — declared by this story's ESS first commit (lifecycle of
  `loom.run.ArgumentRequest`, command `loom.run.CheckArguments`).
- Action -> argument schema: zero or one per action, owned by the protocol definition, not by
  Loom's domain — inferred from the ADR; declared by Canon's `protocol/1` model once Canon ships the
  field, not by an ess/1 document of Loom's. Loom's specification refers to the outcome of the
  check only, never to the schema's content.
- Catalogue entry -> argument schema: none. `loom.run.CatalogueEntry` keeps `action` and `status`
  only (`ess/domains/run.yaml:58-65`); the schema is looked up by action id from the host-supplied
  lookup, not carried by the frontier or the catalogue.

## Acceptance

Arguments that do not match their action's protocol-declared schema, or any non-empty arguments of
an action whose protocol declares none, never reach revalidation or a proposal, and every check is
recorded on its argument request.

- `crates/loom-executor/tests/argument_schema.rs`
  - `arguments_matching_the_declared_schema_are_revalidated_and_proposed`: a scripted generator
    writes `{"service": "checkout-api", "environment": "prod", "limit": 20}` for an action whose
    schema requires those three; the proposal carries them; the argument request is `Matched`.
  - `arguments_not_matching_the_declared_schema_are_never_revalidated_or_proposed`: `limit` as a
    string; `run` returns `NoUsefulAction`; `Loom::revalidations()` is empty; the request is
    `Mismatched` naming the action.
  - `an_action_with_no_declared_schema_takes_only_the_empty_object`: `{}` is proposed; `{"x": 1}`
    is refused naming the action and "declares no argument schema".
  - `a_loom_without_schemas_treats_every_action_as_undeclared`.
  - `an_action_outside_the_catalogue_never_reaches_the_check` (no `CheckArguments` recorded).
- `crates/loom-executor/src/harness/turn_loop/tests.rs`
  - `a_frontier_action_is_published_with_its_protocol_schema`: the published `input_schema` equals
    the declared schema; with none, `{"type": "object", "maxProperties": 0}`.
  - `a_mismatched_call_is_refused_back_to_the_model_and_not_proposed`.
- `crates/loom-protocols/tests/catalog.rs`
  - `a_protocol_whose_argument_schema_is_not_an_object_schema_is_refused_naming_the_action`.
  - `a_protocol_whose_argument_schema_does_not_compile_is_refused_naming_the_action`.
  - `argument_schema_reads_the_declared_schema_and_undeclared_otherwise`.
- `crates/loom-protocols/tests/inbound_answer.rs`
  - `source_read_declares_the_arguments_the_plugin_effect_port_reads`.
- `crates/loom-selector-laya/tests/laya_arguments_slice.rs`
  - `generated_arguments_that_do_not_match_release_inspect_are_never_proposed`: the slice's three
    runs keep passing with a schema for each of `metrics.inspect`, `logs.search`, `release.inspect`;
    a fourth run whose scripted arguments miss a required field proposes nothing.
- `AGENTS.md`'s invariant table gains a row naming these tests.

## ESS first

The first commit changes only `ess/domains/run.yaml`:

- `loom.run.ArgumentRequest`'s lifecycle becomes `initial: Requested`, `states: [Requested,
  Matched, Mismatched]`, `terminal: [Matched, Mismatched]`, with transitions `match` and
  `mismatch` from `Requested`.
- Command `loom.run.CheckArguments` (input `argument_request_id`, `action: String`), outcomes:
  `undeclared-not-empty` (`external`: the protocol declares no schema for the action and the
  arguments are not the empty object), `mismatched` (`external`: the arguments do not satisfy the
  declared schema), each `moves: loom.run.ArgumentRequest.mismatch` and emits
  `loom.run.ArgumentsMismatched`; `matched` `moves: loom.run.ArgumentRequest.match` and emits
  `loom.run.ArgumentsMatched`; `wrong-state` (`wrong_state: true`, error
  `loom.run.ArgumentRequestStateConflict`). The `external:` form is the one
  `loom.run.RevalidateSelection` already uses (`ess/domains/run.yaml:1014-1015`).
- Events `loom.run.ArgumentsMatched {argument_request_id}` and `loom.run.ArgumentsMismatched
  {argument_request_id, action, declared: Boolean, reason: String}`; error
  `loom.run.ArgumentRequestStateConflict {argument_request_id}`.

On it `task drift` fails (the generated `crates/loom-executor` model lags the specification), and
the conformance suite gains `CheckArguments` scenarios that fail without a behaviour; later commits
regenerate, implement `CheckArgumentsBehavior` on `RequestRecord`
(`crates/loom-executor/src/arguments.rs:298-340` is the pattern) and make
`arguments_not_matching_the_declared_schema_are_never_revalidated_or_proposed` pass without
changing `ess/`. The schema itself is not an ESS noun of Loom's: it is Canon's.

## Scope

- `ess/domains/run.yaml`
- generated Loom model (regenerated; the `task drift` target)
- `crates/loom-executor/src/arguments.rs`, `crates/loom-executor/src/lib.rs`
  (`Loom::prepare`, `with_argument_schemas`)
- `crates/loom-executor/src/harness/governed.rs` (`spec`, `CatalogueTools`, refusal back to the
  model)
- `crates/loom-executor/tests/argument_schema.rs` (new),
  `crates/loom-executor/src/harness/turn_loop/tests.rs`
- `crates/loom-protocols/src/lib.rs`, `crates/loom-protocols/tests/catalog.rs`,
  `crates/loom-protocols/tests/inbound_answer.rs`
- `protocols/inbound-answer/1.yaml`, `protocols/system-query/1.yaml` (schemas, if Canon's field
  needs an explicit entry for "none")
- `crates/loom-plugin/src/turn.rs`, `crates/loom-intake-slice/src/run.rs` (pass the lookup)
- `crates/loom-selector-laya/tests/laya_arguments_slice.rs`
- `Cargo.toml`, `Cargo.lock` (Canon and engineering-protocols pins)
- `AGENTS.md`, `CHANGELOG.md` (`Breaking:` names the empty-object rule, the new refusals of
  `add_yaml`, and `with_argument_schemas` for hosts)

Shares `ess/domains/run.yaml` and the generated Loom model with any running unit in the `loom.run`
domain, and `Cargo.lock` with any pin move.

## Not in scope

- Choosing a schema dialect: Canon declares it.
- Validating a workflow's literal deterministic-step arguments at `workflow::load`
  (`crates/loom-executor/src/workflow.rs:440-444`); a follow-up can reuse `ArgumentSchemas`.
- Argument checks in Commission's revalidation or in an effect port; a binding checking its own
  Connector operation's `input_schema` (`crates/loom-connectors/src/cli.rs:285-300`).
- Selection telemetry of argument mismatches.
- Declaring schemas in engineering protocols (another repository's work; see Depends on).

## Depends on

1. **Canon** releases a version that declares an optional argument schema on each `protocol/1`
   action, names its dialect, validates it, and is pinned by Loom (`~/.cache/loom-w18/dec/need-canon.md`
   is the request). Until then `add_yaml` refuses any document carrying the field
   (`crates/canon/src/model/mod.rs:185`, `deny_unknown_fields`).
2. **engineering-protocols** releases `software-change@1` (and any other protocol the CLI runs)
   with schemas for its argument-taking actions (`tests.write`, `implementation.edit`, and the rest
   `crates/loom-intake-slice/src/run.rs:73-78` lists), built on that Canon release. Without it the
   empty-object rule refuses the CLI's software-change arguments.
3. In this repository: `story:laya-arguments-slice` (implemented).

Not ready until 1 and 2 have shipped.
