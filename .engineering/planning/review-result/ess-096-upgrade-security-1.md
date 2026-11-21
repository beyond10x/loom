---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-security-1
kind: review-result
status: active
title: Security review, pass 1, of unit U1 of wave 2026-11-21-w5 (story:ess-096-upgrade) at 12469d71
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
unit: story:ess-096-upgrade, head `12469d71` plus 2 untracked security test files
verdict: green
cases: executed 539→544 (commission lane) and 555→557 (governor + intake-slice lane), red 0
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: 10 paths in sec1/ (listed under Paths at the end)
needs-coordinator: no

**Diff:** `git --no-pager diff --stat` prints nothing. `git status --short` shows two new files, both tests:
- `crates/loom-commission-testkit/tests/security_w20261121w5_u1.rs`
- `crates/loom-intake-slice/tests/security_w20261121w5_u1.rs`

**Named case** (a held Suspension with a non-`Decision` reason and no request member)
- **Who can send it:**
  - No caller outside tests. The runtime sends a hold with no request only for `Decision` (`runtime.rs:1978-1981`), and `suspend_for_approval` always passes the request (`:2403`).
  - The intake slice builds `HoldSuspension` only in `#[cfg(test)]` (`grant.rs:672,777`, `handoff.rs:655`). The conformance adapter (`loom-commission-conformance/src/lib.rs:559`) is test-gate only.
  - In-process SDK code can send it through the public `SuspensionPort::hold`.
  - From outside the process, only a hand-written file in the state directory can make one (`file_suspension.rs:543`). Such a file was refused before; it now reads.
- **What the sender gains:**
  - `approve` answers `OtherCapability{held:None}` and writes no grant, `Pending` or `Resuming`. `GrantAuthority` answers `ApprovalRequired` even given a grant for that capability. The same `approve` on a plain hold writes the grant and the provider then allows.
  - `run_with_suspension` answers `CommissionSuspended{Pending, None}`, as for a plain hold.
  - `resume_after_grant` answers `Store` before the governor is asked; `resume_after_decision` answers `HeldForAuthority`. Both were checked `Pending` and `Resuming` (after a take naming neither), for all 10 reasons, on both stores; nothing is invoked.
  - A take and a release naming neither clear it. A plain hold reaches the same empty store when the caller names the request id that `get` hands out (the take naming neither answers `OtherRequest` first).
  - `already-held` behaves as for any hold. A plain hold with a stale revision stays stored just the same (`CaseMoved` keeps it).
  - A hand-written file makes the commission blocked as `CommissionSuspended` where before it was blocked by a store error. Someone with write access to the state directory can already write a full plain `Authority` file naming any action, which is strictly more.
  - No store becomes unreadable: every other split of the members is still refused, and the file stays byte for byte.
- **Sentence for sdc:** "nothing a plain `Authority` hold does not already give".
- **Tests:** testkit `security_w5_u1_a_hold_without_a_request_moves_no_run_on_both_stores` and `security_w5_u1_a_plain_hold_is_cleared_with_the_id_get_hands_out_on_both_stores`; intake-slice `security_w5_u1_approve_writes_no_grant_for_a_hold_without_a_request` and `security_w5_u1_approve_writes_the_grant_for_a_plain_hold`. All `... ok`.

**Invariants**
1. **Holds.** `security_w5_u1_a_hold_after_a_held_one_leaves_its_file_byte_for_byte` covers 3 kinds of first hold × 11 reasons × masks 0–14, compares file bytes, and checks a single file remains: ok. The implementor compared through `get` only.
2. **Holds.** `security_w5_u1_both_stores_answer_and_keep_the_same_for_every_reason_and_member_set`: ok.
3. **Holds.** Same matrix test: 11 reasons × 16 masks, equal outcomes and equal stored Suspensions, no file written on a refusal.
   - Added `security_w5_u1_a_hand_written_file_reads_as_no_request_only_when_all_four_are_null`: ok.
   - That test kills a mutant that changes `.all` to `.any` at `file_suspension.rs:548`. By reading, no existing case does: every refused non-`Decision` file has `arguments: {}`, which the invariant check refuses anyway. I did not run that mutant.
   - Base-format files: the encoder is unchanged, and `security_w20261121w3_u2.rs:561` still passes.
4. **Holds.** `other_request` and `other_decision` are unchanged (`suspension.rs:233-261`). The wave 3 tests pass.
5. **Holds.** Synthesize: `44 scenario(s) (0 authored), 0 refusal(s)`. The conform test passes inside lane A. `conform.rs` is unchanged and `SKIPPED.md` lists 0 entries. Both gate mutants keep the assertion `1 refusal(s)` + `ESS-SYNTH-011` + `exit status: 0` (`ess_gate.rs:395-400`), and that assertion is unchanged.
6. **Holds.** No code read the `case` relation (the generated code only lost a doc line). `case_id` is still stored (`suspension.rs:213`) and compared (`runtime.rs:2537,2794`).
7. **Holds.** Every changed line matches the implementor's list. No case deleted, no `#[ignore]`. Only the no-request case moved from refused to held, and the mixed cases went from `StorageFailed` to `IncompleteRequest`.
8. **Holds.**
   - `ESS_SHA256` is `8eaf3398…1825` in both workflows: the story's tarball digest. The workflows pin the archive (`grep -Fx … SHA256SUMS`).
   - `Cargo.lock` moves only the 8 ess packages, to `9e91e8b5`, which equals `0.96.0^{commit}`.
   - I did not rerun the drift tasks: the implementor reports them exit 0.

**Findings** (all INFEASIBLE notes; no blocker)
- **F1** `crates/loom-commission/src/ports/suspension.rs:240`: a take or release naming neither cannot tell two later no-request holds of the same commission apart (None==None in both checks). A plain hold is told apart by its request id, a `Decision` hold by its decision id. Nothing found that reaches it: no caller takes naming neither. Introduced.
- **F2** `crates/loom-commission/src/runtime.rs:2527`: `resume_after_grant` reports a hold that `decide_hold` now admits as `ResumeError::Store`, documented as "the suspension port could not be read" (`:985`), and the CLI exits 20 (`loom-cli/src/lib.rs:163`). Reached only by an SDK hold or a hand-written file. Introduced: the state was refused before.
- **F3** `crates/loom-governor/src/file_suspension.rs:214`: for a mixed hold whose `suspension.commission_id` differs from `commission_id`, the file store answers `StorageFailed` (identity check first) and the memory store `IncompleteRequest`. Both store nothing. At the base both answered `StorageFailed`. Nothing found that reaches it: the runtime builds both from one id (`runtime.rs:2427-2429`). Introduced: only the outcome kind now differs.

**Not established:** the `.any` mutant was not run. Drift tasks and `ess validate` were not rerun.

**Suite runs** (no `ESS_TOOLCHAIN`, `CARGO_INCREMENTAL=0`, logs in sec1/):
- Cases alone first: testkit `test result: ok. 5 passed`, intake-slice `ok. 2 passed`, both exit 0.
- `cargo test -p b10x-loom-commission -p b10x-loom-commission-conformance -p b10x-loom-commission-testkit --locked`: exit 0, 544 passed, 0 failed.
- `cargo test -p b10x-loom-governor -p b10x-loom-intake-slice --locked`: exit 0, 557 passed, 4 ignored.
- `target` is 24G, `/` has 139G free.

**Paths** written outside the worktree, all in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w5/u1/sec1/`: `checks.sh`, `checks.out`, `synth.log`, `suite.json`, `case-testkit.log`, `case-slice.log`, `run-suite.sh`, `suite-a.log`, `suite-b.log`, `suite-disk.txt`. Also `suite-a.exit` and `suite-b.exit`, written by the run script. The tests also write under `target/tmp`.

```findings
- file: crates/loom-commission/src/ports/suspension.rs
  line: 240
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "A take or release naming neither request cannot tell two successive no-request holds of one commission apart, unlike plain and Decision holds; no caller sends it."
- file: crates/loom-commission/src/runtime.rs
  line: 2527
  category: contract-drift
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "resume_after_grant reports an admitted no-request hold as ResumeError::Store, documented as an unreadable port, and the CLI exits 20 for it."
- file: crates/loom-governor/src/file_suspension.rs
  line: 214
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "A mixed hold with a mismatched identity answers StorageFailed on the file store and IncompleteRequest on the memory store; both store nothing and the runtime never builds one."
```
