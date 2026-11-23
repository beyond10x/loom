---
format: aep.planning-md/3
id: story:tree-suite-approve-result-and-reasons
kind: story
status: active
title: The suite's approve step and entry pass result, reasons and deny to tree approve
summary: approve members result, reasons (joined by line feeds) and deny become available; a denial is AuthorityRecorded Deny and the wait stays; three Runbook cycles unblocked; split from the stop story (D130), merges before it
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-suite-format
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T13:08:58Z", actor: "agent:loom", revision: 10}
- {from: "proposed", to: "active", at: "2026-11-22T13:09:49Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "runbook-batch3a-2026-11-22"}
---
## Outcome

The suite's `approve` step and its `approve` scripted entry pass `result`, `reasons` and `deny`
to `tree approve`, which already takes all three (`crates/loom-tree/src/approve.rs:53-75`). Three
of Runbook's waiting cycles need this: `decider-rework`, `decider-rework-spent` and
`rework-fresh-allowance` (Runbook `docs/cycles-suite.md:250`, `:251`, `:280`, branch
`impl/runbook-test-suite`). This was split out of `story:tree-suite-stop-resume-reopen-steps` by
decision D130. It is small and merges before that story.

**Security-relevant.** A person's reasons from a suite file enter a blob of the store, and a
denial enters the run log as authority. Both go through `tree approve`'s existing checks; the
suite adds no path around them.

### What changes

- **Today.** The step passes `result: None, reasons: None` and always `ApprovalDecision::Allow`
  (`crates/loom-tree/src/suite/run.rs:971-988`). The answering of a printed wait from an `approve`
  entry does the same (`fn answer`, `suite/run.rs:1103-1164`, the request at `:1143-1154`). The
  members are decoded (`crates/loom-tree/src/suite/decode.rs:633-635`) but wait
  (`crates/loom-tree/src/suite/availability.rs:81-83`).
- **`result`** is passed as given.
  - It needs a person action (`approve.rs:63-75`). On a wait printed as `ApproveAction`, the
    entry takes the action from the printed decision (`suite/run.rs:1139-1142`).
  - An entry with `result` that answers a capability-only wait is refused `NotAPersonAction`,
    which becomes the entry's difference `StepFailed`.
- **`reasons`**, a list of strings, is joined with line feeds and passed as `Reasons::Text`
  (`approve.rs:53-61`). The library's limits apply as they stand: 64 KiB and 60 lines. Reasons
  without a result are refused.
- **`deny: true`** passes `ApprovalDecision::Deny`. `tree approve` records it as an
  `authority recorded` line with decision `Deny`. `tree status` lists it as an approval with
  `answer: Deny` (`crates/loom-tree/src/status.rs:332-352`; `loom.tree.StatusApproval`,
  `ess/domains/tree.yaml:244-258`), and the wait on that capability stays.
- **A refusal from `approve`** ends the step or the entry as today: the step's exit 1 and the
  refusal's name, or the entry's difference `StepFailed`.
- **Availability.** `approve.result`, `approve.reasons` and `approve.deny` become `Available`
  (`availability.rs:81-83`).

### Runbook's side

The three cycles need no change. Their entries hold `capability: change.merge`, `result:
rejected` and `reasons: [..]`, which answer `merge.approve`, a person action whose binding lists
`{performed: approved, rejected: rejected}` (Runbook `bindings/sdc-story.yaml`, `merge.approve`).

## Decisions

- **D1. `deny`.** Default **a**.
  - **a.** Pass `deny` too: the library has `ApprovalDecision::Deny`, the member is decoded
    already, and it waits on the same story id as the other two.
  - **b.** `deny` keeps waiting. Runbook's three cycles do not use it.
- **D2. How a list of reasons becomes one text.** Default **a**.
  - **a.** Joined with line feeds, one entry per line; the 60-line limit then counts entries and
    the lines inside them.
  - **b.** The member takes one string only. A list is refused at decode, and Runbook's entries
    (lists of one) change.

## ESS first

No new noun. The suite step and entry types in `ess/domains/tree.yaml` already hold `result`,
`reasons` and `deny`; the members' availability is code (`availability.rs`).

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

**`crates/loom-cli/tests/tree_test/mod.rs`**

New fixture files go in `fixtures/tree-suite`:

- `review.suite.yaml` has cycles over `review@1`. Its protocols directory is a scratch copy of
  `tree-two-level/protocols` plus `tree-two-level/extra/review.yaml`; the fixture is never
  changed in place.
- `bindings/review.bindings.yaml` is a copy of `tree-two-level/review.bindings.yaml`.
- `approve.suite.yaml` has cycles over `tour@1`.

These tests run `tree test` on the fixtures alone, not on the module's shared run, and each
asserts its time under the module's 5 s (`within_bound`, `tree_test/mod.rs:183-186`).

1. `the_approve_step_records_a_named_result`.
   - The steps are `open`, a `run` until the first wait, and
     `approve action: draft.review result: rejected reasons: ["one", "two"]`.
   - Then come `run` steps until the root waits again, an `approve action: draft.review`, and a
     last `run` with `exit: 0`.
   - Checks: a `sequence` of the root's `draft_review` results is `[rejected, approved]`, and the
     `ActionFinished` line of the first `draft.review`, followed to its blob, contains
     `one\ntwo`.
   - Failing first: the members wait, so the cycle is reported pending.
2. `a_scripted_entry_records_a_named_result`.
   - The entries are `{take: approve, action: draft.review, result: rejected, reasons: [r],
     times: 1}` and then `{take: approve, action: draft.review, times: 1}`, with `run` steps
     until the root ends.
   - The same `sequence` check gives `[rejected, approved]`. This is the shape of Runbook's
     `decider-rework`.
   - Failing first: as test 1.
3. `a_result_on_a_capability_wait_is_a_difference`.
   - An `approve` entry with `result: rejected` answers a `tour@1` capability wait
     (`guide.approve`).
   - The cycle ends with the difference `StepFailed`, naming `NotAPersonAction`.
   - Failing first: as test 1.
4. `a_denial_is_recorded_and_shown`.
   - On `tour@1`, the steps are `open`, `run rounds: 1 exit: 3` (the wait on `guide.approve`),
     and `approve capability: guide.approve deny: true` with exit 0.
   - Then comes a `status` step. No `run` follows the denial.
   - Checks, on the log and the status alone:
     - a `log` count of `AuthorityRecorded` lines with `decision: Deny` is 1;
     - the `status` step's output lists an approval of `guide.approve` with `answer: Deny`.
   - Failing first: as test 1.
   - It also catches the planted defect of passing `Allow` whatever `deny` says: the log line
     would carry `decision: Allow` and the status `answer: Allow`.
   - Why no run after the denial: `TreeAuthority` then answers `Deny` (`approvals.rs:181`) and
     Commission puts the action in `denied` (`runtime.rs:2058`), so the run does not stop at
     `AwaitingAuthority`, and no existing test runs `tree run` after a denial. What that run does
     is not this story's.
5. `the_reasons_limit_counts_lines`.
   - Two cycles over `review@1`, each with `open`, a `run` until the root waits on
     `draft.review`, and one `approve action: draft.review result: rejected` step:
     - `reasons-at-the-limit` has 60 one-line `reasons` and expects exit 0;
     - `reasons-over-the-limit` has 61 one-line `reasons` and expects exit 1 with refusal
       `NotAPersonAction`.
   - The step names `action: draft.review`, so the checks before the reasons pass
     (`approve.rs:76-109`, `person_action` at `:141-143`), and the refusal comes from
     `reasons_fit` (`approve.rs:147-149`): "the reasons are 61 lines, more than 60"
     (`retry.rs:71-74`, `RETRY_LOG_LINES` at `:38`). Under D2 a, 60 entries joined with line
     feeds are 60 lines.
   - A suite step records only the refusal's reason, not its detail (`StepEnd::refused`,
     `suite/run.rs:584-590`). The pair is what ties the refusal to the line count: with the
     60-line limit deleted the over-the-limit cycle ends with exit 0 and fails; with the action
     wrong, or a refusal before the reasons, the at-the-limit cycle fails too.
   - Failing first: as test 1.

**`crates/loom-tree/tests/suite_format/mod.rs`**

6. `the_approve_members_are_available`.
   - `SUITE_AVAILABILITY` holds `approve.result`, `approve.reasons` and `approve.deny` as
     `Available`.
   - Failing first: they hold `Waits("story:tree-stop-record-and-labels")`.

**Gate.** `task check` exits 0.

## Out of scope

- The steps `stop`, `resume` and `reopen`, and the hold record
  (`story:tree-suite-stop-resume-reopen-steps`).
- Running a person's program on approval (`story:person-action-runs-a-program`).
- `--reasons-file` from a suite.

## Scope

Derived 2026-11-22 from the plan tree at `763c6ecf`. **Cited** means read at the line named.

- `crates/loom-tree/src/suite/run.rs` (`:971-988`, `:1103-1164`) — cited.
- `crates/loom-tree/src/suite/availability.rs` (`:81-83`) — cited.
- `crates/loom-tree/src/approve.rs` (`:53-75`, `:76-149`), `crates/loom-bindings/src/retry.rs`
  (`:38`, `:55-77`), `crates/loom-tree/src/status.rs` (`:332-352`) and `ess/domains/tree.yaml`
  (`:244-258`), read and not changed — cited.
- `crates/loom-cli/tests/tree_test/mod.rs` and `crates/loom-cli/tests/fixtures/tree-suite` —
  cited.
- `crates/loom-tree/tests/suite_format/mod.rs` — cited.
- `website/docs/guides/run-a-case-tree.md` (`:509`, the table of waiting kinds) and
  `CHANGELOG.md` — cited.

## Edges

- `depends_on story:tree-suite-format`: the runner, the members and the availability table.
- `depends_on story:cancel-a-running-bound-program`: the merged run path the step's `tree run`
  rounds use.
- **Order**, stated in prose with no depends_on:
  1. `story:person-action-runs-a-program`;
  2. the five suite stories;
  3. `story:cancelled-step-may-run-again`;
  4. `story:handoff-check-holds-the-cancel`;
  5. this story;
  6. `story:tree-suite-stop-resume-reopen-steps`.
- **Would collide with**, no edge:
  - `story:person-action-runs-a-program` (active) on the two `ApproveRequest` literals at
    `suite/run.rs:973` and `:1143`, which it edits for `runner`. This story edits the same
    literals for `decision`, `result` and `reasons`, and it merges after.
  - The five suite stories on `suite/run.rs`, `suite/availability.rs`, `tree_test/mod.rs` and the
    `tree-suite` fixtures.
  - `story:tree-test-command` (active) and `story:tree-test-decide-and-requests` (proposed) on
    the guide's table of waiting kinds, `website/docs/guides/run-a-case-tree.md:509`. This story
    takes `approve` with `result`, `reasons` or `deny` out of that row; they edit other parts of
    the same table. Textual merge; they merge first (step 2 of the order).
  - `story:suite-runs-cycles-in-parallel` (active) and
    `story:suite-cycle-writes-a-workspace-file` (active) on
    `crates/loom-tree/tests/suite_format/mod.rs`. This story changes the expected availability of
    the three `approve` members (test 6); they add their own tests to the
    module. Textual merge; they merge first (step 2 of the order).
  - `story:tree-suite-stop-resume-reopen-steps` on `suite/run.rs`, `suite/availability.rs`,
    `tree_test/mod.rs`, `suite_format/mod.rs` and the guide's table. It merges after this story.
