---
format: aep.planning-md/3
id: review-result:bound-commands-run-confined-security-1
kind: review-result
status: active
title: 'Security review, pass 1: bound-commands-run-confined (wave 2026-11-21-w6, U1, head 200f9e64)'
relations:
- reviews: story:bound-commands-run-confined
revision: 1
---
unit: story:bound-commands-run-confined U1, commit 200f9e64 plus my one untracked test file
verdict: red (2 warnings, no blocker)
cases: executed 426→429, red 1
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (the `sec1/` scratch directory)
needs-coordinator: yes. Finding 1 needs a fix in `engine.rs` or a decision to accept it.

**1. Diff.** `git diff --stat` is empty. `git status --short` shows only `?? crates/loom-intake-slice/tests/security_w20261121w6_u1.rs`, a test file.

**2. Cases** (all in `crates/loom-intake-slice/tests/security_w20261121w6_u1.rs`)
- `under_local_the_timeout_holds_when_the_arguments_exceed_one_pipe_buffer`: **red**. The run alone gave: `the step took 25.362446273s for a timeout of 1 s (bound 12s): the run waited for a process outside the command's group to let go of its standard input`. My first run of it failed only on the model's 64 KiB argument limit, so I changed the arguments to backspace characters; that run is not a finding.
- `a_bound_programs_argv_is_the_bindings_alone`: green. Nothing the model chose reaches argv.
- `a_linux_runner_that_cannot_be_served_is_refused_and_never_falls_back_to_local`: green.

**3. Suite and gates**
- `cargo test -p b10x-loom-intake-slice --locked --no-fail-fast`: exit 101. 71 binaries, 428 passed, 1 failed (only the case above), 4 ignored. `adversary_w20261021w6_u3` passed 4 of 4 in 7.23s.
- The gates the implementor could not run all exited 0: `slot task no-hand-model`, `slot task docs-check`, `cargo clippy --workspace --all-targets -- -D warnings` and `slot task consumer-check` (12 passed).

**Invariants**
1. **Holds.** A bound action starts only one way: `bindings.rs:494` → `CommandRunner::run` (`:358`) → `TestRunner::run_child`. The only constructor is `intent.rs:982` / `run.rs:671`, and `CommandRunner` is `pub(crate)`. Process starts left in the two crates:
   - `git.rs:276` and `executor.rs:1126-1132`: git.
   - `executor.rs:1291/1317`: the test command of `UnconfinedRunner`.
   - `executor.rs:1769`: inside the `#[cfg(test)]` module.
   - `confinement.rs:3269`: `rustup which rustc`.
   - `main.rs:1498`: the `systemd-run` re-exec.
   - None of them is reachable from a bound action.
2. **Holds.** The refusal comes at `intent.rs:219-229`, before the classifier. `select_runner` refuses `NoRunner` before the private directory is created, and no case is opened. Test: `bindings_and_no_runner_are_refused_no_runner_before_any_process_or_model_call`.
3. **Holds.**
   - The environment is the host process's `PATH` alone (`bindings.rs:328-334`). Under `local` it is built from that alone (`engine.rs:360-375`), and Substrate clears the rest (`local.rs:268`).
   - Arguments go on stdin only (`:346`); my argv case confirms it.
   - The working directory is `--workspace` or a private `loom-bound-*/workspace`. Under `linux` only `target/` is writable; under `local` nothing is confined.
4. **Holds.**
   - `bindings.rs:227-255` and `main.rs:1020-1029` / `:1143-1167` decide the runner. `decode` refuses a file that names a reason.
   - The only environment variable involved, `SUBSTRATE_CONFINEMENT_LOCAL`, can only forbid `local` (`local.rs:85-90`).
   - A `linux` runner that is not served is refused (my green case).
   - An SDK host that opens `local` with `UNCONFINED_FLAG_REASON` counts as the flag. That is documented in `embed-the-runtime.md`.
5. **Partly holds.**
   - The `RunId` exists only inside `launch`'s `block_on` (`engine.rs:475`). Loom keeps no copy, and a run with recorded output has no live group kill.
   - One `Kill` holds on the timeout, a failed wait and a failed write (`engine.rs:1205/1229/1255`).
   - A failed output read (`:502`) sends no `Kill`, but it can only happen after the run ended. That line is unchanged from the base.
   - The step text holds (`bound_command_confined.rs:265`).
   - Not holding: inside the joined write and wait, a stalled stdin write keeps `launch` from returning (finding 1).
6. **Holds for 5 of the 6 deleted tests**, mapped as the implementor listed. The exception is `a_group_that_could_not_be_killed…`: a `Kill` that fails is now pinned by no test. By code structure it becomes a runner refusal and is never reported as "killed" (`engine.rs:652-656`).
   - The caller-visible change: when a process in the same group holds the output, the step now reads "was killed after N s" and that process is killed. The Acceptance only implies it ("kill of the whole process group"); the CHANGELOG does not state it (finding 2).
7. **Holds.** The only change under `tests/` is the new file. `engine.rs:1205` only catches a doubled settle (it bounds the run below 10 s). The adversary test runs about 7.2 s against its helper's 10 s, so it can fail under heavy load, but it is not wrong.
8. **Holds.** 9a029bcf changes only `ess/intake/` (2 files). Generated files differ only in their digest lines, the `NoRunner` variant and the summary. No `UNMAPPED:`, no `/home/<name>/` literal in the diff. One gap: the `bindings.yaml` summary says commands are "killed with the processes they started", which overstates `local`.

**Findings**
1. **Timeout lifted under `local`** (warning, CONFIRMED, introduced).
   - Measured: `engine.rs:484-488`. `tokio::join!` waits for `write_all` even after `finish` has given up; the step took 25.4 s against a 1 s timeout.
   - What reaches it: the run must be under `--unconfined-bindings` or the file's opt-in. The bound program leaves a `setsid` process holding its stdin and stdout and exits. The model's arguments must be larger than one pipe buffer on stdin: `json_text` writes each control character as `\u00XX` (6 bytes) while the model's limit counts serde's `\b` (2 bytes), so about 64 KiB becomes about 190 KiB.
   - Why introduced: the base wrote stdin on a detached thread (old `bindings.rs:387-391`), so the run returned at the deadline.
   - Possible fix (not applied): drop the pending write once `finish` returns an error.
2. **CHANGELOG incomplete** (warning, CONFIRMED, introduced). Caller-visible changes it does not state:
   - `run --action-bindings` without an opt-in now runs under `linux` instead of being refused exit 3.
   - `--unconfined-bindings` beside `--state` is now refused, exit 2.
   - The step texts changed: same-group held output now reads "was killed", and a program ended by a signal now reads "refused by its runner: CapabilityUnserved".
   - Each step now prints a `confinement:` line.
3. **Note** (INFEASIBLE, introduced). `engine.rs:543` sets the flag before the `Kill` is sent. If sending fails, no `Kill` goes out and the error path does not retry. Under `local` sending only fails for an unknown run or a `kill` error other than `ESRCH` (`local.rs:178-185`, `:442-450`). I found no caller that reaches either.

**What I could not settle:** finding 1 under the `linux` driver, where stdin goes through a different path (`launch_with_stdin`). I did not run any of my cases against the base.

**Paths written outside the worktree:** `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w6/u1/sec1/`, holding `run-case.sh`, `run-suite.sh`, `run-full.sh`, `summary.txt`, the `case-1`, `case-2`, `slice-suite`, `slice-full`, `no-hand-model`, `docs-check`, `clippy-workspace` and `consumer-check` logs, and the `.disk` files.

```findings
- file: crates/loom-intake-slice/src/confinement/engine.rs
  line: 484
  category: concurrency
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Under local, a stdin write stalled by a process outside the group keeps tokio::join! from returning, so a 1 s timeout took 25.4 s."
- file: CHANGELOG.md
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Not stated: bindings run under linux without an opt-in instead of being refused; --unconfined-bindings with --state exits 2; step texts changed; a per-step confinement line was added."
- file: crates/loom-intake-slice/src/confinement/engine.rs
  line: 543
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "The flag is set before the deadline Kill is sent, so a failed send goes unretried; under local that only happens for an unknown run or a non-ESRCH kill error, which no caller was shown to reach."
```
