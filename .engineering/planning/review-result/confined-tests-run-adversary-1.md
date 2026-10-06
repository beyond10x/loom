---
format: aep.planning-md/3
id: review-result:confined-tests-run-adversary-1
kind: review-result
status: active
title: 'Bounded confinement adversary: external hardlink aliases'
relations:
- reviews: story:confined-tests-run
revision: 1
---
unit: story:confined-tests-run at 5e015d99d5eb7094d4f77d89cdd9edc2c3aaa546; subsequent fix replay explicitly separated
verdict: CONFIRMED
cases: executed 0→4, red 1 at baseline / 0 after coordinator fix
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: none
needs-coordinator: incorporate tests and record final full checks against fixed source

1. Tests-only diff

```text
 .../tests/adversary_confinement.rs                 | 192 +++++++++++++++++++++
 1 file changed, 192 insertions(+)
```

The counts above describe this bounded adversary suite. The coordinator reported its existing slice suite at 45 passed and 2 ignored before this pass; I did not rerun that full suite. Four new substantive cases were executed, each alone in a dedicated delegated scope. The fifth test is a child-only socket client; it is not a standalone qualification case.

2. Added cases and first executable finding

All additions are in `crates/loom-intake-slice/tests/adversary_confinement.rs`; exact diff is `tests.patch` in this report's directory.

- `target_hardlinks_cannot_mutate_source_git_or_external_files`: initially RED against baseline library, then GREEN against coordinator's updated library. Creates three ordinary hardlinks under target pointing at source, Git configuration and an external synthetic file. A confined shell overwrote all three through the aliases while the driver returned exit 0 and scoped target-only confinement.
- `inherited_synthetic_credentials_are_cleared`: GREEN. The launching process explicitly has synthetic AWS_ACCESS_KEY_ID and SSH_AUTH_SOCK values, and asserts they are absent inside the sandbox. No real credential is read.
- `workspace_rust_toolchain_override_never_executes_on_the_host`: GREEN. A rust-toolchain.toml absolute path selects a workspace-local synthetic compiler. ScopeInvalid is returned and the compiler's host marker is absent.
- `target_socket_cannot_reach_an_unconfined_host_service`: GREEN against the coordinator fix. A synthetic host Unix listener is placed under target, and the runner refuses it before the confined test client can connect. This is additional regression coverage, not a measured baseline finding.

Verbatim red assertion and test summary from `hardlinks-red.log`:

```text
assertion `left == right` failed: target aliases bypassed the promised source/Git/external write boundary
  left: ["changed", "changed", "changed"]
 right: ["unchanged", "unchanged", "unchanged"]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test target_hardlinks_cannot_mutate_source_git_or_external_files ... FAILED

failures:

failures:
    target_hardlinks_cannot_mutate_source_git_or_external_files

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

The complete original output, including the actual driver record, remains in `hardlinks-red.log`. The test binary exited 101. An initial compile selected a stale rlib and did not compile; the first fixture revision contained `..` and was correctly refused, so its path was canonicalized before the recorded red. Neither setup issue is a finding. The initial toolchain probe used a relative override, which rustup itself refused; the final probe uses an absolute workspace-local override. The socket probe initially exceeded Unix sockaddr path length; binding through a directory descriptor fixed fixture setup.

3. Post-fix bounded suite

The parent rebuilt its rlib while this pass was running. The final binary therefore linked its updated `validate_writable_artifacts` implementation, which rejects external inode aliases and non-artifact file types. No production source was edited in this adversary checkout. This replay does not claim to be a second baseline run.

Build: `rustc --edition=2024 --test crates/loom-intake-slice/tests/adversary_confinement.rs` with explicit `b10x_loom_intake_slice` and `tempfile` externs and read-only access to the coordinator's existing dependency rlibs. The binary is in this report's directory; no build cache was written outside this worktree.

For each of the four substantive test names above, the command was:

```console
AWS_ACCESS_KEY_ID=loom-synthetic-aws SSH_AUTH_SOCK=/synthetic/loom-agent systemd-run --user --scope -p Delegate=yes .engineering/drafts/confinement-adversary/adversary-confinement --ignored --nocapture TEST_NAME
```

Each command exited 0 and printed, verbatim:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

The exact complete output, including elapsed times and refusal details, is `suite-after-fix.log`. The two successful launches also print Substrate's expected read-only capability-probe errors. These are probe output, not evidence of a failed test. The socket helper must only be selected by the socket fixture; do not run this binary with an unfiltered `--ignored`.

4. Findings

| File:line | Verdict / origin | What was measured | What reaches it |
|---|---|---|---|
| `crates/loom-intake-slice/src/confinement.rs:312` | CONFIRMED / introduced | Pre-existing target hardlinks bypass the promised source, Git and external write boundary while the driver reports target-only confinement. The test overwrote all three synthetic victims and failed, exit 101. | Default SubstrateRunner::run reaches validate_workspace for every test. A real workspace with target aliases from an earlier host build or copy is admitted; the shell command then writes ordinary artifact paths. No privileged API or dishonest runner is needed. |

This finding covers baseline 5e015d9. The coordinator's updated guard makes the replay pass. Fix publication and the full-suite qualification remain the coordinator's work. No additional judgement-only findings.

5. Boundaries not broken

- Explicitly seeded environment credentials are absent inside confined execution.
- A workspace compiler override is refused without executing workspace code on the host.
- The updated writable-artifact guard refuses a target Unix socket before connection.
- Existing code paths for refusal observations, explicit unconfined mode, and re-exec guards were inspected; no new executable finding was produced for them in this pass.

6. Paths outside the worktree

None. Existing coordinator dependency rlibs were read only. All test source, executable, logs and synthetic fixtures were written within the assigned worktree. Fixtures are TempDirs and removed themselves. The managed checkout and retained scratch are handed back to the coordinator.

7. Machine-readable findings

```findings
- file: crates/loom-intake-slice/src/confinement.rs
  line: 312
  category: acceptance
  severity: blocker
  verdict: CONFIRMED
  origin: introduced
  message: Pre-existing target hardlinks bypass the promised source, Git and external write boundary while the driver reports target-only confinement.
```
