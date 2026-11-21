---
format: aep.planning-md/3
id: upstream-blocker:substrate-sdk-confinement-requires
kind: upstream-blocker
status: open
title: The Substrate SDK sends no sandbox.requires with a start
summary: The wire carries requires; b10x-substrate-sdk 0.21.0 always sends None
tags:
- sdc
relations:
- blocks: story:confinement-level-on-oci-and-pod
revision: 2
---
## What is missing

A way for an SDK client to send `sandbox.requires` with a start. Read at tag `0.21.0` on
2026-11-21: the wire carries `requires: Option<ConfinementRequires>` on a start and echoes it on
the exec (`crates/substrate-wire/src/lib.rs:1637`, `:1682-1685`), and `b10x-substrate-sdk` always
sends `requires: None` (`crates/b10x-substrate-sdk/src/lib.rs:1443`, `:1921`) and has no builder
method that sets it. A daemon therefore cannot refuse at admission an exec whose required families
it does not serve; Loom can check only the capability snapshot it read when it opened the daemon.

## What it blocks

In `story:confinement-level-on-oci-and-pod`, sending a required level's families with each start
and the daemon's refusal at admission. Running under `oci` or `pod` through a daemon in the same
story is held by `upstream-blocker:substrate-daemon-oci-pod`. `story:confinement-level-required`,
which defines the four levels and checks them against the published snapshot, does not wait on
this.

## What clears it

A `b10x-substrate-sdk` release with a builder method that sets `sandbox.requires` (families and
facts) on an exec start.

## Who

Session `sdc` sent the request to session `substrate` on 2026-11-21. That session answered the same
day with `story:sdk-confinement-requires`, a draft under Substrate's
`epic:runs-are-governed-and-limit-hits-are-events`, uncommitted on its branch
`plan/sdc-run-governance`, not accepted or implemented; its stated order puts it at any time.
