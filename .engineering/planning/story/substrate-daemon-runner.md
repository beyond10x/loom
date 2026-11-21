---
format: aep.planning-md/3
id: story:substrate-daemon-runner
kind: story
status: draft
title: The engine can name a Substrate daemon endpoint and start every child through it
summary: b10x-substrate-sdk behind the runner; how a run ended comes back with the daemon's refusal code
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:confinement-opens-from-engine-config
revision: 1
---
## Outcome

The engine's confinement setting can name a Substrate daemon endpoint (a Unix socket or an HTTPS
endpoint) instead of an adapter opened in process. Loom then starts, observes, signals and stops
every child through `b10x-substrate-sdk`: `run_child`, `spawn_child` and the test runner behave
as under the in-process `linux` adapter, and how a run ended comes back with the daemon's refusal
code. Naming another host's endpoint moves the processes there with no change to a binding.

## Why

`story:confinement-opens-from-engine-config` opens `local` or `linux` in process. The daemon path
is the only one with a durable event log, leases, a named reason for a stopped run and remote
transport (Substrate 0.21.0: `crates/substrate-daemon/src/app/routes.rs:113-124`,
`crates/b10x-substrate-sdk/src/lib.rs:534-540`). Nothing in Loom depends on the SDK today
(`~/beyond10x/sdc/docs/layers.md` § 3, "nothing in Loom picks a daemon").

## ESS first

`ess/intake/domains/confinement.yaml`: `EngineConfinement` gains the variant `Daemon` with an
endpoint, and the applied record gains the daemon's exec id and `ended_by` (the refusal code,
optional). On that commit `task intake-drift` fails and
`crates/loom-intake-slice/tests/daemon_runner.rs` does not compile.

## Acceptance

Tests start a private Substrate daemon as a child (the SDK's managed daemon) and make no network
call beyond the local socket.

- With `Daemon`, `run_child` of a program returns its exit, output and applied record, and the
  record names the daemon's exec id.
- A program that runs past its timeout comes back with `ended_by` = `exec.timeout`; one that
  exits 3 comes back with exit 3 and no `ended_by`.
- `spawn_child` writes standard input and reads live output through the daemon.
- With an endpoint nothing listens on, the constructor is refused before any child starts, naming
  the endpoint.
- The delegated cases of `tests/confinement.rs` and `tests/confined_child.rs` hold under `Daemon`
  with the daemon's host driver.
- `Cargo.lock` holds one copy of each Substrate crate, the SDK at the pinned release.
- `task check` exits 0.

## Out of scope

Limits per action (`story:action-limits-enforced`). Choosing among several endpoints. The
kubernetes driver of the daemon.
