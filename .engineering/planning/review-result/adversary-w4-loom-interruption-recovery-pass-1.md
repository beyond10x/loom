---
format: aep.planning-md/3
id: review-result:adversary-w4-loom-interruption-recovery-pass-1
kind: review-result
status: active
title: Wave 2026-10-06-w4 adversary, loom story:interruption-recovery, pass 1
relations:
- reviews: story:interruption-recovery
revision: 1
---
```
unit: story:interruption-recovery at a0379ea (worktree loom-w4-interruption-recovery, plus 1 untracked test file)
verdict: red
cases: executed 644→649, red 5
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths (scratch dir adversary-1/ with 2 logs; the assigned build dir)
needs-coordinator: yes (F1 blocker: a resumed run reuses a selection id within one Loom)
```

**1. `git --no-pager diff --stat`**
This is empty, because my only change is an untracked file. `git status --short`:
```
?? crates/loom-executor/tests/adversary_w4_interruption_recovery.rs
```
It is a test file. No implementation file was touched. The file passes clippy `-D warnings` and `cargo fmt --check`.

**2. Cases added** (`crates/loom-executor/tests/adversary_w4_interruption_recovery.rs`)
All five are red now. Each was red the first time it ran, alone, with `cargo test -p b10x-loom-executor --locked --test adversary_w4_interruption_recovery`. Every precondition assert passed; each case fails only at its last assertion.

| Case | Asserts | Failing assertion, verbatim |
|---|---|---|
| `a_second_resume_never_gives_a_selection_id_an_earlier_run_gave` (:141) | 3 selections (merge r1, inspect r2, inspect r3) give 3 records with 3 ids | `left: (2, 2)` `right: (3, 3)`. The r2 inspect selection, which was proposed to Commission, is overwritten by the r3 one |
| `a_governor_outage_at_resume_keeps_the_checkpoint_for_the_next_resume` (:204) | the resume after a one-time governor outage returns the merge proposal and does not ask the model | `left: Suspended(... "model wire refused: Protocol: the script has no further turn")` `right: ProposedAction(... "repository.merge" ...)` |
| `a_resumed_run_interrupted_before_its_gate_keeps_the_checkpoint` (:310) | after a second interrupt (on `InventoryChanged`), the next resume revalidates and proposes the in-flight selection | `left: (Suspended(...no further turn...), Selected)` `right: (ProposedAction(...merge...), Admitted)` |
| `a_resume_narrowed_away_from_the_merge_never_proposes_it_for_a_call_reusing_its_id` (:374) | a resume narrowed to `repository_inspect` never proposes merge | `a resume narrowed to repository_inspect proposed ProposedAction(... action: "repository.merge" ... "squash" ...) for the model's call of repository_inspect that reused the held merge call's id` |
| `a_resume_under_another_narrowing_fails_as_a_changed_configuration` (:422) | a resume whose `admits` differs ends in `LoopError::Config` | `it answered ProposedAction(... "repository.inspect" ...) with Some(Ok(AwaitingApproval { ... }))` |

**3. Suite run** (after the cases existed)
- Command: `cargo test -p b10x-loom-executor --locked --no-fail-fast`
- Result: 644 passed, 5 failed (the five above), 1 ignored. `EXIT=101`.
- `<before>` = 644 executed / 1 ignored, from the implementor's `gate.log` on a0379ea.

**4. Findings**
Origin is `introduced` for all four: `resume_loop`, `interrupt` and `resume_asking` have 0 occurrences at 18d96dd.

| # | Where | Verdict | Sev | Finding | What reaches it |
|---|---|---|---|---|---|
| F1 | governed.rs:731 (with :250, :560) | NEEDS-CHANGE | blocker | A resumed run that selects before recording a turn uses catalogue index `turns_of+1`, the same index as the previous resumed run. Its `attempts` restarts at 0, so the selection id repeats and the stored selection is overwritten. This contradicts lib.rs:306 ("two runs … get different ids"). Possible fix: number selections per Loom or per catalogue across runs, not per run. | `resume_loop` used as documented, default config, a turn with two tool calls, case moving between resumes. No non-test caller exists in the tree. |
| F2 | governed.rs:412 / :432 | NEEDS-CHANGE | warning | `started()` takes the held checkpoint before the gate. A resume that ends before the gate (governor outage at refresh, or an interrupt) releases it. The next resume then starts fresh and asks the model again, and the in-flight selection stays `Selected` forever. This contradicts governed.rs:817 ("an outage of the governor ends the run at the checkpoint, still held") and run.yaml:490. Possible fix: hold the checkpoint again while `Proposer.resuming` is still `Some`. | Same public API, a transient governor error or `Loom::interrupt` from the sink or another thread. Fails toward less authority. |
| F3 | governed.rs:723 | INFEASIBLE | warning | The held call is matched by `call_id` only. A later call to another tool with the same id resolves the held merge, even though the resumed run does not publish merge. The loop's own resume also checks the invoked spec (`expected != *invoked`). Possible fix: also require `entry.action == pending.proposal.action`. | Needs a resume with narrower `admits` (no caller found) and a model that reuses a call id (providers mint call ids). |
| F4 | recovery.rs:168 vs governed.rs:215 | CONFIRMED | warning | The doc says a checkpoint resumed under another config fails. But `admits` is stripped before the checkpoint config is compared, so a changed narrowing is accepted. This is what makes F3 possible. | Any caller that changes `admits` between run and resume. None found in the tree. |

**5. Attacked and not broken**
- Resume against a moved revision, whether the held selection was in flight or already admitted: refused, and the refusal names the revision.
- Same revision but a frontier that no longer lists or admits the action: refused in `resolve`.
- Two concurrent resumes, or resuming while `Active`: refused (`NoUsefulAction`) by `claim_session` under the lock.
- An interrupt racing `end_session`: both serialise on the `recovery` lock.
- Interrupting a session that is `Filed` or unknown: `wrong-state`, and nothing is cancelled.
- ESS transitions against code: every `Interrupted` move the code makes is declared.

**6. Paths written outside the worktree**
- `~/.cache/loom-waves-2026-10-06/w4/interruption-recovery/adversary-1/` (new directory), containing `red-cases.log` and `suite.log`
- `~/.cache/b10x-target/loom-w4-interruption-recovery` (assigned build dir; I added build artifacts only)

```findings
[
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 731, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a resumed run that selects before recording a turn reuses the previous resumed run's catalogue index and restarts attempts at 0, so it gives a selection id an earlier run of the same Loom gave and overwrites that selection's record"},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 412, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the held checkpoint is taken before the resumed loop reaches its gate and released when that run ends earlier (governor outage at refresh, or an interrupt), so the next resume starts fresh and the in-flight selection is never revalidated"},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 723, "category": "judgement", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the held call is matched by call id alone, so a model call of another tool reusing that id resolves the held merge and proposes it from a resume narrowed away from merge; reachable only with a changed narrowing at resume and a model-chosen call id"},
  {"file": "crates/loom-executor/src/recovery.rs", "line": 168, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "resume_loop documents that a checkpoint resumed under another configuration fails, but admits is stripped before the checkpoint config comparison, so a resume with a different narrowing proceeds"}
]
```

Coordinator edit at recording: the adversary closed with the same four findings as a YAML list; they are recorded here as the JSON form the store reads, values unchanged.
