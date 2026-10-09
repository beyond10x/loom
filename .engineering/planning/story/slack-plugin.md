---
format: aep.planning-md/3
id: story:slack-plugin
kind: story
status: draft
title: The slack-handler plugin finds unanswered Slack messages and proposes answers
relations:
- decomposes: epic:plugin-layer
- serves: vision:O1
- depends_on: story:plugin-host
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
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src
- confidence: cited
  path: crates/loom-cli/tests/plugin.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-plugin-slack
- confidence: cited
  path: ess/domains/slack.yaml
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
revision: 6
---
## Why

The first plugin (the operator's approved plan (conductor `charters/loom.md`, DSP-20261009-05)): a Slack walk under long-horizon objectives ("learn about dev stack",
"help people with cheap lookups", "respond in slack when being tagged"), read-only. The operator:
"find recent unanswered - basically random walk", which the age rule and the seeded walk below
implement.

## ESS first

A `loom.slack` domain, `ess/domains/slack.yaml`, plus `ess/system.yaml`: `SlackConfig`, extending
`PluginConfig` with the Slack adapter alias and connection id, the three read operation ids
(`conversations.list`, `conversations.history`, `conversations.replies`), the bot user id,
`min_age_minutes` and the seed. The red test is `task drift` on the spec-only commit.

## Acceptance

Crate `b10x-loom-plugin-slack` (`crates/loom-plugin-slack`), polling through
`ConnectorsCli::invoke_read` with the three operation ids. Fixtures: the fake `connectors` of
`story:connectors-cli-reads` answering from JSON files, recorded model responses.
- `poll` lists only channels the bot is a member of. Test: `poll_lists_member_channels_only`.
- With one seed, two polls over the same fixtures pick the same channel order. Test:
  `walk_is_deterministic_per_seed`.
- With equal staleness and volume, the channel with the higher objective weight is picked first.
  Test: `weight_orders_the_walk`.
- `conversations.history` is called with `oldest` set to the stored `ts` of that channel. Test:
  `history_reads_from_the_stored_ts`.
- A message younger than `min_age_minutes` is not an item. Test: `young_message_is_not_an_item`.
- A message with a thread reply is not an item. Test: `replied_message_is_not_an_item`.
- A message with a reaction by the bot is not an item. Test: `bot_reacted_message_is_not_an_item`.
- A bot message is not an item. Test: `bot_message_is_not_an_item`.
- A system message (a `subtype` such as a join) is not an item. Test:
  `system_message_is_not_an_item`.
- A message naming the bot's user id comes before the others. Test: `mention_comes_first`.
- A config without the bot user id is refused at start. Test: `config_without_bot_id_is_refused`.
- A config without the Slack adapter or connection is refused at start. Test:
  `config_without_slack_source_is_refused`.
- The fixture run (a channel with 5 messages: a mention asking a question, a question, an answered
  thread, a bot message, a task request; a fixture data source `docs` with a `search` operation)
  records exactly 3 lines. Test: `fixture_run_records_three_proposals`.
- Their intents are ask (the mention), find (the question) and task. Test: `fixture_run_intents`.
- The two answer lines are `proposed` and each lists at least one `docs` read; the task line is
  `proposed_case`. Test: `fixture_answers_cite_their_reads`.
- The fake `connectors` logged only describe calls and invokes of the three Slack reads and the
  `docs` search. Test: `fixture_run_invokes_no_write`.
- A second run over the same fixtures adds no record line. Test: `second_run_records_nothing_new`.
- `b10x-loom plugin run slack-handler --config <file> --state <dir> --once` runs the fixture run
  through the CLI (clap derive in `b10x-loom-cli`). Test: `plugin_run_slack_handler_once`.
- `b10x-loom plugin report slack-handler --state <dir>` prints one line per proposal. Test:
  `plugin_report_prints_proposals`.
- An unknown plugin name is refused naming the registered ones. Test:
  `plugin_run_unknown_name_is_refused`.

## Scope

`ess/domains/slack.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`,
`crates/loom-plugin-slack/` (new), `crates/loom-cli/src/`, `crates/loom-cli/Cargo.toml`,
`crates/loom-cli/tests/plugin.rs` (new), `website/docs/reference/cli.md` (generated), `Cargo.lock`, `crates/loom-executor/tests/crate_names.rs`, `website/docs/reference/crates.md` (generated), `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`.
Wave `2026-10-09-w1` also lands on `generated/rust/loom/` (`story:compaction-target-bound`), `AGENTS.md` and the new-crate files (`story:laya-selector`). The wave that merges into `main` second rebases, then runs `task generate` and `task docs-generate` and commits their output (conductor DSP-20261009-05).

## Carried from story:plugin-host

The production turn model is a harness `ModelPort`, while the CLI builds `llm_core` models; the
bridge between the two is this story's, where `b10x-loom plugin run` first runs a real model.
Test: `plugin_run_builds_the_turn_model_from_the_catalog` (recorded responses, no model call).
