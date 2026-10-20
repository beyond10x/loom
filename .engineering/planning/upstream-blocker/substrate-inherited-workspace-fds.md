---
format: aep.planning-md/3
id: upstream-blocker:substrate-inherited-workspace-fds
kind: upstream-blocker
status: cleared
title: Substrate leaves its workspace descriptors open in a confined process
relations:
- blocks: story:child-writable-workspace
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-10-20T13:38:59Z", actor: "human:timo", revision: 2}
---
## Waiting on

A Substrate release that does not leave its pinned workspace descriptors open in a confined
process. Measured on Loom `9bec6e6` (`review-result:adv-w20261020w2-u6-pass-1`): a child confined
through `SubstrateRunner::run_child`, and a test through `TestRunner::run`, wrote the read-only
workspace root and wrote and read beside the workspace through descriptors 10, 12 and 13, which it
inherits at `/proc/self/fd`. The adversary's reading of the cause: `substrate-host`
`process.rs:1994-2002` clears close-on-exec on them (`:865-875`) and passes them to bwrap as
`/proc/self/fd/N` paths; bwrap 0.12.0 leaves a descriptor passed that way open, and closes one
passed with `--ro-bind-fd` / `--bind-fd`. Whether released Loom 0.27.0 reproduces it is being
measured.

## Clears when

1. A Substrate tag passes the workspace descriptors so the confined process cannot reach them;
2. Loom's pin (`crates/loom-intake-slice/Cargo.toml:31-32`) moves to it;
3. `adv_a_child_writes_nothing_outside_its_named_path_through_a_descriptor_it_holds` and
   `adv_a_test_writes_nothing_outside_target_through_a_descriptor_it_holds`
   (`crates/loom-intake-slice/tests/adversary_w20261020w2_u6.rs`) pass on Loom's main.

## Blocks

story:child-writable-workspace, and every claim that a confined child or test writes only its
writable paths (AGENTS.md, CHANGELOG, `website/docs/reference/sdk-stability.md`).
