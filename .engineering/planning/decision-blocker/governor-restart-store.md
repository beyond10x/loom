---
format: aep.planning-md/3
id: decision-blocker:governor-restart-store
kind: decision-blocker
status: cleared
title: Nobody has decided which store keeps a governor case across a process restart
relations:
- blocks: story:governor-restart-durability
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T17:25:38Z", actor: "human:timo", revision: 3}
---
## Question

Which store keeps a governor case across a process restart (`story:governor-restart-durability`):
a file store Loom ships, or only the host-filled `FallibleCaseStore` port?

## Why nothing settles it

- `crates/loom-governor/src/lib.rs` defines `FallibleCaseStore` (host-owned, atomic writes) and
  ships only `MemoryCaseStore`.
- `CaseState` derives no serialisation, so no store can write a case to disk today.
- `epic:governor` acceptance says a case is kept "across a process restart"; with the port alone
  no Loom test can show a real restart.

## Options

- A: `FileCaseStore` in `crates/loom-governor` implements `FallibleCaseStore` (one file per case,
  write to a temporary file then rename); `CaseState` gains serialisation; the acceptance test ends
  a child process and reopens the store in a new one. Cost: one story; Loom owns an on-disk format.
- B: port only; `CaseState` gains serialisation and the testkit gets a reopenable fake store.
  Cost: smallest; no real restart is shown in Loom; every host writes its own store.
- C: declare case persistence in the ESS specification and generate the store. Cost: largest;
  the generated `RunStorage` cannot fail (`story:control-plane-storage`), so it waits on that.

Recommendation: A.

## What it stops

`story:governor-restart-durability` and the restart clause of `epic:governor`.

## Decision

Option A, decided 2026-10-08. `FileCaseStore` in `crates/loom-governor` implements `FallibleCaseStore`; the acceptance test ends a child process and reopens the store in a new one. Spec first: if `CaseState` is a specification noun, its persisted shape is declared or checked in the specification before code; a shape the specification cannot express is reported, not hand-written.
