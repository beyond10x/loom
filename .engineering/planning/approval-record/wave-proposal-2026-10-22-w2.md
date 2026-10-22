---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-22-w2
kind: approval-record
status: draft
title: Wave 2026-10-22-w2 proposal accepted with no operator turn
relations:
- decides: story:checked-timeout-deadlines
- decides: story:run-record-names-agent-sessions
revision: 2
---
# Wave 2026-10-22-w2 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words: "Next: when ess 0.68.0 ships I send
you the tag; pin it then (dependency upgrade approved now under the grant). Meanwhile take
story:checked-timeout-deadlines and story:run-record-names-agent-sessions through acceptance and
implement them for 0.34.0, fanned out. Substrate gaps (writable mount outside /workspace, cwd,
path-bound read-only roots TOCTOU): file them as one story in substrate's store via the substrate
session, or tell me if none is running; do not work around them in loom."

An earlier message of the same session (2026-10-22): "Speed up a bit now. Yeah, instruct
everyone to use more subagents." From now on, fan out: one implementor sub-agent per independent
unit in its own worktree, with adversary passes in parallel.

| unit | serves | surface | adversary |
|---|---|---|---|
| `story:checked-timeout-deadlines` | `vision:governed-autonomy` | `crates/loom-intake-slice/src/bindings.rs`, `crates/loom-intake-slice/src/executor.rs`, `crates/loom-connectors/src/cli.rs` | security review |
| `story:run-record-names-agent-sessions` | `vision:governed-autonomy` | `ess/intake/domains/events.yaml`, `generated/rust/intake/`, `crates/loom-session-executor`, `crates/loom-cli`, `docs/contracts/run-events-v1.json` | adversary |

- Decisions taken in planning:
  - D7: the session summary comes from `metaharness::protocol::summarize` with the metaharness
    crates moved to `0.13.0`, never from a `metaharness` binary, because Loom's tests start no
    harness program.
  - D8: the two units run in parallel in their own trees; they share only `CHANGELOG.md` and
    `AGENTS.md`, merged one at a time.
  - D9: no substrate session is running, so the Substrate gaps go back to the orchestrator, and
    Loom works around none of them.
