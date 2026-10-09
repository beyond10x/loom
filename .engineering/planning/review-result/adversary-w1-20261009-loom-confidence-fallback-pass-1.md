---
format: aep.planning-md/3
id: review-result:adversary-w1-20261009-loom-confidence-fallback-pass-1
kind: review-result
status: active
title: Wave 2026-10-09-w1 adversary, loom story:confidence-fallback, pass 1
relations:
- reviews: story:confidence-fallback
revision: 1
---
## Pass

Adversary pass 1 on `story:confidence-fallback`, wave 2026-10-09-w1, tree
`loom-20261009-w1-confidence-fallback` at b887293 (base 17291ba), plus one untracked adversary test
file `crates/loom-executor/tests/adversary_w1_20261009_confidence_fallback.rs`.

verdict: NEEDS-CHANGE; cases: executed 702→712, red 4; origin: introduced 2 / pre-existing 0 / undecided 0.

## Report

`Confidence::parse` accepts confidences with a leading zero (`00.95`, `01`, `00`). ESS's own decimal
pattern refuses those (`generated/rust/loom/src/json.rs:530`, `decimal_at`: "an optional `-`,
digits without a leading zero, then an optional `.` and digits"), so the acceptance rule "not a
decimal counts as missing" breaks.

Red cases (run of the adversary file alone, `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-executor
--locked --test adversary_w1_20261009_confidence_fallback`):

```
---- adversary_w1_a_leading_zero_confidence_is_not_a_decimal_and_falls_back stdout ----
assertion `left == right` failed: confidence "00.95" is not a decimal in the published pattern, so it counts as missing under threshold 0.9
  left: "tests.run"
 right: "repository.inspect"
---- adversary_w1_a_leading_zero_threshold_is_refused stdout ----
threshold "00.5" is not a decimal in the published pattern
---- adversary_w1_every_confidence_is_a_decimal_in_the_published_pattern stdout ----
Confidence::parse accepts renderings ESS's published Decimal pattern refuses: ["00", "01", "00.95", "000.5", "01.0"]
---- adversary_w1_the_seam_does_not_record_a_leading_zero_confidence stdout ----
assertion `left == right` failed: confidence "00.95" is not a decimal in the published pattern
  left: Some(Decimal("00.95"))
 right: None
test result: FAILED. 6 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=101
```

Whole package with `--no-fail-fast`: 65 binaries, passed 708, failed 4, ignored 1; no other binary
failed, `adversary_w2_conformance_select_action` included.

Findings:

1. NEEDS-CHANGE, introduced, warning: `crates/loom-executor/src/selection.rs:280`,
   `whole.trim_start_matches('0')` lets leading zeros through, so `Confidence::parse` (`:265`)
   accepts `00`, `01`, `00.95`, `000.5`, `01.0`; a fast `00.95` passes a `0.9` threshold,
   `HybridSelector::new` accepts `00.5`, and the seam filter at `:387` records
   `Some(Decimal("00.95"))`. Reached by a host's fast selector through `ActionSelector`; no selector
   in the tree produces such a value.
2. NEEDS-CHANGE, introduced, warning: `crates/loom-executor/tests/confidence_fallback.rs:192` asserts
   `("00.95", "0.9")` is at the threshold, pinning finding 1.

Judgement notes (no case): the story's carried item says "refused"; the code drops the confidence to
`None` and keeps the selection. `CHANGELOG.md:15` does not name the seam's dropping of invalid
confidences for every selector, nor the new public `Confidence` and `InvalidThreshold`.
`select_action` (`selection.rs:430`) still copies `input.confidence` unchecked, as
`ess/domains/run.yaml:654` specifies; the two routes into a `Selection` now disagree on
out-of-range confidences.

Attacked and held: signs, exponents, whitespace, non-ASCII and fullwidth digits, `1.`, `.9`, `--0`,
`-`; `-0` as zero; `1.0000` equal to `1`; thresholds 0 and 1; 10,000-digit fractions; an outside
fast choice at confidence 1 never returned; the stronger selector's outside answer refused by
`select`; an `ApprovalRequired` fast choice returned (admission decides, `lib.rs:488`); conformance
and `SelectAction` unaffected.

```findings
- file: crates/loom-executor/src/selection.rs
  line: 280
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: Confidence::parse strips any run of leading zeros, so 00.95, 01 and 00.5 pass as confidences and thresholds (and are recorded at the select seam, :387) although ESS's published Decimal pattern (generated/rust/loom/src/json.rs:530) refuses a leading zero, breaking the acceptance rule that a non-decimal confidence counts as missing
- file: crates/loom-executor/tests/confidence_fallback.rs
  line: 192
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the unit's own acceptance test asserts that 00.95 is at threshold 0.9 and returns the fast choice, pinning a rendering the published Decimal pattern says is not a decimal
```
