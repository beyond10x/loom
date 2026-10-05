# Confined tests qualification, 2026-10-05

Story: `story:confined-tests-run`. Scope: Rust and system tools, embedded Substrate
0.7.10 at `65304edf6ebdf4a95f9c2c6138b0c20ea47d157e`. No new Loom release.

## Specification and first failing check

Specification-only commit `6c9f384` defines `intake.confinement` and
`StopReason::ConfinementUnavailable`; `84fb170` upgrades the other two systems to ESS 0.53.0.
`ess specify validate --path ess/intake --strict-requires`:

```text
intake v1 — 3 file(s), valid
```

Before implementing the CLI, the new acceptance check ran:

```text
cargo test -p b10x-loom-cli --test loom_cli confinement_defaults -- --nocapture
confinement_defaults_to_substrate_and_requires_explicit_opt_out --- FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out
exit 101
```

The existing command had no confinement flag, cgroup-root flag, or named confinement stop.
The unresolved-import red run for the separate runner established an absent seam only; it
is not behavioural isolation evidence.

ESS synthesis of the existing systems reports 36 Loom and 13 Commission scenarios, zero
refusals. ESS notes that the existing selection-refusal scenarios cannot observe changes to
`confidence`; this change does not claim that coverage.

## Real delegated execution

The dedicated `systemd-run --user --scope -p Delegate=yes` lane executed
`real_confinement_rust_system_tools_and_cleanup`: 1 passed, 0 failed, 0 ignored,
3 filtered out. This was an executed test, not the ordinary lane's explicit prerequisite skip.

The lane checks source, Git metadata, external-file and toolchain writes fail; `target/`
writes succeed; a synthetic external credential is unreadable; inherited credential environment
variables are absent; and a network socket cannot connect. While the command runs, the host
reads its actual cgroup controls: `memory.max=8589934592`, `pids.max=2048`,
`memory.swap.max=0`, `memory.oom.group=1`, and a bounded `cpu.max`. It also verifies cgroup
retirement, timeout cleanup including a `setsid` descendant, offline Cargo builds using cached
registry (`itoa`) and Git (`b10x-llm-core`) dependencies, and an actionable error for a missing
dependency. Each denied operation has its own explicit failure assertion.

The first real attempts found two integration details, corrected before this green run:
Substrate 0.7.10 accepts four read-only roots, so the checked registry cache is one root
(only `index`, `src`, `cache`, `CACHEDIR.TAG` entries admitted); optional resource-usage
measurements need counters this kernel lacks, so they are not requested. Enforced resource
bounds are unchanged and are checked from the kernel.

## Live Codex fixture

The default CLI, with no confinement or cgroup override, ran the same one-function Rust
fixture used for earlier qualifications. Dependencies and its canonical lockfile were prepared
explicitly before starting. Models: `gpt-5.6-sol`; llm 0.1.7; operator Codex login consumed by llm.
Initial fixture commit: `c865b44d6465c4229586440f75c3e60dbff9c1bc`.
Resulting fixture commit after the adversary fix: `94f72aa968794962dd8f1d43a237de6bbd4214fc`.

```text
confinement: substrate
step 1: tests.run {}
  evidence: test_result fail
step 2: repository.inspect {"paths":["src/lib.rs","Cargo.toml","Cargo.lock"]}
  evidence: none
step 3: repository.edit
  effect: committed; HEAD is 94f72aa968794962dd8f1d43a237de6bbd4214fc
  evidence: none
step 4: tests.run {}
  effect: the test command exited with 0
    | confinement: substrate
    | test result: ok. 1 passed; 0 failed; 0 ignored
  evidence: test_result pass
stopped: ApprovalRequired (repository.merge)
exit 0
```

This excerpt abbreviates the edit payload and test output. The full transcript is
[retained beside this record](2026-10-05-confined-tests-transcript.txt).
The fixture diff is exactly `a - b` → `a + b`; test text, `Cargo.toml`, `Cargo.lock` and
`.gitignore` are unchanged, and the worktree is clean. No fixture merge or push occurred.

During the first qualified run, the parent process was observed in `session-3.scope`; the re-executed CLI was in
`user@1000.service/app.slice/run-p3466751-i418194003.scope/loom-controller-3466751`.
Thus the live run exercised the automatic user-systemd re-exec, not only an explicitly delegated
test harness. Substrate's capability probes print expected read-only-write failures before test
output; those probe diagnostics are not test evidence. After fixing the adversary finding, a
fresh copy of the initial fixture repeated the default run, again with exit 0 and the same
one-line function fix. The transcript above is that final run.

## Repository verification

`task check`, `task plan`, and `task website` pass. The gate covers strict ESS validation,
both synthesized suites, Commission conformance execution, all three Rust projection drift
checks, model/dependency guards, formatting, workspace Clippy with warnings denied, workspace
tests, and documentation drift. Final planning validation reports 101 artifacts, valid.

Before the adversary additions the ordinary intake-slice lane reported 45 passed and two ignored:
the host-Git child-process helper (executed by its parent test) and the dedicated confinement lane.
The latter was rerun after the fix: 1 passed, 0 failed, 0 ignored, 3 filtered out, 16.16 seconds.
Two existing executor scenarios remain explicitly deferred to interruption/recovery and
selection revalidation. They establish no confinement claim.

The ESS 0.53 upgrade also required the existing Run conformance interpreter to resolve
`expect_event_values`; all nine Run scenarios pass, and corrupting either the emitted identity
or literal payload fails its new mutation check. The generator bootstrap fixture now includes
the intake model dependency.

## Bounded adversary pass and repair

The pass against `5e015d9` found one executable blocker: pre-existing hardlinks under `target/`
allowed writes to read-only source, Git metadata and an external file, while the driver
reported scoped writes. The first real probe failed: 0 passed, 1 failed, exit 101; all three
synthetic victims changed. The immutable AEP record is
`review-result:confined-tests-run-adversary-1`.

The runner now counts every regular-file inode name under `target/` before launch and refuses
aliases outside that scope. Internal hardlinks remain supported for Cargo's incremental cache.
Sockets, devices and other special files are also refused. Two added unit regressions first
failed (0 passed, 2 failed), then passed with the fix.

All four adversary fixtures passed on the fixed integrated source, each in its own delegated
scope: 1 passed, 0 failed, 0 ignored, 4 filtered out per invocation. They cover external
hardlinks, explicitly seeded synthetic AWS/SSH environment values, a workspace compiler override,
and a host Unix socket under `target/`. The child-only socket helper is never selected directly.
These explicitly executed lanes, plus the full real-confinement lane, establish qualification;
the ordinary suite's ignored integration cases do not.

After integrating the repair and adversary tests, `task check`, `task plan` and `task website`
passed again. The ordinary intake-slice suite has 47 passing cases and seven explicit ignores
(the dedicated lanes and their child-only helpers). The four adversary lanes were then selected
and passed again from Cargo-built test binaries, with no skipped qualification cases.
