---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-23-w1
kind: approval-record
status: draft
title: Wave 2026-10-23-w1 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:runtime-wrapper
- decides: story:session-executor-from-spec
- decides: story:substrate-0-8-pin
- decides: story:stability-version-rule
revision: 1
---
# Wave 2026-10-23-w1 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-23): "Go: wave 2026-10-23-w1
approved (class C, ADR 0099 epic loom-sdk-for-products): accept and implement story:runtime-wrapper,
story:session-executor-from-spec, story:substrate-0-8-pin retargeted to Substrate 0.10.0,
story:stability-version-rule; merge one at a time as you planned; release 0.36.0 per loom
AGENTS.md. 1 build slot, test binaries under systemd-run --user --scope -p MemoryMax=8G, pause
builds under 25G. Note: Substrate 0.10.0 (verified, tag 105cf718) carries substrate's writable-roots
wave; check it against several-egress-apertures/child-workdir when you plan the next wave.
writable-host-root stays on Timo's Q2. Approved under the grant in ~/beyond10x/CLAUDE.md."

| unit | serves | surface | review |
|---|---|---|---|
| `story:runtime-wrapper` | `vision:governed-autonomy` | `crates/loom-sdk/src/runtime.rs`, `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/tests/`, `website/docs/reference/sdk-stability.md` | adversary |
| `story:session-executor-from-spec` | `vision:governed-autonomy` | `ess/domains/session.yaml`, `generated/rust/loom/`, `crates/loom-session-executor` | adversary |
| `story:substrate-0-8-pin` | `vision:governed-autonomy` | `crates/loom-intake-slice/Cargo.toml`, `Cargo.lock`, `crates/loom-intake-slice/src/confinement*` | security review |
| `story:stability-version-rule` | `vision:governed-autonomy` | `crates/loom-xtask`, `website/docs/reference/sdk-stability.md`, AGENTS.md § Releases | adversary |

- Decisions taken in planning:
  - D1: `story:substrate-0-8-pin` targets Substrate 0.10.0 (commit `105cf718`), as the
    orchestrator retargeted it; its body says so.
  - D2: the four units are implemented in their own trees; builds share one build slot, test
    binaries run under `systemd-run --user --scope -p MemoryMax=8G`, and builds pause under 25G
    free. All four touch `CHANGELOG.md`; `runtime-wrapper` and `stability-version-rule` both touch
    `sdk-stability.md`; they merge one at a time and a later unit rebases on the wave head.
  - D3: each unit's build cache is discarded when it merges; one full gate runs on the wave head.
  - D4: release 0.36.0 moves the `ess` pin in `.github/workflows/check.yml` to the newest ESS
    release, as AGENTS.md § Releases requires.
