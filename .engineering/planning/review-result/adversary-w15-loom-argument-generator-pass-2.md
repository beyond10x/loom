---
format: aep.planning-md/3
id: review-result:adversary-w15-loom-argument-generator-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w15 adversary, loom story:argument-generator, pass 2
relations:
- reviews: story:argument-generator
revision: 1
---
unit: loom/argument-generator, working tree on b1f916f + uncommitted phase 2 + pass-1 fix + `crates/loom/tests/adversary2_argument_generator.rs`
verdict: INFEASIBLE
cases: executed 551→555, red 1
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 (scratch/adversary-p2/ dir, scratch/adversary-p2/gate.log)
needs-coordinator: yes. Two Looms on one frontier give different selections the same id. Decide whether that is intended (narrow the docs) or not (add a per-Loom namespace). Until then my red case keeps the gate red.

**1. `git --no-pager diff --stat`:** the same 9 files as the implementor. My only addition is the untracked `crates/loom/tests/adversary2_argument_generator.rs`. I changed no non-test path.

**2. New cases.** I ran the file alone in the unit's build dir before anything else. Result: `test result: FAILED. 3 passed; 1 failed`.

| case | asserts | now |
|---|---|---|
| `every_recorded_id_is_a_canonical_version_8_uuid` | 256 ids from `Loom::run` are canonical, version 8, variant `10` | green |
| `frontier_ids_built_from_the_separator_give_no_two_runs_one_id` | frontier ids `f1` (run 0) and `f` (run 10), plus `\n`, `""`, `selection\nf\n1` and similar: all ids distinct, none is a frontier id | green |
| `two_looms_give_their_different_selections_on_one_frontier_different_ids` | two Looms make different selections on one frontier, so the ids should differ | **red** |
| `concurrent_runs_on_one_loom_record_every_selection_and_request_once` | 8 threads × 256 runs: 2048 distinct selections and 2048 distinct requests, matched one to one | green |

Red output, verbatim (`:241` before rustfmt, `:252` after):
```
assertion `left != right` failed: a selection of repository.inspect and a selection of tests.run carry one selection id
  left: SelectionId(Uuid("61d2a913-7c4f-85fe-a6bd-78c8243a2114"))
 right: SelectionId(Uuid("61d2a913-7c4f-85fe-a6bd-78c8243a2114"))
```

**Mutants were not built.** Free space was 10.1 GiB, and a scratch copy with its own target dir would have gone under the 10G floor. I worked them out from SHA-256 output instead. I first checked that my copy of the derivation reproduces the recorded id `61d2a913-7c4f-85fe-…` exactly.
- **The unit's own id test cannot fail if the version and variant lines are deleted.** That test is `run_ids_are_uuids_of_their_kind_frontier_and_run` (lib.rs:418), and the lines are lib.rs:155-156.
  - Its only sample's digest, `6eae20108a278096a549…`, already has `0x80` at byte 6 and `0xa5` at byte 8, so it passes without those lines.
  - My first input already has `0x98` at byte 6, so that mutant would show version `9` and fail my case.
- **Removing the separators would collide `f1`/run 0 with `f`/run 10.** Both hash `selectionf10`, so my separator case would go red.

**3. Gate.** Workspace, unit build dir. I added `--no-fail-fast` to step 3 so every target runs; log is `scratch/adversary-p2/gate.log`.

| step | exit |
|---|---|
| fmt / clippy | 0 / 0 |
| `cargo test --workspace` | **101**: 34 summary lines, 554 passed, 1 failed, 2 ignored |
| ess_gate | 0 (`10 passed; 0 failed`) |
| ess | 0: `loom v1 — 2 file(s), valid`, `18 scenario(s) (0 authored), 0 refusal(s)` |
| drift / no-hand-model / docs check / cargo doc | 0 / 0 / 0 / 0 |

- Only failing line: `test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s`.
- `--list` shows all 4 new cases in this tree.

**4. Findings** (on the working tree above):
- **lib.rs:150, INFEASIBLE, introduced, warning.** The doc says "two runs … get different ones", but run 0 on Loom A and run 0 on Loom B get the same id.
  - *Measured:* the red case at `adversary2_argument_generator.rs:252`.
  - *What reaches it:* nothing today. Records are only copied out through `selections()` and `argument_requests()`, and session.rs and recovery.rs are "Not built yet".
  - *Harm when records are shared:* `put` replaces by id, so a merged record silently loses one selection. A request would then name a selection of a different action. Later reach: a recovered Loom (story:interruption-recovery) restarts its counter at 0, and O3 compares Loom configurations on one case.
  - *Fix options:* a per-Loom namespace in the hash, or narrow both docs to "unique within one Loom".
- **lib.rs:418, CONFIRMED, introduced, note.** The unit's id test cannot catch deleting lines 155-156 (digest above). My property case closes the gap.
- **arguments.rs:179, CONFIRMED, introduced, note.** The comment says "`Loom::run` gives a run's selection and argument request the same id". Since the pass-1 fix they differ; my separator case checks that.

**5. Attacked and not broken:**
- The UUID layout matches RFC 9562 §5.8 (version nibble at byte 6, variant `10` at byte 8, lowercase 8-4-4-4-12).
- The hash input `kind\nfrontier\nrun` is injective because `kind` and `run` contain no `\n`.
- Within one Loom, every run number is unique, so ids cannot collide.
- `Relaxed` `fetch_add` gives unique numbers, since read-modify-write operations on one atomic are totally ordered. The mutex protects the record.
- Concurrent runs lose nothing.
- Counter wrap needs 2^64 runs.
- The ids satisfy the generated `Uuid` ("canonical textual rendering"). ess/ declares no other constraint.
- The `Loom` doc, the contract doc and the status page make no wrong claim about ids.

**6. Paths written outside the worktree:**
- `~/.cache/ga-wave-2026-10-04-w15/loom-argument-generator/scratch/adversary-p2/` (dir)
- `~/.cache/ga-wave-2026-10-04-w15/loom-argument-generator/scratch/adversary-p2/gate.log`
- In the unit build dir, the gate rewrote `suite.json` and built my test binary.
- No scratch copy or extra target dir was made. I took no worktree lease, because the brief forbids worktree commands.

```findings
- file: crates/loom/src/lib.rs
  line: 150
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "run ids are deterministic per Loom, so two Looms (or a restarted Loom) running one frontier give different selections and requests the same id, contrary to the doc's 'two runs get different ones'; no current caller shares records (red case adversary2_argument_generator.rs:252)"
- file: crates/loom/src/lib.rs
  line: 418
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "run_ids_are_uuids_of_their_kind_frontier_and_run checks one id whose SHA-256 already has 0x80 at octet 6 and 0xa5 at octet 8, so it stays green with the version and variant writes at lines 155-156 removed"
- file: crates/loom/src/arguments.rs
  line: 179
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the test comment says Loom::run gives a run's selection and argument request the same id, which the pass-1 fix made false"
```
