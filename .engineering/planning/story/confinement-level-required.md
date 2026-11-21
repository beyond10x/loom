---
format: aep.planning-md/3
id: story:confinement-level-required
kind: story
status: draft
title: A run and an action can require a confinement level, and the engine refuses what it cannot give
summary: local, linux, oci, pod or a daemon; a level is met by the facts the adapter proves
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
revision: 1
---
## Outcome

A run and an action can require a confinement level, and the engine refuses to start a process
it cannot give that level. The engine configuration names what it can open (`local`, `linux`,
`oci`, `pod`, or a Substrate daemon) and a default requirement; a binding may raise the
requirement for its action and never lower it. A requirement is a named level that Loom maps to
the Substrate families and facts the level stands for (for example whole-tree kill, no egress,
scoped workspace write, limits), so a level is met by what the adapter proves and not by its
name. An unmet requirement refuses the bindings file at load when it can be known then, and
otherwise the step before anything starts, naming the action, the level and the missing fact.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "the confinement level (local,
linux, oci, pod)" per run and per action. `story:confinement-opens-from-engine-config` gives the
engine one adapter for everything, `linux` or `local`. Substrate 0.21.0 has six adapters
(`crates/substrate-host/src/confinement/mod.rs:129-136`) and lets a profile require families and
facts beyond what its fields imply (`profile.rs:44-47`); choosing by adapter name is discouraged
there in favour of reading `served_families` and `facts` (Substrate
`story:confinement-consumer-surface`). `oci` and `pod` serve no network family, and the Substrate
daemon serves only its host driver or its kubernetes driver.

## ESS first

`ess/intake/domains/confinement.yaml`: `EngineConfinement` gains `Oci` and `Pod` with what each
needs to open, `ConfinementLevel` and its mapping to facts, and the refusal `LevelUnmet` (level,
missing fact). `ess/domains/bindings.yaml`: `BoundAction` gains optional `confinement` (a level).
On that commit `task intake-drift` and `task drift` fail and
`crates/loom-bindings/tests/confinement_level.rs` does not compile.

## Acceptance

- With the engine on `local` and a default requirement of `linux`, `tree run` is refused before
  its first step `LevelUnmet`, naming the missing fact.
- With the engine on `linux`, an action that requires `linux` runs; one whose binding requires a
  level whose facts `linux` does not prove is refused at load.
- A binding that names a lower level than the engine's default is refused at load.
- With the engine on `oci` and a runtime present, a stub program runs in a container from the
  configured image digest and its applied record names adapter `oci`; the test is ignored with a
  stated reason where no runtime is installed.
- Each step's record names the adapter, the level required and the facts proved.
- `task check` exits 0.

## Out of scope

The `pod` adapter in Loom's gate (needs a cluster; a qualification record instead). Mixing two
adapters in one run. `macos` and `systemd`.
