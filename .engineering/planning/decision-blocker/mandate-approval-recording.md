---
format: aep.planning-md/3
id: decision-blocker:mandate-approval-recording
kind: decision-blocker
status: cleared
title: Nobody has decided who records the Mandate approval a resume consumes
relations:
- blocks: story:mandate-approval-on-resume
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:39:55Z", actor: "human:timo", revision: 3}
---
## Question

Who records the Mandate approval a resume consumes, and where does its consumption persist?

## Facts

- Mandate v0.9.0 has no command that creates an approval: an approval is an entry in its agent
  records, and `check_with_approval` consumes it only if the caller stores the
  `ApprovedDecisionRecorded` event it returns and applies it again on the next load.
- Mandate's graph and policy have only test doubles outside its tests; its own control plane warns
  against claiming a decision over them.
- `b10x-loom approve` today writes `intake.routing.Grant` into the state directory and performs
  nothing; `resume` takes the Suspension before it performs anything.

## Options

- **A.** `approve` writes a Mandate approval (approver: the operator's principal, with an expiry)
  and an approval event log in the state directory; `resume` folds it, records
  `ApprovedDecisionRecorded` before it acts and under the same lock as taking the Suspension, so a
  crash means the effect never runs rather than runs twice. Cost: Loom keeps a Mandate event log;
  the CLI configures Mandate from a host-trusted file behind an explicit flag, over Mandate's
  doubles until Mandate ships a policy adapter.
- **B.** Wait until Mandate records approvals itself (a change in Mandate); Loom only passes the
  approval id. Cost: the story waits on Mandate; nothing ships here meanwhile.
- **C.** Keep Loom's grant record as the approval and bind it to Mandate's approval-required
  decision id, without Mandate consuming it. Cost: a second resume is refused by Loom's Suspension
  alone, not by Mandate; the story's "consumes the approval through Mandate" is not met.

## Recommendation

A: it meets the story within Loom, keeps the crash order safe, and moves to B's store when
Mandate ships one.

## Decision (2026-10-14)

A. `approve` writes the Mandate approval and an approval event log in the state directory;
`resume` records `ApprovedDecisionRecorded` under the Suspension lock before it acts; the CLI
configures Mandate from a host-trusted file behind an explicit flag, over Mandate's doubles until
Mandate ships a policy adapter. The flag and the log go when Mandate records approvals itself.
