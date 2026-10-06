# Changelog

All notable changes to Loom are recorded here. Loom releases from source at bare-version tags such
as `0.1.0`; nothing is on a registry, and consumers pin a tag (`tag = "0.1.0"`). Changes collect
under **Unreleased** until the next release.

## [Unreleased]

### Added

- The local software-change slice keeps inspected file contents in a bounded run-local result
  store and sends previews with immutable references to the model. Argument generation can read
  selected ranges or JSON values and compose edit contents from references and literals. The
  expanded ordinary arguments reach Commission admission and the existing effect adapter.
- Edit bodies are represented by size and digest in later briefings. Test-result artifacts retain
  the runner's existing output tail and explicitly report partial capture. References expire with
  the briefing; there is no cross-run sharing or claimed task-quality/token-price improvement.

## [0.2.0] - 2026-10-06

Loom 0.2.0 runs the tests of a governed case inside embedded Substrate confinement by default,
keeps the slice's own git calls from running anything a workspace planted, lets a host run the
governor on its own reviewed protocols, trusted time and fallible storage, and lets Loom recheck
its selection against the governor's current frontier before proposing. Depend on it with
`tag = "0.2.0"`; nothing is on a registry.

### Added

- `Loom::with_governor` revalidates a selection against the governor's current frontier before
  proposing it. After the selection, Loom reads the case's frontier from the governor once and
  proposes nothing for a selection made at another case revision (`stale-revision`) or of an
  action that frontier's catalogue does not list (`not-in-frontier`); the refusal is recorded on
  the selection and in `Loom::revalidations`. A governor that cannot answer suspends the run for
  availability. A Loom made by `Loom::new` alone proposes as before, and Commission still
  rechecks every proposal before any effect. Under Commission's `run_until_blocked`, a case that
  moves between Commission reading its frontier and Loom's revalidation (while the selector
  selects or while arguments are generated) is judged on the frontier it left, so a run can end
  with no admissible action for a case that completed, or ask for evidence the moved case no
  longer needs; ending that needs the executor port to report a moved case to Commission.
- `CanonGovernor::with_protocol` compiles host-admitted protocols without replacing built-ins.
  `with_evaluation_time` accepts trusted freshness time, and `FallibleCaseStore` makes durable
  adapter failures explicit while preserving existing infallible `CaseStore` callers.
  Protocols with multiple capabilities on one action are refused because Commission's frontier
  can represent only one; capability requirements are never silently truncated.

### Security

- Tests use embedded Substrate 0.7.10 by default: no network, cleared environment, read-only source
  and Rust toolchain, workspace writes restricted to `target/`, 300-second timeout, 8 GiB memory
  and 2,048 processes. Dependencies must be fetched before a run. Confinement failure stops with
  `ConfinementUnavailable` (exit 3); one user systemd scope is attempted for delegation.
  `--confinement none` is an explicit opt-out, visible in output and observations.
- The SDK exposes an injectable `TestRunner`. Test observations include Substrate's actual
  applied record; a refused launch produces no test evidence. Intake's confinement types are
  generated from ESS, and all three systems now use ESS 0.53.0.

- The slice's own git calls run no workspace hook, no `core.fsmonitor` command and no signing
  program. A case does not open on a workspace whose own git configuration names a program (a
  filter driver, a credential helper, an SSH command and the like), and a later call is refused
  (`ExecuteError::HostGit`) once that configuration, its includes or the git directory changed,
  so a test command that writes `.git/hooks`, `.git/config` or `.git/commondir`, or an edit of
  an included work-tree file, cannot make the slice's git run its code.

## [0.1.0] - 2026-10-05

The first release of Loom, the runtime for governed agents. `b10x-loom run` routes an intent to an
engineering protocol, opens a governed case and runs it until it is blocked, and `b10x-loom-sdk`
embeds the same runtime in an application. Loom became the one runtime repository (Atlas ADR 0090):
Commission's contracts and runtime, the governor and intake moved in with their history, under
`loom-` package names, and their repositories are archived with a pointer here. Depend on it with
`tag = "0.1.0"`; nothing is on a registry.

### Added

- Releases at bare-version tags: `.github/workflows/release.yml` runs on each pushed tag, fails
  unless `loom-xtask release-check` finds the tag equal to the workspace version and a
  `CHANGELOG.md` entry for it, and runs Loom's check on the tagged commit.

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
