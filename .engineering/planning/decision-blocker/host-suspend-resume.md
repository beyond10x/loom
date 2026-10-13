---
format: aep.planning-md/3
id: decision-blocker:host-suspend-resume
kind: decision-blocker
status: cleared
title: Nobody has decided where a host keeps a Suspension, where a command-line grant comes from, or what resume reads
relations:
- blocks: story:cli-run-holds-suspension
- blocks: story:cli-resume-after-grant
- blocks: story:host-suspension-store
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-13T07:10:01Z", actor: "human:timo", revision: 2}
---
## Question

For a host that suspends and resumes a governed run across processes (`b10x-loom run --state`,
`b10x-loom resume`): (a) where does the production file-backed suspension store live, (b) where does
a command-line user's grant come from, and (c) what does the state directory keep so that `resume`
can rebuild the run?

## Options

- (a) A: `loom-governor`, beside `FileCaseStore`; B: a module of `b10x-loom-commission`; C:
  `loom-intake-slice`, usable only by the command line.
- (b) A: `resume --grant <capability>`, the resume command is the approval; B: `b10x-loom approve
  <commission> <capability>` writes a grant record into the state directory and `resume` reads it;
  C: an external provider, such as an AEP approval record.
- (c) A: an intake resume-context entity declared in `ess/intake/`; B: a commission id on
  `SliceRun` only; C: `resume` takes every `run` argument again and only the commission id is kept.

## Decision (2026-10-13)

(a) A: the store lives in `loom-governor`, beside `FileCaseStore`. (b) B: `b10x-loom approve`
writes a grant record into the state directory and `resume` reads it, so approval stays a separate
act from execution. (c) A: an intake resume-context entity in `ess/intake/`. The grant record and
the resume context are declared in the specification before any code.
