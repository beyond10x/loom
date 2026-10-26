---
format: aep.planning-md/3
id: review-result:adversary-command-variant-runs-once-pass-1
kind: review-result
status: active
title: Adversary pass 1, wave 2026-10-26-cmdv u1, story:command-variant-runs-once, at b5e0ab0d
relations:
- reviews: story:command-variant-runs-once
revision: 1
---
# Adversary pass 1, wave 2026-10-26-cmdv u1, story:command-variant-runs-once, at b5e0ab0d

Agent `aep:adversary` (opus). Report as returned (header, findings block verbatim; prose condensed
to the measured items).

```
unit: u1 story:command-variant-runs-once, tree loom-cmdv-u1 at b5e0ab0d plus uncommitted adversary tests
verdict: red
cases: executed 100→105, red 1
origin: introduced 5, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (adv_reconcile_append.rs, reconcile.rs.orig, suite.json, copy/)
needs-coordinator: yes
```

Cases added at the end of `crates/loom-operator/tests/reconcile.rs`:
`adv_an_active_job_is_not_applied_again_with_a_changed_pod_template` (red),
`adv_a_command_deployment_of_an_earlier_release_is_deleted`,
`adv_after_a_restart_an_agent_serve_generation_deletes_the_job_it_finds_by_label`,
`adv_a_run_ends_by_its_count_or_its_condition_alone`,
`adv_after_a_restart_a_finished_job_is_observed_and_not_run_again` (green).

Red output: "an active Job of the current generation is applied again with another pod template
(init image ... 0.40.0 -> 0.41.0); the API server refuses that as immutable"
(`tests/reconcile.rs:1397`). What reaches it: the `stage-secret-files` init container takes the
configured default image, which every release bumps, so a controller upgrade during an active run
fails each reconcile with 422 until the run ends.

Mutants (`reconcile.rs`): M1 `succeeded > 0`, M8 `holds("Complete")`, M2 `holds("Failed")`,
M7 `failed > 0`, M3 condition status survived the implementor's suite and are killed by
`adv_a_run_ends_by_its_count_or_its_condition_alone`; M4 Job out of `Kind::ALL` killed; M5 drop
`.owns(jobs)` survived every case (`run()` has no test); M6 drop Background propagation killed by
`a_new_generation_replaces_the_job`.

```findings
[
{"file": "crates/loom-operator/src/reconcile.rs", "line": 764, "category": "correctness", "severity": "blocker", "verdict": "needs-revision", "origin": "introduced", "message": "an active Job of the current generation is sent again by server-side apply every reconcile, so a pod-template change from a controller upgrade (agentImage.tag in the stage-secret-files init container, defaultResources) gets 422 immutable until the run ends and blocks the NetworkPolicy and Service apply; red case adv_an_active_job_is_not_applied_again_with_a_changed_pod_template (tests/reconcile.rs:1397)"},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 736, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "five run_phase mutants (dropped count, dropped condition, ignored condition status) survived the implementor's suite; now killed by the green case adv_a_run_ends_by_its_count_or_its_condition_alone"},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 990, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "dropping .owns(jobs) survives every case; a finished run would reach the status only after REQUEUE (300 s); not testable without a fake that supports watches"},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 405, "category": "judgement", "severity": "warning", "verdict": "approve", "origin": "introduced", "message": "Background propagation plus create-on-next-reconcile (and Job-before-Deployment on upgrade, :956) lets the old pod and the new run overlap on the same workspace; Foreground propagation and waiting for the Job to be gone would run them one after the other"},
{"file": "docs/design/loom-agent-crd-deployment.md", "line": 203, "category": "contract-drift", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "the RBAC row in design section 5.2 lists five child kinds and does not list jobs, which the chart Role now grants"}
]
```
