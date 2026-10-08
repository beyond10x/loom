---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-08-w7
kind: approval-record
status: draft
title: Wave 2026-10-08-w7 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:governor-restart-durability
- decides: story:control-plane-storage
revision: 1
---
# Wave 2026-10-08-w7 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. The proposal named two units and was accepted
as proposed, with one condition on `story:governor-restart-durability` (declare the held case in
the ESS specification before the codec; see that story's `## Open`).

| unit | serves | surface | scope | adversary |
|---|---|---|---|---|
| `story:governor-restart-durability` | `vision:O1` | `ess/` (held-case noun), `generated/rust/loom/`, `crates/loom-governor` (new `file_store.rs`, restart test) | cited and inferred | one pass (on-disk format, restart round trip) |
| `story:control-plane-storage` | `vision:O1` | `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`, `crates/loom-commission` (`runtime.rs`, `outcome.rs`), `crates/loom-commission-conformance`, `crates/loom-commission-testkit/tests`, `website/docs/reference/commission` | cited and inferred | one pass (breaking variants on `StartRunOutcome`, `SuspendRunOutcome`, `LoopFailure`) |

- Overlap: none by file. `story:governor-restart-durability` writes `ess/` and
  `generated/rust/loom/`; `story:control-plane-storage` writes `ess/commission/` and
  `generated/rust/commission/`. Only `story:control-plane-storage` edits `CHANGELOG.md` under
  Unreleased first; the other unit adds its own line, merged by the coordinator.
- Left out: stories behind an open decision or upstream blocker
  (`story:incident-investigation-open`, `story:revalidation-membership-conformance`,
  `story:compaction-target-bound`); the fast-selector chain (disk).
- Pre-flight: `/` 39G free, load 27.07, no managed worktree before the wave. At most two unit
  builds at once with `CARGO_INCREMENTAL=0`, each into its own tree's `target/`; 10G for the wave;
  under 25G free, building stops.
- Commits authorised: 1 opening store commit, the unit commits on each `impl/<story>` branch, 1
  merge per unit into `wave/2026-10-08-w7`, 1 closing store commit, the merge into `main` through
  the pull request, then the release 0.10.0 through the repository's release process.

## Units

| unit | tree id | branch | stage |
|---|---|---|---|
| governor-restart-durability | loom-20261008-w7-gov | impl/governor-restart-durability | dispatched |
| control-plane-storage | loom-20261008-w7-cps | impl/control-plane-storage | dispatched |

Integration tree `loom-20261008-w7-int`, branch `wave/2026-10-08-w7`. Each unit's build directory
is its tree's `target/`; its scratch root is `~/.cache/loom-w7/<unit>/`.
