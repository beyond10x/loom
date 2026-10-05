# Changelog

All notable changes to Loom are recorded here. Loom has no release yet: the workspace is at
version `0.0.0`, nothing is on a registry, and consumers depend on it by Git revision. Entries
accumulate under **Unreleased** until the first release tag.

## [Unreleased]

Loom became the one runtime repository (Atlas ADR 0090): Commission, the governor and intake moved
in with their history, and their repositories are archived with a pointer here.

### Added

- `b10x-loom run` (package `b10x-loom-cli`): routes an intent to an engineering protocol, opens a
  governed case on it and runs Loom until the run is blocked, printing each step and a last line
  `stopped: <reason>`. Exit status 0 when the run stopped at its human gate (`ApprovalRequired`),
  3 for another stop, 1 for a failure, 2 for a command line that is not valid. It never merges,
  pushes or deploys, and it has no sandbox.
- `b10x-loom-sdk`: one crate to embed the governed runtime. It re-exports Commission's contracts
  and runtime (`commission`), the executor (`loom`), the governor (`governor`) and intake
  (`intake::router`, `intake::slice`), and adds no behaviour. `examples/software_change.rs` runs a
  whole embedding over scripted fake models, without network or login.
- Commission's contracts and runtime, as `b10x-loom-commission`: the responsibility model generated
  from `ess/commission/`, the governor, authority, executor, observation, evidence and effect
  ports, frontier admission, action-request revalidation against the current case revision, run
  outcomes, and `run_until_blocked`. `b10x-loom-commission-testkit` holds fakes of every port and
  conformance kits for governor and authority adapters; `b10x-loom-commission-conformance` holds
  the crate to its synthesized ESS suite.
- The runtime invokes admitted effects: `run_until_blocked` hands each admitted action request to
  an `EffectPort` once, records the effect's observation, and adopts the case's new revision only
  after a `Performed` effect. It stops at `AwaitingApproval` (checked before the step budget),
  `NoPerformableAction`, `NeedsAuthority`, the step budget, or after 64 steps when no budget is
  set.
- `b10x-loom-governor`: `CanonGovernor` evaluates a case's protocol with Canon behind Commission's
  governor, evidence and observation ports, and executes nothing.
- Intake: `b10x-loom-intake-references` extracts tracker keys, chat permalinks, merge and pull
  requests and URLs from an intent, deterministically; `b10x-loom-intake-router` proposes the
  protocol an intent should run under and refuses a pick outside the registry or below the
  confidence threshold; `b10x-loom-intake-slice` opens the case, performs `software.change/1`
  actions in the given workspace through a local effect adapter, and submits evidence only from the
  test command's exit status. The slice stops at `ApprovalRequired` when a step leaves an approval
  gate unchanged.
- The `intake.routing` ESS domain in `ess/intake/`, validated by `task check`.
- JSON from a model or a provider nested deeper than 128 levels is refused with a typed error, in
  every build of the workspace.
- Documentation: a getting-started page, guides, and generated CLI and crate references on the
  site.

### Changed

- **Breaking:** every package carries a `loom-` name, and the intake and governor libraries are
  renamed with them:

  | Before | Now | Library |
  |---|---|---|
  | `b10x-loom` | `b10x-loom-executor` | `b10x_loom_executor` |
  | `b10x-commission` | `b10x-loom-commission` | `b10x_loom_commission` |
  | `b10x-commission-testkit` | `b10x-loom-commission-testkit` | `b10x_loom_commission_testkit` |
  | `b10x-commission-conformance` | `b10x-loom-commission-conformance` | `b10x_loom_commission_conformance` |
  | `b10x-governor` (library `governor`) | `b10x-loom-governor` | `loom_governor` |
  | `b10x-intake-references` (library `intake_references`) | `b10x-loom-intake-references` | `b10x_loom_intake_references` |
  | `b10x-intake-router` (library `intake_router`) | `b10x-loom-intake-router` | `b10x_loom_intake_router` |
  | `b10x-intake-slice` (library `intake_slice`) | `b10x-loom-intake-slice` | `b10x_loom_intake_slice` |
  | `b10x-intake-cli`, binary `b10x-intake` | `b10x-loom-cli`, binary `b10x-loom` | `b10x_loom_cli` |

- **Breaking:** `b10x-intake-model` is removed. Its forced tool call and Codex preset are
  `b10x-llm-tool-call` (`call_tool`, `codex_model`) from llm `0.1.7`.
- Loom builds on llm `0.1.7` (every `b10x-llm-*` crate) and on `b10x-canon-engineering` `0.1.0`
  from beyond10x/engineering-protocols, which replaces `b10x-els`.
- The slice has no loop of its own: Loom proposes each action, and the Commission runtime rechecks
  and invokes it (Atlas ADR 0082).
