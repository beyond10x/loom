---
format: aep.planning-md/3
id: story:mandate-approval-on-resume
kind: story
status: draft
title: A resume clears and consumes the approval through Mandate
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
- depends_on: story:mandate-authority
revision: 1
---
## Outcome

A resume after a grant clears the approval through Mandate: the grant `b10x-loom approve` records
names a Mandate approval, and `resume` performs the held request only when Mandate's
`check_with_approval` clears and consumes it; a second resume with the same approval is refused.

## Why

Mandate v0.9.0's `CheckWithApproval` consumes an approval at the second check after an
approval-required decision. Loom's grant today is a record in the state directory that only Loom
reads.

## Depends on

Mandate only (tag `v0.9.0`), and story:mandate-authority.

## Acceptance (draft)

- A recorded, unexpired approval naming this subject, actor, action and resource clears the held
  request once; any mismatch, an expired or a consumed approval refuses and performs nothing.

## ESS first

`intake.routing.Grant` names the Mandate approval it carries before code.
