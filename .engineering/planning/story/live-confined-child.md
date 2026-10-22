---
format: aep.planning-md/3
id: story:live-confined-child
kind: story
status: active
title: A host runs a confined child live, with standard input and both output streams whole
summary: spawn_child over a Substrate pipe session; stdin; whole stdout and stderr (takt G1-G3)
relations:
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T00:08:44Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T00:08:44Z", actor: "human:timo", revision: 4}
---
## Outcome

A host runs a confined child live: it starts the child, writes to its standard input, reads its
standard output and standard error as they arrive, signals it and waits for its exit. A child run to
its end (`run_child`) gets one-shot standard input and returns both output streams whole and apart.
Closes gaps G1, G2 and G3 of takt's `dependency-blocker:loom-child-confinement`.

## Why

On 0.32.0 (`a238395b`, `crates/loom-intake-slice/src/confinement.rs`):

- G1: `run_child` starts the child with `wait: true` (`:892`) and returns only its result.
  metaharness reads lines from, writes to, kills and waits on a live child; git's
  `cat-file --batch` reader is long-lived.
- G2: `ChildCommand` has no standard input (`:663-674`); Substrate gives a plain exec
  `Stdio::null()`.
- G3: the result is the last 8192 bytes of standard output, then of standard error, merged into one
  lossily decoded string (`:557-561`, `tail()` at `:1370`). Substrate keeps both streams whole and
  apart (`ExecObservation { stdout, stderr, stdout_truncated, stderr_truncated }`); the tail is
  Loom's.

Substrate 0.7.18, as pinned, offers a live child only as a raw-pipe session
(`substrate_host::Driver::start_pipe_session`, `write_pipe_session`, `read_pipe_session`,
`close_pipe_session_input`, `signal`, `observe_exec`): frames of at most 64 KiB, at most 16 queued,
standard input bounded by `input_limit_bytes`, output per stream bounded by
`exec.output-limit-bytes` (1 MiB by default), signals INT, TERM and KILL.

## Design

- ESS first, `ess/intake/domains/confinement.yaml`: `intake.confinement.ChildStream`
  (`Stdout`, `Stderr`), `intake.confinement.ChildOutput { stdout, stderr` (bytes, typed as ESS 0.67.0 allows; base64 `String`
  when it has no bytes type), `stdout_truncated: Boolean, stderr_truncated: Boolean }` and
  `intake.confinement.ChildExit { exit_code: Optional<Integer>, signal: Optional<Integer>,
  timed_out: Boolean }`. Hand-written Rust types do not shadow them (`task no-hand-model`).
- `ChildCommand::with_stdin(bytes)`: `run_child` writes them to the child's standard input and
  closes it; without it the child's standard input is empty and closed, as today. A child given
  standard input runs through a pipe session; more bytes than the session's input limit are refused
  `ScopeInvalid` before launch.
- `TestExecution` gains `child_output: ChildOutput`, both streams whole up to Substrate's output
  limit, each with its own truncation flag. `output` stays as it is for `TestRunner::run` callers.
- `SubstrateRunner::spawn_child(&ChildCommand) -> Result<ConfinedChild, ConfinementError>` starts
  the child in a pipe session on the same launch path and checks as `run_child` (program, network,
  aperture, certificate bundle, writable scopes, the process-wide driver count). `ConfinedChild`:
  `write_stdin(&[u8])`, `close_stdin()`, `read(timeout) -> Result<Option<ChildFrame>, _>`
  (`None` once both streams closed), `signal(ChildSignal)` (`Interrupt`, `Terminate`, `Kill`),
  `wait() -> Result<ChildResult, _>` returning the `ChildExit` and the applied confinement, checked
  after the end exactly as `run_child` checks it (a record naming another network, aperture or
  writable set is refused). The child's timeout still bounds it. Dropping a `ConfinedChild` that
  has not ended kills it and discards its execution from the driver.
- `ConfinedChild::lines()` is not offered; framing is the caller's.
- Re-exported through `loom_sdk` beside `ChildCommand`.

## ESS first

The declarations above in `ess/intake/domains/confinement.yaml`; `task intake-generate`
regenerates. Red tests in `crates/loom-intake-slice/tests/confined_child.rs` (delegated cases,
skipping only on a named missing prerequisite as the file's other cases do).

## Acceptance

- `a_live_child_echoes_what_it_is_written`: `cat` spawned live; bytes written are read back on
  `Stdout`; `close_stdin` ends it with exit 0.
- `a_live_child_keeps_its_streams_apart`: a shell writing `out` to standard output and `err` to
  standard error yields frames on each stream, never merged.
- `a_live_child_can_be_killed`: `sleep 600` spawned, `signal(Kill)`, `wait` reports signal 9 well
  before the timeout.
- `run_child_passes_standard_input_once`: `with_stdin(b"abc")` to `cat` returns `abc` on stdout.
- `run_child_returns_both_streams_whole`: 20000 bytes on standard output and 20000 on standard
  error come back whole and apart, untruncated.
- `stdin_over_the_input_limit_is_refused_before_launch`.
- `a_live_child_whose_record_names_another_network_is_refused` (unit test on the check shared with
  `run_child`).
- Every existing `confined_child.rs` case stays green.

## Not in scope

- Read-only roots and host paths (`story:child-read-only-roots-and-host-paths`).
- A pty; more than one aperture.

## Scope

- `ess/intake/domains/confinement.yaml`, `generated/rust/intake/`
- `crates/loom-intake-slice/src/confinement.rs` (may split a `confinement/child.rs`)
- `crates/loom-intake-slice/tests/confined_child.rs`
- `crates/loom-sdk/src/lib.rs` (re-exports), `website/docs/reference/sdk-stability.md` and
  `crates/loom-sdk/tests/host_ports.rs` if the stability page lists `ChildCommand`
- `AGENTS.md` (§ Intake and the command line, the `run_child` paragraph), `CHANGELOG.md`, `README.md` if it names `run_child`
