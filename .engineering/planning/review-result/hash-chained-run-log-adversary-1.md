---
format: aep.planning-md/3
id: review-result:hash-chained-run-log-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:hash-chained-run-log (wave 2026-11-21-w3, unit U1, head 85133ac4)
relations:
- reviews: story:hash-chained-run-log
revision: 1
---
unit: U1 `story:hash-chained-run-log`, worktree loom-w1121c-u1 at head 85133ac4 plus my one untracked test file
verdict: red
cases: executed 13→25, red 1
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w3/u1/adv1` (the one assigned)
needs-coordinator: decide whether "exit or refusal, one of the two" is a rule the code must enforce (finding 1)

1. `git diff --stat` is empty. `git status --short` shows only `?? crates/loom-runlog/tests/adversary_w20261121w3_u1.rs`, a test path. I edited no implementation file, existing test, manifest or the lock. I also made a temporary harness, `tests/adversary_w20261121w3_u1_mutants.rs`, used it, and deleted it; a copy is `harness.rs.keep` in scratch.

2. **Cases added** (`crates/loom-runlog/tests/adversary_w20261121w3_u1.rs`, 12 cases):
- Red: `an_action_finished_holds_an_exit_or_a_refusal_never_both_and_never_neither`. First run of that case alone (exit 101): `both: an ActionFinished with both of exit and refusal was appended: Ok(RunLogLine { seq: 3, ... exit: Some(1), refusal: Some("denied") ... })`
- Green, 11 cases:
  - the contract `run-log-v1.json` read as the specification: all 10 kinds read, write back as exactly the stated fields, a missing required field, a wrong type, an extra member or an unknown kind is refused;
  - a line before the last that does not read, while the next line names its digest, is refused;
  - an `at` that is no RFC 3339 instant is refused and writes nothing;
  - a symlink, a directory or a FIFO at the log's name is refused and not followed;
  - a member written twice (also through a `\u` escape), or a member of another kind, on the last line is refused;
  - a blank line, trailing bytes, CR LF, a byte-order mark, a non-UTF-8 byte, a lone LF, and a `seq` that is a string, negative or past 2^53 are each refused at the expected line;
  - a pin: a change to the last line alone verifies;
  - case ids `..`, `/etc/passwd`, an id with a NUL, and ids at the 100/101-byte boundary all stay inside the store, and no two ids share a log;
  - the log file has the same mode as the case file;
  - 8 threads × 25 appends leave every line exactly once;
  - a 4 MiB line is written and read back whole.

3. **Suite run.** `cargo test -p b10x-loom-runlog --locked --no-fail-fast` exits 101. `run_log`: `13 passed; 0 failed`. Adversary file: `11 passed; 1 failed`. Lib and doc tests: 0. "Before" is the implementor's `green1.log`: 13.

4. **Findings**
- F1, warning, CONFIRMED, introduced. `ess/domains/runlog.yaml:91` says ActionFinished holds "its exit status or why it was refused (one of the two)", and the story says "exit or refusal". The code takes both, or neither, on append and on verify (`crates/loom-runlog/src/lib.rs:466-472` and `:616-622`). What reaches it: nothing calls it yet; `RunLog::append` is public and `story:tree-run-command` will call it. Two possible fixes: refuse the combination in `decode` (the append's read-back guard then refuses it too), or drop the words "one of the two" from the domain.
- F2, warning, CONFIRMED, introduced. Nothing after the last line holds its digest, so any change to that line alone verifies, and the next append makes the change permanent (pin test above). These texts claim more without saying so:
  - `README.md:731` and `website/data/status.json:24`: "refuses ... a changed byte";
  - `AGENTS.md:1182`: "a changed byte is named by the line after it";
  - the story's Outcome.
  `lib.rs:32-33` covers it only as "rewritten from some line on". Fix: one sentence in each text saying a change to the last line alone is not detected.
- F3, warning, CONFIRMED, introduced. Mutation run on in-memory copies of `lib.rs`, 11 modules, both suites. Baseline failures are subtracted: their two-process test cannot run in the harness, and my F1 case is red anyway. These mutants survive the unit's 13 tests and are killed by my cases:
  - m01: an unreadable line named by the next line is accepted (`lib.rs:406`);
  - m02: the read-back guard is removed (`:257`);
  - m03: the duplicate-member check is removed (`:566`);
  - m04: `closed_at` is removed (`:581`);
  - m05: the regular-file check is removed (`:306`).
  The unit's own tests kill m06 (a `prev` on line 0), m07 (a line feed inside the digest input) and m08 (an append ignoring a refusal).
- F4, note, CONFIRMED, introduced. Two mutants survive both suites:
  - m09: the temporary file is kept on a write failure (`lib.rs:276`);
  - m10: the branch for a second consecutive unreadable line (`:375`). The log is still refused, only the line number changes.
  m09 needs an injected write failure, which I did not build.
- F5, note, INFEASIBLE (no caller), introduced. `LockedDir::temporary` is now public, but its "Only called holding the lock" (`locked_dir.rs:140`) is not enforced. `locked` cannot be re-entered (flock): calling `RunLog::append` inside `LockedDir::locked` on the same directory hangs. Only runlog uses either today.

5. **Checked and sound**
- Writer and verifier hash the same bytes: the raw line without its LF.
- No middle-line change, duplicated line, reordered pair, extra member or non-canonical spelling verifies.
- Two processes, and 8 threads, never write the same `seq` twice.
- The crash test covers every step; `open` removes only `.tmp-` files, while holding the lock.
- No governor test changed. Commit 81a81886 touches only `ess/`.
- The 14 reference pages changed only their model digest.
- `task drift`: "no drift from ess/" (exit 0).
- `system.yaml` is still `ess/25`. No `/home/` literal in the diff; `.engineering/` untouched; commission unchanged.
- The crate reaches Canon, the executor and `b10x-llm-core` only through the governor, as the story requires.
- `docs-check` compares the whole file exactly (`loom-docs/src/main.rs:531-553`): read, not run.

6. **Paths outside the worktree** (all under the assigned `adv1`): `adv-*.log/.exit`, `mut.log/.exit`, `suite.log/.exit`, `drift.log/.exit`, `tree*.log`, `make_mutants.py`, `action-finished-case.rs`, `harness.rs.keep`. Test scratch is under `<worktree>/target/tmp/adversary-w20261121w3-u1` and `mut-*`. `target` is now 7.8G; 95G free on `/`.

```findings
- file: crates/loom-runlog/src/lib.rs
  line: 616
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: ActionFinished with both exit and refusal, or neither, is appended and verifies although the domain says one of the two
- file: README.md
  line: 731
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: README, status.json and AGENTS.md:1182 claim any changed byte is refused, but a change confined to the last line verifies
- file: crates/loom-runlog/src/lib.rs
  line: 406
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: five refusal guards (lib.rs 257, 306, 406, 566, 581) can be removed with the unit's suite green; the adversary file now kills each
- file: crates/loom-runlog/src/lib.rs
  line: 276
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: removing the temporary-file cleanup on a write failure (276) and the second-unreadable branch (375) survives every suite
- file: crates/loom-governor/src/locked_dir.rs
  line: 140
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: public temporary() states an unenforced lock precondition and public locked() deadlocks if RunLog::append is nested inside it
```
