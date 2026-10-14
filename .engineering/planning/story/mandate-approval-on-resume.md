---
format: aep.planning-md/3
id: story:mandate-approval-on-resume
kind: story
status: implemented
title: A resume clears and consumes the approval through Mandate
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
- depends_on: story:mandate-authority
scope:
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/resume_after_grant.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-mandate
- confidence: cited
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: generated/rust/intake
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T05:59:39Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-10-14T05:59:40Z", actor: "human:timo", revision: 12}
- {from: "active", to: "implemented", at: "2026-10-14T07:29:12Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
## Outcome

A resume after a grant clears the approval through Mandate: `b10x-loom approve --mandate <FILE>`
records a Mandate approval for the request the commission's Suspension holds, and
`resume --mandate <FILE>` performs the held request only when Mandate's `check_with_approval`
clears and consumes that approval; a second resume with the same approval is refused and performs
nothing.

## Why

Mandate v0.9.0's `check_with_approval` consumes an approval at the second check after an
approval-required decision. Loom's grant today is a record in the state directory that only Loom
reads. Mandate has no command that creates an approval, and a consumption persists only when the
caller stores the `ApprovedDecisionRecorded` event it returns and applies it again on load.

## Decision

- `approve` writes a Mandate approval (approver: the operator's principal from the host file, with
  an expiry) and appends it to an approval event log in the state directory, beside the grant
  record; the grant (`intake.routing.Grant`) names the approval it carries.
- `resume` folds the log, asks `check_with_approval` inside the authority provider's `decide`,
  bound to the held Suspension as `GrantAuthority` is, and appends `ApprovedDecisionRecorded`
  under the Suspension lock before it performs anything; a crash then means the effect never
  runs rather than runs twice.
- Mandate is configured from a host-trusted file passed by an explicit `--mandate <FILE>` flag on
  `approve` and `resume` (graph, policy and agent records over Mandate's doubles, until Mandate
  ships a policy adapter). Without the flag both behave as on 0.20.0. The flag and the log go when
  Mandate records approvals itself.
- The case's Mandate resource is `loom_mandate::case_resource_id` (UUID version 5 of the case id).

## Acceptance

- A recorded, unexpired approval clears the held request once
  (`crates/loom-cli/tests/resume_after_grant.rs` `a_recorded_approval_clears_the_held_request_once`).
- A second resume with the same approval is refused and performs nothing: no hand-off, no commit
  (`a_second_resume_with_the_same_approval_is_refused_and_performs_nothing`), across separate
  processes.
- An expired approval is refused and performs nothing
  (`an_expired_approval_is_refused_and_performs_nothing`).
- An approval naming another subject, action or resource is refused, one case each
  (`crates/loom-mandate/tests/` `an_approval_naming_another_subject_action_or_resource_is_refused`).
- The grant names its Mandate approval; a grant without it is refused when `--mandate` is given
  (`the_grant_names_its_mandate_approval`).
- Without `--mandate`, `approve` and `resume` reach the outcomes they reached on 0.20.0 (the
  existing `resume_after_grant.rs` tests unchanged).

## ESS first

The first commit changes only `ess/`: `intake.routing.Grant` names the Mandate approval it carries,
and the approval event log entry is declared beside it (`ess/intake/domains/routing.yaml`). On it
`task intake-drift` fails; later commits regenerate and make
`a_recorded_approval_clears_the_held_request_once` pass without changing `ess/`.

## Not in scope

A Mandate policy engine; wiring Mandate into `run`; widening Commission's authority port.
