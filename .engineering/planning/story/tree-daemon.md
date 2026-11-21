---
format: aep.planning-md/3
id: story:tree-daemon
kind: story
status: draft
title: b10x-loom tree serve runs many case trees at once and the tree commands reach them through it
summary: Run slots, a queue, waits that hold no slot, restart from store and run log, tree stop after the current step
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:agent-node-run-records
- depends_on: story:named-egress-per-action
revision: 5
---
## Outcome

`b10x-loom tree serve --engine <path>` is a daemon, in Rust, that runs many case trees at the same
time. It listens on a Unix socket of the local user, `$XDG_RUNTIME_DIR/b10x-loom/tree.sock` unless
`--socket <path>` or `B10X_LOOM_TREE_SOCKET` names another; every `tree` command takes the same
flag and variable to find it. `tree open`, `run`, `approve`, `close`, `status`, `verify` and the new
`tree stop` reach a tree through the daemon when it is running and work on the store directly, as
before, when it is not (`tree stop` then exits non-zero naming `DaemonUnreachable`).

Through the daemon, `tree run <store>` submits the tree (`Submit`) and blocks until it stops, then
exits as a direct `tree run` would for that stop: 0 at a terminal outcome, 3 when it waits on a
person or on authority, 4 when stuck, 5 at a spent budget, and 6, a new status, when `tree stop`
stopped it. `tree run --detach` returns once the daemon accepted the tree, exit 0, printing its
state. `tree stop <store>` (`Stop`) ends a tree's run after its current step and leaves it
`Stopped`; a later `tree run` continues it.

Each tree keeps its own store directory and its one-writer rule: the daemon holds the store's lock
and names itself as the holder (`tree serve` and its socket). A direct command that finds the lock
held by a daemon is refused `StoreServed`, naming the socket, before it would report `StoreInUse`,
which stays for a lock held by another direct command. The daemon runs up to the engine
configuration's `serve.run_slots` trees at once and queues the rest; a tree that waits on a person
uses no run slot; a recorded approval makes the daemon continue that tree without a further
command.

After a restart the daemon continues every tree from its store and run log: a step that was started
and not finished is not counted as performed and is performed again. Before it performs any such
step it waits `lease_seconds` of the engine's daemon confinement (`story:substrate-daemon-runner`)
from its own start, so the Substrate exec of the interrupted step, whose lease nobody renews, has
been killed by the Substrate daemon first and no two copies of a step run.

The per-run wall and CPU totals of `story:action-limits-enforced` are one counter in
`crates/loom-tree`, summed from the used times in the `action finished` lines of a tree's run log
from a starting line: for a direct `tree run`, the invocation's first step, as before; for a served
tree, `tree open`, so its totals survive a restart.

Until `story:agent-session-through-substrate` lands, the daemon refuses at `tree open` and at
`Submit` a tree whose bindings hold an `agent` step, `AgentStepUnconfined`, naming the action; a
direct `tree run` runs it as `story:bound-agent-node` does.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "a daemon in Rust that runs many
protocol case trees at the same time (the `tree` commands talk to it, or it runs them itself)".
`story:tree-run-command` runs one tree per process and returns at every wait. The SDC's driver
has the same shape and its gap 15 names the missing half: "No resume ... an `action_started`
without `action_finished` is not counted as a run, so the next `run` executes that action again"
(`~/beyond10x/sdc/docs/driver.md`). Restart recovery, continuing on a recorded approval and a
`tree stop` that lets the current step finish follow from the daemon itself (epic `## Why`). Loom
has two long-lived hosts already, `agent serve` and the plugin host (`agent_serve_command`,
`crates/loom-cli/src/main.rs:241`; `plugin_command`, `main.rs:708`), and no daemon for cases. In
Substrate 0.21.0 a lapsed lease kills the exec and records it `Expired`
(`crates/substrate-daemon/src/app/service.rs:851-880`, `crates/substrate-wire/src/lib.rs:2105-2107`),
and a lease is at least 1000 ms (`lib.rs:409`). This story is the last unheld one of the epic and
owns the epic's acceptance test, `crates/loom-cli/tests/tree_daemon_limits.rs`.

## Waits on

- `story:named-egress-per-action`: the story before this one in the chain, the last unheld story
  to edit `ess/domains/engine.yaml` and `crates/loom-tree` before this one.
- `story:agent-node-run-records`: `TreeBudget`'s agent-call cap counted from the logs of the whole
  tree, which the daemon counts the same way; it edits `ess/domains/tree.yaml` and
  `crates/loom-cli/src/lib.rs` before this story.
- Through the chain: `story:engine-configuration-document` (`--engine`),
  `story:substrate-daemon-runner` (the exec lease, `lease_seconds`), `story:action-limits-enforced`
  (the per-run ceilings and the used times), `story:limit-hits-in-run-log` (`TreeStatus`'s
  limit-hit count, edited in `ess/domains/tree.yaml` before this story).

## ESS first

`ess/domains/tree.yaml`: `TreeDaemon` (socket, run slots), `ServedTree` with the lifecycle
`Queued`, `Running`, `Waiting`, `Stopped`, `Terminal`, the commands `Open`, `Submit`, `Approve`,
`Close` and `Stop`, the refusals `StoreServed` (socket), `DaemonUnreachable` and
`AgentStepUnconfined` (action), `TreeStop` gains `Stopped`, and `TreeStatus` gains `served`
(optional, the tree's `ServedTree` state). `ess/domains/engine.yaml`: `ServeSection` (`run_slots`)
and `EngineConfiguration` gains optional `serve`. The tree domain then has commands, so its
conformance scenarios are synthesized (`ess/scenarios/`, `crates/loom-conformance`, as
`story:tree-run-command`'s scope names for that case); on that commit `task drift` fails and the
new scenarios are red.

## Acceptance

With fixture trees, stub programs, no model and a managed Substrate daemon, each test daemon on a
socket under the test's directory named by `--socket`:

- Three trees that never wait, submitted with `tree run --detach` to a daemon with two run slots: at
  no instant of their run logs do all three have a step running, the third's first `action started`
  line follows the terminal line of one of the other two, and all three reach their terminal
  outcome.
- With two run slots, a tree `W` that reaches `Waiting`, and two trees submitted after it: both run
  at the same time (their run logs overlap in time) while `tree status` shows `W` served and
  `Waiting`. After `tree approve` on `W` the daemon continues `W` with no `tree run`, and `W`
  reaches its terminal outcome.
- `tree run` without `--detach` through the daemon exits 0 for a tree that reaches its terminal
  outcome and 3 for one that waits on a person.
- With the daemon running, `tree open` creates a tree that `tree status` lists as served; `tree
  close` with a legitimate outcome terminates its case and `tree verify` exits 0; both succeed while
  the daemon holds the store's lock, which a direct command could not take. After one byte of one
  log is changed, `tree verify` through the daemon exits non-zero naming the case and the line.
- `tree run` directly on a served store (with `--socket` naming a path nothing listens on) exits
  non-zero naming `StoreServed` and the daemon's socket, not `StoreInUse`; with no daemon running
  the same command works on the store as in `story:tree-run-command`.
- Under an engine configuration with `lease_seconds: 2`, the daemon killed with SIGKILL during a
  step and started again continues each tree: the interrupted action is performed again, the
  Substrate daemon's event log (`Client::events`) shows the interrupted step's exec `Expired` before
  the step's second exec started, no process of the first exec is left, no evidence is recorded
  twice for one step, and every run log verifies.
- A served tree stops with `BudgetSpent` naming `wall_seconds` once its steps since `tree open`
  have used the engine's per-run wall ceiling, also when those steps span a restart of the daemon;
  the same with `cpu_millis` for the per-run CPU ceiling. A direct `tree run` still counts from its
  own first step.
- `tree stop` lets the current step finish and leaves the tree `Stopped`; the blocking `tree run`
  of that tree exits 6, and a later `tree run` continues it.
- `tree open` and `tree run --detach` of a tree with an `agent` step are refused
  `AgentStepUnconfined` naming the action.
- The default socket is `$XDG_RUNTIME_DIR/b10x-loom/tree.sock`, created with mode 0600; the peer
  check refuses a connection whose credentials name another user (a unit test with a forged
  credential).
- One tree's stuck step (a stub that sleeps until stopped) and another tree's failed store write
  (its store directory made read-only) do not stop the other trees.
- The epic's acceptance checks pass in `crates/loom-cli/tests/tree_daemon_limits.rs` with the
  fixtures under `crates/loom-cli/tests/fixtures/daemon/`.
- `website/docs/reference/cli.md` is regenerated and a guide page shows a daemon running two
  trees with pasted output.
- `task check` exits 0.

## Out of scope

Running two cases of one tree at once. Interrupting a running step (`tree stop --now`). A network
listener, authentication beyond the local user, a hosted service. Placing trees on several hosts.
Reloading a changed engine configuration in a running daemon. Agent steps under the daemon
(`story:agent-session-through-substrate`).
