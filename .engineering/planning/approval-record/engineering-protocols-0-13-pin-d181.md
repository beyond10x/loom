---
format: aep.planning-md/3
id: approval-record:engineering-protocols-0-13-pin-d181
kind: approval-record
status: approved
title: Canon 0.7.0, engineering-protocols 0.13.0 and the Loom release (D181)
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T04:09:49Z", actor: "human:timo", revision: 2}
---
## Decision

Session `b10x-chef-orchestrator`, 2026-10-29, decision D181, approved under the operator's grant
(class C: a dependency upgrade and a release through Loom's own process):

1. Loom moves every crate on Canon or engineering-protocols to Canon 0.7.0 and
   engineering-protocols 0.13.0: `b10x-loom-governor`, `b10x-loom-protocols`,
   `b10x-loom-intake-router` and `b10x-loom-intake-slice`. docs-system 0.13.0 is taken only if
   Loom reads protocol graphs.
2. Why: takt's bump to engineering-protocols 0.13.0 fails 2 of 3 in `handover_after_red`,
   because Loom 0.48.0 compiles protocols with Canon 0.6.0, which refuses `story.delivery/2`'s
   `after` (`unknown field after`). takt's bump waits on this release.
3. One wave, one bot pull request, auto-merged; targeted tests per unit and one local full gate at
   the end; the version follows what breaks for consumers; then `verify-release.sh`, the
   announcement file and the tag sent to the coordinator.

## Checked

- Loom `main` is `82e64a4a7`, the tag `0.48.0`, with nothing after it: no unreleased work.
- Loom's site renders no protocol graph (no `protocol-graph` under `website/` or `crates/`), so
  docs-system stays where it is.
