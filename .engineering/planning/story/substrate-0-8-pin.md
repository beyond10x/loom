---
format: aep.planning-md/3
id: story:substrate-0-8-pin
kind: story
status: implemented
title: Loom's confined runner runs on the newest Substrate release
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T01:01:34Z", actor: "human:timo", revision: 3, correlation: "wave-2026-10-23-w1"}
- {from: "proposed", to: "active", at: "2026-10-23T01:01:34Z", actor: "human:timo", revision: 4, correlation: "wave-2026-10-23-w1"}
- {from: "active", to: "implemented", at: "2026-10-23T02:11:10Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

Loom's confined runner builds and runs on the Substrate release 0.10.0 (tag `0.10.0`, commit `105cf718`), host and
wire pinned together, and every confinement test still passes.

## Why

`b10x-substrate-host` and `b10x-substrate-wire` are pinned at `0.7.18`
(`crates/loom-intake-slice/Cargo.toml:31-32`); Substrate `0.10.0` is tagged. The aperture, working
directory and writable-root stories need the newer Substrate before Loom can expose them.

## Acceptance

- `crates/loom-intake-slice/Cargo.toml` names Substrate `0.10.0` for both crates, one rev (`105cf718bc6a5314486c78f5fd93fabed290e30e`).
- `cargo test -p b10x-loom-intake-slice --locked --test confined_child` passes on a host with
  bubblewrap and a delegated user systemd scope, the tests behind the cleared
  `upstream-blocker:substrate-inherited-workspace-fds` included.
- AGENTS.md § Intake and the command line names the new version where it says `0.7.18`.
- `task check` exits 0.

## ESS first

No behaviour change Loom declares: a dependency move. A break Substrate makes in a type Loom maps is
named in CHANGELOG as `**Breaking:**` and its test is the red step.

## Not in scope

Exposing a new Substrate feature (`story:several-egress-apertures`, `story:child-workdir`,
`story:writable-host-root`).

## Scope

Crates: `loom-intake-slice`. `Cargo.toml`, `Cargo.lock`, `crates/loom-intake-slice/src/confinement.rs`
and `confinement/child.rs` where the wire changed, AGENTS.md, README.md, CHANGELOG.md.

## Ready

Ready now. Retargeted to Substrate 0.10.0 when wave 2026-10-23-w1 opened (orchestrator, 2026-10-23).
