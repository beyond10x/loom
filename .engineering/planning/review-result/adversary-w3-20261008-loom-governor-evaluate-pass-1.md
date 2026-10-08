---
format: aep.planning-md/3
id: review-result:adversary-w3-20261008-loom-governor-evaluate-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w3 adversary, loom story:governor-evaluate, pass 1
relations:
- reviews: story:governor-evaluate
revision: 1
---
# Wave 2026-10-08-w3 adversary, loom story:governor-evaluate, pass 1

Target `impl/governor-evaluate` at 2381837; adversary tests committed as 3ead205
(`crates/loom-governor/tests/evaluate_adversary.rs`, `crates/loom-cli/tests/evaluate_adversary.rs`).

unit: story:governor-evaluate
verdict: CONFIRMED, 4 red cases
cases: executed 77→87, red 4
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: none
needs-coordinator: yes (finding 3)

Suite: `cargo test -p b10x-loom-governor -p b10x-loom-cli --locked --no-fail-fast` EXIT=101; `evaluate_adversary` (governor) 2 passed, 4 failed; `n=1000: decide 0.0217s, refuse 4.2454s, ratio 195.6`.

| # | file:line | verdict / origin | measured |
|---|---|---|---|
| 1 | crates/loom-governor/src/lib.rs:1163 | CONFIRMED / introduced | `attribute` evaluates the snapshot with no records first; on a terminated snapshot whose outcome needs records, a duplicate record or an unreadable time is named Snapshot / `illegitimate-termination` |
| 2 | crates/loom-governor/src/lib.rs:1169 | CONFIRMED / introduced | locating the refused record re-runs Canon on every prefix: quadratic, 4.2 s vs 0.02 s at 1000 records, 42 s at 3000 |
| 3 | crates/loom-governor/src/lib.rs:1093 | CONFIRMED / introduced | a record the governor sets aside (undeclared kind) makes `evaluate` refuse where the governor decides, against the Acceptance's same-case-same-evidence clause |
| 4 | crates/loom-governor/tests/evaluate_adversary.rs:401 | CONFIRMED / introduced | the unit's refusal tests use only open snapshots and never measure locator cost |

Attacked, not broken: equivalence over the 3 bundled protocols with reversed and superseded records; 11 protocol-name spoofs refused as unknown; no shadowing of bundled protocols; no authority passed to Canon; 16 MiB bound, exit codes 0/1/3, stdout/stderr split, JSON depth; no clock, network or model call; no Commission type in the signature.

Coordinator decision on finding 3: the Acceptance stands. A well-formed record that does not apply to the case is set aside as `CanonGovernor` sets it aside; a record that is not a readable `canon-evidence/1` record is still refused naming its index.

```findings
- file: crates/loom-governor/src/lib.rs
  line: 1163
  category: boundary
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: on a terminated snapshot attribute names Snapshot/illegitimate-termination for a duplicate record or an unreadable time (adversary_a_duplicate_record_beside_a_terminated_snapshot_is_named_as_the_record, adversary_an_unreadable_time_beside_a_terminated_snapshot_is_named_as_the_time)
- file: crates/loom-governor/src/lib.rs
  line: 1169
  category: boundary
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: prefix re-evaluation makes refusing N records quadratic, 4.2s vs 0.02s at 1000 and 42s at 3000 (adversary_naming_a_refused_record_costs_a_bounded_multiple_of_deciding)
- file: crates/loom-governor/src/lib.rs
  line: 1093
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: evidence CanonGovernor sets aside makes evaluate refuse where the governor decides, against the Acceptance's same-case-same-evidence clause (adversary_evidence_the_governor_sets_aside_still_gives_the_governors_decision)
- file: crates/loom-governor/tests/evaluate_adversary.rs
  line: 401
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the unit's refusal tests use only open snapshots and never measure locator cost, so findings 1 and 2 pass every gate
```
