---
format: aep.planning-md/3
id: decision-blocker:approve-wave-2026-10-08-w8
kind: decision-blocker
status: cleared
title: Approve wave 2026-10-08-w8 (engineering-protocols 0.3.0 pin and investigation obligation)
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T21:28:05Z", actor: "human:timo", revision: 3}
---
## Question

Approve wave 2026-10-08-w8: one unit, `impl/engineering-protocols-030`, delivering
`story:engineering-protocols-030-pin` then `story:incident-investigation-open` in that order, one
implementor, one adversary pass, one integration branch `wave/2026-10-08-w8` and one pull request.

## Why one unit

The pin cannot pass its own acceptance without rewriting the assertions of
`crates/loom-governor/tests/incident_response_slice.rs` that the second story names.

## Risk

engineering-protocols `0.3.0` brings `b10x-assertion-providers`, which enables `serde_json`
`arbitrary_precision` for every crate built with `loom-governor`, `loom-executor` included; the
adversary pass targets it. Canon moves from `branch = "main"` (`d2e09ae`) to tag `0.1.0`.

## Decided

Approved 2026-10-08: option A, one unit `impl/engineering-protocols-030` delivering both stories in order, with an adversary pass on the `serde_json` `arbitrary_precision` unification.
