---
format: aep.planning-md/3
id: story:handoff-holds-no-suspension
kind: story
status: active
title: A merge proposed again at a handed-off HEAD holds no Suspension
relations:
- serves: vision:O1
- informed_by: review-result:adv-w7-u3-pass-1
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/adversary_w7_u3_resume.rs
- confidence: cited
  path: crates/loom-cli/tests/resume_after_grant.rs
- confidence: cited
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-commission-conformance/src/store.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/approval_suspend_resume.rs
- confidence: cited
  path: crates/loom-commission/src/ports/suspension.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-intake-slice/src/handoff.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: generated/rust/commission
- confidence: inferred
  path: website/docs/guides/run-an-intent.md
- confidence: inferred
  path: website/docs/reference/commission
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T11:48:53Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-13T11:48:53Z", actor: "human:timo", revision: 6}
---
## Outcome

A merge the agent proposes again at a `HEAD` its commission already handed off holds no
Suspension: the run ends `MergeHandedOff` and leaves nothing in the state directory to approve.

## Why

Since `story:cli-resume-after-grant`, the merge hand-off writes a record and changes nothing in the
case, so the frontier still offers `repository.merge`. A merge proposed again at the handed-off
`HEAD` is held as a Suspension that `approve` refuses (`AlreadyHandedOff`); the Suspension and its
resume context stay until something moves the case (adversary pass 1 of that story,
`review-result:adv-w7-u3-pass-1`, F2).

## Decision (2026-10-13)

The runtime asks the host first, inside this repository: a merge proposed again at a handed-off
`HEAD` consults the host's hand-off record before a Suspension is held, so `approve` is never
reached. The hand-off does not become a case observation, and no protocol outside Loom changes.

Only a resume meets this case: `run --state` mints a fresh commission each time and hand-offs are
recorded per commission, so a plain run never matches one.

## ESS first

The first commit changes only `ess/commission/`: a new outcome of
`commission.responsibility.HoldSuspension` saying the host already settled the request the
Suspension would hold (with its error carrying the suspension, the action request and the reason),
and the Suspension's description saying a settled hold leaves nothing held. On it
`task commission:drift` and the synthesized conformance scenario forcing the new outcome
(`task commission:conform`) fail; later commits make both pass without changing `ess/`.

## Acceptance

After `approve` and `resume` handed off a merge at `HEAD` `h`, when the resumed agent proposes
`repository.merge` again at `h`, the run ends `stopped: MergeHandedOff (<request>, HEAD h)`, the
state directory holds no Suspension and no resume context for the commission, and `approve
<commission> repository.merge --state <dir>` is refused `NothingSuspended`. A hold that the host
does not settle is held exactly as before (`approval_suspend_resume.rs` stays green), and a port
that answers the new outcome ends the run with no Suspension and no loop error.

## Not in scope

A work branch fast-forwarded after the grant; recording the hand-off in the case.

## Scope

Derived 2026-10-13 by `story-scoper` (medium confidence).

- Hold site: `crates/loom-commission/src/runtime.rs` (`suspend_for_approval`, which turns any
  answer but `Held` into `LoopFailure::SuspensionNotHeld`); `ports/suspension.rs`
  (`SuspensionPort::hold`; `decide_hold` likely unchanged).
- Specification: `ess/commission/domains/responsibility.yaml` (`HoldSuspension` outcomes, errors,
  the Suspension's description); generated `generated/rust/commission/`,
  `website/docs/reference/commission/`.
- Conformance: `crates/loom-commission-conformance/src/lib.rs` (`force_outcome`,
  `hold_suspension`), `src/store.rs` (`ScenarioSuspensions`).
- Host: `crates/loom-intake-slice/src/run.rs` (`Mode::Resuming`, `drive`, `approval_gate`),
  `handoff.rs` (`MergeHandoffStore::handed_off`), `intent.rs` (`resume_prepared`, `settle`): a
  `SuspensionPort` wrapper used only in a resume answers the new outcome for a `repository.merge`
  at a handed-off `HEAD` and passes every other call to the file store.
- Tests: `crates/loom-cli/tests/resume_after_grant.rs`
  (`approve_refuses_a_suspension_that_would_hand_off_a_head_already_handed_off` flips to no
  Suspension and `NothingSuspended`), `adversary_w7_u3_resume.rs`,
  `crates/loom-commission-testkit/tests/approval_suspend_resume.rs` (a port that settles the hold).
- Documents: `AGENTS.md` § Commission and § Intake and the command line, `README.md`,
  `website/docs/guides/run-an-intent.md`, the comment in `ess/intake/domains/routing.yaml`,
  `crates/loom-cli/src/lib.rs` help, `CHANGELOG.md` Unreleased.
- Package gates: `b10x-loom-commission`, `b10x-loom-commission-conformance`,
  `b10x-loom-commission-testkit`, `b10x-loom-intake-slice`, `b10x-loom-cli`.
