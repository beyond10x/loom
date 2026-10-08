---
format: aep.planning-md/3
id: story:incident-investigation-open
kind: story
status: draft
title: Leaving emergency mode keeps the investigation obligation open
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
revision: 1
---
## Outcome

The incident-response slice also shows that leaving emergency mode leaves the investigation open:
after `emergency.leave` becomes Admissible in `inc-492`, an investigation obligation is still
listed as open.

## Acceptance

With a `b10x-canon-engineering` pin whose `incident.response` protocol declares an investigation
obligation, `crates/loom-governor/tests/incident_response_slice.rs` asserts that obligation is open
at the fixture's last state, in place of its current assertion that `restore_service` is the only
obligation.

## Upstream

`incident.response/1` at engineering-protocols `0.1.0` declares only `restore_service`
(`upstream-blocker:incident-investigation-obligation`). The pin moves in a story that names the
protocol change and re-runs all three slices (`story:incident-response-slice`, Tests).

## ESS first

No Loom specification change; the protocol is engineering-protocols'.

## Source

Split from `story:incident-response-slice`, wave 2026-10-08-w6: its implementor found the clause
cannot be checked at the pinned protocol (commit `edc66ba`).
