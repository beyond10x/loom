---
format: aep.planning-md/3
id: story:revalidation-membership-conformance
kind: story
status: draft
title: Conformance exercises the executor's frontier-membership rule
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
scope:
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: ess/domains/run.yaml
revision: 2
---
## Outcome

Loom's conformance suite exercises the executor's frontier-membership rule for
`loom.run.RevalidateSelection` (`not-in-frontier`), instead of the conformance target answering it.

Today the rule is held only by `crates/loom-executor/tests/adversary_run_revalidation.rs`
(`not_in_frontier_follows_the_frontier_actions`). The suite cannot reach it: `ess/domains/run.yaml`
(comment at line 675) marks `not-in-frontier` as `external:`, ESS 0.55.0 refuses the membership guard
over `frontier_actions` (ESS-SYNTH-003, ESS-SYNTH-004), and the synthesized scenarios send
`frontier_actions: []`, so the target in `crates/loom-conformance/src/lib.rs` decides membership from
the forced outcome. Adversary pass 1 of wave 2026-10-07-w2
(`review-result:adversary-w2-loom-ess-conformance-pass-1`, finding 2) measured it: the mutant that
replaces the membership check with `is_empty()` leaves `task conform` green.

The specification's comment ("answered by the executor from the input's frontier_actions") and
ESS's meaning of `external:` (an outcome the input cannot decide) disagree.

## Acceptance

To be settled when ESS can express a membership guard over a list input, or when an option below is
chosen:

- ESS synthesizes scenarios whose `frontier_actions` decide `not-in-frontier`, and the target passes
  the input through unchanged; the `is_empty()` mutant then fails `task conform`;
- or the specification's comment is corrected to ESS's meaning of `external:`, and the rule stays
  held by the executor test alone, named in `ess/SKIPPED.md`'s header or the target's comment.

## ESS first

Depends on ESS: a membership guard over a list field is refused today (ESS-SYNTH-003/004). The
first option is an ESS change outside this repository; the second is a comment change in
`ess/domains/run.yaml`, made first.

## Source

`review-result:adversary-w2-loom-ess-conformance-pass-1`, finding 2.
