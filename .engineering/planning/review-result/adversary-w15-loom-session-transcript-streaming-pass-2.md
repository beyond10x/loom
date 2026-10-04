---
format: aep.planning-md/3
id: review-result:adversary-w15-loom-session-transcript-streaming-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w15 adversary, loom story:session-transcript-streaming, pass 2
relations:
- reviews: story:session-transcript-streaming
revision: 1
---
```
unit: loom/session-transcript-streaming — the worktree at 21f0f0d plus the uncommitted phase 2 and pass-1 fixes
verdict: NEEDS-CHANGE
cases: executed 551→559, red 5
origin: introduced 10 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 locations (part 6)
needs-coordinator: yes. Findings 1, 3 and 4 each need a design choice: how to recover a session left Active by a dead run, where `session-exists` is refused, and what is filed on AwaitingApproval.
```

Pass 2 found 5 new failing cases, all in `crates/loom/src/session.rs`. The most serious: if a resumed run dies without unwinding (a kill, an out-of-memory kill, or Ctrl-C), its session stays Active and no API call can resume it again. The lock itself holds: no double claim in 16 × 400 rounds.

**1. Diff stat**
- Tracked diff unchanged from the start: `14 files changed, 1717 insertions(+), 33 deletions(-)`.
- My only change is one new untracked test file: `crates/loom/tests/adversary2_session_transcript.rs` (522 lines). No non-test path touched.

**2. Cases added.** Each was run on its own before the suite. Log: `scratch/adversary-p2/cases-alone.log`.

| Test (line) | Now | Red output, verbatim excerpt |
|---|---|---|
| `a_session_whose_resuming_run_died_can_be_resumed_again` :40 | red | `…never resumable again, and nothing in the API can release it: the session is Active, not Filed: another run holds it…` |
| `filing_a_session_already_filed_is_the_specified_wrong_state` :70 | red | `filed twice: FileSession on a Filed session answered Err(Refused("session … is not held by this run…")), not SessionStateConflict { state: Filed }` |
| `opening_a_filed_identity_is_refused_before_anything_is_sent` :100 | red | `OpenSession session-exists came after the run: the model was asked 1 time(s) and the run's answer (Ok("ANSWER")) has nowhere to be filed` |
| `a_run_suspended_for_approval_is_filed_as_a_replayable_conversation` :142 | red | `…holds call(s) [CallId("call-1")] with no result; resuming it replays a function_call without its output: [UserText { text: "QUESTION" }, ToolCall(…fs.write…)]` |
| `a_run_whose_loop_panics_files_what_it_already_spent` :204 | red | `the session filed after a panic reports the run as free: turns 0, usage [], while the sink saw [Usage { … input_tokens: 10, output_tokens: 5 … }]` |
| `the_panic_run_and_file_carries_on_is_the_loops_own_payload` :242 | green | — |
| `concurrent_resumers_claim_a_filed_session_exactly_once` :271 | green | — |
| `a_lock_left_by_a_crashed_resume_is_refused_by_name_and_recoverable` :312 | green | — |

Each green case is shown able to fail by a mutant, run in a scratch copy with its own target (both deleted afterwards):

| Mutant | Effect |
|---|---|
| m-lock: `Claim::take` removed (`:383`) | Nothing in the existing suite kills it: implementor 3/3, pass-1 12/12, `--lib session` 1/1 all pass. My concurrent case went red with `round 213: 2 resumers claimed one session`; my lock case went red at `:293`. |
| m-payload: `resume_unwind(Box::new("m-payload"))` (`:609`) | My payload case went red: `left: None right: Some(Marker(42))`. |

The concurrent case takes about 30–45 s in the gate.

**3. Gate.** Worktree, unit build dir, foreground. I added `--no-fail-fast`. Log: `scratch/adversary-p2/gate.log` and `gate.log.test`.

| Command | Exit | Summary |
|---|---|---|
| `cargo fmt --check` | 1 | My file only. After `rustfmt --edition 2024` on it: 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | |
| `cargo test --workspace --locked --no-fail-fast` | 101 | 34 lanes, 554 passed, 5 failed, 2 ignored |
| `cargo test -p b10x-loom --test ess_gate --locked` | 0 | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s` |
| `ess specify validate … && ess verify conform synthesize …` | 0 | `loom v1 — 2 file(s), valid` / `32 scenario(s) (0 authored), 0 refusal(s), written to …/suite.json` |
| `loom-xtask -- drift` | 0 | `…/generated/rust/loom: no drift from ess/` |
| `loom-xtask -- no-hand-model` | 0 | `…/crates/loom/src: no hand-written model type (98 reserved type names checked)` |
| `loom-docs -- generate --check` | 0 | `website/data/ess: 1 generated files current` / `website/docs/reference/ess: 2 generated files current` / `website/docs: no raw admonition titles` |
| `RUSTDOCFLAGS=-Dwarnings cargo doc -q -p b10x-loom --no-deps --locked` | 0 | |

Session-related lanes, verbatim:
```
tests/adversary2_session_transcript.rs  test result: FAILED. 3 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.76s
tests/adversary_session_transcript.rs   test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
tests/session_transcript_streaming.rs   test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
unittests (b10x-loom)                   test result: ok. 397 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
```
`--list` on my file in this tree prints the 8 names above and `8 tests, 0 benchmarks`. The count checks out: 551 + 8 = 559.

**4. Findings.** All cover the worktree at 21f0f0d plus uncommitted work.

| # | file:line | Verdict / origin | What was measured | What reaches it |
|---|---|---|---|---|
| 1 | `session.rs:392` | NEEDS-CHANGE / introduced | A claimed session dropped without filing is refused `wrong-state` for ever. `load` holds nothing, `file` refuses a value it does not hold, and `open` hits `Exists`, so there is no way out through the API. | Any kill during a resumed run. `run_and_file` gives the caller no cancel handle, so Ctrl-C is a kill. No production caller yet. |
| 2 | `session.rs:307` | NEEDS-CHANGE / introduced | `FileSession` on a Filed or read-only value returns `Refused(String)`, not the spec's `wrong-state` / `SessionStateConflict{Filed}`. | Any second filing of the same value. |
| 3 | `session.rs:217` (refusal at `:465`) | NEEDS-CHANGE / introduced | `OpenSession session-exists` is refused only when the session is first filed. By then the model has run and been billed, and its answer cannot be filed. | Reopening an id that is already filed, e.g. a retried commission run that reuses its session id. |
| 4 | `session.rs:615` | NEEDS-CHANGE / introduced | `AwaitingApproval` is filed `Stopped` with a call that has no result after it. Replaying a call without its output is an error at the provider. The approval checkpoint is not kept in the session. This contradicts the `loom.run.Turn` comment that "a turn that did not complete is never recorded". | Any `ApprovalPort` that returns `Deferred`. None in the workspace yet. |
| 5 | `session.rs:606` | NEEDS-CHANGE / introduced | After a panic, the filed session shows 0 turns and no usage, although the sink saw turn 1 billed. `RunLedger`'s own docs call that "a failed run as free". The spend can be fixed within this unit: `run_and_file` holds the sink, so it can tally `LoopEvent::Usage` and `Cost`. The items of this run's completed turns cannot be recovered without changing `turn_loop` (`run_in` takes them with `mem::take`). | A panic in a caller's port, tool or sink. No caller catches panics today. |
| 6 | `session.rs:540` | CONFIRMED / introduced | `RunPorts` cannot carry hooks, cancel or environment. That means no operator hooks before a call, no `Cancelled` stop through `run_and_file`, and no narrowing of the available tools per turn. The module docs, `status.mdx:29` and the story do not mention this. It broke no caller: the only workspace change is `agent_executor.rs` +2 lines, adding `wire`. | Every future caller of `run_and_file`. |
| 7 | `session.rs:465` | INFEASIBLE / introduced | The first filing needs `hard_link`. On filesystems without hard links (vfat/exFAT, some FUSE or SMB mounts) every first filing would be refused. This is undocumented. | Not tested: no writable filesystem of that kind here. |
| 8 | `session.rs:761` | CONFIRMED / introduced | A crash between writing the temporary file and placing it (or between linking and unlinking it) leaves `<id>.json.tmp` behind. That blocks every later resume and filing of that id. The refusal gives no way to recover, unlike the lock's message. | A crash inside that window. |
| 9 | `ess/domains/run.yaml:66` | CONFIRMED / introduced | The spec says "A session is filed in every case". `run_and_file`'s wire-mismatch return (`:584`) files nothing. A claimed session refused there stays Active (feeds finding 1). | A caller whose model port's wire differs from the session's. |
| 10 | `session.rs:318` | INFEASIBLE / introduced | The claim is tied neither to the directory nor to one value. A claimed session filed into another directory leaves the original Active. A `Clone` of a claimed value can overwrite a later resumer's session. | Constructed. No caller clones or switches directory. |

**5. Attacked and could not break**
- **Two resumers through lock, re-read and state write:** at most one claims, in 16 × 400 rounds.
- **Stale lock after a crash:** refused by name, the message says to remove the file, and resume works once it is gone. The how-to is only in the error message, not in the rustdoc.
- **A lock held by a run that is still active:** cannot happen. The lock is held only inside `resume`; a live run holds `Active` instead, which is finding 1.
- **Hard-link order:** the temporary file is fsynced, then linked, then unlinked, then the directory is fsynced. That order is correct.
- **Non-string panic payload:** resumed unchanged.
- **A panic during filing itself:** not reachable. `file` calls no code supplied by the caller.
- **`RunEnding` mapping:** completed is Answered; every other `LoopStop` variant is Stopped; an error or a panic is Failed. This matches the generated enum and the `FileSession` input.
- **Not tested:** a symlink placed at the lock path.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w15/loom-session-transcript-streaming/scratch/adversary-p2/` (88K: `cases-alone.log`, `gate.log`, `gate.log.test`). The mutant copy and its 533M target are deleted.
- `~/.cache/b10x-target/loom-w15-session-transcript-streaming/tmp/adversary2-*`: deleted; each test run recreates them (largest 1.7M).

Free space on `/` is 16G.

```findings
- file: crates/loom/src/session.rs
  line: 392
  category: concurrency
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "a session whose resuming run dies without unwinding stays Active on disk and every later resume is refused wrong-state, with no API call that can release it"
- file: crates/loom/src/session.rs
  line: 307
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "filing a Filed or read-only session returns SessionError::Refused instead of the specified FileSession wrong-state SessionStateConflict { state: Filed }"
- file: crates/loom/src/session.rs
  line: 217
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "OpenSession session-exists is refused only at the first filing, after the model was asked and billed, so that run's conversation can never be filed"
- file: crates/loom/src/session.rs
  line: 615
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "a run stopped AwaitingApproval is filed Stopped holding a tool call with no result and without its checkpoint, so resuming the session replays a function_call no provider accepts"
- file: crates/loom/src/session.rs
  line: 606
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the session filed after a panic records zero turns and no usage although the sink saw billed turns, which RunLedger's docs call reporting a failed run as free"
- file: crates/loom/src/session.rs
  line: 540
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "RunPorts drops hooks, cancel and environment from any run through run_and_file, and neither the module docs nor status.mdx state the trade-off"
- file: crates/loom/src/session.rs
  line: 465
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "first filing requires hard_link, so on a filesystem without hard links every new session is refused, and nothing documents it; untested for lack of such a filesystem"
- file: crates/loom/src/session.rs
  line: 761
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "a .json.tmp left by a crash blocks every later resume and filing of that id, and unlike the lock refusal the message gives no recovery"
- file: ess/domains/run.yaml
  line: 66
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the spec says a session is filed in every case, but run_and_file's wire-mismatch refusal files nothing and leaves a claimed session Active"
- file: crates/loom/src/session.rs
  line: 318
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a claimed hold is tied neither to its directory nor to one value, so filing into another directory or from a clone bypasses the lifecycle; no caller does either"
```
