---
format: aep.planning-md/3
id: epic:tree-daemon-under-enforced-limits
kind: epic
status: draft
title: One Loom daemon runs many case trees, every process through Substrate, under enforced limits
summary: 'Follow-up to epic:protocol-tree-on-bindings: a tree daemon, a Substrate daemon runner, hard limits, allow-lists, and every limit hit in the run log'
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
- depends_on: epic:protocol-tree-on-bindings
revision: 7
---
## Outcome

One Loom daemon runs many protocol case trees at the same time. Every process a run starts, a
bound program, an `arguments_from` program and an agent session's harness, is started and stopped
through Substrate; with the engine configuration naming a Substrate daemon on the same host, the
processes run under that daemon, with no change to a binding. Per action the engine enforces hard
limits through Substrate on memory, CPU time, wall time and process count, and per run on total
wall time and total CPU time. It starts only programs its executable allow-list admits, refuses to
start a process under a confinement level (`local`, `linux`, `oci`, `pod`) it cannot give, and
gives a process network only to the named egress apertures its action selects. Every limit a
process hits is an event in its case's run log that names the action, the limit and its value. Six
parts wait and are held by blockers: the agent session's harness through Substrate (an open design
question), holding a started program's descendants to the executable lists, serving ingress,
running under the `oci` and `pod` levels through a daemon, a run log line that references the
file-change record of the exec it ran, and a Substrate daemon on another host (`## Upstream`).

## Why

Requirements from Timo, relayed by session `sdc` on 2026-11-21, after
`epic:protocol-tree-on-bindings` was drafted: a daemon for many trees, every process through
Substrate, and hard limits on memory, CPU, wall time and process count with allow-lists whose hits
a live view can show. That epic runs one tree per `tree run` process and opens a Substrate adapter
in process (`story:confinement-opens-from-engine-config`).

A daemon outlives the command that started a tree, so a crash or an upgrade of it would drop every
tree it serves in the middle of a step unless it continues each tree from its store and run log;
the SDC's driver names the same gap for its own loop (`~/beyond10x/sdc/docs/driver.md`, gap 15).
With a daemon no `tree run` process is left to call after a person decides, so a recorded approval
has to let the daemon continue the tree, or every approval would take a second command. A daemon
that can only be killed loses the step in flight, so `tree stop` lets a tree's current step finish,
keeps the rest of the tree in its store and leaves it to be continued.

Read at Substrate tag `0.21.0` on 2026-11-21:

- Limits exist as `ConfinementProfile.process_bounds` (`timeout_ms`, `processes`, `memory_bytes`,
  `cpu_millis`) and `io_bounds` (`output_bytes`)
  (`crates/substrate-host/src/confinement/profile.rs:160-172`). An adapter serves them when it
  serves the contract family `limits`: `linux` where its probe proved it, `oci` and `pod`; `local`
  does not (`crates/substrate-host/src/confinement/local.rs:107-112`).
- Which limit ended a run is the exec's refusal code, `exec.timeout`, `exec.cpu-limit`,
  `exec.memory-limit` or `exec.aperture-byte-limit` (`crates/substrate-host/src/process.rs:3280-3322`);
  a memory end is no `exec.cancelled` (`forced_cancellation` leaves it out, `:3151-3155`) but an
  `Exited` run with that code (`:3221`). The host driver and the daemon both set it. Today Loom's
  in-process `run_child` launches through the host driver and reads the code
  (`settle`, `crates/loom-intake-slice/src/confinement.rs:836-839`), but
  `story:child-launches-through-confinement-adapters`, which this epic comes after through
  `story:confinement-opens-from-engine-config`, moves in-process launches onto Substrate's adapter
  route, whose `RunObservation` has an exit, a signal and, on `pod` only, OOM kills
  (`crates/substrate-host/src/confinement/mod.rs:694-706`). So once this epic lands, the code
  reaches Loom from the daemon only.
- The daemon reports each exec's usage when a start asks for it (`ExecUsage::Observed` of
  `ResourceUsage`: CPU time, memory peak, process peak, process-limit hits;
  `crates/substrate-wire/src/lib.rs:2149-2184`).
- Remote transport, the durable event log and leases exist only on the daemon path
  (`b10x-substrate-sdk`: Unix socket or HTTPS endpoint, `ExecutionPolicy { timeout, cpu_time,
  memory_bytes, processes, output_bytes }`, `crates/b10x-substrate-sdk/src/model.rs:675-681`). An
  exec whose lease lapses is killed by the daemon
  (`crates/substrate-daemon/src/app/service.rs:851-880`).
- A workspace reaches a daemon by adopting a directory already under the daemon's workspace root
  (`Client::adopt_workspace`, `crates/b10x-substrate-sdk/src/lib.rs:218-248`), so a daemon serves
  Loom's case workspace only on the host that holds it.
- A daemon opens one driver, `Host` or `Kubernetes` (`crates/substrate-daemon/src/runtime.rs:520-529`);
  the `oci` and `pod` adapters exist only in process. The SDK sends no `sandbox.requires` with a
  start (`requires: None`, `crates/b10x-substrate-sdk/src/lib.rs:1443`, `:1921`).
- Egress is operator-declared named apertures, `host:port/tcp`, on `linux` only.
- Missing in Substrate: an executable allow-list on `linux`, `oci` and `pod` and a deny-list
  anywhere; ingress (`ingress_routes` is a reserved field, any value is refused,
  `profile.rs:93-96`); an event for a process-count hit; `sandbox.requires` in the SDK; a daemon
  that serves `oci` or `pod`; a file-change record per exec; workspace content for a daemon on
  another host.

Session `sdc` decided on 2026-11-21 (its decision, reversible): the first epic stays on the
in-process path; this epic goes through a Substrate daemon by `b10x-substrate-sdk`. It sent the
missing items to a `substrate` session as requests.

Decided in the plan review of 2026-11-21: output bytes stay the fixed 1 MiB cap Loom applies to
every child (`crates/loom-intake-slice/src/confinement.rs:2969-2977`) and get no ceiling and no
event kind here; the one wall limit of an action is `timeout_seconds` of `loom.bindings/1`; the
engine's configuration is one document with one owner, `story:engine-configuration-document`.

## Stories

1. `story:engine-configuration-document`
2. `story:substrate-daemon-runner`
3. `story:confinement-level-required`
4. `story:action-limits-enforced`
5. `story:limit-hits-in-run-log`
6. `story:budget-and-limit-stops-raise-decision-requests`
7. `story:run-log-references-exec-file-changes`, held by
   `upstream-blocker:substrate-exec-file-change-record`
8. `story:executable-allow-list`
9. `story:named-egress-per-action`
10. `story:agent-session-through-substrate`, held by
    `decision-blocker:agent-harness-confinement-design`
11. `story:tree-daemon`
12. `story:executable-descendants-held`, held by `upstream-blocker:substrate-executable-allow-list`
13. `story:ingress-routes-per-action`, held by `upstream-blocker:substrate-ingress`
14. `story:confinement-level-on-oci-and-pod`, held by `upstream-blocker:substrate-daemon-oci-pod`
    and `upstream-blocker:substrate-sdk-confinement-requires`
15. `story:substrate-daemon-on-another-host`, held by
    `upstream-blocker:substrate-remote-workspace-content`

The eight unheld stories 1 to 5, 8, 9 and 11 form one chain, each depending on the one before it
(story 11 on story 9). They share `ess/domains/engine.yaml`, the step's confinement record
`loom.engine.StepConfinement` and `crates/loom-tree`, and stories 2 to 5, 8 and 9 also share
`crates/loom-intake-slice/src/confinement.rs`, so the order is serial on purpose; each body's
`## Waits on` names the file or the type that orders it. The first depends on
`story:agent-node-run-records`, the last story of `epic:protocol-tree-on-bindings` to edit
`ess/domains/tree.yaml`, `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml` and
`crates/loom-tree`. Item 6 has its own edges. Story 7 depends on `story:hash-chained-run-log` and
`story:limit-hits-in-run-log` and stops no unheld story. Story 10 depends on story 9 and waits on a
design decision; `story:tree-daemon` does not wait on it and refuses an agent step until it lands.
Stories 12 to 15 each depend on `story:tree-daemon` only and on no other held story, so one blocker
holds one story; they share the launch path in `crates/loom-intake-slice/src/confinement.rs`, and
the order among them is settled by scope when their blockers clear. Each moves Loom's Substrate pin
to the release it needs as its second commit, after its `## ESS first` commit.

The set has been before the plan-critic panel twice (round 2: parallel-safety approved; acceptance,
design and scope `needs-revision` with warnings only) and before the story scoper once
(2026-11-21), whose defects are folded into the bodies.

## Unordered against this epic

Three stories of other epics edit `crates/loom-intake-slice/src/confinement.rs` with no
`depends_on` path to or from any story of this epic: `story:python-test-profile`,
`story:writable-host-root` (it declares `ChildCommand::with_writable_root`) and
`story:confines-on-macos-through-substrate` (it edits `tests/confined_child.rs`, which
`story:substrate-daemon-runner` also runs under `Daemon`). No edge is added; of each of them and a
story of this epic on that file, the second to land rebases onto the first.

## Acceptance

`crates/loom-cli/tests/tree_daemon_limits.rs` (`story:tree-daemon`) runs these checks on the Linux
gate with the fixtures under `crates/loom-cli/tests/fixtures/daemon/`, stub programs and no model.
It starts one Substrate daemon as a child (the SDK's `ManagedDaemon`, linked into the test binary)
and one Loom daemon, `tree serve --engine engine-daemon.yaml`, with three run slots.

- `tree-plain` (the three-level tree of `story:tree-run-command`), `tree-limits` and
  `tree-network`, submitted together with `tree run --detach`, each reach their terminal outcome,
  and the time spans of their three run logs overlap.
- In `tree-limits`, the actions `alloc-past-memory`, `spin-past-cpu`, `sleep-past-timeout` and
  `fork-past-processes` each give exactly one `LimitHit` line in the case's run log, of kind
  `memory`, `cpu-time`, `wall-time` and `processes`, naming the action and the configured value.
- `tree-limits` under `engine-daemon-cpu.yaml`, whose per-run CPU ceiling is below the CPU time of
  its steps, stops with `BudgetSpent` naming `cpu_millis`, exit 5, before the first step that would
  start after the CPU time of its steps reached the ceiling.
- `tree open` of `tree-denied`, whose `argv[0]` lies outside the allow-list of
  `engine-daemon.yaml`, exits non-zero naming `ProgramNotAdmitted`, the action and the program,
  and writes no case file.
- In `tree-network`, the action `connect-unselected` connects to a listener on the host that no
  aperture names; the connection fails and the step records the result its table gives for that
  exit.
- While the three trees run, the Loom daemon's process has no descendant process (a walk of
  `/proc/<pid>/task/*/children`), and the process of every stub program the test finds running is
  a descendant of the Substrate daemon's process.
- `tree run` of `tree-level`, whose action requires level `linux`, under `engine-local.yaml`
  (adapter `local` with a reason) exits non-zero before its first step naming `LevelUnmet`, the
  level and the family `filesystem_view`.
- The same three trees run unchanged under `engine-daemon-2.yaml`, which names a second managed
  Substrate daemon on the same host with its own socket and state directory: the exec id of every
  `action finished` line's confinement record is found by `Client::get_exec` on the second daemon
  and not on the first.
- `tree verify` exits 0 for every tree of these checks, and `task check` exits 0.

## Not in scope

- Changes in Substrate. What Loom needs and Substrate lacks is filed as upstream blockers on the
  stories that need it.
- A limit on output size: the fixed 1 MiB cap stays, with no ceiling and no event kind.
- A per-run sum of memory or of process count: Substrate bounds both per process start.
- A per-aperture byte ceiling and its event.
- Scheduling trees across several hosts: one Loom daemon names one Substrate endpoint.
- A Substrate daemon on another host, reached over HTTPS: it needs a TLS listener and an identity
  authority (`DaemonConfig.tls` and `hosted_identity`, `crates/substrate-daemon/src/runtime.rs:513`,
  `:515`, paired at `:666`) and workspace content Substrate cannot carry yet
  (`story:substrate-daemon-on-another-host`, held).
- Interrupting a running step (`tree stop --now`).
- The tree daemon calling the decision tick (`b10x-loom decisions tick --at`,
  `story:silence-and-expiry-at-a-host-tick` of `epic:decision-requests-in-the-runtime`) with its
  own instant: a later story, once that epic is accepted.
- CPU shares, block-IO and bandwidth limits.
- A hosted or multi-tenant service; authentication beyond the local user.

## Upstream

Substrate's side, as session `substrate` reported it on 2026-11-21 (drafts under its
`epic:runs-are-governed-and-limit-hits-are-events`, uncommitted on its branch
`plan/sdc-run-governance`, none accepted or implemented):

- Which limit ended a run, on the in-process adapter route:
  `story:run-observation-names-the-limit-that-ended-it` and `story:audit-records-run-end-and-limit`.
  `story:limit-hits-in-run-log` reads the refusal code the daemon reports and records only the
  wall-time hit in process; it does not wait on them.
- A process-count hit as an observation: `story:process-count-limit-hit-is-observed` (in
  process) and `story:exec-observation-reports-process-count-hits` (on the wire).
  `story:limit-hits-in-run-log` reports that hit from the daemon's measured counter
  (`process_limit_hits`) until then.
- Executable lists: `story:linux-executable-allow-list`, `story:executable-deny-list`,
  `story:executable-lists-on-oci-and-pod`, `story:exec-start-carries-executable-lists`
  (`upstream-blocker:substrate-executable-allow-list`, which holds
  `story:executable-descendants-held`).
- Ingress: `story:inbound-endpoint-for-a-confined-exec`, `story:confinement-profile-ingress-routes`
  (`upstream-blocker:substrate-ingress`, which holds `story:ingress-routes-per-action`).
- Requirements through the SDK: `story:sdk-confinement-requires`
  (`upstream-blocker:substrate-sdk-confinement-requires`, which holds
  `story:confinement-level-on-oci-and-pod`).
- A daemon that serves `oci` or `pod`: no Substrate story plans it
  (`upstream-blocker:substrate-daemon-oci-pod`, which holds `story:confinement-level-on-oci-and-pod`).
- A file-change record per exec: none in Substrate; the SDC's interim is `sdc-watch`
  (`~/beyond10x/sdc/docs/fs-changes.md`) (`upstream-blocker:substrate-exec-file-change-record`,
  which holds `story:run-log-references-exec-file-changes`).
- Workspace content for a daemon on another host: not yet requested
  (`upstream-blocker:substrate-remote-workspace-content`, which holds
  `story:substrate-daemon-on-another-host`).

Substrate's stated order: first the in-process `linux` slice (`story:linux-executable-allow-list`,
`story:executable-deny-list`, `story:run-observation-names-the-limit-that-ended-it`,
`story:audit-records-run-end-and-limit`, `story:process-count-limit-hit-is-observed`), then
`story:executable-lists-on-oci-and-pod`, `story:exec-start-carries-executable-lists`,
`story:exec-observation-reports-process-count-hits`, and the two ingress stories;
`story:sdk-confinement-requires` at any time.

The agent harness's design question is `decision-blocker:agent-harness-confinement-design`, for the
metaharness and Substrate owners through session `sdc`.
