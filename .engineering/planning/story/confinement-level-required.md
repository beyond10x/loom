---
format: aep.planning-md/3
id: story:confinement-level-required
kind: story
status: draft
title: A run and an action can require a confinement level, and the engine refuses what it cannot give
summary: local, linux, oci and pod, each a set of Substrate contract families and one fact, met by what the opened confinement serves and proves, in process or on a daemon
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-run-command
- depends_on: story:substrate-daemon-runner
revision: 8
---
## Outcome

A run and an action can require a confinement level, and the engine refuses to start a process it
cannot give that level. This story defines the four levels of the request, `local`, `linux`, `oci`
and `pod`. A level is a set of Substrate contract families and, for all but `local`, one fact.
Families are spelled one way everywhere in Loom, as the snake_case names of Substrate's host
`FAMILY_ORDER` (`process_run`, `filesystem_view`, ...); the daemon's wire spells them kebab-case
(`ConfinementFamily`, `process-run`), and one function, `family_name` in
`crates/loom-intake-slice/src/confinement.rs`, converts a wire family to that name.

- `local`: `process_run`, `wait`, `workspace`;
- `linux`: those three and `filesystem_view`, `limits`, and the fact `network_held`;
- `oci`: `process_run`, `wait`, `workspace`, `filesystem_view`, `limits`, and the fact
  `image_digest_pinned`;
- `pod`: `process_run`, `wait`, `workspace`, `limits`, and the fact `image_digest_pinned`.

The opened confinement reports what it serves through `SubstrateRunner::served()`, a
`ServedConfinement` (the families it serves and the mapping facts it proves). The mapping is
`level_met(level, served)` and `proves(served, fact)` in `crates/loom-tree/src/level.rs`: a level
is met when every family of it is served and its fact is proved. It answers three facts, and
nothing else, by route:

| fact | in-process `local` | in-process `linux` | `Daemon` |
| --- | --- | --- | --- |
| `image_digest_pinned` | not proved | not proved | not proved: no wire property names it |
| `network_held` | not proved: `local` proves `host_network` | the snapshot's `exec.no-egress` | `Machine.exec_no_egress` |
| `cpu_time_usage` | not proved | not proved: the mapping reads no in-process usage, which the host driver reports only on request and the adapter route that replaces it does not report | `exec_resource_usage.cpu_time` |

In process the families are the adapter's `served_families`; under `Daemon` they are
`Machine::served_families()`, converted. The adapter's name is never read.

The engine configuration's `default_level` is required of every action, and a configuration
without it requires `local`. A binding's `confinement` may name a level that holds every family
and fact of the default, and is refused at `tree open` and `tree run`, `LevelBelowDefault`, when it
names one that lacks any; that check needs no confinement. Whether the opened confinement meets the
levels is checked when it is opened, at `tree run` before the first step and, under
`story:tree-daemon`, when a tree is submitted; `tree open` opens no confinement, since opening
`linux` reconciles the cgroup root. A missing family is `LevelUnmet`, naming the action, the level
and the first family it lacks in `FAMILY_ORDER`; a missing fact, checked after the families, is
`LevelFactUnproved`, naming the action, the level and the fact. The same mapping is the one place
the other stories of this epic ask whether the opened confinement serves a family (`limits`,
`network_egress`) or proves a fact (`network_held`, `cpu_time_usage`). Each step's confinement
record names the level required and the families served.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "the confinement level (local,
linux, oci, pod)" per run and per action. `story:confinement-opens-from-engine-config` gives the
engine one adapter for everything, `linux` or `local`. Substrate 0.21.0 names ten contract families
in one order (`FAMILY_ORDER`, `crates/substrate-host/src/confinement/facts.rs:13-24`, snake_case;
the same ten on the wire, kebab-case, `crates/substrate-wire/src/lib.rs:3620-3632`). `local` serves
`process_run`, `process_watch`, `output_read`, `wait`, `workspace` and `audit`
(`crates/substrate-host/src/confinement/local.rs:107-112`) and proves `host_network` (`:118`);
`linux` adds `filesystem_view`, `network_egress` and `limits`, the last where its probe proved it,
and proves no image fact (`crates/substrate-host/src/confinement/linux.rs:177-197`, facts
`:199-212`); `oci` serves `filesystem_view` and `limits` and proves `image_digest_pinned`
(`oci.rs:409-421`); `pod` serves `limits` and proves `image_digest_pinned` (`pod.rs:197-205`). A
daemon publishes the families it serves (`Machine::served_families`,
`crates/b10x-substrate-sdk/src/model.rs:64`); the confinement properties on its wire do not include
an image-digest fact (`ConfinementProperty::ALL`, `crates/substrate-wire/src/lib.rs:3690-3712`).
Choosing by adapter name is discouraged in Substrate in favour of reading families and facts
(Substrate `story:confinement-consumer-surface`).

`network_egress` is not in the `linux` level: `linux` serves it only when at least one aperture is
declared and proved (`linux.rs:192-195`; `egress_apertures_fact` returns none for an empty
declaration, `crates/substrate-host/src/egress.rs:117`), so it says whether a confinement can
forward an aperture, not whether the network is held. Whether a run's network is held is the
capability fact `exec.no-egress` (`CapabilityFacts.exec_no_egress`, wire `lib.rs:3933`), which the
host probe sets for a host whose exec proof held (`crates/substrate-host/src/probe.rs:135`), which
`linux`'s proof reads from the snapshot it is opened with (`linux.rs:165-215`), and which the SDK
publishes as `Machine.exec_no_egress` (`model.rs:30`). CPU-time usage is published as
`exec_resource_usage.cpu_time` (wire `lib.rs:4127`, `:4348-4359`); the in-process adapter route's
`RunObservation` reports no usage (`crates/substrate-host/src/confinement/mod.rs:694-706`).

A level of families alone would make `linux` meet `oci` and `pod`, since it serves every family of
both; the fact `image_digest_pinned` is what a container level adds. `LevelUnmet` names a family
only, so the fact has its own refusal. Running a process under `oci` or `pod` through a daemon
waits on Substrate (`story:confinement-level-on-oci-and-pod`); this story defines the two levels
and refuses them where they cannot be given, which needs nothing from Substrate.

## Waits on

- `story:substrate-daemon-runner`: the `Daemon` confinement whose `Machine` families and facts this
  story reads, and `ess/intake/domains/confinement.yaml` and `ess/domains/engine.yaml`, which it
  edits before this story.
- `story:tree-run-command`: `tree open` and `tree run`, where the refusals happen.

## ESS first

`ess/intake/domains/confinement.yaml`: `ServedConfinement` (`families`, a list of the intake
`ContractFamily` of ten snake_case values in `FAMILY_ORDER`; `facts`, a list of `ServedFact`:
`ImageDigestPinned`, `NetworkHeld`, `CpuTimeUsage`). `ess/domains/engine.yaml`: `ConfinementLevel`
(`Local`, `Linux`, `Oci`, `Pod`), `ContractFamily` and `LevelFact` (the same values, on the loom
side), `EngineConfiguration` gains optional `default_level`, `StepConfinement` gains `level` and
`families` (`ContractFamily`, filled from `SubstrateRunner::served()` when the run opened the
confinement), and `EngineRefusal` gains `LevelUnmet` (action, level, family), `LevelFactUnproved`
(action, level, fact) and `LevelBelowDefault` (action, level, default level).
`ess/domains/bindings.yaml`: `BoundAction` gains optional `confinement`
(`loom.engine.ConfinementLevel`). On that commit `task intake-drift` and `task drift` fail and
`crates/loom-tree/tests/confinement_level.rs` does not compile.

## Acceptance

- With the engine on `local` (with a reason) and `default_level: linux`, `tree run` is refused
  before its first step `LevelUnmet`, naming `linux` and `filesystem_view`; `tree open` of the same
  tree succeeds.
- With the engine on `local` (with a reason) and no `default_level`, the three-level fixture tree
  of `story:tree-run-command`, whose bindings name no `confinement`, runs, and every step's
  confinement record names level `local`.
- With the engine on in-process `linux` and `default_level: local`, an action whose binding
  requires `linux` runs, and its step's confinement record names level `linux` and families that
  include `filesystem_view` and `limits`, spelled snake_case.
- With `default_level: linux`, a binding that requires `local` is refused at `tree open`
  `LevelBelowDefault`, naming the action, `local` and `linux`; a binding that requires `oci` is
  refused the same way, since `oci` lacks `network_held`.
- The mapping, given the families `local` serves, refuses `linux` naming `filesystem_view`; given
  the families of `linux` without `limits`, it refuses `linux` naming `limits`; given all of
  `linux`'s families without `network_held`, it refuses `LevelFactUnproved` naming `network_held`;
  given those families and `network_held`, it admits `linux` with no `network_egress`.
- With the engine on `local`, `tree run` of a binding that requires `oci` is refused `LevelUnmet`
  naming `filesystem_view`, and of one that requires `pod` `LevelUnmet` naming `limits`.
- With the engine on in-process `linux`, `tree run` of a binding that requires `oci` or `pod` is
  refused `LevelFactUnproved`, naming the level and `image_digest_pinned`.
- `family_name` maps each of the ten wire families to its `FAMILY_ORDER` name (`process-run` to
  `process_run`, and so on).
- Under `Daemon` with a managed daemon, the step's confinement record's families equal that
  daemon's `Machine::served_families()` after `family_name`. `tree run` of a binding that requires
  `oci` is refused: `LevelUnmet` naming the first family of `oci` missing from those families, or,
  when none is missing, `LevelFactUnproved` naming `image_digest_pinned`; the test computes which
  from the daemon's served families.
- `proves` answers the table above: under in-process `local` no fact; under in-process `linux`,
  `network_held` equals the snapshot's `exec_no_egress` and `cpu_time_usage` is not proved; under
  `Daemon`, `network_held` equals `Machine.exec_no_egress` and `cpu_time_usage` equals
  `exec_resource_usage.cpu_time`.
- `task check` exits 0.

## Out of scope

Running a process under `oci` or `pod` through a daemon, and sending a level with each start as
`sandbox.requires` (`story:confinement-level-on-oci-and-pod`, held by
`upstream-blocker:substrate-daemon-oci-pod` and
`upstream-blocker:substrate-sdk-confinement-requires`). Opening `oci` or `pod` in process. Mixing
two adapters in one run. `macos` and `systemd`. Any other fact, `stdin_pipe` among them.
