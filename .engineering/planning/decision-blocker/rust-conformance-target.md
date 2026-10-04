---
format: aep.planning-md/3
id: decision-blocker:rust-conformance-target
kind: decision-blocker
status: cleared
title: Nobody has decided how the Rust Loom is held to its ESS conformance suite
relations:
- blocks: epic:loom-native-harness
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-04T00:01:25Z", actor: "human:timo", revision: 3}
---
## Question

How is the Rust Loom implementation held to its synthesized ESS conformance suite?

## Why it is open

`epic:loom-native-harness` accepts on "`task check` runs the Loom ESS conformance suite". With ess
0.52.0, `ess verify conform synthesize --target` offers `ir`, `go` and `typescript`, and
`ess verify conform run --target` offers `billing`, `oracle-fixture` and `interpreted`; per the
`ess:testing-conformance` skill none of the built-in targets runs an adopter implementation
(`interpreted` reports every scenario unsupported). A Go or TypeScript test package committed to
Loom would break the workspace rule that committed running code is Rust. Options include a Rust
suite target in ESS (filed on `beyond10x/ess`), or accepting a synthesized-only suite until then.

## What it stops

The "runs the conformance suite" clause of the epic acceptance. `story:loom-ess-conformance`
synthesizes and drift-checks the suite and is not stopped.

## Source

Decomposition of `epic:loom-native-harness`; `ess verify conform synthesize --help` and
`ess verify conform run --help` (ess 0.52.0).

## Answer (2026-10-04)

Not an open decision: Mandate already runs its synthesized ESS suite through a Rust target built on the `ess-conformance` crate (`mandate/crates/mandate-conformance/Cargo.toml`; Commission's I-007 story follows the same route). Loom's I-006 does the same.
