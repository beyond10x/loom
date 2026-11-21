---
format: aep.planning-md/3
id: story:executable-descendants-held
kind: story
status: draft
title: A started program's descendants are held to the engine's executable lists where Substrate proves it
summary: Loom hands both lists to Substrate when the opened confinement proves the fact; held by upstream-blocker:substrate-executable-allow-list
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-daemon
revision: 3
---
## Outcome

A program a tree run starts executes, itself and through its descendants, only what the engine's
executable lists admit, where Substrate proves it. Loom passes the allow-list and the deny-list of
the engine configuration's `executables` section to Substrate with each start when the opened
confinement proves the executable-list fact, read through the mapping of
`story:confinement-level-required`, and the step's confinement record then says
`descendants_held: true`. The lists go on the value that reaches Substrate: under `Daemon` the
start request (`ExecStartInput`, through the SDK), in process the request Loom's runner hands
Substrate's `linux` adapter, whichever member the release that clears the blocker defines; Loom's
recorded projection of the request (`intake.confinement.ConfinementProfile`, which
`profile_digest` hashes) records them too, written only when a list is present, so a run without
lists keeps its digest. Where the fact is not proved the lists stay Loom's own check of the program
it starts, and the record says `false`, as `story:executable-allow-list` left it. Loom never hands
Substrate a deny-list without an allow-list.

The story's first commit is the `## ESS first` change and nothing else; its second commit moves
Loom's Substrate pin to the release that clears its blocker: `substrate-host`, `substrate-wire` and
`b10x-substrate-sdk` in `crates/loom-intake-slice/Cargo.toml` (the SDK line is there since
`story:substrate-daemon-runner`), and the version and revision that
`crates/loom-intake-slice/tests/host_git_hardening.rs:549-552` asserts.

## Why

`upstream-blocker:substrate-executable-allow-list`: at Substrate 0.21.0
`ConfinementProfile.exec_subpaths` is proved by `macos` only
(`crates/substrate-host/src/confinement/macos.rs:3-35`), `linux` refuses a profile that uses it
(`linux.rs:29-31`), no adapter has a deny-list, and the wire has only the capability fact
(`confinement_exec_subpaths`, `crates/substrate-wire/src/lib.rs:4042`) with no list on a start.
`story:executable-allow-list` holds the program Loom starts to the lists and records
`descendants_held: false`; this story is the part that waits on Substrate. It needs `linux` and the
daemon's start, not `oci` or `pod`. Session `substrate` planned `story:linux-executable-allow-list`,
`story:executable-deny-list` and `story:exec-start-carries-executable-lists` for that, and
`story:executable-lists-on-oci-and-pod` beside them (drafts, 2026-11-21).

Today's request to Substrate is `ExecStartInput` (`crates/loom-intake-slice/src/confinement.rs:1512-1580`)
with the driver's `HostConfig` (`:224`); Loom's projection is built by `requested_profile`
(`:3401-3426`) and hashed by `profile_digest` (`:3429-3470`), whose digests the test at `:5040` and
`tests/several_apertures.rs` and `tests/workdir.rs` pin.

## Waits on

- `upstream-blocker:substrate-executable-allow-list`: a Substrate release in which `linux` proves
  an executable allow-list and a deny-list as facts and the daemon carries the lists on a start.
- `story:tree-daemon`: the last unheld story of the chain. Through it,
  `story:executable-allow-list` (the lists, `descendants_held`, `StepConfinement` in
  `ess/domains/engine.yaml`), `story:confinement-level-required` (the mapping) and
  `story:substrate-daemon-runner` (`Daemon`, the SDK dependency) come first. No edge orders this
  story against `story:ingress-routes-per-action`, `story:confinement-level-on-oci-and-pod` or
  `story:substrate-daemon-on-another-host`, each held by another blocker; they share the launch path
  in `crates/loom-intake-slice/src/confinement.rs`, and their order on it is settled by scope when
  this story's blocker clears.

## ESS first

`ess/intake/domains/confinement.yaml`: `ConfinementProfile` (the recorded projection) gains the
optional allow-list and deny-list handed to Substrate, and the mapping's `ServedFact` gains
`ExecutableLists`. `ess/domains/engine.yaml`: `LevelFact` gains `ExecutableLists`;
`StepConfinement.descendants_held` exists since `story:executable-allow-list`. On that commit
`task intake-drift` and `task drift` fail and
`crates/loom-intake-slice/tests/executable_descendants.rs` does not compile.

## Acceptance

Under the in-process `linux` adapter and under `Daemon` with a managed daemon, both of the
Substrate release that clears the blocker:

- `Cargo.lock` holds one copy of each Substrate crate, at that release, and
  `tests/host_git_hardening.rs` asserts that release.
- A bound program on the allow-list that executes a program on the deny-list fails to execute it,
  and the step's confinement record says `descendants_held: true`.
- The same program executing another allowed program succeeds.
- Under the `local` adapter, which does not prove the fact, the denied program executes and the
  record says `descendants_held: false`.
- The start request the runner builds, captured before it is sent, never holds a deny-list
  without an allow-list (a test over the constructed requests, in process and under `Daemon`).
- A run with no `executables` section has the same `profile_digest` as before this story (the
  existing digest tests pass unchanged).
- `task check` exits 0.

## Out of scope

The lists on `oci` and `pod`. Checking a program by content digest.
