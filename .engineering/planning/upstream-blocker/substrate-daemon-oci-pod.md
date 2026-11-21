---
format: aep.planning-md/3
id: upstream-blocker:substrate-daemon-oci-pod
kind: upstream-blocker
status: open
title: No Substrate daemon serves the oci or pod confinement adapter
summary: A daemon opens the Host or Kubernetes driver only at 0.21.0; oci and pod exist in process only
tags:
- sdc
relations:
- blocks: story:confinement-level-on-oci-and-pod
revision: 3
---
## What is missing

A Substrate daemon that serves the `oci` or the `pod` confinement adapter. Read at tag `0.21.0` on
2026-11-21: a daemon opens one driver, chosen by configuration, `DriverSelection::Host` or
`DriverSelection::Kubernetes { config }` (`crates/substrate-daemon/src/runtime.rs:520-529`). The
`oci` and `pod` adapters are opened only in process through `substrate_host::confinement::open`
(`AdapterRequest::Oci`, `AdapterRequest::Pod`,
`crates/substrate-host/src/confinement/mod.rs:129-136`). Nothing lets a start or a daemon carry an
image digest: the image is a host-side profile member only (`profile.image`,
`crates/substrate-host/src/confinement/profile.rs:92`), a start's `ConfinementRequest` has no image
member (`crates/substrate-wire/src/lib.rs:1611`), and the SDK sends none. No daemon publishes
`image_digest_pinned`, and the wire has no name for it: `ConfinementProperty::ALL` holds seven
properties without it (`crates/substrate-wire/src/lib.rs:3690-3712`). No story among those session
`substrate` reported on 2026-11-21 (epic `## Upstream` of `epic:tree-daemon-under-enforced-limits`)
plans a daemon that serves them.

## What it blocks

In `story:confinement-level-on-oci-and-pod`, running an action under the `oci` or `pod` level
through a daemon. Sending the level with each start, in the same story, is held by
`upstream-blocker:substrate-sdk-confinement-requires`. `story:confinement-level-required`, which
defines all four levels and refuses `oci` and `pod` where they cannot be given, does not wait on
this.

## What clears it

A Substrate release whose daemon can be configured to open the `oci` adapter and the `pod` adapter
with an image named by digest in the daemon's configuration, publishes their families in its
capability snapshot, publishes `image_digest_pinned` as a wire confinement property (a new member of
`ConfinementProperty`), and reports on each exec the image digest it ran.

## Who

Session `sdc` sent the items missing in Substrate, this one among them, to session `substrate` on
2026-11-21 (epic `## Why`). That session's answer of the same day names no story for it. The image
digest and the published property are not in that request and need to be added to it.
