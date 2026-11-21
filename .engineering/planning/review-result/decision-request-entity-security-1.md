---
format: aep.planning-md/3
id: review-result:decision-request-entity-security-1
kind: review-result
status: active
title: 'Security review, pass 1: decision-request-entity (wave 2026-11-21-w7, U1, head 2a008cf2)'
relations:
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
revision: 1
---
unit: story:decision-request-entity U1, pass 1, head 2a008cf2 plus one untracked test file (worktree loom-w1121g-u1)
verdict: red
cases: executed 25→31, red 2
origin: introduced 6, pre-existing 0, undecided 0
wrote-outside-worktree: 2 (`~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w7/u1/sec1/`, `…/u1/sec1-report.md`)
needs-coordinator: whether F1 and F4 are fixed here or routed (F1 needs a spec answer from session `sdc`)

1. Diff: `git --no-pager diff --stat` is empty. Only untracked file: `crates/loom-decision/tests/security_w20261121w7_u1.rs`, a test file. No non-test path changed. apply.rs in the worktree is byte-identical (cmp); mutants ran only on a scratch copy.
2. Cases, each run alone before the suite (`cargo test -p b10x-loom-decision --locked --test security_w20261121w7_u1 -- --exact <name>`):
   - `security_every_guard_refusal_of_every_command_attempts_no_write`: ok. Every guard of the 8 commands, over a write-counting store.
   - `security_no_command_changes_a_decided_expired_or_superseded_request`: ok.
   - `security_a_principal_named_by_a_command_is_held_to_the_stored_list`: ok.
   - `security_a_listed_decider_naming_itself_a_person_cannot_decide_under_a_rule`: FAILED (exit 101): "decider:haiku … decided a rule_and_decider request by naming its kind person: Decided {…}; stored Some((Decided, Some(Principal("decider:haiku")), Some(Person)))".
   - `security_one_open_request_per_record_key_holds_across_the_check_and_the_write`: FAILED (exit 101): "two open requests for record key change.merge@r9: ["DR-sec-2", "DR-sec-1"]".
   - `security_the_case_revision_taken_with_the_recommendation_is_kept`: ok.
3. Suite: `cargo test -p b10x-loom-decision --locked --no-fail-fast`, exit 101: unit tests 3 passed; decision_request 22 passed; security 4 passed, 2 failed. `before` = 25, the implementor's c1 count. clippy `-D warnings` and fmt `--check` on the package: exit 0.

Invariants:
1. Holds. Each refusal is produced by `apply.rs`: Request :192, :197, :202-212; Recommend :283, :288, :293, :298, :303, :311; Decide :378-442; TakeRecommendation :489-534; RecordProduced :573-601; Escalate :634-647; Supersede :683-693; Expire :729-748. Every guard of decision.yaml has code (case 1). RecordKeyOpen is stricter than the spec: the store's answer comes first (this is documented).
2. Holds. On every refusal and every `Undecided`: 0 write attempts and the store is unchanged (cases 1, 2). The one exception is a Request on an id already held: it attempts one write, gets `Err(Conflict)`, and the store is unchanged (case 2). Callers can tell this apart from an `Ok(outcome)` refusal.
3. Holds for every principal-naming field checked against the stored `eligible`: Escalate's `escalate_to` does not widen the list; near-miss names are refused (case 3). Does not hold for `principal_kind`, which is the command's own claim (case 4, F1).
4. Holds for two calls in a row and for a store rebuilt with `with_requests`. Mutant M6 (store lookup dropped) turns the implementor's test red. The store is memory only, which is what Acceptance asks. There is a window between the check and the write (case 5, F3).
5. Holds. All 8 commands were tried against Decided, Expired, Superseded, and Superseded-from-Open; none reopens a request (case 2).
6. Holds. The value is stored once, as given, from Decide or TakeRecommendation (`apply.rs:453`, `:545`). An empty value is kept, since the spec has no guard on it. A second Decide, RecordProduced and Supersede leave it unchanged (case 6).
7. Holds. `crates/loom-decision/src` has no `std::fs`, `std::process`, `Path` or `Command` (grep finds none). No id is used as a file name.
8. Holds, apart from F6. The SDC 00e3557 file with names rewritten differs from the copy only as the story lists. Both `ordered: true` are kept. 3be62683 touches only `ess/`. `slot task drift`: exit 0, "no drift from ess/". No `UNMAPPED:`. The merge-a5-veto copy differs only as Acceptance lists.
9. Holds. Scratch copies of `ess/`, with format-5 synthesis:
   - e0 (as committed): "10 refusal occurrence(s), 10 covered by authored", exit 0;
   - e1 (one cover dropped): "…, 8 covered", exit 1;
   - e2 (probe outcome nothing reaches): "11 …, 10 covered", exit 1.
   The line for a tree with no refusal is pinned by `synthesize_exiting_0_with_an_uncovered_occurrence_fails_step_3`.
10. A skipped, unsupported or unreported cover fails `task conform`: `conform.rs:279`, applied at `:594`, with a self-test on moved copies on every run. The three covers assert each step's outcome and events plus a view row, so they go red if the crate is wrong. However, a cover that tests nothing passes (e3, F4).
11. Holds. The probe needle `1 refusal(s)` became the refusal id. The exit-0 count arm is now tested directly. The parser cases moved to format 5. Nothing else in `ess_gate.rs` changed. `loom-commission/tests/ess_gate.rs` is unchanged.
12. Holds. `AGENTS.md:206-218` states the rule and the order of preference.

The implementor's statements:
- `derived_statuses` is never more lenient than ESS. ESS's `Inventory::covers`/`is_complete` (coverage.rs:424-441) count a cover only if it is a selected authored scenario, and `covered_by` is validated as one (:653-660). In Loom, a cover that is skipped makes execution inconclusive, and one that is not reported counts as uncovered.
- adversary2: +2 lines (`--scenarios ess`), no assertion changed.
- Mutants on the scratch copy: M1 rule-not-holder, M2 R∧D* escalate, M3/M4 Advised person-only (Decide, Take), M5 Take not-listed and M7 Recommend wrong-state all leave decision_request.rs green and turn case 1 red. M8 (expire due at equality) turns `expire_answers_only_at_or_after_expires_at` red. M9 survives everything (F5).
- Adapter `loom-conformance/src/decision.rs`: maps outcomes only and has no guard logic of its own.

```findings
- file: crates/loom-decision/src/apply.rs
  line: 413
  category: integrity
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "decider-under-rule fires only on the claimed principal_kind (decision.yaml:632), so a listed decider:x naming itself a person decides under rule or rule_and_decider; no caller builds Decide yet, so the inbox story must bind the kind to identity or session sdc must change the guard"
- file: crates/loom-decision/src/lib.rs
  line: 12
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "lib.rs and AGENTS.md:1278 say who may decide is never read from the command, but principal_kind is, and no guard reads addressed_to"
- file: crates/loom-decision/src/apply.rs
  line: 202
  category: concurrency
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "RecordKeyOpen is a lookup followed by a write conditioned on the request id only, so a Request landing between them opens a second request for the key; unreachable over the &mut in-memory store, it binds story:decision-request-file-store"
- file: crates/loom-executor/tests/ess_gate.rs
  line: 219
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "a loom.decision scenario running one unrelated Expire covers loom.run.Probe/outcome/never and synthesis exits 0 with 11 of 11 covered; pin covers == COVERS in conform.rs or require a cover to invoke the command it covers"
- file: crates/loom-decision/src/apply.rs
  line: 149
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "removing the broken_invariant check in write leaves every test green, because the guards upstream already keep it from firing"
- file: ess/domains/decision.yaml
  line: 1000
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "view DecisionRequests gains record_key and decided_case_revision (:1000, :1030), which the story's list of changes does not name"
```

Not established: which conformance scenario goes red under M1-M9 (the charter forbids mutating the worktree); the `ess_gate` and `conform` targets were not re-run by me; e3 was not run through `task conform`.
Scratch: `sec1/{case1-6,suite,clippy,fmt,drift}.log`, `gate-exp.{sh,out}`, `e0-e3/`, `fixtures/`, `cases.sh`, `mutants.{sh,out}`, `mut-*.log`, `mut/` (crate copy and its target), `story.md`, `spec.diff`, `sdc-*.yaml`, `apply.rs.pristine`.
git status --short: `?? crates/loom-decision/tests/security_w20261121w7_u1.rs`
