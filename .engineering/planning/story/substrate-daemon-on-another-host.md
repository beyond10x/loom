---
format: aep.planning-md/3
id: story:substrate-daemon-on-another-host
kind: story
status: draft
title: The engine can name a Substrate daemon on another host over HTTPS and run every child there
summary: Workspace and read-only content to the daemon and back; held by upstream-blocker:substrate-remote-workspace-content
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:tree-daemon
revision: 2
---
## Outcome

The engine configuration's daemon confinement can name a Substrate daemon on another host: an
`endpoint` that is an `https://` URL, with `trust_roots` and `server_identity`. Loom then places
each child's workspace, and the read-only content the child names, into a workspace on that daemon,
runs the child there as under `story:substrate-daemon-runner`, and brings the changed workspace
back before the step's result is taken, by the means the release that clears
`upstream-blocker:substrate-remote-workspace-content` provides. A run moves to another host by
naming that daemon's endpoint, with no change to a binding.

The story's first commit is the `## ESS first` change and nothing else; its second commit moves
Loom's Substrate pin to the release that clears its blocker: `substrate-host`, `substrate-wire` and
`b10x-substrate-sdk` in `crates/loom-intake-slice/Cargo.toml`, and the version and revision that
`crates/loom-intake-slice/tests/host_git_hardening.rs:549-552` asserts.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "every process through Substrate",
with a run movable to another daemon. At Substrate 0.21.0 a workspace reaches a daemon by adoption
of a directory on the daemon's host, or file by file at most 1 MiB each with no mode
(`upstream-blocker:substrate-remote-workspace-content`), so `story:substrate-daemon-runner` serves
a daemon on the same host only. A daemon's HTTPS listener needs `DaemonConfig.tls` and a hosted
identity authority (`crates/substrate-daemon/src/runtime.rs:513`, `:515`, paired at `:666`).

## Waits on

- `upstream-blocker:substrate-remote-workspace-content`: a release that carries a case workspace
  to a daemon on another host and back.
- `story:tree-daemon`: the last unheld story of the chain. Through it,
  `story:substrate-daemon-runner` (`Daemon`, the SDK dependency, `ess/intake/domains/confinement.yaml`
  and `ess/domains/engine.yaml`) comes first. No edge orders this story against
  `story:executable-descendants-held`, `story:ingress-routes-per-action` or
  `story:confinement-level-on-oci-and-pod`, each held by another blocker; they share the launch path
  in `crates/loom-intake-slice/src/confinement.rs`, and their order on it is settled by scope when
  this story's blocker clears.

## ESS first

`ess/intake/domains/confinement.yaml`: the `Daemon` variant of `EngineConfinement` gains an
`https` endpoint form with optional trust roots and server identity, and `workspace_root` becomes
optional for it. `ess/domains/engine.yaml`: the same members on `ConfinementSection`'s `Daemon`
form; `EndpointInvalid` admits an `https://` URL. On that commit `task drift` and
`task intake-drift` fail and `crates/loom-intake-slice/tests/remote_daemon.rs` does not compile.

## Acceptance

With the Substrate release that clears the blocker, against a test daemon with a TLS listener and
an identity authority as that release lets a test start one:

- `Cargo.lock` holds one copy of each Substrate crate, at that release, and
  `tests/host_git_hardening.rs` asserts that release.
- `run_child` of a program in a workspace holding a file over 1 MiB and an executable script runs
  the script on the daemon, and a file the program writes is in the host's workspace afterwards.
- A program that reads a declared read-only root sees its content on the daemon.
- With an `https://127.0.0.1:<port>` endpoint nothing listens on and a trust-roots file, the
  constructor is refused before any child starts, naming the endpoint.
- `task check` exits 0.

## Out of scope

Scheduling trees across several hosts. Choosing among several endpoints. The kubernetes driver.
