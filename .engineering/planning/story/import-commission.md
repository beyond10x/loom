---
format: aep.planning-md/3
id: story:import-commission
kind: story
status: active
title: Commission's crates, ESS and plan live in Loom with history; contracts refuse Canon
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: .github/workflows
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/commission
- confidence: inferred
  path: crates/commission-conformance
- confidence: inferred
  path: crates/commission-docs
- confidence: inferred
  path: crates/commission-testkit
- confidence: inferred
  path: crates/commission-xtask
- confidence: inferred
  path: ess
- confidence: inferred
  path: generated/rust/commission
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T22:39:58Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-04T22:39:58Z", actor: "human:timo", revision: 3}
---
## Outcome

Commission's crates, ESS sources and open plan live in Loom with their git history, and Loom builds
against them by path instead of by git dependency.

## Acceptance

- Commission's history is reachable from Loom `main` (`git log --follow` on a moved file shows
  commits from `beyond10x/commission`).
- `crates/` holds Commission's five crates; Loom's `Cargo.toml` names them by path and has no
  `git = "https://github.com/beyond10x/commission"` dependency.
- Commission's `ess/` domains are part of Loom's ESS input; `ess specify validate` and Loom's
  drift check pass.
- A test refuses `b10x-canon` in the contracts crate's dependency tree (Commission's
  `skeleton.rs:440-471` check, carried over).
- Commission's 5 draft stories and its open epics exist in Loom's store, each citing its old id.
- `task check` exits 0.

## Scope (inferred)

loom: `Cargo.toml`, `Cargo.lock`, `crates/commission*`, `ess/`, `.engineering/`, `Taskfile.yml`.
