---
format: aep.planning-md/3
id: story:cli-run-holds-suspension
kind: story
status: active
title: b10x-loom run --state holds a Suspension on a withheld approval
relations:
- serves: vision:O1
- informed_by: story:approval-suspend-resume-slice
- depends_on: story:host-suspension-store
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/fixtures
- confidence: inferred
  path: crates/loom-cli/tests/slice_run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/case.rs
- confidence: cited
  path: crates/loom-intake-slice/src/clock.rs
- confidence: cited
  path: crates/loom-intake-slice/src/effect.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/verifier.rs
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: generated/rust/intake
- confidence: cited
  path: website/data/ess
- confidence: inferred
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/ess
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T07:10:02Z", actor: "human:timo", revision: 10}
- {from: "proposed", to: "active", at: "2026-10-13T07:35:42Z", actor: "human:timo", revision: 11}
---
## Outcome

`b10x-loom run … --state <dir>` keeps the case (`FileCaseStore`) and the commission's Suspension
(`story:host-suspension-store`) in `<dir>`, and runs the commission through `run_with_suspension`,
so a withheld approval leaves a Suspension a later `b10x-loom resume` can continue. It also stores
a resume context: what `resume` needs to rebuild the run without the run's arguments.

## Why

Today `run` uses `CanonGovernor` over `MemoryCaseStore` (`crates/loom-cli/src/main.rs:343`) and
`run_until_blocked` with `NoDelegatedAuthority` (`crates/loom-intake-slice/src/run.rs:476-484`); a
withheld approval prints `stopped: ApprovalRequired (<action>)` and exits 0, and nothing survives
the process (the commission id is a fresh UUID, `run.rs:930-937`). Decided 2026-10-13
(`decision-blocker:host-suspend-resume`): the resume context is an intake entity declared in
`ess/intake/`.

## ESS first

The first commit changes only `ess/intake/`: a resume-context entity (commission id, agent
revision, intent, route, workspace, test command) in `ess/intake/domains/routing.yaml`. On it
`task intake-drift` fails and the new test `run_with_state_holds_a_suspension_on_a_withheld_approval`
fails; later commits make both pass without changing `ess/`.

## Acceptance

On the `software.change/1` fixture, `b10x-loom run --state d` exits 0 printing
`stopped: ApprovalRequired (repository.merge)`, and `d` holds one `Pending` Suspension, the case
at the request's revision and the resume context naming that commission. Without `--state`,
`run` behaves as in 0.17.0.

## Scope

- `ess/intake/domains/routing.yaml`, `generated/rust/intake` (inferred)
- `crates/loom-intake-slice/src/run.rs` (cited: 476, 930)
- `crates/loom-cli/src/main.rs` (cited: 343), `crates/loom-cli/src/lib.rs` (cited)
- `crates/loom-cli/tests/slice_run.rs` (inferred)
- `website/docs/reference/cli.md`, `CHANGELOG.md` (inferred)

Build: about 7G with headroom.

## Scoping (2026-10-13)

- `FileCaseStore` implements only `FallibleCaseStore` (`crates/loom-governor/src/lib.rs:171`,
  `file_store.rs:318`), so the `S: CaseStore` bounds in `crates/loom-intake-slice/src/{run,intent,
  case,effect,executor,verifier,clock}.rs` widen to `FallibleCaseStore` (cited).
- A Suspension is held only on the `NeedsAuthority` path, when the agent proposes the merge
  (`crates/loom-commission/src/runtime.rs:977-989`); the `AwaitingApproval` path
  (`crates/loom-intake-slice/src/run.rs:551-560`) holds none. The test's scripted agent proposes
  `repository.merge` (inferred).
- The resume context holds, besides the six fields: the case id (the `Commission` is built before
  the Suspension is read, `run.rs:930-937`); the route as `PreparedIntent::handoff` already
  serialises it (protocol and binding: intent revision, catalog digests, context policy,
  threshold; `crates/loom-intake-slice/src/intent.rs:49-87`); `max_steps` (`run.rs:656`); the
  confinement mode and cgroup root (`crates/loom-cli/src/main.rs:392-421`); the agent model alias
  and `--catalog` path. The run id is in the Suspension. Fields are local newtypes, mirroring
  Commission's ids as `ess/domains/governor.yaml:3-6` does (inferred).
- No binary-level test reaches `stopped: ApprovalRequired (repository.merge)` today; it is reached
  at library level only (`crates/loom-cli/tests/slice_run.rs:116-170`,
  `crates/loom-cli/tests/run_events.rs:41-47`). A binary test replays SSE over a loopback
  `--catalog` socket as `crates/loom-cli/tests/catalog_route.rs:34-55` does, with new
  software-change fixtures; an in-process restart is not a restart (`git.rs:129` and the
  `fresh_uuid` counter `executor.rs:1074` are process-global) (cited).
- Also touched: `website/docs/reference/ess/`, `website/data/ess/` (regenerated from
  `ess/intake`), `crates/loom-cli/tests/fixtures/` (inferred).
