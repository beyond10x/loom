---
format: aep.planning-md/3
id: review-result:confinement-opens-from-engine-config-security-2
kind: review-result
status: active
title: Security review pass 2 (last) on story:confinement-opens-from-engine-config (wave 2026-11-21-w4, unit U1, head 37de3e91)
relations:
- reviews: story:confinement-opens-from-engine-config
revision: 1
---
unit: U1 `story:confinement-opens-from-engine-config`, pass 2. The findings cover head `37de3e91` plus my one untracked file.
verdict: red. 3 of my new cases fail. No blocker; the strongest is a warning.
cases: executed 410→414, red 3. The 410 is the implementor's slice count on `37de3e91`. The 414 is the package suite after my file existed: 411 passed, 3 failed, 4 ignored.
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path, the scratch directory `…/u1/sec2/`
needs-coordinator: no

`git diff --stat` is empty: no tracked file changed. My new file is `crates/loom-intake-slice/tests/security_w20261121w4_u1_pass2.rs` with 4 cases, run alone first (`p1`, exit 101). The suite run (`s1`, exit 101) fails only that binary: `` error: 1 target failed: `-p b10x-loom-intake-slice --test security_w20261121w4_u1_pass2` ``. All 11 of my pass-1 cases pass.

1. **F1, printed lines: answered.**
   - Every `confinement:` line goes through the runner's label: `run.rs:473`, `intent.rs:722,871`, and `executor.rs:416` via `applied_label`.
   - The remaining `backend_name` sites only hold a backend value, never a printed line: `executor.rs:1225` (the observation JSON also carries the adapter and the reason), `resume.rs:439` and `:587`, and the conformance rows.
   - linux and none print what they did at `63668a97`: the implementor added cases for both, and `selector_executor.rs:127` is unchanged.
   - The agent document's `confinement` field (`agent.rs:529`) admits only `none` and is unchanged.
2. **F2, resume: answered for `EngineRunner`, not for a wrapper.**
   - `HostText::of` (`intent.rs:399`) refuses with `SliceError::State` (a Rust-only variant, no ESS change). Both `ResumeHost::check` and `keep` (`:479`) go through it; `keep` is the only place a context is stored.
   - The command line's `--state` refusal (`main.rs:915`) is unchanged and still answers first.
   - A hand-made context can only name `Substrate` or `None` (`resume.rs:509`), so `local` cannot be expressed in one.
   - Finding A: my red case `…a_wrapped_local_runner_is_not_kept_for_resume`, output line: `a runner that launches through local but does not forward TestRunner::local_reason (its default answers None) passes ResumeHost::check …`. The workspace has no such wrapper: the only `impl TestRunner` in source are `SubstrateRunner`, `EngineRunner` and `UnconfinedRunner`. The SDK stability page (`:103`, `:134`) lists `TestRunner` but says nothing about `local_reason`.
3. **F3, the escape: the brief's list is complete; two gaps outside it.**
   - Green: `…every_line_break_and_bidi_control_is_escaped_in_label_and_record` covers U+000B, U+000C, U+001C to U+001E, U+0085, U+2028, U+2029, U+202A to U+202E, U+2066 to U+2069, U+061C, U+200E and U+200F. The label, `local_label` and the record all agree. `printable` is unchanged.
   - Finding B, red: `invisible characters reach the label, the record and the model's briefing unescaped: ['\u{200b}', '\u{200d}', '\u{2060}', '\u{feff}', '\u{e0041}']`.
   - Finding C, red: the record now keeps only the escaped text, and a backslash is not escaped, so `left: "a\\u{a}b" right: "a\\u{a}b"`: a reason with a typed `\u{a}` and one with a real newline are recorded identically.
4. **F4, the scan: answered and proven.**
   - `sec2/scan-probe.rs` copies both scans verbatim and runs them, through `slot rustc`, against the real tree and against planted copies held in memory.
   - The tree as committed: green.
   - Red on every planted second open point: imported `open`, an aliased import, an aliased import of `open`, a re-export, `c::open`, a second module building `AdapterRequest::Local`, and a module-level alias after `impl EngineRunner`.
   - The implementor's named gap is not reachable in valid Rust: a `use` cannot stand inside an impl block, so a module-level one is attributed to `child_path` and goes red.
   - The scan reads only `crates/*/src`, so its own test file is out of its match.
   - Outside the story: Substrate's public `oci::open_adapter` and `pod::open_pod` take no `AdapterRequest`. Neither is called anywhere.
5. **What the correction could have broken: nothing found.**
   - No test line was removed except 3 import lines in `engine_confinement.rs`.
   - The committed copy of my pass-1 file equals the delivered copy once whitespace is ignored, apart from 3 trailing commas.
   - `confinement.rs` gained only `local_reason`, `local_label` and `escape_reason`; `confinement/child.rs` did not change. The linux route is untouched.
6. **ESS and generated files:** the correction touched none.
7. **Prose: answered.** All three files gained the "unset permits, the flag and the reason are the whole opt-in" sentence (`AGENTS.md`, `run-an-intent.md`, `governor-and-intake.md`); no other sentence changed. The CHANGELOG has one Added bullet. A defaulted trait method is not a semver break.

**Could not establish:** whether a library host that skips `ResumeHost::check` gets case files written before `keep` refuses (by reading, the governor opens the case first; I did not test it). I did not rerun intake-drift or docs-check, since there was no ESS change.

**Paths written outside the worktree** (all in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u1/sec2/`): `diff-src.txt`, `a.txt`, `b.txt`, `scan-probe.rs`, `scan-probe`, `probe-build.log`, `probe.log`, `run.sh`, `p1.*`, `s1.*`.

`git status --short`: `?? crates/loom-intake-slice/tests/security_w20261121w4_u1_pass2.rs`

```findings
- file: crates/loom-intake-slice/src/confinement.rs
  line: 89
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "the resume refusal rests on the defaulted TestRunner::local_reason, so a host runner wrapping a local EngineRunner that does not forward it passes ResumeHost::check and its run is kept as Backend::None; nothing in the workspace wraps one and the SDK stability page does not state the duty"
- file: crates/loom-intake-slice/src/confinement.rs
  line: 105
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "escape_reason leaves zero-width characters and tag characters (U+200B, U+200D, U+2060, U+FEFF, U+E0041) unescaped in the label, the record and the model's briefing"
- file: crates/loom-intake-slice/src/confinement/engine.rs
  line: 296
  category: integrity
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the record keeps only the escaped reason and the escape leaves a backslash as is, so a typed \\u{a} and a real newline record as the same text"
```
