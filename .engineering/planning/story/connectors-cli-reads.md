---
format: aep.planning-md/3
id: story:connectors-cli-reads
kind: story
status: active
title: Connectors reads run through a plain CLI client
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-connectors/Cargo.toml
- confidence: cited
  path: crates/loom-connectors/src/cli.rs
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_cli.rs
- confidence: cited
  path: crates/loom-connectors/tests/fixtures
- confidence: cited
  path: ess/domains/datasource.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T16:08:17Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":6}}}
- {from: "proposed", to: "active", at: "2026-10-09T16:08:17Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":6}}}
---
## Why

The plugin reads Slack and other data sources through Connectors, both outside a run (the poll)
and inside one (a turn's `source.read`). On an operator's machine Connectors is a CLI with keyring
custody (`connectors operations describe/invoke --output json`), as `examples/zendesk-triage`
reads it (`src/zendesk.rs:252-476`). `ConnectorInvoker` cannot serve the poll: its `invoke` needs an
`AdmittedRequest` only the runtime makes, and its binding is fixed per commission. This story adds
a plain read client; the epic records the deviation from the ADR.

## ESS first

A `loom.datasource` domain, `ess/domains/datasource.yaml`, plus `ess/system.yaml`: `DataSource`
(name, adapter alias, connection id, and the operation ids for `list`, `search` and `get`, each
optional), `ReadKind` (`list | search | get`), `ConnectorsCliConfig` (program, `--config`,
`--state-dir`), `SourceEntity` (operation id, input schema, revision, from `operations describe`),
`ReadResult` (body, the audit reference when the CLI returns one). The red test is `task drift` on
the spec-only commit.

## Acceptance

Module `loom_connectors::cli`, tests in `crates/loom-connectors/tests/connectors_cli.rs` against a
fake `connectors` script that logs its argv:
- `describe` returns an operation's input schema and revision. Test:
  `describe_returns_schema_and_revision`.
- `invoke_read(adapter, connection, operation, input)` runs exactly one `operations invoke` for a
  named read operation. Test: `invoke_read_runs_one_invoke`.
- `read(source, kind, input)` uses the operation the `DataSource` declares for that kind. Test:
  `read_uses_the_declared_operation`.
- A kind the source declares no operation for is refused and starts no process. Test:
  `undeclared_kind_starts_no_process` (empty argv log).
- An operation `describe` reports with the mutation profile is refused after the describe and
  before any invoke. Test: `write_operation_is_never_invoked` (one describe line, no invoke line).
- Every argv holds only the declared flags and ids. Test: `argv_holds_only_declared_flags`.
- The fake's environment holds no token variable. Test: `environment_holds_no_token`.
- A `not_granted` answer comes back as a refusal naming the source. Test:
  `not_granted_is_a_refusal`.
- `sources()` lists the configured sources with each operation's `SourceEntity`. Test:
  `sources_lists_entities_with_schema`.
- The audit reference the CLI prints on invoke is carried in `ReadResult`; the unit checks the
  field name against `connectors operations invoke --help` and the docs and cites them. Test:
  `read_result_carries_the_audit_reference`.
- `AGENTS.md` § Connectors gains the CLI path: who calls it, that it reads no credential, that
  writes are refused.

## Scope

`ess/domains/datasource.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`,
`crates/loom-connectors/src/cli.rs` (new), `crates/loom-connectors/src/lib.rs`,
`crates/loom-connectors/Cargo.toml`, `Cargo.lock`, `crates/loom-connectors/tests/connectors_cli.rs`
(new), `crates/loom-connectors/tests/fixtures/` (the fake `connectors`, shared by the later
stories), `AGENTS.md`. Wave `2026-10-09-w1` also lands on `generated/rust/loom/` (`story:compaction-target-bound`), `AGENTS.md` and the new-crate files (`story:laya-selector`). The wave that merges into `main` second rebases, then runs `task generate` and `task docs-generate` and commits their output (conductor DSP-20261009-05).
