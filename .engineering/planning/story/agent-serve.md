---
format: aep.planning-md/3
id: story:agent-serve
kind: story
status: implemented
title: b10x-loom agent serve hosts the agent a spec declares, reading only allow-listed channels and sending a reply only once approved
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-check
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-agent
- confidence: cited
  path: crates/loom-cli/src/agent.rs
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-plugin-slack/src/config.rs
- confidence: cited
  path: crates/loom-plugin-slack/src/poll.rs
- confidence: cited
  path: crates/loom-plugin/src/effects.rs
- confidence: cited
  path: crates/loom-plugin/src/lib.rs
- confidence: inferred
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: docs/qualification
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: inferred
  path: website/docs/reference/cli.md
revision: 21
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T15:17:33Z", actor: "human:timo", revision: 19}
- {from: "proposed", to: "active", at: "2026-10-25T15:17:33Z", actor: "human:timo", revision: 20}
- {from: "active", to: "implemented", at: "2026-10-25T20:36:27Z", actor: "human:timo", revision: 21, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`b10x-loom agent serve --spec <FILE> --state <DIR> [--once]` hosts the agent the spec declares:
`run_plugin` with a `SpecPlugin` built from the spec, its poll restricted to `channel.allow`, a
turn per classified item on the spec's protocol through the chooser path, and `reply.send`
performed through `ConnectorsCli::invoke_write` in `approval` mode. Milestone M1 of the Kubernetes
design.

## Why

D3: `run_plugin` is Loom's one long-running loop, SIGTERM-aware, with a locked state directory
(`crates/loom-plugin/src/lib.rs:333-352`). `SlackHandler` implements only `name`, `poll`,
`classifier` and `turn_model` (`crates/loom-plugin-slack/src/lib.rs:148-176`), so a spec-driven
plugin generalises it. Today every member channel is walked and `channels` only weights the order
(`crates/loom-plugin-slack/src/walk.rs:1-9`); the allow list is new.

Q2 (orchestrator): "A if Connectors supports standing scoped approval, else C". `connectors
approvals issue` (0.54.0 help) approves "the exact reconstructed subject" of one `--input-*`, so a
proof covers one write's input: no standing approval. Hence C: `approval` mode (a human grant per
send, `b10x-loom approve`) and `none`; `auto` waits on a Connectors story.

## Acceptance

- `a_message_outside_the_allow_list_is_no_item`: a channel `conversations.list` reports and
  `channel.allow` does not name is never read (the fake `connectors` argv log holds no
  history call for it).
- `a_send_outside_the_allow_list_runs_no_command`: refused before any process starts, as an
  effect refusal with `ends_run: true` (the design's "final" needs it,
  `EffectOutcomeRefused.ends_run`).
- `a_send_without_a_proposal_is_not_admissible`.
- `a_send_is_held_until_approved`: in `approval` mode the turn ends `AwaitingApproval`; `approve`
  then `resume` sends once, with the derived idempotency key.
- `a_sixth_send_in_the_window_is_refused` with `max_per_hour: 5` (a counter in the state directory).
- `sigterm_ends_the_host_after_the_item`.
- `SpecAuthority` grants exactly `policy.capabilities` and denies every other.
- Tests use the fake `connectors` and recorded model answers; no network.
- (manual, outside CI) one run against a sandbox channel on a workstation, model route
  `anthropic/claude-haiku-5-5` as given (Q8 A); the record is cited under `docs/qualification/`.

## ESS first

The first commit adds the `loom.agent` serve scenarios to `ess/` only; they are red until
`SpecPlugin` lands.

## Not in scope

`writes.mode: auto` (needs standing scoped approval in Connectors); `http` bindings
(`story:agent-http-binding`); packaging (`story:agent-project-kubernetes`).

## Scope

## Scope

Derived 2026-10-25 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-agent` (`SpecPlugin`, `SpecAuthority`, the write effect) — cited (story § Scope); not in the tree at 0.40.0, `story:agent-check` creates it
- **Files:** `crates/loom-plugin-slack/src/poll.rs:29-41` (the walk over `member_channels`, where `channel.allow` restricts the poll) — cited (story § Why names `walk.rs:1-9`; `walk.rs:1-14` ranks, `poll.rs:29` orders)
- **Files:** `crates/loom-plugin-slack/src/config.rs:130-142` (`channels` entries, today only weights) — cited
- **Files:** `crates/loom-plugin/src/effects.rs:76-116,354-383` (`DataSourceEffects`; `performs` admits only `[READ, PROPOSE, DECLINE]` at :355; `reply.send` lands beside it) — cited (story § Scope)
- **Files:** `crates/loom-plugin/src/lib.rs:218-306` (`Plugin` trait: `poll`, `project`, `result`, `objectives`, `classifier`, `turn_model`) and `:333-352` (`run_plugin`) — cited (story § Why)
- **Files:** `crates/loom-cli/src/agent.rs` (`serve` subcommand) — cited (story § Scope); not in the tree, created by `story:agent-check`
- **Files:** `crates/loom-cli/src/lib.rs:217-236` (`Command` enum gains nothing new if `Command::Agent` already exists from agent-check; the `Agent` subcommand enum gains `Serve`) — inferred
- **Files:** `ess/domains/agent.yaml` (serve scenarios; `WritesSpec` :290-300, `ChannelSpec` :283-289, `PolicySpec` :302-306 already declared) — cited
- **Symbols:** `run_plugin` (`crates/loom-plugin/src/lib.rs:345`), `SlackHandler` (`crates/loom-plugin-slack/src/lib.rs:148-176`), `ConnectorsCli::invoke_write` (`crates/loom-connectors/src/cli/write.rs:91-97`, read-only consumer), `EffectOutcomeRefused.ends_run` (`crates/loom-commission/src/ports/connector.rs:250`, read-only), `PluginAuthority` (`crates/loom-plugin/src/authority.rs:18-20`) — cited
- **Also likely:** `crates/loom-plugin/src/turn.rs:228,276-279` — inferred, where `DataSourceEffects::new` and `&PluginAuthority` are wired into `run_until_blocked`; a spec-driven turn with `SpecAuthority` either passes through here or generalises it
- **Also likely:** `crates/loom-cli/src/main.rs:59-72` (dispatch) and `:326-350` (SIGTERM/SIGINT handler `serve` reuses) — inferred
- **Also likely:** `website/docs/reference/cli.md` — inferred, generated from the clap tree by `crates/loom-docs/src/cli.rs:69-125`, so a new subcommand regenerates it
- **Documents:** `README.md`, `AGENTS.md` (§ Plugin host :622-642), `CHANGELOG.md`, `docs/qualification/` (manual-run record) — cited
- **Confidence:** high — the story's Scope names the files and every cited line resolves; the one unknown is the shape `story:agent-check` gives `crates/loom-agent` and `loom-cli/src/agent.rs`
- **Would collide with:** any unit touching `crates/loom-plugin/src/effects.rs` or `turn.rs`, `crates/loom-plugin-slack/src/poll.rs`, the `Command`/`Agent` enums in `crates/loom-cli/src/lib.rs`, or `crates/loom-agent`
- **Safety fact:** `DataSourceEffects::performs` admits only `[READ, PROPOSE, DECLINE]` (`effects.rs:355`) and `PluginAuthority` grants only `datasource.read` and `reply.propose` (`AGENTS.md:629-630`), so a `reply.send` effect behind `SpecAuthority` is unreachable from the existing `slack-handler` turn — step 2 (`git grep -n 'READ, PROPOSE, DECLINE'`), unproven
- **Not established:** where ESS scenarios live (no `ess/domains/*.yaml` carries `scenarios:`, `ess/scenarios/` does not exist; `task conform` synthesizes from `ess/`, `Taskfile.yml:88-91`); whether `conversations.list` filtering lands in `poll.rs` or in a `SpecPlugin::poll` inside `crates/loom-agent`

## Ready

After `story:agent-check`.

## Q2 after Connectors' standing approval

Session `b10x-chef-orchestrator`, 2026-10-22: "FYI for loom-kubernetes Q2: Timo chose standing scoped
approval in connectors ("a" to decision-blocker:standing-write-approval-mode). Connectors builds it in
the wave after its current one. Until it ships, the operator agent stays in approval mode (C), as
planned. Afterwards, auto mode binds to a standing grant id. Note it on the agent-serve story; no
other change now."

- This story ships approval mode (C) only: `reply.send` is performed once per approved proposal.
- Once Connectors ships standing scoped approval, a later story adds `auto` mode, bound to a
  standing grant id. That story is not filed yet, and this story's scope does not change.
