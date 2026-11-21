---
format: aep.planning-md/3
id: upstream-blocker:engineering-protocols-on-canon-0-9-0
kind: upstream-blocker
status: cleared
title: engineering-protocols has no release built on Canon 0.9.0
tags:
- sdc
relations:
- blocks: story:canon-0-9-0-pin
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-11-21T01:13:01Z", actor: "human:timo", revision: 2}
---
## What is missing

A release of beyond10x/engineering-protocols built on Canon 0.9.0. Its newest tag `0.15.2` is its
`origin/main` (`1deb2ca`) and pins `b10x-canon` at tag `0.8.0`
(`crates/canon-engineering/Cargo.toml:18`) and `b10x-canon-expr` at tag `0.8.0`
(`crates/canon-engineering-assertions/Cargo.toml:14`). Read 2026-11-21.

## Why Loom cannot move alone

- Loom hands the engineering crate's Canon `Protocol` to its own Canon:
  `crates/loom-governor/src/lib.rs:910` (`declared_outcomes(&builtin(protocol)?.model)`) and
  `:1598-1599` (`compile(&builtin.model)`). With two Canon copies the types differ (inferred, not
  compiled).
- Three tests refuse a second copy: `crates/loom-protocols/tests/adversary_w8_pin.rs:103-118`,
  `crates/loom-executor/tests/intake_import.rs:26-32`,
  `crates/loom-xtask/tests/pinned_consumer.rs:107-153`.
- `AGENTS.md` § Governor: Canon moves "together with the `b10x-canon-engineering` tag".

## What clears it

An engineering-protocols tag whose two pins name Canon tag `0.9.0`. Canon 0.9.0 breaks Rust
consumers that match exhaustively over `Predicate` or build `Action` or `EvidenceRecord` with
struct literals (`~/beyond10x/canon/CHANGELOG.md`, entry 0.9.0).

## Who

Session `sdc` was told on 2026-11-21 and answered that it raises the request; it has no
engineering-protocols session open.
