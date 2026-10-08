---
format: aep.planning-md/3
id: story:governor-restart-durability
kind: story
status: active
title: A governor case survives a process restart
relations:
- decomposes: epic:governor
- serves: vision:O1
scope:
- confidence: inferred
  path: crates/loom-governor/Cargo.toml
- confidence: inferred
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml
- confidence: inferred
  path: crates/loom-governor/tests/restart_durability.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T18:56:45Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-08T18:56:45Z", actor: "human:timo", revision: 8}
---
## Outcome

A case the governor holds survives a process restart. Today `epic:governor`'s acceptance ("keeps a
case across a process restart") is undelivered: the governor in `crates/loom-governor` evaluates an
in-process case through `CaseStore` (`story:import-governor`, `story:hosted-governor`,
`story:governor-evaluate`), and no story persists a case. `story:control-plane-storage` makes
storage fallible and excludes a persistence provider, so it does not cover this.

## Acceptance

A `software.change/1` case opened through the governor, with evidence submitted, is read back after
the process that held it ends and a new one starts over the same store: `current_revision`,
`frontier` and `completion` answer as before the restart.

## Open

The store is decided (`decision-blocker:governor-restart-store`, option A): `FileCaseStore` in
`crates/loom-governor` implements `FallibleCaseStore`, one file per case, written to a temporary
file and renamed.

Spec check, 2026-10-08: `CaseState` and `HeldEvidence` are written by hand in
`crates/loom-governor/src/lib.rs:103-124`; no ESS file declares a held case
(`ess/domains/evaluation.yaml:26` declares only the stateless `loom.evaluation.CaseSnapshot`;
`commission.responsibility.Case`, `ess/commission/domains/responsibility.yaml:458-470`, holds protocol
and revision only).

Decided 2026-10-08 with the wave approval: writing a case to disk makes its shape a contract, so
the held case is declared first. Declare the held case (`CaseState`, `HeldEvidence`) as a noun in
Loom's ESS specification (`ess/`), validate it with `ess` 0.56.0, regenerate, and use the
generated type in `FileCaseStore`. If `ess` refuses the shape, the refusal is quoted word for word
in the unit's report; only then does the codec stay beside the hand-written type at
`lib.rs:103-124`, with an `ESS-LIMIT` note citing that refusal.

Constraints the test must hold:
- The frontier id after restart equals the one before only if the case round-trips exactly:
  `frontier_uuid` hashes `id`, `revision` and the decision bytes from `artifacts` and `evidence` in
  stored order (`lib.rs:1232-1234`).
- Protocol definitions are not persisted; the restarted process registers the same protocol before
  reading (`lib.rs:431-432`).
- `FallibleCaseStore` also requires the observation methods (`lib.rs:161-162`); they persist with the
  case.

## Scope

Scoped 2026-10-08 with `story-scoper`.

- Primary surface: `crates/loom-governor` (cited).
- `crates/loom-governor/src/lib.rs`: `CaseState` :104, `HeldEvidence` :119, `CaseStore` :127,
  `FallibleCaseStore` :152, `MemoryCaseStore` :214 as the pattern; `Governor::current_revision`
  :634, `frontier` :638, `completion` :683 (cited).
- New, inferred: `crates/loom-governor/src/file_store.rs` (`FileCaseStore` and the codec, re-exported
  from `lib.rs`); `crates/loom-governor/tests/restart_durability.rs` (the test re-runs its own binary
  as the child process over the `chg-1842` fixture,
  `crates/loom-governor/tests/fixtures/chg-1842.fixture.yaml`, cited); `crates/loom-governor/Cargo.toml`
  only if a dependency is needed.
- Collides with any unit changing `CaseState`, `HeldEvidence`, `CaseStore` or `FallibleCaseStore`.
  Does not touch `crates/loom-commission`, `generated/` or `ess/`.
