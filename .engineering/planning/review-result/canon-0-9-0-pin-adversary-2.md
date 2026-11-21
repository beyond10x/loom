---
format: aep.planning-md/3
id: review-result:canon-0-9-0-pin-adversary-2
kind: review-result
status: active
title: Adversary pass 2 on story:canon-0-9-0-pin (wave 2026-11-21-w2, unit U1, head 52691f71)
relations:
- reviews: story:canon-0-9-0-pin
revision: 1
---
verdict: broken
unit: story:canon-0-9-0-pin, worktree loom-w1121b-u1 at 52691f71 plus one new untracked test file
cases: executed 26→28, red 1 (`case_composition` 7, `evaluate` 7, `evaluate_adversary` 6, `evaluate_adversary_2` 3, my pass-1 file 3; plus 2 new)
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 paths, in part 6
needs-coordinator: none

**1. Diff.** `git diff --stat` is empty. The only change in the tree is a new file, `crates/loom-governor/tests/adversary_w20261121w2_u1_pass2.rs`.

**2. Cases** (run alone first, log `adv2/alone.log`)
- `evaluate_equals_the_governor_over_histories_with_several_duplicates`: green. It compares `evaluate`'s Canon document with the governor's `decision_document` over four histories:
  - three records for one child;
  - duplicates for two children, interleaved;
  - a record refused alone standing between two records for one child;
  - a child whose first record is refused alone (no result) and whose second applies.
- `a_second_record_for_one_child_costs_a_bounded_multiple_of_deciding`: **red**. It uses the same bound as the existing cost test (40 times plus 0.25 s). 1000 records for 1000 children decide once; one more record for `c0` sends `evaluate` into the in-order fallback:
  `n=1000: decide 0.0220s, set aside 4.6475s, ratio 211.0`
  `setting one duplicate child record aside among 1000 took 4.647s against 0.022s to decide them`

**3. Suite run**, after the cases existed:
`cargo test -p b10x-loom-governor --locked --no-fail-fast --test case_composition --test adversary_w20261121w2_u1 --test adversary_w20261121w2_u1_pass2 --test evaluate_adversary_2 --test evaluate --test evaluate_adversary`
- `adversary_w20261121w2_u1_pass2`: FAILED. 1 passed; 1 failed
- everything else ok: pass-1 file 3, `case_composition` 7, `evaluate` 7, `evaluate_adversary` 6, `evaluate_adversary_2` 3
- exit 101

**4. Findings**
- **P2-F1. The new fallback in `evaluate` is quadratic and no test bounds it** (`crates/loom-governor/src/lib.rs:2345-2353`). NEEDS-CHANGE, warning, introduced.
  - What breaks: one duplicate child record makes `evaluate` re-decide every prefix of the records that pass alone. That is 211 times the cost of deciding at n=1000, and it grows with n squared.
  - What the old cost test still guards: `adversary2_the_set_aside_path_costs_a_bounded_multiple_of_deciding` (`evaluate_adversary_2.rs:373`) still guards a record Canon refuses alone, and only that.
  - What reaches it: `loom evaluate` reading a caller's file (`crates/loom-cli/src/main.rs:808`); one extra record is enough. The cost falls on the caller's own process, and nothing produces `case_outcome` records yet, so I rate it warning.
  - Fix, named not applied: settle the duplicates without re-deciding prefixes. Either keep the first record per `child_case` in request order before the combined decide, or drop the records Canon's `duplicate-child-case` refusal cites after the first, then decide again. Both give the order the doc promises at linear cost.

**5. Attacked and held**
- History equivalence: the four histories above all match the governor byte for byte.
- The fast path hides no difference. Canon's refusals are per record plus one-record-per-child, so any subset of a set Canon accepts is accepted too. If Canon decides all the alone-passing records together, the governor would have applied every one of them as they arrived.
- Order: the doc promises the request's order. A `loom evaluate` caller controls that order through the file; the governor uses the order it received.
- Prose: the CHANGELOG bullet is true of Canon 0.9.0 (`eval/evidence.rs`: `missing-child-case`, `missing-child-outcome`).
  - The `AGENTS.md` sentence and `evaluate`'s doc match the code.
  - One imprecision, not raised: `AGENTS.md` says only the together-refused records are judged again, but the in-order pass re-judges every record that passed alone.
- Nothing else moved: changes are only in `crates/loom-governor`, `AGENTS.md` and `CHANGELOG.md`. `case_composition.rs` only gained cases (the one deleted line is the clippy fix inside a new case). My pass-1 file is byte-equal to what I wrote.

**6. Paths written outside the worktree**
- `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w2/u1/adv2/alone.log`
- `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w2/u1/adv2/suite.log`

Disk at the end: 121G free on `/`, `target/` at 30G. `git status --short`: `?? crates/loom-governor/tests/adversary_w20261121w2_u1_pass2.rs`

```findings
- file: crates/loom-governor/src/lib.rs
  line: 2345
  category: mutant
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the in-order fallback re-decides every prefix of the records that pass alone, so one duplicate child record costs 211x deciding at n=1000 and no test bounds that path
```
