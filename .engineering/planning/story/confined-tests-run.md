---
format: aep.planning-md/3
id: story:confined-tests-run
kind: story
status: draft
title: tests.run runs confined by Substrate, refused when confinement is unavailable
relations:
- decomposes: epic:effect-bindings
- informed_by: architecture-design:effect-isolation
- serves: vision:O1
revision: 1
---
## Outcome

`b10x-loom run` executes `tests.run` inside Substrate's host driver, embedded the way Harness embeds
it: no network, the toolchain and source read-only, writes only to `target/`, memory and pid
limits. When confinement is unavailable the step stops with a named `ConfinementRefusal` unless the
run passes `--confinement none`, which the run record names. With no delegated cgroup, `b10x-loom`
re-execs itself under `systemd-run --user -p Delegate=yes --scope`. Dependencies are fetched on the
host before the run and mounted read-only. Every `tests.run` observation carries the
`AppliedConfinement` record.

## Why

architecture-design:effect-isolation, decisions 1, 2, 4, 5, 6 and 7 (operator, 2026-10-05).

## ESS first

The `intake.confinement` domain in `ess/intake/` (design § 6): `ConfinementProfile`,
`ToolchainRoot`, `AppliedConfinement`, `ConfinementRefusal` and
`StopReason::ConfinementUnavailable`, validated before any code.

## Acceptance

- A test that writes outside `target/`, opens a network socket or writes `.git/hooks` fails inside
  the confined run, and the observation names the applied confinement.
- On a machine without confinement the run stops `ConfinementUnavailable` with the named refusal;
  `--confinement none` runs it unconfined and the run record says so.
- The re-exec under `systemd-run` is covered by a test that runs where a user manager exists and is
  skipped, with its reason printed, where none does.
