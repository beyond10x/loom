---
format: aep.planning-md/3
id: story:agent-executor
kind: story
status: draft
title: Loom implements the Commission AgentExecutor on synthesized model types
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:ess-hard-gate
scope:
- confidence: cited
  path: .gitignore
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-xtask/
- confidence: cited
  path: crates/loom/Cargo.toml
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/tests/agent_executor.rs
- confidence: cited
  path: generated/rust/loom/
revision: 10
---
## Outcome

`b10x-loom` implements the Commission `AgentExecutor` on model types synthesized from `ess/`,
replacing the hand-written bootstrap types in `crates/loom/src/lib.rs`, and carries the
model-facing approval stop: an approval-gated selection returns `Suspended` and is never executed
by Loom. It runs after `story:ess-hard-gate` and before `story:run-pipeline-skeleton`, and it owns
the generated model:

- **Layout.** The synthesized model is committed under `generated/rust/loom/`, written by
  `ess generate synthesize --path ess --target rust --layout crate --out <dir>`. As in Commission
  (commission `story:generated-responsibility-model`, probe 2026-10-04, ess 0.52.0), the generated
  crate declares its own `[workspace]`, so it is not a member of `members = ["crates/*"]`.
  `crates/loom/Cargo.toml` takes it as a path dependency and `b10x-loom` re-exports it; every later
  story imports model types through `b10x-loom`.
- **Regeneration.** `generate` writes a fresh tree into a temporary directory and replaces
  `generated/rust/loom/` with it. `.ess-output/` carries a per-run anchor, so it is git-ignored
  (`generated/rust/loom/.ess-output/` in `.gitignore`) and left out of every comparison.
- **Checks are Rust.** A new crate `crates/loom-xtask/` (clap derive) has three subcommands:
  `generate`; `drift`, which regenerates into a temporary directory, compares file by file with
  `generated/rust/loom/` ignoring `.ess-output/`, and fails naming the first differing file; and
  `no-hand-model`, which fails naming file and line when a source file under `crates/loom/src`
  defines a struct or enum whose name is an entity or type name of `ess/` with its `loom.run.`
  prefix removed (names read from `ess specify compile --path ess --format json`, so a type a
  later story adds is covered without editing the check).
- **Taskfile.** Tasks `generate`, `drift` and `no-hand-model` call `cargo run -p loom-xtask -- <sub>`;
  `check` lists `drift` and `no-hand-model` as their own steps. Later stories add their own named
  tasks and one line each to `check`; no story edits another story's task body.
- **Bootstrap types.** The hand-written `Selection` (`crates/loom/src/lib.rs:9-12`) is deleted. The
  Canon bootstrap imports (`lib.rs:5`, `ActionCandidate`, `ActionId`, `ActionStatus`, `Frontier`;
  `lib.rs:111`, `CaseId`, `ClaimValue` in the unit tests) are removed: Canon's wave-1 change deletes those types, and Loom takes the frontier from
  Commission (see Depends on).
- **Minimal seam until the selector and generator stories.** Removing those imports leaves `ActionSelector`,
  `ArgumentGenerator`, `FirstAdmissibleSelector`, `EmptyObjectArguments` and `Loom::run`
  (`lib.rs:14-106`) without their types, so this story re-types them onto Commission's frontier
  and changes nothing else about them. `ActionSelector::select` takes
  `commission.responsibility.Frontier` and returns the `action` of one of its `FrontierAction`s;
  `FirstAdmissibleSelector` returns the first whose `status` is `ActionStatus::Admissible`.
  `ArgumentGenerator::generate` takes one `FrontierAction`; `EmptyObjectArguments` returns `{}`.
  `Loom::run` keeps its refusal of an action outside the frontier and returns `Suspended` on
  `ActionStatus::ApprovalRequired`, naming the action's `capability`. This seam keeps the tree
  green until `story:action-selector` replaces the selector seam with selection over the projected
  catalogue and `story:argument-generator` replaces the generator seam. Neither replacement is done here.

## Shared surface

Re-planned 2026-10-04 for wider waves. `ess/domains/run.yaml`, `generated/rust/loom/` and the
module list of `crates/loom/src/lib.rs` are no longer one chain through every story:

- This story owns the generated layout, `crates/loom-xtask/`, `.gitignore`, the `generate`,
  `drift` and `no-hand-model` tasks and the seam it leaves in `lib.rs`. It depends on
  `story:ess-hard-gate`.
- `story:run-pipeline-skeleton` depends on it and lands, once, every settled ESS declaration of
  the pipeline (catalogue entries, select, argument request, revalidation), regenerates, and
  creates the module files with their `pub mod` lines.
- After the skeleton, stories own disjoint files and are ordered only by behaviour:
  `story:frontier-projection` and `story:harness-crate-port` are independent;
  `story:frontier-projection` → `story:action-selector` → `story:argument-generator` →
  `story:selection-revalidation` → `story:harness-loop-port` is a behavioural chain that also
  shares the executor pipeline in `lib.rs`; `story:session-transcript-streaming` needs only the
  ported loop (`story:harness-crate-port`); `story:compaction-contract` and
  `story:interruption-recovery` each need the session and the wired loop, and still edit
  `loom.run.Session` in `ess/` themselves, so they are kept apart by that collision rather than by
  an edge; `story:loom-ess-conformance` is last.

`Cargo.lock` is shared with `story:harness-crate-port` and `story:loom-ess-conformance`, and
`Taskfile.yml` with `story:loom-ess-conformance`; both come after this story.

## ESS first

- This story does not change `ess/`. The `commission_run` marker on `loom.run.Session` is closed by
  `story:ess-hard-gate` (resolution (b)): `commission_run` is typed `loom.run.CommissionRunId`, a
  `newtype` of `Uuid` mirroring `commission.responsibility.RunId` (commission
  `ess/domains/responsibility.yaml:34-36` at `013e392`), with no cross-system `relations:` entry.
- Generate `generated/rust/loom/` from that specification as above. No hand-written model type.
  `task ess-gate` stays green.
- **Specification change:** none — the specification this story generates from is already closed
  by `story:ess-hard-gate`; the behaviour it adds is generated from it.
- **Red test:** the first commit adds `crates/loom/tests/agent_executor.rs`;
  `session_carries_commission_run_id` fails on that commit because no generated `Session` is
  reachable through `b10x-loom` yet, and `executor_suspends_at_merge_approval` fails because Loom
  does not yet implement the generated `AgentExecutor`. The generation and executor commits make
  both pass.

## Domain relations

- `loom.run.Session -> commission.responsibility.Run`: each session executes exactly one run; the
  run is owned by its commission, not by Loom, so a session never creates or deletes a run and the
  run exists before the session. Ownership: commission `ess/domains/responsibility.yaml`, entity
  `commission.responsibility.Commission`, relation `runs` (owns, many), at commission `013e392`. The
  session side is inferable (inferred from `crates/loom/src/lib.rs:71`, one executor invocation per
  commission and frontier). How many sessions one run may have is not declared by this story.

## Depends on, outside this store

**This story cannot start before commission `story:agent-executor-port` (M-004) is implemented**
(status `proposed` on commission `main` at `f9bea71`, 2026-10-04). The `AgentExecutor` trait this
story implements is today the bootstrap one, `run(&self, &Commission, &Frontier)` with Canon's
`Frontier` (commission `crates/commission/src/lib.rs:53-55`); M-004 replaces it with the trait over
the generated `commission.responsibility.Frontier` returning the generated `ExecutorOutcome`, and
acceptance items 2 and 6 need exactly that (`Suspended` with a `SuspensionReason`, and no
`b10x_canon` import left in `lib.rs`). Its test also needs commission `story:governor-port`
(M-003) and `story:local-runtime-loop` (M-009) for the scripted fake governor; both are `proposed`.
A `depends_on` cannot cross stores, so this is recorded here and not as an edge.

- `commission:story:ess-hard-gate`: declares the frontier's contents in Commission's own ESS
  specification — `commission.responsibility.Frontier.actions: List<FrontierAction>`, with
  `FrontierClaim`, `FrontierObligation` and the enum `ActionStatus` (`Admissible`,
  `ApprovalRequired`, `Blocked`). `AgentExecutor::run` receives that frontier, and the seam above is
  typed on it.
- `commission:story:agent-executor-port` (M-004): the generated `ExecutorOutcome`, its
  `SuspensionReason` and `ProposedActionArguments`. The current `AgentExecutor::run`
  (commission `crates/commission/src/lib.rs:54`) passes no run identity; if M-004 does not add one,
  this story cannot fill `commission_run` from the call and its report says so.
- `commission:story:local-runtime-loop` (M-009): the scripted fake governor. ELS `software.change/1`
  (E-002) for the fixture frontier.

## Scope

- `generated/rust/loom/` (new, generated); `.gitignore`
- `crates/loom-xtask/` (new)
- `crates/loom/Cargo.toml`, `crates/loom/src/lib.rs`, `Cargo.lock`
- `crates/loom/tests/agent_executor.rs` (new)
- `Taskfile.yml` (tasks `generate`, `drift`, `no-hand-model`; the `check` list)
- `ess/domains/run.yaml` is read, not edited (closed by `story:ess-hard-gate`)

## Constraints

Loom decides no authority and no completion. `beyond10x/harness` is not changed.

## Acceptance

`task check` meets each of these expectations:

1. It passes on the story's tree.
2. Its test `executor_suspends_at_merge_approval` (`crates/loom/tests/agent_executor.rs`) drives
   Loom as the `AgentExecutor`, with the Commission scripted fake governor serving the ELS
   `software.change/1` frontier, through successive invocations until it returns `Suspended`
   naming the merge approval capability; no invocation returns a `ProposedAction` for
   `repository.merge`.
3. Its test `session_carries_commission_run_id` (same file) builds a generated `Session` through
   `b10x-loom`'s re-export with a `CommissionRunId`, and reads the same value back from its
   `commission_run` field.
4. With one byte changed in a file under `generated/rust/loom/src/`, its `drift` step fails and
   names that file.
5. With `pub struct Selection { pub action: String }` added to `crates/loom/src/lib.rs`, its
   `no-hand-model` step fails and names that file and line.
6. Its test `lib_imports_no_canon` (same file) reads `crates/loom/src/lib.rs` and finds no
   `b10x_canon` in it: `lib.rs` imports no `b10x_canon` item.

## Source

TASKBOARD L-002; Atlas ADR 0071, 0075; `docs/design/loom-design.md` § Commission integration; the
approval-stop clause of `epic:loom-native-harness`; the generated-model pattern of commission
`story:generated-responsibility-model` (revision 9).
