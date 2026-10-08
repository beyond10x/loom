---
format: aep.planning-md/3
id: decision-blocker:run-storage-failure-spec
kind: decision-blocker
status: cleared
title: Nobody has decided whether a run storage failure is declared in the specification
relations:
- blocks: story:control-plane-storage
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-08T17:25:38Z", actor: "human:timo", revision: 3}
---
## Question

Is a run storage failure (`story:control-plane-storage`) declared in
`ess/commission/domains/responsibility.yaml`, or is it an adapter failure outside the
specification, as `FallibleCaseStore` is?

## Why nothing settles it

- The generated `RunStorage` cannot fail (`generated/rust/commission/src/behaviour.rs`).
- The only error of `StartRunBehavior` and `SuspendRunBehavior` is `UnmetObligation`
  (`generated/rust/commission/src/responsibility.rs`).
- `FallibleCaseStore` (`crates/loom-governor/src/lib.rs`) set the precedent of a hand-written port
  beside generated code for an infrastructure failure.

## Options

- A: declare a storage error on `StartRun`/`SuspendRun` in the specification and regenerate. Cost:
  specification change and a changed generated signature for every caller; unchecked whether
  `ess` can express a non-domain error on a behaviour.
- B: a hand-written fallible run interface in `crates/loom-commission/src/runtime.rs`, implemented
  automatically for every generated behaviour, plus a new `LoopFailure` variant; no specification
  change. Cost: smallest; the failure is not in the specification.
- C: ask the ess repository to make generated behaviour storage fallible. Cost: waits on an ess
  release; covers every ESS consumer.

Recommendation: B.

## What it stops

`story:control-plane-storage`.

## Decision

Option A where the specification can express it, else C; never B, decided 2026-10-08. Probe the newest `ess`: if it declares a storage error on `StartRun`/`SuspendRun`, declare it and regenerate; if it refuses, ask the ess repository for fallible generated storage with the probe and its refusal, and `story:control-plane-storage` stays draft until it lands. A hand-written fallible interface outside the specification (B) is excluded.
