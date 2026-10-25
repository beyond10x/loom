---
format: aep.planning-md/3
id: story:agent-a2a
kind: story
status: active
title: 'A served agent answers A2A: its card, one turn per message, and a refusal without the bearer'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
- depends_on: story:runtime-events
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-a2a
- confidence: inferred
  path: crates/loom-a2a/Cargo.toml
- confidence: cited
  path: crates/loom-agent
- confidence: inferred
  path: crates/loom-executor/tests/crate_names.rs
- confidence: inferred
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: inferred
  path: website/docs/reference/crates.md
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T15:17:34Z", actor: "human:timo", revision: 14}
- {from: "proposed", to: "active", at: "2026-10-25T15:17:34Z", actor: "human:timo", revision: 15}
---
## Outcome

A served agent whose spec declares `expose.a2a` answers A2A: an agent card listing the spec's
skills, `message/send` running one turn of the same plugin and returning its record, task status
from `RuntimeEvent`, and `/ready` and `/live` for the pod's probes. A request without the bearer
from the agent's Secret is refused. Milestone M5 of the Kubernetes design, in a new adapter crate
`loom-a2a`.

## Why

Q7 A (orchestrator): Loom owns the adapter, over a2a's generated server pinned by tag
(`a2a/generated/rust/a2a/src/bin/a2a-agent-server.rs` in the a2a checkout), until a2a ships a
hand-written server crate Loom links.

## Acceptance

- `agent_card_lists_the_spec_skills`.
- `a_message_runs_one_turn_and_returns_its_record`.
- `an_unauthenticated_request_is_refused`, and a wrong bearer the same way.
- Task status follows the turn's `RuntimeEvent`s.
- Tests on a loopback port with recorded model answers; no network.
- (manual) a local Claude Code session reaches it through a port-forward and gets a sourced answer.

## ESS first

The first commit adds the A2A scenarios to `ess/domains/agent.yaml` only; they are red until
`loom-a2a` lands.

## Not in scope

Mandate authentication (later); the MCP read proxy (M6, waits on Connectors' inbound MCP server).

## Scope

## Scope

Derived 2026-10-25 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-a2a` (new adapter: card, `message/send`, task status, `/ready`, `/live`, bearer check) — cited (story § Scope)
- **Files:** `crates/loom-agent` (serve opens the listener when `expose.a2a` is set) — cited (story § Scope); not in the tree at 0.40.0
- **Files:** `ess/domains/agent.yaml:308-337` (`A2aCard`, `A2aExposure`, `ExposeSpec`; `expose` is `Optional` at :437) — cited
- **Files:** `crates/loom-a2a/Cargo.toml` (the a2a pin: generated server `a2a` 1.0.0, `a2a-agent-server` bin, from the a2a repository's `generated/rust/a2a/src/bin/a2a-agent-server.rs`, latest tag there 0.3.0) and `Cargo.lock` — inferred; the story writes "`Cargo.toml`", but pins are per crate here
- **Symbols:** `RuntimeEvent` and `EventSink` (`crates/loom-commission/src/ports/events.rs:23-51`), read-only consumer — cited
- **Also likely:** `crates/loom-plugin/src/turn.rs:276` — inferred, `run_until_blocked` is called there; task status from `RuntimeEvent` needs an `EventSink` threaded through the plugin turn, which today passes none
- **Also likely:** `crates/loom-executor/tests/crate_names.rs:35-70` — inferred, the registry every workspace crate is listed in; a new crate fails it until added
- **Also likely:** `website/docs/reference/crates.md` — inferred, generated from `cargo metadata` by `crates/loom-docs/src/crates.rs:1-37`, so a new crate regenerates it
- **Documents:** `README.md` (crate table :362), `AGENTS.md`, `CHANGELOG.md` — cited
- **Confidence:** medium — the new crate is disjoint by construction, but the serve wiring in `crates/loom-agent` and the event-sink seam in `loom-plugin/src/turn.rs` are inferred
- **Would collide with:** any unit touching `crates/loom-agent`'s serve entry, `crates/loom-plugin/src/turn.rs`, or adding a workspace crate (`crates/loom-executor/tests/crate_names.rs`, `website/docs/reference/crates.md`)
- **Safety fact:** `AgentSpec.expose` is `Optional<ExposeSpec>` (`ess/domains/agent.yaml:437`) and no existing crate depends on `loom-a2a`, so a spec without `expose.a2a` opens no listener and no existing command links the adapter — step 2, unproven; the bearer refusal before any turn is step 1, stated only
- **Not established:** which a2a tag to pin (the a2a repository's newest tag is 0.3.0 while its generated crate says `version = "1.0.0"`); whether the generated crate is consumable as a git dependency (it sits under `generated/`, which Loom's own workspace excludes for its own generated crates, `Cargo.toml:4`); how the bearer secret reaches the process (no env name in `A2aExposure`, `agent.yaml:318-326`)

## Ready

After `story:agent-serve` and `story:runtime-events`.
