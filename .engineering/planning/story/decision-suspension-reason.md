---
format: aep.planning-md/3
id: story:decision-suspension-reason
kind: story
status: active
title: Commission holds a run for a decision request and resumes it only once it is decided and any record it produces is recorded
summary: SuspensionReason::Decision, a hold with no action request, resume_after_decision with Recorded or NoRecord; the optional Suspension members reach the slice's grant and handoff modules and the CLI's resume refusals, so the approval and inbox stories wait on it
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-agent/tests/serve.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/adversary_w10_u3.rs
- confidence: cited
  path: crates/loom-cli/tests/adversary_w7_u2_state.rs
- confidence: cited
  path: crates/loom-cli/tests/adversary_w7_u3_resume.rs
- confidence: cited
  path: crates/loom-cli/tests/resume_after_grant.rs
- confidence: cited
  path: crates/loom-cli/tests/run_state.rs
- confidence: inferred
  path: crates/loom-cli/tests/security_w10_u3.rs
- confidence: cited
  path: crates/loom-commission-conformance/src/codec.rs
- confidence: cited
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-commission-conformance/src/store.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/adversary_w20261021w7_deadline_after_call.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/adversary_w20261021w7_runtime.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/adversary_w20261022w1_host_stop.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/adversary_w6_suspend_resume.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/adversary_w6_suspend_resume_pass2.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/approval_suspend_resume.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/decision_suspend_resume.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/host_stop.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/pending_effect.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests/run_and_action_budgets.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/stop_policy.rs
- confidence: cited
  path: crates/loom-commission/src/ports/suspension.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-governor/src/file_suspension.rs
- confidence: inferred
  path: crates/loom-governor/tests/suspension_budget_limits.rs
- confidence: inferred
  path: crates/loom-governor/tests/suspension_host_stop.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: cited
  path: crates/loom-intake-slice/src/handoff.rs
- confidence: cited
  path: crates/loom-mandate/tests/mandate_authority.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:21Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T04:23:21Z", actor: "agent:sdc", revision: 17, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w3"}
---
## Outcome

Commission can hold a run for a decision request and resume it only once the decision is taken
and, where the decided option produces a Canon record, recorded. `SuspensionReason` gains
`Decision` (`DecisionHold`: the request id and what it unblocks, `approval`, `question` or
`outcome`). When an executor returns `Suspended` with a `Decision` reason, `run_with_suspension`
suspends the Run, as for any executor suspension today
(`crates/loom-commission/src/runtime.rs:1908-1924`), and also holds the commission's Suspension
`Pending` through `HoldSuspension` with that reason, the Run, its case and case revision, and no
action request, expected case revision, action or arguments.

`resume_after_decision` is given the decided hold (`DecidedHold`: the request id and either
`Recorded` with the request's `produced_record`, or `NoRecord` for a decided option that produces
none). In this order it:

1. reads the commission's Suspension: none is `NothingSuspended`;
2. refuses a Suspension whose reason is not `Decision`, which holds an action request, with
   `HeldForAuthority`, and one whose `DecisionHold` names another request id with `OtherDecision`;
   neither changes the Suspension;
3. takes it (`TakeSuspension` with no action request id; the held Suspension has none, so the
   `other-request` guard passes); a `Resuming` Decision Suspension, left by a resume that stopped
   between the take and the release, is not taken again but goes straight to step 5, because a
   Decision hold performs nothing that could be in doubt;
4. resumes the Run (`ResumeRun`);
5. releases the Suspension (`ReleaseSuspension` with no action request id);
6. continues the loop from the case's current revision.

It performs no held request, and a case revision that moved since the hold is expected (an
answered question moves an artifact), not a refusal. After a `NoRecord` hold the loop continues as
well: an outcome left untaken leaves the case where it was, so the run then ends as its frontier
decides, directly after the decision (session `sdc`, 2026-11-21, answer d). Commission cannot tell
whether an option produces a record; the caller that builds the `DecidedHold` reads it from the
request (`story:question-and-outcome-decisions-hold-runs`).

An approval keeps the existing path: the authority provider answers `ApprovalRequired`, the
Suspension's reason is `Authority`, and `resume_after_grant` performs the held request once after
the provider allows it (`runtime.rs:384-392`, `:2374`). `resume_after_grant` on a `Decision` hold
is refused `HeldForDecision` right after the Suspension is read (`runtime.rs:2400-2405`), before
the `InDoubt` and `CaseMoved` checks (`:2406-2426`) read the absent members.

The three refusals are new variants of `ResumeRefusal` (`runtime.rs:896-935`), Commission's Rust
refusal type, not ESS types. `resume_refusal_status` matches that enum with no wildcard
(`crates/loom-cli/src/lib.rs:138-150`), so `b10x-loom resume` gains exit status 19 for
`HeldForDecision`, and `RESUME_HELP` (`lib.rs:116-135`) a line
`19  HeldForDecision: the commission's Suspension holds the run for a decision request; nothing is performed`.
`HeldForAuthority` and `OtherDecision` come only from `resume_after_decision`, which no command
calls in this story; the match maps them to 19 too, and the help does not list them.

`b10x-loom approve` (`crates/loom-intake-slice/src/grant.rs:275`) on a Decision hold: before its
state check (`:291-295`), a Suspension with no action request answers
`ApproveRefusal::OtherCapability { held: None }`, as a `Pending` hold for no capability does today
(`:296-299`), so `approve` exits 19 (`lib.rs:83`) and writes no grant, whether the hold is
`Pending` or `Resuming`. An `Authority` hold is answered as before. `story:approval-decisions-resume-held-runs`
replaces this answer by its own refusal.

The Suspension file of `FileSuspensionStore` keeps all ten members (`MEMBERS`,
`crates/loom-governor/src/file_suspension.rs:56-67`, checked by `exactly` at `:437`): an absent
`action_request_id`, `expected_case_revision`, `action` or `arguments` is written as JSON `null`,
and the decoder reads `null` as absent for those four members only. A file written before this
story has all four and decodes unchanged.

Making those four members optional reaches past Commission. `git grep -n 'HoldSuspension {'` and
`git grep -n action_request_id -- crates ess`, narrowed to the Suspension types, give every file
this story's commit must edit.

Production code:

- `crates/loom-commission/src/ports/suspension.rs`: the `SuspensionHeld` event (`:145-149`), the
  stored `SuspensionData` (`:150-161`), `other_request` (`:174-183`), `decide_take` (`:204`,
  `:210-213`, `:228-231`), `decide_release` (`:256`, `:265-268`);
- `crates/loom-commission/src/runtime.rs`: the `ResumeRefusal` variants (`:896-935`), the
  `CommissionSuspended` Display (`:823-827`), the `SuspensionStateConflict` of a commission that
  holds a Suspension (`:1205-1208`), the executor-suspension branch that now also holds a Decision
  Suspension (`:1908-1924`), the release helper (`:2243-2246`), the `HoldSuspension` literal of
  `suspend_for_approval` (`:2318-2330`, each member wrapped in `Some`), `resume_after_grant`'s
  reads of the held request (`:2406-2435`, `:2519-2522`, `:2566-2578`), and the new
  `resume_after_decision`;
- `crates/loom-governor/src/file_suspension.rs`: the member encoder (`:261-270`), the reason
  encoder (`:281-301`), the member decoder (`:452-464`), the reason decoder (`:469-506`);
- `crates/loom-commission-conformance/src/codec.rs`: the reason codec (`:198-216`) and its inverse
  (`:221-247`); `src/lib.rs`: `SuspensionSettled` (`:462`), `SuspensionStateConflict` (`:479`),
  the `HoldSuspension` input (`:512-523`), `SuspensionHeld` (`:536`), the `TakeSuspension` input
  (`:558-561`), `SuspensionTaken` (`:575`), the `ReleaseSuspension` input (`:600-603`), the
  `SuspensionStates` row (`:656`); `src/store.rs:222`;
- `crates/loom-intake-slice/src/grant.rs`: `approve` (`:291-299`, `:300`, `:315`, `:332`) and
  `GrantAuthority` (`:402-403`); `src/handoff.rs`: the merge check of `SettledHandoffs::hold`
  (`:430`) and its `SuspensionSettled` (`:448-450`);
- `crates/loom-cli/src/lib.rs`: `RESUME_HELP` (`:116-135`) and `resume_refusal_status` (`:138-150`).

Tests whose member reads gain an unwrap and change no assertion:
`crates/loom-agent/tests/serve.rs:628`, `crates/loom-cli/tests/adversary_w10_u3.rs:331-335`,
`adversary_w7_u2_state.rs:196`, `adversary_w7_u3_resume.rs:398`, `resume_after_grant.rs:933-937`,
`run_state.rs:134`, `:155`, `crates/loom-mandate/tests/mandate_authority.rs:586`,
`crates/loom-intake-slice/src/handoff.rs:706` (its test module), and in
`crates/loom-commission-testkit/tests` `adversary_w6_suspend_resume.rs:251`,
`adversary_w6_suspend_resume_pass2.rs` (`:147`, `:171`, `:232`, `:419-470`, `:543-564`),
`approval_suspend_resume.rs` (`:492-562`, `:1331`, `:1425`), `pending_effect.rs:1227-1228` and
`stop_policy.rs:488`.

Tests whose `HoldSuspension`, `TakeSuspension` or `ReleaseSuspension` literals wrap the members in
`Some` and change no assertion: `crates/loom-cli/tests/security_w10_u3.rs:92`,
`crates/loom-governor/tests/suspension_budget_limits.rs:34`, `suspension_host_stop.rs:33`,
`crates/loom-intake-slice/src/grant.rs:660`, `:704-712`, `:763` and `src/handoff.rs:655` (their
test modules), and in `crates/loom-commission-testkit/tests`
`adversary_w20261021w7_deadline_after_call.rs:656`, `adversary_w20261021w7_runtime.rs:657`,
`:1797`, `adversary_w20261022w1_host_stop.rs:520`, `host_stop.rs:665` and
`run_and_action_budgets.rs:1063`, `:1124`.

## Why

A run suspends today only on a capability `ApprovalRequired` (`runtime.rs:384-392`);
`NeedsHumanJudgment` ends it with no Suspension (`crates/loom-commission/src/outcome.rs:118-124`),
and an executor's `Suspended` suspends the Run but holds no Suspension, so nothing can resume a run
held for a question or an outcome decision. The contract's order ends "The run resumes. Resume
requires `produced_record`." (`~/beyond10x/sdc/docs/decisions.md:232`; request item 3,
`decisions.md:309-312`) for an option that produces a record (session `sdc`, 2026-11-21, point 6
and answer d); the `DecidedHold` type carries it, so Commission sees the record and never reads
Canon or the decision store (`AGENTS.md:96-97`).

## Waits on

Nothing in this epic. `story:approval-decisions-resume-held-runs` depends on this story: both edit
`crates/loom-intake-slice/src/grant.rs`, this one because the type change forces it (above), that
one for `approve`'s refusal of a decision-held commission. `story:question-and-outcome-decisions-hold-runs`
(which also depends on this story directly) and `story:approvals-supplied-to-canon-evaluation`,
which edits the same grant module, come after it through that story.
`story:decision-inbox-commands` also edits `crates/loom-cli/src/lib.rs` and now depends on this
story.

`story:child-case-runtime` and `story:deterministic-default-selector` (`epic:protocol-tree-on-bindings`)
also change `ess/commission/domains/responsibility.yaml`; no edge orders them, because
`story:child-case-runtime` waits on an upstream blocker. Whichever is taken second rebases on the
other. The other stories outside this epic that share `responsibility.yaml`, `runtime.rs` or a test
file above are in the typed scope, and `aep plan artifact waves` keeps them apart.

## ESS first

`ess/commission/domains/responsibility.yaml`:

- `SuspensionReason` (`:145-158`) gains `Decision: commission.responsibility.DecisionHold`;
  `DecisionHold` (request id, unblocks `approval` | `question` | `outcome`), `DecidedHold` and the
  union `DecidedRecord` (`Recorded` carrying the record as JSON, `NoRecord`) are added;
- `Suspension` (`:1234-1262`): `action_request_id`, `expected_case_revision`, `action` and
  `arguments` become optional, with an invariant that they are present unless the reason is
  `Decision`; the `action_request` relation keeps `via: action_request_id` (`:1263-1267`), and if
  `ess/22` refuses a relation via an optional field the relation is dropped and the invariant
  stays;
- `HoldSuspension` (`:1618-1675`): the same four inputs become optional; its `settled` payload
  carries `action_request_id` when given;
- `TakeSuspension` (`:1682-1720`) and `ReleaseSuspension` (`:1728-1766`): `action_request_id`
  becomes optional; the `other-request` guard is unchanged (an absent input matches an absent held
  member);
- `SuspensionHeld`, `SuspensionTaken` (`:1949-1963`), `SuspensionSettled` (`:1493-1500`),
  `SuspensionStateConflict` (`:1473-1478`) and `SuspensionRequestMismatch` (`:1482-1490`, both
  request ids): every `ActionRequestId` member becomes optional;
- the `SuspensionStates` row (`:1986-1999`): `action_request_id` becomes optional.

On that commit the commission drift check fails, and
`crates/loom-commission-testkit/tests/decision_suspend_resume.rs` does not compile.

## Acceptance

In `crates/loom-commission-testkit/tests/decision_suspend_resume.rs`, with a fixture executor and
an in-memory suspension port:

- An executor that returns `Suspended(Decision{request_id: "DR-1", unblocks: question})` ends the
  run `Suspended`, and the suspension port holds one `Pending` Suspension with that reason and no
  action request; nothing is performed.
- `resume_after_decision` with `DR-1` and `Recorded`, after an artifact moved, continues the loop
  at the current case revision, and the Suspension is released.
- After that resume, a second `resume_after_decision` with `DR-1` is refused `NothingSuspended`.
- `resume_after_decision` with `DR-1` and `NoRecord` on an outcome hold releases the Suspension and
  the run ends as the unchanged frontier decides (`NoAdmissibleAction` in the fixture), with
  nothing performed.
- `resume_after_decision` with `DR-2` is refused `OtherDecision`, and the Suspension stays.
- `resume_after_grant` on a `Decision` hold is refused `HeldForDecision`, and the Suspension stays.
- `resume_after_decision` on an `Authority` hold is refused `HeldForAuthority`, and the Suspension
  stays.
- `resume_after_decision` on a `Resuming` Decision Suspension (the port fixture stores one)
  releases it and continues the loop, performing nothing.
- A commission holding a `Decision` Suspension starts no Run: `run_with_suspension` answers
  `LoopFailure::CommissionSuspended` with state `Pending` and no action request.

Elsewhere:

- A `FileSuspensionStore` holds a `Decision` Suspension across a real process restart, and its
  file has `"action_request_id": null` (`decision_suspend_resume.rs`).
- A Suspension file written before this story (with an action request) still decodes:
  `crates/loom-commission-testkit/tests/approval_suspend_resume.rs` passes, its diff only unwrapping
  the optional members.
- `b10x-loom resume` on a state directory whose Suspension is a `Decision` hold exits 19 naming
  `HeldForDecision`, and the Suspension file is byte-identical
  (`crates/loom-cli/tests/resume_after_grant.rs`, the hold written through `FileSuspensionStore`).
- `b10x-loom approve` on the same state directory exits 19 (`OtherCapability`) and writes no grant
  (`resume_after_grant.rs`).
- `b10x-loom approve` and `resume` on an `Authority` hold behave as before: the existing cases of
  `crates/loom-cli/tests/resume_after_grant.rs` pass.
- `crates/loom-commission/Cargo.toml` still depends only on the generated `commission` crate.
- `task check` exits 0.

## Out of scope

Who returns the `Decision` suspension, opening the request, and reading whether its option
produces a record (`story:question-and-outcome-decisions-hold-runs`). A command that calls
`resume_after_decision`, and its exit statuses (`story:decisions-resume-the-held-run`:
the command line resumes approval holds only and refuses any other hold `NotRebuildable`, exit
31; `HeldForAuthority` and `OtherDecision` reach a host through `resume_decided_request` and have
no exit status of their own).
Any timer. Changing the approval path. `approve`'s own refusal of a decision-held commission
(`story:approval-decisions-resume-held-runs`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-commission` (`src/runtime.rs`, `src/ports/suspension.rs`) with `ess/commission/domains/responsibility.yaml` — cited
- **Files, ESS first:** `ess/commission/domains/responsibility.yaml:145-158` (`SuspensionReason`), `:1234-1262` (`Suspension`), `:1618-1650` (`HoldSuspension`) — cited
- **Files, Commission:** `crates/loom-commission/src/ports/suspension.rs:150,179-181,230,267`; `src/runtime.rs:1207,2408-2433,2521,2572` and the `HoldSuspension` literal `:2318-2330` — cited
- **Files, stores and conformance:** `crates/loom-governor/src/file_suspension.rs:281-301,452-497`; `crates/loom-commission-conformance/src/codec.rs:198-216` (+ inverse `:221-247`) and `src/lib.rs:512-523` — cited
- **Files, slice:** `crates/loom-intake-slice/src/grant.rs:293,300,315,332,402-403`; `src/handoff.rs:706` — cited
- **Files, tests:** the 16 test files the story names (see `paths`) plus new `crates/loom-commission-testkit/tests/decision_suspend_resume.rs` — cited
- **Symbols:** `SuspensionReason::Decision`, `DecisionHold`, `DecidedHold`, `DecidedRecord`, `resume_after_decision`, `run_with_suspension`, `SuspensionData` members `action_request_id`, `action`, `arguments`, `expected_case_revision` — cited
- **Also forced, not in the story:** `file_suspension.rs:61-65` (`MEMBERS`, checked by `exactly` at `:437`), encoder `:261-270`, decoder `:459-463` — cited from code
- **Also forced, not in the story:** `commission-conformance/src/lib.rs:462,479,536,575,656` and `src/store.rs:222` (events, settled, conflict, view row, all `action_request_id.0.0`) — cited from code
- **Also forced, not in the story:** `runtime.rs:826` (`LoopFailure::CommissionSuspended` Display), `ports/suspension.rs:148,204,212,256` — cited from code
- **Also forced, not in the story:** `HoldSuspension { .. }` literals now needing `Some(..)`: `loom-cli/tests/security_w10_u3.rs:92`, `grant.rs:660,763`, `handoff.rs:655`, `loom-governor/tests/suspension_budget_limits.rs:34`, `suspension_host_stop.rs:33`, testkit `adversary_w20261021w7_deadline_after_call.rs:656`, `adversary_w20261021w7_runtime.rs:657,1797`, `adversary_w20261022w1_host_stop.rs:520`, `host_stop.rs:665`, `run_and_action_budgets.rs:1063,1124`, `adversary_w6_suspend_resume_pass2.rs:545,564` — cited from code (`git grep 'HoldSuspension {'`)
- **Also likely:** `crates/loom-cli/src/lib.rs:140-150` — inferred, `resume_refusal_status` matches `ResumeRefusal` with no wildcard; it breaks only if the new refusals become `ResumeRefusal` variants, which the story does not say
- **Documents:** none beyond the ESS file — cited (story's Acceptance is code and tests); `docs/commission/design/approval-gate.md` would only describe the new hold — inferred, optional
- **Confidence:** high for the primary surface and the story's own list (every cited line was read and says what the story says); medium for the full file set, because the story's list is short by about eight files (below)
- **Would collide with:**
  - `story:approval-decisions-resume-held-runs` — `grant.rs`, `runtime.rs`, `resume_after_grant.rs`, `loom-cli/src/lib.rs`; `depends_on` this story, ordered — cited
  - `story:question-and-outcome-decisions-hold-runs` — `runtime.rs`, `responsibility.yaml`; `depends_on` this story, ordered — cited
  - `story:approvals-supplied-to-canon-evaluation` — `runtime.rs`, `grant.rs`; ordered through `approval-decisions-resume-held-runs` — cited
  - `story:child-case-runtime` (proposed) — `responsibility.yaml` (`Case` lifecycle `Terminated`), body also cites `runtime.rs`; no `depends_on` path either way, unordered — cited
  - `story:frontier-action-names-its-claims` (draft) — scope has `runtime.rs` and `responsibility.yaml`; depends only on `unperformable-names-actions` (implemented), unordered — cited
  - `story:frontier-names-movers`, `story:needs-authority-before-host-stop`, `story:unperformable-names-the-claim` (draft) — bodies edit `runtime.rs` and `responsibility.yaml`, no scope recorded; the last two depend on `frontier-names-movers`, none on this story, unordered — cited from bodies
  - `story:managed-composition` (draft) — scope has `responsibility.yaml`; no edge, unordered — cited
  - `story:decision-inbox-commands` and the tree stories — `loom-cli/src/lib.rs` only if the refusals reach it; no path to this story — inferred
  - not a collision: `story:decision-request-file-store` names `file_suspension.rs` as a model only; `story:tree-run-command` cites `grant.rs` and `runtime.rs` as reading
- **Contradicted by code:** the story's file list is incomplete, and its `ESS first` section leaves `TakeSuspension` and `ReleaseSuspension` taking a required `action_request_id` (see Story defects) — cited
- **Safety fact:** a pre-story Suspension file still decodes because the decoder reads `action_request_id`, `action`, `arguments` and `expected_case_revision` unchanged when present (`file_suspension.rs:452-464`); only `MEMBERS`' strict `exactly` must admit absence — step 2, unproven
