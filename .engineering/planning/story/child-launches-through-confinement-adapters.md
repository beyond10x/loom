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
revision: 3
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

Read 2026-10-28 against Substrate 0.14.0 (tag commit `0b9d50c6`), which Loom now pins.

- Delivered: the `linux` adapter serves `workspace_access: Scoped` on a host whose probe proved
  `exec.workspace-scoped-write` (Substrate 0.14.0 CHANGELOG, `### Added`). The scoped `target`
  write no longer needs `HostDriver::open`.
- Not delivered: the adapter surface takes `Command { argv, env }` only and every adapter starts
  the child with standard input null (`substrate-host` `confinement/mod.rs` `ConfinementAdapter`,
  `local.rs:259`, `systemd.rs:434`, `macos.rs:175`); its output is read after the run
  (`output`), not as a live stream. `run_child` with `ChildCommand::with_stdin` and `spawn_child`
  (`ConfinedChild::write_stdin`, live `ChildStream`) need both, through the host driver's pipe
  sessions today.
- Not delivered: the profile names egress apertures by operator-declared name
  (`Network::Egress { apertures: Vec<String> }`); `ChildCommand::with_egress` passes a full
  `EgressAperture` (host, port, pinned address) to the driver.

So the story still waits on Substrate: standard input and a live output stream through a
confinement adapter, and an aperture the host declares per launch. It stays a draft.
