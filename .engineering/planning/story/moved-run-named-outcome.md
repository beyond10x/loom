---
format: aep.planning-md/3
id: story:moved-run-named-outcome
kind: story
status: implemented
title: A run whose case moved on to an admissible frontier ends with an outcome that says so
relations:
- decomposes: epic:commission-core
- serves: vision:O1
- depends_on: story:moved-case-outcome
- depends_on: story:ess-056-upgrade
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/
- confidence: cited
  path: crates/loom-commission-testkit/tests/moved_case_outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: docs/commission/contracts/commission-executor.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission/
- confidence: cited
  path: website/docs/reference/commission/
revision: 19
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T02:05:32Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-10-08T02:05:32Z", actor: "human:timo", revision: 11}
- {from: "active", to: "implemented", at: "2026-10-08T02:50:11Z", actor: "human:timo", revision: 19, decided_on: {"recorded":{"test_result":2,"review_outcome":1,"verification":1}}}
---
## Outcome

A run whose case moved to a revision whose frontier still admits an action ends with an outcome
that says so, instead of `NoAdmissibleAction`, which reads the same as an empty frontier.

After `story:moved-case-outcome` (wave 2026-10-07-w3), `run_until_blocked` reloads a moved case and
judges the run on its current frontier: a complete case ends `Completed`, an open obligation the
frontier holds ends `NeedsExternalEvidence`. When that frontier admits an action, the Run, bound to
the revision it left, cannot go on, and it ends `NoAdmissibleAction`
(`crates/loom-commission/src/runtime.rs`, `moved_outcome`; asserted on purpose by
`crates/loom-commission-testkit/tests/moved_case_outcome.rs`). Adversary pass 1 of that unit
(`review-result:adversary-w3-loom-moved-case-outcome-pass-1`, F3) measured it; the ambiguity is the
one `decision-blocker:run-stale-outcome` was filed for. The stale-proposal path, where the runtime
finds the move itself, ends the same way.

Decided (option A of `decision-blocker:moved-run-admissible-frontier`): a new `RunOutcome` variant
naming the revision the Run is bound to and the current one; the caller starts a new Run at the
current revision. A Run stays bound to one revision.

## ESS first

- **Specification change (first commit, `ess/commission/domains/responsibility.yaml` only):**
  `commission.responsibility.RunOutcome` gains the variant `CaseMovedOn:
  commission.responsibility.RunOutcomeCaseMovedOn`, a struct with `bound_case_revision` and
  `current_case_revision`, typed as the `case_revision` the specification already declares.
  Modelled with `ess:specifying` and validated with `ess` 0.56.0
  (`ess specify validate --path ess/commission --strict-requires`), the release
  `story:ess-056-upgrade` moves Loom to.
- **Red on that commit:** `task commission:drift` fails against the committed
  `generated/rust/commission/`; record the run.
- **Then:** `task commission:generate`, `task commission:docs`, the runtime, and the callers below.
  Later commits do not change `ess/`.

## Acceptance

- When a run's case moved (reported by the executor as `CaseMoved`, or found by the runtime on a
  stale proposal) to a revision whose frontier, filtered as the executor would be handed it, still
  admits an action, `run_until_blocked` ends the run `CaseMovedOn` with `bound_case_revision` the
  Run's revision and `current_case_revision` the revision it loaded. One test per path asserts it:
  the executor-reported move and the stale-proposal path.
- The case in `crates/loom-commission-testkit/tests/moved_case_outcome.rs` that asserts
  `NoAdmissibleAction` for that frontier asserts `CaseMovedOn` instead, with both revisions.
- A complete case still ends `Completed` and an open obligation still ends `NeedsExternalEvidence`
  on the same paths; the existing cases for them pass unchanged.
- Every match on `RunOutcome` outside tests handles the variant: the intake slice
  (`crates/loom-intake-slice/src/run.rs`) maps it as it maps `NoAdmissibleAction` (a stop that is
  not a success) and its printed stop line names both revisions; the Commission conformance target
  and the SDK example compile against it.
- `task commission:conform` passes; CHANGELOG (Unreleased) names the variant for callers that
  match `RunOutcome`, and the release notes of the release that carries it do too.

## Scope

- `ess/commission/domains/responsibility.yaml` (cited), `generated/rust/commission/` (by task),
  `website/docs/reference/commission/` (by task)
- `crates/loom-commission/src/runtime.rs` (cited: `moved_outcome`), `crates/loom-commission/src/outcome.rs`
  (inferred: the derivation that returns `NoAdmissibleAction`)
- `crates/loom-commission-testkit/tests/moved_case_outcome.rs` (cited)
- `crates/loom-intake-slice/src/run.rs` (cited: the `RunOutcome` match at the stop line),
  `crates/loom-commission-conformance/src/lib.rs`, `crates/loom-sdk/examples/software_change.rs`
  (cited: they match `RunOutcome`)
- `CHANGELOG.md`, `website/data/status.json` (coordinator)

## Shared surface

Edits `generated/rust/commission/` and Commission's gate surface like `story:ess-056-upgrade`, on
which it depends; the two run in order, not in one parallel set.

## Source

`review-result:adversary-w3-loom-moved-case-outcome-pass-1`, F3;
`decision-blocker:moved-run-admissible-frontier` (option A).

## Scope confirmed

Read from `git diff --stat f62ce58 9ed4653` (the unit's six commits, the adversary's test commit and
the correction; 23 files) at the close of wave 2026-10-08-w1. Corrections to the `## Scope` section
above:

| Scope line | Confidence then | What the unit changed |
|---|---|---|
| `crates/loom-commission/src/outcome.rs` | inferred | unchanged; the derivation is in `runtime.rs` (`found_moved`, `moved_outcome`) |
| `crates/loom-commission-conformance/src/lib.rs` | cited | unchanged: it does not match `RunOutcome` |
| `crates/loom-sdk/examples/software_change.rs` | cited | unchanged: its `other =>` arm covers the variant |
| `crates/loom-commission-testkit/tests/moved_case_outcome.rs` | cited | as cited |
| `crates/loom-commission/src/runtime.rs`, `crates/loom-intake-slice/src/run.rs`, `ess/commission/domains/responsibility.yaml`, `generated/rust/commission/`, `website/docs/reference/commission/` | cited | as cited (`types.mdx`) |
| not listed | — | `CHANGELOG.md`, `docs/commission/contracts/commission-executor.md`; testkit tests re-pinned to the variant: `adversary_loop_runtime.rs`, `adversary_runtime_effect.rs`, `runtime_effect.rs`, `runtime_loop.rs`, `skeleton.rs`, `run_outcomes.rs`; the adversary's `adversary_w1_moved_run_named_outcome.rs` |
