---
title: Run a tree of cases
sidebar_position: 7
description: Open a tree of protocol cases with b10x-loom tree open, drive it with tree run, answer what it waits on with tree approve or tree close, and read and check it with tree status and tree verify.
lede: A tree of cases lives in one store directory. tree run takes one step of one case at a time through the action's bound executor until the root ends or the tree waits on a person, and it prints the exact command that answers each wait.
source: crates/loom-tree/src/lib.rs, crates/loom-tree/src/run.rs, crates/loom-tree/src/approve.rs, crates/loom-tree/src/close.rs, crates/loom-tree/src/status.rs, crates/loom-tree/src/log.rs, crates/loom-cli/src/tree.rs, crates/loom-cli/tests/tree_run.rs, ess/domains/tree.yaml
---

# Run a tree of cases

Unreleased. `b10x-loom tree` keeps a tree of protocol cases in one store directory: `tree open`
opens the root case, `tree run` drives every case of the tree, `tree approve` and `tree close`
answer what a run waits on, and `tree status` and `tree verify` read the store.

The example is the fixture of `crates/loom-cli/tests/tree_run.rs`: the protocol `tour.leg`
(registered `tour-leg@1`), one leg of a walking tour, and the bindings files and stub programs of
`crates/loom-cli/tests/fixtures/tree-two-level/`, copied into the directory `~/tour`, which is
the workspace. A planner stub adds a stop to `route.txt` while a check finds the route short, a
guide approves the route and a ticket is issued under the capability `guide.issue`. The output
below is pasted from that run. Where a command ends in `| jq -c`, the filter shortens the one JSON
object `b10x-loom` prints; `(exit N)` is the exit status of `b10x-loom`.

## Open the tree

```console
$ b10x-loom protocols add tour-leg@1 --file crates/loom-cli/tests/fixtures/tree-two-level/protocols/tour-leg.yaml
installed tour-leg@1
(exit 0)
```

```console
$ b10x-loom tree open --protocol tour-leg@1 --store store --bindings tour-leg.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"tour-leg@1","root":"root","store":"~/tour/store","workspace":"~/tour"}
(exit 0)
```

`tree open` reads everything before it writes: the store must be absent or empty, the workspace
an existing directory, and every bindings file of the tree must decode and check against its
protocol. A root's bindings file names each child's file in the `bindings` member of an `open`
executor, relative to its own directory. A binding with an `agent` executor anywhere in the tree
is refused before the store is made.

## Run it until it waits

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:01:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":5,"detail":null,"decisions":[{"kind":"ApproveAction","decides":"route.approve of case root at revision 3"}]}
b10x-loom: tree waits: case root: route.approve of case root at revision 3
  run: b10x-loom tree approve --store ~/tour/store --case root --capability guide.approve --principal guide --action route.approve
(exit 3)
```

`tree run` takes one step at a time with the deterministic selector, every open child before its
parent's next step, and performs it through the action's bound executor: here the check, then the
planner, which the check's `invalid` result sends back until the route holds two stops. It stops
at a terminal root (exit 0), when it waits on a person or on an approval (exit 3), when a case is
stuck (exit 4) or at `--max-steps` or `--max-wall-seconds` (exit 5). Each wait names the case,
what is decided and the command that decides it.

## Answer what it waits on

`route.approve` is bound to the person `guide`: `tree approve --action` records the approval and
then takes the action as that principal, so the record its binding declares is produced by the
guide about the route's revision now.

```console
$ b10x-loom tree approve --store store --case root --capability guide.approve --principal guide --action route.approve --at 2026-11-22T08:02:00Z
{"at":"2026-11-22T08:02:00Z","capability":"guide.approve","case":"root","case_revision":3,"decision":"Allow","principal":"guide"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:03:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingAuthority","steps":0,"detail":null,"decisions":[{"kind":"Approve","decides":"ticket.issue of case root at revision 3"}]}
b10x-loom: tree waits: case root: ticket.issue of case root at revision 3
  run: b10x-loom tree approve --store ~/tour/store --case root --capability guide.issue --principal operator
(exit 3)
```

`ticket.issue` is a program that needs the capability `guide.issue`. `tree approve` without
`--action` records the grant for the case at its current revision; the next run issues the ticket
and the case ends through its one legitimate outcome:

```console
$ b10x-loom tree approve --store store --case root --capability guide.issue --principal operator --at 2026-11-22T08:04:00Z
{"at":"2026-11-22T08:04:00Z","capability":"guide.issue","case":"root","case_revision":3,"decision":"Allow","principal":"operator"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:05:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"Terminal","steps":1,"detail":"passed","decisions":[]}
b10x-loom: tree run: Terminal: case root: passed
(exit 0)
```

## Read the tree

```console
$ b10x-loom tree status --store store | jq -c '.cases[] | {case, protocol, outcome}, (.runs[] | {action, attempt, exit}), (.approvals[] | {capability, action, actor}), (.evidence[] | {kind, result, producer})'
{"case":"root","protocol":"tour-leg@1","outcome":"passed"}
{"action":"route.check","attempt":1,"exit":1}
{"action":"route.write","attempt":1,"exit":0}
{"action":"route.check","attempt":1,"exit":1}
{"action":"route.write","attempt":2,"exit":0}
{"action":"route.check","attempt":1,"exit":0}
{"action":"ticket.issue","attempt":1,"exit":0}
{"capability":"guide.approve","action":"route.approve","actor":"guide"}
{"capability":"guide.issue","action":null,"actor":"operator"}
{"kind":"route_check","result":"invalid","producer":"route-check"}
{"kind":"route_shape","result":"short","producer":"route-check"}
{"kind":"route_check","result":"invalid","producer":"route-check"}
{"kind":"route_shape","result":"short","producer":"route-check"}
{"kind":"route_check","result":"valid","producer":"route-check"}
{"kind":"route_shape","result":"two-stops","producer":"route-check"}
{"kind":"route_approval","result":"approved","producer":"guide"}
{"kind":"ticket_issued","result":"issued","producer":"ticket-office"}
(exit 0)
```

`tree status` prints `loom.tree-status/1`: per case its link, protocol, outcome or stop, its
records in the order received, its program runs with their attempts, its approvals and the records
a `copy` binding copied into it. `tree status` and `tree verify` read the store and never hold it,
so a live view can read the tree while a run holds it.

## Choose between two outcomes

A case for which more than one outcome is legitimate is not ended by `tree run`. The fixture
protocol `fork` has the outcomes `kept` and `left`, both legitimate once the road is seen:

```console
$ b10x-loom protocols add fork@1 --file crates/loom-cli/tests/fixtures/tree-two-level/protocols/fork.yaml
installed fork@1
(exit 0)
```

```console
$ b10x-loom tree open --protocol fork@1 --store fork-store --bindings fork.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"fork@1","root":"root","store":"~/tour/fork-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store fork-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":1,"detail":null,"decisions":[{"kind":"Close","decides":"close case root through kept"},{"kind":"Close","decides":"close case root through left"}]}
b10x-loom: tree waits: case root: close case root through kept
  run: b10x-loom tree close --store ~/tour/fork-store --case root --outcome kept
b10x-loom: tree waits: case root: close case root through left
  run: b10x-loom tree close --store ~/tour/fork-store --case root --outcome left
(exit 3)
```

```console
$ b10x-loom tree close --store fork-store --case root --outcome left --at 2026-11-22T08:00:00Z
{"at":"2026-11-22T08:00:00Z","case":"root","outcome":"left"}
(exit 0)
```

An outcome that is not legitimate for the case now is refused `OutcomeNotLegitimate`, and the
store keeps every byte.

## A writer's handoff

A writer whose binding names a `handoff` hands the case to a person when it has done what it may:
when no step is useful and nothing awaits approval, the run prints the handoff text as the
decision and `tree run` as its command. In `tour-leg-handoff.bindings.yaml` the planner writes
nothing and nothing sends it back:

```console
$ b10x-loom tree open --protocol tour-leg@1 --store handoff-store --bindings tour-leg-handoff.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"tour-leg@1","root":"root","store":"~/tour/handoff-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store handoff-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":2,"detail":null,"decisions":[{"kind":"Handoff","decides":"Add a second stop to route.txt, then run the tree again."}]}
b10x-loom: tree waits: case root: Add a second stop to route.txt, then run the tree again.
  run: b10x-loom tree run --store ~/tour/handoff-store
(exit 3)
```

## Verify the run logs

```console
$ b10x-loom tree verify --store store
{"cases":1}
(exit 0)
```

Each log is checked against its chain and its last line against the head the tree kept beside it,
so a change to any byte is named with the case and the line. After `"exit":1` became `"exit":0`
in the line with `seq` 2:

```console
$ b10x-loom tree verify --store store
{"cases":1,"failure":{"case":"root","detail":"BrokenChain: prev is not the SHA-256 of line 2: the line before it changed","line":3}}
b10x-loom: tree verify: case root line 3: BrokenChain: prev is not the SHA-256 of line 2: the line before it changed
(exit 3)
```

## One writer

A command that writes holds `tree.lock` in the store for the whole command. A second one is
refused `StoreInUse`, names the process that holds the store and writes nothing.
