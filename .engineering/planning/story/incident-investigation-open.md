---
format: aep.planning-md/3
id: story:incident-investigation-open
kind: story
status: active
title: Leaving emergency mode keeps the investigation obligation open
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- depends_on: story:engineering-protocols-030-pin
scope:
- confidence: cited
  path: crates/loom-governor/tests/fixtures/inc-492.fixture.yaml
- confidence: cited
  path: crates/loom-governor/tests/incident_response_slice.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T21:28:06Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-08T21:28:06Z", actor: "human:timo", revision: 6}
---
## Outcome

The incident-response slice also shows that leaving emergency mode leaves the investigation open:
after `emergency.leave` becomes Admissible in `inc-492` (state `service-restored`), the obligation
`investigate_cause` is still listed as open, and it is discharged only once a cause analysis
identifies the cause.

## Acceptance

With `b10x-canon-engineering` at engineering-protocols `0.3.0`,
`crates/loom-governor/tests/incident_response_slice.rs` applies all seven states of `inc-492`. At
`service-restored`, the first state at which `emergency.leave` is Admissible, it asserts that
`cause.identified` is Unknown, `investigate_cause` is open and `restore_service` is discharged. At
the last state, `cause-identified-after-restore`, it asserts that `investigate_cause` is discharged
and `emergency.leave` is still Admissible. The assertion that `restore_service` is the only
obligation is removed. `cargo test -p b10x-loom-governor --test incident_response_slice` exits 0.

## Upstream

`incident.response/1` at engineering-protocols `0.1.0` declared only `restore_service`
(`upstream-blocker:incident-investigation-obligation`, cleared: `0.3.0` declares
`investigate_cause`). The pin moves in `story:engineering-protocols-030-pin`, which re-runs all
three slices; both stories are one unit, because the pin cannot pass without rewriting the
assertions this story names.

## ESS first

No Loom specification change; the protocol is engineering-protocols'.

## Source

Split from `story:incident-response-slice`, wave 2026-10-08-w6: its implementor found the clause
cannot be checked at the pinned protocol (commit `edc66ba`). Acceptance reworded at scoping: at
`0.3.0` the fixture's last state discharges `investigate_cause`.
