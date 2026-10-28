---
format: aep.planning-md/3
id: review-result:adversary-w20261028w1-u2-pass-1
kind: review-result
status: active
title: Wave 2026-10-28-w1 adversary, loom story:session-executor-partial-runspec, pass 1
relations:
- reviews: story:session-executor-partial-runspec
revision: 1
---
Adversary pass 1 on u2 (`aep:adversary`, opus), wave 2026-10-28-w1, tree `loomd163-a2` at `aa3576670`
on integration head `4b0b50fb8`. The report as it returned:

unit: story:session-executor-partial-runspec, covering branch adv/d163-a2 at aa3576670 (fork of 4b0b50fb8; unit merge 4bbd2841f)
verdict: red
cases: executed 12→17, red 1
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 5 paths (listed in part 6)
needs-coordinator: no

1. **Diff**: one test file, `crates/loom-session-executor/tests/adversary_w20261028w1_a2.rs | 527 +++`.

2. **Cases**, committed by the bot as aa3576670.
- `a_named_file_the_copy_does_not_hold_is_refused_as_context_before_the_copy` is **red**. A context path that is listed in `files` but that the copy leaves out passes `SessionOverrides::uncopied`. The three paths tested are a link, a path under a linked directory, and a missing file. AGENTS.md:1213 says such a path is refused "before any copy". What happens instead is that the copy is made and metaharness refuses the path, for all 9 (3 paths x 3 kinds), for example:
  `B10x src/absent.md (absent from the workspace): copy made: true; refusal: metaharness refused the session: --context src/absent.md is refused: it could not be read: No such file or directory (os error 2)`
- `the_scratch_probe_reads_empty_after_a_copy_was_made` is green. It proves that the unit's `no_copy_made` check (partial_runspec.rs:129) can never fail: after a refusal that comes after the copy, `report.cwd` is `Some` but the scratch directory is empty, because the copy deletes itself when dropped.
- Green when first run: `unplain_context_spellings_are_refused_before_the_copy`, `the_budget_renders_exactly_at_its_bounds`.
- `an_effort_cannot_inject_config_or_flags` failed on its first run on the adversary's own wrong prediction: metaharness H8 refuses `--dangerously-skip-permissions` even as the value of `--effort`; the case now expects that refusal and is green.

3. **Suite**: `cargo test -p b10x-loom-session-executor --locked --no-fail-fast --test partial_runspec --test from_spec --test adversary_w20261028w1_a2` exited 101: partial_runspec 5/5, from_spec 7/7, adversary 4 passed, 1 failed.

4. **Findings**
- Context check only looks at names (`lib.rs:379`, `uncopied`): a link, a path under a linked directory or a missing file gets through and is refused by metaharness after the copy; `from_spec` builds although AGENTS.md says `SpecError`. The step still suspends and never starts: the documented promise breaks, not safety. NEEDS-CHANGE, introduced. What reaches it: a caller naming a file it wants the session to create, or one reached through a link; no consumer does yet (takt is blocked on this story).
- A check in the unit's tests can never fail (`partial_runspec.rs:129`, `no_copy_made`); better probe `last_report().cwd.is_none()`. CONFIRMED, introduced.

5. **Tried and could not break**: budget rendering at 0, 999999, 1000000, 1000001, u64::MAX; from a spec i64::MAX builds and i64::MIN gets `BudgetNegative`; TOML syntax and newlines in a Codex effort stay one quoted value; a flag as a Claude Code effort is refused by H8; context `.git/HEAD`, `src/.git`, a trailing `/`, `..` and a NUL byte are refused before the copy; `SessionOverrides` has no field for anything Loom guarantees.

6. **Files written outside the worktree**: under `~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/loom-d163/a2/`: `commit-msg.txt`, `red-alone.log`, `red-alone-2.log`, `suite.log`, `suite-2.log`.

```findings
- file: crates/loom-session-executor/src/lib.rs
  line: 379
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: uncopied checks only that a context path is plain and named in files, so a linked, link-reached or absent named file passes, the copy is made and metaharness refuses it, against AGENTS.md:1213's Loom refusal before any copy (and from_spec builds instead of a SpecError)
- file: crates/loom-session-executor/tests/partial_runspec.rs
  line: 129
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: no_copy_made reads the scratch directory after the step, which is empty even when a copy was made because ScratchCopy removes itself on drop, so the "before the copy" assertions it backs cannot fail
```
