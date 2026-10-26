---
format: aep.planning-md/3
id: review-result:adversary-command-run-exit-status-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-26-cmdv u2, story:command-run-exit-status, at 9da15ad9
relations:
- reviews: story:command-run-exit-status
revision: 1
---
# Adversary pass 1, wave 2026-10-26-cmdv u2, story:command-run-exit-status, at 9da15ad9

Agent `aep:adversary` (opus). Header and findings block verbatim; prose condensed.

```
unit: u2 story:command-run-exit-status (tree loom-cmdv-u2, head 9da15ad9; findings cover that head plus uncommitted test additions)
verdict: red
cases: executed 117→124, red 1
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only
needs-coordinator: yes
```

Cases appended to `crates/loom-operator/tests/reconcile.rs`:
`adv_a_sidecar_does_not_speak_for_the_program`, `adv_the_jobs_pod_that_ended_last_answers`,
`adv_a_pod_the_job_does_not_control_does_not_speak_for_the_run`,
`adv_a_killed_program_without_an_end_time_reports_137`, `adv_a_new_generation_clears_the_run`,
`adv_a_restarted_controller_reads_the_finished_runs_exit_code_again` (green),
`adv_a_run_counted_failed_while_its_pod_terminates_reports_the_exit_code` (red: status `run: {}`
where 143 was expected, tests/reconcile.rs:2143).

Suite: `cargo test -p b10x-loom-operator --locked --no-fail-fast` exit 101, 123 passed, 1 failed.
Mutants m1 first container (839), m2 `min_by` (843), m3 dropped controller check (833) survived the
unit's suite; killed by the new green cases. Reachability of the red case (a pod deleted or evicted
while running, the Job counting it failed before its container terminated) is inferred from the
Job controller's behaviour, not observed on a cluster.

```findings
[
{"file": "crates/loom-operator/src/reconcile.rs", "line": 1108, "category": "correctness", "severity": "warning", "verdict": "needs-revision", "origin": "introduced", "message": "A run counted failed on status.failed while its pod still terminates is observed before the loom-agent container terminated, and a Failed resource is never read again, so the exit code is lost for good (red: adv_a_run_counted_failed_while_its_pod_terminates_reports_the_exit_code, tests/reconcile.rs:2144); reachability inferred from Job controller behaviour, not observed on a cluster."},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 839, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "Mutants first-container (839), min_by (843) and dropped controller check (833) all survived the unit's suite; the adv_ cases at tests/reconcile.rs:1935,1958,1986 now kill them."},
{"file": "ess/domains/resource.yaml", "line": 113, "category": "contract-drift", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "DeadlineExceeded is named as the typical no-exit-code failure, but the rendered Job sets no activeDeadlineSeconds, so Loom's Job cannot reach it; the reachable case is a deleted or evicted pod."},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 823, "category": "judgement", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "A 403/404 pod list is swallowed without a log, and with no re-read after Completed/Failed a missing Role grant becomes a permanent, silent absence of the exit code."}
]
```
