---
format: aep.planning-md/3
id: story:ess-059-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.59.0
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
  path: ess/commission/ess-inputs.yaml
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/intake/ess-inputs.yaml
revision: 22
transitions:
- {from: "draft", to: "proposed", at: "2026-10-12T15:59:29Z", actor: "human:timo", revision: 20}
- {from: "proposed", to: "active", at: "2026-10-12T15:59:29Z", actor: "human:timo", revision: 21}
- {from: "active", to: "implemented", at: "2026-10-12T20:00:14Z", actor: "human:timo", revision: 22, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Loom builds, generates and gates on ESS 0.59.0, the newest release (published
2026-10-11T10:58:22Z, `gh release list -R beyond10x/ess`), under the standing rule that every
repository moves to the newest ESS when it ships. The three systems (`ess/`, `ess/commission/`,
`ess/intake/`) require `ess 0.59.0`; the ESS crates Loom depends on (`ess-conformance`,
`ess-primitives`) are pinned at tag `0.59.0`; CI installs `ess` 0.59.0 with the archive checksum
from the release's `SHA256SUMS`; every generated tree and generated page is regenerated with it.

## What 0.58.0 and 0.59.0 change for Loom

From the release notes of beyond10x/ess `0.58.0` and `0.59.0`:

- `ess/24` struct responses, `undeclared_fields: ignored`, `ess-conformance/48` and `/49`,
  `ess-scenario/5` (`caller:`): Loom declares none of them. Whether its suites change is
  answered by regeneration.
- 0.59.0 synthesizes the scenarios of a `when_subject:` command even when no view observes the
  field. Loom's suites gain scenarios only if one of its commands was refused that way; the
  measurement below found none refused.
- Neither release adds a predicate for membership in a `List` input (beyond10x/ess
  `story:guard-tests-list-membership` is still `draft`), so `not-in-frontier` stays `external:`
  and `story:revalidation-membership-conformance` stays draft.

Measured 2026-10-12 on a copy of `ess/` with `ess` 0.59.0 and `requires: ess 0.59.0`:
`ess specify validate --strict-requires` prints `valid` for all three systems (7, 2 and 8 files),
`ess specify compile` exits 0 for each, `ess verify conform synthesize` writes 61 scenarios for
`ess/` and 15 for `ess/commission/`, 0 refusals each, and `ess generate synthesize` over `ess/`
writes a tree equal to `generated/rust/loom/`. A membership predicate
(`action in input.frontier_actions`) is still refused with `ESS-SPEC-012`.

## ESS first

The first commit changes only `requires:` in `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`
and `ess/intake/ess-inputs.yaml`. On it, with `ess` 0.57.0 first on `PATH`, `task ess-gate` is red
(`--strict-requires` refuses `requires: ess 0.59.0`), and with 0.59.0 it is green; record the red
run. No domain declaration changes.

## Acceptance

- `ess --version` prints `ess 0.59.0` and the package gates of the touched crates pass on the
  integration branch, with every `requires:` line, both `tag = "0.59.0"` pairs
  (`crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml`),
  `ESS_VERSION` in `.github/workflows/check.yml` and the version text the gate tests match naming
  0.59.0, and none naming 0.57.0 as a pin
  (`git grep -n 'ess 0\.57\.0\|tag = "0\.57\.0"\|ESS_VERSION: "0\.57' -- ':!CHANGELOG.md' ':!.engineering'`
  prints nothing).
- `task drift`, `task commission:drift`, `task intake-drift`, `task docs-check` and
  `task commission:docs-drift` pass after regeneration with 0.59.0.
- CHANGELOG (Unreleased), README.md and AGENTS.md § ESS name 0.59.0.

## Scope

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml` (cited, line 2)
- `.github/workflows/check.yml` (cited, line 58)
- `crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml` (cited,
  lines 12-13), `Cargo.lock`
- `crates/loom-executor/tests/ess_gate.rs` (cited, lines 317, 396, 578),
  `crates/loom-executor/tests/adversary2_ess_gate.rs` (cited, lines 19, 183, 204, 243),
  `crates/loom-commission/tests/ess_gate.rs` (cited, lines 89, 364, 414-421, 662),
  `crates/loom-commission/tests/adversary2_gate.rs` (cited, lines 138, 144, 201, 207)
- comments naming 0.57.0: `crates/loom-executor/src/revalidation.rs:10`,
  `crates/loom-executor/tests/adversary_run_revalidation.rs:13`, `ess/domains/run.yaml:993`,
  `crates/loom-commission-docs/src/main.rs:142` (cited)
- `generated/rust/`, `website/docs/reference/` (by task only; inferred)
- `README.md:57`, `README.md:86`, `AGENTS.md:154`, `CHANGELOG.md` (cited)

## Shared surface

Lands first in its wave: `story:granted-gate-ends-run` is validated with 0.59.0. Both touch
`CHANGELOG.md` (Unreleased) only.

## Source

Release notes of beyond10x/ess `0.58.0` and `0.59.0`; `story:ess-057-upgrade` (the same procedure
for 0.57.0).
