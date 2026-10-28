---
format: aep.planning-md/3
id: story:child-launches-through-confinement-adapters
kind: story
status: draft
title: run_child and spawn_child launch through Substrate's confinement adapters, not HostDriver directly
summary: 'ADR 0109 decision 6, second half for loom: launches go through the contract; waits on the linux adapter''s scoped write'
tags:
- adr-0109
relations:
- decomposes: epic:loom-sdk-for-products
revision: 5
---
## Outcome

`SubstrateRunner::run_child` and `spawn_child` launch through Substrate's confinement contract
(`substrate_host::confinement`, ADR 0109) and the adapter it selects, not by building
`HostDriver` requests themselves. Writes stay scoped to the paths the host names, and each launch
records the adapter and facts it ran under, as `tests.run` does.

## Why

ADR 0109 decision 6: "loom-sdk and takt consume Substrate only and delete their own confinement
code". story:substrate-0-12-pin moved admission onto the `linux` adapter's facts
(`linux::proof`), but launches still open `HostDriver` directly (`crates/loom-intake-slice/src/confinement.rs`,
`HostDriver::open`) with a `Scoped` workspace, because the `linux` adapter in Substrate 0.13.0
serves the workspace read-write only. The adapter route waits on Substrate's story
"linux adapter serves a scoped workspace write" (requested 2026-10-27).

## Acceptance

1. `run_child` and `spawn_child` reach the driver only through the contract's adapter; no
   `HostDriver::open` remains outside it.
2. The delegated cases in `tests/confinement.rs` and `tests/confined_child.rs` still hold,
   including the scoped `target` write and a refused write outside it.
3. `task check` passes.

## Waits on

Read 2026-10-28 against Substrate 0.14.0 (tag commit `0b9d50c6`), which Loom pins; set 2026-10-28
from substrate-0109's message relayed by session `b10x-chef-orchestrator` (D166).

Waits on Substrate's story:adapter-stdin-pipe and story:adapter-live-output; story:adapter-live-output
depends on story:adapter-stdin-pipe. Substrate archived its egress story under D160.

- Delivered: the `linux` adapter serves `workspace_access: Scoped` on a host whose probe proved
  `exec.workspace-scoped-write` (Substrate 0.14.0 CHANGELOG, `### Added`). The scoped `target`
  write no longer needs `HostDriver::open`.
- Not delivered, planned: standard input through a confinement adapter. The adapter surface takes
  `Command { argv, env }` only and every adapter starts the child with standard input null
  (`substrate-host` `confinement/mod.rs` `ConfinementAdapter`, `local.rs:259`, `systemd.rs:434`,
  `macos.rs:175`). Substrate answers with its story:adapter-stdin-pipe (substrate PR 189, head
  `d1d1deb3`, not merged): adapter fact `stdin_pipe` (design 32), wave 1 on `local`, `linux` and
  `systemd`; `pod` gets no standard input.
- Not delivered, planned: a live output stream. Output is read after the run (`output`).
  Substrate answers with its story:adapter-live-output (same PR), which depends on
  story:adapter-stdin-pipe: adapter fact `output_live` (design 32), the same wave 1 adapters.
  `run_child` with `ChildCommand::with_stdin` and `spawn_child` (`ConfinedChild::write_stdin`, live
  `ChildStream`) need both facts; until then they go through the host driver's pipe sessions.
- Decided, Loom's side: egress. Substrate's D160 keeps its ADR 0013 and archived its egress story: a
  request names no destination, only an operator-declared aperture
  (`Network::Egress { apertures: Vec<String> }`). Loom's operator declares one aperture per
  destination, and Loom maps the `host:port` of `ChildCommand::with_egress`'s `EgressAperture` to
  that aperture's name. That mapping is part of this story when it is scoped.

So the story waits on the Substrate release that ships story:adapter-stdin-pipe and
story:adapter-live-output. It stays a draft.
