---
format: aep.planning-md/3
id: story:import-intake
kind: story
status: active
title: Intake's router, references, slice and CLI live in Loom; its model helper in llm
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:import-commission
- depends_on: story:import-governor
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/intake-cli
- confidence: inferred
  path: crates/intake-model
- confidence: inferred
  path: crates/intake-references
- confidence: inferred
  path: crates/intake-router
- confidence: inferred
  path: crates/intake-slice
- confidence: inferred
  path: docs/intake
- confidence: inferred
  path: ess/intake
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T00:09:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T00:09:55Z", actor: "human:timo", revision: 3}
---
## Outcome

Intake's router, references, model helper, slice and CLI live in Loom with their history, built
against Loom's own executor, Commission and governor crates by path.

## Acceptance

- `crates/intake-router`, `crates/intake-references`, `crates/intake-model`, `crates/intake-slice`,
  `crates/intake-cli` in Loom, with history from `beyond10x/intake` (`git log --follow` reaches
  intake's commits for a moved `ess/` file and a crate file).
- No `Cargo.toml` in Loom names `github.com/beyond10x/commission`, `/governor`, `/loom` or
  `/intake`; `cargo tree -i` shows one copy each of `b10x-commission`, `b10x-governor`,
  `b10x-loom` and `b10x-canon`.
- Intake's offline slice test passes in Loom and still ends `ApprovalRequired (repository.merge)`.
- Intake's `ess/` domain is part of Loom's ESS input at `ess/intake/`; `ess specify validate`
  passes.
- `docs/qualification/2026-10-04-live-run.md` is in Loom at `docs/intake/qualification/`.
- `task check` passes (every step, run one at a time).

## Changed at wave open (2026-10-05, wave w25)

Moving `intake-model` (`call_tool`, Codex preset) into llm is split out to llm
`story:call-tool-helper`: it needs an llm release before Loom can pin it. Until then
`intake-model` is a Loom crate.

## Depends on

`story:import-commission`, `story:import-governor` (both implemented).
