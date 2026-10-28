---
format: aep.planning-md/3
id: story:substrate-0-12-pin
kind: story
status: implemented
title: Loom's confined runner runs on the newest Substrate release and admits a driver by its proved facts
summary: substrate 0.10.0 -> the release after 0.12.0; OS checks give way to the linux adapter's facts (ADR 0109); the applied record names adapter and facts
tags:
- adr-0109
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T09:09:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-27T09:09:24Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-28T15:03:18Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":2}}, executor: "agent:loom-pins-ess074-sub014", correlation: "D142"}
---
## Outcome

Loom's confined runner builds on the newest Substrate release, which carries the confinement
contract (`substrate_host::confinement`, ADR 0109), and it admits a Substrate driver by the facts
that contract's `linux` adapter proves, never by looking at the operating system: no check for
`/usr/bin/bwrap` is left, and a driver that does not serve a family a launch needs is refused
naming that family. Every confined run records the adapter it ran under and the facts it proved.

## Why

The operator, 2026-10-27: "yes, we want to upgrade to latest substrate for: loom, takt, harness,
metaharness", and ADR 0109 decision 6: "loom-sdk and takt consume Substrate only and delete their
own confinement code [...]; loom's OS checks give way to facts." `b10x-substrate-host` and
`b10x-substrate-wire` are pinned at `0.10.0` (`crates/loom-intake-slice/Cargo.toml:31-32`).
`SubstrateRunner::open` checks that `/usr/bin/bwrap` is an executable file (`validate_backend`)
and reads the driver's raw capability facts one by one; the contract's `linux::proof` reads the
same facts into families and facts with names a refusal can carry.

## Acceptance

1. `crates/loom-intake-slice/Cargo.toml` pins both substrate crates by the release's tag commit
   with an exact `=` version, and `task check` passes on it.
2. Conformance scenario `a_driver_missing_a_family_is_refused_naming_it`
   (`crates/loom-intake-slice/src/confinement.rs`): a driver whose proved facts lack `limits` is
   refused `CgroupUndelegated`, and one lacking `process_run`, `wait`, `output_read`, `workspace`
   or `filesystem_view` is refused `CapabilityUnserved`; each detail names
   `confinement.family-unserved` and the family.
3. `validate_backend` is gone: `grep -n 'usr/bin/bwrap' crates/loom-intake-slice/src` prints
   only the driver configuration's own path, no check.
4. Intake domain `intake.confinement.AppliedConfinement` gains `adapter` (`Optional<String>`) and
   `facts` (`Optional<List<String>>`); a Substrate launch records `adapter: linux` and the proved
   facts in `FACT_ORDER`; an unconfined launch records neither; a record written before this
   change still reads (`resume`). The intake conformance suite (`task intake-conform`) passes.
5. The existing confinement suites pass unchanged against the new driver:
   `crates/loom-intake-slice/tests/confinement.rs`, `confined_child.rs`, `adversary_confinement.rs`
   and the delegated cases in `src/confinement.rs`.

## Scope

`crates/loom-intake-slice/Cargo.toml`, `crates/loom-intake-slice/src/confinement.rs`,
`crates/loom-intake-slice/src/executor.rs`, `crates/loom-intake-slice/src/resume.rs`,
`ess/intake/domains/confinement.yaml`, `generated/rust/intake/`, `crates/loom-intake-conformance`,
`Cargo.lock`, `CHANGELOG.md`, `AGENTS.md`.

## Out of scope

Routing `run_child` and `spawn_child` through `ConfinementAdapter`: a child with standard input, a
live stream or egress apertures needs the host driver's pipe sessions and aperture configuration,
which the frozen consumer surface does not serve. A macOS runner in Loom: takt reaches the `macos`
adapter directly (its story `confinement-through-substrate-on-linux-and-macos`).
