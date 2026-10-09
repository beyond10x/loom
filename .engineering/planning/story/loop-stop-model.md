---
format: aep.planning-md/3
id: story:loop-stop-model
kind: story
status: draft
title: 'LoopStop has one model: its causes declared in ESS, the enum generated'
relations:
- serves: vision:O1
- depends_on: story:compaction-target-bound
scope:
- confidence: cited
  path: crates/loom-executor/src/harness/turn_loop/mod.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom
revision: 2
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

Whether the generated enum keeps the internal `kind` tag and `deny_unknown_fields` the hand-written
one has; settle it at scoping, before the specification changes.

## Source

Wave 2026-10-09-w1, `story:compaction-target-bound`: the new stop's cause could be typed in ESS only
as a second model of `LoopStop`.
