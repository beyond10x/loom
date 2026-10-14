---
format: aep.planning-md/3
id: architecture-decision-record:action-argument-schema-from-protocol
kind: architecture-decision-record
status: accepted
title: Loom validates generated arguments against the argument schema its loaded protocol declares
relations:
- decides: decision-blocker:action-argument-schema
revision: 2
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-14T20:22:48Z", actor: "human:timo", revision: 2}
---
# Loom validates generated arguments against the argument schema its loaded protocol declares

Suggested id: `architecture-decision-record:action-argument-schema-from-protocol` (the name
`decision-blocker:action-argument-schema` already records in its Decision section). Clears
`decision-blocker:action-argument-schema`.

Revisions cited: Loom at `9bd0fa6` (worktree `lw11-int`); Canon `b10x-canon` 0.4.0 at
`7c2cec1395f4c7f383d64c7b01c189fcd48fecf7`, the revision Loom's `Cargo.lock:146-148` pins, read
from `~/.cargo/git/checkouts/canon-604f5fd4ae20f4ae/7c2cec1/`.

## Decision

The Canon protocol definition declares one argument schema per action. Loom reads it from the
protocol it loads and refuses an action whose generated arguments do not match it. The frontier
action and the Connector operation carry no schema of their own.

1. **Owner and cardinality.** An action of a `protocol/1` document has zero or one argument
   schema, declared beside the action in the document. The schema is part of the definition's
   bytes, so it is pinned by the same SHA-256 and provenance Loom already keeps for the definition
   (`crates/loom-protocols/src/lib.rs:185-195`), and it reaches Loom only through a host-selected
   catalog entry (`ProtocolCatalog::add_yaml`, `crates/loom-protocols/src/lib.rs:161-198`), never
   from a model, a frontier or a Connector.
2. **Dialect is Canon's to declare.** Which JSON Schema draft, and whether only a subset of it, is
   decided and documented by Canon, and Canon's validator refuses a schema outside it. Loom does
   not pick a dialect of its own. Loom adds two load-time conditions on top of Canon's: the
   schema's top level is an object schema, and it compiles in Loom's validator (the rule Loom
   already applies to a structured-answer schema, `crates/loom-executor/src/harness/turn_loop/answer.rs:86-105`).
   A schema failing either refuses the whole protocol at `add_yaml`, naming the protocol and the
   action, so a protocol whose schemas Loom cannot enforce is never in a catalog.
3. **An action that declares no schema takes only the empty object.** Its arguments are accepted
   when they are exactly `{}` and refused otherwise, naming the action and saying that its protocol
   declares no argument schema. A Loom built without a schema lookup treats every action this way.
4. **Where the check sits.** In the one pipeline both entry points share (`Loom::prepare`,
   `crates/loom-executor/src/lib.rs:574-687`): after the argument generator returns
   (`lib.rs:681`) and before `Loom::finish` revalidates the selection against the current frontier
   and proposes it (`lib.rs:698-710`). An action outside the catalogue, or one Commission refuses
   to admit, is refused before arguments are generated (`lib.rs:596-669`) and is never checked. A
   refused check proposes nothing and revalidates nothing.
5. **The model never supplies the schema.** The schema Loom validates against is the one the host
   handed it from the loaded protocol. In the governed loop the same schema is published as the
   action's tool `input_schema`, replacing today's `{"type": "object"}`
   (`crates/loom-executor/src/harness/governed.rs:745-770`); for an action with no schema the
   published schema admits only the empty object. Publishing it is guidance, not authority: what
   the model writes is validated against Loom's copy, not against anything the model returned.
6. **How the schema reaches the executor.** The host supplies a lookup from action id to schema,
   read from `ProtocolEntry` (a new `argument_schema(action)` beside `ProtocolEntry::actions`,
   `crates/loom-protocols/src/lib.rs:58-66`), as `workflow::load` is supplied the protocol's
   actions (`crates/loom-executor/src/workflow.rs:119-125`), so `loom-executor` keeps no
   dependency on `loom-protocols` (`AGENTS.md:498-501`).

## Context

- Atlas ADR 0073, Decision step 2, says generated arguments are schema validated, and
  `docs/examples/laya-fast-selection.md:35-45` shows a schema handed to the reasoning model for
  `release.inspect`. Nothing in Loom does it: `crates/loom-executor/src/arguments.rs:10-11` says
  validation is not done because the schema's source is undecided, and the governed loop publishes
  every frontier action as `{"type": "object"}` for the same reason
  (`crates/loom-executor/src/harness/governed.rs:745-761`).
- Canon at the pinned revision declares no schema. Its source `Action` holds `description`,
  `precondition`, `requires`, `effect` and `may_produce` and refuses any other field
  (`crates/canon/src/model/mod.rs:184-197`, `#[serde(deny_unknown_fields)]` at 185); its compiled
  `ir::Action` holds the same (`crates/canon/src/ir/mod.rs:129-139`). So a protocol cannot declare
  a schema until Canon ships the field: `add_yaml` would refuse the document at parse
  (`crates/loom-protocols/src/lib.rs:174`).
- The frontier action Loom receives carries `action`, `status`, `capability` and `reasons` and
  nothing about arguments (`ess/commission/domains/responsibility.yaml:396-406`,
  `generated/rust/commission/src/responsibility.rs:341-350`); Canon's decision gives each action's
  status as an untyped section (`crates/canon/src/model/decision.rs:31-34`). The catalogue entry
  Loom projects from it carries `action` and `status` only (`ess/domains/run.yaml:58-65`). (The
  blocker cited Canon `crates/canon/src/lib.rs:41-44` at `cf29c4b`; at the current pin that file
  holds only module declarations, `crates/canon/src/lib.rs:8-16`.)
- Arguments are already bounded before any check: a tool call's arguments are at most 64 KiB
  (`crates/loom-executor/src/harness/wire/bound.rs:12`) and every model-sent JSON parse refuses
  nesting past 128 levels (`crates/loom-executor/tests/json_depth.rs:1-25`). A protocol document,
  and so every schema in it, is at most 1 MiB (`crates/loom-protocols/src/lib.rs:13-14`).
- Loom already links a JSON Schema validator: `jsonschema` 0.52.1 with default features off
  (`crates/loom-executor/Cargo.toml:13`), used for structured answers
  (`crates/loom-executor/src/harness/turn_loop/answer.rs:99`, `:133-139`).
- Actions that take arguments are already run: `inbound-answer@1`'s `source.read {source, kind,
  input}` (`crates/loom-plugin/src/effects.rs:6-8`), and `software-change@1`'s `tests.write` and
  `implementation.edit` through the CLI's model-written arguments
  (`crates/loom-intake-slice/src/run.rs:73-78`, `:593-602`). Each host checks them in its own
  effect port, after Commission admitted the proposal.

## Consequences

- Validation of generated arguments happens once, in Loom, against a schema with one owner and a
  pinned digest. A wrong-shaped call is refused before it is revalidated or proposed, so
  Commission and the effect port are never handed it.
- In the governed loop the model sees the real schema of each action in its tool list, so the
  model is told the shape it is held to; a mismatch is answered to the model as a refused call, as
  non-object arguments already are
  (`crates/loom-executor/src/harness/turn_loop/tests.rs:4265`), and is not proposed.
- An action whose protocol declares no schema takes only `{}`. Until a protocol that Loom runs
  declares schemas for its argument-taking actions, those actions are refused with arguments:
  `inbound-answer@1` and `system-query@1` are Loom's own (`protocols/`) and gain schemas in the
  same change; `software-change@1` and the other engineering protocols
  (`b10x-canon-engineering` 0.7.0, `Cargo.lock:157-160`) must declare theirs before Loom ships the
  rule, or the CLI's software-change runs regress. The release names it under `Breaking:`.
- `ProtocolCatalog::add_yaml` refuses more documents: one whose schema is not an object schema or
  does not compile in Loom's validator.
- A schema change is a protocol change. It is carried by the protocol's revision and the
  definition digest; `clock_compatible` keeps comparing Loom's clock protocol with itself
  (`crates/loom-protocols/src/lib.rs:69-102`), so declaring a schema on `system.time.read` changes
  both sides at once.
- Canon must ship the field first, with its dialect; Loom's work cannot start before a Canon
  release Loom can pin.
- Not decided here: checking a workflow's literal deterministic-step arguments
  (`crates/loom-executor/src/workflow.rs:440-444`) against the same schema at load, and argument
  checks in Commission's own revalidation.

## Alternatives rejected

- **The frontier action carries the schema.** Each frontier would have to repeat the schema
  Canon computes from nothing but the protocol, so a governor (or a Commission host) could hand a
  different schema per frontier with no digest binding it to the definition the case was opened
  under. The frontier is Commission's contract for status and authority
  (`ess/commission/domains/responsibility.yaml:394-406`); a schema in it widens that contract and
  every governor implementation, and moves the shape of an action out of the one document
  reviewers already review. The schema does not change with case state, so a per-frontier copy
  adds a channel without adding information.
- **The Connector operation carries it.** A Connector operation already describes an
  `input_schema` (`crates/loom-connectors/src/cli.rs:11-15`, `:285-300`), but it is the schema of
  the operation a host binds the action to at run time, read from the connector's own `describe`
  answer. It is absent for actions with no binding (`reply.propose`, `reply.decline` in
  `protocols/inbound-answer/1.yaml:48-69`, `effect: none`), can differ per host and per binding,
  and moves with the connector's release rather than the protocol's. It would let an
  external system widen what a protocol action accepts without the protocol changing. A binding
  may still check its own operation's schema in the effect port; that is the effect port's check,
  after Loom's, and never replaces it.
- **No schema means any object** (today's behaviour). Rejected as failing toward more authority:
  an action whose protocol says nothing about its arguments would accept anything a model wrote.
- **No schema means the action is refused outright.** Rejected: it would make parameterless
  actions such as `system.time.read` (`protocols/system-query/1.yaml:22-27`) unrunnable without
  adding any safety the empty-object rule does not give, since `{}` carries nothing the protocol
  did not declare.

## Source

`decision-blocker:action-argument-schema` (Decision, 2026-10-14); `epic:fast-selector`;
`story:laya-arguments-slice` (Not in this story); Atlas ADR 0073; `docs/examples/laya-fast-selection.md`.
