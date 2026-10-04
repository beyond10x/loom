---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-harness-module-map-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w2 adversary, loom story:harness-module-map, pass 1
relations:
- reviews: story:harness-module-map
revision: 1
---
unit: loom/harness-module-map, working tree at base 7cab979 plus untracked `docs/design/harness-map.md`, `crates/loom/tests/harness_map.rs` and my `crates/loom/tests/adversary_harness_map.rs`
verdict: broken. 1 red case (CONFIRMED, warning); 8 mutants of the doc pass the unit's test
cases: executed 2→9, red 1
origin: introduced 10 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory (scratch/adversary-p1, listed in part 6)
needs-coordinator: none

**1. Diff stat.** `git --no-pager diff --stat` is empty because every change is untracked. `git status --short -uall`:
```
?? crates/loom/tests/adversary_harness_map.rs   <- mine, a test file
?? crates/loom/tests/harness_map.rs             <- implementor
?? docs/design/harness-map.md                   <- implementor
```
Every path I touched is a test file. I edited no implementation file and no doc.

**2. Cases added** in `~/.local/state/worktree/trees/b10x/loom/loom-w2-harness-module-map/crates/loom/tests/adversary_harness_map.rs`. Six are green against the real doc and red against the mutant named. One is red now.

| case | asserts | now | kills |
|---|---|---|---|
| `adversary_map_section_holds_one_contiguous_table` | every `\|` line in § Map belongs to the one table | green | M05 |
| `adversary_port_licence_runs_from_harness_to_loom` | the licence cell is exactly `LicenseRef-B10x-Proprietary → Apache-2.0` | green | M06 |
| `adversary_port_targets_are_distinct_loom_paths` | the target starts with `crates/loom/src/` and no two crates share one | green | M07, M08 |
| `adversary_not_carried_rows_name_only_their_owner` | target and licence are `—`; owner is exactly `beyond10x/<repo>` | green | M09, M10 |
| `adversary_depend_rows_name_the_pinned_revision` | a depend row names `798325f0` | green | M11 |
| `adversary_every_owned_responsibility_reaches_loom` | each § Owns item is served by a port or depend row, or is in New in Loom | green | M12 |
| `adversary_port_rows_name_no_owner_instead` | a port row's "owner instead" cell is `—` | **red** | M13 |

Red output of that case alone, captured before any suite run (`cargo test -p b10x-loom --locked --test adversary_harness_map`):
```
thread 'adversary_port_rows_name_no_owner_instead' (1491683) panicked at crates/loom/tests/adversary_harness_map.rs:158:13:
assertion `left == right` failed: harness-cli: a port row that also names an owner instead
  left: "the binary, approver, hook runner and environment block stay in beyond10x/harness"
 right: "—"
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```
After I ran `rustfmt` on my file, the same assertion is at line 172.

Mutants were run on copies under scratch (`mutate.sh`), against both test binaries:

| mutant | `harness_map.rs` | adversary |
|---|---|---|
| M01 rename a crate, M02 duplicate a row, M03 `port, not carried` in one cell, M04 a 15th row right after the table | killed | n/a |
| M05 a 15th row after a blank line inside § Map | **missed** | killed |
| M06 licence reversed (`Apache-2.0 → LicenseRef-…`) | **missed** | killed |
| M07 harness-wire target moved onto `…/harness/loop/` (two crates, one module) | **missed** | killed |
| M08 target `not crates/loom/ at all` | **missed** | killed |
| M09 owner `not beyond10x/harness` | **missed** | killed |
| M10 a not-carried row that also has a target and a licence | **missed** | killed |
| M11 harness-toolchain changed to `depend` with no revision | **missed** | killed |
| M12 compaction served only by not-carried harness-flow | **missed** | killed |
| M13 a port row that also names an owner | **missed** | killed |

**3. Suite run** (my own CARGO_TARGET_DIR, after the cases existed):
- `cargo fmt --all --check`: exit 1 on the first run, from my own file only. I ran `rustfmt` on that file alone and the rerun exited 0.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0.
- `cargo test --workspace --locked --no-fail-fast`: exit 101.
  - lib unittests: 1 passed.
  - `adversary_harness_map`: 6 passed, 1 failed (`adversary_port_rows_name_no_owner_instead`).
  - `harness_map`: 1 passed.
- `executed 2` is that same run minus my 7 cases. `cargo test -- --list` confirms all 9 test names exist in this tree.

**4. Findings** (they cover the working tree above)
- **harness-map.md:30, red case.** The harness-cli row is `port` and also names an "owner instead". By the doc's own legend that column belongs to `not carried`, so this one row holds two dispositions, and `harness_map.rs` cannot tell it from a whole port. What reads it: the placement rule of story:harness-loop-port, which acts on every `port` row. Fix: put `—` in that cell; the "stays in Harness" part is already in § Why. CONFIRMED, warning, introduced.
- **harness_map.rs:100-101.** The table parser stops at the first line that is not a table row, so a row placed after a blank line shows on the page and is never checked (M05). CONFIRMED, warning, introduced.
- **harness_map.rs:161, :166.** The target and licence checks only test that the text appears somewhere in the cell, so M06, M07 and M08 pass. CONFIRMED, note, introduced.
- **harness_map.rs:157-182.** Nothing is checked on a depend row, or on the target and licence cells of a not-carried row, and the owner check is a substring match. M09, M10 and M11 pass. CONFIRMED, note, introduced.
- **harness_map.rs:146-153.** A row counts toward § Owns coverage whatever its disposition, so a responsibility served only by a crate Loom does not take counts as covered (M12). This matches the literal wording of acceptance item 4 but not the story's Outcome. CONFIRMED, note, introduced.
- **harness-map.md:42.** The doc says every crate except harness-toolchain depends on harness-wire, citing each crate's `Cargo.toml`. That is false for harness-xtask: its `[dependencies]` at 798325f0 are clap, harness-cli, harness-toolchain, serde_json and sha2. It reaches harness-wire only through harness-cli, so the "no depend row" conclusion still holds. CONFIRMED, note, introduced.
- **harness-map.md:79.** "`ApprovalPort::decide` (line 42)" is off by one: `approval.rs:42` is `pub trait ApprovalPort`, and `fn decide` is on line 43. CONFIRMED, note, introduced.
- **harness-map.md:56.** "Loom does not own credentials" overstates its sources. `loom-design.md:44` and AGENTS.md § Boundary both say *connector* credentials, and the doc's own harness-credential row says it serves model API invocation. CONFIRMED, note, introduced.
- **harness-map.md:29.** The target `crates/loom/src/harness/loop/` uses a Rust keyword as a module name, so every caller must write `harness::r#loop`. I checked that `pub mod r#loop;` compiles with rustc 2024. CONFIRMED, note, introduced.
- **harness-map.md:42-47.** The map names crates by directory (`harness-wire`), but `cargo tree` prints package names (`b10x-harness-wire`). If story:harness-loop-port's acceptance 5 matches names exactly, it can never find a port crate, and the guard the doc relies on does nothing. Nothing ties the two names together today. CONFIRMED, note, introduced.

**5. What I attacked and could not break**
- The crate list matches `ls-tree` at 798325f0: 14 crates.
- The "owns today" cells match README § Layout. They are shortened, and nothing is misattributed.
- These cited lines are correct: port.rs:91 and :135, environment.rs:47, approval.rs:13, lib.rs:223 and :2181, bearer.rs:58, transcript.rs:47.
- The ported set is closed:
  - http depends only on wire, among Harness crates.
  - responses and messages depend only on http and wire.
  - loop depends only on wire. Its mentions of `harness_tools` are in doc comments only.
  - transcript.rs uses only harness_loop and harness_wire.
- § Owns is covered: 10 of its 13 items are named by port rows, and the other 3 are in New in Loom.
- The licence string appears in no file under `crates/`.
- The map is consistent with story:harness-loop-port (acceptance 5, placement), story:session-transcript-streaming (`session.rs`) and story:frontier-projection (the `environment.rs:47` seam).
- harness-loop's own tools (answer, delegate, skill, recall) are only published when a caller opts in, so story:harness-loop-port's acceptance 2 still holds.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w2/loom-harness-module-map/scratch/adversary-p1/`, containing:
  - `target/` (build dir)
  - `mut/` (probe package, doc copies, `mutants/*.md` and `*.adv.log`)
  - `mutate.sh`
  - `red-alone.log`, `fmt.log`, `clippy.log`, `suite.log`
- I deleted the `rawmod/` probe. I took a worktree lease under session `adversary-w2-harness-module-map-p1` and released it.

```findings
- file: docs/design/harness-map.md
  line: 30
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the harness-cli port row also fills "owner instead", a second disposition in one row that the unit's test cannot detect
- file: crates/loom/tests/harness_map.rs
  line: 100
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the table parser stops at the first non-row line, so a row after a blank line inside § Map is never checked (M05)
- file: crates/loom/tests/harness_map.rs
  line: 161
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the port target and licence are only checked by substring, so a reversed licence, a non-path target or two crates on one module pass (M06-M08)
- file: crates/loom/tests/harness_map.rs
  line: 157
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: depend rows and not-carried target and licence cells are unchecked and the owner check is a substring, so M09-M11 pass
- file: crates/loom/tests/harness_map.rs
  line: 146
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: § Owns coverage counts not-carried rows, so a responsibility only a dropped crate serves passes as covered (M12)
- file: docs/design/harness-map.md
  line: 42
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: harness-xtask's Cargo.toml at 798325f0 does not depend on harness-wire; it reaches it only through harness-cli
- file: docs/design/harness-map.md
  line: 79
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: ApprovalPort::decide is approval.rs:43, not line 42 (line 42 is the trait)
- file: docs/design/harness-map.md
  line: 56
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the cited sources exclude only connector credentials, not credentials in general
- file: docs/design/harness-map.md
  line: 29
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the target module name loop is a Rust keyword and is reachable only as harness::r#loop
- file: docs/design/harness-map.md
  line: 42
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the map names crates by directory while cargo tree prints b10x-harness-* package names, so an exact-name check in harness-loop-port acceptance 5 can never match
```
