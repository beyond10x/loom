---
format: aep.planning-md/3
id: review-result:adversary-w11-loom-action-selector-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w11 adversary, loom story:action-selector, pass 1
relations:
- reviews: story:action-selector
revision: 1
---
unit: loom/action-selector — worktree loom-w11-action-selector, ecf204c plus phase 2 uncommitted, plus my one new test file
verdict: CONFIRMED (suite gaps: 6 mutants survive the unit's own suite; no defect found in the code)
cases: executed 102→105, red 0 on the real tree (6 red on mutated copies)
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 10 logs in `~/.cache/ga-wave-2026-10-04-w11/loom-action-selector/scratch/adversary-p1/`
needs-coordinator: yes — 5 existing test files were edited outside the story's typed scope (finding 2)

**1. `git --no-pager diff --stat`**
The tracked diff is the implementor's, unchanged: 7 files, 274 insertions, 90 deletions. My only addition is the untracked file `crates/loom/tests/adversary_action_selector.rs`. I changed no non-test path.

**2. Cases added** in `~/.local/state/worktree/trees/b10x/loom/loom-w11-action-selector/crates/loom/tests/adversary_action_selector.rs`. All 3 are green on the real tree. Run alone: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`

| case | asserts | killed (red on a scratch copy) |
|---|---|---|
| `every_id_not_exactly_a_catalogue_entry_is_refused_and_named_verbatim` | 16 ids are refused by `Loom::select` and named byte-for-byte: empty, a space, padded, case variants (one listed Blocked), prefix, suffix, substring, superstring, a Cyrillic look-alike letter. `Loom::run` returns `NoUsefulAction` with 0 generator calls. The listed ids are accepted and generated once. | m1 `contains`, m2 `eq_ignore_ascii_case`, m4 payload lowercased, m6 `trim`, m7 `starts_with` |
| `the_selector_is_told_the_prompt` | the `SelectionContext.prompt` is the run's prompt, through both `select` and `run` | m3 `prompt: String::new()` |
| `a_selector_error_propagates_unchanged_and_generates_nothing` | `Unavailable(msg)`, `Unavailable("")` and `NothingAdmissible` come back as `SelectionRefusal::Selector(e)` after 1 selector call; `run` maps them to `Suspended` / `NoUsefulAction`; 0 generator calls | m5 (existing seams and lib tests also kill it) |

Red output, verbatim (the full text is in `m*.log`):
```
m1: assertion `left == right` failed: "" is not a catalogue entry
  left: Ok(SelectionData { … action: "", … case_revision: 5 })
 right: Err(NotInCatalogue(ActionNotInCatalogue { action: "" }))
m2: "Repository.Inspect" is not a catalogue entry — left: Ok(SelectionData { … action: "Repository.Inspect" … })
m3: Loom::select, then Loom::run — left: [SelectionContext { prompt: "" }, SelectionContext { prompt: "" }]
m4: left: Err(NotInCatalogue(ActionNotInCatalogue { action: "repository.inspect" })) right: …"Repository.Inspect"
m6: " repository.inspect" is not a catalogue entry — left: Ok(SelectionData { … })
m7: "" is not a catalogue entry — left: Ok(SelectionData { … action: "" … })
```
Under m1–m4, m6 and m7, the only failing test was mine. The unit's suite stayed green.

**3. Gate**, run in the unit's build dir after the cases existed:

| step | exit | summary |
|---|---|---|
| `cargo fmt --check` | 0 | — |
| `cargo clippy … -D warnings` | 0 | `Finished \`dev\` profile [unoptimized] target(s) in 0.10s` |
| `cargo test --workspace --locked` | 0 | 26 `test result:` lines, 105 passed, 0 failed, 2 ignored. New file: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `ess specify validate` + `conform synthesize` | 0 | `loom v1 — 2 file(s), valid` / `18 scenario(s) (0 authored), 0 refusal(s), written to …/suite.json` |

`--list` in this tree shows all 3 new test names.

**4. Findings** (cover the worktree as it stands)

| # | file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|---|
| 1 | `crates/loom/src/selection.rs:103` | The membership check and its refusal payload are pinned only by 2 ids that are not prefixes, substrings or case variants of any entry. Mutants m1, m2, m4, m6 and m7 survive the unit's suite. | CONFIRMED / introduced | a model selector returning a near-miss id |
| 2 | `crates/loom/tests/agent_executor.rs` and 4 more | 5 existing test files were edited outside the typed scope (`lib.rs`, `selection.rs`, `action_selector.rs`). The trait change forced the edits; only selector impls changed and no assertion did. | CONFIRMED / introduced | the scope check at merge |
| 3 | `crates/loom/src/lib.rs:94` | Passing the prompt into `SelectionContext` is unasserted; m3 survives. | CONFIRMED / introduced | every real selector |
| 4 | `crates/loom/src/selection.rs:115` | `confidence: Option<Decimal>` is copied from the selector unvalidated, so `Decimal("high")` would be accepted. | INFEASIBLE / introduced | no selector in the tree returns a confidence other than none, or a test literal |

Also noted, not raised as findings: `SelectActionBehavior` is still `Unimplemented`, so the conformance suite never drives `Loom::select`, and `ActionSelected` is never emitted. The implementor declared this in its report.

**5. Attacked, not broken**
- `run` returns before `admit` or `generate` on every refusal. Whether the catalogue refusal comes first can't be observed: catalogue membership is the same as "`admit` does not refuse".
- `deciding_entry` filters on the selected action and its admission, so it cannot pick another action's entry.
- The selection carries the catalogue's id and case revision; the acceptance test asserts both.
- Id reuse: catalogue, turn and selection all take the frontier id. A run makes exactly 1 selection and nothing stores it, so two selections in one run cannot collide. If a later story stores catalogues, a retry on the same frontier would hit `catalogue-exists`.
- The 5 adapted files kept every assertion.
- `ActionNotInCatalogue` is built with the id exactly as the selector returned it, as the spec's payload says (pinned now by m4).

**6. Written outside the worktree:** `~/.cache/ga-wave-2026-10-04-w11/loom-action-selector/scratch/adversary-p1/{m1-contains,m2-ignore-case,m3-no-prompt,m4-payload-lowercase,m5-error-collapsed,m6-trim,m7-starts-with,gate-fmt,gate-clippy,gate-test}.log`. The scratch copy (`copy/`), its build dir (`target/`) and the two backup files are deleted. The gate's `suite.json` went to the unit's build dir, at the path the gate names.

```findings
- file: crates/loom/src/selection.rs
  line: 103
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the unit's suite stays green when membership becomes contains, eq_ignore_ascii_case, trim or starts_with, or when the refusal payload is lowercased; adversary_action_selector.rs now kills all five"
- file: crates/loom/src/lib.rs
  line: 94
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "no unit test asserts the selector is handed the run's prompt; an empty SelectionContext.prompt passed the suite until the_selector_is_told_the_prompt"
- file: crates/loom/tests/agent_executor.rs
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "five existing test files outside the story's typed scope were edited to the new selector signature; assertions unchanged, the coordinator decides whether the scope admits them"
- file: crates/loom/src/selection.rs
  line: 115
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "a selector's confidence is copied into Selection unvalidated, so a non-decimal rendering is accepted; no selector in the tree returns one"
```
