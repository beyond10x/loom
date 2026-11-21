---
format: aep.planning-md/3
id: story:tree-daemon
kind: story
status: draft
title: b10x-loom tree serve runs many case trees at once and the tree commands reach them through it
summary: Run slots, a queue, waits that hold no slot, restart from store and run log, tree stop
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:agent-node-run-records
revision: 1
---
## Outcome

`b10x-loom tree serve` is a daemon, in Rust, that runs many case trees at the same time. It
listens on a Unix socket of the local user; `tree open`, `run`, `approve`, `close`, `status` and
`verify` reach a tree through it when it is running and work on the store directly, as before,
when it is not. Each tree keeps its own store directory and its one-writer rule: the daemon holds
the store, and a direct `tree run` on a served store is refused naming the daemon. The daemon
runs up to a configured number of trees at once and queues the rest; a tree that waits on a
person uses no run slot; a recorded approval makes the daemon continue that tree without a
further command. After a restart the daemon continues every tree from its store and run log, and
a step that was started and not finished is not counted as performed. `tree stop` ends a tree's
run after its current step, or at once with the step's process tree stopped.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "a daemon in Rust that runs many
protocol case trees at the same time (the `tree` commands talk to it, or it runs them itself)".
`story:tree-run-command` runs one tree per process and returns at every wait. The SDC's driver
has the same shape and its gap 15 names the missing half: "No resume ... an `action_started`
without `action_finished` is not counted as a run, so the next `run` executes that action again"
(`~/beyond10x/sdc/docs/driver.md`). Loom has two long-lived hosts already, `agent serve` and the
plugin host (`crates/loom-cli/src/lib.rs:507-537`, `321-328`), and no daemon for cases.

## ESS first

`ess/domains/tree.yaml`: `TreeDaemon` (socket, run slots), `ServedTree` with the lifecycle
`Queued`, `Running`, `Waiting`, `Stopped`, `Terminal`, the commands `Submit`, `Stop` and
`Approve`, and the refusals `StoreServed` and `DaemonUnreachable`. This domain has commands, so
its conformance scenarios are synthesized; on that commit `task drift` fails and the new
scenarios are red.

## Acceptance

With fixture trees, stub programs and no model:

- Three trees submitted to a daemon with two run slots: two run at once (their run logs overlap
  in time), the third starts when one waits or ends, and all three reach their terminal outcome.
- A tree that waits on an approval shows `Waiting` in `tree status`; after `tree approve` the
  daemon continues it with no `tree run`.
- `tree run` directly on a served store exits non-zero naming `StoreServed`; with no daemon
  running the same command works on the store as in `story:tree-run-command`.
- The daemon killed with SIGKILL during a step and started again continues each tree: the
  interrupted action is performed again, no evidence is recorded twice for one step, and every
  run log verifies.
- `tree stop` lets the current step finish and leaves the tree `Stopped`; `tree stop --now`
  stops the step's process tree, and the log holds the step as refused.
- The socket is created with mode 0600 in the user's runtime directory, and a connection from
  another user is refused.
- One tree's stuck step or failed store write does not stop the others.
- `website/docs/reference/cli.md` is regenerated and a guide page shows a daemon running two
  trees with pasted output.
- `task check` exits 0.

## Out of scope

Running two cases of one tree at once. A network listener, authentication beyond the local user,
a hosted service. Placing trees on several hosts.
