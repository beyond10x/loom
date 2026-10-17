---
format: aep.planning-md/3
id: story:routing-record-names-target
kind: story
status: active
title: A routing record names the route target
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-connectors/src/evidence.rs
- confidence: cited
  path: crates/loom-connectors/tests/adversary_w172_governed_write.rs
- confidence: cited
  path: crates/loom-connectors/tests/support_triage_route.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T17:02:03Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-17T17:02:03Z", actor: "human:timo", revision: 6}
---
## Outcome

The `routing_record` evidence the host evidence adapter records for an applied `ticket.route`
write names the route target, the queue or owner the ticket was routed to, as `support-triage@1`
describes the evidence kind ("the record that the ticket was routed to a queue or owner, naming the
target"). Today the record names the step, the admitted request and Connectors' request and audit
ids, but not the target.

## Acceptance

- An applied route write records a `routing_record` whose facts name the target, taken from the
  admitted arguments or from Connectors' answer, never from model output.
- A write whose target cannot be read records no evidence and the adapter says why.

## Source

Adversary review of `story:connectors-governed-write` (finding 4, a judgement note).

## Decisions

- The fact is `target`, a string: the queue or owner the admitted arguments name (`queue`, or
  `owner` for an owner route; exactly one of them). `support-triage@1` names the kind's meaning
  ("naming the target") but no fact field, so Loom names it.
- The admitted arguments are the source. When Connectors' answer names a queue or owner too and
  it differs, the adapter refuses (`WriteEvidenceError::Unreadable`) and records nothing.
- A target that is absent, not text, empty, or named twice (`queue` and `owner`) is unreadable:
  the adapter reads it before it delivers the observation, so nothing is delivered or recorded,
  and the error names the reason.
- The target goes in `facts` only, not in `provenance`.
- No ESS change: the evidence facts are Canon's `canon-evidence/1` record, which `ess/` does not
  declare; `ess specify validate --path ess --strict-requires` stays green.

## Scope

`crates/loom-connectors/src/evidence.rs` (`WriteEvidenceAdapter::record`),
`crates/loom-connectors/tests/support_triage_route.rs`,
`crates/loom-connectors/tests/adversary_w172_governed_write.rs`, the governed-write row of
`AGENTS.md`, `website/docs/reference/sdk-stability.md`, `CHANGELOG.md`.

## Decision revised: where the target lives

Canon's `canon-evidence/1` record denies unknown fields, and the governor reads each recorded
fact set through it after removing `producer` (`crates/loom-governor/src/lib.rs`, `canon_record`),
so a `target` key would make the record silently stop applying. The target stays in the facts
the adapter records, beside `producer`, as a host field Canon does not read: `canon_record` removes
`target` as it removes `producer`, and a test shows a routing record with a target still
discharges `ticket.routed`. Canon's `result` is not used (it takes part in matching), nor
`provenance`. Scope adds `crates/loom-governor/src/lib.rs` and its tests.
