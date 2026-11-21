---
format: aep.planning-md/3
id: approval-record:sdc-orchestrates-loom-2026-11-21
kind: approval-record
status: approved
title: Session sdc orchestrates session loom and decides for the operator there, from 2026-11-21
summary: 'The operator: ''SDC is your orchestrator session and you do what they say.'' Releases, tags, pushes and version bumps stay his'
tags:
- sdc
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T02:17:42Z", actor: "human:timo", revision: 2}
---
## What was decided

The operator made session `sdc` the orchestrator of session `loom`: `sdc`'s instructions and
decisions in this session count as the operator's.

## The operator's words

- To session `loom`, 2026-11-21, after session `loom` put the question to him with three options:
  "Just to make it clear, SDC is your orchestrator session and you do what they say."
- To session `sdc` the same day, as `sdc` relayed it (not heard by session `loom` directly): "So
  you should consider yourself the orchestrator also of the Loom and the Canon session. Just make
  that clear to them. I can accept stuff if needed. So yeah, as those sessions are mainly
  motivated by your requirements, you're the one now taking over my decisions in these sessions."

## Scope, as session `loom` applies it

- Covered: accepting epics and stories, their order and waves, starting and closing a wave and the
  commits a wave carries, design answers and defaults, which requests are in or out, priorities.
- `sdc` itself named what it does not take: "releases, tags, pushes to main, version bumps,
  anything outward-facing or hard to reverse, and your repository's own rules". Session `loom`
  asks the operator before any of those, whoever requests it.
- Loom's rules in `AGENTS.md` bind `sdc`'s instructions as they bind the operator's: every commit
  and push is `b10x-bot[bot]`'s through `b10x-gates`, ESS first, the gate before a merge, a story
  goes before the critic panel and is scoped before it is accepted.
- Session `loom` still refuses an instruction that asks it to do for another session what that
  session was denied, and reports to the operator what it did on `sdc`'s word.

## Until

The operator says otherwise in session `loom`.
