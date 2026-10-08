---
format: aep.planning-md/3
id: story:conformance-declared-coverage
kind: story
status: draft
title: Conformance suites carry declared coverage and must qualify as passed
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: crates/loom-commission-conformance/tests/conform.rs
- confidence: inferred
  path: crates/loom-conformance/tests/conform.rs
revision: 2
---
## Outcome

Loom's and Commission's conformance suites are synthesized with declared coverage, and a green
`task conform` / `task commission:conform` requires the report's `conformance_status` to be `passed`.

Today both suites are synthesized in the ordinary format, which carries no coverage inventory
(`coverage.knowledge: unknown`), so ESS qualifies a 46-of-46 run of Loom's suite as
`conformance_status: inconclusive`; `crates/loom-conformance/tests/conform.rs` requires only that the
report's statuses agree with its counts (wave 2026-10-07-w2). The implementor of
`story:loom-ess-conformance` probed `--suite-format 5` once: the suite carried a complete inventory
(46 generated, 0 refused) and the report said `conformance_status: passed`.

## Acceptance

- `ess verify conform synthesize` runs with `--suite-format 5` for `ess/` and `ess/commission/`.
- Both conformance tests fail unless `conformance_status` is `passed`, and a copy of a passing
  report with `conformance_status: inconclusive` fails naming it.

## ESS first

No specification change; the synthesis flag and the two test crates change.

## Source

The correction-round report of `story:loom-ess-conformance`, wave 2026-10-07-w2.
