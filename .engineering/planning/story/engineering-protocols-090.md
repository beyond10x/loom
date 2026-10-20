---
format: aep.planning-md/3
id: story:engineering-protocols-090
kind: story
status: implemented
title: Loom builds on engineering-protocols 0.9.0, one copy beside takt
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-cli/tests/catalog_route.rs
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/canon_decision_bytes.rs
- confidence: cited
  path: crates/loom-governor/tests/governed_case.rs
- confidence: cited
  path: crates/loom-governor/tests/incident_response_slice.rs
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_w20261020_u1.rs
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "active", to: "implemented", at: "2026-10-20T03:29:52Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":5,"verification":1}}}
---
## Outcome

Loom builds on engineering-protocols 0.9.0, the tag takt pins (takt `Cargo.toml:13-14`), so a
crate depending on both resolves one copy of `b10x-canon-engineering`. Loom pins 0.8.0 in four
manifests (`crates/loom-governor/Cargo.toml:19`, `crates/loom-protocols/Cargo.toml:15`,
`crates/loom-intake-slice/Cargo.toml:18`, `crates/loom-intake-router/Cargo.toml:15`; `Cargo.lock`
at `dc231c7`) and names the tag in four more places (`crates/loom-governor/src/lib.rs:127`,
`crates/loom-protocols/src/lib.rs:153`, `website/docs/reference/sdk-stability.md:48`,
`AGENTS.md:324`).

engineering-protocols 0.9.0 adds one built-in protocol, `eval.gated_change/1`, and changes no
existing one (its `CHANGELOG.md`, `## [0.9.0] - 2026-10-17`, has only `Added` and `Limits`).
`ProtocolCatalog::engineering()` reads the registry's list (`crates/loom-protocols/src/lib.rs:142-146`),
and `bundled()` starts from it (`:111-112`), so Loom's bundled catalog gains `eval-gated-change@1`.

## Why

takt's first story pins loom-sdk next to its own engineering-protocols and fails its one-copy
check until Loom moves; every later takt story depends on that one. Two tags build two
`b10x-canon-engineering` crates whose protocol types do not match (AGENTS.md § Governor says the
same of Canon).

## Acceptance

- Every manifest names `tag = "0.9.0"`; `cargo tree --locked -i b10x-canon-engineering` prints one
  package, and `cargo tree --locked -i b10x-canon` prints one package at tag `0.6.0`.
- Each of the twelve lines that name the engineering-protocols tag today names `0.9.0`: the four
  manifests above, `crates/loom-governor/src/lib.rs:127`, `crates/loom-protocols/src/lib.rs:153`,
  `AGENTS.md:324`, `website/docs/guides/move-to-loom.md:49`, the doc comments
  `crates/loom-governor/tests/canon_decision_bytes.rs:8`, `governed_case.rs:9` and
  `incident_response_slice.rs:23`, and the pin table `website/docs/reference/sdk-stability.md:48`
  (a row for the release that ships the move). `README.md:69` names Loom's own release `0.8.0` and
  stays.
- `crates/loom-protocols/tests/` gains `the_bundled_catalog_lists_eval_gated_change`: `bundled()`
  holds `eval-gated-change@1` beside the existing protocols, and `b10x-loom protocols list` prints it.
- A routed pick of `eval-gated-change@1` stops `NoLocalExecutor` naming it, as other protocols the
  slice does not perform do (`crates/loom-cli/tests/` case on the fixture catalog).
- `CHANGELOG.md` names the move and the new bundled protocol; `task check` exits 0, including
  `task consumer-check`.

## ESS first

No noun the specification lacks: the move adds a protocol to the catalog the router reads, and
`intake.routing` declares no list of protocol names. The red step is
`the_bundled_catalog_lists_eval_gated_change`, failing on 0.8.0.

## Not in scope

- A local executor for `eval-gated-change@1`.
- Moving Canon: 0.9.0 resolves against Canon 0.6.0, as takt's graph does (takt `Cargo.toml:12-13`).

## Scope

Crates: `loom-governor`, `loom-protocols`, `loom-intake-slice`, `loom-intake-router`, `loom-cli`
(test only).

- the four `Cargo.toml` files above, `Cargo.lock`
- `crates/loom-governor/src/lib.rs:127` (one doc line), `crates/loom-protocols/src/lib.rs:153`,
  `crates/loom-protocols/tests/`
- `crates/loom-governor/tests/canon_decision_bytes.rs`, `governed_case.rs`,
  `incident_response_slice.rs` (one doc line each)
- `website/docs/reference/sdk-stability.md:48` (the pin table only),
  `website/docs/guides/move-to-loom.md:49`, `AGENTS.md:324`
- `CHANGELOG.md`

Shared with `story:governor-exposes-canon-decision` (`crates/loom-governor/src/lib.rs`, a different
region; it depends on this story) and `story:authority-sees-the-request` (the stability page, a
different row).

## Correction at close (wave 2026-10-20-w1)

- The adversary found two more breaks the new bundled protocol makes, both now in the `Breaking:`
  sentence: `b10x-loom resume` refuses a commission `run --state` kept under 0.26.0 (its kept route
  names the catalog without `eval-gated-change@1`), and a user-store definition named
  `eval-gated-change@1` fails every command that reads the store (`3bf691bc`).
- `website/docs/guides/move-to-loom.md:57` named Canon `0.4.0`; corrected to `0.6.0` in the same
  commit (pre-existing).
- The pin table at `website/docs/reference/sdk-stability.md:48` got an `Unreleased` row, renamed
  `0.27.0` by the release commit; `adversary_w8_pin.rs` lists `eval-gated-change@1` in the exact set.
- `Cargo.lock` moves only the three engineering-protocols packages; `cargo update -p` also moved
  unrelated edges (`windows-sys`, `getrandom`), which the implementor reverted.
- Found, not fixed here: llm's tag is stale in `AGENTS.md:748` (`0.4.0`) and
  `website/docs/guides/move-to-loom.md:48` (`0.3.1`); `Cargo.lock` holds `0.7.0`. Filed as
  `story:doc-dependency-tags-match-lock`.
