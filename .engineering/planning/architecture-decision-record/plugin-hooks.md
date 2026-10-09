---
format: aep.planning-md/3
id: architecture-decision-record:plugin-hooks
kind: architecture-decision-record
status: accepted
title: Loom hosts plugins through hooks around a governed run
revision: 4
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-09T07:07:46Z", actor: "human:timo", revision: 2}
---
## Decision

Loom gains a plugin layer: hooks around a governed run, so that an unattended agent is a plugin of
Loom, not a program that embeds it. A plugin is a crate linked at build time. Its hooks are
`source.poll` (new items and a cursor), `classify` (an intent and tool hints), `project` (the actions
a turn may use), `turn` (one governed run on the item), `result` (what the run proposes) and
`objectives` (weights for the next poll). `b10x-loom plugin run <name>` hosts it in a loop, with its
state outside any workspace.

The turn stays governed: every action the model sees comes from a protocol's frontier (ADR 0072),
here a Loom-owned read-only protocol `inbound.answer/1`. Data sources are read through Connectors,
behind Commission's `ConnectorInvoker`; nothing is written to an external system in this
decision's first version: a reply is a proposal in the plugin's record.

The first plugin is a Slack handler that walks channels for unanswered messages. After it runs,
the code is measured and what generalises is expressed as YAML (a protocol checked by Canon, and a
YAML-only plugin, `story:yaml-only-plugin`).

## Source

The operator, 2026-10-09: "I want you todo this as loom plugin to build a plugin sdk like layer for
loom. think of \"hooks\". and we see how much code this will produce... eventually extract the
generlizable parts and express in YAML protocol+canon"; writes: "Read-only first"; the pipeline:
"input -> classify(intent -> ask|request|task|find, tools=[...]) -> tool+datasource projection ->
[begin agent_turn] ..agent loop with reasoning + tool-calls... [end] -> result"; data sources: "list
them, get entities + schema, list, search, get them". He approved the plan and then asked
an agent to implement it ("mh, you should implement it now").

## Consequences

- The existing in-run hooks (`HookPort`: before-call, after-call, stop) keep their rule that a hook
  only narrows; the plugin hooks sit outside the run and grant nothing.
- A plugin never publishes a write to a turn in this version.
- Nothing is discovered from a workspace: plugins are linked, configuration is a path the operator
  gives.

## Amendment, 2026-10-09 (plan critics, round 1 and 2)

Reads do not go behind Commission's `ConnectorInvoker`. They go through a plain Connectors CLI
client (`loom_connectors::cli`) behind `DataSourceEffects`, a Commission `EffectPort` in
`loom-plugin`. Reasons: the poll runs outside any run, where no `AdmittedRequest` exists; and a
turn's `source.read` names its source and kind per call, where an `ActionBinding` is fixed per
commission. The governance holds: `source.read` and `reply.propose` are frontier actions of
`inbound.answer/1`, each requiring a capability the host's authority provider grants, and the
effect port refuses any pair outside the turn's projection. A read's result carries the audit
reference the CLI returns.
