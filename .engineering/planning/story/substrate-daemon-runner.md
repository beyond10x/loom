---
format: aep.planning-md/3
id: story:substrate-daemon-runner
kind: story
status: draft
title: The engine can name a Substrate daemon on the same host and start every child through it
summary: b10x-substrate-sdk behind the runner; how a run ended comes back with the daemon's refusal code
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:confinement-opens-from-engine-config
- depends_on: story:engine-configuration-document
revision: 6
---
## Outcome

The engine configuration's `confinement` section can name a Substrate daemon on the same host
instead of an adapter opened in process: `adapter: daemon` with an `endpoint`, the absolute path of
the daemon's Unix socket, a `workspace_root`, the absolute directory the daemon was started with as
its workspace root (`--workspaces`), and an optional `lease_seconds` (1 to 300, default 30). Loom
then starts, observes, signals and stops every child through `b10x-substrate-sdk`, in a new module
`crates/loom-intake-slice/src/confinement/daemon.rs` behind the `Daemon` arm of the one
constructor. A child's workspace directory reaches the daemon by adoption: its parent must be
`workspace_root` and its name a root name of Substrate's set (ASCII letters, digits, `_` and `-`,
not starting with `-`; `is_root_name`, `crates/substrate-wire/src/lib.rs:2514-2521`); the runner
adopts it by that name (`Client::adopt_workspace`, `crates/b10x-substrate-sdk/src/lib.rs:226`) and
forgets the adopted record, leaving the directory, once the child ended. A directory elsewhere is
refused `ScopeInvalid` before anything starts, naming the directory and the root.

Where the tree puts a case's workspace when the runner is the daemon: directly under the daemon's
workspace root. Every case of a tree works in the tree's one workspace, the directory `tree open`
records (`story:tree-run-command`, `story:child-case-runtime`). Under `adapter: daemon` that
directory must be `<workspace_root>/<name>`, with `<name>` a root name. For a daemon started by
`ManagedDaemon` the root is `<data_dir>/workspaces` (`crates/b10x-substrate-sdk/src/managed.rs:164`).
`tree open --engine` and `tree run --engine` refuse a workspace elsewhere, or one whose name is
not a root name, as `WorkspaceOutsideDaemonRoot`, naming the workspace and the root. The refusal
comes before a case file is written or an exec starts. The runner's `ScopeInvalid` stays for a
direct caller of `run_child` or `spawn_child`.

`run_child`, `spawn_child` and the test runner's `run` behave as under the in-process `linux`
adapter for every `ChildCommand` option the daemon can honour, and refuse before start, with
`CapabilityUnserved` naming the option, every one it cannot:

- honoured: the program and arguments, the environment, the writable paths (the start's
  `workspace_access`), read-only roots and files (host paths of this host, which is the daemon's),
  the timeout and the fixed bounds of `limits()` (`crates/loom-intake-slice/src/confinement.rs:2969-2977`)
  as the `ExecutionPolicy` the SDK requires with all five bounds
  (`crates/b10x-substrate-sdk/src/model.rs:675-681`), no network, and standard input, which runs
  the child as a pipe session (`PipeSessionBuilder`, SDK `lib.rs:1233`) written once and closed;
- honoured when the daemon publishes it: `with_egress`, when the daemon's capability facts list an
  aperture of that name whose destination is the aperture's `host:port`
  (`exec_egress_apertures`, `crates/substrate-wire/src/lib.rs:4164`, `EgressApertureFact` `:4994-5003`),
  sent by name (`CommandBuilder::apertures`, SDK `:1813`; `PipeSessionBuilder::apertures`, `:1311`);
- refused: `with_workdir` (the SDK's builders send `workdir: None`, SDK `:1463`, `:1941`),
  `with_ca_bundle` (a daemon's bundle is its own configuration, `DaemonConfig.ca_bundle`,
  `crates/substrate-daemon/src/runtime.rs:499`), `with_profile` with any profile but the workspace
  profile (the builders send `SandboxProfile::Workspace`), and `with_egress` naming an aperture
  the daemon does not publish with that destination.

`ConfinedChild::signal` reaches the daemon's exec. The runner turns the daemon's exec observation
into the same `Ending` that `settle` reads (`confinement.rs:822`), so how a run ended comes back
as the daemon's refusal code on the applied record's `ended_by`, and an `exec.timeout` also sets
`timed_out` as it does in process (`:836-839`). The applied record names `adapter` `daemon` and no
`facts`; the daemon's families reach the step's record through `story:confinement-level-required`.

When the daemon publishes CPU-time usage (`exec_resource_usage`, `crates/substrate-wire/src/lib.rs:4127`),
every start asks for it (`CommandBuilder::measure`, SDK `:1828`; `PipeSessionBuilder::measure`,
`:1326`; `ExecMeasurement::ResourceUsage`, wire `:2071-2073`), and `run_child` and `spawn_child`
return it on `TestExecution.usage` and `ChildResult.usage` (`intake.confinement.ExecResourceUsage`:
CPU milliseconds, memory peak bytes, process peak, process-limit hits) from the ended exec's
`ExecUsage::Observed` (wire `:2149-2160`). A usage still `Pending`, or `Unavailable` (wire `:2133`,
`:2191`), once the exec ended gives no `usage` and the code in `usage_unavailable`
(`exec.metrics-unavailable`, or `exec.metrics-pending` for `Pending`).

Every exec Loom starts carries a lease of `lease_seconds` (`CommandBuilder::lease`, SDK `:1871`;
`PipeSessionBuilder::lease`, `:1369`; at least 1000 ms, wire `:409`), renewed every third of it for
the whole life of the child: by the waiting call for `run_child`, and on the live child's own
runtime (`crates/loom-intake-slice/src/confinement/child.rs:123`) for `spawn_child`, idle or not,
until the child ended or was dropped. The daemon kills an exec whose Loom process died once its
lease lapses.

## Why

`story:confinement-opens-from-engine-config` opens `local` or `linux` in process. The daemon path
is the only one with a durable event log, leases, a named reason for a stopped run once
`story:child-launches-through-confinement-adapters` moved in-process launches onto the adapter
route (epic `## Why`), and remote transport (Substrate 0.21.0:
`crates/substrate-daemon/src/app/routes.rs:113-124`, `crates/b10x-substrate-sdk/src/lib.rs:534-540`).
Nothing in Loom depends on the SDK today (`~/beyond10x/sdc/docs/layers.md` § 3, "nothing in Loom
picks a daemon").

Read at 0.21.0: when a lease lapses the daemon sends `Kill` to an exec that is accepted, running or
unknown (`crates/substrate-daemon/src/app/service.rs:851-880`) and records it `Expired` (wire
`lib.rs:2105-2107`). The exec routes are `POST /v1/execs` and `GET /v1/execs/{id}`
(`routes.rs:101-104`); an exec is read back by `Client::get_exec` (SDK `lib.rs:260`) and the event
log by `Client::events` (`:341`). A workspace reaches a daemon only by adoption of a directory
already under its workspace root (SDK `lib.rs:218-248`) or by creating one and writing it file by
file; the second cannot carry a case workspace (`upstream-blocker:substrate-remote-workspace-content`),
so this story serves a daemon on the same host and `story:substrate-daemon-on-another-host` the
rest. Adoption takes a root name, not a path, so a workspace one level deeper or beside the root
cannot be adopted at all; the tree therefore checks the placement when it opens or runs a tree,
where the operator can still move the workspace, instead of failing at the first step.
`ManagedDaemon` starts a private daemon on a Unix socket, either an external binary or, with
the SDK feature `linked-daemon`, the test binary itself re-executed, which needs
`run_daemon_child_if_requested()` at the start of `main`
(`crates/b10x-substrate-sdk/src/managed.rs:83-92`, `:405-409`).

## Waits on

- `story:engine-configuration-document`: the `confinement` section that this story extends with
  `daemon`, and `loom.engine.StepConfinement`, which gains the exec id and `ended_by`; both in
  `ess/domains/engine.yaml`, which the next stories edit after this one.
- `story:confinement-opens-from-engine-config`: the one constructor that opens the confinement,
  which gains the `Daemon` arm. Through it this story comes after
  `story:child-launches-through-confinement-adapters` and `story:bound-commands-run-confined`,
  which edit `ess/intake/domains/confinement.yaml` and `SubstrateRunner::run_child`
  (`crates/loom-intake-slice/src/confinement.rs:475`) before it.
- Through the chain: `story:tree-run-command`, which records the tree's workspace at `tree open`.

## ESS first

`ess/intake/domains/confinement.yaml`: `EngineConfinement` gains the variant `Daemon` (endpoint,
workspace root, lease seconds); `AppliedConfinement` gains `exec_id` and `ended_by` (the refusal
code), both optional; a new `ExecResourceUsage` (`cpu_millis`, `memory_peak_bytes`,
`processes_peak`, `process_limit_hits`). `ess/domains/engine.yaml`: `ConfinementSection`'s adapter
gains `Daemon` with the same members, `StepConfinement` gains `exec_id` and `ended_by`, and
`EngineRefusal` gains `EndpointInvalid` (the value) and `WorkspaceOutsideDaemonRoot` (workspace,
root). On that commit `task intake-drift` and `task drift` fail and
`crates/loom-intake-slice/tests/daemon_runner.rs` does not compile.

## Acceptance

Tests start a private Substrate daemon as a child: `ManagedDaemon::builder().linked_current_exe()`
(SDK `managed.rs:89`), with `b10x-substrate-sdk` a dev-dependency with the feature `linked-daemon`.
Each test target that starts one is a `harness = false` target whose `main` calls
`run_daemon_child_if_requested()` first, so the daemon is built from the pinned revision and needs
no installed binary. The daemon's workspace root is `<data_dir>/workspaces` (`managed.rs:164`), and
each test's workspace directory is created there. The tests make no network call beyond the
daemon's Unix socket. Every bullet runs under `Daemon`.

- `run_child` of a program returns its exit, output and applied record, and the record names the
  daemon's exec id, which `Client::get_exec` finds.
- A program that runs past its timeout comes back with `ended_by` = `exec.timeout` and
  `timed_out` set; one that exits 3 comes back with exit 3, no `ended_by` and no `timed_out`.
- `run_child` of a program that spins for one second returns `usage` with a CPU time and a memory
  peak above 0 and a process peak of at least 1.
- `spawn_child` writes standard input and reads live output through the daemon;
  `ConfinedChild::signal` with a terminate signal ends the exec, and `Client::get_exec` reports it
  in a terminal state.
- A `spawn_child` child left idle for three times its lease of 2 s is still running, and
  `Client::get_exec` shows its lease renewed.
- The test runner's `run` of a passing and of a failing test command gives the same verdicts as
  under the in-process `linux` adapter.
- A helper process that starts an exec with a 2 s lease and is then killed with SIGKILL leaves an
  exec that `Client::get_exec` reports `Expired` within 10 s, and no process of that exec remains.
- A `ChildCommand` with `with_workdir`, one with `with_ca_bundle`, and one with `with_egress`
  naming an aperture the managed daemon does not publish (it publishes none) are each refused
  `CapabilityUnserved` naming the option, and no exec is started (`Client::events` lists no exec
  start for them).
- A child whose workspace directory is not directly under `workspace_root` is refused
  `ScopeInvalid` naming the directory and the root, before any exec starts.
- `tree open --engine` with `adapter: daemon` and a workspace in another directory, one two levels
  under `workspace_root`, and one directly under it named `-x`, each exits non-zero naming
  `WorkspaceOutsideDaemonRoot`, the workspace and the root, and writes no case file. `tree run
  --engine` with `adapter: daemon` on a store whose recorded workspace is not directly under the
  root exits non-zero with the same refusal, and `Client::events` lists no exec start.
- With a socket path nothing listens on, the constructor is refused before any child starts,
  naming the endpoint; an `endpoint` that is not an absolute path (`https://127.0.0.1:1`) is
  refused by the loader `EndpointInvalid`.
- The delegated cases of `tests/confinement.rs` and `tests/confined_child.rs` that use no refused
  option hold under `Daemon`.
- `Cargo.lock` holds one copy of each Substrate crate, and `crates/loom-intake-slice/Cargo.toml`
  names `b10x-substrate-sdk` by the same `rev` and `version` as `substrate-host` (`:31-32`).
- `CHANGELOG.md` has a `**Breaking:**` bullet naming the new public members of `TestExecution` and
  `ChildResult` (`usage`, `usage_unavailable`), as `website/docs/reference/sdk-stability.md:21`
  requires for the exhaustive structs it lists at `:133-136`.
- `tree run --engine` with `adapter: daemon` runs the three-level fixture tree of
  `story:tree-run-command`, opened with its workspace directly under `workspace_root`, and every
  `action finished` line of a program step names an exec id.
- In the same run, a fixture action whose binding has `arguments_from` runs both its
  `arguments_from` program and its bound program as execs that `Client::get_exec` finds, and
  neither process is a descendant of the `tree run` process.
- `task check` exits 0.

## Out of scope

Limits per action (`story:action-limits-enforced`). Choosing among several endpoints. A daemon on
another host and an HTTPS endpoint (`story:substrate-daemon-on-another-host`, held by
`upstream-blocker:substrate-remote-workspace-content`). The kubernetes driver of the daemon. `oci`
and `pod` through a daemon (`story:confinement-level-on-oci-and-pod`). Usage in process: the adapter
route reports none. Moving or copying a workspace into the daemon's root: the operator places it.
