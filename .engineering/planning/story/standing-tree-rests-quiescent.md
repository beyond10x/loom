---
format: aep.planning-md/3
id: story:standing-tree-rests-quiescent
kind: story
status: draft
title: A tree opened --standing rests Quiescent with no run slot when nothing is to do and nothing is wrong, and names its next expiry
summary: TreeHorizon, exit 7, ServedTree Quiescent with no slot, next_expiry from held records and max_age with no clock read; creates the area fixture; bounded trees still exit 4
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:tree-daemon
revision: 3
---
## Outcome

`tree open --standing` opens a tree whose horizon is `Standing`; without the flag it is `Bounded`,
as every tree is today. The store keeps the horizon beside the root's case file, and `tree status`
prints it.

This story is written against the per-tree stop of `story:tree-daemon` and the sequential run of
`story:tree-run-command`, and lands before `story:concurrent-cases-in-the-tree-daemon`, which
places `Quiescent` among its per-case states. A standing tree rests `Quiescent` where a bounded
tree would stop `Stuck`: no case has a useful action, none waits on a person, authority or a
decision request, and in addition every case that is not terminated has no open obligation and no
legitimate outcome. A direct `tree run` then exits 7, a new status (accepted by session `sdc` on
2026-11-21), and prints `Quiescent` with the tree's next expiry. Through the daemon the served tree
is `Quiescent` and holds no run slot, as a tree that waits on a person holds none; a blocking `tree
run` through the daemon exits 7; after a restart of the daemon it is `Quiescent` again, derived
from its root's last `stopped` line, and no step of it starts. A standing tree with a case that has
an open obligation and nothing useful stops `Stuck`, exit 4, as a bounded tree in either state
does; a bounded tree never rests `Quiescent`.

The next expiry is the earliest instant at which a record held by any case of the tree, applying
at the evaluation's instant and of a kind with a `max_age`, stops applying: its `observed_at` plus
the kind's `max_age` plus one second, by the governor's freshness rule. A new public function of
`crates/loom-governor`, `next_expiry(case, at)`, returns it per case from the case's held records
and its protocol's IR, reading no clock; the tree takes the least over its cases, or none. `tree
status` prints it per tree.

Nothing changes in how a case completes: the governor terminates a case only through exactly one
legitimate outcome, so a standing protocol with no legitimate outcome is never closed by Loom.

The fixture directory `crates/loom-cli/tests/fixtures/area/` is created here, beside the existing
`fixtures/catalog-route/` and `fixtures/session/`: the protocol `area.protocol.yaml` (`area/1`),
its bindings `area.bindings.yaml`, the stub programs under `area/bin/`, and the engine files
`engine-daemon-1.yaml` (one run slot). Later stories of the epic add files to it by name and change
none of these.

## Why

"a standing case never ends done, it is *quiescent* (nothing to triage, nothing accepted and
undelivered, the release observed healthy within its max age) and becomes active again on new
evidence; only `retired` ends it" (`~/beyond10x/sdc/docs/orchestration.md` § 0, answer 3);
"Quiescence is the runtime's resting state: no useful action, nothing awaiting a person. A standing
tree in quiescence holds no run slot" (§ 3); "quiescent: ... every obligation discharged, no
useful action; the case stays open" (§ 3, table). The draft protocol declares the horizon as an
input fixed at opening (`~/beyond10x/sdc/protocol/draft/orchestrate.protocol.yaml:26-30`) and
outcomes `done` and `retired` only.

Today such a tree reads as stuck: `story:tree-run-command` § Acceptance, "A tree with no useful
action, no legitimate outcome and nothing awaiting approval exits 4 and names the case."
`story:tree-daemon` § Outcome frees a slot only for a wait: "a tree that waits on a person uses no
run slot". Blocker `loom-standing-trees` also asks that "the governor never completes a case that
has no legitimate outcome": already held (`AGENTS.md` § Checks that hold the rules,
`crates/loom-governor/tests/completion_rule.rs`). The expiry rule is the governor's `expired`
(`crates/loom-governor/src/lib.rs:1938-1954`: expired when the instant is later than `observed_at`
by more than `max_age`), so the next expiry needs no Canon change at 0.8.0.

## Waits on

- `story:tree-daemon`: `ServedTree` in `ess/domains/tree.yaml`, the slot accounting and the
  restart path this story extends; the last story of the daemon epic's chain to edit
  `ess/domains/tree.yaml` and `crates/loom-tree` before this one.
- Through it, `story:tree-run-command` (`TreeStop`, the exit table, `tree open`, `tree status`) and,
  through `story:record-copy-between-cases`, `story:child-case-runtime`, which edits
  `crates/loom-governor/src/lib.rs` before this story's `next_expiry`. So this story, like every
  story of the epic, waits on Canon 0.9.0 through `story:canon-0-9-0-pin`.

Stories of this epic that come after this one on shared files, each with a `depends_on` edge
naming the file: `story:run-caps-per-arrival` (`ess/domains/tree.yaml` `TreeStop`,
`crates/loom-tree`, `crates/loom-cli/src/lib.rs`, `fixtures/area/`), and through it every later
story of the epic.

## ESS first

`ess/domains/tree.yaml`: `TreeHorizon` (`Bounded`, `Standing`); `TreeRun` gains `horizon`;
`TreeStop` gains `Quiescent` (optional `next_expiry`); the `ServedTree` lifecycle gains
`Quiescent`; `TreeStatus` gains `horizon` and optional `next_expiry`. On that commit `task drift`
fails, the tree domain's scenarios are red, and `crates/loom-cli/tests/tree_standing.rs` does not
compile.

## Acceptance

With the fixture `crates/loom-cli/tests/fixtures/area/` (`area/1`: an artifact `intake`, an
artifact `notes` that no action reads (placed, for the wake checks of
`story:standing-tree-wakes-at-a-host-instant`), an evidence kind `health` with `max_age: 1h`, an obligation discharged by `triage` about the current
`intake`, outcomes `done` legitimate only on a decision and `retired`), stub programs, no model
(`crates/loom-cli/tests/tree_standing.rs`):

- `tree open --standing` then `tree run --at T` performs triage and the health observation, exits
  7, prints `Quiescent` and the next expiry `T + 3601 s`; `tree status` prints `Standing` and that
  instant.
- The same fixture opened without `--standing` exits 4 at the same point, naming the case.
- A standing tree whose obligation is open and nothing useful exits 4 `Stuck`, not 7.
- Through a daemon under `engine-daemon-1.yaml` (one run slot): the blocking `tree run` of the
  standing tree exits 7 and `tree status` shows it served and `Quiescent`; a bounded tree submitted
  after it runs at once and reaches its terminal outcome; the daemon killed and started again shows
  the standing tree `Quiescent` and its run logs gain no `action started` line.
- `next_expiry` (`crates/loom-governor/tests/next_expiry.rs`): two held records of a kind with
  `max_age: 30m` observed at 10:00 and 10:10 give 10:30:01; a record no longer bound to the current
  revision, an expired record and a record of a kind without `max_age` give none; under a host
  clock fixed at 1970 the result is the same.
- A standing root whose `retired` outcome becomes legitimate is terminated through it and the tree
  exits 0.
- `website/docs/reference/cli.md` is regenerated and the exit table of `tree run` lists 7.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, generated `generated/rust/loom/src/tree.rs`, `ess/scenarios/` (tree
domain); `crates/loom-governor/src/lib.rs` (`next_expiry`),
`crates/loom-governor/tests/next_expiry.rs` (new); `crates/loom-tree` (horizon, rest, the daemon's
slot release); `crates/loom-cli/src/lib.rs` (`--standing`, the exit table);
`crates/loom-cli/tests/tree_standing.rs` (new); `crates/loom-cli/tests/fixtures/area/` (new:
`area.protocol.yaml`, `area.bindings.yaml`, `bin/`, `engine-daemon-1.yaml`);
`website/docs/reference/cli.md` (regenerated); `CHANGELOG.md`.

## Out of scope

Waking a quiescent tree (`story:standing-tree-wakes-at-a-host-instant`). Run caps
(`story:run-caps-per-arrival`). Per-case scheduling states
(`story:concurrent-cases-in-the-tree-daemon`). Changing the completion rule. A protocol-declared
horizon: Loom reads none from Canon; the flag is the host's.
