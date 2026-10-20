---
format: aep.planning-md/3
id: story:ess-066-upgrade
kind: story
status: active
title: Loom builds, generates and gates on ESS 0.66.0
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: .github/workflows/pages.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-commission-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-commission-docs/src/main.rs
- confidence: cited
  path: crates/loom-commission/tests/adversary2_gate.rs
- confidence: cited
  path: crates/loom-commission/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-executor/src/revalidation.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary2_ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_run_revalidation.rs
- confidence: cited
  path: crates/loom-executor/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-intake-conformance/Cargo.toml
- confidence: cited
  path: ess/commission/ess-inputs.yaml
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
- confidence: inferred
  path: generated/rust
- confidence: inferred
  path: website/docs/reference
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T20:09:53Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-20T20:09:53Z", actor: "human:timo", revision: 5}
---
## Outcome

Loom builds, generates and gates on ESS 0.66.0, the newest release, moved in the 0.30.0 release as
every Loom release now moves its ess pin to the newest ESS release (Timo, 2026-10-20: "we want all
to upgrade to most recent ess ... whenever one of the things you control is being touched and made
ready for a release"). Today every pin names 0.60.0. With the 0.66.0 release binary (sha256
`aa6b209efb5f9545c85fc31d3b66363d10be208c5bcdeaba61be0d54c50e17f6`), `ess/`, `ess/commission` and
`ess/intake` validate and synthesize 61, 39 and 11 scenarios with 0 refusals (2026-10-20, before the
move). The shared `ess` on the machine stays at 0.60.0: every ess task of this story runs a private
copy of the 0.66.0 release binary first on its own `PATH`.

## Acceptance

- Every `requires:` line, the three `tag = "0.60.0"` pairs (`crates/loom-conformance`,
  `crates/loom-commission-conformance`, `crates/loom-intake-conformance`), `ESS_VERSION` and
  `ESS_SHA256` in `.github/workflows/check.yml` and `.github/workflows/pages.yml`, and the version text
  the gate tests match name 0.66.0; `git grep -n 'ess 0\.60\.0\|tag = "0\.60\.0"\|ESS_VERSION: "0\.60'
  -- ':!CHANGELOG.md' ':!.engineering'` prints nothing.
- `task drift`, `task commission:drift`, `task intake-drift`, `task docs-check`,
  `task commission:docs-drift`, `task conform`, `task commission:conform`, `task intake-conform` and
  `task ess-gate` pass after regeneration with 0.66.0.
- CHANGELOG (Unreleased) has one line for the move; README.md and AGENTS.md § ESS name 0.66.0, and
  AGENTS.md § Releases says a release moves the ess pin to the newest ESS release.
- If the move needs more than the pins, the regeneration and small fixes (a format or semantics
  change, or conformance that fails), it stops and reports the failing command and its output.

## ESS first

No noun changes; the specifications stay at their formats.

## Scope

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml`
- `.github/workflows/check.yml`, `.github/workflows/pages.yml`
- `crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml`,
  `crates/loom-intake-conformance/Cargo.toml`, `Cargo.lock`
- the gate tests and comments naming 0.60.0 (`git grep` above): `crates/loom-executor/tests/ess_gate.rs`,
  `crates/loom-executor/tests/adversary2_ess_gate.rs`, `crates/loom-commission/tests/ess_gate.rs`,
  `crates/loom-commission/tests/adversary2_gate.rs`, `crates/loom-executor/src/revalidation.rs`,
  `crates/loom-executor/tests/adversary_run_revalidation.rs`, `ess/domains/run.yaml`,
  `crates/loom-commission-docs/src/main.rs`
- `generated/rust/`, `website/docs/reference/` (by task only)
- `README.md`, `AGENTS.md`, `CHANGELOG.md`

## Source

Release notes of beyond10x/ess 0.61.0-0.66.0; `story:ess-059-upgrade` (the same procedure).
