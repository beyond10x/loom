---
format: aep.planning-md/3
id: story:tree-test-command
kind: story
status: draft
title: tree test kills a cycle at its timeout, prints the ten slowest and lists cycles
summary: --cycle-timeout with a per-cycle cancel handle and CycleTimedOut; slowest line after the summary; --list reads the suite only; decide and requests now wait on story:tree-test-decide-and-requests (D123)
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:suite-runs-cycles-in-parallel
- depends_on: story:cancel-a-running-bound-program
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 9
---
## Outcome

`b10x-loom tree test` gets the cycle kill (`--cycle-timeout`), the ten-slowest line and
`--list`. These are the pieces `story:tree-suite-format` left to this story (`tree-suite-format.md:
63`, `:422-423`) beyond the workers that `story:suite-runs-cycles-in-parallel` took. The `decide`
step and entry and the `requests` source move to `story:tree-test-decide-and-requests` (decision
D123).

This story's id is the one the code already names: `crates/loom-tree/src/suite/availability.rs:57`
(`TEST_COMMAND`), with the entries `:77`, `:80` and `:95`.

```
b10x-loom tree test --suite <file> [--list] [--cycle-timeout <seconds>] …
```

- **The cycle kill.**
  - `--cycle-timeout <seconds>` takes 1 to 60, default 60 (`CYCLE_WALL_SECONDS`,
    `crates/loom-tree/src/suite/run.rs:317`).
  - A value of 0 or above 60 is refused before the suite is read, with exit 2
    (`TREE_TEST_REFUSED`). The refusal is printed after the existing prefix `b10x-loom: tree test
    refused:` (`crates/loom-cli/src/tree.rs:446`) and names the flag and the bound.
  - The bound is 60 because a `run` step's `max_wall_seconds` derives from `CYCLE_WALL_SECONDS`
    (`suite/run.rs:1061-1066`), and a cycle may not outlive the 60 s test budget.
  - **Why a watcher.** A worker is blocked inside `run_tree` for as long as a step runs.
    `run_tree` reads its caller's flag every 100 ms (`CANCEL_WATCH`,
    `crates/loom-tree/src/run.rs:128-165`) but checks `max_wall_seconds` only before a step. So
    nothing compares a cycle's wall time with its timeout while a program runs.
  - **The cycle's own flag.** Each running cycle gets an `AtomicBool` of its own. It is what the
    worker passes to `run_tree` in place of the suite's flag (today `self.cancel`,
    `suite/run.rs:1071`) and what it checks before each step, beside the suite's flag
    (`:797-805`).
  - **One watcher thread per suite run**, not one per cycle.
    - It holds a deadline (start instant plus `--cycle-timeout`) for each running cycle.
    - Before `run_tree`, the worker hands the watcher its cycle's flag and start instant, and it
      takes them back after `run_tree` returns.
    - Every 100 ms the watcher sets the flag of each cycle whose deadline has passed. When the
      suite's flag is set, it sets the flag of every running cycle.
    - It ends when the last worker has ended.
  - **The thread cost.** The run adds one thread, the watcher, to its `workers` threads. Each
    `run_tree` given a flag already starts its own bridge thread for the call
    (`crates/loom-tree/src/run.rs:146-165`), today too. That cost is unchanged, at one per
    running cycle at most.
  - **How a timed-out cycle ends.**
    - The set flag ends the running `run_tree` `Cancelled` within 10 s, by killing its program
      (`story:cancel-a-running-bound-program`, merged at `fea70cb1`).
    - The cycle then ends with the difference `CycleTimedOut` naming that step: `check` is the
      step's name or `steps[<i>]`, `kind` is the step's kind, and `found` is the seconds the
      cycle ran.
    - Otherwise the watcher only sets the flag, and the cycle ends at the next point where the
      flag is read.
  - The other cycles go on. Today only the `tree run` rounds share the 60 s
    (`suite/run.rs:1061-1070`); a step that is not a `run`, or a program whose binding's timeout
    is longer, is not bounded.
- **The ten slowest.** After the two summary lines comes `slowest: <name> (<s> s), …`: up to ten
  cycles that ran, slowest first, ties in suite order. `SuiteReport` gains no member; the line is
  read from `seconds`.
- **`--list`.**
  - It prints one line per cycle of the suite, in suite order: `<name>`, `<name>: pending`,
    `<name>: pending, waits for story:<id> (<kind>)`.
  - `--only` applies to it.
  - It reads the suite and nothing else: no `when` is evaluated and nothing is written.
    `--protocols`, `--scratch` and `--at` are not needed with it.
  - It exits 0, or 2 when the suite is refused.
- **Availability.**
  - `SUITE_AVAILABILITY` no longer names this story.
  - The step `decide` (`availability.rs:77`), the entry `decide` (`:80`) and the source `requests`
    (`:95`) wait on `story:tree-test-decide-and-requests`, through a new constant beside
    `TEST_COMMAND` (`:57`), which goes.
  - A cycle that uses them stays pending, now printed `waits for
    story:tree-test-decide-and-requests`.

## ESS first

`loom.tree.DifferenceReason` (`ess/domains/tree.yaml:933-954`) gains `CycleTimedOut`, and its
comment names it.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

All tests go in `crates/loom-cli/tests/tree_test/mod.rs`, with fixtures below
`crates/loom-cli/tests/fixtures/tree-suite`. Each asserts its own time under the module's 5 s
(`within_bound`, `:183-186`).

1. `a_cycle_over_its_timeout_is_killed`: `tree test --confinement local --cycle-timeout 2
   --workers 2`. (The default `substrate` refuses more than one worker until
   `story:suite-runs-cycles-under-one-cgroup-root`.)
   - Cycle 1's program writes its own pid (`$$`) to `pid.txt` in its workspace, then sleeps 30 s.
     Cycle 2's program ends in 0.3 s.
   - Both cycles have the default steps `[open, run]`.
   - The watcher sets cycle 1's flag at its 2 s deadline while its worker is blocked in
     `run_tree`.
   - Cycle 1 ends within 5 s of the start of the command, with the difference `CycleTimedOut`
     naming `steps[1]`, kind `run`, found at least 2 s.
   - The test reads `pid.txt` and checks that `kill -0 <pid>` fails afterwards, so no process of
     the program remains.
   - Cycle 2 ends as expected, and the exit is 1.
2. `the_ten_slowest_follow_the_summary`: the module's shared fixture run (`ran`, `:130`) prints a
   `slowest:` line after the two summary lines. It names at most ten cycles, slowest first, each
   with its seconds.
3. `list_prints_the_cycles_and_runs_nothing`: `tree test --list --suite <five cycles, one
   waiting>` prints five lines in suite order, the `waits for` line included, and exits 0.
   `--scratch` is not given and nothing is written. With `--only b`, it prints one line.
4. `a_refused_suite_is_refused_by_list`: `--list` on a suite with an unknown step kind exits 2
   with `StepKindUnknown`.
5. `a_cycle_timeout_above_60_is_refused`.
   - `--cycle-timeout 61` and `--cycle-timeout 0` each exit 2. Stderr starts `b10x-loom: tree
     test refused:` and names `--cycle-timeout`, so neither is clap's usage error: clap takes
     any `u64`, and the command's own code checks both bounds. `--scratch` stays absent.
   - `--cycle-timeout 60 --list` on the same suite only decodes and runs no cycle. It exits 0.
6. `nothing_waits_for_this_story`.
   - `SUITE_AVAILABILITY` holds no `Waits("story:tree-test-command")`.
   - The step `decide`, the entry `decide` and the source `requests` each hold
     `Waits("story:tree-test-decide-and-requests")`.

## Out of scope

- The workers and `--only` (`story:suite-runs-cycles-in-parallel`).
- Substrate with workers (`story:suite-runs-cycles-under-one-cgroup-root`).
- The `decide` step and entry and the `requests` source (`story:tree-test-decide-and-requests`).

## Scope

Derived 2026-11-22 from the plan tree at `5bcc26d9`. The cycle-kill cites were read again at
`fea70cb1`. **Cited** means read at the line named.

- `crates/loom-tree/src/suite/availability.rs` (`:57`, `:77`, `:80`, `:95`) — cited.
- `crates/loom-tree/src/suite/run.rs`: the watcher, the cycle's own flag, the check before each
  step (`:797-805`) and the `run_tree` call (`:1061-1074`); `CYCLE_WALL_SECONDS` (`:317`) —
  cited.
- `crates/loom-tree/src/run.rs`: `CANCEL_WATCH` and `run_tree`'s bridge thread (`:128-165`),
  read and not changed — cited.
- `crates/loom-cli/src/lib.rs` (`TreeTestArgs`, `:315-338`; `TREE_TEST_HELP`, `:296-307`) and
  `crates/loom-cli/src/tree.rs` (`test`, `:444-509`) — cited.
- `ess/domains/tree.yaml` (`:933-954`), `generated/rust/loom`, `generated/ess/loom` — cited.
- `crates/loom-cli/tests/tree_test/mod.rs` and `crates/loom-cli/tests/fixtures/tree-suite` —
  cited.
- `website/docs/guides/run-a-case-tree.md`, `website/docs/reference/cli.md`, `CHANGELOG.md` —
  cited.

## Edges

- `depends_on story:suite-runs-cycles-in-parallel`: the workers inside which a cycle is killed.
- `depends_on story:cancel-a-running-bound-program`: the cancel handle that kills a cycle's
  program.
- **Order:** after `story:suite-cycle-writes-a-workspace-file` and
  `story:suite-runs-cycles-under-one-cgroup-root`. It merges both heads. This is stated in prose,
  with no depends_on.
- **Would collide with**, no edge:
  - `story:suite-cycle-writes-a-workspace-file` (draft): `suite/availability.rs`,
    `suite/run.rs`, the `DifferenceReason` block of `tree.yaml`, the test modules and the guide.
  - `story:suite-runs-cycles-under-one-cgroup-root` (draft): `suite/run.rs`, `tree_test/mod.rs`,
    the fixtures and the guide.
  - `story:tree-test-decide-and-requests` (draft) depends on this story and merges after it.
