---
format: aep.planning-md/3
id: story:moved-run-named-outcome
kind: story
status: draft
title: A run whose case moved on to an admissible frontier ends with an outcome that says so
relations:
- decomposes: epic:commission-core
- serves: vision:O1
- depends_on: story:moved-case-outcome
scope:
- confidence: cited
  path: crates/loom-commission-testkit/tests/moved_case_outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission/
revision: 3
---
## Outcome

A run whose case moved to a revision whose frontier still admits an action ends with an outcome
that says so, instead of `NoAdmissibleAction`, which reads the same as an empty frontier.

After `story:moved-case-outcome` (wave 2026-10-07-w3), `run_until_blocked` reloads a moved case and
judges the run on its current frontier: a complete case ends `Completed`, an open obligation the
frontier holds ends `NeedsExternalEvidence`. When that frontier admits an action, the Run, bound to
the revision it left, cannot go on, and it ends `NoAdmissibleAction`
(`crates/loom-commission/src/runtime.rs`, `moved_outcome`; asserted on purpose by
`crates/loom-commission-testkit/tests/moved_case_outcome.rs`). Adversary pass 1 of that unit
(`review-result:adversary-w3-loom-moved-case-outcome-pass-1`, F3) measured it; the ambiguity is the
one `decision-blocker:run-stale-outcome` was filed for. The stale-proposal path, where the runtime
finds the move itself, ends the same way.

## Acceptance

Decided (option A of `decision-blocker:moved-run-admissible-frontier`): a new `RunOutcome` variant
naming the Run's revision and the current one ends a run whose case moved to a frontier that still
admits an action. A case per path asserts it: the executor-reported move (`CaseMoved`) and the move
the runtime finds itself (the stale-proposal path). `crates/loom-commission-testkit/tests/moved_case_outcome.rs`'s
case that asserts `NoAdmissibleAction` for that frontier is rewritten to the new variant. The
CHANGELOG entry names the variant for callers that match `RunOutcome`.

## ESS first

A run-outcome change is a specification change in `ess/commission/` first.

## Source

`review-result:adversary-w3-loom-moved-case-outcome-pass-1`, F3.
