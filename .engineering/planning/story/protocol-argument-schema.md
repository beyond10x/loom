---
format: aep.planning-md/3
id: story:protocol-argument-schema
kind: story
status: draft
title: Loom refuses generated arguments that do not match the argument schema its loaded protocol declares
tags:
- sdc
relations:
- decomposes: epic:fast-selector
- serves: vision:O1
- depends_on: story:canon-0-10-0-pin
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-executor/Cargo.toml
- confidence: cited
  path: crates/loom-executor/src/arguments.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/tests/argument_schema.rs
- confidence: cited
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: crates/loom-plugin/tests/turn.rs
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/catalog.rs
- confidence: cited
  path: crates/loom-protocols/tests/inbound_answer.rs
- confidence: cited
  path: crates/loom-protocols/tests/operator_reply.rs
- confidence: cited
  path: crates/loom-selector-laya/tests/laya_arguments_slice.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: protocols/inbound-answer/1.yaml
- confidence: cited
  path: protocols/operator-reply/1.yaml
revision: 7
---
# story:protocol-argument-schema

Title: Generated arguments are refused unless they match the argument schema the loaded protocol
declares

Relations: decomposes `epic:fast-selector`; serves `vision:governed-autonomy`, `vision:O1`,
`vision:O3`; depends on `story:laya-arguments-slice` (implemented) and on
`story:canon-0-10-0-pin` (§ Waits on). Records the contract of
`architecture-decision-record:action-argument-schema-from-protocol`; the blocker it answers,
`decision-blocker:action-argument-schema`, is `cleared` (2026-10-14).

Revisions cited: Loom at `8a8fad3b`; Canon 0.9.0 at `8f1f4527`, as `Cargo.lock:187-190` pins it
(read with `git -C ~/beyond10x/canon show 0.9.0:<path>`); Canon 0.10.0 at `2c7a1ea6` for
`check_arguments` (read with `git -C ~/beyond10x/canon show 0.10.0:<path>`).

## Outcome

An action's arguments reach revalidation and a proposal only when they match the argument schema
its protocol declares. Loom reads the schema from the protocol the host loaded and validates the
generated arguments against it after generation and before revalidation. An action whose protocol
declares no schema takes only `{}`. The model never supplies a schema. The check runs for a host
that hands Loom the schemas of the protocol it runs; a Loom given none checks nothing, as today.

## Why

Atlas ADR 0073 step 2 asks for it, and `epic:fast-selector`'s I-004 slice left it out
(`story:laya-arguments-slice`, "Not in this story"). Today no argument is validated by Loom
(`crates/loom-executor/src/arguments.rs:10-11`): `Loom::prepare` hands the generator's value
straight to `Prepared` (`crates/loom-executor/src/lib.rs:597-605`), and an action accepts whatever
object a model writes until a host's effect port inspects it.

Canon 0.9.0 declares the schema: `Action.arguments: Option<Json>` ("Absent: the action takes no
arguments; a runner accepts only `{}`. Canon never validates arguments at run time",
`crates/canon/src/model/mod.rs:206-211`), carried unchanged into the IR
(`crates/canon/src/ir/mod.rs:159-161`). `canon validate` holds each schema to Canon's
argument-schema dialect, one `action-arguments` problem per offending keyword
(`crates/canon/src/validate/mod.rs:566-576`; the dialect,
`crates/canon/src/validate/arguments.rs:1-32`, nesting at most 32 deep, `:40`). The dialect
check is `validate::arguments::check`, `pub(super)` (`arguments.rs:129`). Canon 0.10.0 exports
`validate::check_arguments` (`crates/canon/src/validate/mod.rs:84`, `arguments.rs:869` at
0.10.0), so the value check is Canon's: `check_arguments(schema: Option<&Value>, arguments:
&Value) -> Result<(), ArgumentsRefused>`, pure, `None` taking only `{}`, every refusal an
`ArgumentRefusal {pointer, keyword, reason}` in document order (`arguments.rs:817-824`), and
`ArgumentsRefused::Schema` for a schema outside the dialect (`arguments.rs:835-840`).

Runbook needs the check (`~/beyond10x/sdc/docs/boundary.md` row 15; its driver checks
`required` and `additionalProperties`, `~/beyond10x/sdc/driver/src/main.rs:1365-1383`); four of its
protocols declare `arguments:` (`sdc`, `sdc-direction`, `sdc-epic`, `sdc-spec`). This story gives
the check function and the executor path. A tree's bound program takes its arguments from the
binding (literal or `arguments_from`) after the executor
(`crates/loom-bindings/src/program.rs:17-22`), so the tree path's call is a bindings story's
(§ Not in scope).

## Waits on

- `story:canon-0-10-0-pin`: it moves Loom to Canon 0.10.0, the first tag that exports
  `check_arguments`. Same planner wave; it merges first and this story starts from it.

## Design

- `loom-protocols`: `ProtocolEntry::argument_schema(&self, action) -> ArgumentSchema`, beside
  `ProtocolEntry::actions` (`crates/loom-protocols/src/lib.rs:96-104`), reading
  `Action.arguments` from the entry's model. `ArgumentSchema` is `Declared(Value)` or `Undeclared`;
  an action the protocol does not declare is `None`. `ProtocolEntry::argument_schemas(&self)`
  gives every declared action with its schema, the host's input to the lookup below.
- `ProtocolCatalog::add_yaml` adds no check of its own: it already refuses a protocol Canon's
  validation refuses (`crates/loom-protocols/src/lib.rs:263-264`), and a schema outside the dialect
  is one of those refusals, naming the action (`validate/mod.rs:462`).
- `loom-executor`: a new `arguments::ArgumentSchemas`, a host-supplied lookup from action id to
  `Option<Value>` (none: the protocol declares no schema), and `arguments::check(schema, value) ->
  Result<(), ArgumentMismatch>`, public, which calls `b10x_canon::validate::check_arguments` and
  names the JSON pointer and keyword of the first `ArgumentRefusal`, quoting at most 120 bytes of
  the value; `ArgumentsRefused::Schema` is a mismatch saying the schema is outside Canon's dialect.
  The schema is Canon's `serde_json::Value`; the generated value is written with Commission's
  `json::push_value` and read with `serde_json::from_str`, so Loom rounds no number of its own.
  `crates/loom-executor/Cargo.toml` gains `b10x-canon` at the reference the other three manifests
  name (`AGENTS.md` § Governor: "any other crate that adds Canon uses the same reference"). No
  other dependency is added: `jsonschema` left the executor at `460813ed`. The executor keeps no
  dependency on `loom-protocols` (`crates/loom-executor/Cargo.toml`, `[dependencies]`); hosts
  build the lookup from the entry.
- Numbers are compared by exact value, and the value Canon reads depends on how `serde_json` is
  built (Canon `CHANGELOG.md`, `## [0.10.0]`, "Numbers are compared by exact value";
  `validate/arguments.rs:72`, `# Numbers`): with `arbitrary_precision` it holds number text,
  without it doubles and 64-bit integers. In this workspace the feature comes through
  `connectors-core` (`AGENTS.md:591-592`), so a build of `b10x-loom-executor` alone and a build
  with `b10x-loom-connectors` in it may answer differently, and only for the literals Canon's
  changelog lists (`9007199254740992.5`, `1e400`, an integer beyond 64 bits).
- `Loom::with_argument_schemas(schemas)` beside `Loom::with_governor`
  (`crates/loom-executor/src/lib.rs:142-160`). A Loom built without it checks nothing and records
  no check, as today.
- The check runs in `Loom::prepare` right after `arguments.generate(&context, entry)`
  (`crates/loom-executor/src/lib.rs:600`) and before `Prepared` is returned to `Loom::finish`,
  which revalidates and proposes (`lib.rs:616-623`). It is recorded on the argument request
  (`loom.run.CheckArguments`, below). A declared schema: `check`'s diagnostic on a mismatch. No
  schema: anything but exactly `{}` is a mismatch saying the protocol declares no argument schema
  for the action.
- On a mismatch, `AgentExecutor::run` returns `NoUsefulAction` (`lib.rs:331-333`; arguments are
  what the generator wrote, not an outage), nothing is proposed, and the selection is not
  revalidated.
- What the model is shown is the host's `Briefing::arguments_schema`
  (`crates/loom-executor/src/model_selector.rs:12-14, 69-70`); the governed loop that published
  tool schemas was deleted at `460813ed`. A host's published schema may be narrower than the
  declared one (the plugin's `source.read` lists the item's sources as an `enum`,
  `crates/loom-plugin/src/turn.rs:767-827`); a plugin test holds that it names the same members.
- Loom's own protocols declare schemas: `protocols/inbound-answer/1.yaml` and
  `protocols/operator-reply/1.yaml` for `source.read` (`{source, kind, input}`, the shape
  `crates/loom-plugin/src/effects.rs:6-8` reads), `reply.propose` (`{text}`), `reply.decline`
  (`{reason}`) and, in `operator-reply@1`, `reply.send` (`{channel, thread_ts, text}`, the members
  `turn.rs:796-823` publishes); `protocols/system-query/1.yaml`'s `system.time.read` declares none
  and so takes `{}`. The plugin host builds the lookup from the entry it runs
  (`ProtocolCatalog::plugins()`, `turn.rs:360`) and passes it to the `Loom` it builds
  (`turn.rs:431-436`).
- The command-line host (`crates/loom-intake-slice/src/run.rs:609-620`) passes no lookup: it runs
  the vendored engineering protocols, whose tag declares no `arguments:`
  (`protocols/engineering/lock.json`, tag `0.16.0`), and generates non-empty arguments for them
  (`crates/loom-intake-slice/src/executor.rs:559-592`); with a lookup the empty-object rule would
  refuse every such step. It adopts the check once engineering-protocols declares schemas.
- Canon's dialect is used as Canon declares it; Loom pins no draft of its own.

## Domain relations

- `loom.run.ArgumentRequest` -> `loom.run.Selection`, many-to-one, references — inferable:
  `ess/domains/run.yaml:221-233`, relation `selection`. Unchanged.
- Argument check -> argument request: at most one check per request, recorded as the request's
  move out of `Requested` — declared by this story's ESS first commit (lifecycle of
  `loom.run.ArgumentRequest`, command `loom.run.CheckArguments`).
- Action -> argument schema: zero or one per action, owned by the protocol definition, not by
  Loom's domain — declared by Canon's `protocol/1` model (`Action.arguments`,
  `crates/canon/src/model/mod.rs:206-211` at 0.9.0), not by an ess/1 document of Loom's. Loom's
  specification refers to the outcome of the check only, never to the schema's content.
- Catalogue entry -> argument schema: none. `loom.run.CatalogueEntry` keeps `action` and `status`
  only (`ess/domains/run.yaml:57-63`); the schema is looked up by action id from the host-supplied
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
  - `a_loom_without_schemas_checks_no_arguments`: `{"x": 1}` is proposed and the argument
    request stays `Requested`.
  - `an_action_outside_the_catalogue_never_reaches_the_check` (no `CheckArguments` recorded).
- `crates/loom-executor/src/arguments.rs` (unit tests)
  - `check_passes_canons_refusal_through_with_its_pointer_and_keyword`: `{"limit": "20"}` under a
    schema with `limit` of `type: integer` is refused naming `/limit` and `type`, the pair
    `check_arguments` returns first.
  - `a_schema_outside_canons_dialect_is_a_mismatch_naming_the_dialect`: a hand-built schema with
    `$ref` is a mismatch saying the schema is outside Canon's dialect; no argument is checked.
  - `a_mismatch_quotes_at_most_120_bytes_of_the_value`: a 4 KiB string under `maxLength: 1` gives
    a diagnostic whose quoted value is 120 bytes or fewer.
  - `a_number_past_a_double_is_checked_as_this_build_holds_it`: the test reads the feature state
    (whether `serde_json::from_str::<serde_json::Value>("9007199254740992.5")` writes back as
    `9007199254740992.5`) and prints it as `arbitrary_precision: on|off`; under a schema with
    `n` of `type: number, maximum: 9007199254740992` the value `{"n": 9007199254740992.5}` is
    refused (`maximum`) when it is on and accepted when it is off. It runs in
    `cargo test -p b10x-loom-executor`, whatever state that build has.
- `crates/loom-plugin/tests/turn.rs`
  - `a_source_read_without_kind_is_never_performed`: a scripted model writes
    `{"source": "<fixture>", "input": {}}`; the turn submits no `source_read` evidence and starts
    no Connectors command.
  - `each_published_argument_schema_names_the_declared_members`: for every action of
    `inbound-answer@1` the published `input_schema` has the declared `required` list and property
    names.
- `crates/loom-protocols/tests/catalog.rs`
  - `a_protocol_whose_argument_schema_is_not_an_object_schema_is_refused_naming_the_action`.
  - `a_protocol_whose_argument_schema_is_outside_canons_dialect_is_refused_naming_the_action`
    (`$ref` under `properties`).
  - `argument_schema_reads_the_declared_schema_and_undeclared_otherwise`.
- `crates/loom-protocols/tests/inbound_answer.rs`
  - `source_read_declares_the_arguments_the_plugin_effect_port_reads`.
- `crates/loom-protocols/tests/operator_reply.rs`
  - `reply_send_declares_channel_and_text_as_required`.
- `crates/loom-selector-laya/tests/laya_arguments_slice.rs`
  - `generated_arguments_that_do_not_match_release_inspect_are_never_proposed`: the slice's three
    runs keep passing with a schema for each of `metrics.inspect`, `logs.search`, `release.inspect`;
    a fourth run whose scripted arguments miss a required field proposes nothing.
- `grep -c 'tests/argument_schema.rs' AGENTS.md` prints `1`: § Checks that hold the rules has a
  row naming these tests.

Every test uses fixtures and scripted generators or models; the laya slice's model is a loopback
stub (`laya_arguments_slice.rs:27`, `:94`). No test compiles a protocol set per test: the catalog
tests build one catalog per test binary.

## ESS first

The first commit changes only `ess/domains/run.yaml`:

- `loom.run.ArgumentRequest`'s lifecycle (`ess/domains/run.yaml:234-236`, today `initial:
  Requested`, `states: [Requested]`) becomes `initial: Requested`, `states: [Requested, Matched,
  Mismatched]`, `terminal: [Matched, Mismatched]`, with transitions `match` and `mismatch` from
  `Requested`.
- Command `loom.run.CheckArguments` (input `argument_request_id`, `action: String`), outcomes:
  `undeclared-not-empty` (`external`: the protocol declares no schema for the action and the
  arguments are not the empty object), `mismatched` (`external`: the arguments do not satisfy the
  declared schema), each `moves: loom.run.ArgumentRequest.mismatch` and emits
  `loom.run.ArgumentsMismatched`; `matched` `moves: loom.run.ArgumentRequest.match` and emits
  `loom.run.ArgumentsMatched`; `wrong-state` (`wrong_state: true`, error
  `loom.run.ArgumentRequestStateConflict`). The `external:` form is the one
  `loom.run.RevalidateSelection` already uses (`ess/domains/run.yaml:548`).
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

Derived from `8a8fad3b`. Every line is **cited** (read in the tree) or **inferred** (a reading that
could be wrong).

- **Files:** `ess/domains/run.yaml` (`ArgumentRequest` 221-236, `RequestArguments` 406-432) — cited
- **Files:** generated Loom model under `generated/rust/loom/` (regenerated; the `task drift`
  target, `Taskfile.yml:23`) — cited
- **Files:** `crates/loom-executor/src/arguments.rs` (`ArgumentSchemas`, `check`,
  `CheckArgumentsBehavior`) — cited
- **Files:** `crates/loom-executor/Cargo.toml` (`b10x-canon` at the pinned tag), `Cargo.lock`
  (the executor's dependency list) — inferred
- **Files:** `crates/loom-executor/src/lib.rs` (`Loom` fields 97-105, `new` 130-140,
  `with_governor` 146-160, `prepare` 478-606) — cited
- **Files:** `crates/loom-executor/tests/argument_schema.rs` (new) — cited
- **Files:** `crates/loom-conformance/src/lib.rs` (the conformance target answers
  `loom.run.RequestArguments` today, so it answers `CheckArguments` too) — inferred
- **Files:** `crates/loom-protocols/src/lib.rs` (`ProtocolEntry` 58-104),
  `crates/loom-protocols/tests/catalog.rs`, `tests/inbound_answer.rs`, `tests/operator_reply.rs` —
  cited
- **Files:** `protocols/inbound-answer/1.yaml` (actions 40-69), `protocols/operator-reply/1.yaml`
  — cited
- **Files:** `crates/loom-plugin/src/turn.rs` (catalog 360, `Loom::new` 431-436),
  `crates/loom-plugin/tests/turn.rs` — cited
- **Files:** `crates/loom-selector-laya/tests/laya_arguments_slice.rs` (`run` 415-427) — cited
- **Files:** `AGENTS.md` (§ Checks that hold the rules, line 118; § Plugin host, 1707),
  `CHANGELOG.md` (`Breaking:` names the empty-object rule for plugin turns, the new
  `ArgumentRequest` states, and `with_argument_schemas` for hosts) — inferred
- **Not touched:** `Cargo.toml` (the workspace manifest; `story:canon-0-10-0-pin` moves the
  Canon pin), `crates/loom-intake-slice/src/run.rs` (passes no lookup),
  `protocols/engineering/` (vendored, never edited here, `AGENTS.md:55`) — cited
- **Would collide with:** any unit in the `loom.run` domain (`ess/domains/run.yaml`, generated
  model), `story:selector-stop-names-set-aside-record` (`ess/domains/run.yaml`),
  `story:tree-run-command` (`generated/rust/loom/`, `crates/loom-conformance/src/lib.rs`), any
  unit changing
  `crates/loom-plugin/src/turn.rs` or the two plugin protocols — inferred
- **Safety fact:** the operator-reply and inbound-answer definitions are registered with
  `revision: env!("CARGO_PKG_VERSION")` and their sha256 is computed at load
  (`crates/loom-protocols/src/lib.rs:179-188, 273`); no test pins their digest — inferred, unproven

## Not in scope

- Choosing a schema dialect: Canon declares it.
- The check in the tree path: `ProgramEffects` resolves a step's arguments (request, literal,
  `arguments_from`, `crates/loom-bindings/src/program.rs:17-22`) after the executor, and calls
  `arguments::check` there in a story of the `crates/loom-bindings` lane. The tree host
  (`story:tree-run-command`) passes no lookup to its `Loom`, whose `EmptyObjectArguments` writes
  `{}` for every action.
- The command-line host and the engineering protocols' schemas (another repository's release).
- Validating a workflow's literal deterministic-step arguments at `workflow::load`
  (`crates/loom-executor/src/workflow.rs:220, 305-306`); a follow-up can reuse `ArgumentSchemas`.
- Argument checks in Commission's revalidation or in an effect port; a binding checking its own
  Connector operation's `input_schema` (`crates/loom-connectors/src/cli.rs:315-316`).
- Publishing the declared schema in place of the host's `Briefing::arguments_schema`.
- Selection telemetry of argument mismatches.
