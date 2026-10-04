---
format: aep.planning-md/3
id: story:import-commission
kind: story
status: draft
title: Commission's crates, ESS and plan live in Loom with history; contracts refuse Canon
relations:
- decomposes: epic:runtime-consolidation
revision: 1
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
