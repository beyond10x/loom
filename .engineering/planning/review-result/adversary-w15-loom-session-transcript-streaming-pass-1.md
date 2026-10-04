---
format: aep.planning-md/3
id: review-result:adversary-w15-loom-session-transcript-streaming-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w15 adversary, loom story:session-transcript-streaming, pass 1
relations:
- reviews: story:session-transcript-streaming
revision: 1
---
```
unit: loom/session-transcript-streaming — worktree loom-w15-session-transcript-streaming, phase 1 21f0f0d + uncommitted phase 2
verdict: NEEDS-CHANGE
cases: executed 538→550, red 10
origin: introduced 14 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 locations (part 6)
needs-coordinator: yes. Acceptance 4 contradicts itself (finding 8). The spec-vs-code disagreements in findings 1 and 2 can be fixed in either the code or the spec, and that choice is yours.
```

The 538 comes from the implementor's reported `cases:` line. The 550 is my gate run with my file included: 540 passed and 10 failed. `origin: introduced` is for every finding because `session.rs` at base 0155045 is a 3-line stub with no functions. Several findings are ported unchanged from Harness, and their rows say so.

**1. `git --no-pager diff --stat`**
```
 12 files changed, 1406 insertions(+), 29 deletions(-)
```
All 12 are the implementor's uncommitted phase-2 files, unchanged by me. My only change is one new, untracked test file: `crates/loom/tests/adversary_session_transcript.rs` (516 lines). I changed no non-test path.

**2. Cases added** (each was red on its first run, alone; log: `scratch/adversary-p1/cases-alone.log`)

| # | Test (line) | Asserts | Red output (verbatim excerpt) |
|---|---|---|---|
| 1 | `a_session_resumed_twice_does_not_lose_the_first_runs_turns` :35 | `ResumeSession` `wrong-state`: a second resume is refused, or no turns are lost | `stored [UserText { text: "FILED-TURN" }, UserText { text: "SECOND-RESUMER-TURN" }]` |
| 2 | `opening_an_already_filed_session_id_does_not_replace_it` :72 | `OpenSession` `session-exists` | `its filing replaced the original conversation: stored []` |
| 3 | `a_session_file_whose_id_is_not_its_name_is_refused` :96 | a file's `id` matches its name | ``…ad004.json` holds session `…ad003` and was loaded as if it were `…ad004`` |
| 4 | `a_run_whose_loop_panics_still_files_its_session` :119 | the session is filed when the run dies | `a run that panicked left no session file in …/state/sessions` |
| 5 | `a_run_whose_loop_panics_leaves_the_resumed_conversation_in_the_session` :138 | `mem::take` does not lose the items | `left: [] right: [UserText { text: "EARLIER-TURN" }]` |
| 6 | `a_run_the_provider_cut_short_is_not_filed_as_answered` :158 | `RunEnding` for a stop that is not `Completed` | `stopped ProviderIncomplete { reason: "max_output_tokens" } was filed as having answered left: Some(Answered) right: Some(Failed)` |
| 7 | `a_session_is_never_filed_holding_items_of_another_wire_than_it_records` :186 | the session's wire matches the loop's wire | `recorded on `openai-responses` was filed holding … Opaque { wire: WireId("anthropic-messages") …` |
| 8 | `reasoning_text_streamed_by_the_provider_is_not_filed` :221 | acceptance 4, using a reasoning item shaped as a real provider sends it | `streamed reasoning text `ADVERSARY-REASONING-SUMMARY` is in the filed session` |
| 9 | `outside_workspace_refuses_a_dangling_symlink_into_the_workspace` :284 | the doc's promise that "a symlink cannot disguise" an in-workspace target | `points into the workspace and was accepted as Ok("…/outside/link")` |
| 10 | `a_session_over_a_non_utf8_workspace_is_filed` :302 | filing works for a workspace whose name is not UTF-8 | `Err(Refused("encoding session `…`: path contains invalid UTF-8 characters"))` |
| — | `outside_workspace_refuses_symlinks_dotdot_and_relative_paths_into_the_workspace` :256 | a symlink, a `..` path and a relative path into the workspace are all refused | green |
| — | `a_corrupt_or_truncated_session_file_is_refused_on_resume` :318 | truncated, empty and version-2 files are refused, naming the path | green |

**Mutants of the implementor's three untested absence checks** (scratch copies). All three are killed, so those checks can fail:

| Mutant | Fails at |
|---|---|
| m-cred: `ResponsesClient::turn_shared` appends the request headers as an item | `session_transcript_streaming.rs:122` `credential filed` |
| m-instr: `run_in` inserts `config.instructions` into the items | `:126` `instructions filed` |
| m-reason: `run_and_file` keeps the `ReasoningDelta` text as an item | `:133` ``REASONING-SUMMARY-ALPHA ` filed`` |

The first run of m-instr and m-reason reused the m-cred binary from the shared scratch target, and all three printed `:122`. I re-ran both after a `touch`, and saw `Compiling b10x-loom` from each copy before the results above.

**3. Gate** (unit build dir, foreground; I added `--no-fail-fast` to `cargo test --workspace`; log `scratch/adversary-p1/gate.log`)

| Command | Exit | Summary |
|---|---|---|
| `cargo fmt --check` | 1 | `Diff in …/adversary_session_transcript.rs:99:` (my file only). After `rustfmt --edition 2024` on that file: exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | |
| `cargo test --workspace --locked --no-fail-fast` | 101 | 33 lanes, 540 passed, 10 failed, 2 ignored |
| `cargo test -p b10x-loom --test ess_gate --locked` | 0 | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s` |
| `ess specify validate … && ess verify conform synthesize …` | 0 | `loom v1 — 2 file(s), valid` / `32 scenario(s) (0 authored), 0 refusal(s), written to …/suite.json` |
| `loom-xtask -- drift` | 0 | `…/generated/rust/loom: no drift from ess/` |
| `loom-xtask -- no-hand-model` | 0 | `…/crates/loom/src: no hand-written model type (98 reserved type names checked)` |
| `loom-docs -- generate --check` | 0 | `website/data/ess: 1 generated files current` / `website/docs/reference/ess: 2 generated files current` / `website/docs: no raw admonition titles` |
| `cargo doc -q -p b10x-loom --no-deps` (`RUSTDOCFLAGS=-Dwarnings`) | 0 | |

The workspace test summary lines, verbatim apart from the dropped `0 measured; 0 filtered out; finished in`:
```
tests/adversary_session_transcript.rs  test result: FAILED. 2 passed; 10 failed; 0 ignored;
tests/session_transcript_streaming.rs  test result: ok. 2 passed; 0 failed; 0 ignored;
unittests (b10x-loom)                  test result: ok. 397 passed; 0 failed; 0 ignored;
tests/harness_port_contract.rs         test result: ok. 25 passed; 0 failed; 0 ignored;
tests/adversary2_run_identity.rs       test result: ok. 1 passed; 0 failed; 1 ignored;
tests/adversary_run_revalidation.rs    test result: ok. 1 passed; 0 failed; 1 ignored;
(27 more lanes) test result: ok., 0 failed, 0 ignored each; full list in gate.log
```
`cargo test --test adversary_session_transcript -- --list` in this tree reports `12 tests, 0 benchmarks`.

**4. Findings.** All cover the working tree at 21f0f0d plus uncommitted phase 2.
- **1–2:** the unit's own ESS outcomes `ResumeSession wrong-state` and `OpenSession session-exists` are declared and generated, but `session.rs` never produces them. The result is a filed conversation silently replaced. **What reaches it:** no production caller of `session.rs` yet. It is the public API that the executor stories build on.
- **6:** `run_and_file` (`session.rs:367`) maps every `Ok` to `Answered`. That includes `ProviderIncomplete`, `MaxTurns`, `Cancelled` and `AwaitingApproval`. **What reaches it:** any provider truncation or budget stop.
- **4/5:** a panic in a caller-supplied port, tool or sink files nothing. The `mem::take` at `:364` also leaves the session's items empty. **What reaches it:** no caller yet. Harness behaved the same.
- **7:** nothing compares `session.wire` with the loop's port wire. `check_opaque_items` in the port still refuses before sending, so the safety half holds. What is lost is the refusal by name.
- **8:** the implementor disclosed this. A provider that streams reasoning text also puts that text in the opaque item, so "no streamed reasoning text" and "the opaque item byte for byte" cannot both hold. Only the story's wording can fix it.
- **Resume uses the recorded workspace** (judgement, no case): `file()` checks against the stored `workspace` (`:231`). Harness checks the current run's workspace (`lib.rs:2496`) and warns on a mismatch (`:2554`). `resume` takes no workspace at all.
- **No fsync** (judgement, ported from Harness): nothing fsyncs the file or the directory around the rename (`:260-268`).
- **`SESSION_VERSION = 1` is reused** (judgement): the Loom format adds fields and Harness's does not, so two incompatible formats carry the same version number (`:49`).
- **Stale-tmp removal** (judgement): `save` deletes an existing `.json.tmp` (`:252`), where Harness refuses through `create_new`. In the probe, 2 writers × 150 filings with a concurrent reader: Loom refused 55–94 of 300 filings, the Harness-style code 20–28. Torn reads were 0 out of about 12,800.

**5. Attacked and could not break**
- Mode 0700/0600, credential, instruction and reasoning absence: all three absence checks kill their mutants.
- `SessionError::WireMismatch` wraps the generated `SessionWireMismatch` exactly (`session_id`, `session_wire`, `wire`), and matches the spec's `cross-wire` payload.
- A truncated, empty or other-version file is refused by name.
- A symlink into the workspace, a `..` path and a relative path are all refused.
- Concurrent filing: no torn file observed.
- An IO error while filing returns `filed: Err` and keeps `run`.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w15/loom-session-transcript-streaming/scratch/adversary-p1/`: logs and `probe_race.rs`, 100K. The copies `m-cred`, `m-instr`, `m-reason` and their `target/` (877M) are already deleted.
- `~/.cache/b10x-target/loom-w15-session-transcript-streaming/tmp/adversary-*`: 24 small test dirs inside the unit build dir, recreated on every run.

I created no `loom-w15-*` target dir. The implementor's `loom-w15-sts-mutant` and `loom-w15-sts-base` (about 1.16G) are still there. Free space on `/` is 13G.

```findings
- file: crates/loom/src/session.rs
  line: 297
  category: contract-drift
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "resume never refuses a session another run already resumed (ResumeSession wrong-state), so two resumers' filings silently drop the first one's turns"
- file: crates/loom/src/session.rs
  line: 141
  category: contract-drift
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "opening an id that is already filed is not refused (OpenSession session-exists), and filing it replaces the stored conversation with an empty one"
- file: crates/loom/src/session.rs
  line: 278
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "load accepts a file whose id field differs from its file name, so a resume continues a different session than the one asked for; only reached by renaming or copying a file"
- file: crates/loom/src/session.rs
  line: 357
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "a run whose loop panics files no session, although the story says a session is written whether the run answered or died"
- file: crates/loom/src/session.rs
  line: 364
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "after a caught panic the session's items are empty because of mem::take, so a caller that then files it erases the stored conversation; no caller catches panics today"
- file: crates/loom/src/session.rs
  line: 367
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "every Ok loop outcome is filed as RunEnding Answered, including ProviderIncomplete, MaxTurns, Cancelled and AwaitingApproval"
- file: crates/loom/src/session.rs
  line: 357
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "run_and_file never compares the session's recorded wire with the loop's port wire, so a session can be filed holding another wire's opaque items; the port's check_opaque_items still blocks the send"
- file: crates/loom/tests/session_transcript_streaming.rs
  line: 41
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "acceptance 4 cannot hold for a real provider, because the streamed reasoning text is inside the opaque item it requires byte for byte; the emulator hides this with summary []"
- file: crates/loom/src/session.rs
  line: 414
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "outside_workspace accepts a dangling symlink into the workspace because exists() is false for it; save's create_dir_all then refuses, so nothing is written"
- file: crates/loom/src/session.rs
  line: 248
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "a session over a workspace whose path is not UTF-8 can never be filed, because serde refuses to encode the PathBuf (ported from Harness)"
- file: crates/loom/src/session.rs
  line: 231
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "after a resume, file checks outside-workspace against the recorded workspace rather than the resuming run's, where Harness checks the current workspace and warns on a mismatch"
- file: crates/loom/src/session.rs
  line: 260
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "neither the temporary file nor the directory is fsynced around the rename, so a crash can leave an empty or missing session (ported from Harness)"
- file: crates/loom/src/session.rs
  line: 49
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "SESSION_VERSION 1 names a format that differs from Harness's version 1, so the two incompatible formats carry the same version number"
- file: crates/loom/src/session.rs
  line: 252
  category: concurrency
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "deleting an existing tmp file, where Harness refuses with create_new, raised refused concurrent filings from 20-28 to 55-94 of 300 in the probe, with no torn reads observed"
```
