---
format: aep.planning-md/3
id: story:plugin-host
kind: story
status: draft
title: Loom hosts a plugin in a loop with hooks around a governed run
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
- depends_on: story:inbound-answer-protocol
- depends_on: story:connectors-cli-reads
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-cli/src
- confidence: cited
  path: crates/loom-cli/tests/plugin.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-plugin
- confidence: cited
  path: ess/domains/plugin.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/crates.md
revision: 3
---
## Why

`architecture-decision-record:plugin-hooks`: an unattended agent is a plugin of Loom. The host
runs `poll → classify → project → turn → result` per item and keeps state.

## ESS first

A `loom.plugin` domain, `ess/domains/plugin.yaml`, plus `ess/system.yaml`: `Item`, `Intent`
(`ask | request | task | find`), `Classification` (intent, hints, confidence), `Projection`
(admitted action ids and data-source names), `Proposal`, `RecordLine`, `PluginState` (cursors,
handled item ids). The red test is `task drift` on the spec-only commit.

## Acceptance

Crate `b10x-loom-plugin` (`crates/loom-plugin`), a `Plugin` trait with the hooks `poll`,
`classify`, `project`, `turn`, `result`, `objectives`:
- The default `classify` makes one forced tool call (the `call_tool` pattern of
  `loom-intake-router`) and returns intent, hints and confidence. Test:
  `classify_returns_intent_hints_and_confidence` (recorded model response).
- A confidence below the threshold records `unclassified` and runs no turn. Test:
  `low_confidence_runs_no_turn`.
- The default `project` admits `source.read` and `reply.propose` and the configured sources for
  ask, find and request. Test: `project_admits_reads_for_ask_find_request`.
- For task it admits nothing, and the result is a proposed case naming the protocol
  `loom-intake-router` picks from `ProtocolCatalog::bundled()`. Test:
  `task_records_a_proposed_case` (recorded router response).
- The default `objectives` returns the config's weights. Test: `objectives_return_config_weights`.
- The turn composes what `loom-intake-slice` composes (`src/run.rs`): Commission's
  `run_until_blocked` with `CanonGovernor` over `ProtocolCatalog::plugins()` on `inbound-answer@1`,
  Loom's model-driven loop, an authority provider that grants `source.read` and `reply.propose`
  only, and `DataSourceEffects`, an `EffectPort` in this crate. Test:
  `turn_reaches_proposed_on_a_fixture_source` (recorded model responses, fake `connectors`).
- `DataSourceEffects` performs `source.read {source, kind, input}` through
  `ConnectorsCli::read` and submits one `source_read` evidence per performed read, as
  `loom-intake-slice/src/clock.rs` submits clock evidence. Test: `each_read_submits_evidence`.
- It refuses a `{source, kind}` pair outside the projection, and nothing is invoked. Test:
  `effects_refuse_an_undeclared_read`.
- `reply.propose` appends the proposal to the record and submits `reply_proposed`; nothing leaves
  the host. Test: `propose_writes_the_record_only`.
- Before the first model call, the turn's context holds the projection's sources with their
  entities and schema (from `ConnectorsCli::sources`). Test: `turn_context_lists_sources`.
- Handled ids and cursors are written to the state directory through a temporary file and a
  rename. Test: `state_written_atomically` (a crash between write and rename leaves the old state).
- The state directory is the one the operator names; a path inside a git work tree is refused.
  Test: `state_inside_a_work_tree_is_refused`.
- `b10x-loom plugin run <name> --config <file> --state <dir> [--once]`, clap derive. Test:
  `plugin_run_once_handles_an_item_once` (run twice, one record line).
- `b10x-loom plugin report <name> --state <dir>` prints one line per proposal. Test:
  `plugin_report_prints_proposals`.

No test makes a model or network call: recorded responses only (AGENTS.md § Never).

## Scope

`ess/domains/plugin.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-plugin/`
(new), `crates/loom-cli/src/`, `crates/loom-cli/tests/plugin.rs` (new),
`website/docs/reference/cli.md` (generated), `Cargo.lock`, `crates/loom-executor/tests/crate_names.rs`, `website/docs/reference/crates.md` (generated), `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`. Wave `2026-10-09-w1` (`story:laya-selector`, a new crate) edits the same new-crate files; the wave that merges second rebases and regenerates `crates.md` (conductor DSP-20261009-05).
