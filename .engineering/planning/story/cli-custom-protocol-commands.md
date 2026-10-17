---
format: aep.planning-md/3
id: story:cli-custom-protocol-commands
kind: story
status: implemented
title: b10x-loom run drives an installed custom protocol through commands the operator binds to its actions
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- informed_by: story:command-invocation-contract
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-cli
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: website/docs/guides/system-queries.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-16T16:34:55Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-16T16:34:56Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-17T06:32:59Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":2}}}
---
## Outcome

`b10x-loom run` drives a protocol installed with `protocols add`, its actions bound to commands the
host declares in a run configuration: each admitted action runs its command with the arguments as
JSON on standard input, and the command's report on standard output becomes the action's
`Performed` report. A custom protocol no longer needs an embedding host.

## Acceptance

`a_custom_protocol_runs_through_declared_commands` (`crates/loom-cli/tests/`): a protocol of two
actions installed with `protocols add`, a run configuration binding each to a test script; `run`
invokes each script once with its arguments on stdin, records each report, and the case completes;
an action with no binding still ends `NoLocalExecutor` naming the action.

## Why (Loom 0.22.0)

A custom definition whose actions, artifacts, evidence, authority or completion differ from the
clock protocol returns `NoLocalExecutor`
(`website/docs/guides/system-queries.md:81-85`; installation at `:46-60`), so every custom
protocol needs a host that embeds the SDK.

## Sketch

A run configuration file mapping `action -> command` (argv, no shell), with the workflow YAML
beside it; a command's exit status and stdout decide `Performed` or `Refused`; commands run under
the CLI's existing confinement. The contract and its authority boundary are proposed in
`architecture-decision-record:cli-command-bindings`.

## Workaround it removes

The host embeds the SDK in its own binary.

## Scope (inferred)

`crates/loom-cli/`, `crates/loom-intake-slice/src/run.rs` (executor selection), the CLI contract
snapshot if `story:command-invocation-contract` has landed, `website/docs/guides/system-queries.md`,
`CHANGELOG.md`.

## Not in scope

A plugin ABI; granting authority by configuration (an action that needs a capability still needs
its grant).
