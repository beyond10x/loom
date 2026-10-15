---
format: aep.planning-md/3
id: architecture-decision-record:model-ports-in-sdk
kind: architecture-decision-record
status: proposed
title: The llm model port moves into the SDK, and a Claude Code login is used only through the claude command or llm
relations:
- decides: story:sdk-model-ports
revision: 1
---
## Decision (proposed)

The llm-backed `ModelPort` moves from `b10x-loom-cli` into the SDK (`loom_sdk::llm::LlmModelPort`),
and the CLI uses the SDK's. Loom adds a model port that runs on an operator's Claude Code login only
by invoking the `claude` command, which reads its own login; Loom code reads no credential file.
A port that reads a Claude Code login directly is llm's to build (a credential reader beside its
Codex login reader), and Loom would take it through llm.

## Context

- The port an embedder needs exists only inside the CLI (`crates/loom-cli/src/model_port.rs:33`).
- Loom does not own provider wires or credentials, which are llm's (`AGENTS.md:29-31`), and never
  reads a credential file (`AGENTS.md:751`); the Codex login is read and renewed by llm, never by
  Loom code (`AGENTS.md:685-687`).
- A host asked for a model port on an existing Claude Code login, shipped in the SDK
  (`story:sdk-model-ports`).

## Consequences

- The SDK gains a dependency on the llm crates the CLI uses today; the SDK stability page names the
  new paths, and the CLI's port type is removed (`Breaking:` only for code that named the CLI's
  module, which is not a stable surface).
- The command port runs a subprocess: single-attempt, bounded output, a missing command is a
  typed refusal.

## Alternatives

- Loom reads the Claude Code login itself. Rejected: contradicts the credential boundary above.
- Leave both ports to hosts. Rejected: every host re-implements the same port.

## Source

story:sdk-model-ports.
