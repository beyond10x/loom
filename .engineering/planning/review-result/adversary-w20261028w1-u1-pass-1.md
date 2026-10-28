---
format: aep.planning-md/3
id: review-result:adversary-w20261028w1-u1-pass-1
kind: review-result
status: active
title: Wave 2026-10-28-w1 adversary, loom story:executor-reports-harness-child-failure, pass 1
relations:
- reviews: story:executor-reports-harness-child-failure
revision: 1
---
Adversary pass 1 on u1 (`aep:adversary`, opus), wave 2026-10-28-w1, tree `loomd163-a1` at `6a51bcbe9`
on integration head `4b0b50fb8`. The report as it returned:

unit: story:executor-reports-harness-child-failure, tree loomd163-a1 at 6a51bcbe9 (on integration head 4b0b50fb8)
verdict: red
cases: executed 40→46, red 5
origin: introduced 5, pre-existing 0, undecided 0
wrote-outside-worktree: 4 logs in ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-d163/a1/ (red.log, red-fmt.log, suite-before.log, suite-after.log); shared wave target written via cargo-locked.sh
needs-coordinator: yes (route the 5 red cases to u1)

**Diff:** `crates/loom-session-executor/tests/adversary_w20261028w1_a1.rs | 364 +++`. That is the only path, and it is a test file. Commit `6a51bcbe9`, author and committer `b10x-bot[bot]`.

**Cases.** I ran the file alone before running anything else. 5 failed and 1 passed:
1. `adversary_an_ansi_escape_right_before_a_token_does_not_hide_it_from_the_walk` (red). The `m` that closes `ESC[1m` is a letter, so `sk-` does not count as the start of a word. The key appears in full in the reason and in the record.
2. `adversary_a_github_installation_token_on_stderr_is_withheld` (red). `TOKEN_PREFIXES` lists `ghp_` and `github_pat_` but not `ghs_`, `gho_`, `ghu_` or `ghr_`.
3. `adversary_a_last_line_widened_past_the_bound_by_replaced_controls_still_leaves_a_tail` (red). A 4023-byte progress line of `\r` updates fits the bound as written. Each `\r` becomes U+FFFD (3 bytes), so the line grows to about 5600 bytes. The second pass then keeps zero lines and no note stands in for them:
   `the child wrote 4023 bytes on stderr, the record keeps none of them: SessionFailure { end: Signaled, code: Some(9), stderr_tail: None }`
4. `adversary_a_child_cut_off_inside_its_json_line_reports_its_end_and_stderr_tail` (red). For a malformed line (`Unreplied::Malformed(why) => why`, lib.rs:1518) the reason drops the exit status and the stderr tail. The record for the same session holds both:
   `the reason names no exit status: "the session's JSON line does not read: EOF while parsing a string at line 1 column 20"`
5. `adversary_two_sessions_failed_for_different_reasons_leave_different_records` (red): the records cannot tell "...without the JSON line naming its action (the harness exited with status 0)" from "the session's JSON line does not read: expected value at line 1 column 12", with both sides `Some(SessionFailure { end: Exited, code: Some(0), stderr_tail: None })`
6. `adversary_a_window_starting_inside_a_multibyte_character_keeps_whole_lines` (green; the cut lands inside a 4-byte character and lines end in CRLF).

**Suite.** Without the file: EXIT=0, 40 passed (harness_child_failure, lineage, proposes, adversary_w20261023w3_u2). With `--no-fail-fast` and the file: EXIT=101, the file `1 passed; 5 failed`, the other four unchanged.

**What reaches each case:** case 1: the unit's own fixture treats ANSI-coloured stderr as a real input; no harness child in the organization found that colours stderr. Case 2: nothing found that prints a `ghs_` token on stderr (INFEASIBLE). Case 3: a child killed while drawing a progress meter. Case 4: a model wire that refuses partway through a message, the F2 shape. Case 5: what the yaml's `SessionFailed` comment promises ("recorded with why"); takt reads `sessions().pop()` (harness.rs:586).

**Judgement, no case.** Metaharness's `NO_TERMINAL_RECORD` warning carries the whole unredacted stderr. It still reaches `SessionReport::transcript`, which takt keeps on disk at mode 0600, and the event observer. The crate docs say a withheld line is "never repeated". A note, origin undecided (the base was not run); left out of the findings block.

**Attacked and could not break:** a window starting inside a multi-byte character and CRLF line ends; a PEM block whose BEGIN line the window cuts off; a URL with userinfo containing `?` or `#`; a tab after `Bearer`, a prefix only inside a word; lineage after a failed row; a session refused before its child started is still no record; the `RecordFailedSession` conformance outcome passes (its target only round-trips stderr tails that pass the walk unchanged; a note).

```findings
- file: crates/loom-session-executor/src/lib.rs
  line: 2738
  category: boundary
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: an ANSI SGR sequence ending in `m` right before a token defeats the word-start rule, so `sk-...` reaches the reason and the AgentSession record unredacted
- file: crates/loom-session-executor/src/lib.rs
  line: 143
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: TOKEN_PREFIXES withholds ghp_ and github_pat_ but not GitHub's ghs_/gho_/ghu_/ghr_ tokens; no producer of one on stderr was found
- file: crates/loom-session-executor/src/lib.rs
  line: 2699
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a last line within the bound as written but over it after control characters become U+FFFD leaves an empty tail and no note, so the record says the child wrote nothing
- file: crates/loom-session-executor/src/lib.rs
  line: 1518
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: a child cut off inside its JSON line suspends with a reason naming neither its exit status nor its stderr tail, while its record holds both
- file: ess/domains/session.yaml
  line: 184
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: SessionFailed is documented as recorded with why, yet sessions that failed for different reasons leave identical failure records holding only the child's end
```
