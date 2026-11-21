---
format: aep.planning-md/3
id: review-result:ess-format-31-adversary-1
kind: review-result
status: active
title: Adversary pass 1 on story:ess-format-31 (wave 2026-11-21-w4, unit U2, head 8b69b0b6)
relations:
- reviews: story:ess-format-31
revision: 1
---
unit: story:ess-format-31, head `8b69b0b6` (base `63668a97`), plus my one untracked test file
verdict: green (nothing found: no command's answer, event or stored row changes)
cases: executed 43→46 (`cargo test -p b10x-loom-resource`; 43 is the same binaries in the implementor's `g03-test-a.log`), red 0
origin: introduced 0, pre-existing 0, undecided 0
wrote-outside-worktree: 1 path, `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u2/adv1/`
needs-coordinator: no

1. `git diff --stat` is empty: no tracked file changed. My only addition is the untracked `crates/loom-resource/tests/adversary_w20261121w4_u2.rs`.

2. Cases added, run alone first: `test result: ok. 3 passed` (exit 0, `adv1/case-alone.log`). All three are green on head, so none is a finding:
- `adversary_an_absent_row_is_unknown_beside_every_other_guard_and_writes_nothing`: the five commands on an absent row, crossed with every other input class. That covers each generation from `i64::MIN` to `MAX`, each exit code, both variants, both availabilities, each phase with run ends present and absent, and neighbour names (a prefix, a suffix, different case). Every answer is `Unknown { LoomAgentNotFound }` and the record stays byte-equal.
- `adversary_a_deleted_row_is_unknown_to_every_command`: after a row is deleted, every command answers `Unknown`, a second delete too, and the events stay `[Applied, Running, Deleted]`.
- `adversary_a_stale_generation_answers_before_the_state_conflict_and_writes_nothing`: on a Refused row, a stale generation answers `Stale` before `OutOfOrder`, whatever the exit code.

3. Package suite after the cases existed: `cargo test -p b10x-loom-resource --locked`, exit 0. Result lines 12 + 1 + 2 + 3 (mine) + 20 + 8 + 0 doctests = 46 passed (`adv1/suite.log`).

4. No findings.

5. What I attacked and could not break:
- The claim, by reading: in each of the five commands the new check returns exactly what the old first statement (`let Some(held) … else Unknown`) returns. Nothing runs between the two, and `LoomAgentRecord::get` (`crates/loom-resource/src/lib.rs:88`) has no side effect. It is the only storage implementor.
- In `RecordSelection` and `RequestArguments`, moving `let _ = &related;` and adding the empty `{}` block changes no branch. The declared order (`ess/domains/run.yaml:338-509`) puts existence, then `exists: false`, then predicates, then the accepting branch, which is the same under both precedence rules.
- Synthesized suites at base and head, using ess 0.86.0 and the conform test's own flags: 278 scenarios each, 97 authored each, 0 refusals each. Two scenarios changed, both stronger, and the unchanged hand-written `SelectAction` passes them (`task conform` exits 0 per `g09`):
  - `SelectAction/outcome/not-in-catalogue` now also sends a mismatched revision, so it tests that order.
  - `SelectAction/outcome/revision-mismatch` now also asserts `catalogue_revision`.
- Format raises 26 to 31 against Loom's main system: `ess specify validate` at head gives 97 scenarios, valid, the same as base. The ess/26 change (external refusal beside a row branch) and the ess/29 ordering rule show no difference in validation or in the suite.
- The two corrected reasons, probed on copies:
  - `files: []` in `RecordFailedSession`: refused at ess/25 (`ESS-COMMAND-009 … requires … ess/31`), valid at ess/31.
  - A `when_subject` generation guard beside the state guards in `ObserveRun`: refused at ess/25 (`ESS-COMMAND-004`, hint `ess/27`), valid at ess/31. The four corrected comments are true.
- The changelog bullet is right as it stands: no answer changed, so no `**Breaking:**` line is owed. It is the last bullet under `### Changed`.
- The regression test, by reading (`record.rs:418-470`), catches:
  - a wrong outcome variant;
  - the wrong name in `LoomAgentNotFound`;
  - a row written for an absent agent;
  - an event pushed on `Unknown`;
  - another resource's row being changed.
  
  It passes if the new check is removed, which is correct, because the old branch answers the same. It also passes if the check is inverted to `is_some()`. That mutant breaks present rows, and no `loom-resource` test drives `ObserveScheduledRun` on a present row; only conformance (`resource-schedule/*.yaml`) and the operator tests do. That gap predates this unit, so I am not raising it.
- Consumers (`reconcile.rs:281,372,1297,1429,1460,1540-1600`, `loom-conformance/src/resource.rs:134-426`) all go through `LoomAgentRecord`, whose answers are unchanged.
- Boundaries:
  - `4ab05d5b` touches only `ess/system.yaml`.
  - Nothing under `.engineering/`, no manifest, no lock, no `generated/rust/intake` or `generated/rust/commission`.
  - No `/home/` literal and no `UNMAPPED:` in the diff.
  - Version is still 0.57.2, and no existing assertion changed.
  - intake is still `ess/26`, commission `ess/22`, and all three `requires:` lines say 0.86.0.
- Rerun on head: `slot task drift`, `crd-drift` and `docs-check` all exit 0.

What I could not establish: I did not run my three cases against base, because the brief rules out checking it out. Their expected values come from `resource.yaml`, and the generated paths they reach differ only by the no-op check.

6. Everything outside the worktree is under `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w4/u2/adv1/`:
- scripts: `suites.sh`, `scen.py`, `insert.py`, `insert27.py`, `probe-empty-list.sh`, `probe27.sh`, `tasks.sh`
- ess copies: `tree-63668a97/`, `tree-8b69b0b6/`, `probe-empty-*/`, `probe27-*/`
- suites and their logs: `suite-*.yaml`, `suite-diff.*`, `synth-*`, `validate-*`, `compile-*`, `scen-diff.txt`, `ess-changelog-0.86.0.md`
- runs: `case-alone.*`, `suite.*`, `task-*`, `tasks.*`, `probe-empty*.log`, `probe27-*.log`

`target/` is 13G and 146G is free.

`git status --short`:
```
?? crates/loom-resource/tests/adversary_w20261121w4_u2.rs
```

```findings
[]
```
