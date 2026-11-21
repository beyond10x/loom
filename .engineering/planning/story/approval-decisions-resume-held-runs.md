---
format: aep.planning-md/3
id: story:approval-decisions-resume-held-runs
kind: story
status: proposed
title: b10x-loom run routes its approvals through decision requests and resumes only on a recorded grant
summary: DecisionAuthority, grant record writer at the decided case revision, --escalate-when, grounds checked again before an allow; no rule on the command line; a final holder's end stops the run with exit 3
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:silence-and-expiry-at-a-host-tick
- depends_on: story:llm-decider
- depends_on: story:decision-suspension-reason
scope:
- confidence: inferred
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/approval_decisions.rs
- confidence: inferred
  path: crates/loom-intake-slice/Cargo.toml
- confidence: inferred
  path: crates/loom-intake-slice/src/decision.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/decision_authority_rule.rs
- confidence: cited
  path: ess/intake/domains/routing.yaml
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:23Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom run --state DIR` can route its approvals through decision requests. With the decision
flags (`--autonomy-level`, `--autonomy-presets FILE`, `--decision-classes FILE`,
`--admitted FILE`, `--override CLASS=HOLDER`, `--escalate-when FILE`, `--principals FILE`,
`--decider ROUTE`) the run's authority provider is `DecisionAuthority` (in
`b10x-loom-intake-slice`), over the `FileDecisionStore` in the state directory, in place of the
flag-less `NoDelegatedAuthority` (`crates/loom-intake-slice/src/run.rs:716`, `:727`); on `resume`
it takes the place of `GrantAuthority` (`crates/loom-intake-slice/src/intent.rs:721`) when the
resume context holds the decision flags:

- asked about a capability for an action request, it first runs `complete_pending` over the store
  with the slice's writers (`story:decided-record-written-then-recorded`), so a record written
  before a crash is recorded before any stop is routed; then it answers `Allow` when a Decided
  request for that action and capability carries a `produced_record` at the request's case
  revision whose grant is `granted`, `Deny` when that record is `denied`, and otherwise routes the
  stop (`route_stop`) and answers `ApprovalRequired` with a request naming the decision request id.
  If the route decided the request at once (a decider or rule holder) and its record is written,
  it answers from the record instead;
- before it answers from a record, it checks the Decided request's grounds again through
  `HeldCaseGrounds` (`story:grounds-resolve-against-held-cases`), built with `GroundRoots` whose
  case directory is the run's `--workspace`, the root of `path_line` citations: a ground that no longer resolves makes it
  supersede the request, route the stop anew, and answer `ApprovalRequired`. In a Loom-held case
  an `artifact_revision` ground cannot move alone: moving the artifact raises the case revision
  (`crates/loom-governor/src/lib.rs:1042`), and `resume_after_grant` refuses `CaseMoved` before it
  asks the provider (`crates/loom-commission/src/runtime.rs:2422`; the provider is asked at
  `:2449-2451`). The grounds that can go stale while the case stays are `path_line`,
  `aep_artifact` and `sdc_decisions` citations, and this check is what catches them. Canon will
  set aside a decided authority entry whose ground moved (`story:decision-grounds`, session
  `canon`, 2026-11-21); until Canon judges grounds on those entries
  (`story:authority-records-bound-and-grounded-in-canon`), Loom applies the rule itself, here and,
  once the approvals reach Canon, where they are supplied
  (`story:approvals-supplied-to-canon-evaluation`);
- no flag supplies a rule: Loom ships no rule (`story:decider-port-and-grounding-check`). On the
  command line a `rule`, `rule_final`, `rule_and_decider` or `rule_and_decider_final` holder has
  an absent rule port, which routing treats as a failed rule result with the fixed principal
  `rule:none` (`story:decision-routing-by-holder`): `rule` and `rule_and_decider` escalate to the
  escalation person, `rule_final` and `rule_and_decider_final` end the run. A run without
  `--decider` has an absent decider port, principal `decider:none`. A host with a rule hands it to
  `DecisionAuthority` as a `RuleDecider` through `loom_sdk::intake::slice`;
- the slice hands no touched paths to the router, so a run whose `--escalate-when` lists
  `paths_touched` escalates every decider-held approval (`story:escalate-when-conditions`: an
  unknown counts as matching);
- a failure of the store, the router or the writer is a failure to decide, which Commission
  refuses (`crates/loom-commission/src/ports/authority.rs:39`);
- the router sets the request's `expires_at` through the `ExpiryInstant` port
  (`story:decision-routing-by-holder`), which the slice implements with the clock the run is given
  (`Clock`, `crates/loom-intake-slice/src/clock.rs:28`): the clock's reading plus
  `expires_after(class)` seconds, with the `chrono` the slice already depends on
  (`crates/loom-intake-slice/Cargo.toml:15`). On the command line that clock is `HostClock`
  (`clock.rs:41`), which `b10x-loom run` passes (`crates/loom-cli/src/main.rs:1200`, `:1204`);
  `run` takes no flag that fixes it. `decisions list` and `decisions show` print the request's
  `expires_at`, so `decisions tick --at` over the state directory can expire a request a real run
  opened, or decide it by `silence` with its default, at an instant read from that output. The
  library entry point takes the clock as an argument (`run_with_clock`,
  `crates/loom-intake-slice/src/run.rs:417`), so a library test fixes it.

On `RunEnds` (a final holder with no usable result, an `escalate_when` condition under a final
holder, `rule_final`, or a veto under `rule_and_decider_final`) the run ends:

- `DecisionAuthority` answers `Deny` for the stop's capability, and `Deny` for every capability it
  is asked about later in the same run, so nothing more that needs authority is performed;
- when the Commission loop ends, the slice reports `stopped: Refused (decision <id> ended the run:
  <cause>)` and `run` exits 3, the status the slice's existing table gives `Refused`
  (`crates/loom-cli/src/lib.rs:26-35`, `:195-204`); answer 3 of session `sdc` names exit 6 for
  `run-trial.sh`, which is SDC's table, not Loom's;
- no Suspension is held, so `resume` is refused `NothingSuspended`;
- the request stays as the route left it: Decided on the denying option after a veto, with its
  denied grant written; otherwise Open. It is left Open rather than superseded: `Supersede` needs a
  newer request (`~/beyond10x/sdc/ess/domains/decision.yaml:785-789`) and there is none, a listed
  person may still decide it (`~/beyond10x/sdc/docs/decisions.md:110`), and the next run of the
  case finds it by its record key and routes it again instead of opening a second one.

Commission does not await an action a verdict denied (`crates/loom-commission/src/runtime.rs:51-52`),
so an action that needs no capability may still run before the loop ends; no Commission port lets
an authority provider end a run outright.

`GrantRecordWriter` writes an approval request's record as the slice's grant
(`intake.routing.Grant`, `ess/intake/domains/routing.yaml:466`, kept in the slice's `GrantStore`,
`crates/loom-intake-slice/src/grant.rs:116`) with record id `<capability>@r<N>`, `N` the
request's decided case revision as the governor numbers it, and the request's principal,
idempotent by that id (session `sdc`, 2026-11-21, answers e and g). When this story lands,
`b10x-loom decisions` has `--store DIR` and `--at` (`story:decision-inbox-commands`) and `tick`
(`story:silence-and-expiry-at-a-host-tick`), and gains `--state DIR` here, with `GrantRecordWriter` as the writer of `canon_authority` records for every `decisions`
command over it, `decisions tick` included, and, as the `CaseRevisions` of
`story:decision-inbox-commands` that `decide`, `take` and `tick` take, the state directory's
governor over its `FileCaseStore`, opened as `b10x-loom` opens it for that directory
(`crates/loom-cli/src/main.rs:550`), answering the case revision of `CanonGovernor::action_facts`
(`crates/loom-governor/src/lib.rs:1166`, `:1199`); `--tree STORE` comes with
`story:tree-waits-open-decision-requests`, and
`story:question-and-outcome-decisions-hold-runs` adds the writers for outcome and question records
to `--state`. `b10x-loom resume` then continues the held run through `resume_after_grant`
(`crates/loom-commission/src/runtime.rs:2374`), which asks `DecisionAuthority` again and performs
the held request once only when the record is there.

`b10x-loom approve` on a commission whose Suspension names a decision request is refused
`HeldForDecisionRequest`, naming the request and the `decisions decide` command, and writes no
grant: an `Authority` hold whose request names a `DR-…` (this story's approval stop), and a
`Decision` hold, for which `story:decision-suspension-reason` answers `OtherCapability
{ held: None }` (exit 19) before the state check (`crates/loom-intake-slice/src/grant.rs:291-295`)
and this story replaces that answer. The refusal is in `approve` (`grant.rs:275`), among its reads
of the held Suspension (`:285-316`), with its own exit status in `APPROVE_HELP`
(`crates/loom-cli/src/lib.rs`).
`b10x-loom resume --mandate FILE` on such a commission is refused the same way, naming
`--mandate` and the request, and performs nothing: a grant `GrantRecordWriter` writes names no
Mandate approval. Without the decision flags, `run`, `approve` and `resume` behave as today.

`DecisionAuthority` and `GrantRecordWriter` go in their own module of `crates/loom-intake-slice`
(`decision.rs`), and `--state` on `decisions` in the inbox's module of `crates/loom-cli`
(`decisions.rs`, created by `story:decision-inbox-commands`).

## Why

Item 3 of the request: resume of the suspended run (`resume_after_grant`) after `Decide`, the
Canon write and `RecordProduced` (`~/beyond10x/sdc/docs/decisions.md:307-312`). Today a grant
comes only from `b10x-loom approve` (`crates/loom-cli/src/lib.rs:235-242`) and `GrantAuthority`
allows only the pending held request (`grant.rs:358-448`). Resume requires `produced_record`
(session `sdc`, 2026-11-21, point 6): the provider allows only on a recorded decision. Both
approval options produce a record (answer d). No instant flag is added to `run`: the coordinator
chose to read the printed `expires_at` over a new flag. `run` has no `--mandate` flag
(`RunArgs`, `crates/loom-cli/src/lib.rs:766-889`); `--mandate` is on `approve` (`:261`) and
`resume` (`:278`), so those are the two commands that meet the decision hold.

## Waits on

- `story:decision-suspension-reason`: it makes the Suspension's `action_request_id`, `action`,
  `arguments` and `expected_case_revision` optional, which forces an edit in
  `crates/loom-intake-slice/src/grant.rs` (`:293`, `:300`, `:315`, `:332`, `:402-403`), the file
  whose `approve` this story changes. `story:question-and-outcome-decisions-hold-runs` and
  `story:approvals-supplied-to-canon-evaluation` are ordered after it through this story.
- `story:silence-and-expiry-at-a-host-tick`: both change `crates/loom-cli`, and a silence
  decision is one this resume continues after.
- `story:llm-decider`: `--decider ROUTE` builds it.
- `story:decision-inbox-commands`, `story:decided-record-written-then-recorded` and
  `story:escalate-when-conditions` through `story:silence-and-expiry-at-a-host-tick`.

## ESS first

`ess/intake/domains/routing.yaml` gains `DecisionRouting` (the decision flags as the slice's
resume context keeps them) and the refusal `HeldForDecisionRequest`. On that commit
`task intake-drift` fails, and `crates/loom-cli/tests/approval_decisions.rs` does not compile.

## Acceptance

On the command line (`crates/loom-cli/tests/approval_decisions.rs`), with the intake slice's
fixture protocol (`software-change@1`, the protocol `resume` continues), stub programs, a recorded
selector, fixture autonomy documents and principals, a recorded decider route, and no model call:

- At `A0` a run that reaches `repository.merge` ends `NeedsAuthority`, holds an `Authority`
  Suspension whose request names `DR-…`, and `decisions list --principal timo --state DIR` shows
  it; `decisions decide DR-… A --as timo --state DIR` writes a granted grant with record id
  `repository.merge@r<N>`, `N` the case revision at the decision; `resume` performs the merge
  request once.
- `resume` before the decision is refused naming the capability as not granted, and performs
  nothing.
- Deciding `B` (deny) writes a denied grant; `resume` is refused and performs nothing.
- At `A2` with a classes document that puts `repository.merge` in class `context` (holder
  `decider`) and a recorded decider reply `A` whose grounds resolve, the run does not stop: the
  request is Decided by `decider:stub`, the grant written, and the merge performed in the same run.
- The same `A2` run with `--escalate-when` listing `paths_touched: ["**"]` stops
  `NeedsAuthority` with the request Escalated to the escalation person and holder `person`.
- At `A4` (merge `rule_and_decider`) with no rule, the run stops `NeedsAuthority` and the request
  is Escalated to the escalation person with no recommendation and the reason
  `no rule configured`.
- At `A5` (merge `rule_and_decider_final`) with no rule, the run ends with
  `stopped: Refused (decision DR-… ended the run: failure)` and exit 3, nothing is performed, and
  the request is Open with one `EscalationSuppressedRecord` of cause `failure` whose reasons name
  `no rule configured`.
- At `A5` with `repository.merge` in class `context` (`decider_final`), `--escalate-when` listing
  `decider: escalate` and a recorded decider `Escalate` with `best: A`, the run ends with exit 3
  naming the request and cause `condition`, nothing is performed, the request is Open with one
  `EscalationSuppressedRecord`, and `decisions list --principal decider:stub --state DIR` lists
  it; a second `run` of the same intent routes that request again and opens no other.
- At `A0` with the classes document giving the merge class `expires_after: 3600`, `decisions show
  DR-… --state DIR` prints an `expires_at` 3600 seconds after the request's opened instant;
  `decisions tick --at <expires_at minus 1 second> --state DIR` leaves it Open,
  `decisions tick --at <expires_at> --state DIR` expires it, and `resume` then performs nothing and
  the Suspension stays.
- At `A1` with the same expiry and a recorded decider reply `A`, strong, with a default on silence,
  `decisions tick --at <expires_at> --state DIR`, the instant read from `decisions show`, decides
  the request by `silence`, writes the granted grant `repository.merge@r<N>` and records it;
  `resume` then performs the merge once.
- With the case moved after the decision (the test raises an artifact through
  `CanonGovernor::update_revision` over the state directory's case store), `resume` exits 14
  (`CaseMoved`) naming both case revisions, nothing is performed, and the request stays Decided.
- At `A1` a request whose recorded decider recommendation `A`, strong, has a `path_line` ground on
  lines 1-3 of `notes.md` in the workspace, taken with `decisions take DR-… --as timo --state DIR`:
  after those lines are rewritten and not committed, the case revision is unchanged, `resume` is
  refused naming `repository.merge` as not granted (not `CaseMoved`), the request is Superseded
  naming a new Open request, and nothing is performed.
- `approve` on a commission held for `DR-…` exits non-zero naming `HeldForDecisionRequest`, the
  request and the `decisions decide` command, and writes no grant.
- With a state directory whose commission holds a `Decision` Suspension for `DR-…` (taken through
  `HoldSuspension` with reason `Decision` and no action request, `story:decision-suspension-reason`,
  by the test's setup), `approve` exits with `HeldForDecisionRequest`'s status naming `DR-…`, not
  19 `OtherCapability`, and writes no grant.
- `resume --mandate FILE` on a commission held for a granted `DR-…` exits non-zero naming
  `--mandate` and the request, and performs nothing.
- With the held request's file (`decision-<stem>.json` in the state directory) overwritten by bytes
  that do not decode, `resume` exits non-zero naming the file and performs nothing.
- Without the decision flags, `crates/loom-cli/tests/resume_after_grant.rs` passes, unchanged by
  this story.

As a library test (`crates/loom-intake-slice/tests/decision_authority_rule.rs`) that drives the
slice through `loom_sdk::intake::slice` (`run_with_clock` and the slice's resume) with
`DecisionAuthority` given a `RuleDecider` over a fixture function, a recorded decider, a clock
fixed at `2026-11-21T12:00:00Z`, and no model call:

- At `A0` with the merge class's `expires_after: 3600`, the request's `expires_at` is
  `2026-11-21T13:00:00Z`; a tick at `12:59:59` leaves it Open and one at `13:00:00` expires it.
- At `A4` with a rule verdict that grants and a recorded decider reply `Take`, the run does not
  stop: the request is Decided by the decider through `TakeRecommendation`, the grant written, and
  the merge performed in the same run.
- At `A5` with a rule verdict that grants and a recorded decider veto, the slice reports
  `stopped: Refused (decision DR-… ended the run: veto)`, the merge is not performed, no
  Suspension is held, and the request is Decided `B` with a denied grant `repository.merge@r<N>`.
- At `A4` with `escalate_when: [{decider: escalate}]` and a recorded decider `Escalate`, the request
  is Escalated to the escalation person; at `A5` the same run reports
  `stopped: Refused (decision DR-… ended the run: condition)`, and the request is Open with one
  `EscalationSuppressedRecord` of cause `condition`.
- With the case moved after a granted decision, the slice's resume is refused `CaseMoved` and a
  counting decision store given to `DecisionAuthority` records no read.
- A decision store double that fails every read ends the run with nothing performed and the run
  reports the store failure.
- A store holding a request whose grant was written but not recorded (`RecordProduced` not sent)
  and a case moved since: the next run records it through `complete_pending` before it routes the
  stop, so the request carries its `produced_record` and is not superseded.
- `task check` exits 0.

## Out of scope

Question and outcome decisions (`story:question-and-outcome-decisions-hold-runs`). Mandate
approvals with decision requests: `approve --mandate` and `resume --mandate` on a decision-held
commission are refused. Supplying the grants to Canon's evaluation
(`story:approvals-supplied-to-canon-evaluation`). Computing touched paths from the change's diff.
A command-line flag that supplies a rule. A flag that fixes `run`'s instant.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice` (new `DecisionAuthority` and `GrantRecordWriter` module, the `approve` refusal, the run and resume wiring) — cited, "go in their own module of `crates/loom-intake-slice`"
- **Second surface:** `crates/loom-cli` (decision flags on `run`, `--state` on `decisions`, exit table, clock sites) — cited, story Outcome
- **Files (cited):**
  - `crates/loom-intake-slice/src/grant.rs:275-316` — `approve` gains the refusal for a Suspension naming a decision request; the story cites `:285-316`, which sits inside `approve` at `:275`
  - `crates/loom-intake-slice/src/run.rs:417` — `run_with_clock`, the library entry the test fixes the clock through
  - `crates/loom-cli/src/main.rs:1200,1204` — `&HostClock` passed to `run_prepared_intent*`
  - `crates/loom-cli/src/lib.rs:26-35,195-204,235-242` — exit table, `exit_status`, `Approve` subcommand
  - `ess/intake/domains/routing.yaml` — `DecisionRouting` and `HeldForDecisionRequest` (story "ESS first")
  - `crates/loom-cli/tests/approval_decisions.rs`, `crates/loom-intake-slice/tests/decision_authority_rule.rs` — new tests
- **Files (inferred):**
  - `crates/loom-intake-slice/src/decision.rs` (new; the story names no file), `src/lib.rs` (its `mod` line)
  - `crates/loom-intake-slice/src/run.rs:716,727,1059` — `NoDelegatedAuthority` is the run-time provider and is replaced when the flags are given
  - `crates/loom-intake-slice/src/intent.rs:536,654,721` — `ResumeState` and `resume_with_state` build `GrantAuthority`; resume must build `DecisionAuthority` instead
  - `crates/loom-cli/src/decisions.rs` (the inbox module; the name comes from `story:decision-inbox-commands`, which creates it). `crates/loom-cli/src/main.rs:616` (`resume`) for the decision-held resume
  - `crates/loom-intake-slice/Cargo.toml` — a dependency on the decision crate that sibling stories create (`crates/loom-decision`)
- **Symbols:** `DecisionAuthority`, `GrantRecordWriter`, `HeldCaseGrounds`, `route_stop`, `FileDecisionStore`, `RuleDecider` — cited, all named by the story. None exists in the tree yet (`rg` finds nothing); siblings declare them
- **Symbols that exist:** `resume_after_grant` (`crates/loom-commission/src/runtime.rs:2374`), `ResumeRefusal::CaseMoved` (`:2422`), `NotGranted` (`:2451`), `GrantAuthority` (`grant.rs:358`), `GrantData`, `run_exit_status`, `StopReason::Refused` → 3 (`lib.rs:195-204`) — cited
- **Documents:** none outside doc comments and the CLI help strings (`lib.rs` `EXIT_STATUS`, `APPROVE_HELP`, `RESUME_HELP`) — inferred
- **Confidence:** medium — the story names the surfaces; the new module's file name, whether `intent.rs` or `resume.rs` change, and the wiring are my readings. Everything it builds on is undelivered, so no symbol can be read yet
- **Would collide with**, ordered by a `depends_on` path (from `aep plan artifact graph --format json`):
  - `story:decision-suspension-reason`, `grant.rs` `:293,:300,:315,:332,:402-403` — ordered, direct dependency
  - `story:silence-and-expiry-at-a-host-tick`, `crates/loom-cli` — ordered, direct dependency
  - `story:decision-inbox-commands` (`decisions.rs`, `lib.rs`), `story:decided-record-written-then-recorded`, `story:escalate-when-conditions` — ordered through silence
  - `story:llm-decider`, `story:decision-routing-by-holder`, `story:grounds-resolve-against-held-cases` — ordered, ancestors
  - `story:question-and-outcome-decisions-hold-runs` (`run.rs`, `resume.rs`, `lib.rs`, `--state` on `decisions`) and `story:approvals-supplied-to-canon-evaluation` (`grant.rs`) — ordered after this story
  - `story:tree-waits-open-decision-requests`, `story:requests-from-canon-pending-decisions`, `story:non-independent-review-becomes-decision-request`, `story:budget-and-limit-stops-raise-decision-requests`, `story:flow-limits-recorded-as-events` — ordered after, descendants (`flow-limits-recorded-as-events` edits `crates/loom-cli/src/lib.rs`)
- **Would collide with, no `depends_on` path either way** (a decision for the coordinator):
  - `story:bound-commands-run-confined` (`run.rs`, `intent.rs`, `lib.rs`, `main.rs`)
  - `story:confinement-opens-from-engine-config` (`lib.rs`, `main.rs`, `intent.rs`, `resume.rs`)
  - `story:tree-run-command` (`lib.rs`, `main.rs`, `grant.rs`, `run.rs`)
  - `story:secrets-subcommand` (`lib.rs`, `main.rs`)
  - `story:run-caps-per-arrival` and `story:engine-configuration-document` (`lib.rs`)
  - `story:canon-0-9-0-pin` (wave 2026-11-21-w2, active; `crates/loom-intake-slice/Cargo.toml`)
  - `story:engineering-registry-calls-replaced` (`ess/intake/domains/routing.yaml`; it edits only a comment, `:247`)
  - Their files are from reading their bodies, not their recorded scopes; `story:canon-0-9-0-pin` is the only one in this wave
- **Contradicted by the code:** see Story defects (1) to (4)
- **Safety fact:** without the decision flags `run`, `approve` and `resume` stay as today, because the flag-less path keeps `NoDelegatedAuthority` (`run.rs:716,727`) and `GrantAuthority` (`intent.rs:721`) — step 2, unproven. The acceptance item "`resume_after_grant.rs` passes, unchanged" tests it
- **Safety fact:** `CaseMoved` is refused before the provider is asked, so `DecisionAuthority` is not read on a moved case. `runtime.rs:2422` precedes the authority call at `:2451` — step 2, unproven
