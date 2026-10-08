---
format: aep.planning-md/3
id: story:catalog-pipe-test-writer
kind: story
status: active
title: The catalog pipe test leaves its writer process behind
relations:
- serves: vision:O3
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T10:35:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-08T10:35:23Z", actor: "human:timo", revision: 3}
---
## Outcome

`a_catalog_pipe_with_a_writer_is_refused_naming_it` (`crates/loom-cli/tests/catalog_route.rs`) starts
`sh -c 'cat "$1" > "$2"'` as the writer of a named pipe and asserts that `b10x-loom run --catalog
<fifo>` refuses the path. The run refuses without opening the pipe, so the writer stays blocked in
its `open` and outlives the test, reparented to init: three such `sh` processes were found holding
two worktrees on 2026-10-08 after three local gate runs, and `worktree finish` refused both trees
until they were killed by PID.

## Acceptance

After `cargo test -p b10x-loom-cli --locked --test catalog_route`, no process started by the test
is alive: the test kills and reaps its writer (or opens the read end itself) on every path,
including a failed assertion.

## ESS first

Test-only change; no behaviour or ESS contract change.

## Scope

`crates/loom-cli/tests/catalog_route.rs` (cited, the test named above); `crates/loom-cli/tests/catalog_route_adversary.rs` and `crates/loom-cli/tests/evaluate.rs` (inferred, their FIFO cases if they start a writer).
