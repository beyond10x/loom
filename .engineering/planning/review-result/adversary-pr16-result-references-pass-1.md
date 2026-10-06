---
format: aep.planning-md/3
id: review-result:adversary-pr16-result-references-pass-1
kind: review-result
status: active
title: PR 16 adversary, loom story:result-references, pass 1 (session loom, landing)
relations:
- reviews: story:result-references
revision: 1
---
```
unit: story:result-references (PR 16), worktree loom-pr16-result-references at 7d87136 (PR diff bccdaa5...db9bf2b)
verdict: red
cases: executed 70→78, red 5
origin: introduced 5, pre-existing 0, undecided 0
wrote-outside-worktree: ~/.cache/loom-waves-2026-10-06/pr16/adversary/ (7 logs), ~/.cache/b10x-target/loom-pr16 (assigned build dir, now 1.2G)
needs-coordinator: yes — decide whether finding 3 (bad reference ends the run as ExternalAvailability) is accepted design or a change
```

**1. Diff proof.** `git --no-pager diff --stat` is empty, so no tracked file changed. `git status --short` shows only 5 untracked test files, all under `crates/loom-intake-slice/tests/`: `adversary_pr16_{lookups,briefing,refusal,reference_cap,edit_budget}.rs`. Nothing was committed or staged, and nothing was written under `.engineering/`.

**2. Cases, each run alone before the suite**

| File | Case | Now | Red output when first run alone |
|---|---|---|---|
| adversary_pr16_lookups.rs | `eight_lookups_of_the_size_the_model_is_told_leave_room_for_the_action` | red | `Err("stored-result lookup context budget exhausted")` |
| adversary_pr16_lookups.rs | `a_malformed_catalogue_offset_is_a_lookup_error_not_an_aborted_generation` | red | `Err("result listing offset must be a nonnegative integer")` |
| adversary_pr16_briefing.rs | `a_tests_run_entry_still_tells_the_model_the_confinement_the_executor_reported` | red | prompt is `tests.run exit_code=Some(1) timed_out=false implementation=Some("21bc…")` plus a descriptor; no confinement |
| adversary_pr16_briefing.rs | `a_tests_run_on_a_dirty_tree_still_says_the_tree_had_uncommitted_changes` | red | prompt is `… implementation=None`; no "uncommitted" |
| adversary_pr16_refusal.rs | `an_unresolvable_reference_is_not_reported_as_an_unreachable_model` | red | `Err(Suspended("ExternalAvailability(Object([(\"error\", Text(\"unknown result in this run\"))]))"))`; output has no `step` or `stopped:` line |
| adversary_pr16_reference_cap.rs | 2 cases that catch a mutant | green | n/a |
| adversary_pr16_edit_budget.rs | `two_files_composed_from_one_full_size_capture_exceed_the_per_edit_budget` | green | n/a (catches a mutant) |

In both briefing cases the first assertion passed: the executor's own `Report` still carries `confinement: none` and `on a work tree with uncommitted changes`. Only the briefing drops them.

**3. Suite.** Command: `cargo test -p b10x-loom-intake-slice --locked --no-fail-fast`, exit 101. Every binary was green except my three red files. The final run shows 78 executed, 7 ignored and 5 failed. The 7 ignored tests (`adversary_case` 1, `adversary_confinement` 5, `confinement` 1) are opt-in confinement tests that were already ignored before this pass. The "before" count of 70 comes from the same run minus my 5 binaries (8 cases). Every binary named in the run exists in this tree.

**4. Findings** (all cover 7d87136; every line cited was added by the PR)

- **F1, selector.rs:376 against :84.** The model is told "At most eight lookups, each at most 8192 UTF-8 bytes". The 64 KiB total cap on lookup responses is never mentioned to it. One plain-ASCII 8192-byte lookup renders as 8,269 bytes, so 8 of them come to 66,152, over 65,536. I measured the error; that it trips on the 8th lookup is my arithmetic. Text that needs JSON escaping trips it sooner. When it trips, generation aborts instead of answering. **What reaches it:** a model reading a file larger than 8 KiB in consecutive chunks, which the instructions invite. Fix either way: tell the model about the 64 KiB cap, or return over-budget lookups as `lookup_failed`.
- **F2, selector.rs:370.** The design says "errors are explicit and consume the same budget", and a failed `$read_result` is answered that way. An invalid `$list_results` offset instead aborts generation. **What reaches it:** the provider projection is `strict:false`, so the schema's `minimum: 0` is not enforced and a model can send a negative or string offset.
- **F3, selector.rs:382 → loom-executor lib.rs:507 `outage`.** An edit reference that does not resolve (an off-by-one range, a wrong digest, an unknown id) ends the whole run as `SliceError::Suspended(ExternalAvailability)`. The CLI then exits 1, and the model is never told why. The story says such selections "refuse before effects", and `run.rs` reserves `Suspended` for when "the agent model could not be reached". The same mistake with literal arguments becomes a refused step that the model hears about. **What reaches it:** any reference error in a real model's output. The PR's own test asserts `is_err()`, so this was a choice and is the coordinator's call.
- **F4, selector.rs:148.** At base (bccdaa5:selector.rs:98) the tests.run briefing entry quoted `Report`'s Display. It now drops the applied confinement, the timeout seconds and the "uncommitted changes" wording, which become only `implementation=None`. That contradicts the module doc ("An entry quotes what the executor reported") and AGENTS.md ("Keep original statuses … explicit").
- **F5, selector.rs:584.** `tiny_selection_cannot_amplify_an_oversized_reference` passes with the 4 KiB encoded-reference cap deleted, because `parse_reference` already rejects its 5001-byte pointer with a "4096" message. `adversary_pr16_reference_cap.rs` shows this and gives the cap an input only the cap rejects.
- **F6, selector.rs:212.** No test calls `resolve_arguments` with more than one file, so a mutant that never decrements `remaining` would pass the whole suite. That conclusion comes from grep, not a mutation run. `adversary_pr16_edit_budget.rs` is green now and would catch it.

**5. Attacked and could not break**
- **Ordering:** references expand before admission. `generate` returns the resolved arguments, which then go through revalidation, the ProposedAction and the executor.
- **Isolation:** a reference from another run or another briefing does not resolve, because the store is looked up by object identity, not by the result id.
- **Confinement and git hardening:** references carry no path, authority or case revision, so path traversal, symlinks and the host-git rules are unaffected. The hardening scan covers `src/` only, and the new code runs no commands.
- **Digest:** checked at expansion against the stored capture. The current file on disk is never read; the design intends this.
- **Selectors and JSON reader:** UTF-8, CRLF, phantom-line and empty-file edges hold. Pointer escapes, duplicate keys, number spelling, depth and node limits, and slicing on character boundaries all hold.
- **Evidence:** the briefing never becomes evidence; the verifier still reads only the `Report`.
- **ESS:** the `results` domain is data-only and matches the generated types. There are no behaviour scenarios to drive.

**6. Paths written outside the worktree**
- `~/.cache/loom-waves-2026-10-06/pr16/adversary/`: `case-lookups.log`, `case-briefing.log`, `case-reference-cap.log`, `case-refusal.log`, `case-edit-budget.log`, `suite.log`, `suite2.log`
- `~/.cache/b10x-target/loom-pr16`: assigned build dir, 149M → 1.2G
- Test fixture workspaces under that build dir's `tmp/` are deleted when each test ends.

```findings
[
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 376, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The model is told it may make eight 8192-byte lookups, but the unmentioned 64 KiB response cap is exceeded by the eighth and aborts argument generation."},
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 370, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "An invalid $list_results offset aborts generation instead of being answered as a lookup error that uses up budget, as the design says lookup errors are."},
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 382, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "An edit reference that does not resolve ends the run as an ExternalAvailability suspension (CLI exit 1, model never told) rather than the refusal the story and run.rs describe."},
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 148, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "The tests.run briefing entry no longer quotes the executor's report: applied confinement, timeout seconds and the uncommitted-tree statement are dropped."},
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 584, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "tiny_selection_cannot_amplify_an_oversized_reference passes with the 4 KiB encoded-reference cap deleted, because the pointer field limit alone produces its 4096 message."},
  {"file": "crates/loom-intake-slice/src/selector.rs", "line": 212, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "No package test drives resolve_arguments with more than one file, so a mutant that drops the per-edit 16 MiB budget across files would pass the suite."}
]
```
