---
format: aep.planning-md/3
id: upstream-blocker:substrate-dns-resolver-aperture
kind: upstream-blocker
status: open
title: Substrate serves no name resolution and no UDP to a confined run
summary: No resolver, no resolv.conf, TCP-only egress at 0.21.0; DNS out of scope in its egress design
tags:
- sdc
relations:
- blocks: story:dns-resolution-through-a-resolver-aperture
revision: 1
---
## Waiting on

Substrate: a released, operator-declared way to resolve a name at a declared resolver for a
confined run or for the host on its behalf, reporting the answer (addresses, TTL, NXDOMAIN, or the
failure), with no general UDP egress. At `0.21.0` (`adfde402`, Loom's pin) a confined run gets no
resolver and no `resolv.conf` ("The sandbox never resolves anything: it gets no resolver at all",
`crates/substrate-host/src/aperture.rs:9`), egress is TCP only ("no UDP, no ICMP, no raw sockets",
`docs/design/10-destination-bound-egress.md:58-59`), and the network namespace has no route
(`crates/substrate-host/src/egress.rs:3-8`). Substrate's implemented `story:destination-bound-egress`
lists DNS as out of scope (read by a helper agent:
`.engineering/planning/story/destination-bound-egress.md:63`); no story in its store proposes it.

## Clears when

A Substrate release tag ships such a resolver surface, and
`story:dns-resolution-through-a-resolver-aperture` is rewritten against it and pins that tag.

## Upstream story

None filed. Substrate's design keeps DNS out of its first egress slice, so the request may be
declined; then the operator decides whether `net.host.resolves` is dropped from what Loom collects.

## Who

Session `substrate`; the operator if Substrate declines.
