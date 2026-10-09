---
format: aep.planning-md/3
id: story:plugin-host
kind: story
status: active
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
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-plugin
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: ess/domains/plugin.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/crates.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T16:33:55Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":6}}}
- {from: "proposed", to: "active", at: "2026-10-09T16:33:55Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":6}}}
---
## Why

`architecture-decision-record:plugin-hooks`: an unattended agent is a plugin of Loom. The host
runs `poll → classify → project → turn → result` per item and keeps state.

## ESS first

A `loom.plugin` domain, `ess/domains/plugin.yaml`, plus `ess/system.yaml`: `Item`, `Intent`
(`ask | request | task | find`), `Classification` (intent, hints, confidence), `Projection`
(admitted action ids, data-source names), `PluginConfig` (the `ConnectorsCliConfig`, the
`DataSource`s, objectives with weights, `classify_threshold` default 0.5 as `loom-intake-router`'s
`--threshold`, `poll_interval_seconds`), `Proposal`, `RecordLine` (item id, intent, confidence,
outcome `proposed | declined | proposed_case | unclassified | stopped`, the `{source, kind}` reads
performed, the proposal text or the proposed case), `PluginState` (cursors, handled item ids). A
plugin's own config extends `PluginConfig`. The red test is `task drift` on the spec-only commit.

## Acceptance

Crate `b10x-loom-plugin` (`crates/loom-plugin`), a `Plugin` trait with the hooks `poll`,
`classify`, `project`, `turn`, `result`, `objectives`, and `run_plugin(plugin, config, state,
stop)`. Tests use a fake plugin defined in the crate's tests, recorded model responses and the
fake `connectors` from `story:connectors-cli-reads`; no model or network call.
- The default `classify` makes one forced tool call (the `call_tool` pattern of
  `loom-intake-router`) and returns intent, hints and confidence. Test:
  `classify_returns_intent_hints_and_confidence`.
- A confidence below `classify_threshold` records `unclassified` and runs no turn. Test:
  `low_confidence_runs_no_turn`.
- The default `project` admits `source.read` and `reply.propose` for ask, find and request. Test:
  `project_admits_reads_for_ask_find_request`.
- Its data sources are a subset of `PluginConfig`'s. Test: `project_stays_within_configured_sources`.
- For task it admits nothing and records a proposed case naming the protocol `loom-intake-router`
  picks from `ProtocolCatalog::bundled()`. Test: `task_records_a_proposed_case`.
- The default `objectives` returns the config's weights. Test: `objectives_return_config_weights`.
- The turn is a thin caller of Commission's `run_until_blocked` through the public `loom_sdk` API,
  as `examples/zendesk-triage/src/triage.rs:183-232` composes it: `CanonGovernor` over
  `ProtocolCatalog::plugins()` on `inbound-answer@1`, Loom's model-driven selection, an authority
  provider and `DataSourceEffects`. Anything it needs that is crate-private today is exported from
  `loom-sdk`, not copied. Test: `turn_reaches_proposed_on_a_fixture_source`.
- The authority provider grants the capabilities `datasource.read` and `reply.propose` and refuses
  any other. Test: `authority_grants_only_the_two_capabilities`.
- `DataSourceEffects` performs `source.read {source, kind, input}` through
  `ConnectorsCli::read` and submits one `source_read` evidence per performed read, as
  `loom-intake-slice/src/clock.rs` submits clock evidence. Test: `each_read_submits_evidence`.
- It refuses a `{source, kind}` pair outside the projection, and the fake's argv log stays empty.
  Test: `effects_refuse_an_undeclared_read`.
- `reply.propose` appends the proposal to the record and submits `reply_proposed`; the fake's argv
  log gains no line. Test: `propose_writes_the_record_only`.
- Before the first model call the turn's context lists the projection's sources with their
  entities and schema. Test: `turn_context_lists_sources`.
- A record line carries the fields `RecordLine` declares, the reads included. Test:
  `record_line_carries_reads`.
- Handled ids and cursors are written through a temporary file and a rename. Test:
  `state_written_atomically`.
- A state directory inside the plugin's configured workspace roots or a checkout given in config is
  refused; the check reads paths only and runs no git command (`host_git_hardening`). Test:
  `state_inside_a_checkout_is_refused`.
- `run_plugin` polls again after `poll_interval_seconds` until `stop` is set. Test:
  `host_polls_until_stopped` (interval 0, stop after the second poll: two polls recorded).
- A second `run_plugin` with `once` over the same fixtures adds no line. Test:
  `once_twice_handles_an_item_once`.

The CLI subcommands (`b10x-loom plugin run|report`) and their tests are `story:slack-plugin`'s,
where the first registered plugin exists.

## Scope

`ess/domains/plugin.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-plugin/`
(new), `crates/loom-sdk/src/lib.rs`, `Cargo.lock`, `crates/loom-executor/tests/crate_names.rs`, `website/docs/reference/crates.md` (generated), `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`. Wave `2026-10-09-w1` also lands on `generated/rust/loom/` (`story:compaction-target-bound`), `AGENTS.md` and the new-crate files (`story:laya-selector`). The wave that merges into `main` second rebases, then runs `task generate` and `task docs-generate` and commits their output (conductor DSP-20261009-05).

## Deviations accepted by the coordinator (implementation, 2026-10-09)

- A turn is not one `run_until_blocked` call. After one granted `source.read`, Commission ends the
  Run `AwaitingApproval` because the gated actions are unchanged, though the authority allowed them
  (`crates/loom-commission/src/runtime.rs:514`). The turn starts another Run on the same case only
  when the Run performed an effect, every awaited action needs a capability the plugin's authority
  grants, and steps remain of `TURN_STEP_BUDGET` (8). Test: `a_turn_that_only_reads_stops_at_its_budget`.
  Commission's behaviour is `story:granted-gate-ends-run`.
- `loom.plugin.Item` is `loom.plugin.InboundItem`: `task no-hand-model` refuses the name beside the
  hand-written `enum Item` of `loom-executor/src/harness/wire/item.rs:137`.
