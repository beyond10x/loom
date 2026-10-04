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
revision: 1
---
> Re-filed from `beyond10x/commission` `story:approval-suspend-resume-slice` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

Approval suspend/resume across a real process restart, as an integration test on the
`software.change/1` case: execution suspends where `repository.merge` requires authority, the
process exits, a new process resumes from durable state, and the merge happens only after the grant,
at the same case revision. This carries the first-demonstrator steps that I-001 does not.

Observable steps (build pack `START-HERE.md`, section First demonstrator; Atlas
`epic:ga-vertical-slices` I-003):

1. a run can suspend and resume around authority;
2. the case outlives the run and session (the process that held them exits);
3. the case revision before and after the restart is the same;
4. no merge is invoked before authority is granted.

## Domain relations

- Commission -> Run, one-to-many, commission owns its runs: `ess/domains/responsibility.yaml`,
  `commission.responsibility.Commission` relation `runs`.
- Commission -> Case, many-to-one, references (the case is not owned by the commission or its runs,
  so it outlives them): `commission.responsibility.Commission` relation `case`.
- Frontier -> Case, many-to-one, references, carrying `case_revision`:
  `commission.responsibility.Frontier` relation `case`.

## Blocked on

- `decision-blocker:suspended-run-continuity`: does resuming continue the suspended run or start a
  new run of the same commission?
- `decision-blocker:authority-decision-owner`: does an authority decision belong to a commission, a
  case or one action request?

The acceptance below asserts neither answer: it names the commission and the case, not the run.

## Acceptance

Driving the ELS `software.change/1` fixture case through Commission and Loom on the AEP governor
across two separate OS processes, the I-003 integration test observes that execution suspends when
`repository.merge` requires authority, that no merge is invoked before the AuthorityProvider grants
it, and that after the first process exits and the second resumes the same commission from durable
state alone and the grant is given, `repository.merge` is invoked exactly once with the case id and
case revision recorded at suspension equal to those at the merge.

## Tests

The tests live under commission `tests/` and drive ELS fixtures through Loom (pinned
dev-dependency). Loom is a dev-dependency pinned by `Cargo.lock`; the pin moves only in a story that
names the Loom change it takes and re-runs all three slices. No loom or els source changes here.

## Notes

- Suspension and run outcomes are defined in TASKBOARD M-007 (`epic:commission-core`); the AEP
  adapter suspend/resume conformance is `epic:governor-adapter`.

## Source

TASKBOARD I-003; build pack `START-HERE.md` section First demonstrator; Atlas
`epic:ga-vertical-slices`.
