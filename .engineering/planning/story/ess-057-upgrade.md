---
format: aep.planning-md/3
id: story:ess-057-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.57.0
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
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-10T04:21:19Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Loom builds, generates and gates on ESS 0.57.0, the newest release (published
2026-10-09T22:33:42Z, `gh release list -R beyond10x/ess`), under the standing rule that every
repository moves to the newest ESS when it ships. The three systems (`ess/`, `ess/commission/`,
`ess/intake/`) require `ess 0.57.0`; the ESS crates Loom depends on (`ess-conformance`,
`ess-primitives`) are pinned at tag `0.57.0`; CI installs `ess` 0.57.0 with the archive checksum
from the release's `SHA256SUMS`; every generated tree and generated page is regenerated with it.

## What 0.57.0 changes for Loom

From the release notes of beyond10x/ess `0.57.0`:

- `ess-conformance/46`: a command response reaching a constrained `String` newtype keeps its
  success scenarios. Whether Loom's suites change is answered by regeneration.
- Validation refuses a wire name containing `/` (`ESS-DOMAIN-012`, `ESS-COMMAND-012`,
  `ESS-VIEW-012`) and a refusal whose `when:` always holds (`ESS-COMMAND-004`).
- `ess-diff/15`–`/17` and `ess-cli/2`: Loom uses neither.

Measured 2026-10-10 on a copy of `ess/` with `ess` 0.57.0 and `requires: ess 0.57.0`:
`ess specify validate --strict-requires` prints `valid` (7 files) and
`ess verify conform synthesize` writes 46 scenarios, 0 refusals.

## ESS first

The first commit changes only `requires:` in `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`
and `ess/intake/ess-inputs.yaml`. On it, with `ess` 0.56.0 first on `PATH`, `task ess-gate` is red
(`--strict-requires` refuses `requires: ess 0.57.0`), and with 0.57.0 it is green; record the red
run. No domain declaration changes.

## Acceptance

- `ess --version` prints `ess 0.57.0` and the package gates of the touched crates pass on the
  integration branch, with every `requires:` line, both `tag = "0.57.0"` pairs
  (`crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml`),
  `ESS_VERSION` in `.github/workflows/check.yml` and the version text the gate tests match naming
  0.57.0, and none naming 0.56.0 as a pin
  (`git grep -n 'ess 0\.56\.0\|tag = "0\.56\.0"\|ESS_VERSION: "0\.56' -- ':!CHANGELOG.md' ':!.engineering'`
  prints nothing).
- `task drift`, `task commission:drift`, `task intake-drift`, `task docs-check` and
  `task commission:docs-drift` pass after regeneration with 0.57.0.
- CHANGELOG (Unreleased), README.md and AGENTS.md § ESS name 0.57.0.

## Scope

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml` (cited, line 2)
- `.github/workflows/check.yml` (cited, line 58)
- `crates/loom-conformance/Cargo.toml`, `crates/loom-commission-conformance/Cargo.toml` (cited,
  lines 12-13), `Cargo.lock`
- `crates/loom-executor/tests/ess_gate.rs` (cited, line 396), `crates/loom-executor/tests/adversary2_ess_gate.rs`
  (cited, line 243), `crates/loom-commission/tests/ess_gate.rs`,
  `crates/loom-commission/tests/adversary2_gate.rs` (inferred: matched 0.56.0 before)
- comments naming 0.56.0: `crates/loom-executor/src/revalidation.rs:10`,
  `crates/loom-executor/tests/adversary_run_revalidation.rs:13`, `ess/domains/run.yaml:818` (cited)
- `generated/rust/`, `website/docs/reference/` (by task only; inferred)
- `README.md:57`, `AGENTS.md:152`, `CHANGELOG.md` (cited)

## Shared surface

Lands first in its wave: `story:fallback-selection-recording` and `story:selection-telemetry`
change `ess/domains/run.yaml` and are validated with 0.57.0.

## Source

Release notes of beyond10x/ess `0.57.0`; `story:ess-056-upgrade` (the same procedure for 0.56.0).
