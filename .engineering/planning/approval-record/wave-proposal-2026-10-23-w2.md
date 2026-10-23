---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-23-w2
kind: approval-record
status: draft
title: Wave 2026-10-23-w2 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:several-egress-apertures
- decides: story:child-workdir
revision: 2
---
# Wave 2026-10-23-w2 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-23): "A, approved under the
grant in ~/beyond10x/CLAUDE.md: wave 2026-10-23-w2, u1 several-egress-apertures then u2
child-workdir in sequence, adversary on both, security review on u1, release 0.37.0. Reason: takt
needs both (your proposal line 47, design § 5 gaps 2 and 3); same files, so sequential. Acceptance
edits at accept as you wrote them: drop workdir's \"writable root\" until Timo's Q2; apertures 2-4
require host names. Name the Breaking line in the CHANGELOG. Disk: one build at a time, pause
under 25G, discard at merge."

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:several-egress-apertures` | `vision:governed-autonomy` | `ess/intake/`, `generated/rust/intake/`, `crates/loom-intake-slice/src/confinement.rs`, `confinement/child.rs`, `crates/loom-intake-slice/tests/confined_child.rs` | adversary, security review |
| u2 `story:child-workdir` | `vision:governed-autonomy` | the same files | adversary |

- Decisions taken in planning:
  - D1: Substrate 0.10.0 (`105cf718`), linked by Loom 0.36.0, serves both stories (`apertures`,
    2 to `MAX_EGRESS_APERTURES` = 4; `workdir` under `/workspace` or a read-only root), so
    Substrate does not move.
  - D2: u2 starts from the wave branch after u1 merged, since both change the same files; the
    Scope line of `story:child-workdir` now says "not in parallel with it".
  - D3: acceptance edits at accept: `story:child-workdir` drops a workdir under a writable root
    until `story:writable-host-root` (Timo's Q2); `story:several-egress-apertures` requires host
    names for apertures 2 to 4 and refuses more than 4.
  - D4: one build at a time, test binaries under `systemd-run --user --scope -p MemoryMax=8G`,
    builds pause under 25G free, each unit's build cache discarded at merge, one full gate on the
    wave head; release 0.37.0 names its `**Breaking:**` line in `CHANGELOG.md`.

- Decisions taken while the wave ran:
  - D5: every numeric aperture host that `inet_aton` reads as an address (decimal, hex or octal
    labels) is refused on apertures 2 to 4, and on the first only `127.0.0.1` is accepted; the
    adversary reached aperture 1's destination through a hex host on a later aperture.
  - D6: `AppliedConfinement.network` is `Option<Vec<AppliedAperture>>`, absent for an unconfined
    run, so an unconfined run never reads as no network; a third `**Breaking:**` bullet.
  - D7: a workdir the process cannot search is refused before launch, and a run whose output
    carries bubblewrap's chdir diagnostic for its workdir is refused after it; the second check is
    held by a unit test only.
  - D8: the implementor's `### Security` bullet for the numeric-host refusal stays in
    `CHANGELOG.md`.
