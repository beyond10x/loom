---
format: aep.planning-md/3
id: story:ess-054-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.54.0
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-commission-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-commission/tests/adversary2_gate.rs
- confidence: cited
  path: crates/loom-commission/tests/ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary2_ess_gate.rs
- confidence: cited
  path: crates/loom-executor/tests/ess_gate.rs
- confidence: cited
  path: ess/commission/ess-inputs.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
- confidence: cited
  path: generated/rust/commission/
- confidence: cited
  path: generated/rust/intake/
- confidence: cited
  path: generated/rust/loom/
- confidence: inferred
  path: website/docs/reference/
revision: 19
transitions:
- {from: "draft", to: "proposed", at: "2026-10-06T18:08:48Z", actor: "human:timo", revision: 17}
- {from: "proposed", to: "active", at: "2026-10-06T18:08:48Z", actor: "human:timo", revision: 18}
- {from: "active", to: "implemented", at: "2026-10-06T18:28:59Z", actor: "human:timo", revision: 19, decided_on: {"recorded":{"test_result":2}}}
---
## Outcome

Loom builds, generates and gates on ESS 0.54.0, the newest release (published 2026-10-06T17:40Z),
under the standing rule that every repository moves to the newest ESS when it ships. The three
systems require `ess 0.54.0`; the ESS crates Loom depends on are pinned at tag `0.54.0`; CI installs
`ess` 0.54.0 with the archive checksum from the release's `SHA256SUMS`
(`3f0a953323b1621b69a2a9bd5ccfd9606e68d4254aac6ef13cb43d804c66f7f9` for
`ess-0.54.0-x86_64-unknown-linux-gnu.tar.gz`); every generated tree and generated page is
regenerated with it.

## Why now

With `ess` 0.54.0 on PATH, `--strict-requires` refuses `requires: ess 0.53.0`, so `ess_gate`,
`adversary_ess_gate` and `adversary2_ess_gate` fail locally (adversary pass 1 of wave
2026-10-06-w3, `review-result:adversary-w3-loom-compaction-contract-pass-1`).

## ESS first

The first commit changes only `requires:` in `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`
and `ess/intake/ess-inputs.yaml`; on it `task drift` (or a generation check) is red against the
committed generated trees, and the run is recorded. No domain declaration changes. Source formats
stay as they are unless 0.54.0 refuses one.

## Acceptance

- `ess specify validate --path <system> --strict-requires` exits 0 for all three systems with
  `ess 0.54.0`.
- `task check` exits 0 locally with `ess 0.54.0` on PATH, including both ESS gates, the three drift
  checks, both no-hand-model checks, Commission's conformance suite and the docs checks.
- `.github/workflows/check.yml` installs `ESS_VERSION: "0.54.0"` with the checksum above;
  `AGENTS.md` names 0.54.0 where it names the CI pin.
- `Cargo.lock` holds no `ess-*` package at `0.53.0`.
