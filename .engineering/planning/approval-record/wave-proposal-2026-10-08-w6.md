---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-08-w6
kind: approval-record
status: draft
title: Wave 2026-10-08-w6 proposal accepted with no operator turn
relations:
- decides: story:connector-read-performed
- decides: story:conformance-declared-coverage
- decides: story:incident-response-slice
- decides: story:software-change-slice
revision: 1
---
# Wave 2026-10-08-w6 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. The proposal named four units and was accepted
as proposed.

| unit | serves | surface | scope | adversary |
|---|---|---|---|---|
| `story:connector-read-performed` | `vision:O3` | `ess/commission/domains/responsibility.yaml`, `crates/loom-commission/src/ports/connector.rs`, `crates/loom-connectors` | inferred | one pass (effect path; `Performed` without an attempt) |
| `story:conformance-declared-coverage` | `vision:O1` | `Taskfile.yml` synthesis flag, `crates/loom-conformance/tests/conform.rs`, `crates/loom-commission-conformance/tests/conform.rs` | inferred | none (test gate only) |
| `story:incident-response-slice` | `vision:O1` | `crates/loom-governor/tests/` (new test file, `inc-492` fixture copied from engineering-protocols `a7d02b4`) | inferred | none (tests only) |
| `story:software-change-slice` | `vision:O1` | `crates/loom-governor/tests/` (new test file), `crates/loom-governor/Cargo.toml` dev-dependencies | inferred | none (tests only) |

- Overlap: the two slice stories both add files under `crates/loom-governor/tests/`; each adds its
  own file. Only `story:software-change-slice` may edit `crates/loom-governor/Cargo.toml`.
- Left out: `story:compaction-target-bound` (its acceptance option is not chosen); the fast-selector
  stories (four units; disk); stories behind an open decision or upstream blocker.
- Decisions recorded with this wave: `decision-blocker:governor-restart-store` and
  `decision-blocker:run-storage-failure-spec`, both cleared.
- Pre-flight: `/` 51G free, load 3.65, no managed worktree before the wave. Each tree builds only
  its touched crates with `CARGO_INCREMENTAL=0` into its own `target/`.
- Commits authorised: 1 opening store commit, the unit commits on each `impl/<story>` branch, 1
  merge per unit into `wave/2026-10-08-w6`, 1 closing store commit, the merge into `main` through
  the pull request, then the release through the repository's release process.

## Units

| unit | tree id | branch | stage |
|---|---|---|---|
| connector-read-performed | loom-20261008-w6-read | impl/connector-read-performed | dispatched |
| conformance-declared-coverage | loom-20261008-w6-conf | impl/conformance-declared-coverage | dispatched |
| incident-response-slice | loom-20261008-w6-inc | impl/incident-response-slice | dispatched |
| software-change-slice | loom-20261008-w6-chg | impl/software-change-slice | dispatched |

Integration tree `loom-20261008-w6-int`, branch `wave/2026-10-08-w6`. Each unit's build directory
is its tree's `target/`; its scratch root is `~/.cache/loom-w6/<unit>/`.
