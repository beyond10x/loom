---
format: aep.planning-md/3
id: review-result:adversary-w1-20261009-loom-confidence-fallback-pass-2
kind: review-result
status: active
title: Wave 2026-10-09-w1 adversary, loom story:confidence-fallback, pass 2
relations:
- reviews: story:confidence-fallback
revision: 1
---
## Pass

Adversary pass 2 on `story:confidence-fallback`, wave 2026-10-09-w1, tree
`loom-20261009-w1-confidence-fallback` at 6ca6a2b (base 17291ba), plus one untracked file
`crates/loom-executor/tests/adversary_w1p2_20261009_confidence_fallback.rs`.

verdict: nothing found; cases: executed 712→722, red 0; origin: introduced 0 / pre-existing 0 / undecided 0.

## Report

Correction check (`b887293..6ca6a2b`): no assertion dropped; every list in `confidence_fallback.rs`
only gains entries (`00`, `01`, `00.95`, `000.5`, `01.0` count as missing; `00.5`, `01` refused as
thresholds; `00.95` not recorded at the seam). One row replaced: `("00.95", "0.9")` became
`("-0", "0")` in `equal_values_written_differently_are_at_the_threshold`, the row pass 1 said pinned
the defect, with `00.95` moved to the missing list. Pass 1's cases are committed unchanged and green.
`Confidence::parse` calls the generated `decimal_at` first, so `whole` is exactly `0` or `1`.

Cases added (all green): two seeded property tests against the specification's `decimal_at` and a
64-digit padded reference (20,000 inputs, each of three classes over 1,000; 300 renderings ordered
pairwise, the hybrid's decision over 150 pairs); exact boundary with 5,000-digit fractions; `-0`
spellings as threshold and confidence; `-0.0` recorded as written under `select`; both selectors
receive the same context and candidates; both selectors erring in all four combinations refuses with
the stronger's error; a stronger error after a confident outside fast choice is the refusal; a
fallback records the stronger's confidence, never the fast one; a nested hybrid applies both
thresholds.

Run of the file alone: `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered
out; finished in 0.23s`, EXIT=0. Suite (`CARGO_INCREMENTAL=0 cargo test -p b10x-loom-executor
--locked --no-fail-fast`): 66 binaries, 722 passed, 0 failed, 1 ignored, EXIT=0.

Pass 1's judgement notes stay open, not re-raised: `select_action` copies the confidence unchecked;
the story's carried item says "refused" while the code drops the confidence.

```findings
[]
```
