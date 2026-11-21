---
format: aep.planning-md/3
id: review-result:decision-suspension-reason-security-2
kind: review-result
status: active
title: Security review pass 2 (last) on story:decision-suspension-reason (wave 2026-11-21-w3, unit U2, head 61e38aad)
relations:
- reviews: story:decision-suspension-reason
revision: 1
---
unit: story:decision-suspension-reason (w3 U2), pass 2, head 61e38aad, tree clean apart from my 1 new test file
verdict: green
cases: executed 527→531, red 0
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: 1 directory, `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w3/u2/sec2/` (scripts, logs, reconstructed originals of my pass-1 files)
needs-coordinator: no

The correction holds: my pass-1 blocker is fixed for the right reason, and I found one small new note. `git diff --stat` is empty; my one new file is untracked: `crates/loom-commission-testkit/tests/security_w20261121w3_u2_pass2.rs`, 4 cases, all green. Suite: `ESS_TOOLCHAIN=0.86.0 cargo test -p b10x-loom-commission -p b10x-loom-commission-conformance -p b10x-loom-commission-testkit --locked --no-fail-fast` exited 0, 531 passed, 104 binaries.

1. **The F1 rule on every path.** No interleaving lets a run continue without its own decision, or continue twice. `security_pass2_a_stale_decision_resume_never_continues_a_run_held_for_another_request ... ok` runs a stale DR-1 resume against a later DR-2 hold in 8 combinations: in-memory store and `FileSuspensionStore`, DR-1 read Pending or Resuming, DR-2 Pending or Resuming. In every one nothing continues, nothing is performed, and the DR-2 hold is left as it was. A Resuming DR-2 hold takes the release-first path (take answers `wrong-state`, then the release answers `other-decision`), and the resume is refused `OtherDecision{DR-1,DR-2}` in both stores. In the file store the comparison runs inside the same locked step as the state change: `decide_*` is called within `locked` (file_suspension.rs:193-201, unchanged). Only one release can succeed, so a run cannot continue twice. My pass-1 red case now fails at the take with `other-decision`, the right reason.
2. **What can still take or release a Decision hold without naming it.** Nothing. The approval-side callers name an action request, so `other_request` refuses them before any other check: `resume_after_grant` (runtime.rs:2642) and `release_taken` (:2315). `security_pass2_a_take_or_release_naming_an_action_request_never_consumes_a_decision_hold ... ok`, in both stores. The take/release literals in `grant.rs` are in its test module only. `handoff.rs` forwards to the inner store. The governor routes everything through `decide_*`. The conformance forcing (`misname_next`, store.rs:188) exists only in `ScenarioSuspensions`, and neither the runtime nor the file store depends on that crate.
3. **The approval path is unchanged.** `security_pass2_an_approval_hold_is_taken_as_before_and_never_by_a_decision ... ok`: an approval hold is taken and released by its action request id, and a take or release that also names a decision request is refused `OtherDecision` with nothing changed, in both stores. In existing tests every changed line since 794fa6c2 adds `decision_request_id: None` or a trailing comma. In `decision_suspend_resume.rs` only import lines were removed (re-wrapped), plus 6 new cases.
4. **The stored file.** Unchanged: `crates/loom-governor` has no diff since 794fa6c2, so `MEMBERS` and the encoder are the same. My pass-1 case that decodes a file in the 39283f1b format is still green, and a file written at 794fa6c2 uses the same encoder.
5. **F2.** Closed. `security_pass2_the_memory_store_holds_no_suspension_whose_reason_and_members_disagree ... ok`: all 30 disagreeing combinations of reason and the four members answer `storage-failed` and store nothing, and the two agreeing ones are held. A Decision hold with `arguments: Some(Null)`, which is what the conformance codec produces from `arguments: null`, is refused too, and the conformance store holds through this same check. No existing assertion changed.
6. **ESS and generated files.** `git show --stat 0c716b3a` shows only `ess/commission/domains/responsibility.yaml`. I ran these myself (logs in `sec2/`), all exit 0: `commission:drift` ("no drift"), `commission:docs-drift` with the 0.86.0 PATH ("3 reference pages current"), `docs-check`, and `commission:conform` ("1 passed"). The conformance suite still covers the mismatch outcome: the two new `other-decision` scenarios are not in `ess/SKIPPED.md`, and `conform.rs` fails on any failed or unlisted unsupported scenario, so they ran and passed.
7. **My own pass-1 files.** I rebuilt my originals from my own writes into `sec2/orig-*.rs`. The approve file is identical apart from whitespace. The testkit file differs only by `cargo fmt` line breaks, one fmt-added trailing comma, and the 4 listed `decision_request_id` lines (now at 292, 300, 665, 674). Lines 292 and 300 correctly make the simulated DR-1 resume name DR-1. No assertion of mine changed.

**Finding**
- Note, introduced (runtime.rs:2886). Sequence: a stale resume reads its DR-1 hold as Resuming; before its release, another resume consumes DR-1 and the run is held again for DR-2, Pending. The stale resume's release then answers `not-taken`, and it ends in a loop error (`SuspensionNotReleased`) rather than the refusal `OtherDecision`. Nothing changes and nothing continues; covered by the stale-resume test above. This comes from the outcome order `ess/22` forced on the release (`not-taken` is checked before `other-decision`). A host will see an error where a refusal would describe it better.

**Could not establish:** the CLI, agent and intake suites are not rerun on 61e38aad; I rely on the implementor's gate exits, all 0. Disk at the end: 90G free on `/`, `target/` at 43G.

`git status --short`:
```
?? crates/loom-commission-testkit/tests/security_w20261121w3_u2_pass2.rs
```

```findings
- file: crates/loom-commission/src/runtime.rs
  line: 2886
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: a stale resume that read its decision hold Resuming and meets a later Pending hold for another decision ends in a SuspensionNotReleased loop error instead of an OtherDecision refusal, with nothing changed
```
