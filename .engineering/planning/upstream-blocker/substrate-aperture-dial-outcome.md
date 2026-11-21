---
format: aep.planning-md/3
id: upstream-blocker:substrate-aperture-dial-outcome
kind: upstream-blocker
status: open
title: Substrate reports no dial outcome for a connection through an egress aperture
summary: A child's connect succeeds at Substrate's own listener; the record has bytes per aperture only
tags:
- sdc
relations:
- blocks: story:tcp-reachability-through-an-aperture
revision: 1
---
## Waiting on

Substrate: a released way for a host to learn, per connection through an egress aperture, whether
the relay's dial of the pinned destination succeeded, was refused, or failed otherwise, without the
host reaching the destination itself. At `0.21.0` (`adfde402`, Loom's pin,
`crates/loom-intake-slice/Cargo.toml:31-32`) the listener inside the sandbox is Substrate's and a
relay outside dials the destination per connection (`crates/substrate-host/src/egress.rs:15-26`), so
a child's connect succeeds before the dial's outcome is known (inferred from that design), and the
applied record carries bytes per aperture (`bytes {to_destination, from_destination}`), not the
outcome (wire `crates/substrate-wire/src/lib.rs:1864-1913`, read by a helper agent). Substrate's
planning store holds no story about it (searched `.engineering/planning` at the tag, which is also
`origin/main`, for collector, probe and dial; none).

## Clears when

A Substrate release tag ships the outcome (on the run record, the applied aperture, or a probe
call on the driver), and `story:tcp-reachability-through-an-aperture` is rewritten against it and
pins that tag.

## Upstream story

None filed. Session `substrate` is asked to draft one; this blocker names the need, not the API.

## Who

Session `substrate` decides whether and how; the operator if Substrate declines.
