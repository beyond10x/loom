---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-23-w3
kind: approval-record
status: draft
title: Wave 2026-10-23-w3 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:confinement-spec
- decides: story:session-lineage
- decides: story:loop-context-split
- decides: story:pinned-consumers-agency-shape
revision: 4
---
# Wave 2026-10-23-w3 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-23): "Wave 2026-10-23-w3
approved as A, under the grant in ~/beyond10x/CLAUDE.md: confinement-spec, session-lineage,
loop-context-split and pinned-consumers-agency-shape, then release 0.38.0. writable-host-root stays
out until Q2." It released builds later the same day: "open wave w3. Build one unit tree at a
time, not in parallel: ess and substrate are building too. Stop new builds if free space drops
under 25G, and discard each unit's cache after it merges."

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:confinement-spec` | `vision:governed-autonomy` | `ess/intake/`, `generated/rust/intake/`, `crates/loom-intake-slice/src/confinement*.rs`, `crates/loom-intake-slice/tests/confinement_spec.rs`, `crates/loom-sdk`, `sdk-stability.md` | adversary, security review |
| u2 `story:session-lineage` | `vision:governed-autonomy` | `ess/domains/session.yaml`, `generated/rust/loom/`, `crates/loom-session-executor/` | adversary |
| u3 `story:loop-context-split` | `vision:governed-autonomy` | `crates/loom-commission/src/runtime.rs` and every `LoopContext` implementor, `crates/loom-commission-testkit/tests/loop_context_split.rs`, `crates/loom-sdk`, `consumers/pinned/`, `sdk-stability.md` | adversary |
| u4 `story:pinned-consumers-agency-shape` | `vision:governed-autonomy` | `consumers/pinned/`, `crates/loom-xtask/tests/pinned_consumer.rs` | adversary |

- Decisions taken in planning:
  - D1: the units run one after another, each tree cut from the wave branch after the previous
    unit merged, and each unit's build cache is discarded at its merge: one build tree at a time.
    u1, u3 and u4 share `sdk-stability.md`, `host_ports.rs` or `consumers/pinned/`.
  - D2: `ConfinementSpec` is declared in `intake.confinement` (`ess/intake/`), beside the applied
    confinement it configures; `loom.agent` keeps `ConfinementMode` as Q6 A left it.
  - D3: `story:writable-host-root` stays out (Q2); `ConfinementSpec.writable_roots` is refused
    while non-empty, naming the field.
  - D4: if the blanket `impl LoopContext for T` makes an existing direct `impl LoopContext`
    overlap, each implementor that must change is a `**Breaking:**` bullet, and release 0.38.0 is
    a minor version move in any case.

- Decisions taken while the wave ran:
  - D5: the SDK's loop clock trait keeps the name the story uses, `Clock`, although the slice also
    has a `clock::Clock`; the SDK exports it as `loom_sdk::Clock`.
  - D6: the pinned agent consumer takes its clock from the host (`agency::run(spec, now)`, with
    `SystemNow` and `FixedNow`), after the adversary found the frozen 2026-01-01 clock read a
    passed deadline as ahead.
  - D7: main (PR 120, the `governed_loop` example, 45dcfe4c) merged into the wave before it
    closed; the `CHANGELOG.md` conflict kept both sides' bullets.
  - D8: D4's overlap did not occur (the adversary's consumer shapes compile), but release 0.38.0
    carries one `**Breaking:**` bullet: `continues`, added to the generated `loom.session` structs
    (`AgentSessionData`, `ProposeSessionChange`), breaks a consumer that builds them with a struct
    literal.

  - `b10x-chef-orchestrator` accepted D5 and D8 on 2026-10-23: "D5 and D8 are accepted; record
    both in your store."
