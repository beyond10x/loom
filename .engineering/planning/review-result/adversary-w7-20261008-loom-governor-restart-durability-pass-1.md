---
format: aep.planning-md/3
id: review-result:adversary-w7-20261008-loom-governor-restart-durability-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w7 adversary, loom story:governor-restart-durability, pass 1
relations:
- reviews: story:governor-restart-durability
revision: 1
---
# Wave 2026-10-08-w7 adversary, loom story:governor-restart-durability, pass 1

Tree `loom-20261008-w7-gov` at `9bd86f7` (base `7a48bb5`), plus the untracked test file
`crates/loom-governor/tests/restart_durability_adversary.rs` (8 cases: 5 red, 3 green).
`cargo test -p b10x-loom-governor --locked --no-fail-fast`: exit 101, 60 cases executed (52 before),
"test result: FAILED. 3 passed; 5 failed" for the adversary binary, every other binary ok.

| case | state | first-run output |
|---|---|---|
| `a_case_id_the_governor_accepts_is_one_the_file_store_can_hold` | red | `StoreUnavailable { ... File name too long (os error 36) }` |
| `a_case_file_with_an_undeclared_member_is_refused` | red | `Ok(Some(CaseState { id: CaseId("case-1"), ... revision: 1 }))` |
| `a_case_file_naming_a_member_twice_is_refused` | red | `Ok(Some(CaseState { ... revision: 1 }))` |
| `a_temporary_file_left_by_an_earlier_life_with_the_same_pid_does_not_fail_a_write` | red | `StoreUnavailable { ... .tmp-2426087-0: File exists (os error 17) }` |
| `deeply_nested_facts_the_memory_store_takes_the_file_store_takes` | red | `Err("Governor(GovernorUnavailable)")` |
| `an_awkward_case_and_its_observations_round_trip_exactly` | green | |
| `two_stores_over_one_directory_lose_nothing` | green | |
| `a_corrupted_observations_file_is_an_error_and_is_kept` | green | |

Attacked without a break: exact round trip of member order, number spellings (`-0`, `1E+2`,
30-digit, `007`), duplicate keys inside facts, `\u0000`, U+2028, surrogate pairs, `i64` bounds,
empty collections and evidence order; path-hostile ids (`..`, `/`, `\`, NUL, emoji) stay inside
the store directory; two stores on two threads lose none of 80 updates and 80 observations; a
truncated, empty or non-UTF-8 `observations.json` is an error and is kept; `CaseState` and the
`Stored*` types cannot drift silently (full struct literals in `stored_case` and `held_case`).

```findings
[
  {"file": "crates/loom-governor/src/file_store.rs", "line": 109, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "A case id over 122 bytes, which is_identifier and MemoryCaseStore accept, makes a file name over 255 bytes, so open_case fails with StoreUnavailable (ENAMETOOLONG)."},
  {"file": "crates/loom-governor/src/file_store.rs", "line": 531, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "decode_case ignores members loom.governor does not declare and the file has no format marker, so a case file from another format version reads back as a silently different case instead of an error."},
  {"file": "crates/loom-governor/src/file_store.rs", "line": 562, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "A stored-case object that names revision twice reads the first value and drops the second without an error."},
  {"file": "crates/loom-governor/src/file_store.rs", "line": 219, "category": "concurrency", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "A stale .tmp-<pid>-<n> left by a crash between temp write and rename makes a write fail with EEXIST in a restarted process that gets the same pid, and open never removes stale temp files."},
  {"file": "crates/loom-governor/src/file_store.rs", "line": 159, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "Facts nested 62 to 65 levels deep, which Commission's reader and MemoryCaseStore accept, go past the 64-level limit inside the case document and the file store refuses the evidence; documented, and nothing found that sends them."}
]
```
