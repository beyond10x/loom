---
format: aep.planning-md/3
id: story:substrate-0-8-pin
kind: story
status: draft
title: Loom's confined runner runs on the newest Substrate release
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Loom's confined runner builds and runs on the newest Substrate release (0.8.0 or later), host and
wire pinned together, and every confinement test still passes.

## Why

`b10x-substrate-host` and `b10x-substrate-wire` are pinned at `0.7.18`
(`crates/loom-intake-slice/Cargo.toml:31-32`); Substrate `0.8.0` is tagged. The aperture, working
directory and writable-root stories need the newer Substrate before Loom can expose them.

## Acceptance

- `crates/loom-intake-slice/Cargo.toml` names the newest Substrate tag for both crates, one rev.
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

Ready now. Folded into the 0.34.0 Substrate item when the Substrate tag the orchestrator names
arrives.
