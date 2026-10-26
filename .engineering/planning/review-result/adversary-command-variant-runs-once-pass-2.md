---
format: aep.planning-md/3
id: review-result:adversary-command-variant-runs-once-pass-2
kind: review-result
status: active
title: Adversary pass 2, wave 2026-10-26-cmdv u1, story:command-variant-runs-once, at 1b09f360
relations:
- reviews: story:command-variant-runs-once
revision: 1
---
# Adversary pass 2, wave 2026-10-26-cmdv u1, story:command-variant-runs-once, at 1b09f360

Agent `aep:adversary` (opus). Report as returned (header and findings block verbatim; prose
condensed to the measured items).

```
unit: u1 story:command-variant-runs-once, tree loom-cmdv-u1 at 1b09f360 plus one untracked adversary test file
verdict: green
cases: executed 107→111, red 0
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: scratch only (logs, mutants.sh, reconcile.rs.orig)
needs-coordinator: no
```

Cases in `crates/loom-operator/tests/adv_cmdv_u1_foreground.rs` (own fake API server checking the
uid precondition, a Foreground delete answering with the object still terminating; how a real API
server answers is inferred, not observed on a cluster):
`..._replaces_the_job_over_a_real_foreground_delete`,
`..._a_conflicting_delete_of_the_old_job_is_not_gone`,
`..._an_old_job_already_gone_at_the_delete_is_gone`,
`..._a_restart_while_the_old_job_terminates_still_waits`. All green.

`cargo test -p b10x-loom-operator --locked --no-fail-fast`: 111 passed, 0 failed.

Mutants (`reconcile.rs`): A `:416` 409 -> gone and B `:415` 404 -> not gone survived the
implementor's suite, killed by the new cases; C `:790`, D `:414`, E `:978`, I `:779` killed; G, H
(`:777`, `:788` drop `unremember`) survived every case with no effect found.

The re-pinned `a_new_generation_replaces_the_job`: its dropped status checks moved into
`the_job_waits_for`; the later reconcile and the generation-2 run end are restored by the new case.

```findings
[
{"file": "crates/loom-operator/src/reconcile.rs", "line": 416, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "409 -> Ok(true) survived the implementor's suite (the fake checks no uid precondition); killed by the green guard adv_cmdv_u1_a_conflicting_delete_of_the_old_job_is_not_gone"},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 415, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "404 -> Ok(false) survived the implementor's suite; killed by the green guard adv_cmdv_u1_an_old_job_already_gone_at_the_delete_is_gone"},
{"file": "crates/loom-operator/src/reconcile.rs", "line": 777, "category": "mutant", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "dropping either unremember in workloads_gone (:777, :788) survives every case; no observable effect found"},
{"file": "crates/loom-operator/tests/reconcile.rs", "line": 1301, "category": "judgement", "severity": "note", "verdict": "approve", "origin": "introduced", "message": "the re-pinned test asserts delete and create in one reconcile, which a real API server's Foreground answer (object returned, terminating) is not expected to produce (inferred, not observed); the dropped later-reconcile and run-end coverage is restored by adv_cmdv_u1_a_new_generation_replaces_the_job_over_a_real_foreground_delete"}
]
```
