---
format: aep.planning-md/3
id: story:tree-run-command
kind: story
status: proposed
title: b10x-loom tree runs a case tree from a store to a terminal outcome or the next approval and names the approval command
summary: tree open, run, approve, close, verify and status on Commission's loop with the deterministic selector and bound executors
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-artifact-revisions
- depends_on: story:deterministic-default-selector
- depends_on: story:record-copy-between-cases
- depends_on: story:hash-chained-run-log
scope:
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: inferred
  path: crates/loom-cli/tests/fixtures/tree
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: inferred
  path: crates/loom-conformance/src/tree.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/scenarios/tree-run
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: website/data/status.json
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":4}}}
---
## Outcome

`b10x-loom` has a command group that runs a tree of protocol cases from a case store. The run
logic is a new crate, `b10x-loom-tree` (`crates/loom-tree`); `crates/loom-cli` holds the flags.

- `tree open` creates the root case in a store directory from a registered protocol, a
  `loom.bindings/1` file and initial artifact revisions, with the file's `independence` map;
- `tree run` drives the tree: every open child before its parent's next step, each step selected
  by the deterministic selector and performed through the action's bound executor, until the root
  is terminated through its one legitimate outcome or the tree waits on a person. A case is
  terminated when its completion gives exactly one legitimate outcome and no action in it is
  useful; a child's termination is written into its parent;
- when it waits, it prints, per decision, the case, the action, what is decided (the subject, its
  revision and the path to read) and the exact command that records it; for an admissible writer
  whose binding has a `handoff`, the decision is the binding's `handoff` text and the command is
  `tree run` again;
- `tree approve` records a principal's decision on a capability for one case, and with `--action`
  then takes that action as the principal: the record a `person` executor's evidence entry
  declares is produced by that principal about the subject's revision at that moment;
- `tree close` terminates a case through a named outcome when more than one is legitimate;
- `tree verify` checks the run log of every case of the tree and names the first line that fails;
- `tree status` prints the tree as JSON for a live view: per case its id, protocol, revision,
  outcome or stop, open obligations, and the path of its case file and run log.

A case's workspace is the tree's one workspace, the existing directory `tree open` is given as
`--workspace <dir>` and records in the store beside the root case file: every case of the tree
works in it, every path of the bindings is relative to it, and under a Substrate daemon it must
lie directly under the daemon's workspace root (`story:substrate-daemon-runner`).

`tree run` ends with exit 0 at a terminal outcome, 3 when it waits on a person or on authority,
4 when a case is stuck (nothing useful, no legitimate outcome, nothing awaiting approval), and 5
when the step or wall-time budget is spent, each naming the case. One writer per store: a second
`tree` command on a store in use is refused naming the holder. Every instant comes from the host
clock, or from `--at` when given.

This story is what emits the run log's events and implements the selector's `StepHistory` from
the store: `performed_at` from the log's `action finished` lines, `due` from `retry_records`
(`story:bound-agent-node`'s function) and `copy_due` (`story:record-copy-between-cases`).

What exists and what is new, read 2026-11-21. Exists and is used as it is: a model-free
executor (`Loom` over a selector and `EmptyObjectArguments`,
`crates/loom-executor/src/lib.rs:97-168`, `arguments.rs:45-56`), the in-memory run records
(`RunStore`, `crates/loom-commission/src/outcome.rs:246-345`), and the runtime's `Clock`, `Ids`,
`Limits` and `HostControl` ports (`crates/loom-commission/src/runtime.rs:588-649`). New in
`crates/loom-tree`: an authority provider over the approvals recorded per case, capability and
principal (the intake slice's `GrantAuthority` allows a capability only for one suspended
request, `crates/loom-intake-slice/src/grant.rs:358-448`); the one-writer lock that names its
holder (the governor's lock is taken per operation); and ids derived from the case, its revision
and the step, so that two runs give the same bytes. Nothing moves out of
`crates/loom-intake-slice`.

## Why

`b10x-loom run` takes one intent and opens one case (`crates/loom-cli/src/lib.rs:766-890`,
`crates/loom-intake-slice/src/run.rs:10-11`); no command takes a case store, and the workflow
executor has no subcommand. The SDC's driver has the surface (`init`, `run`, `approve`, `close`,
`~/beyond10x/sdc/driver/src/main.rs:32-85`, `791-826`) and
`~/beyond10x/sdc/docs/boundary.md` row 1 puts it in Loom's command line. A wait on a person must
name its command (`~/beyond10x/sdc/docs/driver.md` § Status, third pass: "`run` names the exact
command when it waits on a person"). The SDC's live view needs the list of cases and where each
log is (request 7 of session `sdc`), which is what `tree status` gives it.

## ESS first

A new domain `loom.tree` in `ess/domains/tree.yaml`, declared in `ess/system.yaml`: `TreeRun`,
`TreeStop` (`Terminal`, `AwaitingPerson`, `AwaitingAuthority`, `Stuck`, `BudgetSpent`),
`NextDecision` (case, action, decides, command), `TreeStatus`, `TreeBudget`, `CaseApproval`
(case, capability, decision, principal, case revision, instant; kept in the store beside the
case file), and the refusal `StoreInUse`. The domain declares types only, no command, so no
conformance scenario is added. On that commit `task drift` fails, and `crates/loom-cli/tests/tree_run.rs` does not
compile. The generated CLI reference (`website/docs/reference/cli.md`) is regenerated in the
implementing commits.

## Acceptance

With a fixture tree of three levels (root, two children, one grandchild), stub programs, no
model and no engineering vocabulary:

- `tree open` then `tree run` reaches the first approval, exits 3, and prints a `tree approve`
  command; running that command and `tree run` again continues; after the last approval the root
  is terminated through its one legitimate outcome and `tree run` exits 0.
- The order of steps is the same on two runs from two fresh stores with the same `--at` values,
  and each case's final Canon decision is byte-identical between the two.
- A child's wait stops the run naming the child's case, and `tree approve` on that case is the
  printed command.
- `tree approve --action` on an action bound to `person` records the declared evidence with the
  principal as producer and the subject's revision at that moment; after the subject's file
  changes, the next `tree run` waits on the same approval again. `tree run` never takes a
  `person` action itself.
- A fixture writer with a `handoff` that is admissible when nothing is useful makes `tree run`
  exit 3 and print the `handoff` text.
- A root with two legitimate outcomes is not terminated by `tree run`; it exits 3 and prints the
  two `tree close` commands. `tree close` with a non-legitimate outcome exits non-zero and leaves
  the case file byte-identical.
- A tree with no useful action, no legitimate outcome and nothing awaiting approval exits 4 and
  names the case.
- A root opened from a bindings file with an `independence` entry accepts that producer's record
  on a match that demands the dimension.
- An action with no `may_produce` is taken once per case revision, a writer whose check failed
  with a `retry_when` result is taken again, and a `copy` action is not taken while the source
  holds nothing new.
- `--max-steps` and `--max-wall-seconds` each stop the run with exit 5 before the step that
  would pass the limit.
- A second `tree run` started while the first holds the store exits non-zero naming
  `StoreInUse` and the holder's process id, and writes nothing.
- After the run `tree verify` exits 0; after one byte of one log is changed it exits non-zero
  naming the case and the line. `tree status` prints one entry per case with the outcome each
  ended through and a log path that exists.
- In each case's log `action finished` precedes the `evaluated` line that shows the moved
  revision, and a `child closed` line in the parent follows the child's `terminated` line.
- With a canary string as an argument value and as a stub program's output, no log line of the
  tree contains the canary.
- The loop is Commission's (`run_until_blocked` and its ports): every selected action is
  revalidated against case revision, frontier and authority before its effect, and a test fails
  if `crates/loom-tree` calls an effect port outside it.
- The engine's confinement is one flag with the values `b10x-loom run` has
  (`--confinement substrate|local`, with `--local-reason`; `none` is not offered).
  `crates/loom-tree` and `crates/loom-bindings` contain no `Command::new` (a test greps), and
  under `--confinement substrate` a stub program that reads a host file outside the workspace fails
  in every case of the tree.
- `website/docs/reference/cli.md` is regenerated and a guide page shows one tree run with pasted
  output.
- `task check` exits 0.

## Out of scope

A cap on agent calls and agent events in the log (`story:agent-node-run-records`). Mandate
approvals for tree cases. A replay command. Running two cases of a tree at once (the daemon's,
`story:concurrent-cases-in-the-tree-daemon`; a direct `tree run` stays sequential). A service or
daemon that watches a store. Turning a wait into a decision request, and `tree approve` through that
request (`story:tree-waits-open-decision-requests`, `epic:decision-requests-in-the-runtime`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (`b10x-loom-tree`), a new crate holding the run logic — cited (story § Outcome)
- **Files:**
  - `crates/loom-cli/src/lib.rs` (clap types for the `tree` group) — cited (story § Outcome; caller)
  - `crates/loom-cli/src/main.rs` (dispatch; `Command` is matched exhaustively at `main.rs:63-84`) — cited
  - `crates/loom-cli/tests/tree_run.rs` — cited (story § ESS first)
- **ESS:**
  - `ess/domains/tree.yaml` (`TreeRun`, `TreeStop`, `NextDecision`, `TreeStatus`, `TreeBudget`, `StoreInUse`) — cited (story § ESS first)
  - `ess/system.yaml`, which lists `loom.tree` under `domains:` — cited (story; `ess/system.yaml:10-23`)
  - `ess/ess-inputs.yaml`, which lists the domain file. Each earlier domain did this (`adcbf763`, `7021cd30` each touch `ess-inputs.yaml` + `system.yaml`) — cited
- **House-rule test:** `crates/loom-executor/tests/crate_names.rs`. `EXPECTED: [..; 28]` must equal the set of workspace packages (`workspace_package_set_is_the_loom_names`), so the new crate must be added to it — cited
- **Also likely:**
  - `crates/loom-cli/Cargo.toml`, a path dependency on `b10x-loom-tree` — inferred (story puts the logic in the crate and the flags in the CLI)
  - `crates/loom-cli/src/tree.rs`, the tree output and exit-status mapping as a module — inferred (same pattern as `agent.rs` and `stage.rs`)
  - `crates/loom-cli/tests/fixtures/tree/`, the three-level fixture tree with stub programs — inferred (fixtures live in `tests/fixtures/`)
  - `ess/scenarios/tree-run/`, plus `crates/loom-conformance/src/tree.rs` and `crates/loom-conformance/src/lib.rs` — inferred, and needed **only if** `tree.yaml` declares a command. `crates/loom-conformance/tests/conform.rs:8` needs one scenario per declared command, and `LoomTarget` answers each domain in its own module (`resource.rs:31-35`)
- **Documents:**
  - a new guide page showing one tree run with pasted output — cited (story § Acceptance). The path `website/docs/guides/run-a-case-tree.md` is inferred: the name is a placeholder, and the sidebar is autogenerated (`website/sidebars.ts:15`)
  - `website/data/status.json`: a change that ships a capability updates it (`AGENTS.md:310-312`) — cited
- **House-rule files (not listed as paths):**
  - `website/docs/reference/cli.md`, regenerated; "a flag change in `b10x-loom-cli` regenerates the CLI reference" (`AGENTS.md:297,304`)
  - `website/docs/reference/crates.md`, regenerated for the new crate (`AGENTS.md:299`)
  - `website/docs/reference/ess/` and `generated/rust/loom/`, from `tree.yaml`
  - `CHANGELOG.md`, `README.md` and `AGENTS.md`: a new command and a new crate (`AGENTS.md:310-312`)
  - `Cargo.lock`
  - `docs/contracts/run-events-v1.json` is **not** touched: it covers only `run --output jsonl` (`AGENTS.md:301,172`) — cited
  - `website/docs/reference/sdk-stability.md` is touched only if `loom-sdk` re-exports `loom-tree`, which the story does not say (`AGENTS.md:171`) — inferred
  - no CLI contract test walks every subcommand: `crates/loom-cli/tests/loom_cli.rs` reads only `run --help` — cited
- **CLI house pattern:**
  - each command has a `*_HELP` const with an `Exit status:` table, an exit-status function and a unit test, all in `lib.rs` (`APPROVE_HELP` `lib.rs:59-86`, `approve_exit_status`, `each_stop_reason_has_its_exit_status` `lib.rs:995`). `tree run` follows it with 0/3/4/5 — cited
  - `--confinement` reuses `lib.rs:985 Confinement`, which `confinement-opens-from-engine-config` changes to `substrate|local` + `--local-reason` — cited
- **Composition (what exists):**
  - **Model-free executor exists:**
    - `Loom<S, G, V>` (`crates/loom-executor/src/lib.rs:97,130,351`) over a selector and `EmptyObjectArguments` (`arguments.rs:45-56`, always `{}`)
    - `with_governor` revalidates each selection against the current frontier before proposing (`lib.rs:146`)
    - `with_instance` fixes the id namespace (`lib.rs:168`)
    - all of the above — cited. Empty arguments are enough because bound executors take their argv from the binding — inferred
  - **Run records exist:**
    - `RunStore` (`crates/loom-commission/src/outcome.rs:246-345`) is public and in memory, implements `StartRunBehavior` and `SuspendRunBehavior`, and is used as `Generated::new(RunStore::new(ids))` (`crates/loom-intake-slice/src/run.rs:705`, `crates/loom-cli/tests/session_events.rs:228`) — cited
    - a fresh Run per `tree run`, with the case store and run log as the only durable state, is enough for `run_until_blocked` — inferred
  - **Commission record:**
    - `operator_commission` is `pub(crate)` (`run.rs:1427`), so `loom-tree` builds its own `CommissionData` (five fields, principal per approver) — cited / inferred
  - **Clock and limits:**
    - the runtime's `Clock`, `Ids`, `Limits` and `HostControl` (`crates/loom-commission/src/runtime.rs:588-649`) are re-exported by `loom-sdk` (`lib.rs:91`). `--max-steps` maps to `Limits::step_budget` and `--max-wall-seconds` to `HostControl::host_stop`, which is asked before each step — cited
    - a host instant is available from `loom_intake_slice::clock::{SystemClock, instant}` (`clock.rs:61-75`, public) — cited
    - `loom-tree` needs its own deterministic `Ids`: the slice's `fresh_uuid` is salted with the process id and time (`executor.rs:1645-1653`), so it cannot give byte-identical runs — inferred
  - **Authority does not fit and must be new in `loom-tree`:**
    - `GrantAuthority` allows a capability only for the request a `Pending` Suspension holds (`crates/loom-intake-slice/src/grant.rs:358-448`). `GrantData` is `intake.routing`'s per-commission grant, and `grant.rs` imports the merge handoff — cited
    - `NoDelegatedAuthority` is private (`run.rs:1058`) — cited
    - `loom-tree` needs its own `AuthorityProvider` over per-case, per-capability, per-principal approvals kept in the store. Nothing has to move out of `crates/loom-intake-slice` — inferred
  - **Store lock is new:**
    - no lock that holds the store for one writer and names its process id exists. The governor's `locked_dir` takes a lock per operation (`crates/loom-governor/src/locked_dir.rs:1-12`) — cited
    - so `StoreInUse` is new code in `loom-tree` — inferred
  - **Run log bridge:** run-log emission goes through the runtime's `EventSink` (`ports/events.rs:26`, via `HostControl::events`) into `loom-runlog` — inferred
- **Confidence:** high for `crates/loom-tree`, the CLI files, the ESS files and `crate_names.rs` (the story or a gate test names them); medium for the conformance and scenario additions and the guide path
- **Would collide with:**
  - `crates/loom-cli/src/lib.rs` and `main.rs` (the `Confinement` enum and any other `run` or `tree` flags)
  - `ess/system.yaml` and `ess/ess-inputs.yaml` (every story that adds an ESS domain)
  - `crates/loom-executor/tests/crate_names.rs` (every story that adds a crate)
  - `crates/loom-tree` (any later unit testing the tree)
  - `crates/loom-conformance/src/lib.rs` (any domain that adds commands)
  - `website/data/status.json`
  - generated: `website/docs/reference/cli.md`, `crates.md`, `generated/rust/loom/`
- **Safety fact:** the change adds and alters nothing. `Command` is matched exhaustively only in `crates/loom-cli/src/main.rs:63-84`, and `loom-docs` reads it only through clap (`crates/loom-docs/src/cli.rs:132`). The `run` exit-status table that the run-events contract reads (`b10x_loom_cli::EXIT_STATUS`, `crates/loom-docs/src/main.rs:477`) is untouched by a separate `tree` table. Proven to level 2, unproven
