---
format: aep.planning-md/3
id: story:ess-hard-gate
kind: story
status: implemented
title: Hold the Loom ESS specification to a hard gate in task check, with no open question
refs:
- provider: atlas
  reference: adr:0076
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: crates/loom/tests/ess_gate.rs
- confidence: cited
  path: ess/domains/run.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T01:17:41Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-04T01:17:41Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-04T01:42:22Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":3,"verification":1}}}
---
## Outcome

The Loom ESS specification under `ess/` is held by a hard gate in `task check` (operator rule
2026-10-04, Atlas ADR 0076): it validates (`ess specify validate --path ess --strict-requires`),
compiles (`ess specify compile --path ess --format json`), synthesizes its conformance suite with
0 refusals (`ess verify conform synthesize`), and carries no open question — no `UNMAPPED:` string
in any file under `ess/`. This story closes every marker in `ess/domains/run.yaml` as it stands at
`b08be9c` and installs the gate, so no later story can reopen one.

The four resolutions below were each applied to a scratch copy of `ess/` on 2026-10-04 with
ess 0.52.0 (the `requires: ess 0.52.0` of `ess/ess-inputs.yaml`): `validate --strict-requires`
printed `loom v1 — 2 file(s), valid`, `compile` exited 0, and `conform synthesize` printed
`0 scenario(s) (0 authored), 0 refusal(s)`; a grep for `UNMAPPED:` over the copy found nothing.

- **(a) Header.** The file header comment of `ess/domains/run.yaml` (lines 1-4) no longer contains
  the marker string: "Every relation the sources do not settle is marked UNMAPPED: and stays out of
  the model until a story decides it" becomes "Every relation the sources do not settle stays out of
  the model until a story decides it".
- **(b) Commission run.** `loom.run.Session.commission_run` (`run.yaml:51-52`, today `String`) is
  typed `loom.run.CommissionRunId`, a new `newtype` of `Uuid` whose comment names Commission's
  `commission.responsibility.RunId`. No cross-system `relations:` entry: ESS 0.52.0 cannot name
  another system's entity. The marker at `run.yaml:44-45` is deleted.
- **(c) Selection confidence.** `loom.run.Selection` gains `confidence: Optional<Decimal>`; the
  marker at `run.yaml:99` is deleted. Not `Binary64`: on the same probe with
  `Optional<Binary64>`, `ess verify conform synthesize` refused with
  `entity loom.run.Selection.fields.confidence.of: UnsupportedPrimitive: finite Binary64 is not
  admitted by the current conformance suite and codecs` and exited 1. `ess generate synthesize
  --target rust` renders the field as `Option<primitives::Decimal>` (a string newtype).
- **(d) Catalogue per turn.** Operator decision 2026-10-04: a catalogue is projected once per turn
  (`decision-blocker:catalogue-ownership`, cleared). `loom.run.ActionCatalogue` gains
  `turn_id: loom.run.TurnId`, and `loom.run.Turn` gains the relation `catalogue` (owns, cardinality
  one, via `turn_id`). The marker at `run.yaml:81-82` is deleted.
- **(e) Gate.** A Rust integration test `crates/loom/tests/ess_gate.rs` runs the four gate steps
  against `ess/` and fails on the first that does not hold, naming it. The gate is a function over
  a directory, so the test can run it on `ess/` and on a temporary copy of it. The suite is written under
  `CARGO_TARGET_TMPDIR`, never `/tmp` and never inside `ess/`. The marker scan reads every file
  under `ess/` and names each file and line holding `UNMAPPED:`. `Taskfile.yml` gains task
  `ess-gate` (`cargo test -p b10x-loom --test ess_gate --locked`), and `check` lists it as its own
  step.
- **(f) Rule.** `AGENTS.md` § ESS states the hard gate: the four conditions, that `task check`
  enforces them through `task ess-gate`, and that an open question is settled (in a story or a
  `decision-blocker`) before the specification changes, never written into `ess/` as a marker.

## Shared surface

First link of the `epic:loom-native-harness` chain over `ess/domains/run.yaml`; `Taskfile.yml` is
edited along the same chain. `story:agent-executor` depends on it and generates
`generated/rust/loom/` from the specification this story leaves. The whole order is in
`story:agent-executor` § Shared surface.

## ESS first

Change `ess/domains/run.yaml` as in (a) to (d); `ess specify validate --path ess --strict-requires`
passes. No other ESS file changes. No model is generated here (`story:agent-executor` owns
`generated/rust/loom/`).

## Domain relations

- `loom.run.Turn -> loom.run.ActionCatalogue`: owns, one, via `ActionCatalogue.turn_id` — operator
  decision 2026-10-04 recorded on `decision-blocker:catalogue-ownership`. A turn's catalogue does
  not outlive the turn.
- `loom.run.Session -> commission.responsibility.Run`: a typed id (`CommissionRunId`), not a
  `relations:` entry, because ESS 0.52.0 has no cross-system relation. The Commission side is
  commission `ess/domains/responsibility.yaml`, `commission.responsibility.RunId`, at `013e392`.

## Scope

- `ess/domains/run.yaml`
- `crates/loom/tests/ess_gate.rs` (new)
- `Taskfile.yml` (task `ess-gate`; one line in `check`)
- `AGENTS.md` (§ ESS)

## Constraints

The test uses only the standard library and the `ess` binary on `PATH`; `crates/loom/Cargo.toml`
and `Cargo.lock` do not change. Anything that runs is Rust (`AGENTS.md` § Rules).

## Acceptance

`task check` passes on the story's tree and runs, through the task `ess-gate`, the test `ess_gate`
in `crates/loom/tests/ess_gate.rs`, which checks items 1 to 7. The test never calls `task`.

1. `ess specify validate --path ess --strict-requires` exits 0.
2. `ess specify compile --path ess --format json` exits 0, and its output declares
   `loom.run.CommissionRunId` as a newtype of `Uuid`, `loom.run.Session.commission_run` of that
   type, `loom.run.Selection.confidence` as an optional of the `decimal` primitive
   (`Optional<Decimal>`), and on `loom.run.Turn` a
   relation `catalogue` (owns, one, target `loom.run.ActionCatalogue`, via `turn_id`).
3. `ess verify conform synthesize --path ess --out <file under CARGO_TARGET_TMPDIR>` exits 0 and
   reports 0 refusals.
4. No file under `ess/` contains the string `UNMAPPED:`.
5. Applied to a copy of `ess/` under `CARGO_TARGET_TMPDIR` with the line `# UNMAPPED: probe`
   appended to `domains/run.yaml`, the marker scan fails and names `domains/run.yaml` and the
   appended line's number.
6. The whole gate, run over a temporary copy of `ess/` under `CARGO_TARGET_TMPDIR` with the line
   `# UNMAPPED: probe` appended to `domains/run.yaml`, fails: steps 1 to 3 pass on the copy, and the
   marker scan fails and reports `domains/run.yaml` and the appended line's number. That
   `task ess-gate`, and with it `task check`, fails on a real tree carrying a marker is observed by
   CI running `task check`, not by this test.
7. `AGENTS.md` § ESS names `task ess-gate` and the four conditions.

## Source

Atlas ADR 0076 (operator rule 2026-10-04); operator decision 2026-10-04 on
`decision-blocker:catalogue-ownership`; probe of 2026-10-04 with ess 0.52.0 on a scratch copy of
`ess/` at `b08be9c`; `ess:specifying` § Conformance is a record, not a claim.
