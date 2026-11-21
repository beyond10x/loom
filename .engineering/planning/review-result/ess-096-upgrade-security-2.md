---
format: aep.planning-md/3
id: review-result:ess-096-upgrade-security-2
kind: review-result
status: active
title: Security review, pass 2, of unit U1 of wave 2026-11-21-w5 (story:ess-096-upgrade), diff 12469d71..590b01f8
relations:
- reviews: story:ess-096-upgrade
revision: 1
---
unit: story:ess-096-upgrade pass 2, `git diff 12469d71..590b01f8` at head `590b01f8`
verdict: green
cases: executed 29 (4 testkit binaries, 27, plus 1 intake-slice binary, 2), red 0; I added no case this pass
origin: introduced 0, pre-existing 0, undecided 0
wrote-outside-worktree: 6 paths in sec1/ (listed at the end)
needs-coordinator: no

**1. Holds.**
- `incomplete_request` is a pure function: it builds the answer from `input.suspension` only (`suspension.rs:154-168`).
- `FileSuspensionStore::hold` asks it first (`file_suspension.rs:216`), before the identity check, `self.command` (the lock) and `self.path` (where a file name is formed from the id).
- So a mixed hold now takes no lock and reads no file. Before, a mixed hold with a matching identity took the lock and read the file. A commission id that is no valid file name reaches no path and no error text: the answer names only the key, as the in-memory store's answer always did.
- A hold that is not mixed runs the same lines as at `12469d71`.
- Test: `a_mixed_hold_naming_another_commission_is_incomplete_on_both_stores` passes (`hold_incomplete_request`: `ok. 10 passed`).

**2. Holds.**
- The extraction changed no branch. The mixed test is the same expression. `members[0]` became `input.action_request_id.is_some()`, which is the same value once a mixed hold has been refused.
- Test: `security_w5_u1_both_stores_answer_and_keep_the_same_for_every_reason_and_member_set ... ok`, run unchanged (`ok. 5 passed`).

**3. Holds.**
- The base code matches the implementor's reading:
  - The base in-memory `hold` (`git show 29e5d199:…/suspension.rs:384-389`) has no identity check and stores under `input.suspension.commission_id`.
  - The base file store refuses a misnamed hold first (`file_suspension.rs:209-216` at base).
- At base, the in-memory store held a misnamed `Authority` hold with all four members and a misnamed `Decision` hold with none, and refused `Authority` with none as `StorageFailed`.
- Head differs only in that `Authority` hold with none, which is now held, by the decision of `correction-0.md`. The named case of pass 1 covers it, and the `Store` error in `resume_after_grant` comes before its `OtherCommission` check.
- Test: `a_misnamed_hold_with_all_four_or_none_is_held_in_memory_and_refused_by_the_file_store ... ok`.

**4. Holds.**
- F2: `runtime.rs:985-986` and the help text at `loom-cli/src/lib.rs:130-131` match the let-else at `runtime.rs:2516-2530`. `cli.md:637-641` carries the same text.
- F1: every take or release outside test code names a request or a decision:
  - the runtime at `runtime.rs:2328`, `:2643` and `:2845`;
  - the slice's only calls (`grant.rs:716,722`) are inside `#[cfg(test)]`.
- The conformance target passes on whatever its scenario sends, including neither (`loom-commission-conformance/src/lib.rs:614-619`, `:660-665`).
- One caveat: code outside the workspace can also name neither through the public `SuspensionPort`. The prose is about callers inside the workspace. I am not raising it as a finding.

**5. Holds.**
- `7ffc43fa` differs from `sec1-as-delivered/` only by formatting:
  - the testkit file is identical once white space is removed;
  - the intake-slice file differs only in that, plus rustfmt moving the `use loom_governor::FileSuspensionStore;` line down among the imports.
- `hold_incomplete_request.rs` only gained lines; neither commit deletes a test line.
- No `#[ignore]` in the diff. `ess/`, `generated/` and `Cargo.lock` are untouched by both commits.
- The wave 3 files pass: `ok. 8 passed` and `ok. 4 passed`. The intake-slice file passes: `ok. 2 passed`.

findings: []

**Not established:**
- I did not run the `.any` mutant myself. The implementor's `c1-mutant.log` shows my hand-written-file case failing on it.
- I did not rerun the docs-drift task for `cli.md`.

`git status --short`: empty. `target` is 27G; `/` has 130G free.

Paths written in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w5/u1/sec1/`: `run-pass2.sh`, `pass2-testkit.log`, `pass2-testkit.exit`, `pass2-slice.log`, `pass2-slice.exit`, `pass2-disk.txt`.
