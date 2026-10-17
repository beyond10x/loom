---
format: aep.planning-md/3
id: story:llm-070-pin
kind: story
status: implemented
title: Loom links llm 0.7.0
relations:
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-17T17:00:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-17T17:00:55Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-17T18:17:26Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Every Loom crate links llm 0.7.0. llm 0.7.0 adds `TurnRequest.controls` (thinking, parallel tool
calls, advisory hints); a turn without controls stays the `llm.turn/3` envelope byte for byte, so
Loom builds every request without controls and its recorded requests do not change.

## Acceptance

- Every `b10x-llm-*` dependency in every `Cargo.toml` names tag `0.7.0`, and `Cargo.lock` holds one
  version of each `b10x-llm-*` crate (`grep` over the manifests, `cargo tree -d` shows no 0.6.0).
- Every `TurnRequest` Loom constructs compiles against 0.7.0 with no controls set; the existing
  recorded-request tests pass unchanged.
- The package gates of every touched crate pass: `cargo clippy -p <crate> --all-targets -- -D
  warnings` and `cargo test -p <crate>`.
- `CHANGELOG.md` § Unreleased names the pin and what it breaks for Loom's callers, or
  `Breaking: none.`

## Scope

All `crates/*/Cargo.toml` that name `b10x-llm-*`, `Cargo.lock`, `consumers/pinned/Cargo.lock` if it
resolves llm, the call sites the bump forces to change, `CHANGELOG.md`.

## Not in scope

Setting any `controls` field; the two adversary tests ignored until llm resolves one endpoint at a
time (llm issue 36, still open).

## ESS first

No noun, command or outcome changes: the pin adds no declaration to `ess/`. `ess specify validate
--path ess --strict-requires` is run on the pinned tree to show it stays green.
