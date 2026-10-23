---
format: aep.planning-md/3
id: approval-record:needs-authority-before-host-stop-option-b
kind: approval-record
status: draft
title: 'Stop order of story:needs-authority-before-host-stop: option B (Timo, 2026-10-23)'
relations:
- decides: story:needs-authority-before-host-stop
revision: 1
---
# The stop order of story:needs-authority-before-host-stop is option B

Timo answered the stop-order fork of `story:needs-authority-before-host-stop` (options A, B and C in
its § Decision needed before acceptance) on 2026-10-23 with "#4 B". Session
`b10x-chef-orchestrator` relayed it on 2026-10-24: "Timo on 2026-10-23 answered the stop-order fork
in story:needs-authority-before-host-stop with "#4 B": a stop before the executor reports
AwaitingApproval. That approves the change to AGENTS.md § Commission."

- Option B: a run whose open obligations only a grant moves ends before the executor with the
  existing `AwaitingApproval { actions }`; no new outcome variant.
- The change to the documented stop order in AGENTS.md § Commission (the host-stop rows) is
  approved with it.
- The story stays `draft` until it is scheduled; it still depends on `story:frontier-names-movers`.
