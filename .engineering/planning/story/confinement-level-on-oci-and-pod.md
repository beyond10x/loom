---
format: aep.planning-md/3
id: story:confinement-level-on-oci-and-pod
kind: story
status: draft
title: A process runs under the oci or pod level through a Substrate daemon that serves those adapters
summary: The daemon's configuration names the image; each start carries the level's families as sandbox.requires; held by two upstream blockers
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-daemon
revision: 5
---
## Outcome

A process runs under the level `oci` or `pod` of `story:confinement-level-required` through a
Substrate daemon that serves those adapters. The image is the daemon's configuration: Loom names
none, and the step's confinement record names the image digest the daemon reports for the exec.
The daemon proves `image_digest_pinned`, which the mapping reads from the daemon's published
facts. Loom sends the required level's families with each start as `sandbox.requires`, set by a new
builder `ChildCommand::with_requires(families)` and carried by `ChildCommand::input` and
`session_input` (`crates/loom-intake-slice/src/confinement.rs:1538`), so the daemon itself refuses
at admission an exec whose families it does not serve. `run_child` and `spawn_child` do no families
check of their own; Loom's check against the families the daemon published is the tree's, when the
confinement is opened (`story:confinement-level-required`). `check_record` (`:2836-2837`), which
refuses any record that echoes `requires`, then accepts a record whose `requires` equals what the
start sent and still refuses one that differs or that echoes `requires` for a start that sent none.

The story's first commit is the `## ESS first` change and nothing else; its second commit moves
Loom's Substrate pin to the release that clears both blockers: `substrate-host`, `substrate-wire`
and `b10x-substrate-sdk` in `crates/loom-intake-slice/Cargo.toml` (the SDK line is there since
`story:substrate-daemon-runner`), and the version and revision that
`crates/loom-intake-slice/tests/host_git_hardening.rs:549-552` asserts.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "the confinement level (local,
linux, oci, pod)". `story:confinement-level-required` defines the four levels and refuses `oci`
and `pod` where they cannot be given; what is left waits on Substrate. At 0.21.0 the `oci` and
`pod` adapters exist in process only (`crates/substrate-host/src/confinement/oci.rs:409-421`,
`pod.rs:197-205`), and a daemon opens the `Host` or the `Kubernetes` driver
(`crates/substrate-daemon/src/runtime.rs:520-529`). The image is host-side profile only
(`profile.image`, `crates/substrate-host/src/confinement/profile.rs:92`): a start's
`ConfinementRequest` has no image member (`crates/substrate-wire/src/lib.rs:1611`), and the daemon's
confinement properties carry no image-digest fact (`ConfinementProperty::ALL`, `:3690-3712`), so
under a daemon `oci` is never met today: `upstream-blocker:substrate-daemon-oci-pod` holds running
under them. The wire carries `requires` on a start and echoes it on the exec (`lib.rs:1637`,
`:1682-1685`), and the SDK always sends `requires: None` (`crates/b10x-substrate-sdk/src/lib.rs:1443`,
`:1921`): `upstream-blocker:substrate-sdk-confinement-requires` holds sending the level. Loom asserts
the absent echo today in `confinement.rs:3974`, `:3997-4003` and
`tests/security_w20261121w1_u1.rs:76`.

## Waits on

- `upstream-blocker:substrate-daemon-oci-pod` (running under `oci` or `pod` through a daemon) and
  `upstream-blocker:substrate-sdk-confinement-requires` (sending the level with a start).
- `story:tree-daemon`: the last unheld story of the chain. Through it,
  `story:confinement-level-required` (`ConfinementLevel`, the mapping, `LevelUnmet`) and
  `story:substrate-daemon-runner` (`Daemon`, the SDK dependency) come first. No edge orders this
  story against `story:executable-descendants-held`, `story:ingress-routes-per-action` or
  `story:substrate-daemon-on-another-host`, each held by another blocker; they share the launch path
  in `crates/loom-intake-slice/src/confinement.rs` and `ess/domains/engine.yaml`, and their order on
  them is settled by scope when this story's blockers clear.

## ESS first

`ess/intake/domains/confinement.yaml`: `ConfinementSpec` gains optional `requires` (families),
`AppliedConfinement` gains `requires` (the families the start carried) and optional
`image_digest`. `ess/domains/engine.yaml`: `StepConfinement` gains optional `image_digest`. On that
commit `task drift` and `task intake-drift` fail and
`crates/loom-tree/tests/confinement_level_oci_pod.rs` does not compile.

## Acceptance

With the Substrate release that clears both blockers, and a container runtime on the gate:

- `Cargo.lock` holds one copy of each Substrate crate, at that release, and
  `tests/host_git_hardening.rs` asserts that release.
- Under a daemon configured to serve `oci` with an image, a stub program of an action that
  requires `oci` runs in a container from that image, and its confinement record names level `oci`,
  the daemon's families and the image digest the daemon reports.
- The exec observation of every start echoes `requires` with the families of the required level,
  and the applied record names them; `check_record` accepts that record and refuses one whose
  `requires` differs (the tests at `:3974` and `:3997-4003` changed to say so), and a start with no
  `requires` still gets a record with none (`tests/security_w20261121w1_u1.rs:76`).
- `run_child` of a command built `with_requires` naming a family absent from the managed daemon's
  `Machine::served_families()` (the test picks the first absent one) reaches the daemon, is refused
  at admission before any process starts, and the runner returns the daemon's refusal
  (`confinement.family-unserved`) naming the family.
- `task check` exits 0.

## Out of scope

Running under `pod` in the gate: it needs a cluster; the `pod` level is checked here by the
families Loom sends. Mixing two adapters in one run. Choosing the image per action.
