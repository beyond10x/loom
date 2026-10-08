---
format: aep.planning-md/3
id: story:software-change-slice
kind: story
status: active
title: 'Software-change vertical slice: stale evidence, projected merge, authority outside the model'
refs:
- provider: commission
  reference: story:software-change-slice
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: crates/loom-governor/Cargo.toml
- confidence: inferred
  path: crates/loom-governor/tests
- confidence: cited
  path: crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml
- confidence: inferred
  path: crates/loom-intake-slice/tests
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T17:26:32Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-08T17:26:32Z", actor: "human:timo", revision: 9}
---
> Re-filed from `beyond10x/commission` `story:software-change-slice` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090 (loom `story:import-commission`); paths below are Loom's.

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

- Commission -> Case, many-to-one, references: `ess/commission/domains/responsibility.yaml`,
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

The test is a Rust integration test in this workspace. It drives the `chg-1842` fixture
(`crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml`) through Commission's runtime
(`crates/loom-commission`), Loom's executor (`crates/loom-executor`) and the governor
(`crates/loom-governor`, `CanonGovernor`), with authority from `crates/loom-commission-testkit`
(`StaticAuthorityProvider`). Commission, Loom and the governor are path crates of one workspace,
so no dependency pin moves here. The change adds tests only: no `crates/*/src` source and no
engineering-protocols (ELS) change.

## Notes

- Claim and action ids (`tests.pass`, `repository.merge`) are taken from Atlas
  `epic:ga-vertical-slices` and `docs/commission/contracts/frontier.md`. ELS `software.change/1`
  ships as `software-change@1` in engineering-protocols `0.1.0`; its fixture case `chg-1842` is copied
  at `crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml`.
- Sequenced after `epic:governor` (which replaces Commission's `epic:governor-adapter`).
- Unchecked: which of the six steps existing tests already cover; `crates/loom-cli/tests/slice_run.rs`
  already stops at `ApprovalRequired (repository.merge)`.

## Source

TASKBOARD I-001; build pack `START-HERE.md` section First demonstrator; Atlas
`epic:ga-vertical-slices`.
