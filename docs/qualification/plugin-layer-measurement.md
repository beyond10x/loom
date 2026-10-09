# The plugin layer, measured

Story `plugin-measurement`, epic `plugin-layer`, ADR `plugin-hooks`. Measured on wave
`2026-10-09-w2` at `e665529`, after the slack-handler merged. The operator asked to "see how much
code this will produce... eventually extract the generalizable parts and express in YAML
protocol+canon"; this page is the count and the list of what could move.

## Lines

`./docs/qualification/plugin-layer-measurement.sh` counts lines that are neither blank nor `//`
comments (doc comments `//!` and `///` are comments and not counted), from the repository root:

```
   307  crates/loom-plugin-slack/src/config.rs
    97  crates/loom-plugin-slack/src/lib.rs
   279  crates/loom-plugin-slack/src/poll.rs
    72  crates/loom-plugin-slack/src/walk.rs
    22  crates/loom-plugin/src/authority.rs
   137  crates/loom-plugin/src/classify.rs
   322  crates/loom-plugin/src/codec.rs
   338  crates/loom-plugin/src/effects.rs
   426  crates/loom-plugin/src/lib.rs
    33  crates/loom-plugin/src/project.rs
   234  crates/loom-plugin/src/state.rs
   419  crates/loom-plugin/src/turn.rs
   484  crates/loom-connectors/src/cli.rs

  1931  loom-plugin (crate total)
   755  loom-plugin-slack (crate total)
   484  loom-connectors/cli (crate total)
```

Outside the measured crates, the command line adds `crates/loom-cli/src/plugin.rs` (93) and
`crates/loom-cli/src/model_port.rs` (219, the bridge from catalog models to the harness model
port), and the specification adds `ess/domains/{datasource,plugin,slack}.yaml` and
`protocols/inbound-answer/1.yaml`.

Per hook, the lines of the module that implements it:

| hook | module(s) | lines |
|---|---|---|
| `poll` | `loom-plugin-slack/src/poll.rs`, `walk.rs` | 351 |
| `classify` | `loom-plugin/src/classify.rs` | 137 |
| `project` | `loom-plugin/src/project.rs` | 33 |
| `turn` | `loom-plugin/src/turn.rs`, `effects.rs`, `authority.rs` | 779 |
| `result` | inside `loom-plugin/src/lib.rs` (the host loop, 426 in all) | — |
| `objectives` | the weights in `loom-plugin-slack/src/walk.rs` (counted under `poll`) | — |

A plugin of its own (the Slack handler) is 755 lines, 307 of them configuration parsing and checks.

## What could be YAML

One row per measured file (`./docs/qualification/plugin-layer-measurement.sh --files` lists the
same 13 files).

| file | kind | what it holds | as YAML |
|---|---|---|---|
| `loom-plugin/src/lib.rs` | mechanism | the host loop: lock, retries from state, poll, classify, project, turn, record, stop | stays Rust |
| `loom-plugin/src/state.rs` | mechanism | the state directory, atomic writes, the lock, the torn-line repair | stays Rust |
| `loom-plugin/src/codec.rs` | mechanism | the JSON of record lines and state over the generated types | stays Rust (generated types already come from ESS) |
| `loom-plugin/src/turn.rs` | mechanism, with policy constants | the composition of `run_until_blocked`, the re-run rule, the model budget (8) | the budget and step limits as plugin config; the composition stays Rust |
| `loom-plugin/src/effects.rs` | mechanism | `DataSourceEffects`: projection check, CLI read, evidence submission | stays Rust |
| `loom-plugin/src/authority.rs` | policy | the two granted capabilities | derived from the protocol: grant what the plugin's protocol `requires` |
| `loom-plugin/src/classify.rs` | policy, with a mechanism | the intent labels, their descriptions, the threshold; one forced tool call | `intents: [{name, description}]`, `threshold:`; the call stays Rust |
| `loom-plugin/src/project.rs` | policy | which actions and sources each intent may use | `projection: {ask: {actions: [...], sources: [...]}, task: {route: bundled}}` |
| `loom-plugin-slack/src/config.rs` | policy | the configuration's shape and checks | already ESS (`loom.slack`); the checks become ESS constraints |
| `loom-plugin-slack/src/lib.rs` | policy | the plugin's wiring of hooks | `plugin: {source: ..., classify: ..., project: ..., protocol: inbound-answer@1}` |
| `loom-plugin-slack/src/poll.rs` | policy over a mechanism | which Slack operations, the cursor field, the unanswered rule (age, replies, reactions, subtypes, mentions first) | `source: {list: conversations.list, items: conversations.history, cursor: ts, thread: conversations.replies, unanswered: {min_age, no_reply_by_other, no_reaction_by: bot, skip_subtypes, first: mentions}}`; the paging and request shaping stay Rust |
| `loom-plugin-slack/src/walk.rs` | policy | the walk order: objective weight, staleness, volume, seeded draw | `walk: {order: [objective_weight, staleness, volume], seed:}` |
| `loom-connectors/src/cli.rs` | mechanism | the Connectors CLI client: describe, invoke, timeouts, refusals | stays Rust |

## Reading

- Mechanism: 6 files, 2,223 lines (`lib.rs`, `state.rs`, `codec.rs`, `turn.rs`, `effects.rs`,
  `cli.rs`). It is written once and serves every plugin.
- Policy: 7 files, 947 lines (`authority.rs`, `classify.rs`, `project.rs`, and the four Slack
  files). All of it has a YAML shape in the table. `story:yaml-only-plugin` starts from this list:
  a plugin declared as one YAML document would replace the 755 lines of `loom-plugin-slack` and the
  192 lines of policy in `loom-plugin`.
- `story:reference-enrichment` adds one hook (`enrich`) between `project` and `turn`; on this
  measure it belongs to the mechanism side, with its per-reference-kind declarations on the policy
  side.
