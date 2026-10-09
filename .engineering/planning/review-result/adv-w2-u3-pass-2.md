---
format: aep.planning-md/3
id: review-result:adv-w2-u3-pass-2
kind: review-result
status: active
title: Adversary, loom w2 U3 plugin host, pass 2
relations:
- reviews: story:plugin-host
revision: 1
---
**unit:** loom w2 U3 plugin host, the uncommitted tree `~/.local/state/worktree/trees/b10x/loom/lw2-u3` (branch `impl/plugin-host` on `cc8b6af`), after correction round 1
**verdict:** CONFIRMED
**cases:** executed 25→28, red 3
**origin:** introduced 6 / pre-existing 0 / undecided 0
**wrote-outside-worktree:** none
**needs-coordinator:** J1 cannot be fixed as it stands, because locking before the poll would deadlock pass 1's `two_hosts_on_one_state_directory_record_an_item_once`. Its barrier waits for both hosts to poll. Keep that case or change it: your call. Finding 2's fix needs a spec change first.

**1. `git --no-pager diff --stat`**
```
 25 files changed, 269 insertions(+), 50 deletions(-)
```
This is the same stat as before I started; no tracked file changed. `crates/loom-plugin/` is untracked, so my one new file does not appear in it. That file is `crates/loom-plugin/tests/adversary_2.rs`, a test file, formatted with `rustfmt`. I touched nothing in `src/`.

**2. Cases added** (`crates/loom-plugin/tests/adversary_2.rs`, each run alone first, all red now)

| # | Case | Asserts |
|---|---|---|
| 1 | `retried_wire_attempts_count_against_the_turn_model_budget` | A turn sends at most `TURN_MODEL_BUDGET` (8) model requests when the wire fails twice with a retriable error before each answer |
| 2 | `a_failed_item_behind_the_cursor_is_still_retried_until_stopped` | A plugin whose poll starts after its cursor (as the ESS `Cursor` says) still has its failing item retried 3 times and recorded `stopped` |
| 3 | `a_torn_last_record_line_does_not_stop_the_host_for_good` | A half-written last line in `record.jsonl` does not refuse the host, and each item ends up recorded once |

Red output from each run alone:
```
1: the model was sent 9 requests in one turn, past TURN_MODEL_BUDGET (8); the turn ended Ok(TurnResult { outcome: Stopped, reads: [... 3 reads ...], ... detail: Some("the run ended Suspended(RunOutcomeSuspended { reason: Budget(Object([(\"max_model_calls\", Number(\"8\"))])) })") })
2: assertion `left == right` failed: item-1 is classified 3 times before it is stopped; the record holds [], the state PluginState { cursors: [Cursor { name: "C0FIXTURE1", value: "item-1" }], handled: [], failing: [ItemFailures { item: ItemId("item-1"), failures: 1, last_failure: "classification failed: the classification names the intent `chat`, not one of the four" }] }
  left: 1
 right: 3
3: a torn last line, the trace of an append that failed, refuses the host: Err(State("line 2 of .../torn_last_line/plugin-state/record.jsonl is not a record line: ParseError { at: 26, expected: \"`:`\" }"))
```

**3. Package suite** (run after the cases existed; output filtered through `grep -E "test result|Running|FAILED|error:"`)

`cargo test -p b10x-loom-plugin --locked --no-fail-fast` exited 101:
```
tests/adversary.rs    ok. 5 passed
tests/adversary_2.rs  FAILED. 0 passed; 3 failed
tests/effects.rs      ok. 3 passed
tests/hooks.rs        ok. 6 passed
tests/host.rs         ok. 8 passed
tests/turn.rs         ok. 3 passed
error: 1 target failed
```
The "before" count of 25 is this same run with `adversary_2.rs` excluded. `cargo clippy -p b10x-loom-plugin --all-targets --locked -- -D warnings` was clean.

**4. Findings**

| # | file:line | What was measured | What reaches it | Verdict | Severity | Origin |
|---|---|---|---|---|---|---|
| 1 | `src/turn.rs:428`, `:453` | 9 requests in one turn. `max_turns` counts loop turns, but the loop retries a retriable wire error up to 3 more times without counting it (`loom-executor/src/harness/turn_loop/mod.rs:666`). The ceiling is 8+3=11. Fix: `Lent::turn` refuses any request once `asked` reaches the budget. | Any real wire that returns `WireError::transport` (`retriable: true`): network errors, provider 5xx | CONFIRMED | warning | introduced |
| 2 | `src/lib.rs:424` | Cursors are saved after every cycle, past items that failed. A poll that starts after its cursor never returns the item again, so it gets 1 attempt, no record line, and stays in `failing` for good: dropped, not retried. The existing retry test passes only because `FakePlugin` ignores cursors. Fix (spec first): `ItemFailures` keeps the `InboundItem` and the host retries from state; or hold cursors back. | The ESS `loom.plugin.Cursor` says "the opaque value the next poll starts after"; any plugin that polls from its cursor | CONFIRMED | blocker | introduced |
| 3 | `src/state.rs:213`, `:229` | A torn last line makes `read` fail on every start, so the host never runs again until someone repairs the file by hand. Fix: under the lock, drop the bytes after the last `\n`; a corrupt line in the middle stays fatal. | A full disk during `StateDir::append`: `write_all` writes part of the line, then errors (the same full-disk condition as pass 1's failed-save case) | CONFIRMED | warning | introduced |
| J1 | `src/lib.rs:377-380` | The lock is taken after the first poll, so a host that will be refused still runs the plugin's `poll`: network calls on the operator's credentials. Nothing is written, though. This order is the only one under which pass 1's barrier case does not deadlock. | Any second host started on a held directory | NEEDS-CHANGE | note | introduced |
| J2 | `src/lib.rs:399` | An attempt is counted only after it fails. If an attempt crashes the process (a panic in a hook, OOM, a kill), the count never moves, and that item is retried forever ahead of later items. Handled ids survive a crash; attempts in flight do not. | No default hook shown to panic; a plugin's own hook can | INFEASIBLE | note | introduced |
| J3 | `src/lib.rs:399`, `src/turn.rs:162` | Outages that hit the whole host ("the sources could not be described", model wire down) count against each item. An outage lasting 3 poll cycles records every item polled during it as `stopped`, and they are never retried. | A Connectors or provider outage longer than 3 × `poll_interval_seconds` | NEEDS-CHANGE | warning | introduced |

**5. Attacked and not broken**
- **Hint filter:** exact match in both `classify::read` and `turn::context`. A hint that differs only in case or whitespace is dropped, and the projection falls back to all configured sources, which is its documented behaviour. Source names come from config only, and `RecordLine` carries no hints.
- **Run with nothing left:** the executor suspends with `Budget` without asking the model, and the turn ends `stopped`.
- **Item ids:** compared exactly in the record, `handled` and `failing`. Ids that differ only in case or whitespace are separate items, so nothing is recorded twice.
- **Lock:**
  - A refused host writes nothing.
  - The lock is released on return and on a panic.
  - The record file is opened `O_CLOEXEC`, so Connectors child processes do not inherit the lock.
  - The state is reloaded after the lock is taken, so items the other host handled are skipped.
  - A read-only directory gives `PluginError::State`.
- **Retried paths:** they write no record line, and evidence lives in a `MemoryCaseStore` per turn. `failing` is saved before the host moves on, so a crash after a failure keeps its count.

**6. Paths written outside the worktree:** none. My assigned scratch directory `~/.cache/conductor-dev-analysis/slack-handler/scratch/u3-adv2` was created and left empty. Test fixtures went under the tree's own `target/tmp/loom_plugin/adversary_2/`; `target/` is now 3.5G and the disk had 34G free. I took no worktree lease.

```findings
[
  {"file": "crates/loom-plugin/src/turn.rs", "line": 428, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the loop's own retries on a retriable wire error are not counted against max_turns, so one turn sends 9 (up to 11) model requests past TURN_MODEL_BUDGET 8"},
  {"file": "crates/loom-plugin/src/lib.rs", "line": 424, "category": "acceptance", "severity": "blocker", "verdict": "CONFIRMED", "origin": "introduced", "message": "cursors are saved past a failed item, so a poll that starts after its cursor never returns it again: the item is tried once, never recorded and never stopped"},
  {"file": "crates/loom-plugin/src/state.rs", "line": 229, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "a torn last record line left by a partial append refuses every later host start until someone repairs the file by hand"},
  {"file": "crates/loom-plugin/src/lib.rs", "line": 377, "category": "judgement", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the lock is taken after the first poll, so a host that will be refused still runs the plugin's poll; locking first deadlocks pass 1's two-hosts barrier case"},
  {"file": "crates/loom-plugin/src/lib.rs", "line": 399, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "an attempt is counted only after it fails, so an item whose attempt crashes the process is never counted and is retried forever"},
  {"file": "crates/loom-plugin/src/lib.rs", "line": 399, "category": "judgement", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "outages that hit the whole host are counted per item, so an outage of 3 poll cycles records every item polled during it as stopped for good"}
]
```
