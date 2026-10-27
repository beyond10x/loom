---
format: aep.planning-md/3
id: approval-record:metaharness-0-20-pin-d71
kind: approval-record
status: approved
title: metaharness 0.20.0 pin and the Loom release (D71)
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-27T03:20:24Z", actor: "human:timo", revision: 2}
---
# Approval: metaharness 0.20.0 pin and the Loom release (D71)

## Decision

Session `b10x-chef-orchestrator`, 2026-10-27, decision D71, approved under the operator's grant
(class C: a dependency upgrade and a release through Loom's own process), on the operator's
message of 2026-10-27 01:31Z about the codex version check and the operator's rule "use cargo to
use a local reference ... and then when it's done, create the versions and PRs":

1. Loom: `b10x-loom-session-executor` pins metaharness 0.20.0, `tests/kinds.rs`'s H9 hold test is
   rewritten with a story id, and the dead H9 branch of the floor in `lib.rs` is removed; the
   story is filed. takt is patched onto this tree first, its harness parity tests are rewritten,
   and a takt `run --harness codex` with codex 0.164.0 must reach `ApprovalRequired` as before.
2. Then a Loom pull request through the bot, the merge and a release through Loom's process: a
   pin-only hop, no wave and no adversary; `verify-release.sh` exit 0 and the announcement file
   written. Loom `main` is checked first: if it carries unreleased work beyond this pin, stop and
   report instead of releasing it.
3. takt pins the new `loom-sdk` on its integration branch (fetch, merge, affected suites,
   fast-forward push, never force), its story implemented with the pin commit; no takt pull
   request, tag or release.

Loom's frozen wave 2 stays untouched. A build starts only with over 30G free on `/`; trees are
finished with `--discard-cache`.

## Checked

- Loom `main` was `8a28a410`, the tag `0.44.0`, with nothing after it: no unreleased work.
- The release moves the ESS pin to the newest ESS release, 0.70.0, as § Releases asks.
