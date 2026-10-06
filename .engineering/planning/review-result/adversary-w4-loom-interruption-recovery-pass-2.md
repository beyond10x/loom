---
format: aep.planning-md/3
id: review-result:adversary-w4-loom-interruption-recovery-pass-2
kind: review-result
status: active
title: Wave 2026-10-06-w4 adversary, loom story:interruption-recovery, pass 2
relations:
- reviews: story:interruption-recovery
revision: 1
---
I found 4 failures, all from code this unit added. The worst: a run can still propose an action after `Loom::interrupt` has accepted the interrupt.

unit: story:interruption-recovery @ b00e69f (+1 untracked test file)
verdict: red
cases: executed 651→655, red 4
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: `~/.cache/loom-waves-2026-10-06/w4/interruption-recovery/adversary-2/` (5 logs); `~/.cache/b10x-target/loom-w4-interruption-recovery` (assigned build dir)
needs-coordinator: yes

## 1. What I changed

`git --no-pager diff --stat` is empty: no tracked file changed. `git status --short`:
```
?? crates/loom-executor/tests/adversary_w4p2_interruption_recovery.rs
```
That is the only path, and it is a test file. Lease `loom-w4-adv2` was taken on arrival and released at hand-back. Nothing was committed.

## 2. Cases added

Each case was written first and run alone. Command: `cargo test --locked -p b10x-loom-executor --test adversary_w4p2_interruption_recovery -- <name> --exact`, exit 101 each.

| # | test | red at | failing assertion (verbatim) |
|---|---|---|---|
| 1 | `an_interrupted_run_proposes_nothing_when_its_session_is_resumed_before_it_ends` | :129 | `left: ProposedAction(ExecutorOutcomeProposedAction { action: "repository.merge", arguments: ProposedActionArguments(Object([("strategy", Text("squash"))])) })` / `right: NoUsefulAction(Unit(true))` |
| 2 | `a_run_resumed_while_the_interrupted_run_still_reads_its_model_can_be_interrupted` | :205 | `run C holds session S, so the operator's interrupt of S reaches it; S was Some(Filed) after the interrupted run A ended, and the interrupt answered Ok(WrongState { error: SessionStateConflict { state: Filed } })` |
| 3 | `a_governorless_resume_never_proposes_an_in_flight_selection_on_a_moved_case` | :321 | `assertion left != right failed: the in-flight merge, selected at revision 1 and never revalidated, is proposed on revision 2, where the same Loom refused the admitted held merge as stale` (both sides: the `repository.merge` squash proposal) |
| 4 | `a_resumed_run_interrupted_at_its_held_call_leaves_the_selection_unrevalidated` | :401 | `left: (Admitted, 1)` / `right: (Selected, 0)` |

- Cases 1 and 2 use a second thread. Every wait has a 20 s cap.
- Case 2 also passes if a fix refuses the resume while run A is still running.

## 3. Suite run (after the cases existed)

`cargo test --locked -p b10x-loom-executor --no-fail-fast` → 651 passed, 4 failed (exactly the 4 above), 1 ignored, `EXIT=101`. The log is `adversary-2/suite.log` and starts with HEAD.

`<before>` = 651 executed: the implementor's own gate at b00e69f (`gate-c1.log`, test section: 651 passed, 1 ignored). All 5 pass-1 cases pass.

## 4. Findings

| # | where | severity | verdict | what was measured | what reaches it |
|---|---|---|---|---|---|
| F1 | `governed.rs:810` (also `:463`) | blocker | NEEDS-CHANGE | Case 1: run A's interrupt answered `Interrupted`, yet A returns `ProposedAction(merge)`. The "was I interrupted?" check reads the session's current state, and a resume set that state to `Active` and then `Filed`. | The public API used as documented: `interrupt` "from another thread", then `resume_loop` "from `Interrupted`". `interrupt` never cancels a model read: the cancel made at `governed.rs:432` goes only to the loop (`:283`), never to the model port. So the old run's thread routinely outlives the interrupt. No production caller of interrupt or resume_loop exists yet. |
| F2 | `governed.rs:457`/`:466` (also `recovery.rs:132`) | blocker | NEEDS-CHANGE | Case 2: interrupted run A ends after run C resumed the session. `end_session` files C's `Active` session and drops C's cancel along with its own, because both are matched by session id only. The operator's interrupt of C then fails (`WrongState { Filed }`). | Same as F1. It breaks "one run holds a session at a time". |
| F3 | `governed.rs:878` (with `lib.rs:546`) | warning | NEEDS-CHANGE | Case 3: on a Loom without a governor, `revalidate` returns `Ok`. So a never-revalidated in-flight selection from revision 1 is proposed on revision 2. The same Loom refuses an admitted held selection on the same move (`:886-893`): the less-trusted state gets fewer checks. | `Loom::new` with no `with_governor`, then `run_loop`, `interrupt`, `resume_loop`. `recovery.rs:33` and `run.yaml` `ResumeSession` both promise revalidation. No production caller. |
| F4 | `governed.rs:867` (`resolve`) | note | CONFIRMED | Case 4: a resumed run interrupted at its held call still revalidates it. The selection becomes `Admitted` and one `Admitted` revalidation is recorded. The docs say it stays `Selected` (`recovery.rs:8-10`). Nothing is proposed. The next resume then uses the revision/admission check instead of asking the governor again. | The implementor's own technique (interrupt from the sink at `ApprovalRequired`), applied to a resumed run. |

How to fix them (not applied):
- **F1 and F2 share one cause:** a run is identified by its session id alone. There are two ways to fix it, and that choice is why needs-coordinator is yes:
  - (a) A run judges "I was interrupted" by its own cancel token. `stop`, `hold` and `file` act only while this run still holds the session.
  - (b) Refuse a resume from `Interrupted` while the interrupted run is still registered as running.
- **F3:** for in-flight selections, apply the revision and admission check against `offered.frontier` as well.
- **F4:** check `is_interrupted` in `resolve` before revalidating, and defer with the pending selection unchanged.

## 5. Attacked and could not break

- **Pass-1 fixes F1–F4:** they hold. The 5 pass-1 cases are green, and so is the implementor's class test. (tested)
- **Exact held call replayed later in the same resumed run:** it is answered as held, but only through revalidation or the revision/admission check. That grants no more than a fresh selection would. (read only)
- **Second resume of an `Active` session:** returns `NoUsefulAction` before anything is sent (`governed.rs:406`). (read only)
- **Interrupt of a `Filed`, `Interrupted` or unknown session:** returns `wrong-state` and cancels nothing. (read only)
- **Frontier changed at the same revision (status becomes `ApprovalRequired`):** the proposal carries no status, and Commission rechecks authority. (read only)
- **`with_instance` shared by two Looms, and compaction ids without a namespace:** documented decisions (`lib.rs:172`, `:326`), with no caller. (read only)
- **`resume_asking`:** `resume_approval` takes the unchanged path, and the harness-port suites are green. (tested by the suite)
- **ESS lifecycle:** every `TurnRecord` transition the code makes is declared in `run.yaml`. (read only)

## 6. Paths written outside the worktree

- `~/.cache/loom-waves-2026-10-06/w4/interruption-recovery/adversary-2/`: `red-<test>.log` ×4 and `suite.log`
- `~/.cache/b10x-target/loom-w4-interruption-recovery`: assigned build dir; I added the new test binary to it.

## 7. Findings block

```findings
[
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 810, "category": "concurrency", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "An interrupted run whose session another run resumes before it ends judges 'interrupted' from the session's current state, so it proposes its merge although Loom::interrupt answered Interrupted (test adversary_w4p2_interruption_recovery.rs:129)."},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 457, "category": "concurrency", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "end_session stops, holds and files by session id only, so an interrupted run that ends late files the Active session a resumed run holds and drops that run's cancel, and the operator can no longer interrupt it (test :205)."},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 878, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "On a Loom without a governor, revalidate returns Ok, so an in-flight selection from revision 1 is proposed on revision 2, while the same Loom refuses an admitted held selection on the same move (test :321)."},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 867, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "resolve has no is_interrupted check, so a resumed run interrupted at its held call revalidates the in-flight selection to Admitted, against recovery.rs's 'stays Selected' (test :401)."}
]
```
