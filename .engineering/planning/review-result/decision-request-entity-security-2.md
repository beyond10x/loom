---
format: aep.planning-md/3
id: review-result:decision-request-entity-security-2
kind: review-result
status: active
title: 'Security review, pass 2: decision-request-entity (wave 2026-11-21-w7, U1, head 0357f12a)'
relations:
- reviews: story:decision-request-entity
revision: 1
---
unit: story:decision-request-entity U1, pass 2, head 0357f12a plus one untracked test file (diff 2a008cf2..0357f12a)
verdict: green
cases: executed 35→38, red 0
origin: introduced 1, pre-existing 0, undecided 0
wrote-outside-worktree: 2 (`~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w7/u1/sec2/`, `…/u1/sec2-report.md`)
needs-coordinator: none

Diff: no tracked file changed. Only untracked file: `crates/loom-decision/tests/security_w20261121w7_u1_pass2.rs` (3 cases, each run alone first, all ok). Suite `cargo test -p b10x-loom-decision --locked --no-fail-fast`, exit 0: lib 4, decision_request 25, security 6, security pass2 3 passed. clippy `-D warnings` on the package: exit 0.

1. F1 is closed.
   - The ESS guard (`decision.yaml:603-613`, and its copy before `no-request` in TakeRecommendation) is exactly equivalent to `kind != kind_of(principal)` (`apply.rs:174`, checked at `:411` and `:527`). ESS `starts_with` is a byte prefix, case-sensitive and untrimmed (ess 0.96.0 `ess-synth/src/rust/invariant.rs:1308`), which is what `str::starts_with` does.
   - pass-1 case `…cannot_decide_under_a_rule`: ok.
   - Edge names: `decider:`, `decider:x\n` and `decider:rule:x` are deciders; `rule:` and `rule:decider:x` are rules; `Decider:x`, ` decider:x`, `xdecider:x`, `RULE:x`, `silence `, `Silence` and the empty name are persons. Under all 8 holders, on both commands and for all 4 kinds, only the kind the ESS text gives passes, and every refusal writes nothing: `security_kind_not_principal_at_the_edges_of_the_name_form_matches_the_ess_text ... ok`.
   - A listed name outside the form decides as what its form says (e.g. ` decider:x` as a person). That is the rule as written; whether names are canonical is the caller's matter.
   - Order: the guard comes first, before `no-request`, silence and option-or-answer, and on a Decided request before wrong-state. That matches ESS precedence, since it reads the command alone and is declared first: `security_kind_not_principal_comes_before_existence_input_guards_and_state ... ok`.
   - `decided_by_kind` equals the form of `decided_by` after every Decide and TakeRecommendation, across all holders, names and kinds. Expire sets neither field: `security_the_stored_decided_by_kind_is_the_kind_of_the_decided_by_name ... ok`.
   - The adapter (+12 lines) maps `KindNotPrincipal` to `kind-not-principal` and holds no rule of its own.
2. The implementor's three input changes to my file (`:434`, `:477`, `:541-548`) still reach the guards they were written for: silence-takes-recommendation, Decide person-only under person, Take person-only under advised (`security … 6 passed`). No assertion or expected value changed. The three pairs they no longer send (operator/silence, operator/decider, `decider:x`/rule) are refused by `a_principal_kind_its_name_does_not_have_is_refused_on_both_commands` (`decision_request.rs:921`, which uses `decider:model`/Rule).
3. F3 is closed.
   - In `InMemoryDecisionStore::write` (`store.rs:229-260`) the check and the write are one step under `&mut self`, with no call out between them. pass-1 case `…across_the_check_and_the_write`: ok, unchanged.
   - The check skips the request's own id, so Escalate (Open/Escalated to Escalated) and Recommend still pass and cannot be refused against themselves. Only a store that already holds two open requests for one key (`with_requests`) refuses those moves, with `Err(Conflict)`; Supersede and Expire still close it.
   - No command writes `record_key` except Request (`apply.rs:242`).
   - The obligation is written on `DecisionStore::write`'s doc (`store.rs:133-137`). The default `open_for_record` is only a read, so a store without the rule keeps the window.
   - `StoreConflict` is not at c1a1f443 (`git show` fails: the path is not there). It is public with all fields public, so it can be matched exhaustively, but it is new in this unit: no break.
4. F4 is closed.
   - The e3 form on a scratch repository copy at 0357f12a: synthesis "362 selected …, 18 refusal occurrence(s), 18 covered by authored", exit 0. Then `cargo test -p b10x-loom-conformance --test conform`, exit 101, `conform.rs:723`, naming "the suite's refusal `loom.run.Probe/outcome/never` covered by `loom.decision/authored/vacuous-cover` is not in the pinned table".
   - The 17 rows: each scenario runs the command and asserts the outcome of every row: 7 wrong-state on Expired; supersede from Escalated; recorded, already-recorded and produced-conflict, plus a view row; decider-under-rule, rule-not-holder and silence-takes-recommendation under rule_and_decider. So each goes red if the crate answers that outcome wrongly. The two `invariant/after` rows are reached; whether the runner checks invariants after authored steps is not established.
   - The summaries give code and cause. The decide-kind-guards cover says why: synthesis has no prefixed principal.
   - 362→360, ess-only synthesis at 0357f12a: 7 synthesized scenarios left the suite: Decide decider-under-rule, rule-not-holder, silence-takes-recommendation; RecordProduced already-recorded, produced-conflict; the invariants after recorded and after already-recorded. Synthesized Decide and TakeRecommendation `kind-not-principal` joined, plus 3 authored scenarios.
   - Every outcome that left is asserted by its cover. decider-under-rule under rule, rule_final and granting R∧D* is now held only by crate tests (pass-1 case 1, `under_rule_and_decider_final_…`).
   - I did not check that no behaviour-neutral change removes the 7. `RecordProduced/option-produces-none` is still synthesized, so ESS does reach a Decided request (N1).
5. F5 is closed: `apply.rs:930` calls the module's own `write` (`:939`), the same function the commands use.
6. F2 holds. `lib.rs:11-16` is true of the code: `holder` and `eligible` come from storage; `principal` and kind come from the command; `kind_of` is checked first; no guard reads `addressed_to`. AGENTS.md § Decision requests is true. The § ESS item 3 sentence on `COVER_TABLE` is true. N1 is the exception.
7. Boundaries:
   - 54cf9f95 touches only `ess/` (6 files).
   - `slot task drift`: exit 0, "no drift from ess/".
   - No `#[ignore]` added. The only removed lines in the tests are `COVERS`, replaced by `table_covers()`.
   - Commission, bindings, governor and `.engineering/`: no change. No `/home/` literal added.

```findings
- file: AGENTS.md
  line: 221
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "AGENTS.md:221 and :1304 say synthesis arranges no Decided request for RecordProduced, but RecordProduced/outcome/option-produces-none is still synthesized; it reaches no Decided request whose produced record selects recorded, already-recorded or produced-conflict"
```

Not established: that no behaviour-neutral change removes the 7 new occurrences; whether authored steps get invariant checks.
Scratch `sec2/`: `e0/`, `e3/` (ess copies and suites), `repo/` (repository copy, target 2.8G), `case1-3.log`, `suite.log`, `clippy.log`, `drift.log`, `conform-e3.{sh,log,disk}`, `synth2.{sh,out}`, `compare.py`, `compiled.json`.
git status --short: `?? crates/loom-decision/tests/security_w20261121w7_u1_pass2.rs`
