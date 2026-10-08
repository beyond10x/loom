# Changelog

All notable changes to Loom are recorded here. Loom releases from source at bare-version tags such
as `0.1.0`; nothing is on a registry, and consumers pin a tag (`tag = "0.1.0"`). Changes collect
under **Unreleased** until the next release.

## [Unreleased]

## [0.11.0] - 2026-10-08

Loom 0.11.0 builds on engineering-protocols 0.3.0, whose `incident.response/1` keeps an
investigation open after emergency mode ends: the obligation `investigate_cause` stays open once
`emergency.leave` is admissible, until a cause analysis identifies the cause. Canon is now named
by its tag, `b10x-canon` `0.1.0`; a consumer that adds Canon itself moves to that tag, or it builds
a second Canon whose types do not match. Callers that read an incident case's frontier or
completion see one more obligation, and the engineering registry lists `support-triage@1`.

### Changed

- Loom builds on `b10x-canon-engineering` `0.3.0` from engineering-protocols (was `0.1.0`) and
  names Canon by the tag that release names, `b10x-canon` `0.1.0` (was `branch = "main"`). A
  consumer that adds Canon itself moves to `tag = "0.1.0"`, or it builds a second Canon whose types
  do not match.
- `incident.response/1` lists one more obligation, `investigate_cause`: open until a cause analysis
  of a current revision identifies the cause, and still open once `emergency.leave` is admissible.
  Callers that read an incident case's frontier or completion see it beside `restore_service`.
- The engineering registry, and so `ProtocolCatalog::engineering` and `ProtocolCatalog::bundled`,
  now lists `support-triage@1`; engineering entries carry revision `0.3.0`.
- `SessionFile::load` reads a filed session from its text. engineering-protocols 0.3.0 and Canon
  0.1.0 turn on `serde_json`'s `arbitrary_precision` in every build that links the governor; read
  through an untyped value first, a session whose tool call holds an integer above `u64::MAX`
  would be refused under it.

## [0.10.0] - 2026-10-08

Loom 0.10.0 lets a durable host report a failed write and keep a governor case across a process
restart. The commission specification declares a `storage-failed` outcome on `StartRun` and
`SuspendRun`: a run store that cannot write says so, `run_until_blocked` ends a failed start before
any executor, authority or effect call, and a failed suspension is reported as
`LoopFailure::RunStorage`, never as an obligation. `FileCaseStore` keeps each governor case in its
own file as the new `loom.governor` domain's `StoredCase`, so a case opened before a restart answers
the same revision, frontier and completion after it. Hosts that match `StartRunOutcome`,
`SuspendRunOutcome` or `LoopFailure` exhaustively add an arm, and ports other than `RunStore` under
the generated commission behaviour implement `StartRunBehavior` and `SuspendRunBehavior` themselves.

### Added

- `FileCaseStore` (`loom_governor`) keeps each governor case in its own file, written atomically
  as the `loom.governor` domain's `StoredCase`, so a case survives a process restart.

### Changed

- A run store can report that it cannot write. The commission specification declares the error
  `RunStorageFailed` and a `storage-failed` outcome on `StartRun` and `SuspendRun`, so
  `StartRunOutcome` and `SuspendRunOutcome` gain `StorageFailed`. `run_until_blocked` reports it
  as the new `LoopFailure::RunStorage`: a failed start ends the loop with no Run named before any
  executor, authority or effect call, and a failed suspension is carried in
  `LoopError::suspension`, no longer as `NotSuspended`. Callers that match these enums
  exhaustively add the arm. `StartRun` and `SuspendRun` are now host obligations: `RunStore`
  implements both, so `Generated<RunStore>` is unchanged, but the generated commission crate no
  longer has `Context`, `TryContext` or `unmet_context`, and other ports under `Generated` implement
  `StartRunBehavior` and `SuspendRunBehavior` themselves.

## [0.9.0] - 2026-10-08

Loom 0.9.0 lets a read leave Loom through Connectors. A read action bound to a Connector read
operation answers `Performed` naming the execution audit record Connectors completed for it, where
0.8.0 answered an error; a write still names its attempt. The commission specification declares
each binding's `effect` (`Read` or `Write`) and the read's `audit` on `EffectOutcomePerformed`, so
hosts that build `ActionBindingData` or `EffectOutcomePerformed` add a field. Both conformance
suites now carry declared coverage, and `task conform` passes only when ESS rates the run `passed`.
The two governor vertical slices, software change and incident response, run end to end as tests.

### Changed

- `task conform` and `task commission:conform` synthesize their suites with declared coverage
  (`ess verify conform synthesize --suite-format 5`) and fail unless the report's
  `conformance_status` is `passed`. A run whose scenarios all pass over a suite without a coverage
  inventory, which ESS rates `inconclusive`, no longer counts as green, and neither does a run with
  an `unsupported` scenario named in `ess/SKIPPED.md`, which ESS rates `failed`.
- A read action bound to a Connector read operation answers `Performed` with the operation's
  result. `ActionBinding` declares the bound operation's `effect` (`Read` or `Write`), and
  `EffectOutcomePerformed` names the read's execution audit record (`audit`, the `audit_ref`
  Connectors completed) where a write's names its attempt. `ConnectorEffects` requires exactly the
  one the binding's effect names; a consequential (`Write`) action still answers an error when
  Connectors records no attempt. `ConnectorsInvoker` refuses, before invoking, a binding whose
  effect is not the one the service describes (`Write` for the `mutation` profile, `Read`
  otherwise). Hosts that build `ActionBindingData` or `EffectOutcomePerformed` add the new field,
  and `BindingError::OtherCommission` now boxes the binding it names.

## [0.8.0] - 2026-10-08

Loom 0.8.0 lets a commission's bound actions leave Loom through Connectors. The new crate
`b10x-loom-connectors`, re-exported as `loom_sdk::connectors`, fills Commission's
`ConnectorInvoker` over a Connectors `v0.35.0` service: it invokes each admitted request once on
`POST /v1alpha2/invoke` and answers `Performed` with the Connector attempt the service recorded,
`Refused` only when Connectors states nothing was performed, and an error otherwise. The commission
specification declares the host's Connector endpoint per instance. A read operation through the
invoker answers an error in this release, because Connectors records no attempt for reads.

### Added

- `b10x-loom-connectors` (`crates/loom-connectors`, re-exported as `loom_sdk::connectors`) fills
  Commission's `ConnectorInvoker` over a Connectors service. `ConnectorsInvoker` resolves a
  binding's `instance_id` to the one endpoint the host declares for it, resolves the endpoint's
  credential reference through the host's `CredentialResolver`, describes the service and invokes
  the bound operation once on `POST /v1alpha2/invoke`, with the admitted request's arguments. It
  answers `Performed` naming the attempt Connectors recorded, `Refused` only when Connectors
  reports that attempt `refused` or `not_attempted`, and an error for everything else: a success
  that names no attempt, an error without a recorded attempt, an unknown outcome, a protocol or
  transport failure. An unknown instance, an unresolvable credential and a service that describes
  another instance are errors before the operation is invoked, and so is an argument number that
  cannot be sent without changing its value, such as an integer beyond the 64-bit range. A
  success naming another instance's attempt is an error. It never falls back to `/v1/invoke` and
  never resends. Known limit: a read operation answers an error, because Connectors `v0.35.0`
  records no attempt for a read and the port requires an attempt on every `Performed`.
- `commission.responsibility.ConnectorEndpoint` in `ess/commission/domains/responsibility.yaml`:
  the host's Connectors endpoint for one Connector instance, with its URL
  (`ConnectorEndpointUrl`), a non-secret credential reference (`ConnectorCredentialRef`) and
  whether plain `http` is admitted. `ActionBinding.endpoint` references it through `instance_id`.

### Changed

- Loom now depends on Connectors `v0.35.0` (`connectors-client`, `connectors-core`), through
  `b10x-loom-connectors` only. `b10x-loom-commission` and `b10x-loom-executor` still link no
  Connectors crate.

## [0.7.0] - 2026-10-08

Loom 0.7.0 makes Loom usable by a supervisor that is not written in Rust and by a model that is not
a Codex model. `b10x-loom run --output jsonl` writes the run as a versioned event stream with one
terminal record. A ported wire can take its credential from an llm credential reference, such as an
OAuth subscription login, with no secret in any configuration. `loom_governor::evaluate` and
`b10x-loom evaluate` decide a case snapshot and its evidence against a catalog protocol with no
Commission type in the signature. `b10x-loom run --catalog` drives a run through llm catalog route
aliases, for example a self-hosted endpoint. Every llm dependency moves to `0.4.0`.
`harness::wire::CredentialKind` no longer implements `PartialOrd`, `Ord` or `Hash`.

### Added

- A ported wire can take its credential from a reference instead of a held value.
  `credentials::ResolvedBearer` is a `BearerSource` that resolves a `WireCredential` (a
  `CredentialReference` and a `CredentialKind`, declared in `ess/domains/run.yaml` as
  `loom.run.WireCredential`) through an llm `b10x-llm-credentials` `SecretResolver` the embedder
  injects, at each call. A run can use an OAuth subscription login, such as a Codex login through
  llm's `CodexAuthFile`, with no secret value in any configuration. A missing reference or a
  resolver refusal stops the call with an `Unauthorized` error that names the reference and never
  the value, and so does a resolved secret that is empty, not UTF-8 or contains a control
  character (a key file written with `echo` ends in a newline and is refused); none is retried.
  Each call blocks the calling thread until the resolver answers or `credentials::RESOLVE_BOUND`
  (30 s) passes, then refuses; on a current-thread runtime, a resolver that needs that runtime
  cannot finish, so call the wire from `spawn_blocking`. `credentials::encode_wire_credential` and
  `decode_wire_credential` write and read the record as
  `{"reference": …, "kind": "oauth" | "api-key"}`; a decode error names the field and never echoes
  a value. The executor depends on
  `b10x-llm-credentials` at tag `0.4.0`.
- `b10x-loom run --output jsonl` writes the run as one JSON object per line on standard output
  instead of the human lines, for a process that supervises it. Every line carries
  `schema_version` 1 and a `kind`: `Route`, `Turn`, `ToolCall`, `Usage`, `Approval`, and
  exactly one `Terminal` record, last, with `stop_reason` and `exit_status`, the status the
  process exits with; a failed run's terminal record has `exit_status` 1 and its `error`. The
  records are declared as the `intake.events` domain (`ess/intake/domains/events.yaml`) and
  documented on the Run events reference page, which `task docs-check` holds to that
  declaration. Human output stays the default and is unchanged. `SliceRun` gains `approvals`,
  the actions a run stopped to await approval for.
- `loom_governor::evaluate(&ProtocolCatalog, &EvaluationRequest)` decides a case the caller keeps
  itself, with no Commission type in its signature and no case store: a protocol named
  `<name>@<major>` from the host's catalog, a `canon-case/1` snapshot, the `canon-evidence/1`
  records and an optional trusted time go in; Canon's decision comes out as `CanonGovernor`
  reports it (each action's status, required capabilities and reasons, claims, obligations, the
  one legitimate outcome when complete) with the whole `canon-decision/1` document beside it. It
  runs the governor's own evaluation, holds nothing and reads no clock. An unusable input is
  refused naming it: the protocol, the snapshot (or a termination the records do not make
  legitimate), an evidence record by position (unreadable, or repeating an earlier id), or the
  time. A readable record that does not apply to the case is set aside, as `CanonGovernor` sets
  it aside, and is not listed in the decision. Unlike `CanonGovernor`, which sets an unreadable
  record and the later of two records with one id aside and still decides, `evaluate` refuses
  both (`duplicate-identifier`, the later position). The types are the new `loom.evaluation`
  domain (`ess/domains/evaluation.yaml`), re-exported as `loom_governor::model`.
- `b10x-loom evaluate` is the same call over JSON: the request on standard input or from
  `--input <PATH>`, the decision on standard output (exit 0), or the refusal on standard output
  with the input named on standard error (exit 3). `--input` must name a regular file: a named
  pipe, a device or a directory is refused naming the path (exit 1) instead of being waited on.
- `b10x-loom run --catalog <PATH>` reads an llm catalog (`llm.catalog/1` TOML), and
  `--model` and `--classifier-model` then each name one of its route aliases instead of a Codex
  model, both of them. Each model is the port llm's `b10x-llm-models` builds for the route's first
  target, so a self-hosted Chat Completions, Responses or Messages endpoint can drive a run.
  Before any model call, the run stops with exit status 1, naming the flag and the alias, on an
  alias the catalog does not declare, a route that permits fallback to a second target, an
  account that needs a credential (the command line supplies no credential resolver), or a model
  that cannot take a forced tool call; a catalog that cannot be read or is not valid is refused
  naming the file. The catalog must be a regular file: a named pipe, including the one
  `--catalog <(cmd)` hands the run, a device or a directory is refused naming the path, before
  it is read. Without `--catalog` both flags name Codex models as before.

### Changed

- Every llm dependency moves from tag `0.3.1` to `0.4.0`; no existing caller needed a change.
  `b10x-loom-cli` adds `b10x-llm-models`, `b10x-llm-routing`, `b10x-llm-http` and
  `b10x-llm-credentials`, and `libc` for opening a named file without waiting on a pipe.

- `harness::wire::CredentialKind` is now the generated `loom.run.CredentialKind`. It no longer
  implements `PartialOrd`, `Ord` or `Hash`.

## [0.6.0] - 2026-10-08

Loom 0.6.0 names the outcome of a run whose case moved on. Commission's `RunOutcome` gains
`CaseMovedOn`, which names the Run's revision and the current one when the case moved to a revision
whose frontier still admits an action; before, such a run ended `NoAdmissibleAction`, which read the
same as an empty frontier. A Run stays bound to one revision, so the caller starts a new Run at the
current one. Callers that match `RunOutcome` exhaustively must handle the new variant. Loom now
requires ESS 0.56.0. This is a source release; install or embed it from tag `0.6.0`.

### Added

- Commission's `RunOutcome` has a `CaseMovedOn` variant (`RunOutcomeCaseMovedOn`,
  `bound_case_revision`, `current_case_revision`); callers that match `RunOutcome` exhaustively
  must handle it. A run whose case moved to a revision whose frontier, as the executor would be
  handed it, still admits an action now ends `CaseMovedOn`, naming the Run's revision and the
  current one, instead of `NoAdmissibleAction`, which read the same as an empty frontier. A Run
  stays bound to its revision; start a new Run at the current one. This holds whether the executor
  reported the move (`CaseMoved`) or the runtime found it itself, as after a stale proposal; there
  the runtime now reads the current frontier before ending the run. A moved case whose frontier
  admits nothing still ends as before.
- The intake slice stops a run that ends `CaseMovedOn` as `NothingAdmissible`, and its stop line
  names both revisions: `stopped: NothingAdmissible (case moved on from revision <bound> to
  <current>)`.

### Changed

- Loom requires ESS 0.56.0: the Loom, Commission and intake specifications require it, the
  conformance targets build on the 0.56.0 `ess-conformance` and `ess-primitives`, and CI installs
  the 0.56.0 `ess`. Every specification validates under 0.56.0's new ordering rule for held-state
  branches (`ESS-COMMAND-004`) without change, and the generated Rust is the same as under 0.55.0;
  only the Commission domain graph page names the new compiler.

## [0.5.0] - 2026-10-08

Loom 0.5.0 judges a run whose case moved under its executor on the case as it is now. Commission's
`ExecutorOutcome` gains `CaseMoved`, by which an executor reports the move, naming the revision it
was handed; callers that match `ExecutorOutcome` exhaustively must handle it. The governor decides
whether the case moved, and `run_until_blocked` reloads it before ending a run that proposed
nothing, so a case completed meanwhile ends `Completed` instead of asking for evidence it no longer
needs. `task docs-check` no longer shares ESS output with other processes. This is a source
release; install or embed it from tag `0.5.0`.

### Added

- Commission's `ExecutorOutcome` has a `CaseMoved` variant (`ExecutorOutcomeCaseMoved`,
  `expected_case_revision`): an executor reports that the case moved to another revision while it
  worked, naming the revision of the frontier it was handed. On it, `run_until_blocked` loads the
  case once more, and the governor decides whether it moved: still at the Run's revision, the step
  counts as `NoUsefulAction` and the run goes on. Moved, the run is judged on the frontier current
  then, as the executor would be handed it: a complete case ends the run `Completed`; otherwise
  the outcome is derived from that frontier, and a frontier that would let the run go on ends it
  with no admissible action, since the Run holds the case at the revision it left. The executor's
  revision is observed, never trusted.

### Fixed

- A Loom built with `Loom::with_governor` reports a selection made at a revision the case has left
  (`stale-revision`) as `CaseMoved` instead of `NoUsefulAction`. Under `run_until_blocked`, a case
  that moves while the selector selects or while arguments are generated is no longer judged on the
  frontier it left: a case completed meanwhile ends `Completed`, and a case moved past an open
  obligation ends on the obligation its current frontier holds. In the governed loop
  (`Loom::run_loop`) such a selection is still denied to the model, which chooses again from the
  next turn's catalogue, projected from the governor's current frontier. `LoopExecutor` reports
  `CaseMoved`, naming the revision of the frontier it was handed, for a run in which a selection
  was refused `stale-revision` or a turn's catalogue was projected at another revision, when that
  run ends with a proposal, `CompletedLocalReasoning` or `NoUsefulAction`: no proposal reaches
  Commission that was not selected and admitted at the revision the Run holds.
  `run_until_blocked` no longer ends a run on the frontier the case left when an executor proposed
  nothing without noticing the move: before it ends a run on the outcome derived from the frontier
  the executor was handed after `CompletedLocalReasoning` or `NoUsefulAction`, it loads the case
  once more, and judges a case that moved as on a `CaseMoved` the governor bears out.
- `task docs-check` writes ESS output under the workspace's build directory
  (`target/loom-docs/<run>/`), a parent each run owns alone, instead of the system temporary
  directory, where another process's `ess` run held the output lock and failed the check
  ("output ownership busy", os error 11).

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
