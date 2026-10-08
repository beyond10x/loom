---
format: aep.planning-md/3
id: story:engineering-protocols-030-pin
kind: story
status: implemented
title: Loom builds on engineering-protocols 0.3.0 and Canon 0.1.0
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/fixtures/inc-492.fixture.yaml
- confidence: cited
  path: crates/loom-governor/tests/governed_case.rs
- confidence: cited
  path: crates/loom-governor/tests/incident_response_slice.rs
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: inferred
  path: crates/loom-intake-router/tests/adversary_classify.rs
- confidence: inferred
  path: crates/loom-intake-router/tests/classify.rs
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: inferred
  path: crates/loom-intake-slice/tests/adversary_case.rs
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
revision: 21
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T21:28:06Z", actor: "human:timo", revision: 19}
- {from: "proposed", to: "active", at: "2026-10-08T21:28:06Z", actor: "human:timo", revision: 20}
- {from: "active", to: "implemented", at: "2026-10-08T21:59:53Z", actor: "human:timo", revision: 21, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Loom builds on `b10x-canon-engineering` `0.3.0` from `beyond10x/engineering-protocols`, the release
whose `incident.response/1` declares the obligation `investigate_cause`, and on the one Canon that
release names: `b10x-canon` at tag `0.1.0`, no longer `branch = "main"`.

## Why

- engineering-protocols `0.3.0` (2026-10-08) adds `investigate_cause` to `incident.response/1` and
  re-expects fixture `inc-492`; `story:incident-investigation-open` needs it.
- From `0.2.0` on, `b10x-canon-engineering` names Canon by `tag = "0.1.0"`. Loom's
  `loom-governor` and `loom-protocols` name `branch = "main"`; two references build two Canons whose
  types do not match (`AGENTS.md` § Governor), so Loom's Canon reference moves with the pin.

## Acceptance

- Every `b10x-canon-engineering` dependency in the workspace names tag `0.3.0`; `cargo tree
  --locked -i b10x-canon-engineering` shows one copy, from that tag.
- Every `b10x-canon` dependency names tag `0.1.0`; `cargo tree --locked -i b10x-canon` shows one
  copy, from that tag.
- The fixtures copied from the pinned release (`crates/loom-governor/tests/fixtures/`) equal that
  release's files byte for byte, and the three slices (software change, incident response, system
  query) pass on the new pin; where `inc-492`'s new expectations change an assertion, the change is
  the one `story:incident-investigation-open` names.
- `AGENTS.md` § Governor names the Canon tag and the `b10x-canon-engineering` tag `0.3.0`;
  `CHANGELOG.md` (**Unreleased**) names the pin move and the new obligation callers will see.
- `cargo clippy -p <crate> --all-targets -- -D warnings` and `cargo test -p <crate>` exit 0 for
  every crate whose manifest changed.

## ESS first

None: no Loom behaviour or declaration changes; the protocol is engineering-protocols'. Existing
tests cover the move.

## Not in scope

Any change to Canon or engineering-protocols; new Loom behaviour.
