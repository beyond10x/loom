---
format: aep.planning-md/3
id: review-result:adv-w7-u3-pass-1
kind: review-result
status: active
title: Adversary, loom w7 U3 approve and resume after a grant, pass 1
relations:
- reviews: story:cli-resume-after-grant
revision: 1
---
unit: U3 `story:cli-resume-after-grant`, tree `lw7-u3` on `impl/cli-resume-after-grant` (findings on `56ecb7d`, cases committed as `95f0f63`)
verdict: NEEDS-CHANGE; 3 red cases
cases: executed 82→88, red 3
origin: introduced 3 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-cli/tests/adversary_w7_u3_resume.rs` (6), each through the binary.

Red:

| case | failed at |
|---|---|
| `a_commit_made_after_approval_is_not_handed_off_as_the_approved_head` | the hand-off named a `HEAD` committed after the approval |
| `the_same_head_is_not_handed_off_twice_through_a_re_proposed_merge` | a re-proposed merge, approved again, handed off the same `HEAD` a second time |
| `a_resume_that_stops_approval_required_leaves_something_to_approve` | the resume exited 0 printing `stopped: ApprovalRequired (repository.merge)`; the `approve` it invites was refused `NothingSuspended` |

Green: a crash between take and hand-off leaves the request in doubt (exit 11, no merge record);
two resumes at once write one merge record; a corrupt grant or a deleted workspace resumes nothing
and calls no model.

Suite after the cases: `cargo test -p b10x-loom-cli --locked --no-fail-fast` 85 passed, 3 failed;
`task intake-drift`, `task no-hand-model`, `task commission:deps-guard` exit 0.

**Findings**

| # | file:line | finding | verdict | origin |
|---|---|---|---|---|
| F1 | `crates/loom-intake-slice/src/intent.rs:621`, `handoff.rs:186` | resume never reports `HEAD` to the governor, so a commit after the Suspension does not move the case and is handed off | CONFIRMED, NEEDS-CHANGE | introduced |
| F2 | `crates/loom-intake-slice/src/handoff.rs:43` | the hand-off changes nothing in the case, so a re-proposed merge holds a new Suspension and the same `HEAD` can be handed off twice | CONFIRMED, NEEDS-CHANGE | introduced |
| F3 | `crates/loom-cli/src/lib.rs` (`RESUME_HELP`) | a resume ending `AwaitingApproval` with no Suspension prints the approvable line and exits 0 | CONFIRMED, NEEDS-CHANGE | introduced |

Decided for the unit (2026-10-13, at the coordinator): F1: resume reports the current `HEAD` to the
governor before `resume_after_grant`, so a moved workspace is refused `CaseMoved`; the hand-off also
refuses a `HEAD` other than the case's implementation revision. F2: `approve` refuses, with its own
status, a Suspension whose request would hand off a `HEAD` the commission already handed off. F3:
with `--state`, a run ending `ApprovalRequired` without a held Suspension prints
`stopped: MergeHandedOff (…)` when its `HEAD` was handed off, else
`stopped: ApprovalRequired (…), nothing suspended` with its own status; without `--state`, nothing
changes.
