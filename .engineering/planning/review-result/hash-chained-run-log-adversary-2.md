---
format: aep.planning-md/3
id: review-result:hash-chained-run-log-adversary-2
kind: review-result
status: active
title: Adversary pass 2 (last) on story:hash-chained-run-log (wave 2026-11-21-w3, unit U1, head 600b9fdd)
relations:
- reviews: story:hash-chained-run-log
revision: 1
---
unit: U1 `story:hash-chained-run-log`, the correction 85133ac4..600b9fdd, plus my amended file and one new file
verdict: nothing found
cases: executed 27→30, red 0
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w3/u1/adv2` (the one assigned)
needs-coordinator: none

The correction holds against everything I tried; the one note below is a doubled phrase in `status.json`.

`git diff --stat`: `crates/loom-runlog/tests/adversary_w20261121w3_u1.rs | 114 ++++++++++++++-------`, a test file and the only tracked file I changed. All its hunks fall inside `every_kind_of_the_contract_reads_and_writes_exactly_the_fields_it_states` and its doc comment. Before amending, the tree was clean, so the file committed in 42fbd544 was my pass-1 file as I left it.

**Amended case.** For `ActionFinished` it now reads two lines:
- every field plus `exit`, no `refusal`;
- every field plus `refusal`, no `exit`.
Each must read and write back as exactly the stated fields. Leaving out the only one of the pair is refused `CutLine`. The line with both and the line with neither are each refused `CutLine 0`. The other nine kinds are checked as before.
Green: `test every_kind_of_the_contract_reads_and_writes_exactly_the_fields_it_states ... ok`.

**New cases** (`crates/loom-runlog/tests/adversary_w20261121w3_u1_pass2.rs`, all green when first run, `3 passed`):
- `every_shape_without_exactly_one_of_exit_and_refusal_is_refused`. These are refused `CutLine 0`:
  - neither, or both;
  - `exit:null` with a refusal, `exit:null` alone, `refusal:null` alone, `exit:0` with `refusal:null`;
  - `exit` twice, `refusal` twice;
  - `exit` as a string, a boolean or 1.5, and `refusal` as a number;
  - a second `refusal` spelt `\u0072efusal`;
  - a `refusal` on `ActionStarted`, an `exit` on `Terminated`.
  These are read: `exit` 0, -1 or 3.0 alone, and a refusal alone.
- `a_middle_action_finished_with_both_is_named_itself`: when the next line names its digest correctly, the line is refused `BrokenChain` at its own index (tried at line 0 and line 2).
- `an_append_of_both_or_neither_writes_nothing_and_leaves_no_temporary`: on an absent log, no file is created; on a 3-line log, the bytes are identical; in both cases no `.tmp-` file is left. The log then still takes an append that has exactly one.

**Suite.** `cargo test -p b10x-loom-runlog --locked --no-fail-fast` exits 0:
- `run_log`: `15 passed`
- `adversary_w20261121w3_u1`: `12 passed`
- `adversary_w20261121w3_u1_pass2`: `3 passed`

"Before" is the 15 + 12 cases at 600b9fdd.

**Mutants re-run.** These ran on copies of `lib.rs` in a temporary harness, which I deleted; a copy is `adv2/harness2.rs.keep`. Before counting, I subtracted the two-process test, which fails in the harness with no mutation.
- m09 (keep the temporary file on a failed write) is now killed: `theirs::a_rename_that_fails_removes_its_temporary_file_and_changes_no_byte ... FAILED`.
- m10 (drop the branch for a second unreadable line) is now killed: `theirs::two_unreadable_lines_in_a_row_are_named_by_the_second ... FAILED`.
- m11 (drop the new exactly-one guard) is killed by my two cases only. `run_log.rs` has no case for the rule, so it rests on the adversary files the implementor commits.

**Finding**
- Note, CONFIRMED, introduced: `website/data/status.json:24` now says "the log is not signed" twice. The new sentence ends with it and the old one is still there. A one-word fix. No test, since it is prose.

**Checked and sound**
- The rule is in `decode`, after the duplicate-member and closed-member checks, so `verify`, the read-back guard and `read` all enforce it. Running the guard on a copy of a line naming no exit and no refusal confirms that.
- `run_log.rs` lost no line: the correction only adds the 2 new cases. My pass-1 file went in unchanged.
- All four texts (`README.md:731`, `status.json:24`, `AGENTS.md:1182-1187`, `lib.rs:32-38`) now say a change to the last line alone is not detected. None of them weakened another claim.
- `ess/domains/runlog.yaml:91` still says "one of the two". The generated contract carries no descriptions, so it says nothing either way. `ess/` is unchanged.
- The `LockedDir` change is doc comments only, and they match what pass 1 measured.

A `refusal` of `""` counts as present and is accepted; this matches the rule as worded. Raised as an observation, not a finding.

`git status --short`:
```
 M crates/loom-runlog/tests/adversary_w20261121w3_u1.rs
?? crates/loom-runlog/tests/adversary_w20261121w3_u1_pass2.rs
```
Disk at the end: `target` is 8.2G, `/` has 86G free.

Files in `adv2`: `mine.log/.exit`, `mut2.log/.exit`, `suite.log/.exit`, `make_mutants2.py`, `harness2.rs.keep`. Test scratch is under `<worktree>/target/tmp/adversary-w20261121w3-u1-pass2` and `mut-*`.

```findings
- file: website/data/status.json
  line: 24
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the corrected run-log entry states "the log is not signed" twice
```
