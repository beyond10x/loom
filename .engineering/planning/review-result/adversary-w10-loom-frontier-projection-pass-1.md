---
format: aep.planning-md/3
id: review-result:adversary-w10-loom-frontier-projection-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w10 adversary, loom story:frontier-projection, pass 1
relations:
- reviews: story:frontier-projection
revision: 1
---
```
unit: loom/frontier-projection, working tree on e4194ec plus uncommitted crates/loom/src/projection.rs
verdict: NEEDS-CHANGE
cases: executed 87→92, red 5
origin: introduced 3 / pre-existing 1 / undecided 0
wrote-outside-worktree: 3 paths (see 6)
needs-coordinator: whether "frontier marks blocked" means Commission's least-authority rule (my reading) or one entry at a time (the reading of the ESS comment on ProjectCatalogue)
```

**1. What I touched**

`git --no-pager diff --stat`:
```
 crates/loom/src/projection.rs | 48 ++++++++++++++++++++++++++++++++++++++++++-
```
That line is the implementor's uncommitted phase 2; I did not edit it. My only file is untracked: `?? crates/loom/tests/adversary_projection_duplicates.rs`. No implementation file was edited.

**2. Cases added** (`~/.local/state/worktree/trees/b10x/loom/loom-w10-frontier-projection/crates/loom/tests/adversary_projection_duplicates.rs`)

Each case first checks that Commission's `admit` (commission 174bf07) gives the expected answer for the frontier it builds. All five are red now. They were run alone before the suite, with `cargo test --locked -p b10x-loom --test adversary_projection_duplicates`, which exited 101.

| Case | Frontier → expected | Projected (verbatim) |
|---|---|---|
| `admissible_beside_blocked_is_not_projected` :72 | merge Admissible plus merge Blocked, both orders; admit refuses → only inspect | `[inspect Admissible, merge Admissible]` |
| `admissible_beside_approval_is_one_approval_entry` :103 | Admissible plus ApprovalRequired("repository.write"); admit needs authority → merge once, ApprovalRequired | `[inspect Admissible, merge Admissible, merge ApprovalRequired]` |
| `approval_without_capability_is_not_projected` :132 | ApprovalRequired with capability None, "" or "  "; admit refuses → only inspect | `[inspect Admissible, merge ApprovalRequired]` |
| `conflicting_capabilities_are_not_projected` :155 | two ApprovalRequired entries, write and admin; admit refuses → only inspect | `[inspect Admissible, merge ApprovalRequired, merge ApprovalRequired]` |
| `agrees_with_commission_admission` | every frontier of 1 to 3 merge entries over 5 entry kinds, in every order | `149 of 155 frontiers project differently from Commission's admission`; even `[(Admissible, None), (Admissible, None)]` projects merge twice |

`--list` shows all five names, and `projection_follows_frontier`, in this tree's binaries. `cargo fmt --check` exits 0 and clippy `-D warnings` on the file exits 0.

**3. Suite run, after the cases existed**

`cargo test --workspace --locked --no-fail-fast` exited 101. Summed summary lines: passed 87, failed 5, ignored 2. The only failing binary is `adversary_projection_duplicates`. With that file left out, 87 passed, which matches the implementor's count. The log is at `.../scratch/adversary1/suite-nff.log`. A first run without `--no-fail-fast` stopped at my binary; its log is `suite.log`.

**4. Findings**

| # | file:line | Finding | Verdict / origin | What reaches it |
|---|---|---|---|---|
| F1 | crates/loom/src/projection.rs:29-36 | `filter_map` goes one entry at a time. An action listed Admissible and Blocked is shown to the model as Admissible, even though Commission refuses it. That breaks acceptance 3 ("an action the frontier marks blocked is never in a projected catalogue") and contradicts the module doc at :3-5. SelectAction checks only that the action id is in the catalogue (`ess/domains/run.yaml`, not-in-catalogue), so the action is selectable. | NEEDS-CHANGE / introduced | Commission's `docs/contracts/frontier.md` allows "lists one action more than once" and says the least-authority entry decides. Commission's own suites and Loom's `adversary2_executor_seams.rs` build these frontiers. The only governors that exist are fakes (`FakeGovernor` serves any list). |
| F2 | crates/loom/src/projection.rs:29-36 | Duplicate entries become duplicate catalogue entries, sometimes with two statuses for one action (Admissible plus ApprovalRequired). Entry order then depends on the frontier's order. `admit` does not. | NEEDS-CHANGE / introduced | Same as F1 |
| F3 | crates/loom/src/projection.rs:46 | An ApprovalRequired entry with no capability, or a blank one, is projected as ApprovalRequired. Commission refuses it, the same as Blocked. | NEEDS-CHANGE / introduced | The `capability: Option` field allows it; Commission's contract rule 2 covers it |
| F4 | ess/domains/run.yaml (`loom.run.ActionCatalogue`) | A catalogue carries no `case_id`, and `frontier` is an opaque string. A catalogue projected from another case's frontier cannot be told apart, and SelectAction cannot check it. | INFEASIBLE / pre-existing | `ess/` is outside this unit's scope; the spec landed at base 9b342c8. Nothing in this unit takes a case. |

The fix I would suggest, not applied: project each distinct action once, at its first position, with its status taken from `b10x_commission::admission::admit`. Admissible maps to Admissible, NeedsAuthority to ApprovalRequired, and Refused is dropped. Then update the doc at :3-5 and the ESS comment on ProjectCatalogue to say "per action, by Commission's admission".

**5. Attacked, could not break**

- Single-entry frontiers: order, `case_revision`, frontier id and turn id are carried exactly, and the acceptance test asserts literal values, so each of those mutants would be caught.
- A revision the catalogue later claims: SelectAction's revision-mismatch compares against the catalogue's own `case_revision`, which `project` copies. I read this; I did not run it.
- The ProjectCatalogue generated behaviour copies all five fields and guards only catalogue-exists. A projected catalogue satisfies it. I read this; I did not run it.
- The reverse direction (an action Commission admits but the catalogue omits) does not occur in the 155-frontier enumeration.

**6. Paths written outside the worktree**

- `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary1/suite.log`
- `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary1/suite-nff.log`
- build output in the assigned `~/.cache/b10x-target/loom-w10-frontier-projection`

**7. Findings block**

```findings
- file: crates/loom/src/projection.rs
  line: 29
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "An action listed both Admissible and Blocked is projected as Admissible, so a frontier-blocked action Commission refuses reaches the model and passes SelectAction's membership guard."
- file: crates/loom/src/projection.rs
  line: 29
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Duplicate frontier entries become duplicate catalogue entries, one action with two statuses and an order that depends on frontier order, where Commission's admission gives one order-independent answer."
- file: crates/loom/src/projection.rs
  line: 46
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "An ApprovalRequired entry with no, empty or blank capability, or two conflicting capabilities, is projected as ApprovalRequired although Commission refuses it."
- file: ess/domains/run.yaml
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: "ActionCatalogue carries no case id and an opaque frontier string, so a catalogue projected from another case's frontier cannot be detected; the spec is outside this unit's scope."
```
