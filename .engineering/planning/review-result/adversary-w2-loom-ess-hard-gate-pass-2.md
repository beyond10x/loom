---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-ess-hard-gate-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w2 adversary, loom story:ess-hard-gate, pass 2
relations:
- reviews: story:ess-hard-gate
revision: 1
---
```
unit: loom/ess-hard-gate, working tree ~/.local/state/worktree/trees/b10x/loom/loom-w2-ess-hard-gate (base 7cab979 + uncommitted implementor and pass-1 changes)
verdict: CONFIRMED (2 surviving gate-code mutants; 0 implementation defects)
cases: executed 19→23, red 2
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 12 paths (part 6)
needs-coordinator: none
```

The gate behaves correctly on every input I tried. The gap is in its tests: `ess_gate.rs` would still pass if two of its checks were removed.

**1. Diff stat**

`git --no-pager diff --stat` shows `AGENTS.md | 13`, `Taskfile.yml | 6`, `ess/domains/run.yaml | 33`. All three lines are the implementor's edits. My only change is one new, untracked test file: `crates/loom/tests/adversary2_ess_gate.rs`. I touched no other path.

**2. Cases added** (`adversary2_ess_gate.rs`; same replica method as pass 1: copy the repo, change the copy, run the real `cargo test --test ess_gate` in it)

| case | asserts | now |
|---|---|---|
| `mutant_gate_accepting_any_refusal_count_is_killed` | the suite fails if `Some(0) => {}` becomes `Some(_) => {}` in the copy of `ess_gate.rs:99` | **red** |
| `mutant_validate_without_strict_requires_is_killed` | the suite fails if `"--strict-requires"` is removed from the copy of `ess_gate.rs:78` | **red** |
| `mutant_spec_refused_by_synthesize_with_exit_0_is_killed` | the current gate fails when `run.yaml` gets a command with an unsatisfiable outcome (`ESS-SYNTH-003`, ess exits 0) | green |
| `mutant_spec_requiring_an_older_ess_is_killed` | the current gate fails on `requires: ess 0.51.0` | green |

Red output, from the first run of the file alone (`~/.cache/ga-wave-2026-10-04-w2/loom-ess-hard-gate/scratch/adv2-red.log`, EXIT=101):
```
mutant `accept_any_refusal_count` survives: the ESS gate suite passed
exit status: 0 ... test result: ok. 8 passed; 0 failed
warning: unreachable pattern --> crates/loom/tests/ess_gate.rs:100:9   (proves the mutant compiled)
mutant `validate_without_strict_requires` survives: the ESS gate suite passed
test result: FAILED. 2 passed; 2 failed
```

**3. Suite run** (after the cases existed): `cargo test --workspace --locked --no-fail-fast` → TEST_EXIT=101.
- lib: 1 passed
- `adversary_ess_gate`: 10 passed
- `ess_gate`: 8 passed
- doc-tests: 0
- `adversary2_ess_gate`: 2 passed, 2 failed

Before = 19 (my binary left out of the same run's per-binary counts). `rustfmt --check` on my file: 0. `cargo clippy -p b10x-loom --all-targets --locked -- -D warnings`: 0.

**4. Findings**

| # | file:line | what was measured | what reaches it | verdict / origin |
|---|---|---|---|---|
| F1 | `crates/loom/tests/ess_gate.rs:99` | Accepting any refusal count leaves the suite green. Its only refusing copy (`Optional<Binary64>`, :289) makes ess exit 1, so the count check is never used. | ess 0.52.0 exits 0 while refusing. Observed: a copy of `ess/` plus one command with an unsatisfiable outcome printed `1 scenario(s) (0 authored), 1 refusal(s)`, EXIT=0. The next stories in this chain add commands, so the count is the only thing that holds step 3. | CONFIRMED, warning / introduced |
| F2 | `crates/loom/tests/ess_gate.rs:78` | Dropping `--strict-requires` leaves the suite green. | Acceptance item 1 names the flag. Without it, `requires: ess 0.51.0` under ess 0.52.0 only warns and exits 0. It matters whenever someone changes `ESS_VERSION` in CI or runs a different local ess without updating `requires`. | CONFIRMED, warning / introduced |
| F3 | `AGENTS.md` § ESS | It leaves out two ADR 0076 rules: "No story ... is implemented while its gate is red" and that the marker test "is removed when the ESS release that refuses open entries is pinned". Story (f) does not require either. | Agents read AGENTS.md, not the ADR. | CONFIRMED, note / introduced |

Suggested fix (I applied neither; both belong in `ess_gate.rs`):
- **F1:** run `run_gate` over a copy of `ess/` with the unsatisfiable command (`UNSATISFIABLE_COMMAND` in my file), and expect `Step::Synthesize` with `1 refusal(s)` in the detail.
- **F2:** run `run_gate` over a copy with `requires: ess 0.51.0`, and expect `Step::Validate`.

**5. Attacked and could not break**
- **Refusal summary parse.** In ess 0.52.0 source (`main.rs` `synthesize_suite`), `refused:`, `outside:` and `note:` lines print to stdout before the summary, and the summary is always the last line. A refusal's extra lines (seen: `no candidate of the 4 tried…`) come before it. The only text after the count is `written to <--out>`, and `find` takes the first match on the line. So nothing can fool it short of a newline in the target path, which I could not make happen.
- **Scratch-key hashing.** It hashes the canonicalized path with 64-bit SipHash. Collisions are negligible. A non-UTF-8 root makes `path_arg` panic, so the gate fails rather than passes.
- **`run.yaml` vs the story.** All four resolutions match:
  - (a) header text;
  - (b) `CommissionRunId` is a newtype of Uuid, and its comment names `commission.responsibility.RunId` (checked against commission `013e392:ess/domains/responsibility.yaml:34`);
  - (c) `Optional<Decimal>`;
  - (d) `turn_id` plus `catalogue` (owns, one, via `turn_id`).
- **CI.** `.github/workflows/check.yml` installs ess 0.52.0 with SHA256 checks and runs `task check`, which lists `task: ess-gate`.
- **Mutant not pursued:** accepting a missing refusal count (`None` arm). Only an ess output-format change could reach it, and CI pins 0.52.0.

**6. Paths written outside the worktree**
- In `~/.cache/ga-wave-2026-10-04-w2/loom-ess-hard-gate/scratch/`:
  - `adv2-ess-main-0.52.0.rs`
  - `adv2-loom-refusing/`
  - `adv2-loom-refusing.suite.json`
  - `adv2-older-requires/`
  - `adv2-older-requires.suite.json`
  - `adv2-red.log`
  - `adv2-refusing/`
  - `adv2-refusing-suite.json`
  - `adv2-suite.json`
  - `adv2-suite.log`
  - `adv2-suite.log.clippy`
- Test scratch in the brief's build dir: `~/.cache/b10x-target/loom-w2-ess-hard-gate/tmp/adversary2_ess_gate/`

I also took and released a session lease on the worktree (`adv2-loom-ess-hard-gate-w2`).

**7. Findings block**
```findings
- file: crates/loom/tests/ess_gate.rs
  line: 99
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "no case runs the gate over a specification ess synthesizes with exit 0 and refusals, so removing the refusal-count check leaves the suite green"
- file: crates/loom/tests/ess_gate.rs
  line: 78
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "no case runs the gate over a requires/ess version mismatch, so dropping --strict-requires (acceptance item 1) leaves the suite green"
- file: AGENTS.md
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "§ ESS omits ADR 0076's rules that no story is implemented while the gate is red and that the marker test is removed once ESS refuses open entries"
```
