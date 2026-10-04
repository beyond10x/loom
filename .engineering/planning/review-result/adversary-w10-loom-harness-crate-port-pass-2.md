---
format: aep.planning-md/3
id: review-result:adversary-w10-loom-harness-crate-port-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w10 adversary, loom story:harness-crate-port, pass 2
relations:
- reviews: story:harness-crate-port
revision: 1
---
unit: loom/harness-crate-port, the working tree on 4e299d5 (phase 2 and the pass-1 fixes uncommitted)
verdict: CONFIRMED
cases: executed 507→511, red 2
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: logs under `scratch/adv2/` (part 6); I deleted the scratch tree, its target dir and the Harness export
needs-coordinator: yes. Decide whether to keep `adversary2_harness_port.rs`. Its allowlist case catches a planted break that pass 1's boundary guard lets through.

**1. `git --no-pager diff --stat`**
```
 Cargo.lock                     | 1911 ++++++++++++++++++++++++++++++++++++++--
 crates/loom/Cargo.toml         |   10 +-
 crates/loom/src/harness/mod.rs |   42 +-
 3 files changed, 1880 insertions(+), 83 deletions(-)
```
- Those three paths are the implementor's uncommitted changes, not mine.
- My only write is one untracked file: `crates/loom/tests/adversary2_harness_port.rs`.
- No non-test path was edited. One exception: running `touch -c crates/loom/src/lib.rs` changed that file's modification time only, to force rustdoc to rebuild. Its content is unchanged.

**2. Cases added** (all in `crates/loom/tests/adversary2_harness_port.rs`)

| line | case | now |
|---|---|---|
| 528 | the allowlist scan catches 15 planted ways of crossing a boundary (`super::super::responses`, brace groups, a `use` tree over several lines, an `as` alias, `use super::*` reaching the harness root, a leading `::tokio`, `serde` in http, `crate::selection`, `b10x_commission`) and accepts 8 legal forms | green |
| 618 | each of the 36 ported files names only the sibling modules and crates its Harness crate listed under `[dependencies]` at 798325f0. The crate list is read from `crates/loom/Cargo.toml` | green |
| 686 | every comment paragraph that cites a Harness document names Harness | **red** |
| 727 | no ported comment line is just the dashes of a split divider | **red** |

Red output from running this file alone, before the suite (EXIT=101):
```
20 ported comment paragraphs cite a Harness document without naming Harness:
turn_loop/delegate.rs:29: A **tree** of delegates is still milestone M4 of design 0002 — …
turn_loop/event.rs:357: A hook was consulted and this is what it said (design 0002 § 3).
turn_loop/mod.rs:387 … :4660 (13 sites), turn_loop/parallel.rs:22, turn_loop/tests.rs:5189, 5432, 6181
---
comment lines that are only the dashes of a divider whose heading is on the line above:
turn_loop/tests.rs:3926
turn_loop/tests.rs:6073
turn_loop/tests.rs:6182
test result: FAILED. 2 passed; 2 failed; …
```

Mutants, run on a scratch copy with its own target dir:

| mutant | pass-1 `adversary_harness_port_boundaries` | case :618 |
|---|---|---|
| M1a: `pub const ADV2_OTHER_WIRE: &str = super::super::responses::WIRE;` in `messages/project.rs` | **3 passed** | red: `messages names module \`responses\`` |
| M1b: `#[derive(serde::Serialize)]` in `http/status.rs` (harness-http had no `serde` dependency) | **3 passed** | red: `http names crate \`serde\`` |
| M2: `wide as u8` in a `#[test]` in `turn_loop/tests.rs` | — | clippy refuses it with `cast_possible_truncation`, pointing at `mod.rs:22:22` |
| M3: the five outer `///` doc lines on the `pub mod` declarations in `harness/mod.rs` removed | — | rustdoc warnings fall from 16 to 0 |

**3. Suite run, after the cases existed** (unit build dir)

| step | summary line | exit |
|---|---|---|
| `cargo fmt --check` | (no output) | 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `Finished \`dev\` profile [unoptimized] target(s) in 0.19s` | 0 |
| `cargo test --workspace --locked --no-fail-fast` | 23 result lines: 509 passed, 2 failed, 2 ignored; the failing binary reports `test result: FAILED. 2 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s` | 101 |
| `cargo test -p b10x-loom --test ess_gate --locked` | `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s` | 0 |
| `ess specify validate --path ess --strict-requires` | `loom v1 — 2 file(s), valid` | 0 |
| `ess verify conform synthesize …` | `18 scenario(s) (0 authored), 0 refusal(s), written to …/suite.json` | 0 |
| loom-xtask `drift` | `…/generated/rust/loom: no drift from ess/` | 0 |
| loom-xtask `no-hand-model` | `…/crates/loom/src: no hand-written model type (73 reserved type names checked)` | 0 |
| `loom-docs generate --check` | `website/data/ess: 1 generated files current` / `website/docs/reference/ess: 2 generated files current` | 0 |

- `-- --list` shows all 4 of my cases in this tree.
- Suite runs 2 and 3 matched run 1: 509 passed, the same 2 failed.
- My first gate run failed fmt and clippy on my own file only. I fixed two collapsible `if`s and ran `rustfmt` on that file, then reran everything above.

**4. Findings** (all against the working tree on 4e299d5)

- **A. 20 citations of "design 0002" are still unqualified** (warning, CONFIRMED, introduced; case :686).
  - The F3 fix only rewrote the forms the pass-1 guard looks for.
  - Loom's `docs/design/` has no 0002: it holds only `harness-map.md` and `loom-design.md`.
  - What reaches it: whoever reads `turn_loop` next, starting with `story:harness-loop-port`.
- **B. The wrap_comments run split 3 section dividers** (note, CONFIRMED, introduced; case :727). `turn_loop/tests.rs:3926, 6073, 6182` are now a heading line followed by a line of dashes alone. Harness has no such line.
- **C. 16 intra-doc links no longer resolve** (note, CONFIRMED, introduced).
  - Cause: the outer `///` docs on the `pub mod` lines at `harness/mod.rs:32-42`. They make rustdoc resolve each module's own `//!` links in the parent's scope. A scratch crate and mutant M3 both confirm it.
  - Examples: `TRANSPORT`, `HookPort`, `Delegation`, `OutputSchema`, `Item::ToolResult`.
  - Nothing reaches it today, because no gate runs `cargo doc`.
  - Fix: move those five lines into each module's `//!` docs.
- **D. Pass 1's boundary guard misses breaks** (note, NEEDS-CHANGE, introduced; `adversary_harness_port_boundaries.rs:141`).
  - It is a denylist of names, so it lets M1a and M1b through.
  - Module privacy cannot hold these rules either: a module private to `harness` is visible to every child of `harness`.
  - Only separate crates (a compile error again) or the allowlist case :618 hold them.
  - Nothing in the tree breaks the rules today.
- **E. The provenance note in `harness/mod.rs:5-11` is incomplete** (note, CONFIRMED, introduced). It says the source is "unchanged except for paths, SPDX headers, one hostname and citation comments". It leaves out 11 `assert!(x.is_empty())` → `assert_eq!(x, [] as [T; 0])` rewrites and 17 comment blocks that rustfmt reflowed with no word changed.

**5. Attacked and could not break**
- **The 11 assert rewrites:** each target is a `Vec<T>`, so they assert exactly what the originals did.
- **The F5 deny:** it covers test modules (M2). No ported file adds an `allow`, except one `type_complexity` that Harness already had. The carried `harness_port_contract.rs` gives 0 pedantic warnings.
- **Code blocks in doc comments:** both fences (`price.rs` json, `project.rs` text) are intact. No rewrap created a list, heading or link definition. The rustfmt result is stable: running it again changed 0 lines.
- **Licences:** all 36 files have SPDX Apache-2.0 on line 1. No `LicenseRef` appears under `crates/`.
- **Forbidden names:** 0 hits across 58 files for the 6 policy literals, the 8 policy patterns and the private repo names. The hostnames present are vendor hostnames and `gateway.example.com`.
- **Determinism:** no test binds a fixed port (all use `127.0.0.1:0`). I ran 8 concurrent runs of the harness lib tests at 64 threads each: 392/392 passed every time.
- **Loom's model:** no ported type has the name of a generated type, and no-hand-model exits 0.
- **Harness:** HEAD is still `798325f03cf5a18df8fadb346d31b314826136ec`, and its tree is clean.

**6. Paths written outside the worktree** (under `~/.cache/ga-wave-2026-10-04-w10/loom-harness-crate-port/scratch/adv2/`)
- Kept:
  - `red-adv2.log`, `doc.log`
  - `mutant-tests.log`, `mutant-clippy.log`, `mutant-doc.log`
  - `pedantic-tests.log`
  - `stress-1.log` … `stress-8.log`
  - `gate/` (1-fmt … 9-docs, plus 3-test-run2 and 3-test-run3)
- Deleted:
  - `tree/`, `target/` (551M)
  - `docprobe/` and its target
  - `h/` (the Harness export)
  - `cw/`, `port.diff`, `cw.diff`, `lits.txt`, `pats.txt`
- I also built in the assigned dir `~/.cache/b10x-target/loom-w10-harness-crate-port`.

**7.**
```findings
- file: crates/loom/src/harness/turn_loop/delegate.rs
  line: 29
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "20 ported comment paragraphs still cite design 0002 or its milestones without naming Harness, and Loom has no design 0002."
- file: crates/loom/src/harness/turn_loop/tests.rs
  line: 3926
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The wrap_comments rewrap split three section dividers, leaving lines that are only dashes at tests.rs:3926, 6073 and 6182."
- file: crates/loom/src/harness/mod.rs
  line: 32
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Outer doc comments on the five pub mod lines make rustdoc resolve each module's own inner docs in the parent's scope, so 16 intra-doc links that resolved in Harness no longer resolve."
- file: crates/loom/tests/adversary_harness_port_boundaries.rs
  line: 141
  category: mutant
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "The pass-1 boundary guard is a denylist and stays green when messages reaches responses through super::super:: or http derives serde; the allowlist case in adversary2_harness_port.rs:618 catches both."
- file: crates/loom/src/harness/mod.rs
  line: 5
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The provenance note lists paths, SPDX headers, one hostname and citation comments as the only changes, but the port also rewrote 11 asserts and reflowed 17 comment blocks with no word changed."
```
