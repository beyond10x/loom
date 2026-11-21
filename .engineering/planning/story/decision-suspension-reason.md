---
format: aep.planning-md/3
id: story:decision-suspension-reason
kind: story
status: draft
title: Commission holds a run for a decision request and resumes it only once it is decided and any record it produces is recorded
summary: SuspensionReason::Decision, a hold with no action request, resume_after_decision with Recorded or NoRecord
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
revision: 4
---
## Outcome

Commission can hold a run for a decision request and resume it only once the decision is taken
and, where the decided option produces a Canon record, recorded. `SuspensionReason` gains
`Decision` (`DecisionHold`: the request id and what it unblocks, `approval`, `question` or
`outcome`). When an executor returns `Suspended` with a `Decision` reason, `run_with_suspension`
suspends the Run, as for any executor suspension today
(`crates/loom-commission/src/runtime.rs:1908-1924`), and also holds the commission's Suspension
`Pending` with that reason and no held action request.

`resume_after_decision` is given the decided hold (`DecidedHold`: the request id and either
`Recorded` with the request's `produced_record`, or `NoRecord` for a decided option that produces
none). It refuses with no Suspension, a Suspension held for another reason, or another request id;
otherwise it takes the Suspension, resumes the Run, releases the Suspension, and continues the loop
from the case's current revision. It performs no held request, and a case revision that moved
since the hold is expected (an answered question moves an artifact), not a refusal. After a
`NoRecord` hold the loop continues as well: an outcome left untaken leaves the case where it was,
so the run then ends as its frontier decides, directly after the decision (session `sdc`,
2026-11-21, answer d). Commission cannot tell whether an option produces a record; the caller that
builds the `DecidedHold` reads it from the request (`story:question-and-outcome-decisions-hold-runs`),
and Commission only refuses a `DecidedHold` that is neither.

An approval keeps the existing path: the authority provider answers `ApprovalRequired`, the
Suspension's reason is `Authority`, and `resume_after_grant` performs the held request once after
the provider allows it (`runtime.rs:384-392`, `:2374`). A `Decision` hold refuses
`resume_after_grant` (`HeldForDecision`) and an `Authority` hold refuses `resume_after_decision`.

## Why

A run suspends today only on a capability `ApprovalRequired` (`runtime.rs:384-392`);
`NeedsHumanJudgment` ends it with no Suspension (`crates/loom-commission/src/outcome.rs:118-124`),
and an executor's `Suspended` suspends the Run but holds no Suspension, so nothing can resume a run
held for a question or an outcome decision. Resume "once the decision is written to Canon"
(`decisions.md:241-243`) requires `produced_record` for an option that produces one (session
`sdc`, 2026-11-21, point 6 and answer d); the `DecidedHold` type carries it, so Commission sees the
record and never reads Canon or the decision store (`AGENTS.md:96-97`).

## Waits on

Nothing in this epic. `story:child-case-runtime` and `story:deterministic-default-selector`
(`epic:protocol-tree-on-bindings`) also change `ess/commission/domains/responsibility.yaml`; no
edge orders them, because `story:child-case-runtime` waits on an upstream blocker. Whichever is
taken second rebases on the other.

## ESS first

`ess/commission/domains/responsibility.yaml`: `SuspensionReason` gains
`Decision: commission.responsibility.DecisionHold`; `DecisionHold` and `DecidedHold` (with the
union `DecidedRecord`: `Recorded` carrying the record as JSON, `NoRecord`) are added;
`Suspension.action_request_id`, `action`, `arguments` and `expected_case_revision` become optional
with an invariant that they are present unless the reason is `Decision`; the refusals
`HeldForDecision`, `HeldForAuthority` and `OtherDecision` are added. On that commit the commission
drift check fails, and `crates/loom-commission-testkit/tests/decision_suspend_resume.rs` does not
compile.

## Acceptance

- An executor that returns `Suspended(Decision{request_id: "DR-1", unblocks: question})` ends the
  run `Suspended`, and the suspension port holds one `Pending` Suspension with that reason and no
  action request; nothing is performed.
- `resume_after_decision` with `DR-1` and `Recorded` continues the loop at the current case
  revision, after an artifact moved, and the Suspension is released; a second resume is refused
  `NothingSuspended`.
- `resume_after_decision` with `DR-1` and `NoRecord` on an outcome hold releases the Suspension and
  the run ends as the unchanged frontier decides (`NoAdmissibleAction` in the fixture), with
  nothing performed.
- `resume_after_decision` with `DR-2` is refused `OtherDecision`, and the Suspension stays.
- `resume_after_grant` on a `Decision` hold is refused `HeldForDecision`; `resume_after_decision`
  on an `Authority` hold is refused `HeldForAuthority`; neither changes the Suspension.
- A `FileSuspensionStore` holds a `Decision` Suspension across a real process restart, and a
  Suspension file written before this story (with an action request) still decodes
  (`crates/loom-commission-testkit/tests/approval_suspend_resume.rs` passes unchanged).
- A commission holding a `Decision` Suspension starts no Run (`LoopFailure::CommissionSuspended`).
- `crates/loom-commission/Cargo.toml` still depends only on the generated `commission` crate.
- `task check` exits 0.

## Out of scope

Who returns the `Decision` suspension, opening the request, and reading whether its option
produces a record (`story:question-and-outcome-decisions-hold-runs`). Any timer. Changing the
approval path.
