---
format: aep.planning-md/3
id: story:run-pipeline-skeleton
kind: story
status: draft
title: Declare the run pipeline's settled ESS nouns up front and create Loom's module files
summary: 'Spec-first skeleton: catalogue entries, select, argument request and revalidation in ess/, one regeneration, empty module files with their pub mod lines.'
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom/src/arguments.rs
- confidence: cited
  path: crates/loom/src/compaction.rs
- confidence: cited
  path: crates/loom/src/harness/mod.rs
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: cited
  path: crates/loom/src/projection.rs
- confidence: cited
  path: crates/loom/src/recovery.rs
- confidence: cited
  path: crates/loom/src/revalidation.rs
- confidence: cited
  path: crates/loom/src/selection.rs
- confidence: cited
  path: crates/loom/src/session.rs
- confidence: inferred
  path: crates/loom/tests/run_skeleton.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 3
---
## Outcome

One spec-first story lands, up front, every ESS declaration of `epic:loom-native-harness` whose
shape the story bodies already settle, regenerates `generated/rust/loom/` once, and creates the
empty module files of the epic with their `pub mod` lines in `crates/loom/src/lib.rs`. After it,
`story:frontier-projection`, `story:action-selector`, `story:argument-generator` and
`story:selection-revalidation` no longer edit `ess/`, `generated/` or the module list of `lib.rs`,
and `story:harness-crate-port`, `story:session-transcript-streaming`, `story:compaction-contract`
and `story:interruption-recovery` find their module file already declared. It adds no behaviour:
every module file holds only its module doc comment.

### Declarations landed here, and where each is settled

1. **Catalogue entries** (`story:frontier-projection` § ESS first and § Domain relations: "the
   projected entries (action id and status)"; one entry per `FrontierAction` whose status is
   `Admissible` or `ApprovalRequired`, none for `Blocked`):
   - `loom.run.CatalogueEntryStatus`, `enum`, variants `Admissible`, `ApprovalRequired`: the two
     projected statuses of `commission.responsibility.ActionStatus`, mirrored as a Loom type the way
     `loom.run.CommissionRunId` mirrors `commission.responsibility.RunId` (ESS 0.52.0 cannot name
     another system's type);
   - `loom.run.CatalogueEntry`, `struct`, fields `action: String` (the type of
     `FrontierAction.action`) and `status: loom.run.CatalogueEntryStatus`;
   - field `entries: List<loom.run.CatalogueEntry>` on `loom.run.ActionCatalogue`;
   - command `loom.run.ProjectCatalogue`, creating `loom.run.ActionCatalogue` from its
     `catalogue_id`, `turn_id`, `frontier`, `case_revision` and `entries` (outcome `projected`).
2. **Selection** (`story:action-selector` § ESS first): command `loom.run.SelectAction`, creating
   `loom.run.Selection` from `selection_id`, `catalogue_id`, `action`, `confidence`
   (`Optional<Decimal>`, as declared by `story:ess-hard-gate`) and `strategy` (outcome `selected`),
   with a refusal outcome whose error names the action id when it is absent from the catalogue's
   entries.
3. **Argument request** (`story:argument-generator` § ESS first): command
   `loom.run.RequestArguments`, creating `loom.run.ArgumentRequest` from `argument_request_id` and
   `selection_id` (outcome `requested`).
4. **Revalidation** (`story:selection-revalidation` § ESS first and § Outcome): `loom.run.Selection`
   lifecycle `Selected` → `Admitted` or `Refused` (initial `Selected`, terminal `Admitted` and
   `Refused`); command `loom.run.RevalidateSelection` whose input carries the case revision and the
   action ids of the frontier current at revalidation, with outcomes `admitted` (→ `Admitted`),
   `not-in-frontier` (→ `Refused`, naming the action id) and `stale-revision` (→ `Refused`, naming
   the selection's catalogue revision and the current one).

Names of commands, outcomes and errors are this story's choice; their inputs, outcomes and refusals
are the ones the cited sections settle. If ESS 0.52.0 cannot express one of the refusal conditions,
the story stops and reports it; it does not drop the refusal and writes no `UNMAPPED:` marker
(ADR 0076).

### Not declared here, and why

- `loom.run.Session` and `loom.run.Turn` changes for filing, resume, compaction and interruption
  (`story:session-transcript-streaming`, `story:compaction-contract`,
  `story:interruption-recovery`). Their shapes are not settled: two stories each add a "resume"
  command on `Session` without saying whether it is one command, the `Session` lifecycle states are
  not named, and the shape of a compaction's usage and record is not given. Those three stories keep
  `ess/domains/run.yaml` and `generated/rust/loom/` in their own scope.
- Any field of `CatalogueEntry` beyond action id and status (for example the approval
  `capability`), and an action's argument schema (`decision-blocker:action-argument-schema`).

### Module files created, each holding only its module doc comment

`crates/loom/src/projection.rs`, `selection.rs`, `arguments.rs`, `revalidation.rs`, `session.rs`,
`compaction.rs`, `recovery.rs`, and `crates/loom/src/harness/mod.rs` (the parent module of the
`port` rows of `docs/design/harness-map.md`; `story:harness-crate-port` adds its submodules), each
with its `pub mod` line in `crates/loom/src/lib.rs`. No trait, function or type moves into them
here: the seam `story:agent-executor` leaves in `lib.rs` stays where it is.

## Shared surface

This story is the one place after `story:agent-executor` that edits `ess/domains/run.yaml`,
`generated/rust/loom/` and the module list of `crates/loom/src/lib.rs` for the pipeline nouns. It
depends on `story:agent-executor`, which owns the generated layout, `task generate`, `task drift`
and `task no-hand-model`. Every other open story of `epic:loom-native-harness` depends on it.

## ESS first

- **First commit:** the four declaration groups above in `ess/domains/run.yaml` and nothing else.
- **Red on it:** `task drift` fails, naming the first file of `generated/rust/loom/` that differs
  from a fresh `ess generate synthesize` of the changed specification.
- **Then:** `task generate`, the module files, the `pub mod` lines and the test
  `run_skeleton_declares_pipeline_nouns`. `task ess-gate` stays green on every commit.

## Scope

- `ess/domains/run.yaml`, `generated/rust/loom/` (regenerated)
- `crates/loom/src/lib.rs` (`pub mod` lines only)
- `crates/loom/src/projection.rs`, `selection.rs`, `arguments.rs`, `revalidation.rs`,
  `session.rs`, `compaction.rs`, `recovery.rs`, `harness/mod.rs` (new, doc comment only)
- `crates/loom/tests/run_skeleton.rs` (new)

## Acceptance

`task check` passes on the story's tree, and its test `run_skeleton_declares_pipeline_nouns` in
`crates/loom/tests/run_skeleton.rs` passes. It checks:

1. `ess specify compile --path ess --format json` names the commands `loom.run.ProjectCatalogue`,
   `loom.run.SelectAction`, `loom.run.RequestArguments` and `loom.run.RevalidateSelection`, the
   types `loom.run.CatalogueEntry` and `loom.run.CatalogueEntryStatus`, and the `loom.run.Selection`
   lifecycle states `Selected`, `Admitted` and `Refused`.
2. An `ActionCatalogue` with two `CatalogueEntry` values, one `Admissible` and one
   `ApprovalRequired`, is built through `b10x-loom`'s re-export of the generated model, and its
   `entries` read back in order.
3. Each of the eight module files exists and `crates/loom/src/lib.rs` declares each with `pub mod`.
   (That the files hold only doc comments at delivery is checked by review of the diff, not by this
   test, which later stories keep green while they fill the files.)

## Source

Operator instruction of 2026-10-04 (waves hold several stories; spec-first skeleton story); Atlas
ADR 0076 and ADR 0080; the `## ESS first` sections of the four stories named in § Declarations.
