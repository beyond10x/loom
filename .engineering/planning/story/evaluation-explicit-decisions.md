---
format: aep.planning-md/3
id: story:evaluation-explicit-decisions
kind: story
status: draft
title: An evaluation through Loom carries explicit decisions, so a decision-gated outcome can become legitimate
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

An evaluation through Loom can carry explicit decisions: `EvaluationRequest` and the governor take
`canon-decisions/1` entries taken at the case revision with the principal that took each, and Canon
evaluates them, so an outcome that `requires: decision: X` (for example `inconclusive` once a budget
is spent) can become legitimate through Loom.

## Acceptance

`an_outcome_requiring_a_decision_is_legitimate_only_with_that_decision`
(`crates/loom-governor/tests/`): a protocol whose `inconclusive` outcome requires
`decision: explicitly_inconclusive`; terminating the case `inconclusive` is refused without the
decision, and accepted, through both `loom_governor::evaluate` and `CanonGovernor`, when the
decision is supplied at the case's current revision with a principal; a decision taken at an older
revision is refused as Canon refuses it.

## Why (Loom 0.22.0)

- The governor "supplies no explicit decision or authority decision"
  (`crates/loom-governor/src/lib.rs:33-37`); `decide_case` calls `evaluate_with` with
  `Supplied { at, ..Supplied::default() }` (`crates/loom-governor/src/lib.rs:1278-1286`).
- `loom.evaluation.EvaluationRequest` has `protocol`, `snapshot`, `evidence` and `at` only
  (`ess/domains/evaluation.yaml:38-47`).
- Canon already reads `canon-decisions/1` at the version Loom links (its evaluation inputs include
  explicit decisions); only Loom does not pass them.

## Decisions followed

No accepted ADR in this store or in Canon's store names explicit decisions. Canon's cleared
`decision-blocker:outcome-decision-source` settled the source: a new decision construct and
evaluation input (`canon-decisions/1`), not an authority decision and not a case field. This story
passes that input through; Loom does not invent a decision and grants no authority with one.

## Sketch

`EvaluationRequest { .., decisions: List<ExplicitDecision> }` declared in
`ess/domains/evaluation.yaml` first; `CanonGovernor::decide(&case, decision, principal)` records a
decision on the held case, so later `frontier` and `completion` calls evaluate with it.

## Workaround it removes

The host records the decision outside Canon and leaves the outcome blocked.

## Scope (inferred)

`ess/domains/evaluation.yaml`, generated Loom model, `crates/loom-governor/src/lib.rs`,
`crates/loom-governor/tests/`, `crates/loom-sdk`, `CHANGELOG.md` (the request gains a field:
`Breaking:` for a host that builds it with a struct literal).

## Not in scope

Who may take a decision (organizational authority is not Loom's); authority decisions.
