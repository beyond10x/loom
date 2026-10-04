---
format: aep.planning-md/3
id: review-result:adversary-w8-loom-agent-executor-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w8 adversary, loom story:agent-executor, pass 1
relations:
- reviews: story:agent-executor
revision: 1
---
unit: loom/agent-executor, worktree loom-w8-agent-executor at fad4bb1 plus the uncommitted phase 2
verdict: NEEDS-CHANGE
cases: executed 60→69, red 8
origin: introduced 4 / pre-existing 0 / undecided 3
wrote-outside-worktree: 4 scratch files, plus the assigned build dir
needs-coordinator: whether Loom should follow Commission's `admit` precedence or the story's narrower text (findings 1–3), and the approval-stop conflict in finding 6

**Verdict: hold the unit.** Loom's executor disagrees with Commission's own admission rule on 32 of 42 frontier shapes. In every one of those shapes Commission refuses the action, but Loom either proposes it or asks for authority over it. The acceptance test passes only because its selector never picks a blocked merge.

**1. Diff stat.** `git --no-pager diff --stat` lists only the implementor's phase-2 files (7 files, +326 −86); my two files are untracked, so they do not show there. I added exactly two files, both tests:
- `crates/loom/tests/adversary_executor_admission.rs`
- `crates/loom-xtask/tests/adversary_checks.rs`

I changed no implementation file.

**2. Cases, each run alone before the suite** (logs `scratch/adv1-executor.log`, `scratch/adv1-xtask.log`)

| Case | What it asserts | Now | Red output (excerpt) |
|---|---|---|---|
| `blocked_selection_is_not_proposed` | a merge listed only as Blocked is not proposed | red | `Loom proposed repository.merge, which the frontier lists as Blocked: ProposedAction(...)` |
| `merge_seeking_model_never_gets_merge_proposed` | acceptance 2 with a selector that picks merge whenever the frontier lists it | red | `all: [ProposedAction(... "repository.merge" ...), Suspended(Authority ...)]` |
| `blocked_entry_beside_approval_entry_does_not_ask_for_authority` | entries [ApprovalRequired, Blocked] for one action: no authority request (Commission refuses) | red | `Loom asked for authority over a Blocked action` |
| `conflicting_capabilities_do_not_depend_on_order` | entries [AR a, AR b] and [AR b, AR a] give the same answer | red | `left: Some(Text("repository.write")) right: Some(Text("release.publish"))` |
| `approval_without_capability_does_not_ask_for_nothing` | ApprovalRequired with capability None or `"  "` is not an authority request | red | `("capability", Null)` |
| `loom_agrees_with_commission_admission` | 42 frontier shapes with 1–2 entries: Loom's outcome matches `b10x_commission::admission::admit` | red | `32 frontier(s) where Loom disagrees`. Every Refused shape fails; every Admissible and NeedsAuthority shape agrees |
| `nothing_admissible_is_not_an_external_outage` | all actions Blocked plus an open obligation: Commission's `derive` gives the same result for Loom's outcome as for `NoUsefulAction` | red | `left: Ended(Suspended(... ExternalAvailability ... "no admissible action")) right: Ended(NeedsExternalEvidence { requirements: ["verify.tests"] })` |
| `no_hand_model_refuses_derived_model_types` | a hand-written `SessionData`, `SelectionSnapshot`, `AnySession` or `TurnData` in lib.rs is refused | red | all four `(exit Some(0))` |
| `drift_names_edited_manifest_and_plan` | an edit to generated `Cargo.toml`, `PLAN.md` or `plan.json` is drift, named by file | green | none |

**3. Suite run, after the cases existed:** `cargo test --workspace --locked --no-fail-fast` gave EXIT=101 (log `scratch/adv1-suite.log`).
- Summary lines: `adversary_executor_admission: 0 passed; 7 failed`, `adversary_checks: 1 passed; 1 failed`, every other binary ok.
- Without my 9 cases the suite runs 60, which matches the implementor's report.
- `cargo fmt --check` returned 0 and `cargo clippy --workspace --all-targets --locked -- -D warnings` returned 0, so my files do not break the gate for any other reason.

**4. Findings** (tree: fad4bb1 plus uncommitted phase 2)

| # | file:line | Verdict | Origin | What reaches it |
|---|---|---|---|---|
| 1 | `crates/loom/src/lib.rs:117` | NEEDS-CHANGE | undecided | Any `ActionSelector`, including the model selector `story:action-selector` adds. The guard checks only ApprovalRequired, so a Blocked entry is proposed. That breaks Commission's rule (`docs/contracts/frontier.md`): an executor may not invoke an action outside the "admissible set". The base lib.rs had the same check on Canon types; I did not run it. |
| 2 | `crates/loom/src/lib.rs:117` | NEEDS-CHANGE | undecided | Duplicate entries. Loom takes the first ApprovalRequired entry, so the result depends on entry order. Commission's `admit` instead lets Blocked, a missing capability or conflicting capabilities refuse the action, whatever the order. To the brief's question of which capability gets named with several approval entries: the first one listed. |
| 3 | `crates/loom/src/lib.rs:126` | NEEDS-CHANGE | undecided | A null or blank capability becomes `Suspended(Authority {capability: null})`, an authority request that names nothing to ask for. |
| 4 | `crates/loom/tests/agent_executor.rs:116` | NEEDS-CHANGE | introduced | The `MergeSeeking` selector skips merge while it is Blocked. Acceptance 2 therefore never exercises Loom's own guard on the initial frontier, so finding 1 stays green. |
| 5 | `crates/loom/src/lib.rs:60` | CONFIRMED | introduced | `FirstAdmissibleSelector` on a frontier that admits nothing. Its error is mapped to ExternalAvailability, and Commission's `derive` rule 2 then ends the run as an outage, not `NeedsExternalEvidence` or `NoAdmissibleAction`. `NoUsefulAction` is the variant for this case. The error text is kept: `{"error": msg}`. |
| 6 | `crates/loom/src/lib.rs:117` | INFEASIBLE | introduced | Judgement, no test. The story requires `Suspended` at ApprovalRequired, but Commission's `derive` rule 5 expects the executor to propose the action and then asks the AuthorityProvider. Suspended wins at rule 2, so through Loom an action whose capability the AuthorityProvider would allow can never continue. This is a story-level conflict and cannot be fixed inside this unit. |
| 7 | `crates/loom-xtask/src/main.rs:435` | CONFIRMED | introduced | Names come from `ess specify compile` only, so generated types such as `*Data`, `*Snapshot`, `Any*`, the state markers and the primitives are not reserved. Commission's `no-hand-model` reserves every type its generated crate declares. This follows the story text but is weaker than the check it was ported from. |
| 8 | `crates/loom-xtask/src/main.rs:210` | CONFIRMED | introduced | Judgement, note only. Ported from Commission's xtask, but its `drift` drops Commission's check that the crate takes its model from the generated tree (manifest path dependency and re-export). `--ess` is global here but per-subcommand in Commission. `no-hand-model --generated` is missing. |

**5. What I attacked and could not break**
- ExternalAvailability keeps the error text.
- An action outside the frontier is refused.
- The approval stop on the scripted ELS frontier holds when the selector does not pick a blocked merge.
- `drift` catches edits to `Cargo.toml`, `PLAN.md` and `plan.json`.
- Aliases (`type X = ...`), newtypes and `use ... as X` renames are caught.
- The doc comment on `run` matches its behaviour.

Not done: I did not acquire a worktree session lease.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w8/loom-agent-executor/scratch/adv1-executor.log`
- `~/.cache/ga-wave-2026-10-04-w8/loom-agent-executor/scratch/adv1-xtask.log`
- `~/.cache/ga-wave-2026-10-04-w8/loom-agent-executor/scratch/adv1-suite.log`
- `~/.cache/ga-wave-2026-10-04-w8/loom-agent-executor/scratch/adv-compiled.json`
- `~/.cache/b10x-target/loom-w8-agent-executor`, the assigned build dir. The test copies under its `CARGO_TARGET_TMPDIR` are removed by the tests.

**Suggested fix (not applied):** have `Loom::propose` call `b10x_commission::admission::admit(frontier, &selected)` and map the result:
- Admissible → ProposedAction
- NeedsAuthority → Suspended(Authority) with that capability
- Refused → not proposed, and not an authority request

The "no admissible action" case should return `NoUsefulAction`.

```findings
- file: crates/loom/src/lib.rs
  line: 117
  category: contract-drift
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: undecided
  message: "Loom proposes a selected action that the frontier lists as Blocked, which Commission's admit refuses and the frontier contract forbids an executor to invoke."
- file: crates/loom/src/lib.rs
  line: 117
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: undecided
  message: "With duplicate entries Loom names the first ApprovalRequired capability, which depends on entry order and asks for authority where Commission refuses for Blocked or conflicting capabilities; 32 of 42 frontier shapes disagree with admit."
- file: crates/loom/src/lib.rs
  line: 126
  category: boundary
  severity: warning
  verdict: NEEDS-CHANGE
  origin: undecided
  message: "An ApprovalRequired entry with no or blank capability becomes Suspended(Authority) with capability null or blank, an authority request that names nothing to ask for."
- file: crates/loom/tests/agent_executor.rs
  line: 116
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "The acceptance selector MergeSeeking skips merge while it is Blocked, so acceptance 2 never tests Loom's guard; a selector that names merge whenever listed gets repository.merge proposed on the first invocation."
- file: crates/loom/src/lib.rs
  line: 60
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "A frontier that admits nothing becomes Suspended(ExternalAvailability), so Commission's derive ends the run as an outage and hides open obligations it would report as NeedsExternalEvidence for NoUsefulAction."
- file: crates/loom/src/lib.rs
  line: 117
  category: judgement
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "The story requires Suspended at ApprovalRequired, but Commission's derive rule 2 then pre-empts rule 5, so an action whose capability the AuthorityProvider would allow can never continue through Loom; this is a story-level conflict."
- file: crates/loom-xtask/src/main.rs
  line: 435
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "no-hand-model reserves only ess compile names, so a hand-written SessionData, SelectionSnapshot, AnySession or TurnData in crates/loom/src passes, while Commission's check reserves every generated type."
- file: crates/loom-xtask/src/main.rs
  line: 210
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "Unlike Commission's xtask, drift does not check that b10x-loom takes its model from the generated tree, --ess is global rather than per-subcommand, and no-hand-model has no --generated."
```
