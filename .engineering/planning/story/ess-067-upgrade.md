---
format: aep.planning-md/3
id: story:ess-067-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.67.0
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:metaharness-session-executor
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T11:12:15Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T11:12:15Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T12:21:03Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Loom builds, generates and gates on ESS 0.67.0, the newest release (published 2026-10-21T11:00:49Z),
moved in the 0.31.0 release because every Loom release moves its ess pin to the newest ESS release
(AGENTS.md § Releases). Today every pin names 0.66.0. The private release binary is
`~/.cache/loom-ess-0.67.0/ess-0.67.0-x86_64-unknown-linux-gnu/ess` (sha256
`60ffd3d1120be4cc5e85d4c6ab37d3cbd0bdaf397fde4f243bd1fffe71f13e5c`, checked against the release's
SHA256SUMS); the shared `ess` on the machine is not touched.

## Acceptance

- Every `requires:` line, the three `tag = "0.66.0"` pairs (`crates/loom-conformance`,
  `crates/loom-commission-conformance`, `crates/loom-intake-conformance`), `ESS_VERSION` and
  `ESS_SHA256` in `.github/workflows/check.yml` and `.github/workflows/pages.yml`, and the version text
  the gate tests match name 0.67.0; `git grep -n 'ess 0\.66\.0\|tag = "0\.66\.0"\|ESS_VERSION: "0\.66'
  -- ':!CHANGELOG.md' ':!.engineering'` prints nothing.
- `task drift`, `task commission:drift`, `task intake-drift`, `task docs-check`,
  `task commission:docs-drift`, `task conform`, `task commission:conform`, `task intake-conform` and
  `task ess-gate` pass after regeneration with 0.67.0.
- CHANGELOG (Unreleased) has one line for the move; README.md and AGENTS.md § ESS name 0.67.0.
- The specifications stay at their formats (`ess/24`, `ess/20`, `ess/26`); `ess/29` is not adopted.
- If the move needs more than the pins, the regeneration and small fixes (a format or semantics
  change, or conformance that fails), it stops and reports the failing command and its output.

## ESS first

No noun changes; the specifications stay at their formats.

## Scope

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml`
- `.github/workflows/check.yml`, `.github/workflows/pages.yml`
- `crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml`,
  `crates/loom-intake-conformance/Cargo.toml`, `Cargo.lock`
- the gate tests and comments naming 0.66.0 (`git grep` above)
- `generated/rust/`, `website/docs/reference/`, `website/data/ess/` (by task only)
- `README.md`, `AGENTS.md`, `CHANGELOG.md`

## Source

Release notes of beyond10x/ess 0.67.0 (format `ess/29`, `ess-conformance/56`/`57` for new suites;
"older suites keep their bytes"); `story:ess-066-upgrade` (the same procedure).

## Shipped

Merged into wave 2026-10-21-w5 as 20299cc2 (62e36eba, 64b00a61). No behaviour change, so no red step; a planted one-byte change under generated/rust/loom made task drift exit 201.
