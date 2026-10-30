---
format: aep.planning-md/3
id: story:connectors-cli-invoke-form
kind: story
status: implemented
title: ConnectorsCli invokes through Connectors v0.60.0's invoke <connector>/<instance> form
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-30T08:00:35Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
## Outcome

`ConnectorsCli` reads and its governed write invoke through Connectors v0.60.0's
`connectors invoke <connector>[/<instance>] <operation>` form, never the removed
`operations invoke`. The fake `connectors` the tests run refuses the removed forms as v0.60.0 does
(a parse refusal, exit status 2).

## Why

D264 ride-along. Connectors v0.60.0 removed `operations invoke`, `invoke --endpoint`,
`describe --endpoint` and `serve`; each is now a parse refusal. `git grep` in Loom at 0.51.0 finds
`operations invoke` in `crates/loom-connectors/src/cli.rs` and `src/cli/write.rs`, so every read
and write through the operator's `connectors` fails against v0.60.0.

## Acceptance

- `crates/loom-connectors/tests/connectors_cli.rs` and `connectors_cli_write.rs`: the logged argv
  of each invoke is the `invoke <target> <operation>` form with the same `--connection`,
  `--schema`, `--revision`, `--input-stdin`, `--approval-file` and `--idempotency-key` it carried,
  and no argv names `operations invoke`.
- Every existing read and write test passes against the updated fake.

## ESS first

Decided by the unit: none when the configuration Loom already holds names the target; otherwise
the `loom.datasource` change lands first in `ess/domains/datasource.yaml`.

## Scope

- `crates/loom-connectors/src/cli.rs`, `src/cli/write.rs`
- `crates/loom-connectors/tests/fixtures/connectors/fake-connectors`, its tests
- AGENTS.md § Connectors, `ess/domains/datasource.yaml` comments, CHANGELOG.md
