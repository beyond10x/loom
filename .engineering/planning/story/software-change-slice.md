---
format: aep.planning-md/3
id: story:software-change-slice
kind: story
status: draft
title: 'Software-change vertical slice: stale evidence, projected merge, authority outside the model'
refs:
- provider: commission
  reference: story:software-change-slice
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 1
---
> Re-filed from `beyond10x/commission` `story:software-change-slice` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

The first demonstrator, end to end, as an integration test: a `software.change/1` case at
implementation revision R2, with test evidence only for R1, driven through Commission and Loom on
the AEP governor. The agent may edit and run tests but may not merge; after tests pass on R2, merge
becomes admissible but requires authority.

Observable steps (build pack `START-HERE.md`, section First demonstrator, plus Atlas
`epic:ga-vertical-slices` I-001):

1. stale R1 evidence makes `tests.pass` evaluate `Unknown` at R2, not `True`;
2. `repository.merge` is not executable while it is unavailable;
3. the projected action set changes after R2 test evidence is admitted;
4. authority stays outside the model;
5. a proposal made against a superseded case revision is refused by the governor;
6. Loom cannot execute an action the governor did not project.

The demonstrator steps "a run can suspend and resume around authority" and "the case outlives the
run/session" belong to `story:approval-suspend-resume-slice` (I-003), not here.

## Domain relations

- Commission -> Case, many-to-one, references: `ess/domains/responsibility.yaml`,
  `commission.responsibility.Commission` relation `case`.
- Frontier -> Case, many-to-one, references, carrying `case_revision`:
  `commission.responsibility.Frontier` relation `case`.
- Evidence -> Case, many-to-one, references, carrying `subject_revision`:
  `commission.responsibility.Evidence` relation `case`.

## Acceptance

Driving the ELS `software.change/1` fixture case through Commission and Loom on the AEP governor,
the I-001 integration test observes that with test evidence for R1 only `tests.pass` evaluates
`Unknown` at R2 and `repository.merge` is neither admissible in the frontier nor projected as a
tool; that after a passing test result for R2 is admitted the projected tool set changes and
`repository.merge` is admissible but marked as requiring authority; that a merge proposal whose
model-generated arguments claim approval is not executed while the AuthorityProvider withholds
authority; that a proposal made against the superseded case revision is refused by the governor;
and that the trusted action adapter records no invocation of any action absent from the frontier
current at the call.

## Tests

The tests live under commission `tests/` and drive ELS fixtures through Loom (pinned
dev-dependency). Loom is a dev-dependency pinned by `Cargo.lock`; the pin moves only in a story that
names the Loom change it takes and re-runs all three slices. No loom or els source changes here.

## Notes

- Claim and action ids (`tests.pass`, `repository.merge`) are taken from Atlas
  `epic:ga-vertical-slices` and `docs/contracts/frontier.md`; ELS does not define the
  `software.change/1` fixture yet (TASKBOARD E-002, E-005).
- Sequenced after `epic:governor-adapter` (the epic `depends_on` edge).

## Source

TASKBOARD I-001; build pack `START-HERE.md` section First demonstrator; Atlas
`epic:ga-vertical-slices`.
