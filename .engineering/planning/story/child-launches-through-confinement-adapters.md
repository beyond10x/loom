---
format: aep.planning-md/3
id: story:child-launches-through-confinement-adapters
kind: story
status: draft
title: run_child and spawn_child launch through Substrate's confinement adapters, not HostDriver directly
summary: '''ADR 0109 decision 6, second half for loom: launches go through the contract; b10x-substrate-host regains its kubernetes feature so pod opens, which brings aws-lc-rs back into the operator''''s graph'''
tags:
- adr-0109
relations:
- decomposes: epic:loom-sdk-for-products
- depends_on: story:substrate-0-21-pin
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/child.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement/several_apertures.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement/workdir.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confinement.rs
- confidence: inferred
  path: crates/loom-operator/src/main.rs
- confidence: inferred
  path: crates/loom-operator/tests/startup.rs
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 12
---
## Outcome

`SubstrateRunner::run_child` and `spawn_child` launch through Substrate's confinement contract
(`substrate_host::confinement`, ADR 0109) and the adapter it selects, not by building
`HostDriver` requests themselves. Writes stay scoped to the paths the host names, and each launch
records the adapter and facts it ran under, as `tests.run` does.

Loom builds `b10x-substrate-host` with the `kubernetes` feature again, so that the contract can
open `pod` as it opens `oci`, `linux` and `local`. Since wave 2026-11-21-w1 the manifest reads
`default-features = false, features = ["git"]` (`crates/loom-intake-slice/Cargo.toml:31`), and in
that build Substrate 0.21.0 compiles `pod` without a cluster client: every open is refused
`confinement.adapter-unserved` naming `pod` and the feature
(`crates/substrate-host/src/confinement/pod.rs:2012-2014`, `pod_unserved.rs`). This story makes
the line `features = ["git", "kubernetes"]`, still with `default-features = false`.

That brings `aws-lc-rs` back into the operator's graph. The feature adds
`b10x-substrate-kubernetes-client` (`crates/substrate-host/Cargo.toml:22`, `:41`), whose `rustls`
and `tokio-rustls` are Substrate's workspace pins with `aws_lc_rs` (Substrate `Cargo.toml:54`,
`:68`). `b10x-loom-operator` reaches `b10x-substrate-host` through `b10x-loom-sdk`
(`crates/loom-operator/Cargo.toml:23`), so its graph again enables both rustls providers: `ring`
through kube's `rustls-tls` and its own `rustls` pin (`crates/loom-operator/Cargo.toml:29-30`),
`aws-lc-rs` through Substrate. The binary stays correct because `run` installs `ring` as the
process default before it builds a client (`crates/loom-operator/src/main.rs:58`), which is what
`run_builds_its_tls_client_and_connects_to_the_cluster` checks: the process connects and its
standard error holds no `CryptoProvider` (`crates/loom-operator/tests/startup.rs:89`). Only the
comments that say `aws-lc-rs` left with Substrate 0.21.0 become wrong (`main.rs:52-56`,
`startup.rs:2-6`), and this story rewrites them.

## Why

ADR 0109 decision 6: "loom-sdk and takt consume Substrate only and delete their own confinement
code". story:substrate-0-12-pin moved admission onto the `linux` adapter's facts
(`linux::proof`), but launches still open `HostDriver` directly (`crates/loom-intake-slice/src/confinement.rs`,
`HostDriver::open`) with a `Scoped` workspace, because the `linux` adapter in Substrate 0.13.0
served the workspace read-write only. The adapter route waited on Substrate's story
"linux adapter serves a scoped workspace write" (requested 2026-10-27), delivered in 0.14.0.

The `kubernetes` feature was turned off in `story:substrate-0-21-pin` (security review pass 1 of
wave 2026-11-21-w1), which took `aws-lc-rs` out of the graph and left the operator with `ring`
only. Pass 2 recorded the cost for this story: with `["git"]` the `pod` adapter is not openable;
`oci`, `linux` and `local` are (`approval-record:loom-wave-2026-11-21-w1`).

## Acceptance

1. `run_child` and `spawn_child` reach the driver only through the contract's adapter; no
   `HostDriver::open` remains outside it.
2. The delegated cases in `tests/confinement.rs` and `tests/confined_child.rs` still hold,
   including the scoped `target` write and a refused write outside it.
3. `crates/loom-intake-slice/Cargo.toml` builds `b10x-substrate-host` with
   `default-features = false, features = ["git", "kubernetes"]`, and a `pod` open through the
   contract on a host with no cluster is refused for a reason other than
   `confinement.adapter-unserved` naming the feature.
4. With that feature on, `cargo tree -p b10x-loom-operator -i aws-lc-rs --locked` lists a path
   through `b10x-substrate-host`; `run_builds_its_tls_client_and_connects_to_the_cluster`
   (`crates/loom-operator/tests/startup.rs`) passes with its assertions unchanged; the comments
   at `crates/loom-operator/src/main.rs:52-56` and `crates/loom-operator/tests/startup.rs:2-6`
   say both providers are in the graph and `run` installs `ring`; and `CHANGELOG.md` names
   `aws-lc-rs` back in the graph.
5. `task check` passes.

## Waits on

Read 2026-11-21 against Substrate 0.21.0 (tag commit `adfde402`), which Loom pins since wave
2026-11-21-w1 (`crates/loom-intake-slice/Cargo.toml:31-32`, `version = "=0.21.0"`). This section
replaces the reading of 2026-10-28.

- Delivered in Substrate 0.14.0: the `linux` adapter serves `workspace_access: Scoped` on a host
  whose probe proved `exec.workspace-scoped-write`.
- Delivered in Substrate 0.15.0: standard input through a confinement adapter
  (`StdinMode::Piped`, fact `stdin_pipe`, `write_stdin`, `close_stdin`), proved by `local`,
  `linux`, `macos` and `systemd` (Substrate `CHANGELOG.md`, entry 0.15.0). Substrate's
  story:adapter-stdin-pipe is `implemented` at tag 0.21.0.
- Delivered in Substrate 0.15.0: a live output stream (`OutputDelivery::Live`, fact
  `output_live`, `read_output`). Substrate's story:adapter-live-output is `implemented` at tag
  0.21.0.
- Decided, Loom's side, unchanged: egress. A request names no destination, only an
  operator-declared aperture (`Network::Egress { apertures: Vec<String> }`); Loom maps the
  `host:port` of `ChildCommand::with_egress` to that aperture's name. That mapping is part of
  this story when it is scoped.

The story waits on nothing: `story:substrate-0-21-pin`, which moved the pin onto a release with
both facts, is `implemented`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs` — cited, the story names it and `HostDriver::open` (line 402)
- **Files:** `crates/loom-intake-slice/src/confinement.rs:402`, `:475`, `:525` (`HostDriver::open`, `run_child`, `spawn_child`) — cited
- **Files:** `crates/loom-intake-slice/src/confinement/child.rs` — cited, `ConfinedChild` calls `HostDriver` directly (`child.rs:9`, `:182`, `:188`, `:281`)
- **Files:** `crates/loom-intake-slice/tests/confinement.rs`, `crates/loom-intake-slice/tests/confined_child.rs` — cited, named in Acceptance 2 as the cases that must still hold
- **Symbols:** `SubstrateRunner::run_child`, `SubstrateRunner::spawn_child`, `HostDriver::open`, `substrate_host::confinement` (`open`, `AdapterRequest::Linux`, `LinuxRequest`, `ConfinementAdapter`) — cited
- **Symbols:** `OpenDriver`, `Drivers`, `Release`, `SubstrateRunner::launch` / `launch_with_stdin`, `ConfinedChild`, `PipeStream` / `PipeFrame` — cited in the tree (`confinement.rs:243,274,556,594`; `child.rs`), inferred as the code that changes
- **Also likely:** `crates/loom-intake-slice/src/confinement/workdir.rs`, `crates/loom-intake-slice/src/confinement/several_apertures.rs` — inferred, in-crate unit tests that call `settle`, `requested_profile` and `driver_config`, which sit on the launch path being rewritten
- **Also likely:** `crates/loom-intake-slice/src/executor.rs` — inferred, it reads the applied record (`adapter`, `facts`, `cgroup`) that `OpenDriver::adapted` writes; it changes only if that record's shape changes
- **Documents:** `website/docs/reference/sdk-stability.md` (lines 134, 136 describe "a Substrate pipe session" and the shared driver per directory) — inferred, it changes only if the wording of the launch path changes
- **Confidence:** high on the primary surface and the two test files (the story and the tree agree). Medium on how many files change, because the adapter-to-launch mapping is not proved.
- **Would collide with:** any unit touching `loom-intake-slice`'s `src/confinement.rs` or `src/confinement/` (in particular where a driver is opened, the `OpenDriver` / `Drivers` cache, `launch`, `ConfinedChild`), and any unit touching `crates/loom-intake-slice/Cargo.toml` Substrate pins
- **Safety fact:** `HostDriver` is named only in `src/confinement.rs` and `src/confinement/child.rs`. No file under `tests/` names `HostDriver`, so the integration tests reach the runner only through its public API. Outside the crate, `SubstrateRunner` is reached through the `loom-sdk` re-export (`crates/loom-sdk/src/lib.rs:99-114`). — step 2 (`git grep -n 'HostDriver\|substrate_host'`), unproven
- **Safety fact (second):** `HostDriver::open` at `confinement.rs:402` is shared by `TestRunner::run` (`:983-1010`), `run_child` and `spawn_child` through `SubstrateRunner::driver` (`:321`). Acceptance 1 ("no `HostDriver::open` remains outside it") therefore moves the `tests.run` path too, though the story names only the child launches. — step 2, unproven
- **House-rule files:** `CHANGELOG.md`, `AGENTS.md` (line 2032 says "Substrate raw-pipe session"), `Cargo.lock`. No generated file or reference page is touched. `ess/intake/domains/confinement.yaml:67-70` already carries `adapter` and `facts`, and the acceptance asks for no new declaration.
