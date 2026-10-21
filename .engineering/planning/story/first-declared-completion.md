---
format: aep.planning-md/3
id: story:first-declared-completion
kind: story
status: draft
title: A governor can complete on the first declared legitimate outcome
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A host that builds a `CanonGovernor` can choose that a case completes on the first legitimate
outcome in the protocol's declaration order when several are legitimate at once. A host that
chooses nothing gets today's rule: complete only when exactly one is legitimate.

## Why

On `9e63c11b`, `crates/loom-governor/src/lib.rs:1837-1853` (`completed`): a case completes only
when exactly one declared outcome is `legitimate`; with two at once the case stays open. A host
whose protocol can make two outcomes legitimate together (an outcome with a claim and another with
a decision recorded after it) and whose own rule takes the first declared cannot express it.
"Never a choice between several" is the documented default (`lib.rs:1837-1838`), which other
`CanonGovernor` hosts rely on, so it stays and the other rule is opt-in.

## Design

- ESS `loom.governor.CompletionRule`, an enum `[ExactlyOne, FirstDeclared]`, in
  `ess/domains/governor.yaml`, generated into `generated/rust/loom/`.
- `CanonGovernor::with_completion_rule(CompletionRule)`, default `ExactlyOne`. It is host
  configuration given at construction, never read from a case or a model, and not stored with a
  case.
- `FirstDeclared`: among the outcomes Canon reports `legitimate`, the one the protocol declares
  first, by the order in the admitted protocol definition, never the order of the decision
  document's map. An outcome Canon holds blocked is never taken. With one legitimate outcome both
  rules agree.
- `evaluate` (`loom_governor::evaluate`) keeps `ExactlyOne`.

## ESS first

`ess/domains/governor.yaml` gains `CompletionRule`; `task generate` regenerates. Red test:
`crates/loom-governor/tests/completion_rule.rs`
`first_declared_completes_on_the_first_declared_legitimate_outcome`.

## Acceptance

- `first_declared_completes_on_the_first_declared_legitimate_outcome`: a protocol declaring
  outcomes `b_first` then `a_second` (declaration order differs from name order), both legitimate,
  completes `b_first` under `FirstDeclared` and stays open under `ExactlyOne`.
- `a_blocked_outcome_is_never_completed_under_first_declared`.
- The existing completion tests stay green under the default.

## Not in scope

- `evaluate` and `b10x-loom evaluate`.
- Commission's runtime (stop policy, budgets, pending answers).

## Scope

- `ess/domains/governor.yaml`
- `generated/rust/loom/`
- `crates/loom-governor/src/lib.rs`
- `crates/loom-governor/tests/completion_rule.rs`
- `AGENTS.md` § Governor, `CHANGELOG.md`, `website/docs/reference/ess/`, `website/data/ess/`
  (regenerated)
