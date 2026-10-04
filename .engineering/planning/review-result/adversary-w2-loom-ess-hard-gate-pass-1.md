---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-ess-hard-gate-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w2 adversary, loom story:ess-hard-gate, pass 1
relations:
- reviews: story:ess-hard-gate
revision: 1
---
unit: loom/ess-hard-gate, working tree at base 7cab979 plus the uncommitted diff and the untracked `crates/loom/tests/ess_gate.rs`
verdict: NEEDS-CHANGE
cases: executed 8→18, red 1
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths
needs-coordinator: none

**1. What I touched**

`git --no-pager diff --stat` only shows the implementor's tracked files: AGENTS.md, Taskfile.yml and ess/domains/run.yaml (42 lines added, 10 removed). My only addition is the untracked `crates/loom/tests/adversary_ess_gate.rs`, a test file (`git status -uall` shows it). I did not touch any non-test path.

**2. Cases added** (all in `~/.local/state/worktree/trees/b10x/loom/loom-w2-ess-hard-gate/crates/loom/tests/adversary_ess_gate.rs`)

Each case builds a small copy of the repo under `CARGO_TARGET_TMPDIR`. The copy has the real `ess_gate.rs` copied unchanged, plus `ess/`, `AGENTS.md`, `Taskfile.yml` and an empty library. The case changes one thing in the copy, runs `cargo test --test ess_gate` there, and checks the exit status and the failing test names.

| case | asserts | now |
|---|---|---|
| `gate_checks_the_tree_it_runs_in_when_worktrees_share_a_build_directory` | trees A (clean) and B (with `# UNMAPPED: probe` appended) share one target dir; the gate must fail in B | **red** |
| 9 `mutant_*_is_killed` cases | the gate suite fails, naming the right test and file:line | green |

Red output, from running the case alone (`cargo test -p b10x-loom --locked --test adversary_ess_gate -- --exact gate_checks_…`, exit 101):
```
panicked at crates/loom/tests/adversary_ess_gate.rs:161:5:
the ESS gate passed in a worktree whose ess/domains/run.yaml ends with `# UNMAPPED: probe`: it ran the binary built for …/shared_target/worktree-a and checked that tree's ess/
exit status: 0
test gate_holds_on_the_specification ... ok
test result: ok. 7 passed; 0 failed
--- stderr
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.00s
     Running tests/ess_gate.rs (…/shared_target/target/debug/deps/ess_gate-26075a946da8124a)
```

**3. Suite run** (after the cases existed)

Command: `cargo test --workspace --locked --no-fail-fast`, exit 101.

| target | result |
|---|---|
| lib unit tests | 1 passed |
| `adversary_ess_gate` | 9 passed, 1 failed |
| `ess_gate` | 7 passed |
| doc tests | 0 |

- **Before count (8):** this same run with my file left out, i.e. 1 lib test plus 7 `ess_gate` tests.
- **fmt and clippy:** `cargo fmt -p b10x-loom --check` exited 0. `cargo clippy --workspace --all-targets --locked -- -D warnings` exited 0.

**4. Findings**

| file:line | verdict | origin | finding |
|---|---|---|---|
| `crates/loom/tests/ess_gate.rs:219-224` | NEEDS-CHANGE | introduced | `repo_root()` takes the repo path from `env!("CARGO_MANIFEST_DIR")`, which is fixed into the binary when it is built. If two worktrees share a build directory, cargo reuses one worktree's `ess_gate` binary in the other without recompiling. The gate then checks the first worktree's `ess/` and passes on a tree that carries a marker. |

- **What was measured:** the red case above, and a manual repeat in scratch: B passed with the shared target (exit 0) and failed with its own target (exit 101, `domains/run.yaml:153`).
- **What reaches it:** `Taskfile.yml:4` sets `CARGO_TARGET_DIR` to `~/.cache/b10x-target/loom` for every loom worktree's `task check` / `task ess-gate`. Worktrees created before another one builds have older files, so cargo treats the other build as up to date. This is the same failure as the 2026-09-23 incident in `~/.claude/CLAUDE.md`. CI is not affected, because each run starts from a fresh checkout.
- **Fix (not applied):** in `repo_root()`, read the path at run time with `PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR")…)`. I tested this on the scratch copies: B reused the same binary and correctly failed at `domains/run.yaml:153`.

**5. Attacked and could not break**
- **Exit status:** every `ess` step checks its exit status; an `ess` missing from PATH makes the gate fail rather than pass.
- **Refusal count:** if no count can be read, the gate fails. A Binary64 confidence makes synthesize exit 1.
- **Marker scan:** it searches every file under `ess/` at any depth, including hidden and non-YAML files, and finds the marker mid-line. Those files are invisible to ess because `ess-inputs.yaml` lists the spec files explicitly, so the scan is the only check that sees them.
- **JSON reader (item 2):** every expectation compares against a literal value and a missing path fails the test, so it cannot pass vacuously.
- **Mutants caught:**
  - confidence changed to Binary64
  - the `Turn.catalogue` relation dropped
  - its cardinality changed to `many`
  - `commission_run` back to `String`
  - `CommissionRunId` as a newtype of `String`
  - `task check` with the `ess-gate` step removed
- **Output location:** the suite file is written under `CARGO_TARGET_TMPDIR`; nothing is written into `ess/` (git shows only the implementor's change there).
- **CI:** `check.yml` installs ess 0.52.0 and runs `task check`.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w2/loom-ess-hard-gate/scratch/adv` (8.0M: probe copies, `stale/` trees and targets, logs)
- `~/.cache/b10x-target/loom-w2-ess-hard-gate/tmp/adversary_ess_gate` (78M, inside the unit's build dir; my test runs recreate it)

```findings
- file: crates/loom/tests/ess_gate.rs
  line: 220
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "repo_root uses compile-time env!(\"CARGO_MANIFEST_DIR\"), so under the Taskfile's shared CARGO_TARGET_DIR a reused ess_gate binary checks another worktree's ess/ and passes a tree carrying an UNMAPPED: marker"
```
