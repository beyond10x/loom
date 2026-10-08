---
format: aep.planning-md/3
id: story:ess-056-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.56.0
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
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
  path: ess/commission/ess-inputs.yaml
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/commission/domain-graph.json
revision: 27
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T02:05:32Z", actor: "human:timo", revision: 18}
- {from: "proposed", to: "active", at: "2026-10-08T02:05:32Z", actor: "human:timo", revision: 19}
- {from: "active", to: "implemented", at: "2026-10-08T02:50:10Z", actor: "human:timo", revision: 27, decided_on: {"recorded":{"test_result":2,"verification":1}}}
---
## Outcome

Loom builds, generates and gates on ESS 0.56.0, the newest release (published
2026-10-07T23:16:28Z, `gh release list -R beyond10x/ess`), under the standing rule that every
repository moves to the newest ESS when it ships. The three systems (`ess/`, `ess/commission/`,
`ess/intake/`) require `ess 0.56.0`; the ESS crates Loom depends on (`ess-conformance`,
`ess-primitives`) are pinned at tag `0.56.0`; CI installs `ess` 0.56.0 with the archive checksum
from the release's `SHA256SUMS`; every generated tree and generated page is regenerated with it.

## What 0.56.0 changes for Loom

From the release notes of beyond10x/ess `0.56.0`:

- Validation refuses an accepting `when:` branch or an `external:` branch declared before a
  held-state branch of the same command where one request can satisfy both guards
  (`ESS-COMMAND-004`). Measured 2026-10-08 with `ess` 0.56.0, without `--strict-requires`:
  `ess specify validate` prints `valid` for `ess` (2 files), `ess/commission` (2 files) and
  `ess/intake` (7 files); no Loom command is refused.
- Generated Rust type libraries (`ess generate types`) gain a default-on `exact-numbers` feature.
  Loom generates through `ess generate synthesize`; whether any generated manifest changes is
  answered by the regeneration, not assumed.
- `ess generate` writes `ess-output-state/3`; a settled record written by an earlier release is
  rewritten once on the next write-mode generation, and `--check` reports no drift for it.
  Committed output records that change are part of this story's regeneration commit.

## ESS first

The first commit changes only `requires:` in `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`
and `ess/intake/ess-inputs.yaml`. On it, with `ess` 0.55.0 first on `PATH`, `task ess-gate` is red
(`--strict-requires` refuses `requires: ess 0.56.0`), and with 0.56.0 it is green; record the red
run. No domain declaration changes.

## Acceptance

- `ess --version` prints `ess 0.56.0` and `task check` passes on the integration branch, with every
  `requires:` line, both `tag = "0.56.0"` pairs (`crates/loom-conformance/Cargo.toml`,
  `crates/loom-commission-conformance/Cargo.toml`), `ESS_VERSION` in `.github/workflows/check.yml`
  and the version text the gate tests match (`crates/loom-commission/tests/ess_gate.rs`,
  `crates/loom-commission/tests/adversary2_gate.rs`) naming 0.56.0, and none naming 0.55.0
  (`git grep -n '0\.55\.0' -- ':!CHANGELOG.md' ':!.engineering'` prints nothing).
- `task drift`, `task commission:drift`, `task intake-drift`, `task docs-check` and
  `task commission:docs-drift` pass after regeneration with 0.56.0.
- CHANGELOG (Unreleased) and AGENTS.md § ESS name 0.56.0.

## Scope

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml` (cited)
- `.github/workflows/check.yml` (cited, line 58)
- `crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml`,
  `Cargo.lock` (cited)
- `crates/loom-commission/tests/ess_gate.rs`, `crates/loom-commission/tests/adversary2_gate.rs`,
  `crates/loom-commission-docs/src/main.rs`, `crates/loom-executor/src/revalidation.rs`,
  `crates/loom-executor/tests/adversary2_ess_gate.rs` (cited: they name 0.55.0)
- `generated/rust/`, `website/docs/reference/` (by task only; inferred: which files change)
- `AGENTS.md`, `CHANGELOG.md` (cited)

## Shared surface

Edits `generated/rust/commission/` and the Commission gate tests, like
`story:moved-run-named-outcome`, which depends on this story so its specification change is
validated with 0.56.0.

## Source

Release notes of beyond10x/ess `0.56.0`; `story:ess-055-upgrade` (the same procedure for 0.55.0).

## Scope confirmed

Read from `git diff --stat 5b57e75 e28dc70` (the unit's three commits, 18 files) at the close of wave
2026-10-08-w1. Corrections to the `## Scope` section above:

| Scope line | Confidence then | What the unit changed |
|---|---|---|
| `generated/rust/` | inferred | nothing: regenerating with 0.56.0 changed no generated Rust |
| `website/docs/reference/` | inferred | `website/docs/reference/commission/domain-graph.json` (names the 0.56.0 compiler) |
| every cited line | cited | as cited |
| not listed | — | `crates/loom-executor/tests/ess_gate.rs` (its version-check text named 0.55.0), comments in `crates/loom-executor/tests/adversary_run_revalidation.rs` and `ess/domains/run.yaml` (no declaration) |

The acceptance `git grep -n '0\.55\.0' -- ':!CHANGELOG.md' ':!.engineering'` prints three lines that
are not ESS pins: `Cargo.lock:1423` and `:1548` (`gix-discover` and `gix-index` at version 0.55.0) and
`docs/handoff/2026-10-06-loom.md:49`, a dated record; both left as they are.
