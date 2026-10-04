---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-harness-module-map-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w2 adversary, loom story:harness-module-map, pass 2
relations:
- reviews: story:harness-module-map
revision: 1
---
unit: loom/harness-module-map, working tree at base 7cab979 with four untracked files
verdict: CONFIRMED (no red on the real tree; 15 doc mutants that the existing 8 cases miss, all killed by the 9 new cases)
cases: executed 9→18, red 0
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (scratch/adversary-p2/)
needs-coordinator: `cargo fmt --all --check` exits 1 on the pass-1 file `adversary_harness_map.rs:17-19` (the coordinator's `design_doc()` edit is not rustfmt-formatted). That file is not mine to change.

**1. Diff stat**
`git diff --stat` is empty because every file is untracked. `git status --short`: `adversary2_harness_map.rs` (new, mine, test file) plus the three files that were already there. I touched no non-test path. I did not take a worktree session lease.

**2. Cases added** in `crates/loom/tests/adversary2_harness_map.rs`. All 9 are green on the real tree. Each is red on the mutant it names, run with `CARGO_MANIFEST_DIR` pointed at a scratch copy. On every mutant, `harness_map` stays green (1 passed) and the pass-1 file stays green (7 passed).

| case | kills mutant | red output on the mutant |
|---|---|---|
| `every_owns_line_is_a_responsibility_the_check_reads` | m01 `* provider failover;`, m02 indented `  - …`, m03 `14. …`, m04 a second `## Owns` | ``§ Owns line `* provider failover;` is not a `- item;` bullet, so the map's coverage check never reads it`` |
| `map_sections_appear_once` | m15 a second `## Map` holding a 15th row | :136 assert |
| `port_targets_are_distinct_unnested_modules` | m05 http onto `harness/wire.rs`, m06 cli onto `harness.rs`, m07 cli onto `lib.rs` | `harness-http and harness-wire are both ported onto the module harness::wire` |
| `port_targets_land_where_a_porting_story_writes` | m08 cli onto `transcript.rs`, m09 wire onto `src/wire/` | ``harness-cli: port target `crates/loom/src/transcript.rs` is in the scope of no story that depends on the map`` |
| `seams_are_named_from_ported_crates` | m10 § Seams deleted | panic: no section |
| `new_in_loom_keeps_the_three_without_counterpart` | m11 harness-tools ported as "action selection strategy" | ``New in Loom lacks `action selection strategy` `` |
| `carried_rows_serve_a_responsibility` | m12 harness-flow `port` while serving `none` | `harness-flow: port a crate that serves no § Owns responsibility` |
| `not_carried_rows_name_another_repository` | m13 owner instead `beyond10x/loom` | :324 assert |
| `every_row_says_what_it_owns_today` | m14 "owns today" set to `—` | :339 assert |

The per-mutant table is in `scratch/adversary-p2/mutants.log`.

**3. Gate** (my file excluded nothing)
- `cargo fmt --all --check`: exit 1, on `adversary_harness_map.rs` only.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked`: exit 0. Results: lib 1 passed, adversary2 9, adversary 7, harness_map 1.

**4. Judgement findings** (cover the working tree; Harness read at `798325f0`)

| file:line | finding | verdict | origin |
|---|---|---|---|
| `docs/design/harness-map.md:58-61` | It says only `src/transcript.rs` of harness-cli is carried. But two behaviours that story:session-transcript-streaming must keep live in `harness-cli/src/lib.rs`: the pre-request refusal naming both wires (`open_session`, :2539-2550, story acceptance 6) and saving the session however the run ended (`close_session`, :2584-2590, Outcome and acceptance 7). The map says lib.rs stays in Harness. Reached by: that story porting per the map. | CONFIRMED | introduced |
| `crates/loom/tests/harness_map.rs:258-266` | The "no ported module inside another" check compares strings. `harness.rs` against `harness/http/`, `wire.rs` against `wire/`, and `lib.rs` all pass (m05-m07). | CONFIRMED | introduced |
| `crates/loom/tests/harness_map.rs:80-89,142` | The § Owns parser reads only the first § Owns, and only lines starting `- ` at column 0. A responsibility written any other way is silently uncovered (m01-m04). | CONFIRMED | introduced |
| `docs/design/harness-map.md:69-70` | It says § Does not own excludes "connector concerns" such as MCP connections. § Does not own lists only "connector credentials". | CONFIRMED | introduced |
| `docs/design/harness-map.md:64-65` | "Loom reads no … environment variable itself" is ambiguous. The ported `transcript.rs:356,369` reads `XDG_STATE_HOME` and `HOME`. True only if the sentence means credential variables. | CONFIRMED | introduced |

**5. Attacked and could not break**
- Package column: all 14 match `b10x-<dir>` in each `Cargo.toml` at 798325f0.
- Every cited line is correct: `port.rs:91/135`, `environment.rs:47`, `approval.rs:13/43`, `lib.rs:223/2181`, `transcript.rs:47`, `bearer.rs:58`.
- The § Why dependency claims are true for `[dependencies]`: wire is reached by every crate except toolchain; xtask reaches it only via cli; the ported set is closed. The dev-deps on harness-credential and harness-loop fall outside the claim.
- `798325f0` is tag 0.13.3, workspace licence `LicenseRef-B10x-Proprietary`.
- Harness has compaction, token/time/cost budgets, and both projections implement `ModelPort`.
- New in Loom: none of its three has a Harness counterpart.
- Port targets fit the scopes of story:harness-loop-port (`crates/loom/src/harness/`) and story:session-transcript-streaming (`session.rs`).
- ADR 0071/0072 exist only on Atlas PR branch `plan/governed-autonomy-north-star`, not on main. The stories cite them the same way.

**6. Written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w2/loom-harness-module-map/scratch/adversary-p2/` holds `mutate.sh`, `mutants.log`, `fmt.log`, `clippy.log`, `test.log` and `m/m00…m15/` (mutated doc copies plus a copy of the story directory).
- Build output went to `~/.cache/b10x-target/loom-w2-harness-module-map`, the build dir the brief assigned.

**7. Findings block**
```findings
- file: docs/design/harness-map.md
  line: 58
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the harness-cli row carries only transcript.rs, but the cross-wire refusal and save-on-any-end that story:session-transcript-streaming must keep live in harness-cli/src/lib.rs:2539 and :2584"
- file: crates/loom/tests/harness_map.rs
  line: 258
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the nesting and uniqueness check compares strings, so harness.rs, wire.rs beside wire/, and lib.rs pass as distinct, unnested port targets"
- file: crates/loom/tests/harness_map.rs
  line: 80
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the § Owns parser ignores star, indented and numbered items and a second § Owns, so a responsibility added that way escapes acceptance 4"
- file: docs/design/harness-map.md
  line: 69
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the doc says § Does not own excludes MCP connections as connector concerns, but it lists only connector credentials"
- file: docs/design/harness-map.md
  line: 64
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "\"Loom reads no … environment variable itself\" is false of the ported transcript.rs:356,369 unless it means credential variables only"
```
