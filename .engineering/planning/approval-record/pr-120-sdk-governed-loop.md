---
format: aep.planning-md/3
id: approval-record:pr-120-sdk-governed-loop
kind: approval-record
status: draft
title: PR 120 (SDK governed-loop example) merged outside a wave with no operator turn
tags:
- non-interactive
revision: 3
---
# beyond10x/loom PR 120 merged outside a wave, accepted with no operator turn

PR 120 (https://github.com/beyond10x/loom/pull/120), branch `example/sdk-governed-loop`, head
`45dcfe4c`, adds `crates/loom-sdk/examples/governed_loop.rs`, its test
`crates/loom-sdk/tests/governed_loop_runs.rs` and a paragraph each in README.md, the `loom_sdk`
crate documentation and CHANGELOG.md. It went out as a bot pull request outside a wave and has no
story.

Session `b10x-chef-orchestrator` approved the merge under the grant in `~/beyond10x/CLAUDE.md`
(2026-10-23): "Go for PR 120, class C under the grant in ~/beyond10x/CLAUDE.md. Record it in your
store. [...] The condition is that `loom / task check` passes on 45dcfe4c and main is still
6437fc52. If both hold, fast-forward main to 45dcfe4c as the bot."

- Both conditions held: `loom / task check`, `loom / macos build`, `Build documentation`,
  `common / Security and privacy` and `b10x-gates / local evidence` completed success on
  `45dcfe4c`, and `main` was `6437fc52`.
- `main` was fast-forwarded to `45dcfe4c` by `b10x-bot[bot]` (author and committer bot); GitHub
  records the PR merged at 2026-10-23T08:39:14Z.
- Wave 2026-10-23-w3 merges `main` in before it closes, so release 0.38.0 carries the example.
