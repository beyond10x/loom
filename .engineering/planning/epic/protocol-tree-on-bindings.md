---
format: aep.planning-md/3
id: epic:protocol-tree-on-bindings
kind: epic
status: proposed
title: A tree of protocol cases runs on Loom from bindings files to a terminal outcome or the next approval
summary: 'Loom replaces the SDC''s own driver: bindings, confined executors, child cases, a deterministic selector, a tree command and a chained run log'
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:38Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

An application hands Loom a set of Canon protocols, one bindings file per protocol and a case
store, and Loom runs the whole tree of cases: it selects each next action without a model, runs
a bound program inside the configured Substrate adapter, turns the result into evidence through
the binding's result table, opens child cases and writes their outcomes back into the parent, and
stops at a terminal outcome or at the next decision a person owes, naming the command that
records it. Every step is in a hash-chained run log, and the tree's state is printed as JSON for
a live view. The application owns data only: protocols, bindings and result tables.

## Why

The SDC prototype runs its eight protocols on a driver of its own, `sdc-drive` (943 lines,
`~/beyond10x/sdc/driver/src/main.rs`). `~/beyond10x/sdc/docs/boundary.md` reads that driver row by
row: about 22 of its 31 responsibilities are generic runtime that Loom has or must have (case
store, revisions, event log, evaluation, selection, budgets, tool invocation, evidence, authority,
termination), and it is the fourth copy of that runtime beside Loom, takt and `metaharness aep
drive`. Atlas ADR 0090 and ADR 0096 name Loom as the one governed runtime. What Loom lacks, read at
`3cb82b7c` (0.57.2):

- Canon is pinned at 0.8.0 (`crates/loom-governor/Cargo.toml:15`); `case_open`, `case_outcome` and
  the `children` form arrived in Canon 0.9.0.
- A binding is an action and an argv only (`ess/intake/domains/bindings.yaml:30-36`), for one
  installed protocol that declares the `intent` artifact alone
  (`crates/loom-intake-slice/src/bindings.rs:150-174`). There is no result table, no executor
  kind, no budget and no agent node.
- Bound commands start unconfined (`crates/loom-intake-slice/src/bindings.rs:414-439`), gated only
  in the command line (`crates/loom-cli/src/main.rs:983-1003`), although a Substrate runner exists
  (`crates/loom-intake-slice/src/confinement.rs:475`). Substrate is pinned at 0.14.0; 0.21.0 is
  released.
- The only deterministic selector is "used only for tests/examples"
  (`crates/loom-executor/src/selection.rs:140-163`), and a catalogue entry carries neither the
  action's effect class nor the evidence it may produce.
- The governor holds no link between cases and records no termination
  (`crates/loom-governor/src/lib.rs:1333-1350, 1661`).
- `b10x-loom run` takes one intent and opens one case; no command takes a case store.
- Run events are held in memory (`crates/loom-commission/src/ports/events.rs:26-35`) or written as
  `run-events-v1` lines with no sequence number and no digest of the previous line.

## Stories

In dependency order. `story:child-launches-through-confinement-adapters` is an existing draft under
`epic:loom-sdk-for-products`; this epic depends on it and does not re-plan it (only its stale
"Waits on" section was rewritten, and it gained `depends_on story:substrate-0-21-pin`).

1. `story:substrate-0-21-pin`
2. `story:confinement-opens-from-engine-config`
3. `story:bound-commands-run-confined`
4. `story:bindings-document`
5. `story:bound-program-result-table`
6. `story:bound-artifact-revisions`
7. `story:bound-agent-node`
8. `story:deterministic-default-selector`
9. `story:hash-chained-run-log`
10. `story:canon-0-9-0-pin` (blocked by `upstream-blocker:engineering-protocols-on-canon-0-9-0`)
11. `story:child-case-runtime`
12. `story:record-copy-between-cases`
13. `story:tree-run-command`
14. `story:agent-node-run-records`

Some `depends_on` edges order two stories only because both change one file
(`ess/domains/bindings.yaml`, `ess/system.yaml`, `ess/commission/domains/responsibility.yaml`,
the pin manifests); each such story says so in its `## ESS first` or `## Why`.

## Acceptance

`~/beyond10x/sdc/scripts/run-full-cycle.sh`, with its agent nodes bound as stub programs and with
`sdc-drive` replaced by the Loom command of `story:tree-run-command`, runs the eight SDC protocols
to the root outcome `operating`, and every case ends through the same outcome as in the recorded
`sdc-drive` run (`~/beyond10x/sdc/runs/full-cycle-1/`: intake, direction, spec, plan, story,
release and epic `passed`, root `operating`). Every program of that run starts through the
configured Substrate adapter, and `tree verify` exits 0 on the store. The SDC session runs this
check and reports its output; Loom's own gate holds a three-level fixture tree with no
engineering vocabulary (`story:tree-run-command`).

## Not in scope

- Confining an agent session by Substrate. An `agent` executor's shipped adapter starts Claude
  Code through the session executor and metaharness, under metaharness's tool policy and outside
  the Substrate adapter (`story:bound-agent-node`, `story:confinement-opens-from-engine-config`
  § Out of scope). The acceptance run binds its agent nodes as stub programs.
- Changes to Canon: usefulness in the decision, authority bound to a revision and a principal,
  `revision: current` on the `children` form. They are Canon requests
  (`~/beyond10x/sdc/docs/canon-requests.md`); Loom computes what it needs until Canon ships them.
- Moving the SDC protocols into engineering-protocols, and converting the SDC bindings files and
  tools: both are the SDC's.
- JavaScript or Python bindings of the SDK, and a protocol builder that emits `protocol/1`.
- Moving `loom-operator`'s Jobs, NetworkPolicy and secret staging onto Substrate.
- A replay command that re-evaluates a logged case and compares digests
  (`boundary.md` row 30). The run log of `story:hash-chained-run-log` is what it would read.
- Report checks that name ESS or git (spec digest match, clean tree): they are SDC data on top of
  the report-field result table.
- Accepting `loom.bindings/1` in `b10x-loom run --action-bindings`.

## Source

Session `sdc`, 2026-11-21, seven requests. Documents read: `~/beyond10x/sdc/docs/boundary.md`,
`layers.md`, `loom-sdk-and-agent-node.md`, `driver.md`. The same session reviewed the
`loom.bindings/1` member list on 2026-11-21.
