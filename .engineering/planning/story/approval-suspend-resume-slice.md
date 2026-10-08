---
format: aep.planning-md/3
id: story:approval-suspend-resume-slice
kind: story
status: draft
title: Approval suspend/resume across a process restart
refs:
- provider: commission
  reference: story:approval-suspend-resume-slice
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-commission-testkit/src/fake_authority.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/authority.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/mod.rs
- confidence: inferred
  path: crates/loom-commission/src/runtime.rs
- confidence: inferred
  path: crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml
- confidence: inferred
  path: docs/commission/contracts/commission-executor.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: inferred
  path: generated/rust/commission/src
revision: 23
---
> Re-filed from `beyond10x/commission` `story:approval-suspend-resume-slice` at `e61e4f0` (Atlas ADR 0090, loom `story:import-commission`); paths below are Loom's.

## Outcome

Approval suspend/resume across a real process restart, as an integration test on the
`software.change/1` case: execution suspends where `repository.merge` requires authority, the
process exits, a new process resumes from durable state, and the merge happens only after the grant,
at the same case revision. This carries the first-demonstrator steps that I-001
(`story:software-change-slice`) does not.

Observable steps (Atlas `epic:ga-vertical-slices` I-003; `epic:vertical-slices`):

1. a run can suspend and resume around authority;
2. the case outlives the run and session (the process that held them exits);
3. the case revision before and after the restart is the same;
4. no merge is invoked before authority is granted.

## Domain relations

- Commission -> Run, one-to-many, commission owns its runs: `ess/commission/domains/responsibility.yaml`,
  `commission.responsibility.Commission` relation `runs`.
- Commission -> Case, many-to-one, references (the case is not owned by the commission or its runs,
  so it outlives them): `commission.responsibility.Commission` relation `case`.
- Frontier -> Case, many-to-one, references, carrying `case_revision`:
  `commission.responsibility.Frontier` relation `case`.

## Blocked on

- `suspended-run-continuity` (filed in Commission, not in this store): does resuming continue the
  suspended run or start a new run of the same commission? `crates/loom-commission/src/outcome.rs`
  (module doc) already says resume continues the same run; whether that answers it is unchecked.
- `authority-decision-owner` (filed in Commission, not in this store): does an authority decision
  belong to a commission, a case or one action request?

The acceptance below asserts neither answer: it names the commission and the case, not the run.

## Acceptance

Driving the ELS `software.change/1` fixture case (`crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml`)
through Commission and Loom on the AEP governor across two separate OS processes, the I-003
integration test observes that execution suspends when `repository.merge` requires authority, that
no merge is invoked before the AuthorityProvider grants it, and that after the first process exits
and the second resumes the same commission from durable state alone and the grant is given,
`repository.merge` is invoked exactly once with the case id and case revision recorded at suspension
equal to those at the merge.

## Tests

The tests live with the Commission integration tests in `crates/loom-commission-testkit/tests/`
(or beside `crates/loom-cli/tests/slice_run.rs`, which already drives `software.change/1` through
the binary) and drive ELS fixtures through Loom, which is in this workspace. No changes to Loom's
runtime crates (`loom-executor`, `loom-governor`) or to ELS here.

## Notes

- Suspension and run outcomes are defined in M-007 (`epic:commission-core`); the AEP adapter
  suspend/resume conformance belongs to Commission's governor-adapter epic, which is not in this store.
- The in-memory `RunStore` does not keep a suspended run across a restart
  (`crates/loom-commission/src/outcome.rs`, module doc); no file in the tree holds one yet.

## Source

Atlas `epic:ga-vertical-slices` (I-003); `epic:vertical-slices`.
