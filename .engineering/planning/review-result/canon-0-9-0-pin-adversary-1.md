---
format: aep.planning-md/3
id: review-result:canon-0-9-0-pin-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:canon-0-9-0-pin (wave 2026-11-21-w2, unit U1, head adbcf507)
relations:
- reviews: story:canon-0-9-0-pin
revision: 1
---
verdict: broken
unit: story:canon-0-9-0-pin, worktree loom-w1121b-u1 at adbcf507 plus one new untracked test file
cases: executed 5→8, red 1 (`case_composition` 4 and `canon_decision_bytes` 1 from the implementor's gate, plus my 3)
origin: introduced 1 / pre-existing 0 / undecided 0
wrote-outside-worktree: 9 paths, in part 6
needs-coordinator: none

**1. Diff.** `git diff --stat` is empty. The only change in the tree is a new file, `crates/loom-governor/tests/adversary_w20261121w2_u1.rs`. No implementation file was touched, not even briefly.

**2. Cases** (run alone first, log `adv1/adv-alone.log`)
- `evaluate_sets_aside_a_second_record_for_one_child_as_the_governor_does`: **red**. The governor gets two records for one child (`open`, then `accepted`), keeps the first and decides. `evaluate` gets the same two and refuses:
  `evaluate refused a snapshot with no termination because of two evidence records, where the governor sets the second aside and decides: input Snapshot, index None, code duplicate-child-case, message child case `CHG-1` has more than one case_outcome record: `o1`, `o2``
- `a_case_outcome_record_about_another_artifact_counts_at_any_revision`: green.
- `a_children_form_only_in_a_nested_precondition_reads_case_outcome_at_any_revision`: green.

**3. Suite run**, after the cases existed:
`cargo test -p b10x-loom-governor --locked --no-fail-fast --test case_composition --test adversary_w20261121w2_u1 --test canon_decision_bytes`
- `adversary_w20261121w2_u1`: FAILED. 2 passed; 1 failed
- `canon_decision_bytes`: ok. 1 passed
- `case_composition`: ok. 4 passed
- exit 101. Afterwards I ran rustfmt on my file; it only re-wrapped one `assert_eq!`.

**4. Findings**
- **F1. `evaluate` refuses the whole request and blames the snapshot** (`crates/loom-governor/src/lib.rs:2324-2336`). NEEDS-CHANGE, warning, introduced.
  - What breaks: Canon 0.9.0 adds the first refusal over a set of records, `duplicate-child-case`. `evaluate`'s fallback tests each record alone, so both records pass, and the set is still refused. The error is reported as `EvaluationInput::Snapshot`.
  - What it contradicts: its own doc (`:2235-2237`, `:2328`, "what Canon still refuses with the rest is the snapshot's termination") and `AGENTS.md:480`. The governor's `applies` (`:2044-2056`) decides record by record against what already applies, so the two disagree on the same history.
  - What reaches it: `loom evaluate` (`crates/loom-cli/src/main.rs:808`) given a child's `open` and later outcome records. Nothing writes `case_outcome` today, which is why I rate it warning; it becomes a blocker once `story:child-case-runtime` writes them.
  - Fix, named not applied: build `applying` one record at a time, as `applies` does.
- **F2. CHANGELOG `**Breaking:**` bullet misses a reserved kind name** (`CHANGELOG.md:25`). CONFIRMED, note, introduced. It doesn't say that Canon now refuses any record of a kind named `case_outcome` without `child_case`, so a host protocol already using that kind name breaks. No engineering-protocols 0.16.0 protocol declares it.

**5. Attacked and held**
- The `Children` arm: it handles two `children` claims and nesting under `all`/`any`/`not`. A protocol without a `case_outcome` kind can't compile (Canon's `validate` requires it). Records about any subject count, at any revision.
- Mutants, by reasoning only (I did not edit `lib.rs`):
  - A no-op arm, or one that pushes into `matches`: killed by `case_composition.rs:265`.
  - An arm limited to the opening's subject, or a walk that skips action preconditions, `any` or `not`: would have survived `case_composition.rs`. My two green cases cover them, but I did not run them against a mutant.
- Rule B: an action is not useful forever or never because of this arm. Declaring `may_produce: case_outcome` closes it once a record applies; without it, the action is useful once per revision.
- Lock: `cargo metadata --locked --offline` exits 0 and leaves the lock unchanged. The diff against base is exactly the 5 Canon and engineering blocks. One `b10x-canon` and one `b10x-canon-expr` (0.9.0 `8f1f452`) and one engineering registry (0.16.0 `70e6eff`). `consumers/pinned/Cargo.lock` is gitignored and matches.
- Touched tests: only doc comments, one assertion message, and `child_case: None`. No assertion was relaxed, nothing was ignored, no value re-pinned.
- Protocols and fixtures are byte-equal between engineering-protocols 0.15.2 and 0.16.0.
- Boundaries: no Canon evaluation outside the governor, no change to the commission manifest, no `/home/` literal, nothing under `.engineering/`, no network call in the new test.
- Left to `story:child-case-runtime`, not raised: a parent stuck on `open` (one record per child) and the opening-cycle check. That story's scope already covers both.

**6. Paths written outside the worktree.** I ran `cargo metadata` and `cargo tree` (no build) with `SLOT_SHIM_OFF=1`; the first ran at 42G free. Files, all under `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w2/u1/adv1/`:
`base-lock`, `lock-before`, `metadata.json`, `metadata.err`, `tree-d.txt`, `wait-disk.sh`, `wait-disk.out`, `wait-file.sh`, `adv-alone.log`, `suite.log`, `suite2.log`.

Disk at the end: 62G free on `/`, `target/` at 26G. `git status --short`: `?? crates/loom-governor/tests/adversary_w20261121w2_u1.rs`

```findings
- file: crates/loom-governor/src/lib.rs
  line: 2324
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: evaluate tests each record alone, so Canon 0.9.0's set-level duplicate-child-case refuses the whole request as a Snapshot refusal where the governor keeps the first record and decides
- file: CHANGELOG.md
  line: 25
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the Breaking bullet does not say Canon 0.9.0 now reserves the kind name case_outcome and refuses its records without child_case
```
