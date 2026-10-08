---
format: aep.planning-md/3
id: review-result:adversary-w5-20261008-loom-connectors-invoker-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w5 adversary, loom story:connectors-invoker, pass 1
relations:
- reviews: story:connectors-invoker
revision: 1
---
# Wave 2026-10-08-w5 adversary, loom story:connectors-invoker, pass 1

Target `impl/connectors-invoker` at f61e67f; adversary tests committed as f77f7ad
(`crates/loom-connectors/tests/adversary.rs`, 15 cases).

unit: story:connectors-invoker
verdict: NEEDS-CHANGE, 2 red cases
cases: executed 4→19, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: none (empty scratch directory)
needs-coordinator: no

Suite: `cargo test --locked -p b10x-loom-connectors --no-fail-fast` EXIT=101; `adversary.rs` 13 passed, 2 failed; `connectors_invoker.rs` 4 passed.

| # | file:line | verdict / origin | finding | routed |
|---|---|---|---|---|
| 1 | crates/loom-connectors/src/lib.rs:265 | NEEDS-CHANGE / introduced | a success whose `attempt.instance` names another instance answers `Performed` | fix: `Err` unless the attempt names the bound instance |
| 2 | crates/loom-connectors/src/lib.rs:317 | NEEDS-CHANGE / introduced | an admitted integer beyond u64 is sent as a rounded float | fix: `Err` before the call when the number cannot be sent unchanged |
| 3 | crates/loom-connectors/src/lib.rs:219 | CONFIRMED / introduced | the credential reference appears in the unresolved-credential error and in `Debug` | kept: the spec declares it a non-secret reference |
| 4 | crates/loom-connectors/src/lib.rs:269 | CONFIRMED / introduced | every successful read answers `Err`: Connectors v0.35.0 records no attempt for reads and the port requires one on `Performed` | documented as a known limit; `story:connector-read-performed` |

Not broken: all first-binding error codes, `outcome_unknown`, `upstream_protocol`, `applied` with an attempt-store cause, refusal before the audit anchor, HTTP 404/503, truncated body, mismatched request id, `replayed: true`, null attempt; one describe and one invoke with no resend; plaintext refused without `allow_plaintext`; no secret in errors or `Debug`; no panic or deadlock inside current-thread or multi-thread runtimes; 0 Connectors crates under Commission or the executor (`cargo tree`).
