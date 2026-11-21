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
revision: 3
---
## Outcome

One Loom daemon runs many protocol case trees at the same time. Every process a run starts, a
bound program, an `arguments_from` program and an agent session's harness, is started and stopped
by a Substrate daemon, so a run can be placed on another host by naming another endpoint. Per run
and per action the engine enforces hard limits through Substrate: memory, CPU time, wall time,
process count and output size; which executables may run; the confinement level; and the egress
and ingress a process may use. Every limit a process hits is an event in the case's run log that
names the action, the limit and its value.

## Why

Requirements from Timo, relayed by session `sdc` on 2026-11-21, after
`epic:protocol-tree-on-bindings` was drafted: a daemon for many trees, every process through
Substrate, and hard limits with allow-lists whose hits a live view can show. That epic runs one
tree per `tree run` process and opens a Substrate adapter in process
(`story:confinement-opens-from-engine-config`). Read at Substrate tag `0.21.0` on 2026-11-21:

- Limits exist as `ConfinementProfile.process_bounds` (`timeout_ms`, `processes`, `memory_bytes`,
  `cpu_millis`) and `io_bounds` (`output_bytes`)
  (`crates/substrate-host/src/confinement/profile.rs:160-172`), enforced on `linux`, `systemd`,
  `oci` and `pod`, not on `local` or `macos`.
- Which limit ended a run is reported only on the daemon path: an `exec.cancelled` event whose
  refusal code is `exec.timeout`, `exec.cpu-limit`, `exec.memory-limit` or
  `exec.aperture-byte-limit` (`crates/substrate-host/src/process.rs:3280-3322`). The in-process
  `RunObservation` has no such field (`crates/substrate-host/src/confinement/mod.rs:681-706`).
- Remote transport, the durable event log and leases exist only on the daemon path
  (`b10x-substrate-sdk`: unix socket or HTTPS endpoint, `ExecutionPolicy { timeout, cpu_time,
  memory_bytes, processes, output_bytes }`).
- Egress is operator-declared named apertures, `host:port/tcp`, on `linux` only.
- Missing in Substrate: an executable allow-list on `linux`, `oci` and `pod` and a deny-list
  anywhere; ingress (`ingress_routes` is a reserved field, any value is refused,
  `profile.rs:93-96`); an event for a process-count hit; `sandbox.requires` in the SDK; a daemon
  that serves `local`, `oci` or `pod`.

Session `sdc` decided on 2026-11-21 (its decision, reversible): the first epic stays on the
in-process path; this epic goes through a Substrate daemon by `b10x-substrate-sdk`. It sent the
missing items to a `substrate` session as requests.

## Stories

1. `story:substrate-daemon-runner`
2. `story:action-limits-enforced`
3. `story:limit-hits-in-run-log`
4. `story:executable-allow-list`
5. `story:confinement-level-required`
6. `story:named-egress-per-action`
7. `story:agent-session-through-substrate`
8. `story:tree-daemon`

The set has not been before the plan-critic panel and no story is scoped.

## Acceptance

With one Loom daemon and one Substrate daemon on a host: three fixture trees submitted together
run to their terminal outcomes at the same time; a program that allocates past its memory limit,
one that runs past its wall time and one that forks past its process count are each stopped, and
each case's run log holds one limit event naming the action, the limit and its value; a bound
program outside the executable allow-list does not start; a program that connects to a host its
action did not select fails; and `ps` during the run shows no child of the Loom daemon that is
not a Substrate client connection. The same three trees run unchanged with the Substrate endpoint
of a second host.

## Not in scope

- Changes in Substrate. What Loom needs and Substrate lacks is filed as upstream blockers on the
  stories that need it.
- Scheduling trees across several hosts: one Loom daemon names one Substrate endpoint.
- Ingress to a confined process, until Substrate serves it.
- CPU shares, block-IO and bandwidth limits.
- A hosted or multi-tenant service; authentication beyond the local user.

## Upstream

Substrate's side, as session `substrate` reported it on 2026-11-21 (drafts under its
`epic:runs-are-governed-and-limit-hits-are-events`, uncommitted on its branch
`plan/sdc-run-governance`, none accepted or implemented):

- Which limit ended a run, on the in-process path: `story:run-observation-names-the-limit-that-ended-it`
  and `story:audit-records-run-end-and-limit`. `story:limit-hits-in-run-log` reads the daemon's
  refusal code and does not wait on them.
- A process-count hit as an observation: `story:process-count-limit-hit-is-observed` (in
  process) and `story:exec-observation-reports-process-count-hits` (on the wire).
  `story:limit-hits-in-run-log` reports that hit from the measured counter until then.
- Executable lists: `story:linux-executable-allow-list`, `story:executable-deny-list`,
  `story:executable-lists-on-oci-and-pod`, `story:exec-start-carries-executable-lists`
  (`upstream-blocker:substrate-executable-allow-list`).
- Ingress: `story:inbound-endpoint-for-a-confined-exec`, `story:confinement-profile-ingress-routes`
  (`upstream-blocker:substrate-ingress`).
- Requirements through the SDK: `story:sdk-confinement-requires`, which
  `story:confinement-level-required` needs when the engine names a daemon.

Substrate's stated order: first the in-process `linux` slice (`story:linux-executable-allow-list`,
`story:executable-deny-list`, `story:run-observation-names-the-limit-that-ended-it`,
`story:audit-records-run-end-and-limit`, `story:process-count-limit-hit-is-observed`), then
`story:executable-lists-on-oci-and-pod`, `story:exec-start-carries-executable-lists`,
`story:exec-observation-reports-process-count-hits`, and the two ingress stories;
`story:sdk-confinement-requires` at any time. Its first slice is in process, so the in-process
`linux` adapter would name the limit that ended a run before the daemon path is needed for it.
