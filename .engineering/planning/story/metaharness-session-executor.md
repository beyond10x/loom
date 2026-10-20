---
format: aep.planning-md/3
id: story:metaharness-session-executor
kind: story
status: draft
title: Loom hosts the metaharness session executor for harness, Claude Code and Codex
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:delete-harness-port
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-sdk/Cargo.toml
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: inferred
  path: crates/loom-session-executor
- confidence: inferred
  path: ess/domains/session.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 7
---
## Outcome

Loom hosts the `AgentExecutor` that runs an agent session through metaharness (harness, Claude Code
or Codex) in a scratch copy of the workspace and returns the session's change as a
`ProposedAction`, which Commission revalidates and an effect applies (ADR 0098 points 2 and 3). The
workspace itself is untouched until the effect applies. takt builds the executor first as a
standalone crate with no takt-only dependency (ADR 0099 point 3); this story moves that crate into
Loom and exports it from loom-sdk. Loom depends on metaharness; metaharness never depends on
loom-sdk. No crate in Loom depends on metaharness today (`Cargo.lock`, 0 matches, 2026-10-20).

## Why

The session executor is agent work, which ADR 0098 sends through metaharness, and Loom is where an
application finds it. takt's Codex executor extends the same crate.

## Acceptance

- `crates/loom-session-executor/tests/proposes.rs::a_session_change_returns_as_a_proposed_action`:
  over a recorded metaharness session on a fixture workspace, the executor returns
  `ProposedAction` carrying the changed files, and the fixture workspace's bytes are unchanged.
- `…::a_proposal_on_a_moved_case_is_refused_at_revalidation`: run through `run_until_blocked`, a
  proposal whose case moved while the session ran is refused and the effect is not invoked.
- `…::the_effect_applies_the_admitted_change`: with a test effect, the admitted proposal's files
  reach the workspace only through the effect.
- `…::the_session_trace_is_never_evidence`: the step's observation carries the trace reference and
  no evidence record is submitted for it.
- `diff -r` of the crate's `src/` against takt's at the commit `upstream-blocker:takt-session-executor-crate`
  names when it clears shows no change but the package name and its path dependencies.
- `crates/loom-sdk/tests/host_ports.rs` reaches the executor through `loom_sdk`, and the stability
  page lists it.
- Loom's graph gains metaharness through the new crate only: `cargo tree --locked -i` of the
  metaharness library package in Loom's workspace lists `loom-session-executor` (and `b10x-loom-sdk`
  through it) and no other Loom package; today `Cargo.lock` has no metaharness entry.
- `task check` exits 0, with no live model call.

## ESS first

The proposal shape (the change a session returns) is declared in Loom's `ess/` (a new
`loom.session` domain, inferred name) before the crate moves; the red step is the conformance
scenario synthesized from it.

## Depends on

- takt's standalone session-executor crate (`upstream-blocker:takt-session-executor-crate`).
- `story:delete-harness-port`, which edits `README.md` and the loom-sdk files first and itself lands
  after every other story that edits `host_ports.rs`, `crates/loom-sdk/src/lib.rs`, the stability
  page or `Cargo.lock` (`story:engineering-protocols-090`, `story:governor-exposes-canon-decision`,
  `story:sdk-confines-any-child`, `story:authority-sees-the-request`, `story:agent-executor-host-port`,
  `story:chooser-executor-for-plugin-turns`).

## Not in scope

- Governing a session per tool call: that waits on metaharness (ADR 0098, Consequences).
- Changes inside metaharness or takt.

## Scope

Crates: a new `loom-session-executor` crate (name inferred), `loom-sdk`.

- `crates/loom-session-executor/` (new), `Cargo.toml` (workspace members), `Cargo.lock`
- `crates/loom-sdk/Cargo.toml`, `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/tests/host_ports.rs`
- `website/docs/reference/sdk-stability.md`
- `ess/domains/session.yaml` (new, inferred), `ess/system.yaml`, `generated/rust/loom/`
- `CHANGELOG.md`, `AGENTS.md`, `README.md`
