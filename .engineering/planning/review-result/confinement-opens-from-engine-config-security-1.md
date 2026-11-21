---
format: aep.planning-md/3
id: review-result:confinement-opens-from-engine-config-security-1
kind: review-result
status: active
title: Security review pass 1 on story:confinement-opens-from-engine-config (wave 2026-11-21-w4, unit U1, head 3cc79cbd)
relations:
- reviews: story:confinement-opens-from-engine-config
revision: 1
---
unit: story:confinement-opens-from-engine-config (U1). The findings cover head `3cc79cbd` plus my one untracked test file.
verdict: red. 3 of my cases fail. None is a blocker; the strongest is a warning.
cases: executed 394→405, red 3. The 394 is the implementor's intake-slice count (g3.log: 394 passed, 4 ignored). The 405 is the package suite run after my 11 cases existed: 402 passed, 3 failed, 4 ignored.
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path, the scratch directory `…/u1/sec1/`
needs-coordinator: no

**What I touched.** `git diff --stat` is empty. `git status --short` shows only `?? crates/loom-intake-slice/tests/security_w20261121w4_u1.rs`. No implementation file was changed, not even temporarily.

**My cases.** All are in that file and run alone first (`m1`, `m2`, exit 101). 8 pass:
- linux with a reason, or local with a `cgroup_root`, is refused `ScopeInvalid`.
- Reasons with no letter or digit are refused `LocalReasonMissing`: NUL, `\n`, U+200B, U+2028, U+FEFF, `---`, U+3164.
- The host policy, in a separate child process per value: `""`, `PERMIT`, `permit `, `permit\n` and `allow` refuse; `permit` opens.
- The label escapes `\n`, `\r`, ESC, U+0085 and NUL.
- A local child receives none of Loom's environment (run_child: only `PATH=/usr/bin:/bin` and the variable the caller set; `run`: only the 8 variables the story lists).
- Credential-named variables (`API_TOKEN`, `HTTPS_PROXY` and others) are refused before launch.
- A live group that ignores SIGTERM is killed after the 5 s grace, and its group is gone.
- A grandchild that calls `setsid` makes the run come back refused `CapabilityUnserved` ("left the group") in under 20 s, with that process still alive.

The 3 red cases, output verbatim from the run alone:
- `…the_slice_run_line_names_local_and_its_reason`: `left: ["confinement: none"]  right: ["confinement: local (x)"]`
- `…the_library_refuses_to_keep_a_local_run_for_resume`: `a resume context of a local run would be kept as None, and a resume would reopen it under another adapter with no reason`
- `…the_label_breaks_no_line_on_a_unicode_separator`: `"local (x\u{2028}confinement: substrate\u{2029})"`

**Suite.** `cargo test -p b10x-loom-intake-slice --locked --no-fail-fast`, exit 101. Only my 3 cases failed: `error: 1 target failed: -p b10x-loom-intake-slice --test security_w20261121w4_u1`. The adversary port flake from the brief did not come up.

**Invariants**
1. Holds. `engine.rs:90-113` has no fallback branch, and `adapter` is a required field with no Deserialize. Shown by `…a_reason_beside_linux_is_refused_and_never_opens_local` and the implementor's `an_undelegated_root_is_refused_and_nothing_falls_back`.
2. Holds. Shown by `…reasons_without_a_word_are_missing` and `…only_an_unset_or_exact_permit_policy_opens_local`. Substrate permits `local` when `SUBSTRATE_CONFINEMENT_LOCAL` is unset (`local.rs:85-90`), so for a host that sets nothing the policy check is a pass. That is Substrate's design.
3. Holds by reading. The only code that builds `EngineConfinement` is `main.rs:1177`, reached from the `--confinement local` flag; nothing reads it from an environment variable, a protocol, a bindings document or a stored file. The generated type has no serde.
4. Partly. All four launches record adapter, reason and `host_network` (the implementor's tests). Not every `confinement:` line says local, though: see finding 1.
5. Holds. Egress, the certificate bundle, read-only roots, read-only files and host paths are refused (`engine.rs:329-346`). Writable scopes and the no-network default are dropped, as the story states, and the record says `unconfined`. The child's environment is only what Loom passes (`engine.rs:355-370`, `:684-705`). Substrate clears the environment and refuses names that look like credentials or proxies.
6. Holds within the group. Shown by the implementor's 4 deadline tests and my SIGTERM case. A process that left the group survives the kill; the run is then refused after `KILL_SETTLE`, which a second kill makes about 11 s in total. My `setsid` case shows this. It matches the story's "none that left it".
7. CLI holds (`loom_cli.rs:82`); library does not, see finding 2.
8. Holds. The changed lines in `confined_child.rs`, `security_w20261121w1_u1.rs` and `confinement.rs` only add `.expect` or `Some(...)`. No deleted case, no `#[ignore]`, and `Cargo.lock`, the manifests, `.engineering/`, `ess/system.yaml`, `ess/domains/` and `ess/commission/` are unchanged.

**Findings**
1. warning, NEEDS-CHANGE, introduced, acceptance. `run.rs:477` prints `confinement: none` for a local runner; `executor.rs:417` (`TestRun::summary`, which the model's briefing quotes) does the same. Both use `backend_name` instead of `TestRunner::label`. Reached by `run::run_with_options`, which is public in intake-slice but not used by the CLI. Fix: use `runner.label()` in both places.
2. warning, INFEASIBLE, introduced, acceptance. `intent.rs:797` stores `host.confinement` without checking the runner. `ResumeHost::check` (`:379`) accepts a local runner, so `run_prepared_intent_with_state` keeps a resume context that `resume` reopens as `UnconfinedRunner` or `SubstrateRunner`. Reached only by a library host: the CLI refuses at `main.rs:915`, and the SDK does not export this function.
3. note, CONFIRMED, introduced, boundary. `escape_controls` (`engine.rs:205`) lets U+2028 and U+2029 through. Readers that split lines on Unicode separators (Python's `splitlines`, JavaScript) then see a forged `confinement:` line; Rust `lines()` does not split there. `printable` behaves the same.
4. note, CONFIRMED, introduced, judgement. `engine_confinement.rs:297` matches only the literal `confinement::open(`. A second open point written as `use substrate_host::confinement::open; open(AdapterRequest::Local(..))` would leave it green. A scan for every `AdapterRequest` outside `EngineRunner::open` would close the gap.

**Implementor decisions, checked by reading and cleared:**
- The CLI still builds `SubstrateRunner` directly; the flag decides on every path, and only `local` reaches `open`.
- The baseline values copied into Loom equal Substrate's `baseline_value`; only drift is a risk.
- Backend `None` does make a local run look like `none` to a reader of `backend` alone, as the story intends; the adapter and reason are in the observation JSON (`executor.rs:1236`), and resume only sees local through finding 2.
- `ScopeInvalid` at `engine.rs:87-89` is right.
- `--local-reason` without `local` gives exit 2, and `none` prints its deprecation.
- No test leaks the policy variable: both policy tests set it on a child process only.
- Every reader of `driver_record` handles `None`.
- The CHANGELOG has its `**Breaking:**` lines, and the SDK stability page is accurate.

**Could not establish:** whether the generated code still matches a fresh generation (I did not rerun intake-drift; the implementor's g6 exited 0). The delegated `linux` cases are gated and did not run here.

**Paths written outside the worktree** (all in `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u1/sec1/`): `story.md`, `diff-src.txt`, `run-mine.sh`, `run-suite.sh`, `m1.*`, `m2.*`, `s1.*`.

`git status --short`: `?? crates/loom-intake-slice/tests/security_w20261121w4_u1.rs`

```findings
- file: crates/loom-intake-slice/src/run.rs
  line: 477
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "the slice run line and TestRun::summary (executor.rs:417) print \"confinement: none\" for a local EngineRunner instead of its label"
- file: crates/loom-intake-slice/src/intent.rs
  line: 797
  category: acceptance
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "a library host can keep a resume context for a local runner under Backend None or Substrate, so resume reopens it under another adapter; only the CLI refuses"
- file: crates/loom-intake-slice/src/confinement/engine.rs
  line: 205
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "escape_controls lets U+2028 and U+2029 into the confinement line, so a reader splitting on Unicode line breaks sees a forged line"
- file: crates/loom-intake-slice/tests/engine_confinement.rs
  line: 297
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the single-open-point scan matches only the literal confinement::open( and misses an imported open"
```
