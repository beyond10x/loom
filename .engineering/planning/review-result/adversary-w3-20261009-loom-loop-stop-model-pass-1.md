---
format: aep.planning-md/3
id: review-result:adversary-w3-20261009-loom-loop-stop-model-pass-1
kind: review-result
status: active
title: Wave 2026-10-09-w3 adversary, loom story:loop-stop-model, pass 1
relations:
- reviews: story:loop-stop-model
revision: 1
---
## Pass

Adversary pass 1 on `story:loop-stop-model`, wave 2026-10-09-w3, tree `lw3-lsm`, branch
`impl/loop-stop-model` at eba20a9 (base 6a4b165). Cases committed 29f9847:
`crates/loom-executor/tests/adversary_w3_20261009_loop_stop_model.rs` (5 tests) and
`the_model_grant_admits_turn_loop_alone_and_the_model_alone` in
`crates/loom-executor/tests/adversary2_harness_port.rs`.

verdict: CONFIRMED (no failing case); cases: executed 825 -> 831, red 0; origin: introduced 4 /
pre-existing 0 / undecided 0.

Suite: `cargo test --locked --no-fail-fast -p b10x-loom-executor -p b10x-loom-governor -p
b10x-loom-sdk`: eba20a9 exit 0, 825 passed, 0 failed, 1 ignored (89 binaries); after the cases
exit 0, 831 passed, 0 failed, 1 ignored (90 binaries). `cargo check --locked --workspace
--all-targets` clean.

## Findings

1. `crates/loom-executor/tests/loop_stop_wire.rs` `every_cause_has_a_pinned_case` counts the tags
   of its own fixture and never reads ESS or runs the codec; its comment claimed an added cause
   fails it (mutant, warning, introduced). Comment corrected in 269b55e; the declaration check
   below is the test that fails.
2. `crates/loom-executor/src/harness/turn_loop/stop_codec.rs`: the tag table, read arms and field
   names are copies of the ESS names the compiler does not tie to the declaration (contract drift,
   note, introduced). Covered by
   `every_cause_the_ess_union_declares_reads_and_writes_back_under_its_declared_names`, which reads
   the union from `ess/domains/run.yaml`, and its planted-copy counterpart.
3. `turn_loop/mod.rs` `figure()` saturation had no case above `i64::MAX` (mutant, note,
   introduced). Covered by
   `a_reported_count_above_i64_max_saturates_in_the_stop_and_still_reads_past_the_limit`.
4. `adversary2_harness_port.rs` `LOOM_GRANTS` had no planted case showing it refuses (mutant, note,
   introduced). Covered by `the_model_grant_admits_turn_loop_alone_and_the_model_alone`; no way past
   the grant found. The grant covers all of `crate::model`, as `AGENTS.md` states.

## Attacked, held

Duplicate keys (including `kind`), missing or non-string `kind`, unknown tags, unknown fields per
cause, null fields, fractions, numbers as strings, negatives, numbers above `i64::MAX`
(`arbitrary_precision` included), field order and kebab tags through `LoopOutcome`, `Finished`,
`DelegateFinished` and a nested `Delegated`, the announced map length. No mirror enum of the
causes exists.

## Notes

- Reading is more permissive in one place: `asked_again` accepts values above `u32::MAX`.
- A downstream embedder that calls `LoopStop::is_completed` through a pinned `b10x-loom-sdk` breaks
  when its pin moves; the CHANGELOG names the removal.
