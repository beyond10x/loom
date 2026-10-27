---
format: aep.planning-md/3
id: story:confines-on-macos-through-substrate
kind: story
status: draft
title: Loom confines tests.run and host children on macOS through Substrate's macos adapter, or refuses by name
summary: today every macOS launch is refused CgroupUndelegated; Substrate 0.13.0 ships a Seatbelt adapter
tags:
- adr-0109
relations:
- decomposes: epic:loom-sdk-for-products
revision: 2
---
## Outcome

On macOS, Loom's confined runner runs `tests.run` and a host's confined children through
Substrate's `macos` (Seatbelt) adapter, or refuses naming the family that adapter does not serve.
Today every macOS launch is refused `CgroupUndelegated`.

## Why

Substrate 0.13.0 ships a `macos` adapter (ADR 0109), and takt is moving onto it
(story:confinement-through-substrate-on-linux-and-macos). Loom's runner needs a delegated cgroup v2
root before it opens a driver, so on macOS it can only refuse; since story:substrate-0-12-pin that
refusal is `CgroupUndelegated` (`tests/confined_child.rs` `past_every_check`).

## Acceptance

1. The macOS CI job runs a confined `tests.run` through the `macos` adapter and records
   `adapter: macos` with its facts.
2. A family the `macos` adapter does not serve is refused by name, and a test pins it.
3. `task check` passes, and the macOS job runs the delegated-equivalent cases.
