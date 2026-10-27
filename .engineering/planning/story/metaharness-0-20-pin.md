---
format: aep.planning-md/3
id: story:metaharness-0-20-pin
kind: story
status: implemented
title: Loom builds against metaharness 0.20.0, so a Codex session off the pin proposes its change
summary: H9 advisory since metaharness 0.20.0; the dead H9 arms go; ESS pin 0.70.0
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T03:20:23Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-10-27T03:20:24Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-10-27T03:44:31Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
# Story: metaharness-0-20-pin

## Outcome

Loom builds against metaharness 0.20.0 (tag `0.20.0`, `f982a69b`), in which H9 (the vendor's
version against the adapter's pin) is an advisory row on every kind. A Codex session written by a
codex outside the pin therefore proposes its change: its H9 `gap` is kept in
`SessionReport::rows` and holds nothing back. A protocol error and a gating `gap` still hold the
proposal back as before.

## Why

Operator, 2026-10-27, relayed by the coordinating session: "Another defect is that currently my
current codex version is just not working together with takt because it, for whatever reason, is
doing some weird version check, which I think doesn't make much sense because we're using codex via
metaharness, and metaharness should just use that protocol from codex and just work. There
shouldn't be a hard failure due to that."

Reproduced 2026-10-27 with takt on loom 0.43.0 and metaharness 0.19.0: a takt `run --harness codex`
whose child resolved codex 0.164.0 spent about 46k tokens, then stopped "the session proposed
nothing: the session's hermetic floor has a gating gap: H9 Gap: the vendor is 0.164.0 and the
adapter is pinned to ["0.153.4", "0.160.0"]", exit 5. The hold was Loom's: the Codex arm of the
session executor's floor held any H9 that was not `ok`. metaharness 0.20.0
(`story:off-pin-vendor-version-is-advisory` there) makes H9 advisory; this story takes it, removes
the now dead H9 arms and moves the ESS pin to 0.70.0 as the release rule asks. Decision D71,
`approval-record:metaharness-0-20-pin-d71`.

## Acceptance

- `crates/loom-session-executor/Cargo.toml` names `metaharness` and `metaharness-b10x` at tag
  `0.20.0`; `Cargo.lock` resolves both to `f982a69b`.
- `a_codex_session_outside_the_pin_proposes_its_change` (`crates/loom-session-executor/tests/kinds.rs`):
  a Codex session at 0.999.0 proposes its change, is recorded `Codex`, its report's rows hold
  `H9 gap` and its floor no H9 line. It replaces `a_codex_session_outside_the_pin_is_held_by_h9`.
- The Claude and Codex arms of the floor in `crates/loom-session-executor/src/lib.rs` name no H9.
- `cargo test -p b10x-loom-session-executor --locked` passes and `task check` passes in CI.
- End to end: takt patched onto this tree, `takt run --harness codex` with a child codex at
  0.164.0 reaches `ApprovalRequired (repository.merge)` past step 2.
- The ESS pin is 0.70.0 in the three `ess-inputs.yaml`, the conformance crates, `check.yml`,
  `pages.yml`, README.md and AGENTS.md.

## Scope

- `crates/loom-session-executor/Cargo.toml`, `Cargo.lock` — cited
- `crates/loom-session-executor/src/lib.rs`, `crates/loom-session-executor/tests/kinds.rs` — cited
- CHANGELOG.md, README.md, AGENTS.md — cited
- the ESS pin files named above — cited
