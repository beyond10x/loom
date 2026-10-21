# Changelog

All notable changes to Loom are recorded here, in the format of
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). Loom releases from source at bare-version tags
such as `0.1.0`; nothing is on a registry, and consumers pin a tag (`tag = "0.1.0"`). Changes collect under
`[Unreleased]` until the next release; the detail of each change is in its story and commit history.

Every release names what breaks for a consumer in `**Breaking:**` bullets, or says `No breaking changes.` on a line.
`loom-xtask release-check` refuses an entry with neither, and refuses `No breaking changes.` when `consumers/pinned/`,
`crates/loom-sdk/tests/host_ports.rs` or `docs/contracts/run-events-v1.json` changed since the previous tag. Entries
before 0.22.0 predate that rule: there, `No breaking changes.` means the original entry named no break.

## [Unreleased]

## [0.32.0] - 2026-10-21

### Added

- `LoopContext::stop_policy` returns a `StopPolicy`: the host turns the approval gate off and sets the idle bound (story:stop-policy-gate-and-idle-bound)
- `StopPolicy.on_breach`: `KeepStep` performs a proposal after a spending limit or the deadline was passed; `DiscardStep` stays the default (story:run-budget-tokens-and-deadline)
- `CanonGovernor::with_completion_rule(CompletionRule::FirstDeclared)` completes on the first declared legitimate outcome (story:first-declared-completion)

### Changed

- **Breaking:** `JudgeIdleIteration` gains a `bound` input, which `outcome::judge_idle` judges against (story:stop-policy-gate-and-idle-bound)
- **Breaking:** `RunBudget` gains `max_tokens` and `deadline`, `BudgetLimit` gains `MaxTokens` and `Deadline` (story:run-budget-tokens-and-deadline)
- **Breaking:** `LoopFailure` gains `InvalidDeadline`: a `RunBudget.deadline` not RFC 3339 is refused before a Run starts (story:run-budget-tokens-and-deadline)
- **Breaking:** `EffectOutcomePending` gains `ends_run`: `true` ends the run `Waiting` for `Time` (story:pending-answer-ends-run)

### Fixed

- A timed-out `run --action-bindings` command is killed with every process it started (story:bound-command-process-group)
- `SubstrateRunner::run` takes the toolchain from the workspace, never from an inherited `RUSTUP_TOOLCHAIN` (story:confinement-adversary-runs)

## [0.31.0] - 2026-10-21

### Added

- A workflow's judgement step runs on any `AgentExecutor` the host supplies (story:judgement-step-any-executor)
- `WorkflowExecutor::with_executor(executor, definition, effects)`: a closure builds the host's executor per call
- `workflow::Judgement` gives it the workflow, the step, its instructions (with `<workflow-inputs>`) and its actions
- The host's executor is handed only the step's actions; another action, or reads proposed together, never return
- A host executor's usage through `run_with_usage` is the call's, kept in its run record, so a resumed run reports it
- `WorkflowExecutor::with_admitted(actions)` is the host's narrowing of judgement steps
- `b10x-loom-session-executor`: an `AgentExecutor` over one metaharness session (story:metaharness-session-executor)
- `SessionExecutor` returns the session's change as a `ProposedAction` with `{files}` arguments; an effect applies it
- `loom_sdk::session` re-exports it; the SDK stability page lists `SessionExecutor`, `Proposal` and `ProposedFile`
- The `loom.session` domain: `AgentSession`, `ProposeSessionChange`, `SessionChangeProposed` and `AgentSessions`
- `SessionExecutor::sessions` and `SessionExecutor::proposed` read the sessions it recorded, one id per call
- `SessionExecutor::run_with_usage` reports the session's tokens, duration and cost, charged to the run
- `ExecutorStep` carries `transcript_digest` and `summary`; the step's observation carries both, never as evidence
- `task conform` runs `loom.session.ProposeSessionChange` against the session executor

### Changed

- `WorkflowExecutor::new(governor, model, definition, effects)` runs judgement steps on the chooser over an llm `Model`
- The chooser sends a selection request among the step's actions, then an argument request, each led by its instructions
- A judgement answer without a usable call is no useful action
- `with_max_input_tokens` suspends the run with `LoopBudget` when a call reaches it or reports no final input tokens
- `CHANGELOG.md` follows Keep a Changelog 1.1.0 (story:changelog-keep-a-changelog)
- `loom-xtask release-check` reads `**Breaking:**` bullets or `No breaking changes.`, no longer a `Breaking:` sentence
- **Breaking:** `WorkflowExecutor` has one lifetime parameter instead of `<'l, S, G, V, M>`
- **Breaking:** `WorkflowExecutor::new` takes `&dyn Governor`, `&dyn llm_core::Model`, the definition and the effects
- **Breaking:** `WorkflowExecutor::new` no longer takes `loom`, a `ModelPort`, a `LoopConfig` or a session
- **Breaking:** the host's narrowing moves from `LoopConfig::admits` to `WorkflowExecutor::with_admitted`, by action
- **Breaking:** a judgement step sends two model requests per call (selection, then arguments) instead of one loop turn
- **Breaking:** `workflow::load` no longer refuses an action whose id cannot be a tool name
- **Breaking:** `ExecutorStep` gains `transcript_digest` and `summary`; a struct literal of it must name both
- **Breaking:** Loom requires ESS 0.67.0 (story:ess-067-upgrade); specifications built with Loom's tasks need it
- **Breaking:** Loom links Substrate 0.7.18 (`rev = "17e0e96946b9588c150298ac0f951211681b2043"`); a host linking `b10x-substrate-host` or `-wire` moves with it

### Removed

- **Breaking:** `WorkflowExecutor::into_model` is removed
- **Breaking:** Loom's port of Harness is deleted: `loom_sdk::loom::harness` and its modules (story:delete-harness-port)
- **Breaking:** `loom_sdk::loom::harness::wire::ModelPort` and `loom_sdk::loom::harness::wire::ToolPort` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::BearerSource` and `loom_sdk::loom::harness::wire::StreamSink` are removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::ApprovalPort` is removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::HookPort` is removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::TurnEnvironmentProvider` is removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::LoopSink`, `loom_sdk::loom::harness::turn_loop::LoopEvent` are removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::AgentLoop`, `loom_sdk::loom::harness::turn_loop::LoopConfig` are removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::Budget` and `loom_sdk::loom::harness::turn_loop::DenyAll` are removed
- **Breaking:** `loom_sdk::loom::harness::turn_loop::OutputSchema`, `loom_sdk::loom::harness::turn_loop::RunLedger` removed
- **Breaking:** the harness `RateCard`, `loom_sdk::loom::harness::turn_loop::RateCard`, is removed
- **Breaking:** the module `loom_sdk::loom::harness::wire` and `loom_sdk::loom::harness::wire::WireId` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::Item` and `loom_sdk::loom::harness::wire::Usage` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::ToolCall` and `loom_sdk::loom::harness::wire::ToolName` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::ToolOutcome` and `loom_sdk::loom::harness::wire::ToolSpec` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::TurnOutcome` and `loom_sdk::loom::harness::wire::TurnRequest` are removed
- **Breaking:** `loom_sdk::loom::harness::wire::WireError` and `loom_sdk::loom::harness::wire::WireErrorCode` are removed
- **Breaking:** `loom_sdk::llm::LlmModelPort` and the module `loom_sdk::llm` are removed; a host hands Loom its llm `Model`
- **Breaking:** `loom_sdk::loom::wire::ClaudeCodePort` and `loom_sdk::loom::wire` are removed (Claude Code is metaharness's)
- **Breaking:** `loom_sdk::loom::session::SessionFile` and the rest of `loom_sdk::loom::session` are removed
- **Breaking:** `loom_sdk::loom::compaction`, `loom_sdk::loom::credentials` and `loom_sdk::loom::recovery` are removed
- **Breaking:** `Loom::run_loop`, `Loom::resume_loop`, `Loom::interrupt` and `Loom::sessions` are removed
- **Breaking:** `Loom::turns`, `Loom::catalogues`, `Loom::compactions` and `LoopExecutor` are removed
- **Breaking:** `MeteredModelPort` is renamed `MeteredModel`, an llm `Model` only; its harness `ModelPort` impl is removed
- **Breaking:** `loom.run` drops `Session`, `Turn`, `Compaction`, their commands, events and errors, and `Sessions`
- **Breaking:** `loom_sdk::loom::model::run::SessionData` and `loom_sdk::loom::model::run::SessionId` are removed
- **Breaking:** `loom_sdk::loom::model::run::RunEnding` is removed, and so are `CompactionId` and `ReportedUsage`
- **Breaking:** `loom.run` drops `WireCredential`, `CredentialReference` and `CredentialKind`
- **Breaking:** `loom.run.LoopStop` keeps `max-input-tokens` and `budget-unobservable`, the stops a judgement step raises

## [0.30.0] - 2026-10-21

### Added

- The SDK stability page lists `AgentExecutor` as a port a host implements (story:agent-executor-host-port)
- It also lists `Governor`, `CredentialResolver`, `TestRunner` and `Briefing`; a change to any of them is named as a break
- `loom_sdk::serde_json` re-exports `serde_json`, whose `Value` a `Briefing` returns as an action's argument schema
- `ess/intake/` declares `KeepResumeContext` and two views (story:ess-covers-agent-sessions-and-intake)
- `ResumeContextStore::keep_context` answers `KeepResumeContext` with its declared outcome; `keep` is unchanged
- `b10x-loom-intake-conformance` holds the slice's stores to intake's suite; `task check` runs `task intake-conform`
- `b10x_loom_intake_slice::selector::send_with_retries`, the caller retries a plugin turn sends with
- `model_selector::AskError`, by which `answer`, `ask`, `select_once` and `arguments_once` report an unusable answer

### Changed

- A refused protocol registration names the registration its protocol id takes (story:registration-name-protocol-id)
- A protocol id registers with dots and underscores as hyphens and `/<major>` as `@<major>` (`self-improvement@1`)
- A registration refusal states the `name@major` rule and quotes at most 120 bytes of the refused name
- `ess/domains/metering.yaml` counts a call as one `turn` of an llm `Model`; the generated model does not change
- A plugin turn runs on the chooser over an llm `Model`, not the harness loop (story:chooser-executor-for-plugin-turns)
- A turn step sends a forced `select_action`, then an `action_arguments` request, earlier steps as one data block
- The llm-backed selector and argument generator move to `b10x_loom_executor::model_selector`, told a host's `Briefing`
- `b10x_loom_intake_slice::selector` keeps its paths: `ModelSelector` and `ModelArguments` are aliases there
- `b10x-loom plugin run` hands each turn the agent's llm `Model` and builds no `LlmModelPort`
- A turn answered without a usable call ends `stopped`; only a request never answered is `PluginError::Unavailable`
- `MeteredModelPort` is an llm `Model` over the host's model, one call per `turn` (story:metering-on-llm-model)
- A failed or dropped turn is a call with unknown tokens; a model metered twice on one meter counts each turn once
- A turn's usage counts only when final, with input and output tokens, priced at the served model; else cost is absent
- The rate card moves to `b10x_loom_executor::metering::RateCard` (also `loom_sdk::RateCard`), with `ModelRates`
- `model_selector::ModelSelector` is the `ReasoningModel` selector; an answer calling the forced tool twice is unusable
- A `ReasoningModel` selection record reports 0 tokens; the meter still counts its turns
- Loom requires ESS 0.66.0 (story:ess-066-upgrade); intake's specification moves to format `ess/26`
- **Breaking:** `b10x_loom_plugin::TurnModel` holds `model: Arc<dyn llm_core::Model>`, no harness `ModelPort` and name
- **Breaking:** a turn sends a selection and an argument request per step, not one request with every action as a tool
- **Breaking:** `TURN_MODEL_BUDGET` is 16, two requests per step
- **Breaking:** `MeteredModelPort::new` takes any value; it is an `llm_core::Model` over one that derefs to a `Model`
- **Breaking:** a host meters a selector or generator by handing it the metered model
- **Breaking:** `RunMeter::new` takes `b10x_loom_executor::metering::RateCard`, not `harness::turn_loop::RateCard`
- **Breaking:** `RateCard::price` takes `(model, &llm_core::Usage)`
- **Breaking:** specifications built with Loom's tasks need ESS 0.66.0

### Removed

- **Breaking:** `ReasoningModelSelector` is removed from `b10x_loom_executor` and `b10x_loom_executor::selection`
- **Breaking:** its replacement is `model_selector::ModelSelector`, told a host's `Briefing`

## [0.29.0] - 2026-10-20

### Added

- `EffectOutcome::Pending` (`EffectOutcomePending`): the work a request started is unfinished (story:pending-effect)
- `RunOutcome::Waiting` (`RunOutcomeWaiting`): a budget stopped the run while requests were pending, naming them
- `AgentExecutor::run_with_usage` returns an `ExecutorStep` with an optional `ExecutorUsage` (story:executor-reports-usage)
- `ChildCommand::with_ca_bundle(path)`: an egress child trusts the host's CA bundle (story:child-egress-ca-bundle)
- `SliceRun::unperformed`, and `unperformed_actions` on the jsonl terminal record, a new optional V1 field

### Changed

- Loom links llm 0.8.1 (story:llm-080-pin); the requests Loom sends do not change byte for byte
- llm's `Capabilities` gains `output_streaming` and `input_during_output`; a routes document or catalog may declare both
- A routes document or catalog declaring `input_during_output: true` or `output_streaming: false` is refused
- Responses `server_error`, `rate_limit_exceeded`, Messages `overloaded_error`, `api_error` before output are retriable
- A streamed Responses turn's text is what the sink was shown; Messages opening text reaches the sink as a `TextDelta`
- Messages `container`, `diagnostics`, `stop_details`, `usage.speed`, `toolset_name` and a `direct` `caller` are accepted
- `response.queued` is ignored, no longer an `unknown-stream-event` warning
- `NoPerformableAction` names the unperformed `actions` and the frontier's `open_claims` (story:unperformable-names-actions)
- A pending request is invoked again, revalidated each time, as a step and one trial, never an idle iteration
- A budget stop while a request is pending ends `Waiting`; the intake slice stops it `StepBudget`
- An executor's reported usage is charged to the run and its budget, never to an action; a breach suspends `RunBudget`
- `MeteredExecutor` adds an executor's usage to its step when called through `run_with_usage`
- **Breaking:** a host that links llm itself moves to llm 0.8.1 so it resolves one copy of each `b10x-llm-*` crate
- **Breaking:** a full `Capabilities { … }` literal names the two new fields or ends with `..Capabilities::text(…)`
- **Breaking:** `RunOutcome::NoPerformableAction` carries `RunOutcomeNoPerformableAction`, no longer `Unit`
- **Breaking:** a host that builds `SliceRun` sets `unperformed`
- **Breaking:** a found move onto a frontier the port performs none of ends `NoPerformableAction`, not `NoAdmissibleAction`
- **Breaking:** a host that matches `EffectOutcome` or `RunOutcome` exhaustively handles `Pending` and `Waiting`
- **Breaking:** `LoopEnd::requests`, `admitted` and `effects` list a request answered pending once per invocation
- **Breaking:** `LoopEnd::steps` counts the executor calls and every invocation again
- **Breaking:** an executor wrapping another forwards `run_with_usage`, or the inner usage reaches neither run nor meter
- **Breaking:** `LoopEnd::charges.run` includes an executor's usage beside its actions' sums
- **Breaking:** a step with no model call costs what its executor reported, or zero, with or without a rate card

## [0.28.0] - 2026-10-20

### Added

- `SubstrateRunner::run_child(&ChildCommand)` confines a program, never a shell line (story:sdk-confines-any-child)
- `loom_sdk::ChildCommand`; `loom_sdk::intake::slice::confinement` re-exports the Substrate types it is named with
- `ChildCommand::with_egress(EgressAperture)`: a child reaches one named host (story:harness-child-egress-profile)
- `ChildCommand::with_writable(path)`: a workspace directory a child may write (story:child-writable-workspace)
- `EffectOutcomeRefused.ends_run`: a final refusal ends the run `EffectRefused` (story:refused-effect-ends-run)
- `RunOutcome::IdleBound` names the idle iterations in a row that ended a run (story:idle-bound-outcome)

### Changed

- A child runs on a test's launch path and guarantees; an unserved guarantee is a `ConfinementError`, never unconfined
- Tests and children through one runner and its clones share one Substrate driver per workspace directory
- An aperture Substrate's daemon would never declare (no host or port, a zero ceiling, …) is refused `ScopeInvalid`
- An egress child's result counts only when Substrate's record is exactly the declared aperture
- An egress child gets no CA bundle yet, and Substrate serves one aperture per run
- A frontier that admits nothing still ends `NoAdmissibleAction`
- The slice maps `EffectRefused` to `StopReason::Refused` and `IdleBound` to `NothingAdmissible`; jsonl is unchanged
- The confinement code builds on Substrate 0.7.17; a record listing `read_only_files` is refused `CapabilityUnserved`
- **Breaking:** a test command or child whose program contains `=` or starts with `-` is refused `ScopeInvalid`
- **Breaking:** so `run --test-cmd 'NAME=VALUE cargo test'`, which ran with that variable set, is refused
- **Breaking:** a launch needing a new driver while another runs under its cgroup root is refused `CapabilityUnserved`
- **Breaking:** a host that builds an `EffectOutcomeRefused` sets `ends_run` (`false` keeps today's behaviour)
- **Breaking:** a host that matches `RunOutcome` exhaustively handles `EffectRefused` and `IdleBound`
- **Breaking:** `NetworkMode::Aperture` without an aperture is refused `ScopeInvalid` before start, not `CapabilityUnserved`
- **Breaking:** a run with no step budget that made no progress ends `IdleBound`, no longer `NoAdmissibleAction`
- **Breaking:** if the last idle iteration's performed effect completed the case, the run ends `Completed`
- **Breaking:** a host that builds Substrate's `AppliedConfinement` (`TestExecution::driver_record`) sets `read_only_files`

## [0.27.1] - 2026-10-20

### Changed

- Loom links Substrate 0.7.17: `b10x-substrate-host` and `b10x-substrate-wire`, pinned together
- The pin is `rev = "00fa634cb24eb307946bfe3bbe40142f4b7a469a"`, the commit tagged `0.7.17`
- `substrate_wire::AppliedConfinement` (`TestExecution::driver_record`) gains `read_only_files` (Substrate 0.7.13)
- The slice's runner leaves `read_only_files` empty, and the record's JSON omits it while it is empty
- **Breaking:** a host that links `b10x-substrate-host` or `b10x-substrate-wire` moves to 0.7.17, at the same `rev`
- **Breaking:** a host building a `substrate_wire::AppliedConfinement` struct literal adds `read_only_files`

### Security

- Loom 0.27.0 and earlier, on Substrate below 0.7.13: a confined child could write the read-only workspace and outside it through inherited descriptors; fixed by Substrate 0.7.13.

## [0.27.0] - 2026-10-20

### Added

- The bundled catalog gains `eval-gated-change@1` (`eval.gated_change/1`), at revision `0.9.0`
- `CanonGovernor::decision_document(&case, &frontier_id)` (story:governor-exposes-canon-decision)
- It returns the whole `canon-decision/1` document behind a frontier the governor issued, as `evaluate`'s JSON value
- `frontier` keeps each document in memory under the id it issues, so a lookup ignores trusted time moved since
- A lookup never evaluates: an id the governor did not issue, or no longer keeps, is `None`
- `loom_governor::KEPT_DOCUMENTS_PER_CASE` (16): documents kept per case, dropped once the held case changes
- `StaticAuthorityProvider::requests` records each request the testkit's provider was asked about

### Changed

- Loom links engineering-protocols 0.9.0: `b10x-canon-engineering` at `tag = "0.9.0"`, `b10x-canon` still at `0.6.0`
- `b10x-loom run` stops `NoLocalExecutor` on a pick of `eval-gated-change@1`, before a case opens
- `AuthorityProvider::decide` receives the revalidated `ActionRequestData` (story:authority-sees-the-request)
- Its id, Run, case and `expected_case_revision` are the runtime's; its action and arguments the proposal as written
- A provider can allow one request at one case revision and refuse the same action at the next
- The runtime reads the case revision again after an allow, and a resume once more right before it invokes
- On a move it sees, the run loop records the request `Stale`, invokes nothing and judges the move next iteration
- A resume that sees the move before its take is refused `ResumeRefusal::CaseMoved`, the Suspension left `Pending`
- A move while the effect runs is the effect port's to refuse, by the request's `expected_case_revision`
- No provider Loom ships reads the request: `MandateAuthority`, `PluginAuthority` and the slice's grant as before
- **Breaking:** a host that links `b10x-canon-engineering` itself moves to tag `0.9.0` so it resolves one copy
- **Breaking:** the classifier is offered one more protocol, `eval-gated-change@1`
- **Breaking:** `b10x-loom resume` refuses a commission kept under 0.26.0 (route without `eval-gated-change@1`): rerun it
- **Breaking:** an `eval-gated-change@1` installed under 0.26.0 fails every command reading the store as a duplicate
- **Breaking:** `protocols remove` refuses it: delete its entry from `catalog.json` in the user protocol store by hand
- **Breaking:** the store is `$XDG_DATA_HOME/loom/protocols`, or `~/.local/share/loom/protocols`
- **Breaking:** a host's own `eval-gated-change@1` in `ProtocolCatalog::bundled()` or `engineering()` is refused
- **Breaking:** so is one registered with `CanonGovernor::with_protocol`: the name is taken
- **Breaking:** `AuthorityProvider::decide(&self, commission, request: &ActionRequestData, capability)` gains `request`
- **Breaking:** `check_authority(provider, commission, request, capability)` takes the request too
- **Breaking:** an allowed request is not performed when the runtime sees its case moved before the invoke
- **Breaking:** a resume that sees such a move after its take releases the Suspension and performs nothing
- **Breaking:** `LoopFailure::SuspensionNotReleased` also covers a release refused after such a stale request

## [0.26.0] - 2026-10-17

### Added

- The `routing_record` of an applied `ticket.route` names its `target` (story:routing-record-names-target)
- `target` is the `queue`, or for an owner route the `owner`, the admitted arguments name, never model output
- A case kept with `run --config --state` resumes on its configured models (story:cli-run-config-resume)
- `ResumeContext` gains `config`, the configuration's absolute path; `model` keeps the name serving the role `agent`
- `ResumeHost` and `ResumeContextData` gain the field `config`

### Changed

- Loom links llm 0.7.0; `Cargo.lock` holds one version of each `b10x-llm-*` crate
- llm adds `TurnRequest.controls`; Loom sets none, so its requests stay the `llm.turn/3` envelope byte for byte
- llm refuses an unknown key at every level of a Chat Completions or Responses request, naming its path
- A route whose target is absent, not text, empty, named twice or answered otherwise by Connectors records nothing
- The governor removes `target` before Canon reads the record, so the record still discharges `ticket.routed`
- `run --config` with `--state` is no longer refused; the classifier is not kept, as a resume classifies nothing
- `resume` reloads the configuration through `loom_sdk::models::from_config` and checks the agent's credential first
- A configuration gone, a name no longer served or a secret unresolved stops `resume`, exit 1, naming file and role
- A context kept earlier reads as having no configuration; one naming `config` and `catalog` is refused
- **Breaking:** a host that links llm itself moves to llm 0.7.0 so it resolves one copy of each `b10x-llm-*` crate
- **Breaking:** a `ticket.route` with no readable target records no `routing_record`: `WriteEvidenceError::Unreadable`
- **Breaking:** a caller building `intent::ResumeHost` or the generated `ResumeContextData` by its fields adds `config`

## [0.25.0] - 2026-10-17

### Added

- ESS domain `loom.models`: `ModelsConfig` (`routes`, `roles`), `RoleModel` and `ModelRole` (`agent`, `classifier`)
- `loom_sdk::models::from_config(path)` loads each role's llm `Model` from a configuration (story:sdk-models-from-config)
- `loom_sdk::models::parse_config` refuses an unknown field, an undeclared role and a role listed twice
- `loom_sdk::models::from_routes(path, name)` returns the model a route or `<endpoint>/<model-id>` name serves
- `Models::model(name)`, `Models::name_of(role)` and `Models::check_credential(name)`
- Credentials resolve from a routes document's `env:` and `file:` references; `keychain:` is refused
- `b10x-loom run --config <PATH>` takes its models from a `loom.models.ModelsConfig` (story:cli-run-config)
- `--model` and `--classifier-model` override one role each; with `--config` they have no default
- `loom_connectors::cli::ConnectorsCli::invoke_write` performs a governed write (story:connectors-governed-write)
- It is bound to the admitted request's action and arguments; anything else is refused before a process starts
- It passes Connectors the approval proof's path (`--approval-file`) and never reads the proof
- `loom_connectors::cli::idempotency_key` derives the write's key from the admitted request's id
- `WriteResult` carries Connectors' `mutation.classification`; an answer lost after the invoke started is `Unknown`
- `loom_connectors::evidence::WriteEvidenceAdapter` submits an applied write's protocol evidence for its step
- `WriteEvidenceAdapter::support_triage_route()`: a `routing_record` for `support-triage@1`'s `ticket.route`
- `loom.datasource` gains `WriteOperation`, `WriteApproval`, `MutationClassification`, `WriteResult`, `WriteRefusal`

### Changed

- Loom links llm 0.6.0, from 0.4.0; `Cargo.lock` holds one version of each `b10x-llm-*` crate (story:llm-routes-pin)
- `b10x-loom-sdk` depends on `b10x-llm-models` (`environment`, `file`), `b10x-llm-http` and the generated `loom` crate
- `run --config` stops before any model call, exit 1, on a configuration, role, name, model or secret it cannot use
- `run --config` conflicts with `--catalog` and with `--state`
- **Breaking:** a host that links llm itself moves to llm 0.6.0 so it resolves one copy of each `b10x-llm-*` crate

## [0.24.0] - 2026-10-17

### Added

- `loom_sdk::llm::LlmModelPort`, the llm-backed model port, moved from the command line into the SDK
- `loom_sdk::loom::wire::ClaudeCodePort`: one turn is one `claude -p --output-format json` call on the operator's login
- It is single-attempt and bounded in output and time; a missing or failing command is a typed refusal naming it
- `b10x-loom run --action-bindings <FILE>` runs an installed custom protocol through commands bound to its actions
- A bound program gets its arguments as JSON on stdin, no shell, only `PATH`, and is killed after `timeout_seconds`
- Exit 0 is `Performed`, its stdout the report; reported evidence must be of a kind the action may produce
- An unbound action stops the run `NoLocalExecutor`; a file binding an undeclared action is refused; bindings grant nothing
- `--unconfined-bindings`, or `"unconfined_bindings": true` in the file, opts in to unconfined bound programs
- `intake.bindings.ActionBindings` and `intake.confinement.ConfinementRefusal::CommandUnconfined`
- `loom.evaluation.EvaluationRequest.decisions`: `canon-decisions/1` entries `evaluate` hands to Canon
- `CanonGovernor::decide(case, outcome, decision, principal)` records a decision at the case's current revision
- `FileCaseStore` keeps decisions in `StoredCase.decisions`; Canon's refusal is `DecisionError::Refused`
- `CanonGovernor::open_with_independence` and `open_case_with_independence` declare producers' independence
- `CaseIndependence` (also `loom_sdk::CaseIndependence`), `CanonGovernor::independence`, `StoredCase.independence`
- `LoopContext::run_budget` returns a `RunBudget` for the run, with an `ActionBudget` per action
- `EffectOutcomePerformed.usage` (`EffectUsage`): what an effect's own model calls spent
- `LoopEnd::charges` (`RunCharges`), and `RunUsageReport.effects`, filled by `RunMeter::charge_effects`
- `WorkflowExecutor::with_store(WorkflowRunStore)` and `resume(&run_id)` (story:dw-durable-run-state)
- `loom_governor::FileWorkflowRunStore` keeps one file per Run; another Run's or definition's record is refused
- `RunMeter::carry`, so a resumed run's report covers every segment

### Changed

- A run stopped by its model loop's budget ends `Suspended` with `SuspensionReason::LoopBudget`, carrying the `LoopStop`
- A cancelled, unstructured, provider-incomplete or above-trigger loop stop still ends `NoUsefulAction`
- A trial limit stops a run before the request that would pass it, a spending limit once a charge passes it
- A run budget stop suspends the Run for `SuspensionReason::RunBudget`, naming action, limit, value and charge
- A request proposed and not recorded performed is proposed again after a restart: an effect tolerates one repeat
- `b10x-loom-governor` depends on `b10x-loom-executor`
- Loom links Canon 0.6.0 and engineering-protocols 0.8.0; `Cargo.lock` holds one `b10x-canon`
- **Breaking:** `IntentRequest` gains `bindings`; a struct literal adds `bindings: None`
- **Breaking:** `SuspensionReason` gains `LoopBudget` and `RunBudget`; an exhaustive match handles them
- **Breaking:** a governed, `LoopExecutor` or judgement-step run a loop budget stopped is `Suspended`, not `NoUsefulAction`
- **Breaking:** `EffectOutcomePerformed` gains `usage`; a struct literal adds `usage: None`
- **Breaking:** `LoopEnd` gains `charges` and `loom.metering.RunUsageReport` gains `effects`; struct literals set them
- **Breaking:** `EvaluationRequest` and `CaseState` gain `decisions`; struct literals add `decisions: Vec::new()`
- **Breaking:** `EvaluationInput` gains `Decisions`; an exhaustive match handles it
- **Breaking:** `CaseState` gains `independence`; a struct literal adds `independence: CaseIndependence::none()`
- **Breaking:** `OpenError` gains `InvalidProducer`, `UndeclaredDimension`, `RepeatedProducer`, `RepeatedDimension`
- **Breaking:** a host that links Canon itself moves to Canon 0.6.0 so it resolves one `b10x-canon`
- **Breaking:** the catalog's engineering protocols record revision `0.8.0` as their source

### Removed

- **Breaking:** `b10x_loom_cli::model_port` is removed; its `LlmModelPort` is `loom_sdk::llm::LlmModelPort`

## [0.23.0] - 2026-10-16

### Added

- `ExecutorOutcome::ProposedReads`: an executor proposes up to `MAX_FAN_OUT` (16) reads at once
- `EffectPort::reads` (`false` by default) and `EffectPort::invoke_reads`; a `Sync` port answers with `invoke_concurrently`
- `ConnectorInvoker::concurrent`, which `ConnectorsInvoker` opts in to
- `loom.workflow.JudgementStep.inputs`, a list of `loom.workflow.OutputReference` (a step and an output)

### Changed

- A fan-out is admitted only if the port reads each action, each is `Admissible` without authority, and none repeats
- Admitted reads are revalidated, invoked concurrently, each once, joined and recorded in proposal order, as one step
- A stale read is not invoked and the move it reveals ends the run `CaseMovedOn`; an effect error fails after the join
- `ConnectorEffects` reads exactly the actions whose binding's effect is `Read`; writes stay one per iteration
- A judgement step's listed inputs are appended to its instructions as one escaped `<workflow-inputs>` data block
- A listed output the run did not produce, or of another type, ends the call `NeedsHumanJudgment` with no model call
- `workflow::load` refuses an input naming no earlier output, and one listed twice
- A `RunMeter` counts overlapping and nested executor calls in their own steps
- A plain metered port turned on a thread the executor call spawned counts in step 0; a fork made in the call, in its step
- Loom requires ESS 0.60.0
- **Breaking:** `ExecutorOutcome` gains `ProposedReads`; an exhaustive match handles it
- **Breaking:** `loom.workflow.JudgementStep` gains `inputs`; a struct literal adds `inputs: Vec::new()`
- **Breaking:** specifications built with Loom's tasks need ESS 0.60.0

## [0.22.0] - 2026-10-14

### Added

- Workflow definitions: `b10x_loom_executor::workflow`, re-exported by the SDK, and the ESS domain `loom.workflow`
- A deterministic step names one action and literal arguments, proposed without a model; a judgement step asks one
- A judgement step runs the governed loop on its actions the host also admits; a step may wait on a claim (`when`)
- `WorkflowExecutor` implements `AgentExecutor`; a step whose action the frontier lacks ends `NeedsHumanJudgment`
- `RecordingEffects` advances a workflow past a step only once its request was `Performed`
- `workflow::load` reads a YAML definition, refusing outside actions, repeated or padded names, empty judgement steps
- `ProtocolEntry::actions` lists a protocol's actions; `b10x-loom-executor` depends on `serde_yaml_ng`
- Typed step outputs: `outputs` (`loom.workflow.StepOutput`, `OutputType`), read as `{"$step": …, "$output": …}`
- A missing, mistyped or unproduced output ends the call `NeedsHumanJudgment` naming the step and `output`
- `workflow::load` refuses a reference to a later, unknown or undeclared output, a malformed one, a duplicate output
- `WorkflowExecutor::outputs` lists what the run produced; outputs are kept in process
- `RunMeter`, `MeteredModelPort` and `MeteredExecutor` (`b10x_loom_executor::metering`, in the SDK) count model calls
- `RunMeter::report` returns `loom.metering.RunUsageReport`: calls, tokens and cost, in total and per step
- A step is one executor call, from 1, step 0 outside any; `.with_meter(&meter)` labels it with its workflow step
- A cost no rate card priced is absent, never zero; a failed turn's cost is unknown; ESS domain `loom.metering`

### Changed

- `loom-xtask release-check` refuses an entry without a `Breaking:` sentence, or `Breaking: none.` beside a changed surface
- The stable surfaces are `consumers/pinned/`, `crates/loom-sdk/tests/host_ports.rs`, `docs/contracts/run-events-v1.json`
- `release.yml` checks out the full history and its tags for that diff
- `run --regression-test-command <CMD>`: a bugfix's `regression_test_result` runs the test the run wrote, never the suite
- It runs once per test path the run's `tests.write` committed, after each later `tests.run`, `{path}` filled
- `TestCommand::with_regression(RegressionTestCommand::new(..)?)` and `TestRun::regressions` for library callers
- The resume context keeps the command (`ResumeContext.regression_test_command`); a 0.21.0 context reads as none
- The governor takes a record's `observed_at` only as the instant of the observation it cites
- `CanonGovernor::check_evidence` names a refused record and the reason (`EvidenceRefusal`)
- **Breaking:** a caller that builds `loom.workflow.WorkflowStep` itself sets the new field `outputs`
- **Breaking:** a definition using `$step` or `$output` as an argument member name is now read as a reference
- **Breaking:** a bugfix run without `--regression-test-command` no longer discharges `red_first.required`
- **Breaking:** `ResumeContextData` gains `regression_test_command`
- **Breaking:** a record citing an observation the governor never received, or with another instant, is refused

### Security

- rustls moves to 0.23.46 for RUSTSEC-2026-0346, RUSTSEC-2026-0347, RUSTSEC-2026-0348 and RUSTSEC-2026-0349

## [0.21.0] - 2026-10-14

### Added

- The SDK stability page names `b10x-loom-sdk`'s stable surface: its host ports, embedding calls and control-plane paths
- That surface is stable within a release line; items off the page, `#[doc(hidden)]` items and generated types are not
- `b10x-loom-mandate` (`loom_mandate`): `MandateAuthority` answers Commission's authority port with Mandate `v0.9.0`
- The capability is the action and the case the resource (`loom.case`); a Mandate allow is a grant
- `loom_mandate::case_resource_id`: UUID v5 of the case id's bytes under `735b2acd-620b-43ec-b1ab-144c2aec7ba7`
- Mandate's approval requirement is `ApprovalRequired`; an outage, a non-Mandate context or an empty case id grant nothing
- `b10x-loom approve --mandate <FILE>` and `resume --mandate <FILE>` clear an approval through Mandate
- `FILE` is a host-trusted `loom.authority.MandateHost`; `approve` records the approval in `mandate-approvals.jsonl`
- A grant names it (`intake.routing.Grant.mandate_approval`); `resume` consumes it once, else refuses `NotGranted` (16)
- `resume` without `--mandate` refuses a grant naming a Mandate approval; `b10x-loom-cli` links `b10x-loom-mandate`
- The `loom.authority` domain; the `loom` specification moves to format `ess/24` and the model gains `authority`
- The `run --output jsonl` records and exit statuses are a V1 contract, `docs/contracts/run-events-v1.json`
- Within schema version 1 only a new optional field of an existing record kind is allowed; `task docs-check` holds it
- `task consumer-check`, run by `task check`, builds `consumers/pinned/` against the SDK as an external crate would
- The migration guide gains "Move from Harness": each of the 14 Harness crates and the `loom_sdk` path replacing it
- The slice submits an `implementation_change` for every `implementation.edit` that commits
- On a `bugfix`, a `tests.run` after the first committing `tests.write` also submits a `regression_test_result`
- Every record carries `observed_at`, the host clock's instant, and the verifier's producer
- A bugfix that writes its regression test, runs it red and then edits discharges `red_first.required`
- `run::slice_governor` evaluates at the host instant (`clock::HostTime`), so an evidence kind's `max_age` expires
- `run::run_with_clock` and `LocalExecutor::with_clock` take the host clock; the others read `clock::SystemClock`
- An edit's observation (`executor::EDIT_SOURCE`) is delivered beside a test run's; a step prints each record it submitted
- `TestRun::observed_at()` and `TestRun::after_tests_write()`

### Changed

- Observation instants increase strictly in whole seconds; the executor waits up to `executor::CLOCK_WAIT` (2 s)
- A clock that stepped back fails the step with `ExecuteError::Clock`, naming both instants, before anything is submitted
- `task commission:deps-guard` also refuses any Mandate crate in `b10x-loom-commission`'s graph
- The governor gives Canon the trusted producer `submit_evidence` attributed, removing a `producer` in the facts
- **Breaking:** `grant::approve` takes a `mandate` argument and `intent::ResumeState` gains `mandate`
- **Breaking:** `grant::GrantData` gains `mandate_approval` and `grant::ApproveRefusal` gains `Mandate`
- **Breaking:** the slice's items taking a governor are generic over `T: EvaluationTime`, defaulting to `NoEvaluationTime`
- **Breaking:** `LocalExecutor`, `TestResultVerifier` and `LocalEffects` gain a defaulted type parameter
- **Breaking:** `run::run`, `run_with_options`, `case::open`, `open_with_catalog`, `report_head` take `CanonGovernor<S, T>`
- **Breaking:** so do the `intent` functions; `loom-governor`'s public API is unchanged
- **Breaking:** `executor::Report::Edited` gains `change: Option<Change>`; a pattern names it `Edited { revision, .. }`
- **Breaking:** `ExecuteError` gains `Clock`

## [0.20.0] - 2026-10-14

### Added

- Case inputs: a case opens with every input its protocol declares, given by the host (`loom_governor::CaseInputs`)
- `software-change@1` declares `risk`, `change_kind` and `affects_security_boundary`; the other bundled protocols none
- `CanonGovernor::inputs` returns a case's inputs; `CanonGovernor::protocol` names its protocol
- `b10x-loom run --risk`, `--change-kind` and `--affects-security-boundary`, defaults `standard`, `feature`, `false`
- `b10x_loom_intake_slice::case::RUN_DEFAULTS`; with `--change-kind bugfix` an edit waits for a failing test
- `intake.routing.SliceRun` declares the run's inputs; a resume's request carries the inputs the held case opened with
- `run --test-path <GLOB>`, repeatable, replaces the default test-path patterns (`test_paths::TestPaths`)
- `LocalExecutor::with_test_paths`; `SliceRun.test_paths` and `ResumeContext.test_paths` keep them

### Changed

- `tests.write` writes only test paths; another path is refused `ExecuteError::NotATestPath` before anything is written
- The slice performs `tests.write` and `implementation.edit`, the two write actions, where it performed `repository.edit`
- An obligation is open only when Canon reports it `open`; one the inputs do not select (`not_applicable`) is not
- The slice reads a software change's frontier without `release.rollback`, which it never performs (`run::SliceReads`)
- `run::never_performed` and `run::PERFORMED`; other protocols' frontiers pass through unchanged
- Loom builds on `b10x-canon-engineering` 0.7.0 and `b10x-canon` 0.4.0; the bundled catalog gains `ml-experiment@1`
- **Breaking:** `CanonGovernor::open` and `open_case` take the case inputs as a third argument
- **Breaking:** `OpenError` gains `UndeclaredInput`, `UnlistedInputValue`, `MissingInput` and `RepeatedInput`
- **Breaking:** `CaseState` gains `inputs`; `case::open` and `case::open_with_catalog` take the host's `CaseInputs`
- **Breaking:** `SliceRequest` and `IntentRequest` gain `inputs` and `test_paths`
- **Breaking:** `CaseInputs::with` returns a `Result`, and `FromIterator` is replaced by `CaseInputs::try_from_pairs`
- **Breaking:** a held case whose inputs do not satisfy its protocol is never decided: every read is `GovernorUnavailable`
- **Breaking:** `intent::resumed_request` takes the governor; its request carries the held case's inputs, not run defaults
- **Breaking:** a resume context without `test_paths`, as 0.19.0 wrote every one, is refused as a state failure
- **Breaking:** `executor::EDIT` is replaced by `TESTS_WRITE` and `IMPLEMENTATION_EDIT`
- **Breaking:** `case::governed_inputs` returns a `Result`
- **Breaking:** a case of a protocol the governor no longer has registered is `GovernorUnavailable` on every read
- **Breaking:** `StoredCase` gains `inputs`; a case file without them is refused, so a 0.19.0 state directory is not resumed
- **Breaking:** the action `repository.edit` is gone; scripts and hosts name `tests.write` or `implementation.edit`
- **Breaking:** a consumer that adds Canon itself moves to `tag = "0.4.0"`

## [0.19.0] - 2026-10-13

### Added

- `HoldSuspension` outcome `settled` (`SuspensionSettled`): the host already settled the hold, so nothing is held
- `run_with_suspension` and `resume_after_grant` then put the Run back `Running` (`ResumeRun`) and end with no error

### Changed

- In `b10x-loom resume`, a merge proposed again at a handed-off `HEAD` holds nothing (`handoff::SettledHandoffs`)
- Such a run stops `stopped: MergeHandedOff (<request>, HEAD <sha>)`, and `approve` is refused `NothingSuspended` (10)
- The slice's state-directory stores read only regular files as records; a symbolic link is refused, never followed
- `FileCaseStore` and `FileSuspensionStore` read their files the same way
- **Breaking:** `HoldSuspensionOutcome` gains `Settled`; an exhaustive match handles it
- **Breaking:** `LoopFailure` gains `NotResumed`, a `ResumeRun` that fails after a settled hold
- **Breaking:** `run_with_suspension`'s run store implements `ResumeRunBehavior`, as `RunStore` does

## [0.18.0] - 2026-10-13

### Added

- `loom_governor::FileSuspensionStore` keeps each commission's Suspension in a file, beside a `FileCaseStore`
- It refuses a commission id over `LONGEST_COMMISSION_ID` bytes; the testkit's `FileSuspensionStore` re-exports it
- `b10x-loom run --state <DIR>` keeps the case, the Suspension and the resume context (`intake.routing.ResumeContext`)
- `ResumeContextStore` keeps, removes and lists contexts (`ResumeContexts`); `run_prepared_intent_with_state`
- `b10x-loom approve <COMMISSION> <CAPABILITY> --state <DIR>` writes a grant (`intake.routing.Grant`), performing nothing
- `approve` refuses: no Suspension (10), being resumed (11), another capability (19), `AlreadyHandedOff` (23)
- `b10x-loom resume <COMMISSION> --state <DIR>` continues the run after the grant, through `resume_after_grant`
- `resume` reports the workspace's `HEAD` first, so a commit since the Suspension is refused `CaseMoved` (14)
- A granted `repository.merge` is a merge hand-off record (`intake.routing.MergeHandoff`); no Git ref moves
- `b10x_loom_cli::run_exit_status`; each resume refusal and failure has its own exit status (10 to 18, 20 to 22)
- `PreparedIntent::accepted_route` and `restore_route`; `ResumeHost::check`
- `FileCaseStore::open` and `FileSuspensionStore::open` prove their directory takes a write

### Changed

- With `--state`, a run stopping `ApprovalRequired` without a Suspension prints `..., nothing suspended`, exit 4
- The slice's functions take any `FallibleCaseStore`, which every `CaseStore` is
- `b10x-loom-commission-testkit` depends on `b10x-loom-governor`, `b10x-loom-cli` on `b10x-loom-commission`
- **Breaking:** `SliceError` gains `State` and `Resume`; an exhaustive match handles them
- **Breaking:** `b10x_loom_cli::Command` gains `Approve` and `Resume`; an exhaustive match handles them
- **Breaking:** `SliceRun` gains `gate` (`ApprovalGate`) and `RunArgs` gains `state`; struct literals set them

## [0.17.0] - 2026-10-13

### Added

- `b10x_loom_commission::ports::suspension::SuspensionPort`, which a host implements over durable storage
- `run_with_suspension` holds a commission's Suspension when the authority provider withholds an approval
- `resume_after_grant` performs the pending request once after the grant, at the case revision it was suspended at
- The port answers `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension`, each atomically
- Of two racing resumes one performs the request; the other is refused `NothingSuspended` or `InDoubt`
- A take or release for another request is refused (`other-request`); such a resume is refused `OtherRequest`
- A resume stopped between take and release leaves the Suspension `Resuming`; later resumes are refused `InDoubt`
- A commission holding a Suspension starts no Run (`LoopFailure::CommissionSuspended`)
- The testkit's file-backed `FileSuspensionStore` and in-memory `SuspensionStore`

### Changed

- **Breaking:** `ResumeRun` declares `storage-failed` (`ResumeRunOutcome::StorageFailed`, `RunStorageFailed`)
- **Breaking:** a run store implements `ResumeRunBehavior` itself, as `RunStore` does; `Generated` forwards it
- **Breaking:** `LoopFailure` gains `CommissionSuspended`, `SuspensionStore`, `SuspensionNotHeld`, `SuspensionNotReleased`

## [0.16.0] - 2026-10-12

### Changed

- With the step budget used up after a performed effect, a case that step completed ends the Run `Completed`
- A plugin turn is one Run; it no longer starts another Run at a granted gate
- Loom requires ESS 0.59.0
- **Breaking:** a step performed under an authority grant passes the gate; `run_until_blocked` runs the executor again
- **Breaking:** a caller that matched `AwaitingApproval` after a granted effect now gets the Run's own end

## [0.15.0] - 2026-10-10

### Added

- `loom.run.SelectionRecord` per selection: strategy, candidates, choice, confidence, `fell_back_to`, latency, tokens
- `Loom::selection_records`; `Session.boundary_refusals` counts selections refused at the execution boundary
- ESS commands `loom.run.RecordSelection` and `loom.run.CountBoundaryRefusal`, and the view `loom.run.SelectionRecords`
- `ActionSelector::resolve`, a provided method `HybridSelector` overrides; it times `select` and reports no tokens
- `loom.run.OverruleSelection` emits `loom.run.SelectionOverruled`
- `RequestRecord::overrule` refuses an unknown (`SelectionNotFound`) or unusable (`OverruleRefused`) replacement

### Changed

- Loom requires ESS 0.57.0; regenerating changed only documentation
- **Breaking:** the generated `SessionData` gains `boundary_refusals`; a caller building a session sets it to `0`
- **Breaking:** `selection::Pick` gains `latency_ms`, `input_tokens` and `output_tokens`
- **Breaking:** `selection::Resolved` gains `pick` and `overruled_pick`
- **Breaking:** a selector that overrides `ActionSelector::resolve` fills them
- **Breaking:** a confidence fallback records two selections, each its own selector's strategy, not one `Hybrid` selection
- **Breaking:** the fast one is `Overruled`, naming the stronger selector's in `replaced_by`; only that one reaches arguments
- **Breaking:** `loom.run.Selection` gains `replaced_by` and the terminal state `Overruled`
- **Breaking:** `Loom::select` returns the pick under the strategy of the selector that made it, never `Hybrid`

## [0.14.0] - 2026-10-09

### Added

- Payload structs `LoopStopMaxTurns`, `LoopStopMaxInputTokens`, `LoopStopMaxOutputTokens`, `LoopStopMaxCost`
- Payload structs `LoopStopBudgetUnobservable`, `LoopStopDeadline`, `LoopStopCancelled`, `LoopStopAwaitingApproval`
- Payload structs `LoopStopProviderIncomplete`, `LoopStopUnstructured`, `LoopStopContextAboveTrigger`

### Changed

- **Breaking:** `LoopStop` is generated from `loom.run.LoopStop` in `ess/domains/run.yaml` (format `ess/22`)
- **Breaking:** its variants are tuple variants over the payload structs, re-exported from `harness::turn_loop`
- **Breaking:** every count field is `i64` (was `u64`, and `u32` for `asked_again`)
- **Breaking:** `LoopStop` no longer implements `Serialize` or `Deserialize`; `LoopOutcome` and `LoopEvent` keep their JSON
- **Breaking:** reading is stricter: a `completed` stop with extra fields, and a negative, fractional or too large number

### Removed

- **Breaking:** `LoopStop::is_completed` is removed; compare with `LoopStop::Completed`

## [0.13.0] - 2026-10-09

No breaking changes.

### Added

- `protocols/inbound-answer/1.yaml` (`inbound-answer@1`): a read-only protocol answering an inbound item from data sources
- It offers `source.read`, `reply.propose` (records a proposal, never sends) and `reply.decline`
- `ProtocolCatalog::plugins()` holds `inbound-answer@1`; `ProtocolCatalog::bundled()`, which routing reads, does not
- `b10x-loom-plugin`, the plugin host, and the ESS domain `loom.plugin`
- `Plugin` hooks `poll`, `classify`, `project`, `turn`, `result`, `objectives`; `run_plugin(plugin, config, state, stop)`
- A turn runs `run_until_blocked` on `inbound-answer@1`, granting only `datasource.read` and `reply.propose`
- `DataSourceEffects` reads only within the turn's projection; `loom_connectors::cli` reads and refuses writes
- A turn sends at most 8 model requests; an item failing three times is recorded `stopped`
- Each handled item adds one line to `record.jsonl`; `state.json` is written atomically; one host per state directory
- `b10x-loom-plugin-slack`, the slack-handler plugin, and the ESS domain `loom.slack` (`SlackConfig`)
- It walks the bot's channels through three Connectors reads and records proposed answers, posting nothing
- `b10x-loom plugin run <PLUGIN> --config <file> --state <dir> [--once]` and `b10x-loom plugin report <PLUGIN>`
- A plugin turn runs an llm model through `model_port::LlmModelPort`; SIGTERM or SIGINT ends the host after the item

## [0.12.0] - 2026-10-09

### Added

- `ReasoningModelSelector` (strategy `ReasoningModel`): one turn of any `ModelPort`, held to the candidate ids
- An action outside them is refused `not-in-catalogue`; an answer naming none is `SelectorError::Unavailable`
- `HybridSelector` (strategy `Hybrid`): the fast choice at or above a host-supplied threshold, else the stronger answer
- The threshold has no default and must be a decimal in [0, 1] (`InvalidThreshold`); `Confidence` compares as a number
- `b10x-loom-selector-laya`, an experimental `FastTyped` selector over a Laya endpoint (`POST /v1/systemone`)
- Neither `b10x-loom-cli` nor `b10x-loom-sdk` links `b10x-loom-selector-laya`

### Changed

- `select` drops, for every selector, a confidence that is not a decimal in [0, 1]
- A compaction of a declared context window leaves the session at or below its target (50 %) where eliding can reach it
- A fold not replaced by a summary is elided behind one `ELISION_MARKER` item saying how many, how many bytes and why
- What no compaction removes includes the instruction and tool schemas the provider counts in its input
- **Breaking:** `LoopStop` gains `ContextAboveTrigger { window, target, occupied }`, in tokens
- **Breaking:** a run whose session stays at or above its trigger (80 %) ends with it and sends no further request

## [0.11.0] - 2026-10-08

### Changed

- Loom builds on `b10x-canon-engineering` 0.3.0 and names Canon by tag, `b10x-canon` 0.1.0 (was `branch = "main"`)
- `incident.response/1` gains the obligation `investigate_cause`, open until a cause analysis identifies the cause
- The engineering registry, so `ProtocolCatalog::engineering` and `bundled`, lists `support-triage@1`, at revision `0.3.0`
- `SessionFile::load` reads a filed session from its text, so a tool call's integer above `u64::MAX` still reads
- **Breaking:** a consumer that adds Canon itself moves to `tag = "0.1.0"`, or builds a second Canon whose types differ

## [0.10.0] - 2026-10-08

### Added

- `loom_governor::FileCaseStore` keeps each governor case in its own file as `loom.governor`'s `StoredCase`

### Changed

- A run store can report a failed write: `StartRun` and `SuspendRun` declare `storage-failed` (`RunStorageFailed`)
- `run_until_blocked` reports it as `LoopFailure::RunStorage`; a failed start ends before any executor or effect call
- A failed suspension is carried in `LoopError::suspension`, no longer as `NotSuspended`
- **Breaking:** `StartRunOutcome`, `SuspendRunOutcome` gain `StorageFailed` and `LoopFailure` `RunStorage`: add the arms
- **Breaking:** ports other than `RunStore` under `Generated` implement `StartRunBehavior` and `SuspendRunBehavior`

### Removed

- **Breaking:** the generated commission crate no longer has `Context`, `TryContext` or `unmet_context`

## [0.9.0] - 2026-10-08

### Changed

- A read bound to a Connector read operation answers `Performed` with its result, naming its `audit` record
- `ActionBinding` declares the bound operation's `effect` (`Read` or `Write`); `EffectOutcomePerformed` gains `audit`
- `ConnectorsInvoker` refuses, before invoking, a binding whose effect is not the one the service describes
- A `Write` action still answers an error when Connectors records no attempt
- **Breaking:** a host that builds `ActionBindingData` or `EffectOutcomePerformed` adds the new field
- **Breaking:** `BindingError::OtherCommission` boxes the binding it names

## [0.8.0] - 2026-10-08

No breaking changes.

### Added

- `b10x-loom-connectors` (`loom_sdk::connectors`): `ConnectorsInvoker` fills `ConnectorInvoker` over Connectors `v0.35.0`
- It invokes each admitted request once on `POST /v1alpha2/invoke`, never `/v1/invoke`, and never resends
- `Performed` names the attempt Connectors recorded; `Refused` only for `refused` or `not_attempted`; else an error
- It resolves the endpoint's credential through the host's `CredentialResolver` before invoking
- Known limit: a read operation answers an error, as Connectors `v0.35.0` records no attempt for a read
- `commission.responsibility.ConnectorEndpoint`: one Connector instance's URL, credential reference and `http` admission

### Changed

- Loom depends on Connectors `v0.35.0` (`connectors-client`, `connectors-core`) through `b10x-loom-connectors` only

## [0.7.0] - 2026-10-08

### Added

- `credentials::ResolvedBearer`: a ported wire resolves a `loom.run.WireCredential` reference at each call
- It asks an llm `SecretResolver` the embedder injects, so an OAuth login (a Codex login) needs no secret in configuration
- A missing reference, a refusal or an unusable secret is `Unauthorized`, naming the reference, never the value
- A call blocks until the resolver answers or `credentials::RESOLVE_BOUND` (30 s) passes
- `credentials::encode_wire_credential` and `decode_wire_credential`
- `b10x-loom run --output jsonl`: one JSON object per line, `schema_version` 1, exactly one `Terminal` record, last
- Record kinds `Route`, `Turn`, `ToolCall`, `Usage`, `Approval` and `Terminal`, declared as the `intake.events` domain
- `SliceRun` gains `approvals`, the actions a run stopped to await approval for
- `loom_governor::evaluate(&ProtocolCatalog, &EvaluationRequest)` decides a case the caller keeps, no Commission type
- It returns Canon's decision with the whole `canon-decision/1` document, holds nothing and reads no clock
- A refusal names the input; unlike `CanonGovernor`, it refuses an unreadable record and a repeated id
- The `loom.evaluation` domain, re-exported as `loom_governor::model`
- `b10x-loom evaluate`: the same over JSON, from stdin or `--input <PATH>`, exit 0 or 3; `--input` must be a regular file
- `b10x-loom run --catalog <PATH>` reads an llm catalog; `--model` and `--classifier-model` name its route aliases
- A self-hosted Chat Completions, Responses or Messages endpoint can drive a run
- Before any model call, exit 1 on an undeclared alias, a fallback route, a credentialed account or no forced tool call
- The catalog must be a regular file; a named pipe, as `--catalog <(cmd)` hands, is refused naming the path

### Changed

- Every llm dependency moves from tag `0.3.1` to `0.4.0`; the executor depends on `b10x-llm-credentials`
- `b10x-loom-cli` adds `b10x-llm-models`, `b10x-llm-routing`, `b10x-llm-http`, `b10x-llm-credentials` and `libc`
- `harness::wire::CredentialKind` is now the generated `loom.run.CredentialKind`
- **Breaking:** `harness::wire::CredentialKind` no longer implements `PartialOrd`, `Ord` or `Hash`

## [0.6.0] - 2026-10-08

### Added

- `RunOutcome::CaseMovedOn` (`RunOutcomeCaseMovedOn`): the case moved to a revision whose frontier still admits an action
- It names `bound_case_revision` and `current_case_revision`; start a new Run at the current one
- The intake slice stops it `NothingAdmissible (case moved on from revision <bound> to <current>)`

### Changed

- A run whose case moved that way ends `CaseMovedOn`, not `NoAdmissibleAction`, however the move was found
- Loom requires ESS 0.56.0
- **Breaking:** callers that match `RunOutcome` exhaustively handle `CaseMovedOn`

## [0.5.0] - 2026-10-08

### Added

- `ExecutorOutcome::CaseMoved` (`ExecutorOutcomeCaseMoved`, `expected_case_revision`): an executor reports a moved case
- The governor decides: unmoved, the step is `NoUsefulAction`; moved, the run is judged on the current frontier

### Changed

- **Breaking:** callers that match `ExecutorOutcome` exhaustively handle `CaseMoved`

### Fixed

- `Loom::with_governor` reports a `stale-revision` selection as `CaseMoved` instead of `NoUsefulAction`
- Under `run_until_blocked`, a case that moves while selecting or generating arguments is judged on its current frontier
- `LoopExecutor` reports `CaseMoved` after a selection refused `stale-revision` or a catalogue of another revision
- `run_until_blocked` loads the case again before ending a run that proposed nothing, judging a move it finds
- `task docs-check` writes ESS output under `target/loom-docs/<run>/`, no longer the shared system temporary directory

## [0.4.0] - 2026-10-07

No breaking changes.

### Added

- `task install` builds the checkout and installs `b10x-loom` into `~/.local/bin`
- `system-query@1` with the read-only `system.time.read`: `run` answers date and time with no workspace
- A verified system query completes with exit 0; software changes still need a Git worktree and confinement
- `b10x-loom-protocols` composes engineering, Loom and custom definitions into one catalog for routing and admission
- `protocols add`, `list` and `remove` install validated snapshots or Git blobs at full commits; runs load offline
- `--context-policy bounded`, opt-in: report-derived working state, a recent-event tail and retrievable history
- It caps a request at 64 KiB serialized and history at 16 MiB and 4,096 events; legacy context stays the default
- `--context-report` writes payload-free request and usage measurements; embedders gain `run_with_options`
- Commission's `ActionBinding` binds a frontier action to one Connector operation (`instance_id`, `operation_id`)
- `ports::connector::ConnectorEffects`, an `EffectPort` over those bindings through the new `ConnectorInvoker` port
- The testkit adds `RecordingInvoker`; `ScriptedExecutor` records the frontier of each call
- `b10x-loom-conformance` holds the executor to Loom's synthesized ESS suite; `task check` runs `task conform`
- `selection::select_action` answers `loom.run.SelectAction`
- `session::TurnRecord` answers `loom.run.ReleaseSession` and `loom.run.ProjectCatalogue`

### Changed

- Intake model calls retry transient failures, provider overload included (llm 0.3.1), at most three attempts
- Context reports count every attempt; tool effects and actual refusals are never retried
- Before the executor runs, the runtime drops the actions the effect port does not perform and that need no authority
- `EffectOutcomePerformed` names the Connector attempt (`attempt`); the local slice names none
- Loom requires ESS 0.55.0

### Fixed

- A compaction never grows the session: a summary no shorter than its items is `summary-failed`, and they are elided

## [0.3.0] - 2026-10-07

No breaking changes.

### Added

- `Loom::run_loop` runs the ported Harness loop over a commission's frontier; `harness::governed::LoopExecutor` too
- Each turn's tools are the catalogue projected from the current frontier (`harness::governed::tool_name`)
- A model's call of an action is selected, revalidated and returned as a `ProposedAction` at an approval checkpoint
- Turns are recorded on the run's session (`loom.run.RecordTurn`, `Loom::turns`), filed at the end (`Loom::sessions`)
- `Loom::interrupt` cancels the run holding a session, which becomes `Interrupted`
- `Loom::resume_loop` resumes a session from its held checkpoint; `Loom::catalogues`; `AgentLoop::resume_asking`
- Compactions are recorded with the endpoint's reported usage (`loom.run.RecordCompaction`, `Loom::compactions`)
- `LoopEvent::Compacted` carries that usage
- The local slice keeps inspected file contents as run-local results, read through bounded previews and references
- Later briefings show edit bodies by size and digest; test results report partial capture

### Changed

- Each `Loom` derives its ids in a namespace of its own, random by default, fixed with `Loom::with_instance`
- Loom requires ESS 0.54.0

## [0.2.0] - 2026-10-06

No breaking changes.

### Added

- `Loom::with_governor` revalidates a selection against the governor's current frontier before proposing it
- A selection at another revision (`stale-revision`) or outside the frontier (`not-in-frontier`) proposes nothing
- `CanonGovernor::with_protocol` compiles host-admitted protocols; `with_evaluation_time` takes trusted time
- `FallibleCaseStore` makes durable storage failures explicit; `CaseStore` callers are unchanged
- A protocol with several capabilities on one action is refused

### Security

- Tests run in embedded Substrate 0.7.10 by default: no network, cleared environment, read-only source and toolchain
- A test writes only under `target/`, within 300 seconds, 8 GiB and 2,048 processes; dependencies are fetched first
- Confinement failure stops `ConfinementUnavailable` (exit 3); `--confinement none` is an explicit opt-out
- The SDK exposes an injectable `TestRunner`; a test observation carries Substrate's applied record
- Intake's confinement types are generated from ESS; all three systems use ESS 0.53.0
- The slice's git calls run no workspace hook, `core.fsmonitor` command or signing program
- A case does not open on a workspace whose git configuration names a program; `ExecuteError::HostGit` once it changes

## [0.1.0] - 2026-10-05

### Added

- `.github/workflows/release.yml` verifies a pushed tag with `loom-xtask release-check` and runs Loom's check on it
- `b10x-loom run` routes an intent to an engineering protocol, opens a governed case and runs it until it is blocked
- Its exit status: 0 at the human gate (`ApprovalRequired`), 3 for another stop, 1 for a failure, 2 for bad arguments
- It never merges, pushes or deploys, and it has no sandbox
- `b10x-loom-sdk` embeds the runtime: `commission`, `loom`, `governor`, `intake::router` and `intake::slice`
- `examples/software_change.rs` runs a whole embedding over scripted fake models
- `b10x-loom-commission`, Commission's contracts and runtime with `run_until_blocked`
- `b10x-loom-commission-testkit` (port fakes, adapter kits) and `b10x-loom-commission-conformance`
- `run_until_blocked` hands each admitted request to an `EffectPort` once; a new revision counts only after `Performed`
- It stops at `AwaitingApproval`, `NoPerformableAction`, `NeedsAuthority`, the step budget, or after 64 steps without one
- `b10x-loom-governor`: `CanonGovernor` evaluates a case's protocol with Canon and executes nothing
- `b10x-loom-intake-references`, `b10x-loom-intake-router` and `b10x-loom-intake-slice`
- The slice performs `software.change/1` actions in a workspace and stops at `ApprovalRequired`
- The ESS domain `intake.routing` in `ess/intake/`
- JSON from a model or provider nested deeper than 128 levels is refused with a typed error
- Documentation: a getting-started page, guides, and generated CLI and crate references

### Changed

- Commission's contracts and runtime, the governor and intake moved into Loom with their history (Atlas ADR 0090)
- Loom builds on llm `0.1.7` and on `b10x-canon-engineering` `0.1.0`, which replaces `b10x-els`
- The slice has no loop of its own: Loom proposes each action, and the Commission runtime rechecks and invokes it
- **Breaking:** every package carries a `loom-` name, and the intake and governor libraries are renamed with them
- **Breaking:** `b10x-loom` is now `b10x-loom-executor` (`b10x_loom_executor`)
- **Breaking:** `b10x-commission` is now `b10x-loom-commission` (`b10x_loom_commission`)
- **Breaking:** `b10x-commission-testkit` is now `b10x-loom-commission-testkit` (`b10x_loom_commission_testkit`)
- **Breaking:** `b10x-commission-conformance` is now `b10x-loom-commission-conformance`
- **Breaking:** `b10x-governor` (library `governor`) is now `b10x-loom-governor` (`loom_governor`)
- **Breaking:** `b10x-intake-references` (`intake_references`) is now `b10x-loom-intake-references`
- **Breaking:** `b10x-intake-router` (`intake_router`) is now `b10x-loom-intake-router` (`b10x_loom_intake_router`)
- **Breaking:** `b10x-intake-slice` (`intake_slice`) is now `b10x-loom-intake-slice` (`b10x_loom_intake_slice`)
- **Breaking:** `b10x-intake-cli`, binary `b10x-intake`, is now `b10x-loom-cli`, binary `b10x-loom` (`b10x_loom_cli`)

### Removed

- **Breaking:** `b10x-intake-model` is removed; its forced tool call and Codex preset are `b10x-llm-tool-call`

[Unreleased]: https://github.com/beyond10x/loom/compare/0.32.0...HEAD
[0.32.0]: https://github.com/beyond10x/loom/releases/tag/0.32.0
[0.31.0]: https://github.com/beyond10x/loom/releases/tag/0.31.0
[0.30.0]: https://github.com/beyond10x/loom/releases/tag/0.30.0
[0.29.0]: https://github.com/beyond10x/loom/releases/tag/0.29.0
[0.28.0]: https://github.com/beyond10x/loom/releases/tag/0.28.0
[0.27.1]: https://github.com/beyond10x/loom/releases/tag/0.27.1
[0.27.0]: https://github.com/beyond10x/loom/releases/tag/0.27.0
[0.26.0]: https://github.com/beyond10x/loom/releases/tag/0.26.0
[0.25.0]: https://github.com/beyond10x/loom/releases/tag/0.25.0
[0.24.0]: https://github.com/beyond10x/loom/releases/tag/0.24.0
[0.23.0]: https://github.com/beyond10x/loom/releases/tag/0.23.0
[0.22.0]: https://github.com/beyond10x/loom/releases/tag/0.22.0
[0.21.0]: https://github.com/beyond10x/loom/releases/tag/0.21.0
[0.20.0]: https://github.com/beyond10x/loom/releases/tag/0.20.0
[0.19.0]: https://github.com/beyond10x/loom/releases/tag/0.19.0
[0.18.0]: https://github.com/beyond10x/loom/releases/tag/0.18.0
[0.17.0]: https://github.com/beyond10x/loom/releases/tag/0.17.0
[0.16.0]: https://github.com/beyond10x/loom/releases/tag/0.16.0
[0.15.0]: https://github.com/beyond10x/loom/releases/tag/0.15.0
[0.14.0]: https://github.com/beyond10x/loom/releases/tag/0.14.0
[0.13.0]: https://github.com/beyond10x/loom/releases/tag/0.13.0
[0.12.0]: https://github.com/beyond10x/loom/releases/tag/0.12.0
[0.11.0]: https://github.com/beyond10x/loom/releases/tag/0.11.0
[0.10.0]: https://github.com/beyond10x/loom/releases/tag/0.10.0
[0.9.0]: https://github.com/beyond10x/loom/releases/tag/0.9.0
[0.8.0]: https://github.com/beyond10x/loom/releases/tag/0.8.0
[0.7.0]: https://github.com/beyond10x/loom/releases/tag/0.7.0
[0.6.0]: https://github.com/beyond10x/loom/releases/tag/0.6.0
[0.5.0]: https://github.com/beyond10x/loom/releases/tag/0.5.0
[0.4.0]: https://github.com/beyond10x/loom/releases/tag/0.4.0
[0.3.0]: https://github.com/beyond10x/loom/releases/tag/0.3.0
[0.2.0]: https://github.com/beyond10x/loom/releases/tag/0.2.0
[0.1.0]: https://github.com/beyond10x/loom/releases/tag/0.1.0
