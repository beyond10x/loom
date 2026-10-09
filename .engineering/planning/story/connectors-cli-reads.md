---
format: aep.planning-md/3
id: story:connectors-cli-reads
kind: story
status: draft
title: Connectors reads run through the connectors CLI behind ConnectorInvoker
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
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
revision: 3
---
## Why

The plugin reads Slack and other data sources through Connectors, both outside a run (the poll)
and inside one (a turn's `source.read`). On an operator's machine Connectors is a CLI with keyring
custody (`connectors operations describe/invoke --output json`), as `examples/zendesk-triage`
reads it (`src/zendesk.rs:252-476`). `ConnectorInvoker` cannot serve the poll: its `invoke` needs
an `AdmittedRequest` only the runtime makes (design critic, round 1). So this story adds a plain
read client; the effect port that uses it inside a turn is `story:plugin-host`'s.

## ESS first

A `loom.datasource` domain, `ess/domains/datasource.yaml`, plus `ess/system.yaml`: `DataSource`
(name, adapter alias, connection id, and the operation ids for `list`, `search` and `get`, each
optional), `ReadKind` (`list | search | get`), `ConnectorsCliConfig` (program, `--config`,
`--state-dir`), `SourceEntity` (operation id, input schema, revision, from `operations describe`).
The red test is `task drift` on the spec-only commit (the generated Rust under
`generated/rust/loom/` lags the new domain).

## Acceptance

- `loom_connectors::cli::ConnectorsCli` runs `operations describe` for an operation and returns
  its input schema and revision. Test: `describe_returns_schema_and_revision` (a fake `connectors`
  script under `tests/fixtures/`).
- `ConnectorsCli::read(source, kind, input)` runs exactly one `operations invoke` with the
  operation the `DataSource` declares for that kind, its schema and revision, and the source's
  connection id. Test: `read_invokes_once_with_the_declared_operation` (asserts the fake's argv
  log holds one invoke line).
- A kind the source declares no operation for is refused, and no `connectors` process starts.
  Test: `undeclared_kind_starts_no_process` (the argv log is empty).
- An operation `operations describe` reports as a write (mutation profile) is refused after the
  describe and before any invoke. Test: `write_operation_is_never_invoked` (the argv log holds the
  describe line and no invoke line).
- No argv carries anything but the declared flags and ids. Test: `argv_carries_no_credential`
  (every logged argv matches the allowed shapes; the fake's environment is checked for no token
  variable).
- A `not_granted` answer comes back as a refusal naming the source. Test:
  `not_granted_is_a_refusal`.
- `sources()` lists the configured sources with their `SourceEntity` from describe. Test:
  `sources_lists_entities_with_schema`.

## Scope

`ess/domains/datasource.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`,
`crates/loom-connectors/src/cli.rs` (new), `crates/loom-connectors/src/lib.rs`,
`crates/loom-connectors/tests/connectors_cli.rs` (new), `crates/loom-connectors/tests/fixtures/`,
`AGENTS.md` (§ Connectors: a second, CLI path).
