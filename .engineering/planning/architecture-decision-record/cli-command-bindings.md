---
format: aep.planning-md/3
id: architecture-decision-record:cli-command-bindings
kind: architecture-decision-record
status: accepted
title: b10x-loom run may bind a custom protocol's actions to operator-declared commands, conferring no authority
relations:
- decides: story:cli-custom-protocol-commands
revision: 2
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-15T20:32:19Z", actor: "human:timo", revision: 2}
---
## Decision (proposed)

`b10x-loom run` may execute a custom protocol's actions through commands the operator declares in
a run configuration: one argv per action (no shell), the arguments as JSON on standard input, a
report as JSON on standard output, the exit status deciding `Performed` or `Refused`. A binding
confers no authority: an action that needs a capability still needs its grant, and the governor
still decides admission. Commands run under the CLI's existing confinement, never with Loom's own
credentials.

## Context

A custom protocol's actions end `NoLocalExecutor`
(`website/docs/guides/system-queries.md:81-85`), so every custom protocol needs an embedding host.
The CLI runs only the built-in bindings (software change, clock).

## Consequences

- The run configuration and the stdin/stdout contract become part of the `b10x-loom` command's
  stable surface, named in the CHANGELOG when they change (`story:command-invocation-contract`).
- An operator can make the CLI run any program they declare; the boundary is the operator's
  configuration plus the governor's admission, as for an embedding host.

## Alternatives

- A plugin ABI loaded by the CLI. Rejected for now: larger surface, same authority question.
- Keep custom protocols for embedders only. Rejected: each consumer ships its own binary.

## Source

story:cli-custom-protocol-commands.
