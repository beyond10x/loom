---
format: aep.planning-md/3
id: review-result:adversary-w6-20261008-loom-connector-read-performed-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w6 adversary, loom story:connector-read-performed, pass 1
relations:
- reviews: story:connector-read-performed
revision: 1
---
# Wave 2026-10-08-w6 adversary, story:connector-read-performed, pass 1

Unit `impl/connector-read-performed` at `b565e96`; cases committed as `1ffe1c7`
(`crates/loom-connectors/tests/adversary_read_performed.rs`, +450).

Verdict: CONFIRMED (red). Cases executed 245→248, red 2. Origin: introduced 2, pre-existing 0,
undecided 0.

| case | asserts | result |
|---|---|---|
| `a_read_naming_an_empty_audit_record_is_an_error` (:403) | read success with `audit_ref: ""`, `audit_status: complete` is `Err`, invoked once | red: got `Performed` with `audit: Some(ConnectorAuditRef(""))` |
| `a_read_whose_failure_carries_a_recorded_attempt_is_an_error_not_a_refusal` (:420) | `Read` binding, error answer carrying a `mutation` classified `refused` (403) or `not_attempted` (503), is `Err`, invoked once | red: got `Refused` |
| `a_read_of_an_operation_the_service_does_not_describe_is_never_invoked` (:444) | `Read` binding of an undescribed operation is `Err` after describe alone | green |

Gate: `cargo test -p b10x-loom-commission -p b10x-loom-commission-testkit -p b10x-loom-connectors --locked --no-fail-fast`: 246 passed, 2 failed, exit 101.

Reach: both only from a Connectors service that breaks its contract (compatibility § 5: record
absence is never replaced by a plausible opaque string; § 2.1: `mutation` is absent for reads).

Attacked and not broken: write bound as `Read` and read bound as `Write` refused before invoking; a
read success carrying an attempt, or with audit status `incomplete`/`unavailable`, is `Err`, invoked
once; every attempt/audit combination in `ConnectorEffects`; no resend on any read failure; an
operation missing from describe is refused locally; no loader builds `ActionBindingData` with a
default `effect`.

```findings
- file: crates/loom-connectors/src/lib.rs
  line: 297
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a read success with audit_ref "" and audit_status complete is answered Performed naming an empty audit record instead of Err
- file: crates/loom-connectors/src/lib.rs
  line: 277
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a Read binding's error answer carrying a mutation (refused/not_attempted) is answered Refused, though the same contradiction on a success is Err
```
