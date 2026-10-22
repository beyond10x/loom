---
format: aep.planning-md/3
id: story:agent-serve
kind: story
status: draft
title: b10x-loom agent serve hosts the agent a spec declares, reading only allow-listed channels and sending a reply only once approved
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-check
revision: 2
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

`crates/loom-agent` (`SpecPlugin`, `SpecAuthority`, the write effect), `crates/loom-plugin-slack`
(the allow list in the poll), `crates/loom-plugin/src/effects.rs` (`reply.send` beside
`DataSourceEffects`), `crates/loom-cli/src/agent.rs`, `ess/domains/agent.yaml` (scenarios),
README.md, AGENTS.md, CHANGELOG.md.

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
