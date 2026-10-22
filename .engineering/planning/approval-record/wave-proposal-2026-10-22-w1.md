---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-22-w1
kind: approval-record
status: draft
title: Wave 2026-10-22-w1 proposal accepted with no operator turn
relations:
- decides: story:breach-choice-per-limit
- decides: story:host-stop-before-step
- decides: story:live-confined-child
- decides: story:child-read-only-roots-and-host-paths
revision: 2
---
# Wave 2026-10-22-w1 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words: "plan and run a loom wave that resolves
takt's two dependency blockers: ~/beyond10x/takt/.engineering/planning/dependency-blocker/loom-runtime-stops.md
(gap C: a breach choice per limit instead of one StopPolicy.on_breach; stops for active time and low
disk) and ~/beyond10x/takt/.engineering/planning/dependency-blocker/loom-child-confinement.md
(G1-G5). Completion order is settled as A, Loom declaration order (takt
approval-record/completion-order-first-declared.md); no Loom work for it. File the stories in loom's
own store, release as loom 0.33.0, and tell me the tag."

| unit | serves | surface | adversary |
|---|---|---|---|
| `story:breach-choice-per-limit` | `vision:governed-autonomy` | `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`, `crates/loom-commission/src/runtime.rs`, `crates/loom-commission-testkit/tests/stop_policy.rs`, `StopPolicy` literals | one pass over both Commission units |
| `story:host-stop-before-step` | `vision:governed-autonomy` | the same specification and runtime, `crates/loom-commission-testkit/tests/host_stop.rs`, `crates/loom-governor/src/file_suspension.rs`, `SuspensionReason` matches | one pass over both Commission units |
| `story:live-confined-child` | `vision:governed-autonomy` | `ess/intake/domains/confinement.yaml`, `generated/rust/intake/`, `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/tests/confined_child.rs`, `crates/loom-sdk` | one pass over both confinement units (security review) |
| `story:child-read-only-roots-and-host-paths` | `vision:governed-autonomy` | the same specification and confinement module | one pass over both confinement units (security review) |

- Decisions taken in planning:
  - D1: active time and low disk are one host port (`LoopContext::host_stop`), not Loom-measured
    limits. Commission cannot see a pause inside an effect's `invoke`, and its core reads no disk.
  - D2: a step past several limits is discarded when any of them is `DiscardStep`.
  - D3: G5 is closed as far as Substrate 0.7.18 goes: read-only mounts at host paths and
    `ChildCommand::sandbox_path`. A writable mount outside the workspace and a working directory
    other than `/workspace` are not in Substrate 0.7.18; the CHANGELOG and AGENTS.md say so.
- Overlap: the Commission units run one after the other, and so do the confinement units. The two
  chains share only `CHANGELOG.md` and `AGENTS.md`, merged one at a time.
- Breaks for a consumer: `StopPolicy` gains a field, `SuspensionReason` a variant,
  `TestExecution` a field. Each is a `**Breaking:**` line.
- Builds go through `b10x-build-slot`, each unit into its own tree's `target/`.
- Commits authorised: 1 opening store commit, the unit commits on each `impl/<story>` branch, 1
  merge per unit into `wave/2026-10-22-w1`, adversary-test commits, the release commit for 0.33.0,
  1 closing store commit, the fast-forward of `main` to the wave head through the bot, the
  annotated tag 0.33.0 and its GitHub Release.

## Units

| unit | tree id | branch |
|---|---|---|
| breach-choice-per-limit | loom-w20261022w1-u1 | impl/breach-choice-per-limit |
| host-stop-before-step | loom-w20261022w1-u2 | impl/host-stop-before-step |
| live-confined-child | loom-w20261022w1-u3 | impl/live-confined-child |
| child-read-only-roots-and-host-paths | loom-w20261022w1-u4 | impl/child-read-only-roots-and-host-paths |

Integration tree `loom-w20261022w1`, branch `wave/2026-10-22-w1`.
