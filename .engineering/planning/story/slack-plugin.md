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
  path: crates/loom-cli/src
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
  path: website/docs/reference/crates.md
revision: 3
---
## Why

The first plugin (the operator's approved plan (conductor `charters/loom.md`, DSP-20261009-05)): a Slack walk under long-horizon objectives ("learn about dev stack",
"help people with cheap lookups", "respond in slack when being tagged"), read-only.

## ESS first

A `loom.slack` domain, `ess/domains/slack.yaml`, plus `ess/system.yaml`: `SlackConfig` (the
`DataSource` of the Slack adapter, bot user id, objectives and their weights, `min_age_minutes`,
seed, and the data sources a turn may read). The red test is `task drift` on the spec-only commit.

## Acceptance

Crate `b10x-loom-plugin-slack` (`crates/loom-plugin-slack`), polling through `ConnectorsCli`
(`conversations.list`, `conversations.history`, `conversations.replies`). Fixtures: a fake
`connectors` answering from JSON files, recorded model responses.
- `poll` lists only channels the bot is a member of. Test: `poll_lists_member_channels_only`.
- With one seed, two polls over the same fixtures pick the same channel order; another seed may
  differ. Test: `walk_is_deterministic_per_seed`.
- A channel with a higher objective weight is picked first when staleness and volume are equal.
  Test: `weight_orders_the_walk`.
- `conversations.history` is called with `oldest` set to the stored `ts` of that channel. Test:
  `history_reads_from_the_stored_ts`.
- A message younger than `min_age_minutes` is not an item. Test: `young_message_is_not_an_item`.
- A message with a thread reply, or a reaction by the bot, is not an item. Test:
  `answered_message_is_not_an_item`.
- Bot and system messages are not items. Test: `bot_and_system_messages_are_not_items`.
- A message naming the bot's user id comes before the others. Test: `mention_comes_first`.
- A config missing the Slack data source or the bot user id is refused at start. Test:
  `config_without_bot_id_is_refused`.
- The fixture run (a channel with 5 messages: a mention asking a question, a question, an answered
  thread, a bot message, a task request; and a fixture data source `docs` with a `search`
  operation the turns read) records exactly 3 lines. Test: `fixture_run_records_three_proposals`.
- Their intents are ask (the mention), find (the question) and task. Test:
  `fixture_run_intents`.
- The two answer lines are `proposed` and each cites at least one `docs` read; the task line is a
  proposed case. Test: `fixture_answers_cite_their_reads`.
- The fake `connectors` logged only describe calls and invokes of the three Slack reads and the
  `docs` search. Test: `fixture_run_invokes_no_write`.
- A second run over the same fixtures adds no record line. Test: `second_run_records_nothing_new`.

## Scope

`ess/domains/slack.yaml` (new), `ess/system.yaml`, `generated/rust/loom/`,
`crates/loom-plugin-slack/` (new), `crates/loom-cli/src/` (registering the plugin), `Cargo.lock`, `crates/loom-executor/tests/crate_names.rs`, `website/docs/reference/crates.md` (generated), `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`.
After `story:plugin-host` (shared `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-cli/src/`
and the new-crate files). Wave `2026-10-09-w1` (`story:laya-selector`, a new crate) edits the same new-crate files; the wave that merges second rebases and regenerates `crates.md` (conductor DSP-20261009-05).
