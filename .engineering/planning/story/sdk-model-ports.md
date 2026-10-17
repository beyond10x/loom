---
format: aep.planning-md/3
id: story:sdk-model-ports
kind: story
status: implemented
title: The SDK ships the llm-backed model port and a model port on a Claude Code login through the claude command
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-cli/src/model_port.rs
- confidence: inferred
  path: crates/loom-executor/src/harness/wire
- confidence: inferred
  path: crates/loom-sdk
- confidence: inferred
  path: website/docs/reference
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-16T16:35:01Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-16T16:35:02Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-17T00:12:46Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A host embedding the SDK gets two model ports from `loom_sdk` without writing its own:
`LlmModelPort`, the llm-backed port the CLI uses today, moved from `b10x-loom-cli` into the SDK;
and a port that runs on an operator's existing Claude Code login by running the `claude` command
(`claude -p --output-format json`) and reporting the usage that command returns. Loom code reads no
credential for either (see `architecture-decision-record:model-ports-in-sdk`).

## Acceptance

`a_host_runs_a_governed_loop_on_the_sdk_command_port` (`crates/loom-sdk/tests/`): an external-path
test builds `loom_sdk::loom::wire::ClaudeCodePort` over a stand-in `claude` executable on `PATH`
that prints a recorded JSON result, runs one governed loop turn with it, and the turn's usage equals
the recorded usage; and `loom_sdk::llm::LlmModelPort` is the type `b10x-loom` uses (the CLI no
longer defines its own).

## Why (Loom 0.22.0)

- The llm-backed port lives in the CLI crate (`crates/loom-cli/src/model_port.rs:33`,
  `impl ModelPort for LlmModelPort` at `:186`), so an embedder cannot use it.
- The SDK's wires take an API key or an OAuth bearer through a `SecretResolver` the embedder
  injects (`AGENTS.md:687-688`); the CLI's `--catalog` path supplies no credential resolver and
  refuses a credentialed account (`crates/loom-cli/src/model_catalog.rs:8-9`).
- Loom owns no provider credentials (`AGENTS.md:29-31`) and never reads a credential file
  (`AGENTS.md:751`), so a reader of a Claude Code login cannot be Loom code; it belongs in llm, or
  the login is used through the `claude` command, which reads it itself.

## Sketch

`loom_sdk::llm::LlmModelPort` (moved, the CLI depends on it);
`loom_sdk::loom::wire::ClaudeCodePort::new(model, cwd)`: one turn is one `claude -p` invocation,
single-attempt like every model port (Loom owns retries), its JSON usage mapped to the turn's usage;
a missing or failing command is a typed refusal naming it. A Claude Code credential reader, if
wanted, is requested from llm, not built here.

## Workaround it removes

The host implements `ModelPort` over `claude -p` itself.

## Scope (inferred)

`crates/loom-cli/src/model_port.rs` (moved), `crates/loom-sdk/`, `crates/loom-executor/src/harness/wire/`
(new port), `website/docs/reference/` (SDK stability page), `AGENTS.md`, `CHANGELOG.md`.

## Not in scope

Reading any credential file in Loom; tool calls executed by the `claude` command itself (the port
returns the model's turn; Loom executes tools).
