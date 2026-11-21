---
format: aep.planning-md/3
id: story:capacity-given-to-an-admission-program
kind: story
status: proposed
title: 'A program bound with capacity: true reads the run slots, the engine''s ceilings, the tree''s budget and its usage per run and since tree open on standard input'
summary: loom.capacity added after revalidation; slots at the step's start; recorded on the action started line; not given to arguments_from; Loom decides nothing from it
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:concurrent-cases-in-the-tree-daemon
- depends_on: story:standing-tree-wakes-at-a-host-instant
scope:
- confidence: cited
  path: crates/loom-bindings/src
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: inferred
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-bindings/tests/capacity_member.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/admit-from.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/admit-gated.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/admit-plain.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/admit.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/admit.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/copy-stdin
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/copy-stdin-err
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/engine-limits-none.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/engine-limits.yaml
- confidence: cited
  path: crates/loom-cli/tests/tree_capacity.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:08Z", actor: "agent:sdc", revision: 12, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

A `program` executor in a `loom.bindings/1` document may say `capacity: true`. The member is
`Optional<Bool>`: `false` means the same as no member and is accepted on every executor; `true` on
an `agent`, `person`, `open` or `copy` executor is refused at load `MemberNotForExecutor` naming
the action and `executor.capacity`; a value that is not a boolean is refused `Malformed`, the
decoder's reason for a member of the wrong type. With `capacity: true` the program's standard
input, the arguments object (`story:bound-program-result-table`), also holds the top-level member
`loom.capacity`, a `loom.tree.Capacity` taken when the step starts:

- `served` (whether a daemon runs the tree), `run_slots`, `running` (cases of every served tree
  holding a slot, this step's included), `queued` (runnable cases waiting for a slot while none is
  free), `blocked` (runnable cases waiting only for their workspace while a slot is free,
  `story:concurrent-cases-in-the-tree-daemon`) and `free` (`run_slots` minus `running`); `queued`
  is 0 whenever `free` is above 0. A direct `tree run` gives `served: false`, one slot, one
  running, none queued or blocked, none free. The slot figures are those of one instant, the
  step's start; a slot freed or taken a moment later is not in them. Because a case keeps its slot
  across consecutive steps while it stays runnable, a step that follows another step of its case
  counts that case's own slot as running and no queued case can have taken it in between;
- `ceilings`: the engine configuration's `limits` section as loaded
  (`story:action-limits-enforced`: per action the ceiling and default of memory bytes, CPU
  milliseconds, processes and `timeout_seconds`; per run `wall_seconds` and `cpu_millis`), absent
  when the engine has none;
- `budget`: the tree's `TreeBudget` in force, its model budget included (`--max-agent-calls`,
  `--max-model-cost-micro-usd`, `story:tree-usage-and-model-budget`), as the tree keeps it
  (`story:standing-tree-wakes-at-a-host-instant`); once
  `story:budget-and-limit-stops-raise-decision-requests` lands, a member a `budget extended` line
  extended is in force at its extended value;
- `used`: the tree's `TreeUsage` for the current run, in the windows of
  `story:tree-usage-and-model-budget`, and beside it `since_open`, the same figures since `tree
  open` (for a standing tree the run starts at its latest `woken` line that starts a run,
  `new_run: true`, `story:standing-tree-wakes-at-a-host-instant`; session `sdc`, 2026-11-21: "the
  capacity member given to an admission program carries both, so an admission rule can bound a
  standing tree's total spend").

How the figures reach the program: `crates/loom-bindings` declares a capacity port, a trait the
program port calls at the step's start for the step's case, and `crates/loom-tree` implements it
from its scheduler and counters (a direct `tree run` answers with its one slot). The program port
lives in `crates/loom-bindings/src`; no story names its module. The member is added after
revalidation, by the program port, so the request Commission revalidates and the arguments a
protocol's schema checks do not hold it. `loom.capacity` is Loom's: literal `arguments` holding a
top-level member `loom.capacity` are refused at load `ReservedArgument`, and a request whose
arguments hold one is refused by the program port `ReservedArgument` before the program starts.
The step's `action started` line in the run log carries the capacity it was given, so what an
admission read is recorded. An `arguments_from` program is not given it. Loom decides nothing from
it: the program does.

## Why

"story count ≤ free run slots: the engine's configured ceilings (... ceilings in engine config, a
binding may only lower them) and free build slots (`slot status`), and the remaining model budget"
(`~/beyond10x/sdc/docs/orchestration.md` § 2.4, rule 3); blocker `loom-run-ceilings`: "the
configured run ceilings (a binding may only lower them) and the free run slots are readable by a
program, so `wave.admit` rule 3 can count them"; session `sdc`, 2026-11-21, change 6: the capacity
member carries "the tree's model budget and what is used (calls and cost, per tree; tokens if you
have them)", and change 5: both wall figures. The ceilings live in the engine configuration
(`story:action-limits-enforced` § Outcome: "The engine configuration's `limits` section holds, per
action, a ceiling and a default") and the slots in the daemon (`story:tree-daemon` § Outcome); a
bound program runs confined, with "its arguments as JSON on standard input and a cleared
environment" (`story:bound-program-result-table` § Outcome), so it can reach neither the daemon's
socket nor the engine file. Standard input is the one channel it has. Free build slots of `slot`
are the host's. A new executor member enters the decoder's `EXECUTOR` list
(`crates/loom-bindings/src/decode.rs:27`) and the loader's `members` table in `fit`
(`crates/loom-bindings/src/load.rs:228-244`), whose refusal is `MemberNotForExecutor`; a member of
the wrong type is refused `Malformed` in `executor` (`decode.rs:297-350`).

## Waits on

- `story:concurrent-cases-in-the-tree-daemon`: slots counted per running case and kept across a
  case's consecutive steps, the `Queued` and `Blocked` states, which `running`, `queued`,
  `blocked` and `free` report; the story before this one on `ess/domains/tree.yaml`,
  `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml` (`action started`) and the scheduler in
  `crates/loom-tree`.
- `story:standing-tree-wakes-at-a-host-instant`: the run's starting line, `since_open` and the kept
  `TreeBudget`, which `used` and `budget` read; `pulse/1`, `bin/gate` and `bin/sleep1`.
- Through them, `story:tree-usage-and-model-budget` (`TreeUsage`, the model budget),
  `story:child-case-own-workspace` (the latest story on the program port),
  `story:run-caps-per-arrival` (the earlier story on `crates/loom-bindings/src/decode.rs` and
  `crates/loom-bindings/src/load.rs`), `story:tree-daemon` and `story:action-limits-enforced`
  (`LimitsSection` in `ess/domains/engine.yaml`).

Comes before, on shared files: `story:flow-limits-recorded-as-events` (`Capacity` in
`ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`).

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists them): `story:budget-and-limit-stops-raise-decision-requests`
(`TreeBudget`, which `Capacity.budget` carries; `ess/domains/runlog.yaml`, `crates/loom-tree`),
`story:non-independent-review-becomes-decision-request`,
`story:run-log-references-exec-file-changes`, `story:authority-records-bound-and-grounded-in-canon`,
`story:requests-from-canon-pending-decisions`,
`story:bindings-child-path-refusal-names-the-bindings-directory` and
`story:agent-session-through-substrate`. No edge is added for them.

## ESS first

`ess/domains/tree.yaml`: `Capacity` (`served`, `run_slots`, `running`, `queued`, `blocked`,
`free`, optional `ceilings` as `loom.engine.LimitsSection`, `budget` as `TreeBudget`, `used` as
`TreeUsage` with `since_open`). `ess/domains/bindings.yaml`: `ExecutorSpec` gains `capacity`
(`Optional<Bool>`, `true` for `Program` only), `BindingsRefusalReason` gains `ReservedArgument`,
and the program port's refusals gain `ReservedArgument`. `ess/domains/runlog.yaml`: `action
started` gains optional `capacity`. On that commit `task drift` fails and
`crates/loom-cli/tests/tree_capacity.rs` does not compile.

## Acceptance

Files added to `crates/loom-cli/tests/fixtures/area/`:

- `admit.protocol.yaml`, `admit/1`: a placed artifact `intake`; two observations, `intake.prep`
  (may produce `prep_done` about `intake`) and `wave.admit` (may produce `admitted` about
  `intake`, admissible when a `prep_done` about the current `intake` is held); no obligation, no
  outcome. Both are non-`write`, so the selector takes `intake.prep` then `wave.admit` (the sort
  key at `crates/loom-executor/src/selection.rs:331`, the least `(writes, id)` at `:336-338`);
  each runs once per revision of `intake`.
- `admit.bindings.yaml`: `intake.prep` bound to `bin/sleep1`; `wave.admit` bound to
  `bin/copy-stdin` (prints its standard input to standard output, exit 0, `results: {"0": ok}`)
  with literal `arguments: {"wave": 1}` and `capacity: true`. The test reads what the admission
  received from the blob its observation names (`story:bound-program-result-table`:
  `blobs/<sha256>` in the store directory). `admit-gated.bindings.yaml`: the same with
  `intake.prep` bound to `bin/gate`. `admit-from.bindings.yaml`: `admit.bindings.yaml` with
  `wave.admit` given an `arguments_from` program `bin/copy-stdin-err` in place of the literal
  arguments, which writes its own standard input to standard error and prints `{"wave": 1}`, so
  the blob of the `arguments_from` run is the test's record of it. `admit-plain.bindings.yaml`:
  `admit.bindings.yaml` without `capacity`. `admit-reserved.bindings.yaml`: `admit.bindings.yaml`
  with literal `arguments: {"wave": 1, "loom.capacity": 1}`.
- `engine-limits.yaml`: adapter `daemon`, two run slots, a `limits` section with per action a
  memory ceiling of 512 MiB, a CPU ceiling of 60000 ms, 64 processes and `timeout_seconds` 120,
  and per run `wall_seconds` 600 and `cpu_millis` 600000. `engine-limits-none.yaml`: the same
  with no `limits` section.

`crates/loom-cli/tests/tree_capacity.rs` drives `b10x-loom tree` with stub programs and a managed
Substrate daemon, no model; each bullet names its engine file and ends as it says:

- Direct `tree run --at T --max-agent-calls 5 --max-model-cost-micro-usd 9000` of `admit/1` under
  `engine-limits.yaml` (bounded): the admission's copied object holds `wave: 1` and
  `loom.capacity` with `served: false`, `run_slots: 1`, `running: 1`, `queued: 0`, `blocked: 0`,
  `free: 0`, the `limits` section of `engine-limits.yaml` as `ceilings`, and `budget` with
  `max_agent_calls` 5 and `max_model_cost_micro_usd` 9000; `used.steps` is 1;
  `used.wall_millis_sum` equals the wall time of `intake.prep`'s `action finished` line and
  `used.wall_millis_elapsed` the span from its `action started` to its `action finished` instant,
  both read with the run log reader of `b10x-loom-runlog`; the run then stops `Stuck`, exit 4.
- Through the daemon under `engine-limits.yaml` (two slots), three trees each opened over its own
  workspace directory, in this order: tree `A` of `pulse/1` with `pulse-gated.bindings.yaml` is
  submitted with `tree run --detach` and its gate held (slot 1); the admission tree `B` of
  `admit/1` with `admit-gated.bindings.yaml` is submitted and its `intake.prep` gate held (slot
  2); tree `C`, the same as `A`, is submitted and shows `Queued`. The test then releases `B`'s
  gate; `B` keeps its slot for `wave.admit`, whose copied object reads `served: true`,
  `run_slots: 2`, `running: 2`, `queued: 1`, `blocked: 0`, `free: 0`. After the test releases
  every gate, `B` stops `Stuck` and `C` runs.
- Standing, through the daemon under `engine-limits.yaml`: `admit/1` with `admit.bindings.yaml`
  opened `--standing`; `tree run --at T` takes two steps and rests `Quiescent`. The test appends a
  line to `intake` and calls `tree wake --at T+60s`: the admission's copied object holds
  `used.steps` 1 and `since_open.steps` 3. A second append and `tree wake --at T+120s`:
  `used.steps` 1 and `since_open.steps` 5.
- Direct, under `engine-limits-none.yaml`: `ceilings` is absent.
- In the direct run of the first bullet, the `action started` line of the admission step holds the
  same capacity, member by member, as the copied object.
- Direct, under `engine-limits.yaml` with `admit-from.bindings.yaml`: the `arguments_from`
  program's input, in its blob, holds no `loom.capacity`; the admission program's does.
- Direct, under `engine-limits.yaml` with `admit-plain.bindings.yaml`: `wave.admit`'s copied object
  equals, as JSON, the literal arguments `{"wave": 1}`, with no `loom.capacity`.

`crates/loom-bindings/tests/capacity_member.rs` loads bindings against `admit/1` and the
fixture protocols of `story:child-case-own-workspace`:

- `capacity: true` on an `agent`, `open` or `person` executor is refused `MemberNotForExecutor`
  naming the action and `executor.capacity`; `capacity: false` on each loads.
- `capacity: "yes"` is refused `Malformed` naming the action and `executor.capacity`.
- `admit-reserved.bindings.yaml` is refused `ReservedArgument` naming `wave.admit` and
  `arguments`.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml`, generated
`generated/rust/loom/src/{tree,bindings,runlog}.rs`; `crates/loom-bindings/src/decode.rs`
(`EXECUTOR` gains `capacity`, read as a boolean), `crates/loom-bindings/src/load.rs` (`fit`'s
`members` gains `capacity` for `Program`; `ReservedArgument`), the capacity port and the program
port in `crates/loom-bindings/src` (no story names the program port's module);
`crates/loom-tree` (the capacity port's implementation from the scheduler and the counters);
`crates/loom-cli/tests/tree_capacity.rs`, `crates/loom-bindings/tests/capacity_member.rs` (new);
`crates/loom-cli/tests/fixtures/area/` (adds `admit.protocol.yaml`, `admit.bindings.yaml`,
`admit-gated.bindings.yaml`, `admit-from.bindings.yaml`, `admit-plain.bindings.yaml`,
`admit-reserved.bindings.yaml`, `engine-limits.yaml`, `engine-limits-none.yaml`, `bin/copy-stdin`,
`bin/copy-stdin-err`); `website/docs/guides/bindings.md` (the `capacity` member and
`loom.capacity`); `CHANGELOG.md`. `crates/loom-runlog` is used to read the line; the event type is
the generated one.

## Out of scope

Flow limits (`story:flow-limits-recorded-as-events`). Build slots of `slot`. Loom refusing a step
on its capacity. Reserving slots for an admitted wave.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the scheduler's slot counts and the budget and usage counters, which build `loom.tree.Capacity` at a step's start) — cited, story § Files; the crate does not exist at `61fd500b` (`ls crates`), `story:tree-run-command` creates it (tree-run-command.md:57)
- **Files:** `ess/domains/tree.yaml` (`Capacity`; the domain `loom.tree` does not exist yet, `ls ess/domains`) — cited, story § ESS first
- **Files:** `ess/domains/bindings.yaml:37` (`ExecutorSpec` gains optional `capacity`, Program only) — cited
- **Files:** `ess/domains/runlog.yaml` (`action started` gains optional `capacity`; the file does not exist yet, `story:hash-chained-run-log` creates it) — cited, story § ESS first
- **Files:** `crates/loom-bindings/src/decode.rs:27` (`EXECUTOR` is `[&str; 12]`, takes 13), `:353` (the only `ExecutorSpec { .. }` literal in the tree, `git grep`), `:297` (`fn executor`; no boolean reader exists beside the `text` and `paths` closures at `:327-345`) — cited
- **Files:** `crates/loom-bindings/src/load.rs:232` (`fit`, `members: [_; 11]` takes a twelfth row `("capacity", .., Executor::Program)`; it raises `Reason::MemberNotForExecutor` naming action and `executor.<member>` at `:249-254`, which is the "refused at load" bullet) — cited
- **Files:** `crates/loom-cli/tests/tree_capacity.rs` (new) and `crates/loom-cli/tests/fixtures/area/` (`admit.protocol.yaml`, `admit.bindings.yaml`, `admit-gated.bindings.yaml`, `admit-from.bindings.yaml`, `admit-plain.bindings.yaml`, `engine-limits.yaml`, `engine-limits-none.yaml`, `bin/copy-stdin`, `bin/copy-stdin-err`) — cited, story § Files; the `area/` directory does not exist yet (`story:standing-tree-rests-quiescent` creates it)
- **Symbols:** `loom.tree.Capacity`, `loom.tree.TreeBudget`, `loom.tree.TreeUsage` (with `since_open`), `loom.engine.LimitsSection`, `ExecutorSpec`, `Executor::Program` — cited, story; none but `ExecutorSpec` and `Executor` exists in the tree (`git grep` for `TreeBudget`, `LimitsSection` and `loom.tree` finds nothing under `ess/` or `crates/`)
- **Symbols:** `DeterministicSelector::select` ordering, non-`write` actions by id, so `intake.prep` before `wave.admit` — cited, `crates/loom-executor/src/selection.rs:319` (the ordering comment; the `.min()` is at `:336-338`)
- **Also likely:** the program port module in `crates/loom-bindings/src/` (`program.rs` is a guess; `story:bound-program-result-table` names the crate and the test `crates/loom-bindings/tests/bound_program.rs`, not the module), which adds `loom.capacity` to the arguments object — inferred
- **Also likely:** a capacity port in `crates/loom-bindings` that `crates/loom-tree` implements, because the port lives in `loom-bindings` and the counters in `loom-tree`; neither body says how the figures cross — inferred
- **Also likely:** `crates/loom-runlog` (writer of the `action started` line; the acceptance reads it with its reader) — inferred, `story:hash-chained-run-log` creates the crate
- **Also likely:** `crates/loom-bindings/tests/bindings_document.rs` (asserts on `executor.*` members and on `MemberNotForExecutor`, `:271-304`, `:457-486`) — inferred
- **Documents:** `website/docs/guides/bindings.md` ("What each member means", `:140-153`, lists each executor member) — inferred; the story names no guide
- **Confidence:** medium, because the story names its files and the two `loom-bindings` files are read in the tree, but `crates/loom-tree`, `ess/domains/{tree,runlog,engine}.yaml`, `crates/loom-runlog`, the program port and `fixtures/area/` all come from unlanded ancestors, so their paths are the stories' words
- **Would collide with:** `crates/loom-bindings/src/{decode,load}.rs` and `ess/domains/bindings.yaml`. Ordered by `depends_on`: `story:bound-program-result-table`, `story:bound-agent-node`, `story:child-case-own-workspace`, `story:run-caps-per-arrival` (all ancestors; `bindings-document` is delivered). Unordered: `story:bindings-child-path-refusal-names-the-bindings-directory` (decode/load.rs, `bindings.yaml`; it depends only on `bindings-document`) and `story:agent-session-through-substrate` (`bindings.yaml`; its dependency path does not reach this story)
- **Would collide with:** `ess/domains/tree.yaml`, `ess/domains/runlog.yaml`, `crates/loom-tree`. Ordered: `tree-run-command`, `tree-daemon`, `concurrent-cases-in-the-tree-daemon`, `standing-tree-wakes-at-a-host-instant`, `tree-usage-and-model-budget`, `action-limits-enforced`, `agent-node-run-records`, `hash-chained-run-log` (ancestors) and `flow-limits-recorded-as-events` (descendant, it depends on this story, so this one lands first). Unordered: `story:budget-and-limit-stops-raise-decision-requests` (tree.yaml, runlog.yaml, loom-tree; it changes `TreeBudget`, which `Capacity.budget` carries), `story:non-independent-review-becomes-decision-request` (tree.yaml, runlog.yaml, loom-tree), `story:run-log-references-exec-file-changes` (runlog.yaml, loom-tree; it depends on the budget story, not on this one), `story:authority-records-bound-and-grounded-in-canon` (tree.yaml), `story:requests-from-canon-pending-decisions` (loom-tree)
- **Would collide with:** `crates/loom-cli/tests/fixtures/area/`, additive, no shared file names. Ordered: `standing-tree-rests-quiescent`, `standing-tree-wakes-at-a-host-instant`, `concurrent-cases-in-the-tree-daemon`, `run-caps-per-arrival`, `child-case-own-workspace`, `tree-usage-and-model-budget` (ancestors) and `flow-limits-recorded-as-events` (descendant, it adds `admit-open.protocol.yaml` beside this story's `admit/1`)
- **Safety fact:** `loom.capacity` cannot reach an `arguments_from` run or the request Commission revalidates, because the port adds it to its own stdin after revalidation — story § Outcome, 1 stated, unproven
- **Safety fact:** `ExecutorSpec` is built in one place only, `decode.rs:353`, and no other crate constructs it, so the new optional member breaks no caller — `git grep -n 'ExecutorSpec {'`, 2 pointed at a line, unproven
