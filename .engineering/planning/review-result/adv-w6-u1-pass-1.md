---
format: aep.planning-md/3
id: review-result:adv-w6-u1-pass-1
kind: review-result
status: active
title: Adversary, loom w6 U1 approval suspend/resume slice, pass 1
relations:
- reviews: story:approval-suspend-resume-slice
revision: 1
---
unit: U1 `story:approval-suspend-resume-slice`, tree `lw6-u1` on `impl/approval-suspend-resume-slice` (HEAD `7ffaaa9`)
verdict: NEEDS-CHANGE; 3 red cases
cases: executed 189→194, red 3
origin: introduced 4 / pre-existing 0 / undecided 0

**Cases added** (untracked, committed with the fix): `crates/loom-commission-testkit/tests/adversary_w6_suspend_resume.rs` (5).

Red:

| case | failed at |
|---|---|
| `two_racing_resumes_merge_at_most_once` | two resumes of one commission each perform `repository.merge` (2, expected 1) |
| `a_new_run_of_a_suspended_commission_leaves_no_suspension_to_merge_again` | a new Run merges, the stored Suspension stays, a later resume merges again |
| `a_crash_between_consuming_the_suspension_and_the_merge_leaves_the_merge_to_be_done` | a crash after the Suspension is removed and before the merge loses the merge; the retry finds nothing suspended |

Green: a port whose remove fails performs nothing and keeps the Suspension; a torn Suspension file is refused and performs nothing.

Suite after the cases: `cargo test -p b10x-loom-commission-testkit --locked --no-fail-fast` 191 passed, 3 failed.

**Findings**

| # | file:line | finding | verdict | origin |
|---|---|---|---|---|
| F1 | `crates/loom-commission/src/runtime.rs:1084` | `resume_after_grant` ignores the port's `Ok(false)` (nothing was stored), so the losing resume of a race still performs the request | CONFIRMED, NEEDS-CHANGE | introduced |
| F2 | `crates/loom-commission/src/runtime.rs:451` | a new Run of a commission with a stored Suspension leaves the Suspension resumable | CONFIRMED, NEEDS-CHANGE | introduced |
| F3 | `crates/loom-commission/src/runtime.rs:1083` | the Suspension is removed before the effect is invoked, so a crash or a later loop failure loses the merge with no durable trace | CONFIRMED | introduced |
| F4 | `ess/commission/domains/responsibility.yaml:669` | the Suspension has one state, `Pending`, and no command consumes it; consume-once and the case-moved refusal live only in a comment, so conformance exercises none of it | NEEDS-CHANGE | introduced |

Decided for the unit (2026-10-13, at the coordinator): spec first, the Suspension's lifecycle gains a consume step: `Pending` → `Resuming` when a resume takes it (a second take is `wrong-state`), removed once the effect was performed. F1: a take that finds nothing or a `Resuming` Suspension refuses and performs nothing. F2: starting a Run of a commission that has a stored Suspension is refused and performs nothing. F3: a resume that finds a `Resuming` Suspension (a crash or failure after the take) refuses with an in-doubt refusal naming the request, so the merge is never performed twice and never lost without a durable trace; settling an in-doubt request is left to the host.

**Could not break:** a resume after the case revision moved; a provider that still withholds or denies; a Suspension of another commission or case; a port that fails on write; a store directory shared by two commissions; runs without a port against 0.16.0.
