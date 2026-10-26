---
format: aep.planning-md/3
id: review-result:adversary-command-run-exit-status-pass-2
kind: review-result
status: active
title: Adversary pass 2, wave 2026-10-26-cmdv u2, story:command-run-exit-status, at e068878e
relations:
- reviews: story:command-run-exit-status
revision: 1
---
# Adversary pass 2, wave 2026-10-26-cmdv u2, story:command-run-exit-status, at e068878e

Agent `aep:adversary` (opus). Header and findings block verbatim; prose condensed.

```
unit: u2 story:command-run-exit-status (tree loom-cmdv-u2, head e068878e, plus 5 uncommitted adv_ cases)
verdict: red
cases: executed 124→129, red 2
origin: introduced 2, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only
needs-coordinator: yes
```

Cases appended to `crates/loom-operator/tests/reconcile.rs`:
`adv_a_pod_stuck_terminating_past_its_grace_period_does_not_hold_the_run_open` (red),
`adv_a_job_deleted_while_its_pod_terminates_is_not_run_again` (red),
`adv_a_terminating_pod_holds_the_run_open_and_retries_after_30_s`,
`adv_a_refused_pod_list_is_not_noted_on_the_next_generation`,
`adv_a_pod_list_refused_after_the_wait_records_the_run_with_the_note` (green).

Suite: `cargo test -p b10x-loom-operator --locked --no-fail-fast` exit 101; `tests/reconcile.rs`
40 passed, 2 failed. Mutants m1 (wait requeues 300 s), m2 (wait publishes nothing), m3 (note never
cleared), m4 (note on an Unknown condition) survived the unit's suite and are killed by the new
green cases; m5 ("Succeeded" dropped from final phases) survives everything, judged unreachable.
The re-pinned `a_failed_run_without_a_terminated_container_reports_the_jobs_reason` was re-pinned,
not relaxed. Reachability of F1 (an unreachable node) is inferred, not observed; F2 needs a manual
Job delete.

```findings
[
{"file": "crates/loom-operator/src/reconcile.rs", "line": 905, "category": "acceptance", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "The wait for a terminating pod has no limit: a pod stuck terminating (unreachable node) keeps a Command resource Running/RunNotFinished for good while its Job reports the run failed (red: adv_a_pod_stuck_terminating_past_its_grace_period_does_not_hold_the_run_open, tests/reconcile.rs:2232); reachability inferred, not observed."},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 1224, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "A Job deleted while the controller waits for its counted-failed pod is created again and the program runs a second time for the same generation, against D2 and AGENTS.md:951 (red: adv_a_job_deleted_while_its_pod_terminates_is_not_run_again, tests/reconcile.rs:2258)."},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 1228, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "Mutants m1 (wait requeues 300 s), m2 (wait publishes nothing), m3 (refusal note never cleared, :810) and m4 (note on an Unknown Completed, status.rs:247) survived the unit's suite; the green adv_ cases at tests/reconcile.rs:2193 and :2285 now kill them."}
]
```
