---
format: aep.planning-md/3
id: story:handoff-holds-no-suspension
kind: story
status: draft
title: A merge proposed again at a handed-off HEAD holds no Suspension
relations:
- serves: vision:O1
- informed_by: review-result:adv-w7-u3-pass-1
revision: 1
---
## Outcome

A merge the agent proposes again at a `HEAD` its commission already handed off holds no
Suspension: the run ends `MergeHandedOff` and leaves nothing in the state directory to approve.

## Why

Since `story:cli-resume-after-grant`, the merge hand-off writes a record and changes nothing in the
case, so the frontier still offers `repository.merge`. A merge proposed again at the handed-off
`HEAD` is held as a Suspension that `approve` refuses (`AlreadyHandedOff`); the Suspension and its
resume context stay until something moves the case (adversary pass 1 of that story,
`review-result:adv-w7-u3-pass-1`, F2). Not holding it needs Commission's runtime to know the
request was already handed off, or the case to record the hand-off so the frontier drops the merge.

## Not decided

Whether the hand-off becomes a case observation the protocol reads (a change in the
software-change protocol, outside this repository), or the runtime consults the host before it
holds a Suspension. A work branch fast-forwarded after the grant is a separate option.
