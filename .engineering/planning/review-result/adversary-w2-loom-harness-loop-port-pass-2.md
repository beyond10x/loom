---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-harness-loop-port-pass-2
kind: review-result
status: active
title: Wave 2026-10-06-w2 adversary, loom story:harness-loop-port, pass 2
relations:
- reviews: story:harness-loop-port
revision: 1
---
unit: story:harness-loop-port at 1d9c16b (worktree loom-w2-harness-loop-port, one new uncommitted test file)
verdict: red
cases: executed 624→627, red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: ~/.cache/loom-waves-2026-10-06/w2/harness-loop-port/adversary-2/{red-1,red-1b,red-2,red-3,suite,lint,lint2}.log; build output in ~/.cache/b10x-target/loom-w2-harness-loop-port (the assigned build dir)
needs-coordinator: yes

## 1. What I touched

`git --no-pager diff --stat` is empty because the file is untracked. `git status --short` shows one path, a test file:
```
?? crates/loom-executor/tests/adversary_w2p2_harness_loop_port.rs
```
No implementation file was touched. Nothing was committed or staged, and nothing was written under `.engineering/`.

## 2. Cases added (each run alone first, all red now)

The cases use a scripted in-process `ModelPort` and `FakeGovernor`; no socket and no network.

| # | test | asserts | red output (verbatim) |
|---|---|---|---|
| A | `adversary_w2p2_a_narrowing_is_not_widened_when_the_frontier_drops_the_granted_action` | A run limited by `with_admitted(Some([tests_run]))` never publishes or proposes anything else after the frontier blocks `tests.run`. | `request 2 publishes ["repository_inspect", "repository_edit", "repository_merge"] to a run granted only `tests_run`; the run ended ProposedAction(ExecutorOutcomeProposedAction { action: "repository.merge", arguments: ProposedActionArguments(Object([])) })` (`:98`, red-1b.log; red-1.log is the first run without the outcome in the message) |
| B | `adversary_w2p2_arguments_the_model_nests_too_deep_are_not_an_external_outage` | Model arguments nested 70 deep are not returned as `Suspended(ExternalAvailability)`. A precondition shows serde_json accepts them and Commission's reader refuses them. | `the model's own arguments were returned to Commission as an external outage: Suspended(ExecutorOutcomeSuspended { reason: ExternalAvailability(Object([("error", Text("the model's arguments cannot be carried as Commission's JSON: at byte 73: expected a document nested less than 64 deep"))])) })` (red-2.log) |
| C | `adversary_w2p2_two_runs_in_one_session_lose_no_completed_turn` | Every turn the provider completes is recorded once, with two runs in one session at the same time. | `the provider completed 2 turns in session `…b012` and 1 were recorded: [… index: 1, items: [...call_b...] …]; the runs ended [ProposedAction(… "tests.run" …), ProposedAction(… "tests.run" …)]` `left: 1 right: 2` (red-3.log) |

Each case was run with: `CARGO_TARGET_DIR=~/.cache/b10x-target/loom-w2-harness-loop-port CARGO_INCREMENTAL=0 cargo test -p b10x-loom-executor --locked --test adversary_w2p2_harness_loop_port -- --exact <name>`, exit 101 each time.

## 3. Package suite (run after the cases existed)

`cargo test -p b10x-loom-executor --locked --no-fail-fast`, exit 101: 627 executed, 3 failed (all three mine), 1 ignored.
- Only failing binary: `test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out`.
- The before count of 624 comes from the implementor's c1-gate.log at 1d9c16b.
- On the new file: `cargo fmt --check` exits 0, and `cargo clippy --test adversary_w2p2_harness_loop_port -- -D warnings` exits 0.

## 4. Findings

**A: a limit on the run's tools widens when the frontier changes** (`governed.rs:151-155`, doc at `:96-98`)
- **Measured:** `run_loop` clears four config fields but passes `admits` through unchanged. The ported limiting logic assumes the tool port's names never change (`AgentLoop::needs_routes`/`routes`, `turn_loop/mod.rs:4544-4580`). In this loop the port is each turn's catalogue. So once the allowed action leaves the frontier, the whole catalogue is published, and `repository.merge` was proposed.
- **Doc:** "Its tools are the catalogue's, whatever it says" is false in both directions.
- **What reaches it:** only the public `LoopConfig::with_admitted`. Nothing in the tree calls `run_loop` or `LoopExecutor`, and the `admits` field's own doc says it is `None` for every run a caller starts. So the verdict is INFEASIBLE, but severity is warning because reaching it would break the rule "Never silently broaden capability".
- **Fix (your call):** either apply the limit as a fixed intersection with each turn's catalogue, or refuse a governed run that sets `admits`. Simply clearing `admits` would silently drop what the caller asked for.

**B: a model's own arguments are reported as an external outage** (`governed.rs:513-521`, `:559`)
- **Measured:** `ModelArguments` returns an error, `propose` turns it into `Suspended(ExternalAvailability)`, and `Proposer::decide` stops the run on it.
- **What reaches it:** any model writing arguments nested 65 to 128 deep. The Responses wire decodes them with serde_json, which allows 128 levels (`tests/json_depth.rs`). I showed that leg with the precondition, not with an end-to-end wire run.
- **Why it matters:** the crate already treats this kind of outage misclassification as a defect (`nothing_admissible_is_not_an_external_outage`).
- **Fix:** deny the call to the model, which then chooses again, or return `NoUsefulAction`. Keep ExternalAvailability for a governor that cannot answer.

**C: two runs at once in one session lose a turn** (`governed.rs:251-255`, `:359`)
- **Measured:** `open_session` treats an existing session that is still Active with the same data as "continue". In `ess/domains/run.yaml`, `OpenSession` answers that case with `session-exists`, and only a Filed session can be resumed.
- `run_loop` also never files its session, although the spec says "A session is filed in every case".
- Both runs number their first turn 1 and record it under the same id. The second record replaces the first (`TurnStorage::put`), so `call_a`'s turn was lost.
- **What reaches it:** nothing I found. `LoopExecutor` runs one at a time behind its model mutex. Verdict INFEASIBLE, severity note.

## 5. What I attacked and could not break

| attack | result |
|---|---|
| F1/F2/F3 corrections | Hold: the pass-1 cases and `adversary2_seams_are_named_from_ported_crates` pass in the suite. |
| Paths to `ToolPort::call` | None: every tool spec requires approval, `Proposer` never approves, the Write envelope is never batched, no hooks are attached (code read). |
| Model influence on revision, frontier or authority | None: `ProposedAction` carries only the action and arguments, and the frontier is read for the commission's case (code read). |
| Acceptance item 2 in substance | Met: the call goes through `selection::select` and `RequestArguments` once each, and the round-trip's literal action and arguments would catch a selector or generator that ignored the call. |
| Turn recording with compaction and retries | Holds: compaction runs before the catalogue refresh (`mod.rs:2551-2567`), and failed attempts are not recorded. |
| Same frontier id issued again with new content | Not reachable with the real governor: frontier ids are content hashes (`loom-governor/src/lib.rs:835`). |

Not pursued: the Messages wire caps tool names at 128 bytes, while `tool_name` allows up to 256; no test in the unit covers the early return in `run_loop` (`:157`).

## 6. Paths written outside the worktree

- `~/.cache/loom-waves-2026-10-06/w2/harness-loop-port/adversary-2/` holding `red-1.log`, `red-1b.log`, `red-2.log`, `red-3.log`, `suite.log`, `lint.log`, `lint2.log`.
- A new test binary and clippy output in `~/.cache/b10x-target/loom-w2-harness-loop-port`.

```findings
[
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 151, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "run_loop passes LoopConfig::admits through to a limiting rule written for a fixed port, so once the frontier drops an allowed action every catalogue tool is published and repository.merge is proposed; no caller sets admits today."},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 513, "category": "judgement", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "Model arguments nested past Commission's 64-level JSON limit (serde_json on the wire allows 128) end the run as Suspended(ExternalAvailability) instead of being denied to the model."},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 251, "category": "concurrency", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "open_session continues an Active session where OpenSession answers session-exists, so two runs at once in one session both record turn 1 under one id and one completed turn is lost."}
]
```
