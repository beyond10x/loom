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
revision: 2
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
