---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-22-w3
kind: approval-record
status: draft
title: Wave 2026-10-22-w3 proposal accepted with no operator turn
relations:
- decides: story:agent-domain
- decides: story:runtime-events
- decides: story:tool-decisions-read-back
revision: 2
---
# Wave 2026-10-22-w3 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-22): "D: first SDK wave A
(agent-domain, runtime-events, tool-decisions-read-back; 3 Opus), approved under the grant, after
PR #105 is green and merged. Q2=C accepted. Also approved: file a draft story for Loom release
binaries (GitHub Release assets per target, the download the tutorial and M3 image need), next
wave candidate. Report the wave merge sha."

| unit | serves | surface | review |
|---|---|---|---|
| `story:agent-domain` | `vision:governed-autonomy` | `ess/domains/agent.yaml`, `ess/system.yaml`, `generated/rust/loom/`, `crates/loom-sdk`, `examples/agents/operator.yaml`, `website/docs/reference/sdk-stability.md` | adversary |
| `story:runtime-events` | `vision:governed-autonomy` | `ess/commission/`, `generated/rust/commission/`, `crates/loom-commission/src/runtime.rs`, `crates/loom-commission-testkit`, `crates/loom-sdk` | security review |
| `story:tool-decisions-read-back` | `vision:governed-autonomy` | `ess/domains/session.yaml`, `generated/rust/loom/`, `crates/loom-session-executor` | adversary |

- Decisions taken in planning:
  - D1: Q2 of the Kubernetes design is C, as the orchestrator accepted ("Q2=C accepted").
  - D2: the three units run in parallel in their own trees, one Opus implementor each.
    `agent-domain` and `tool-decisions-read-back` both change `ess/` and `generated/rust/loom/`;
    the second to merge regenerates with `task generate`. `agent-domain` and `runtime-events` both
    add re-exports to `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/tests/host_ports.rs` and
    `website/docs/reference/sdk-stability.md`; merged one at a time.
  - D3: `story:release-binaries` is filed as a draft under `epic:loom-sdk-for-products`, a
    candidate for the next wave, not this one.

- Decisions taken while the wave ran (2026-10-22):
  - D4: `story:agent-domain`'s decoder `loom_sdk::agent::parse` is hand-written over the generated
    types, as `models.rs` is, because ESS 0.67 generates no decoder; `RunBudget` and `StopPolicy`
    in `loom.agent` are field-for-field copies of Commission's, because ESS cannot reference
    another system's types.
  - D5: `story:tool-decisions-read-back` puts the decisions on `SessionReport` only; the step's
    observation carries `transcript_digest`, which names the record they were read from. Adding
    them to `ExecutorStep` changes `ess/commission/` and about 45 construction sites in 22 files,
    outside the story's scope; it is filed as a draft story for a later wave.
  - D6: disk at 27G free (orchestrator, 2026-10-22: "Run `worktree discard-cache` on each unit
    tree as soon as it merges into the wave branch, not at the end of the wave."): each unit's
    cache is discarded at its merge, and the wave's one full gate runs after the last.
  - D7: `loom_sdk::agent::parse` bounds nesting by refusing more than `MAX_FLOW_OPENERS` (4096) `[`
    or `{` bytes anywhere in a document, checked before loading, because libyaml's flow scan is
    quadratic before any depth check runs; `models::parse_config` is not changed in this wave.
  - D8: on a resume whose case moved after its take, no `StepStarted` is emitted, since nothing is
    invoked (security finding on `story:runtime-events`); `EffectInvoked` stays before the
    governor observes the effect, as the specification allows.
