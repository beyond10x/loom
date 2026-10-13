---
format: aep.planning-md/3
id: story:approval-suspend-resume-slice
kind: story
status: implemented
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
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-commission-testkit/src/fake_authority.rs
- confidence: inferred
  path: crates/loom-commission-testkit/src/file_suspension.rs
- confidence: inferred
  path: crates/loom-commission-testkit/src/lib.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests
- confidence: inferred
  path: crates/loom-commission-testkit/tests/approval_suspend_resume.rs
- confidence: cited
  path: crates/loom-commission/src/outcome.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/authority.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/mod.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/suspension.rs
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
- confidence: inferred
  path: website/docs/reference/commission
revision: 34
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T05:12:40Z", actor: "human:timo", revision: 31}
- {from: "proposed", to: "active", at: "2026-10-13T05:12:40Z", actor: "human:timo", revision: 32}
- {from: "active", to: "implemented", at: "2026-10-13T06:22:43Z", actor: "human:timo", revision: 34, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
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
- New: Commission -> Suspension, one-to-at-most-one, owned (the suspension lives and dies with the
  commission, `decision-blocker:suspension-durable-record`, option B). A Suspension holds the
  pending approval request: the run it suspended, the `ActionRequest` (action and
  `expected_case_revision`) it was raised for, and the `SuspensionReason` (`Authority`).

## Settled

- `suspended-run-continuity`: resuming continues the same run with its id and case revision
  (`responsibility.yaml:594-595`; `crates/loom-commission/src/outcome.rs:10-11`).
- `authority-decision-owner`: a decision belongs to one action request at one case revision
  (`responsibility.yaml:721-722`; `crates/loom-commission/src/ports/authority.rs:11-12`).
- Where the record lives: a durable suspension port Commission defines and the host implements
  (`decision-blocker:suspension-durable-record`, option B).
- How a run reaches `Suspended` (decided 2026-10-13 at wave planning, reversible): when the runtime
  is given a suspension port and a step needs authority the provider has not granted, the runtime
  runs `SuspendRun` with reason `Authority`, writes the Suspension through the port, and the Run
  still ends `NeedsAuthority` for the caller, the end a withheld grant already gave in 0.16.0
  (`AwaitingApproval` comes only from the approval gate, where no request is pending, and writes
  no Suspension). `ResumeRun` gains a `storage-failed` outcome, breaking for callers that match
  `ResumeRunOutcome` exhaustively; the CHANGELOG names it. Without a suspension port the runtime behaves as in
  0.16.0, so no caller that matches `RunOutcome` sees a change. A resume entry continues the same
  run from the stored Suspension after the grant (`ResumeRun`), and refuses when the case revision
  moved.

## Scope today (scoped 2026-10-13)

- No run is suspended on an approval today: `Track::end` takes no store and never runs `SuspendRun`
  (`crates/loom-commission/src/runtime.rs:419`, `:535`); `run_until_blocked` always starts a new run
  (`runtime.rs:330-359`).
- `SuspendRun` does not store its reason (`responsibility.yaml:806-807`); `ResumeRun` declares no
  `storage-failed` outcome (`:830-845`).
- `crates/loom-commission-testkit/tests/durable_run_storage.rs` persists nothing; `RunStore` is in
  memory.
- The two-process pattern to copy: `crates/loom-governor/tests/restart_durability.rs` reruns its own
  test binary (`current_exe()` with a role variable) over one store directory.

## ESS first

The unit's first commit changes only `ess/commission/`: the `Suspension` entity, its relation to
`Commission`, the reason on `SuspendRun`, and a `storage-failed` outcome on `ResumeRun`. On it,
`task commission:drift` fails until the generated model is regenerated, and the new test
`approval_suspends_and_resumes_across_a_process_restart` fails; the run is recorded. Later commits
make both pass without changing `ess/`.

## Acceptance

Driving the ELS `software.change/1` fixture case (`crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml`)
through Commission and Loom on the AEP governor across two separate OS processes, the I-003
integration test observes that execution suspends when `repository.merge` requires authority, that
no merge is invoked before the AuthorityProvider grants it, and that after the first process exits
and the second resumes the same commission from durable state alone and the grant is given,
`repository.merge` is invoked exactly once with the case id and case revision recorded at suspension
equal to those at the merge.

## Tests

- `crates/loom-commission-testkit/tests/approval_suspend_resume.rs`: the two-process test above
  (the test binary reruns itself as the second process), plus: no suspension port, no change to the
  0.16.0 outcome; a resume after the case revision moved is refused and invokes nothing; a resume
  with no stored suspension is refused.
- The file-backed suspension store the test uses is the testkit's
  (`crates/loom-commission-testkit/src/file_suspension.rs`); a production host store is not in this
  story.
- No changes to `loom-executor`, `loom-governor` or ELS.

## Notes

- Suspension and run outcomes are defined in M-007 (`epic:commission-core`); the AEP adapter
  suspend/resume conformance belongs to Commission's governor-adapter epic, which is not in this store.

## Source

Atlas `epic:ga-vertical-slices` (I-003); `epic:vertical-slices`.
