# AGENTS.md — loom

What Loom is and how to build it is in [README.md](README.md); this file is what an agent changing it
must know. The cross-repository architecture is Atlas ADRs 0066–0075 and Atlas
`docs/design/governed-autonomy/`.

## Serves

- **O1 — governed reach.** The model only sees and can only execute actions the current frontier
  admits; authority is rechecked before every effect.
- **O3 — any harness, observed and compared.** Loom is the native executor Metaharness compares
  against other harnesses on the same case.

## Boundary

- Loom owns the model/tool loop (Atlas ADR 0071): prompt and context assembly, model turns, the
  projected action catalogue, action selection, argument generation, tool round trips, streaming,
  compaction, sessions and transcripts, budgets, interruption and recovery, model-facing approval
  suspension.
- Loom does not own protocol semantics, engineering semantics, case truth, organizational authority,
  connector credentials, final completion or system conformance semantics.
- Loom depends on Commission's core contracts and implements `AgentExecutor`. Commission core never
  depends on Loom (Atlas ADR 0075).
- `beyond10x/harness` is the predecessor. Port its implementation; do not rewrite from zero, and do
  not break its current consumers while they still depend on it. Harness is
  `LicenseRef-B10x-Proprietary`; code ported from it into Loom is relicensed Apache-2.0 (operator,
  2026-10-04). Harness itself keeps its licence.

## Rules

- Model-visible actions are derived from the current frontier and runtime capability; nothing
  consequential is granted because it was registered at startup (Atlas ADR 0072).
- A selector picks only from the candidate set it was given; unknown action ids are rejected;
  confidence never grants authority; low confidence falls back to a stronger path (Atlas ADR 0073).
- Revalidate every selected action against case revision, frontier and authority before execution.
- A trace is not evidence (Atlas ADR 0074).
- The model is not trusted context: it never supplies identity, authority, case revision, trusted
  time, approval results or tenant context.
- Do not make Loom semantically dependent on one selector vendor or model.
- Anything that runs is Rust; command lines use clap derive.

## ESS

Loom is specified in ESS under `ess/`. The domain is drafted and validated before any story
introduces a noun, and `task check` runs its conformance suite once synthesized. Change the
specification first.

The specification is a hard gate (Atlas ADR 0076). `task check` enforces it through
`task ess-gate`, which runs `crates/loom/tests/ess_gate.rs`; a failure of any of the four
conditions fails `task check`:

1. `ess specify validate --path ess --strict-requires` exits 0;
2. `ess specify compile --path ess --format json` exits 0;
3. `ess verify conform synthesize` exits 0 with 0 refusals;
4. no file under `ess/` contains `UNMAPPED:` — the specification carries no open question.

No story is implemented while the gate is red. The `UNMAPPED:` scan stands in for ESS until
ESS refuses open entries itself: it is removed when the ESS release that does
(beyond10x/ess `epic:typed-open-questions`) is pinned.

An open question is settled before the specification changes — in a story, or in a
`decision-blocker` when nobody has decided it — and is never written into `ess/` as an
`UNMAPPED:` marker.

## Work

- Planned in the AEP store under `.engineering/`, written only through `aep plan artifact`. Body
  drafts go in `.engineering/drafts/` (ignored).
- Build with `CARGO_TARGET_DIR=$HOME/.cache/b10x-target/loom` (the Taskfile sets it).
- `b10x-canon` and `b10x-commission` are git dependencies on `main`, pinned by `Cargo.lock`.
- Every commit and push is `b10x-bot[bot]`'s through `b10x-gates bot`; every GitHub write goes
  through `b10x-gates api`.
- Use a managed worktree (`worktree create --repo loom --purpose …`) for changes.
