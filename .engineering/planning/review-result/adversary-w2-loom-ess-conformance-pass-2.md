---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-ess-conformance-pass-2
kind: review-result
status: active
title: Wave 2026-10-07-w2 adversary, loom story:loom-ess-conformance, pass 2
relations:
- reviews: story:loom-ess-conformance
revision: 1
---
unit: story:loom-ess-conformance, branch impl/loom-ess-conformance at a650b1e (worktree loom-20261007-w2-conformance) plus 2 untracked test files of mine
verdict: CONFIRMED. One red case: `task conform` passes a report that ESS itself rates `execution_status: failed` and `conformance_status: failed`.
cases: executed 2→6, red 1
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (my worktree lease record, acquired and released)
needs-coordinator: (a) should `task conform` be allowed to stay green over a report ESS rates `failed`? Finding 1 is a different question from the "statuses agree with counts" decision. (b) keep my 2 test files or not

**1. Diff**
`git --no-pager diff --stat` prints nothing, because my changes are only new untracked files. `git status --short` shows:
```
?? crates/loom-conformance/tests/adversary2_w2_conformance_status.rs
?? crates/loom-conformance/tests/adversary2_w2_conformance_wrong_state.rs
```
Both are test paths, and I changed no implementation file. I created a probe file `tests/adversary2_w2_conformance_probe.rs` twice and deleted it both times; it is not in the tree.

**2. Cases added**

| file | asserts | now |
|---|---|---|
| `adversary2_w2_conformance_status.rs:75` | A copy of `ess/` gives `ReleaseSession` an actor with an attribute, so `LoomTarget` answers its 5 scenarios `unsupported` (`lib.rs:180`). `SKIPPED.md` names all 5 with a reason, as its header says to do. The case checks first that ESS's report says `failed`/`failed`, then runs the real `conform` binary with `CARGO_MANIFEST_DIR` pointed at the copy, and requires it to fail. | **red** |
| `adversary2_w2_conformance_wrong_state.rs:85, :118, :127` | A wrong-state refusal carries `state: Filed`, `state: Interrupted` and `state: Admitted`; an unknown session gets no `state` field | green; kills mutants L11 and L12 |

Red output, from running that case alone before any suite run (`logs/case-a-alone.log`), EXIT=101:
```
panicked at crates/loom-conformance/tests/adversary2_w2_conformance_status.rs:147:5:
task conform's check passed a report whose execution_status and conformance_status are both `failed` (ESS: any unsupported scenario fails execution), because each of its 5 unsupported scenario(s) is named in ess/SKIPPED.md: ["loom.run.ReleaseSession/outcome/released", "loom.run.ReleaseSession/outcome/wrong-state", "loom.run.Session/state/Filed/refuses/loom.run.ReleaseSession", "loom.run.Session/state/Interrupted/refuses/loom.run.ReleaseSession", "loom.run.Session/transition/release/by/loom.run.ReleaseSession/released"]
running 1 test
test ess_conformance_report ... ok
```
The run was at line 147. Line 154 is the same assert after I ran rustfmt on my two files.

Against the mutants, the wrong-state cases fail with `left: {} right: {"state": Text("Filed")}` (L11) and `left: {} right: {"state": Text("Admitted")}` (L12).

**3. Suite runs, after the cases existed**
- `cargo test --locked -p b10x-loom-conformance --test conform` (my files left out, which is also exactly what `task conform` runs): `2 passed; 0 failed`, EXIT=0. This is the before count.
- `cargo test --locked --no-fail-fast -p b10x-loom-conformance`: 6 cases ran in 3 targets (unit tests and doc-tests ran 0). conform 2 ok, wrong_state 3 ok, status 1 FAILED. EXIT=101.
- Pass 1's file `adversary_w2_conformance_select_action` 3 ok, and `adversary_run_revalidation` 2 ok. EXIT=0.
- My two files pass `cargo clippy --all-targets -p b10x-loom-conformance -- -D warnings` (EXIT=0) and `cargo fmt -p b10x-loom-conformance --check` (EXIT=0).

**4. Findings (they cover a650b1e)**

| # | file:line | verdict / origin | what was measured | what reaches it |
|---|---|---|---|---|
| 1 | `crates/loom-conformance/tests/conform.rs:217` (with :198) | CONFIRMED / introduced | The status check only requires the statuses to agree with the counts. ESS's rule (`counts.rs:463`) makes any `unsupported` scenario `failed`. So once every unsupported scenario is named in `SKIPPED.md`, the check passes a report rated `failed`/`failed`; the red case shows this. The `conform` task description says "fail unless its report passes" (`Taskfile.yml:82`). | Nothing reaches it today: 0 unsupported scenarios, and `SKIPPED.md` is empty. It is reached the first time a spec change gives a command a caller attribute (or anything else `LoomTarget` answers `unsupported`) and someone follows the `SKIPPED.md` header. |
| 2 | `crates/loom-conformance/tests/conform.rs:237` | CONFIRMED / introduced | `run_suite` builds the report with `CountReport::from_run` (`lib.rs:852`), which already refuses statuses that contradict the counts (`counts.rs:390-397`). So the status check can never fire on a real report; it can only fail where `derived_statuses` differs from ESS. The U and S copies get their statuses from `derived_statuses` itself (`:269`), so its own branches are never checked against ESS. Three mutants leave `--test conform` green: drop `unsupported`→`failed`, drop the in-scope refusal clause, drop `total > 0`. It also differs from ESS on `skipped` under the Rust profile: ESS refuses such a report (`counts.rs:460`), `derived_statuses` says `inconclusive`. | Every `task conform` run. The only effect is a check that can never fire, or a false red later; no defect gets through. |
| 3 | `crates/loom-conformance/src/lib.rs:359`, `:369` | CONFIRMED / introduced | The synthesized wrong-state scenarios check only the error's name (`expect_error`, with no fields). Mutants L11 and L12 drop the `state` field from `SessionStateConflict` and `SelectionStateConflict`, and the suite still passes 46/46. Nothing else tests `lib.rs`. My wrong-state cases kill both mutants. | Every wrong-state answer the target gives. |

Possible fixes, which I did not apply:
- **Finding 1:** either refuse `execution_status: failed`, which makes `SKIPPED.md` unusable for the Rust producer, or say in the Taskfile description and AGENTS.md that named scenarios can leave a green `task conform` over an ESS `failed`.
- **Finding 2:** read the statuses back through `CountReport::from_json` against the admitted suite, or give the U and S copies literal expected statuses.
- **Finding 3:** keep my wrong-state test file.

**5. Attacked and held**
- **Pass 1 finding 3 (M11):** dropping `unsupported` from the skip check is now killed by the U self-check (`added []`). Deleting the status loop is also killed.
- **Pass 1 finding 1:** the select_action file is unchanged and green.
- **Pass 1 finding 2:** the comment at `lib.rs:765-772` is accurate, and the test it names exists (`adversary_run_revalidation.rs:263`).
- **`derived_statuses` against ESS:** it matches `execution`/`qualification`/`is_complete` on every status a Rust-profile report can carry.
- **Other target commands:** 20 target mutants (event payloads, view fields, error fields, event filter); 17 are killed by `task conform`. L13 (no event-type filter) is equivalent, because the runner filters by event itself (`runner.rs:1609`). L11 and L12 are finding 3.
- **Taskfile:** `conform` is its own step in `check`, and `cargo test --workspace` runs it again.
- **Codec round trips:** every generated enum variant is decoded, the struct literals cover every field, and integers and UUIDs pass in the 46 scenarios.

**6. Paths written outside the worktree**
- The worktree lease record for session `adversary-2-w2-conformance`, acquired and released through `worktree hook`.

Inside the tree (git-ignored): `<worktree>/.engineering/drafts/scratch/adversary-2/`, 1.7M (logs, mutant sources, a spec-copy probe). I created no mutant `target/`. I removed the one file I wrote in `target/tmp` (`adversary2-mutant-suite.json`).

**7. Findings block**
```findings
- file: crates/loom-conformance/tests/conform.rs
  line: 217
  category: acceptance
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: the status check only requires agreement with the counts, so a report whose unsupported scenarios are all named in SKIPPED.md passes task conform while ESS rates it execution_status failed and conformance_status failed
- file: crates/loom-conformance/tests/conform.rs
  line: 237
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: derived_statuses re-derives what CountReport::from_run already enforces, so the status check cannot fire on a real report, and with_outcome builds the U and S copies with derived_statuses itself, leaving three mutants of its ESS rule alive
- file: crates/loom-conformance/src/lib.rs
  line: 359
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the synthesized wrong-state scenarios check only the error name, so dropping the state field from SessionStateConflict or SelectionStateConflict (lib.rs:359, :369) leaves task conform at 46 of 46, and only the added wrong-state cases catch it
```
