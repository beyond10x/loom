---
format: aep.planning-md/3
id: review-result:bound-commands-run-confined-security-2
kind: review-result
status: active
title: 'Security review, pass 2: bound-commands-run-confined (wave 2026-11-21-w6, U1, head 6c5a3466)'
relations:
- reviews: story:bound-commands-run-confined
revision: 1
---
unit: story:bound-commands-run-confined U1 pass 2, head 6c5a3466, plus my uncommitted additions to my own test file
verdict: green (1 warning: the CHANGELOG still omits the working-directory change)
cases: executed 431→432, red 0
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (the `sec2/` scratch directory: 2 scripts, 2 logs, 1 `.disk` file)
needs-coordinator: no

**1. Finding 1 is closed.**
- My case `under_local_the_timeout_holds_when_the_arguments_exceed_one_pipe_buffer` now passes unchanged. Run alone beside my new case: `test result: ok. 2 passed … finished in 6.30s`. It was 25.36 s at 200f9e64.
- **A write that fails first** still gives the answer: `engine.rs:494` returns `written.and(finish.await)`, which waits until the run ends or the deadline's `Kill` has been sent. The error path then sends a `Kill` only if none was sent (`:551`), so one at most. Substrate closes the pipe after a failed write (`stdin.rs:290`).
- **A write dropped half way** leaves nothing of Loom's holding the pipe. Substrate's `Armed` guard poisons the writer and closes the pipe when dropped (`stdin.rs:196`, `:137`). The write is a plain future that starts no task or thread. The adapter's run record keeps the writer, with its pipe closed. A drop before the guard exists would need the lock to be contended; Loom is the only writer, so it is not.
- **A run that ends by itself** before the write completes: Substrate ends the stdin writer before it publishes the run's end (`local.rs:330-339`). The pending write then fails as `write-failed`, which `write_all` treats as a normal stop, so the step is the program's own outcome. My new case `under_local_a_program_that_reads_none_of_its_large_arguments_ends_as_itself` is green; it covers both exit 0 with a report and exit 3 with "ended exit status: 3".
- **One `Kill` per run:**
  - `:1327` asserts one `Kill` and a return under `KILL_SETTLE`. It goes red when the change is reverted (the implementor's red run shows the hang).
  - `:1360` passes no stdin, so it does not depend on the `select!`. By reading, it goes red if the flag at `:551` is removed (two `Kill`s).
- `:1205`, `:1229` and `:1255` are unchanged.

**2. The `linux` claim holds.** At the exec timeout Substrate kills every process in the run's cgroup (`process.rs:3445-3448`). A process that called `setsid` is still in the cgroup, so every reader of the pipe dies. The pending write then fails `write-failed` (`process.rs:1553`). `confinement.rs:784-794` maps that to Ok once the run is terminal, and the earlier `Kill` at `:765-775` guarantees it is.

**3. Holds.** `:1360` pins what the deleted test held: a refusal (`CapabilityUnserved`), no "killed" in the error, one `Kill` and no settle. In the step, an `Err` prints "was refused by its runner" (`bindings.rs:497`). It also makes my pass-1 note 3 (no retry of a failed `Kill`) a pinned behaviour.

**4. The four new bullets are each true** at `main.rs:920`, `:1028` and `:1164`, at `engine.rs:556-563`, and at `bindings.rs:497` and `:503`.
- The implementor is right and my pass-1 wording was wrong. The base already printed "was killed" for output held inside the group: its finish loop never reaps the leader before the group kill (base `bindings.rs:565-611`).
- One caller-visible change is still not stated (finding below).

**5. Holds.** `bindings.yaml:14-17` and its copies now name the cgroup kill under `linux`, the process-group kill under `local`, and the refusal when a process that left the group still holds the output. That matches `local.rs:17-23` and `engine.rs:657-666`.

**6. Holds.** The only change to my file between b7154fab and 6c5a3466 is a `cargo fmt` reflow at `:100`; no assertion changed. This pass I added one case (30 lines).

**Finding**
- **CHANGELOG still omits the working-directory change** (warning, CONFIRMED, introduced; I found it by reading, no test).
  - A bound program now starts in `--workspace` or in a private temporary directory that is removed after the run (`bindings.rs:300`, `:339-342`). On the base it started in the directory `b10x-loom` ran in (base `bindings.rs:415`, no `current_dir`).
  - So a relative argv like `./record-note` no longer resolves from that directory, and files the program writes by relative path land in the private directory and are lost. The guide's example was quietly changed to `/opt/...` and its `>> notes.log` line was dropped.
  - The step text for a program that cannot be found also changed, from "did not start: …" to "was refused by its runner: …". That is not stated either.

**Not established:** I did not run the `linux` path, since it needs a delegated runner. Point 2 rests on reading the code.

**Suite:** `cargo test -p b10x-loom-intake-slice --locked --no-fail-fast`: exit 0, 71 binaries, 432 passed, 0 failed, 4 ignored.

**`git status --short`:** ` M crates/loom-intake-slice/tests/security_w20261121w6_u1.rs`, a test file only (+30 lines, the one new case).

```findings
- file: CHANGELOG.md
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Not stated: bound programs now start in --workspace or a private temp directory, not b10x-loom's own directory (bindings.rs:300, :339-342), so a relative argv or relative output path no longer resolves there; a missing program now reads 'refused by its runner'."
```
