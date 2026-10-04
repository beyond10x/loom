---
format: aep.planning-md/3
id: story:loom-ess-conformance
kind: story
status: draft
title: Carry the Loom ESS specification to a synthesized conformance suite in task check
refs:
- provider: taskboard
  reference: I-006
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:frontier-projection
- depends_on: story:action-selector
- depends_on: story:argument-generator
- depends_on: story:selection-revalidation
- depends_on: story:session-transcript-streaming
- depends_on: story:compaction-contract
- depends_on: story:interruption-recovery
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:harness-loop-port
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-conformance/
- confidence: cited
  path: ess/SKIPPED.md
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 4
---
## Outcome

`task check` holds the Loom ESS specification (`ess/`), complete for this epic, to its synthesized
conformance suite, run against `b10x-loom`:

- **Where the suite comes from.** `ess verify conform synthesize --path ess` writes it, into the test
  crate's own build directory on every run, so it tracks `ess/` without a committed copy to drift.
- **What it runs against.** `b10x-loom`, through a Rust target in a new crate,
  `crates/loom-conformance/` (package `b10x-loom-conformance`), built on the `ess-conformance` crate
  as a git dependency at ESS tag `0.52.0` (the `requires: ess 0.52.0` of `ess/ess-inputs.yaml`),
  pinned by `Cargo.lock`. This is the route Mandate takes in `crates/mandate-conformance/`
  (mandate `95e0a4b`) and commission `story:commission-ess-conformance` plans. ESS's Go and
  TypeScript packages are not used: anything committed here that runs is Rust (`AGENTS.md` § Rules).
  `decision-blocker:rust-conformance-target` is cleared with this answer.
- **How it is started.** A new task `conform` runs `cargo test -p b10x-loom-conformance --test
  conform`, and `check` lists it as its own step.
- **How the verdict is read.** From the report document, not the runner's exit code.
- **Skips.** This story creates `ess/SKIPPED.md`: a header saying each line names one skipped
  scenario and why, then the list. A line is added only for a scenario Loom cannot answer.
- **Markers.** None: `ess/` carries no `UNMAPPED:` marker. `story:ess-hard-gate` closed the last
  ones (the catalogue marker by the operator decision of 2026-10-04 on
  `decision-blocker:catalogue-ownership`: one catalogue per turn), and its `task ess-gate` refuses
  any new one, so this story checks none.

The model drift gate is `story:agent-executor`'s (`task drift`), and the validate / compile /
synthesize / no-marker gate is `story:ess-hard-gate`'s (`task ess-gate`), not this story's.

## Shared surface

Link 10, the last, of the `epic:loom-native-harness` chain over `ess/domains/run.yaml` and
`generated/rust/loom/`. It depends on `story:interruption-recovery`; the whole order is in
`story:agent-executor` § Shared surface. `Taskfile.yml` and `Cargo.lock` are edited along the same
chain.

## ESS

The suite covers the whole of `ess/`: `ess-inputs.yaml`, `system.yaml` and `domains/run.yaml`. This
story changes the specification only when a scenario shows it is wrong; then `task ess-gate` passes
and `task generate` regenerates. Every synthesize `note:` is relayed in the closing report.

## Scope

- `crates/loom-conformance/` (new): `Cargo.toml`, `src/lib.rs`, `tests/conform.rs`
- `Cargo.lock`; `Taskfile.yml` (task `conform`, one line in `check`)
- `ess/SKIPPED.md` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface)

## Acceptance

`task check` runs the test `ess_conformance_report` in `crates/loom-conformance/tests/conform.rs`,
and it passes. It synthesizes the suite from `ess/`, runs it against `b10x-loom` through the Rust
target, and checks:

1. The report records at least one executed scenario.
2. Every command in `ess specify compile --path ess --format json` has at least one scenario in the
   suite.
3. The report records 0 failed scenarios.
4. Every skipped scenario in the report is named in `ess/SKIPPED.md`; applied to a copy of the
   report that adds one skipped scenario `S` the file does not name, the check fails and names `S`.

## Source

TASKBOARD I-006; Atlas ADR 0071; workspace AGENTS.md § ESS drives every product repository;
`ess:specifying` § Conformance is a record, not a claim; the answer recorded on
`decision-blocker:rust-conformance-target` (2026-10-04).
