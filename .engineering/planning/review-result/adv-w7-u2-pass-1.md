---
format: aep.planning-md/3
id: review-result:adv-w7-u2-pass-1
kind: review-result
status: active
title: Adversary, loom w7 U2 run --state holds a Suspension, pass 1
relations:
- reviews: story:cli-run-holds-suspension
revision: 1
---
unit: U2 `story:cli-run-holds-suspension`, tree `lw7-u2` on `impl/cli-run-holds-suspension` (findings on `eeada50`, cases committed as `4242bc8`)
verdict: NEEDS-CHANGE; 2 red cases
cases: executed 199→207, red 2
origin: introduced 4 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-cli/tests/adversary_w7_u2_state.rs` (5),
`crates/loom-intake-slice/tests/adversary_w7_u2_store.rs` (3).

Red:

| case | failed at |
|---|---|
| `a_read_only_state_directory_a_run_already_used_stops_before_any_model_call` | the classifier model was called before the run failed writing the case (`Permission denied`) |
| `a_run_ending_without_a_suspension_leaves_no_resume_context` | a run ending `StepBudget` with no Suspension left 1 resume context |

Green: a state path that is a file stops before any model call; a second run over a held directory
overwrites nothing; `run` without `--state` writes no store file; contexts kept while the governor's
stores reopen lose nothing; a dead write's temporary is swept; awkward text reads back exactly.

Suite after the cases: `cargo test -p b10x-loom-cli -p b10x-loom-intake-slice --locked --no-fail-fast`
205 passed, 2 failed; `task intake-drift` and `task no-hand-model` exit 0.

**Findings**

| # | file:line | finding | verdict | origin |
|---|---|---|---|---|
| F1 | `crates/loom-cli/src/main.rs:357` | the store opens do not prove the state directory writable, so a read-only directory holding `.lock` passes them and the classifier is called before the run fails | CONFIRMED, NEEDS-CHANGE | introduced |
| F2 | `crates/loom-intake-slice/src/run.rs:434` | the resume context is kept before the commission runs and never removed, so a run ending without a Suspension leaves one a listing cannot tell from a resumable one | CONFIRMED | introduced |
| F3 | `crates/loom-intake-slice/src/resume.rs:167` | `list` refuses the whole directory when one resume file is corrupt | judgement | introduced |
| F4 | `crates/loom-intake-slice/src/intent.rs:353` | with `--state`, a non-UTF-8 path fails the run after the classifier call and the case open | judgement | introduced |

Decided for the unit (2026-10-13, at the coordinator): F1: opening a state directory writes, syncs
and removes a probe under the lock before any model call. F2: the context stays written before the
commission runs; a run that ends without a held Suspension removes it, declared in `ess/intake`
first; a reader pairs each context with a Suspension. F3: `list` returns the readable contexts and
names the unreadable files. F4: a non-UTF-8 path is refused before the classifier call.
