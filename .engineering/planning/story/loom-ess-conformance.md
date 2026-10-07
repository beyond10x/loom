---
format: aep.planning-md/3
id: story:loom-ess-conformance
kind: story
status: implemented
title: Carry the Loom ESS specification to a synthesized conformance suite in task check
refs:
- provider: taskboard
  reference: I-006
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:frontier-projection
- depends_on: story:action-selector
- depends_on: story:argument-generator
- depends_on: story:selection-revalidation
- depends_on: story:session-transcript-streaming
- depends_on: story:compaction-contract
- depends_on: story:interruption-recovery
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:harness-loop-port
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-crate-port
- depends_on: story:ess-055-upgrade
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-conformance/
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/src/session.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: ess/SKIPPED.md
- confidence: cited
  path: website/docs/reference/crates.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
- {from: "proposed", to: "active", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
- {from: "active", to: "implemented", at: "2026-10-07T06:42:43Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"test_result":1,"review_outcome":10,"verification":1}}, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
---
## Outcome

`task check` holds the Loom ESS specification (`ess/`), complete for this epic, to its synthesized
conformance suite, run against `b10x-loom-executor`:

- **Where the suite comes from.** `ess verify conform synthesize --path ess` writes it, into the test
  crate's own build directory on every run, so it tracks `ess/` without a committed copy to drift.
- **What it runs against.** `b10x-loom-executor` (the library that implements the nine `loom.run` commands), through a Rust target in a new crate,
  `crates/loom-conformance/` (package `b10x-loom-conformance`), built on the `ess-conformance` crate
  as a git dependency at the ESS tag `story:ess-055-upgrade` pins (`0.55.0`, the `requires:` of `ess/ess-inputs.yaml`),
  pinned by `Cargo.lock`. This is the route Mandate takes in `crates/mandate-conformance/`
  (mandate `95e0a4b`) and commission `story:commission-ess-conformance` plans. ESS's Go and
  TypeScript packages are not used: anything committed here that runs is Rust (`AGENTS.md` § Rules).
  `decision-blocker:rust-conformance-target` is cleared with this answer.
- **How it is started.** A new task `conform` runs `cargo test -p b10x-loom-conformance --test
  conform`, and `check` lists it as its own step.
- **How the verdict is read.** From the report document, not the runner's exit code.
- **Skips.** This story creates `ess/SKIPPED.md`: a header saying each line names one skipped
  scenario and why, then the list. A line is added only for a scenario Loom cannot answer.
- **Markers.** None: `ess/` carries no `UNMAPPED:` marker. `story:ess-hard-gate` closed the last
  ones (the catalogue marker by the operator decision of 2026-10-04 on
  `decision-blocker:catalogue-ownership`: one catalogue per turn), and its `task ess-gate` refuses
  any new one, so this story checks none.

The model drift gate is `story:agent-executor`'s (`task drift`), and the validate / compile /
synthesize / no-marker gate is `story:ess-hard-gate`'s (`task ess-gate`), not this story's.

## Shared surface

The last story of `epic:loom-native-harness`: it depends on every other open story of the epic,
because the suite runs against the whole of `ess/` as they leave it. It edits `ess/domains/run.yaml`
and `generated/rust/loom/` only when a scenario shows the specification wrong, and `Taskfile.yml`
and `Cargo.lock` after `story:agent-executor` and `story:harness-crate-port`. The whole order is in
`story:agent-executor` § Shared surface.

## ESS first

The suite covers the whole of `ess/`: `ess-inputs.yaml`, `system.yaml` and `domains/run.yaml`. This
story changes the specification only when a scenario shows it is wrong; then `task ess-gate` passes
and `task generate` regenerates. Every synthesize `note:` is relayed in the closing report.

- **Specification change:** none planned — this story holds the implementation to the specification
  the other stories leave; a change is made only when a scenario shows the specification wrong, and
  then that change is its own first commit, red on `task drift`.
- **Red test:** the first commit adds `crates/loom-conformance/` with the test
  `ess_conformance_report` and an empty Rust target; it fails on that commit because the target
  answers no command, so the report records failed scenarios (item 3). The target commits make it
  pass.

## Scope

Derived 2026-10-06 by `story-scoper` at `04a1a73`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-conformance/` (new; package `b10x-loom-conformance`: `Cargo.toml`, `src/lib.rs`, `tests/conform.rs`) — cited
- **Files:** `ess/SKIPPED.md` (new; `ess/` holds only `ess-inputs.yaml`, `system.yaml`, `domains/`, `commission/`, `intake/`) — cited
- **Files:** `Taskfile.yml` (existing): new task `conform`, one step in `check` (`Taskfile.yml:13-32`, beside `commission:conform` at line 26) — cited
- **Files:** `Cargo.lock` (existing): one new package entry — cited
- **Files:** `ess/domains/run.yaml`, `generated/rust/loom/` (existing): only when a scenario shows the specification wrong — cited
- **Symbols:** test `ess_conformance_report`; a `ess_conformance::ConformanceTarget` impl for the nine `loom.run.*` commands (`ess/domains/run.yaml:288-607`) — cited
- **Pattern:** `crates/loom-commission-conformance/` (`src/lib.rs` `CommissionTarget`, `tests/conform.rs` `ess_conformance_report`; `ess/commission/SKIPPED.md` for the header). It is in-repo and replaces the external mandate reference — inferred
- **Also likely:** `crates/loom-executor/`: all nine commands are implemented there (`src/session.rs`, `src/selection.rs`, `src/arguments.rs`), so the target binds to `b10x-loom-executor`, and any scenario that fails on the implementation (not the specification) is fixed there — inferred
- **Documents:** none
- **Confidence:** high. The story names every file, and the tree confirms which are new; only the executor line is inferred.
- **Would collide with:** any unit adding a step to `Taskfile.yml` `check` or changing dependencies in `Cargo.lock`; any unit editing `ess/domains/run.yaml` / `generated/rust/loom/` (`story:compaction-contract`, `story:interruption-recovery` cite both, already ordered by `depends_on`); any unit in `crates/loom-executor/` if a scenario fails there
- **Safety fact:** the new crate joins the workspace through `members = ["crates/*"]` (`Cargo.toml:3`), and `ess-conformance` / `ess-primitives` at tag 0.53.0 are already locked (`Cargo.lock:889-891`, `933-935`). So the root `Cargo.toml` is untouched and the lock gains one entry — step 2, unproven

Corrections to the sections above, found while scoping (2026-10-06):

- The ESS tag is `0.53.0`, not `0.52.0`: `ess/ess-inputs.yaml:2` reads `requires: ess 0.53.0`, and `Cargo.lock` locks `ess-conformance` at `0.53.0`. Build on the newest ESS release.
- "Run against `b10x-loom`": `b10x-loom` is the CLI binary (`crates/loom-cli/Cargo.toml:14`). The nine commands are implemented in `b10x-loom-executor`, and the Commission pattern binds its target to the library; the target binds to `b10x-loom-executor` — inferred.
- `story:commission-ess-conformance` does not resolve in this store since Commission moved in; its in-repo equivalent is `crates/loom-commission-conformance/`.
- Not established: whether any scenario fails today (`ess verify conform synthesize` was not run); whether `loom.run.RecordTurn` has an implementation outside the generated one (`generated/rust/loom/src/behaviour.rs:241`; nothing under `crates/` names it); `ess/system.yaml` is covered by the suite but not listed above.
- 2026-10-07: the corrections above are applied to the Outcome and Acceptance: the target binds to
  `b10x-loom-executor`, and the ESS tag is the one `story:ess-055-upgrade` pins (this story depends
  on it).

## Acceptance

`task check` runs the test `ess_conformance_report` in `crates/loom-conformance/tests/conform.rs`,
and it passes. It synthesizes the suite from `ess/`, runs it against `b10x-loom-executor` through the Rust
target, and checks:

1. The report records at least one executed scenario.
2. Every command in `ess specify compile --path ess --format json` has at least one scenario in the
   suite.
3. The report records 0 failed scenarios.
4. Every skipped scenario in the report is named in `ess/SKIPPED.md`; applied to a copy of the
   report that adds one skipped scenario `S` the file does not name, the check fails and names `S`.

## Source

TASKBOARD I-006; Atlas ADR 0071; workspace AGENTS.md § ESS drives every product repository;
`ess:specifying` § Conformance is a record, not a claim; the answer recorded on
`decision-blocker:rust-conformance-target` (2026-10-04).

## Scope confirmed (wave 2026-10-07-w2)

From the implementor's confirmation table and the merged commits (0706e2a, a650b1e, 9a96b0a):

- `crates/loom-conformance/` (new), `ess/SKIPPED.md` (new), `Taskfile.yml`, `Cargo.lock` (one
  entry), `website/docs/reference/crates.md` — cited, changed.
- Pattern `crates/loom-commission-conformance/` (inferred) — confirmed.
- Target binds to `b10x-loom-executor` (inferred) — confirmed.
- "All nine commands are implemented" in `session.rs`, `selection.rs`, `arguments.rs` (inferred) —
  **wrong**: `ess/` has 11 commands. `SelectAction` had no command implementation, `ReleaseSession`
  existed only on the on-disk `SessionFile`, and `ProjectCatalogue` only as `projection::project`;
  `RevalidateSelection` is in `revalidation.rs`. Changed as a result:
  `crates/loom-executor/src/selection.rs` (`select_action`, `chosen`) and
  `crates/loom-executor/src/session.rs` (`TurnRecord` answers ProjectCatalogue and ReleaseSession).
- Not in the first scope, changed: `crates/loom-executor/tests/crate_names.rs` (the fifteenth
  package), `crates/loom-executor/tests/adversary_w2_conformance_select_action.rs`, and the
  adversary files under `crates/loom-conformance/tests/`.
- `ess/domains/run.yaml`, `generated/rust/loom/` — not changed; the scenario that shows the
  specification's `not-in-frontier` text at odds with ESS is `story:revalidation-membership-conformance`.
