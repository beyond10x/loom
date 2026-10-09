---
format: aep.planning-md/3
id: story:stale-governor-comments
kind: story
status: implemented
title: Two governor doc comments describe code that has since changed
relations:
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T23:16:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-08T23:16:41Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-09T16:30:02Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

Two doc comments describe code that has changed since they were written; both say what the code
does today.

1. `crates/loom-governor/src/lib.rs:67` says "Commission is pinned to `e61e4f0`". Commission is a
   path dependency of the governor (`crates/loom-governor/Cargo.toml:16`,
   `b10x-loom-commission = { path = "../loom-commission" }`) since its history was merged into
   Loom; no commit pin exists. The sentence names the path dependency instead.
2. `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs:139-141` says
   `SessionFile::load` reads the session "through a `Value` (`session.rs:498`, `:527`)". Since
   afa7937, `SessionFile::parse` (`crates/loom-executor/src/session.rs:497`) reads only the
   version through a `Value` and the session itself from the text. The comment says so and names
   the function, not line numbers.

## Acceptance

`grep -rn 'e61e4f0' crates/` prints nothing, the test's doc comment names `SessionFile::parse`
reading the session from the text, and `cargo test -p b10x-loom-governor --locked` passes with
the same test count as before.

## ESS first

Exempt: comments only, no behaviour change.

## Source

Found while closing wave 2026-10-08-w8; checked against `main` at 5e3d0cb.
