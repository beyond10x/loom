---
format: aep.planning-md/3
id: review-result:adversary-w20261028w1-observer-pass-1
kind: review-result
status: active
title: Wave 2026-10-28-w1 adversary, loom story:session-executor-event-observer, pass 1
relations:
- reviews: story:session-executor-event-observer
revision: 1
---
Adversary pass 1 on the event observer (`aep:adversary`, opus), wave 2026-10-28-w1, tree
`loomd163-a3` at `6721d4b11` on integration head `4b0b50fb8`. The report as it returned:

unit: story:session-executor-event-observer (covers wave head 4b0b50fb8 plus my commit 6721d4b11 on adv/d163-a3)
verdict: red
cases: executed 136→141, red 2
origin: introduced 2, pre-existing 0, undecided 0
wrote-outside-worktree: 3 logs in ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-d163/a3/, plus build output in the wave's shared target
needs-coordinator: no

**1. Diff stat**: one file, `crates/loom-session-executor/tests/adversary_w20261028w1_a3.rs`, 362 lines added. Commit `6721d4b11`, author and committer `b10x-bot[bot]`.

**2. Cases**
- **Red:** `adversary_an_observer_writing_into_the_copy_changes_no_proposal`:
  `assertion left == right failed: an observer changes no proposal: a session proposes what it would without one; left: ["src/planted.rs"], right: []`
- **Red:** `adversary_a_session_started_from_inside_the_observer_has_its_child_killed`:
  `panicked at crates/loom-session-executor/src/lib.rs:1267:42: RefCell already borrowed` then `the nested session's child is killed (the call left with Some("RefCell already borrowed"))`
- **Green:** `adversary_a_panicking_observer_records_no_session_and_keeps_the_last_report` (no session recorded, nothing proposed or failed, `last_report` unchanged, the halt line reaches the child, the child is killed, the pending call is denied; first run red on the adversary's own wrong assertion, corrected).
- **Green:** `adversary_the_observer_sees_a_tool_call_before_the_executor_decides_it` (would catch moving the observer call after the `match`; reasoned, not measured on a mutant).
- **Green:** `adversary_an_observer_reads_the_executor_while_the_session_runs`.

**3. Suite**: `cargo test -p b10x-loom-session-executor --locked --no-fail-fast --lib --test <each of 21 files>`: EXIT=101, 141 cases, the only failures the 2 red cases; without the file 136, all passing, `event_observer` 4/4.

**4. Findings**
- A. The observer can change the proposal: `with_observer` is documented as reaching "neither the report nor the session, so the call proposes and reports what it would without one" (`src/lib.rs:847-850`), but it receives the live copy's path (`src/lib.rs:1293`) and any file it writes there is proposed back. What reaches it: nothing found; no consumer calls `with_observer` yet. INFEASIBLE, warning, introduced. Fix: the doc ("must not write to the copy") or not handing over a writable path.
- B. A nested session's child is never killed: `self.observer.borrow_mut()` stays held for the whole loop (`src/lib.rs:1267`); an observer that runs the executor again starts a nested child, then panics on that borrow; the nested child is never halted. What reaches it: nothing found. INFEASIBLE, note, introduced. Fix: refuse a nested call before the copy, or take the observer out of its `RefCell` for the loop.

**5. Attacked and not broken:** observed events against the transcript (order, count, with a vendor record kept); every event observed before its decision is written; the path handed over is the report's `cwd`; the panic path; reading the executor from inside the observer; an unserializable event (none found, not tested).

```findings
- file: crates/loom-session-executor/src/lib.rs
  line: 1293
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: the observer is handed the live copy's path and a file it writes there is proposed back, contradicting with_observer's doc (lib.rs:847-850) and the story's "cannot change the proposal"; no consumer does this yet
- file: crates/loom-session-executor/src/lib.rs
  line: 1267
  category: concurrency
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the observer RefCell is held across the poll loop, so an observer that runs the executor again panics with "RefCell already borrowed" after the nested child started, and that nested child is never halted
```
