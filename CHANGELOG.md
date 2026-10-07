# Changelog

All notable changes to Loom are recorded here. Loom releases from source at bare-version tags such
as `0.1.0`; nothing is on a registry, and consumers pin a tag (`tag = "0.1.0"`). Changes collect
under **Unreleased** until the next release.

## [Unreleased]

## [0.4.0] - 2026-10-07

Loom 0.4.0 adds opt-in bounded working context, verified system time queries without a workspace,
and installed custom protocol definitions. Transient provider overload is classified correctly
through llm 0.3.1 and retried within a fixed attempt budget before output. Context reports count
every attempt, and tool effects are never replayed by model retries. Legacy context remains the
default. This is a source release; install or embed it from tag `0.4.0`.

- Recognize provider overload through the updated llm adapter and retry transient intake model
  failures for at most three attempts before output. Count every attempt in context reports, preserve
  cancellation and final error evidence, and never retry tool effects or actual refusals.

### Added

- `task install` rebuilds the current checkout and installs `b10x-loom` into `~/.local/bin`.

- Loom owns `system-query@1` with the governed read-only `system.time.read` action. `run` can
  answer local date/time and UTC without a workspace or confinement, and reports verified query
  completion as exit 0. Software changes still require a Git worktree and retain confinement.
- `b10x-loom-protocols` composes engineering, Loom and custom definitions into the same catalog
  used by routing and governor admission. `protocols add/list/remove` install validated local
  snapshots or regular Git blobs at full commits. Runs verify installed content and load offline;
  replacement is explicit and packages supply no executable tools. Catalog-aware SDK entrypoints
  preserve the legacy embedding callers.


- Opt-in `--context-policy bounded` uses report-derived working state, a recent-event tail and
  retrievable history in the local CLI slice. It retires history in batches above 48 KiB toward
  32 KiB and enforces a 64 KiB serialized request ceiling. The separate history archive is limited
  to 16 MiB and 4,096 events; capacity failures stop later model requests while preserving completed
  effects. Selection and arguments each support eight history/result lookups. `--context-report`
  writes payload-free request and provider-usage measurements, including failures after the slice
  starts. Embedders gain `run_with_options`; existing callers and the CLI default remain legacy.

- Commission declares the action-operation binding: a commission's composition binds a frontier
  action id to exactly one Connector operation (`instance_id`, `operation_id`), at most one binding
  per action id (`ActionBinding`, identity `(commission_id, action)`).
  `ports::connector::ConnectorEffects` is the Connector-backed `EffectPort` over a commission's
  bindings; it invokes each admitted request once through the new `ConnectorInvoker` port, and
  refuses bindings of another commission or a second binding for one action. The testkit adds
  `RecordingInvoker`, and `ScriptedExecutor` records the frontier each call was handed. No
  Connectors client fills the port yet.
- `task check` holds Loom's own specification (`ess/`) to its synthesized ESS conformance suite:
  `task conform` synthesizes the suite, runs it against `b10x-loom-executor` through the new Rust
  target `b10x-loom-conformance`, and reads the verdict from the report. All 46 scenarios pass and
  none is named in `ess/SKIPPED.md`. The target answers `loom.run.RevalidateSelection`'s
  `not-in-frontier` branch itself (it is `external:` in the specification); the executor's
  membership rule is held by `adversary_run_revalidation.rs`.
- `selection::select_action` answers `loom.run.SelectAction` over stored catalogues and selections,
  and the session record (`session::TurnRecord`) answers `loom.run.ReleaseSession` and
  `loom.run.ProjectCatalogue` through their generated behaviours.
- `crates/loom-executor/tests/connector_boundary.rs` holds the Connectors boundary: a consequential
  action a Loom run selects leaves Loom only as a `ProposedAction`, and `b10x-loom-executor`'s
  normal dependency graph holds no `connectors*` or `b10x-connectors*` package.

### Changed

- Before the executor runs, the Commission runtime removes from the frontier it hands over every
  action the effect port does not perform and that needs no authority, action by action. An action
  with an entry that needs approval or names a capability stays with all its entries, so
  `b10x-loom run` still lists `repository.merge (blocked)` and stops at
  `ApprovalRequired (repository.merge)`. The approval gate and the run outcome read the frontier
  the executor was handed.
- `EffectOutcomePerformed` names the Connector attempt the invocation produced (`attempt`, an
  optional `ConnectorAttemptId`, present exactly when the effect was invoked through a Connector),
  and the effect observation carries it only then. The local slice names none.
- `task commission:deps-guard` also refuses a Connectors crate in `b10x-loom-commission`.
- Loom requires ESS 0.55.0: the Loom, Commission and intake specifications require it, the
  Commission conformance target builds on the 0.55.0 `ess-conformance` and `ess-primitives`, and
  CI installs the 0.55.0 `ess`. The source formats are unchanged, and the generated Rust is the
  same as under 0.54.0: no specification has a `{generated: true}` value of an `Optional` type, so
  no context gains the new `generate_optional_<t>` port method.

### Fixed

- A compaction never leaves the session larger than it found it. The ported loop folded any
  non-empty summary, so a model that answered the summary request with more text than it was asked
  to fold grew the conversation (14,596 bytes became 17,170, 4,292 tokens of a 4,000-token window)
  and the next request was still sent. A summary no shorter than the items it would replace is now
  a failed summary (`summary-failed`): it is not kept, those items are elided instead behind one
  item beginning with `ELISION_MARKER`, and the usage the endpoint reported for the summary request
  is still recorded on the session. Where even that item would not be shorter (the items are
  reasoning items the loop carries verbatim), they stand.

## [0.3.0] - 2026-10-07

Loom 0.3.0 runs the ported Harness loop over a governed run: each turn's tools are the catalogue
projected from the governor's current frontier, a model's call of an action is revalidated before
Loom proposes it, and every turn and compaction is recorded on the run's session. A governed run
can be interrupted and resumed from its approval checkpoint. The local slice keeps inspected file
contents as run-local results that the model reads through bounded previews and reuses in edits
through exact references. Loom requires ESS 0.54.0. Depend on it with `tag = "0.3.0"`; nothing is
on a registry.

### Added

- `Loom::run_loop` runs the ported Harness loop over one frontier of a commission, and
  `harness::governed::LoopExecutor` runs it behind Commission's `AgentExecutor` port. Before every
  turn Loom reads the case's current frontier from the governor and projects it, and the turn's
  tool list is exactly that catalogue, each action under its published name
  (`harness::governed::tool_name`: `repository.merge` is published as `repository_merge`); the
  loop's own tools (an answer schema, delegation, skills, memories) are not published, whatever
  the caller's configuration says, so no delegate runs, and a narrowing in it (`admits`) keeps
  only the catalogue entries it names, turn by turn. A model's call of a catalogue action is the
  selection, by the reasoning model, and carries its
  arguments: Loom records the selection and the argument request, revalidates the selection
  against the governor's current frontier, and returns it as a `ProposedAction`, stopping the loop
  at an approval checkpoint before the effect. A call of anything outside the catalogue is refused
  to the model by name and proposes nothing. Each completed turn is recorded once into the run's
  session with the provider items it added (`loom.run.RecordTurn`, `Loom::turns`); one run holds
  a session at a time, a second run proposes nothing, and the session is filed when its run ends
  (`Loom::sessions`). Arguments the model wrote that Commission's JSON cannot carry end the run
  with `NoUsefulAction`. Budgets as a Commission suspension are not wired yet, and `b10x-loom run`
  does not use the loop yet.
- A governed run can be interrupted and recovered. `Loom::interrupt` cancels the run holding a
  session: the session becomes `Interrupted` (`loom.run.InterruptSession`), nothing more is
  recorded into it, the run proposes nothing, and a call it was selecting when the cancel came is
  stopped before revalidation, its selection left `Selected`. A run that stops at the approval
  checkpoint of a call leaves that checkpoint held for its session, in memory.
  `Loom::resume_loop` resumes a session by id, from `Filed` or `Interrupted`
  (`loom.run.ResumeSession`), and continues from its held checkpoint: the catalogue is projected
  from the frontier current at resume first, a selection in flight is revalidated, and a proposal
  already returned (a merge awaiting its approval) is returned again without asking the model
  while the case is at the revision it was selected at; otherwise the model is told the held call
  is stale, naming both revisions, and chooses again. A resume that ends before its held call is
  answered (a governor that cannot answer, an interrupt, a panic) keeps the checkpoint for
  the next; a resume whose narrowing admits a tool the stopped run's did not, or whose commission
  is for another case, fails as a changed configuration. `Loom::run_loop` still starts a
  conversation of its own and drops a held checkpoint. `Loom::catalogues` lists the catalogue
  each governed turn was offered, and the ported loop gains `AgentLoop::resume_asking`, which
  resumes a checkpoint by asking the approval port again.
- A governed run records each compaction of its session. The ported loop compacts before a
  request once the conversation passes 80 % of a declared context window, aiming at 50 % (the
  byte rule without a window); `Loom::run_loop` records each compaction on the run's session
  (`loom.run.RecordCompaction`, `Loom::compactions`) with the usage the endpoint reported for its
  summary request (`loom.run.ReportedUsage`), never an estimate, and `LoopEvent::Compacted` now
  carries that usage. The request after a compaction carries the run's standing instruction
  unchanged and the catalogue projected from the frontier current then; the model's summary stays
  conversation content and is not recorded as a turn. The session file format is unchanged.
- The local software-change slice keeps inspected file contents in a bounded run-local result
  store and sends previews with immutable references to the model. Argument generation can read
  selected ranges or JSON values and compose edit contents from references and literals. The
  expanded ordinary arguments reach Commission admission and the existing effect adapter.
- Edit bodies are represented by size and digest in later briefings. Test-result artifacts retain
  the runner's existing output tail and explicitly report partial capture. References expire with
  the briefing; there is no cross-run sharing or claimed task-quality/token-price improvement.

### Changed

- Each `Loom` derives its turn, catalogue, selection and argument-request ids in a namespace of
  its own, random by default and fixed with `Loom::with_instance`, so two Looms, a Loom built to
  recover a run among them, never give one id to two different selections on one frontier.
- Loom requires ESS 0.54.0: the Loom, Commission and intake specifications require it, the
  Commission conformance target builds on the 0.54.0 `ess-conformance` and `ess-primitives`, and
  CI installs the 0.54.0 `ess`. The source formats are unchanged, and the generated Rust is the
  same as under 0.53.0.
- The Taskfiles no longer set `CARGO_TARGET_DIR`: `task check` and every other task build into the
  work tree's own `target/`, so two work trees never share test binaries.

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
