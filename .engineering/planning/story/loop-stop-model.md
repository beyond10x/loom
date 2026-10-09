---
format: aep.planning-md/3
id: story:loop-stop-model
kind: story
status: active
title: 'LoopStop has one model: its causes declared in ESS, the enum generated'
relations:
- serves: vision:O1
- depends_on: story:compaction-target-bound
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: inferred
  path: crates/loom-executor/src/compaction.rs
- confidence: cited
  path: crates/loom-executor/src/harness/governed.rs
- confidence: inferred
  path: crates/loom-executor/src/harness/turn_loop/answer.rs
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/event.rs
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/mod.rs
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/tests.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w2_harness_loop_port.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w3_compaction_contract.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w4_20261009_compaction_target_bound.rs
- confidence: cited
  path: crates/loom-executor/tests/compaction_contract.rs
- confidence: cited
  path: crates/loom-executor/tests/compaction_target_bound.rs
- confidence: cited
  path: crates/loom-executor/tests/harness_loop_port.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: inferred
  path: website/data/ess/loom-run.domain-graph.json
- confidence: inferred
  path: website/docs/reference/ess/loom-run.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T19:23:53Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-09T19:23:53Z", actor: "human:timo", revision: 7}
---
## Outcome

`LoopStop` (`crates/loom-executor/src/harness/turn_loop/mod.rs:126-177` on `main` at 5e3d0cb), the
hand-written serde enum that says why the governed loop stopped, has one model: `ess/domains/run.yaml`
declares its causes, and the Rust enum is generated from that declaration. Today the specification
declares only the coarse ending (`loom.run.RunEnding`, `[Answered, Stopped, Failed]`, `:63-69`) and
names the causes of a `Stopped` ending in a comment; wave 2026-10-09-w1 added
`LoopStop::ContextAboveTrigger` and named it there the same way, because a stop-cause enum declared
beside the hand-written one would be a second model.

## Acceptance

`ess/domains/run.yaml` declares every `LoopStop` cause with its fields; `task generate` produces the
enum `turn_loop` uses; `task no-hand-model` reports no hand-written `LoopStop`; `task drift` is clean;
the JSON a filed session or a `LoopEvent::Finished` carries is unchanged (each existing `kind` tag
reads back), shown by a round-trip case per variant.

## ESS first

The unit's first commit changes only `ess/`: the declaration of the stop causes and, if the run's
filed session or its event names the cause, that field. The red test is `task no-hand-model` (the
hand-written enum shadows the declared one) or the story's round-trip case.

## Open at drafting

Settled at scoping (2026-10-09), from a trial on a copy of `ess/` with `ess` 0.56.0:

- **Declaration.** `loom.run.LoopStop` is a `kind: union` with `tag: kind`, one variant per cause
  keyed by its kebab-case tag (`max-turns`, `context-above-trigger`, …), each naming a payload
  struct `loom.run.LoopStop<Cause>` with the cause's fields; `completed` has no payload. A union
  variant with no payload needs `format: ess/22`, so `ess/system.yaml` moves from `ess/20` to
  `ess/22`; nothing else in the generated crate or the conformance suite changes but digests.
  `validate --strict-requires`, `compile` and `verify conform synthesize` exit 0 on the trial.
- **What ESS cannot say.** The generated enum derives no serde and has no codec; a union takes no
  `closed`; variants cannot carry inline fields; there is no unsigned integer (`Integer` is `i64`).
- **Decision.** The generated enum is the one model: `turn_loop` re-exports it and every match
  uses it (variants become tuple variants over the payload structs; `u64`/`u32` fields become
  `i64`, a breaking change the CHANGELOG names). The JSON of `LoopOutcome.stop`,
  `LoopEvent::Finished.stop` and `LoopEvent::DelegateFinished.stop` stays byte-for-byte what it
  is: a hand-written serde codec module, used through `#[serde(with = …)]` on those three fields,
  writes and reads the internal `kind` tag and the field names, refuses unknown fields, unknown
  tags and negative numbers. The codec declares no type of its own that mirrors the causes (no
  second enum), so it is a wire codec of the declared type, not a second model.
- **Not affected.** A filed session stores only `RunEnding` (`crates/loom-executor/src/session.rs`,
  `Ending`), not `LoopStop`.
- **Red test of the ESS-only first commit:** `task no-hand-model` (the declared name
  `loom.run.LoopStop` shadows the hand-written enum at `turn_loop/mod.rs`), and `task drift`.

## Source

Wave 2026-10-09-w1, `story:compaction-target-bound`: the new stop's cause could be typed in ESS only
as a second model of `LoopStop`.
