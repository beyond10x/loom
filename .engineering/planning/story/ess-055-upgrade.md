---
format: aep.planning-md/3
id: story:ess-055-upgrade
kind: story
status: implemented
title: Loom builds, generates and gates on ESS 0.55.0
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T02:13:06Z", actor: "human:timo", revision: 5, executor: "agent:loom", correlation: "wave/2026-10-07-w1"}
- {from: "proposed", to: "active", at: "2026-10-07T02:13:06Z", actor: "human:timo", revision: 6, executor: "agent:loom", correlation: "wave/2026-10-07-w1"}
- {from: "active", to: "implemented", at: "2026-10-07T02:40:59Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"verification":1}}, executor: "agent:loom", correlation: "wave/2026-10-07-w1"}
---
## Outcome

Loom builds, generates and gates on ESS 0.55.0, the newest release (published
2026-10-06T22:14:48Z), under the standing rule that every repository moves to the newest ESS when it
ships. The three systems (`ess/`, `ess/commission/`, `ess/intake/`) require `ess 0.55.0`; the ESS
crates Loom depends on (`ess-conformance`, `ess-primitives`) are pinned at tag `0.55.0`; CI installs
`ess` 0.55.0 with the archive checksum from the release's `SHA256SUMS`; every generated tree and
generated page is regenerated with it.

## What 0.55.0 changes for Loom

From the release notes of beyond10x/ess `0.55.0`:

- A `{generated: true}` payload value of type `Optional<T>` is now supplied by the implementation
  through a new context port method, `generate_optional_<t>` (`try_generate_optional_<t>` on
  `TryContext`), instead of always being filled absent (beyond10x/ess#467). A context that
  implements the port must add the method. Whether any Loom, Commission or intake behaviour has such
  a payload is not established; the regenerated trees and `cargo check` answer it, and every context
  implementation under `crates/` that fails to compile gains the method, answering absent where the
  previous behaviour was absent.
- `ess generate cli` admits `Json` result fields (beyond10x/ess#468). Loom generates no CLI package
  with `ess generate cli`; no change expected.

## ESS first

The first commit changes only `requires:` in `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`
and `ess/intake/ess-inputs.yaml`; on it `task drift` (or a generation check) is red against the
committed generated trees, and the run is recorded. No domain declaration changes. Source formats
stay as they are unless 0.55.0 refuses one.

The machine's shared `ess` (`~/.cargo/bin/ess`) has been 0.55.0 since 2026-10-06T23:31Z, so the
unit uses it; until this story lands, a loom gate run against it fails the `--strict-requires`
check (`requires: ess 0.54.0`), which is the red state this story ends.

## Acceptance

- `ess specify validate --path <system> --strict-requires` exits 0 for all three systems with
  `ess 0.55.0`.
- `task check` exits 0 with `ess 0.55.0` on `PATH`, including both ESS gates, the three drift
  checks, both no-hand-model checks, Commission's conformance suite and the docs checks.
- `.github/workflows/check.yml` installs `ESS_VERSION: "0.55.0"` with the release checksum for
  `ess-0.55.0-x86_64-unknown-linux-gnu.tar.gz`; `AGENTS.md` names 0.55.0 where it names the CI pin.
- `Cargo.lock` holds no `ess-*` package at `0.54.0`.

## Scope

Confirmed by the implementation (wave 2026-10-07-w1, commits 32ecaf2 and 2235aa8):

- `ess/ess-inputs.yaml`, `ess/commission/ess-inputs.yaml`, `ess/intake/ess-inputs.yaml`: the
  `requires:` line — cited, changed.
- `ess/domains/run.yaml`: one comment naming the ESS version (line 676) — not in the first scope,
  changed.
- `.github/workflows/check.yml`, `AGENTS.md`, `Cargo.lock`,
  `crates/loom-commission-conformance/Cargo.toml` — cited, changed.
- The four `ess_gate` test files — cited, changed.
- `crates/loom-commission-docs/src/main.rs`, `crates/loom-executor/src/revalidation.rs`,
  `crates/loom-executor/tests/adversary_run_revalidation.rs`: comments naming the ESS version — not
  in the first scope, changed.
- `website/docs/reference/commission/domain-graph.json`, regenerated by `task commission:docs` — the
  inferred `website/docs/reference/` line, narrowed.
- `CHANGELOG.md`: one Unreleased entry — not in the first scope, changed.
- `generated/rust/{loom,commission,intake}/` — cited, **not changed**: regeneration under 0.55.0 is
  byte-identical, since no specification has a `{generated: true}` value of an `Optional` type.

## Shared surface

Touches every generated tree and all three `ess-inputs.yaml`, so no other unit that edits `ess/` or
`generated/rust/` shares its wave. Draft PR 20 (`feat/bounded-context`) edits `ess/intake/` and
`generated/rust/intake/`; it will need to regenerate after this lands.
