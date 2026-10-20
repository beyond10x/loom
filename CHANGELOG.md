# Changelog

All notable changes to Loom are recorded here. Loom releases from source at bare-version tags such
as `0.1.0`; nothing is on a registry, and consumers pin a tag (`tag = "0.1.0"`). Changes collect
under **Unreleased** until the next release. Every release's entry names what breaks for a
consumer in a `Breaking:` sentence; `loom-xtask release-check` refuses an entry without one, and
refuses `Breaking: none.` when `consumers/pinned/`, `crates/loom-sdk/tests/host_ports.rs` or
`docs/contracts/run-events-v1.json` changed since the previous tag.

## [Unreleased]

Loom links engineering-protocols 0.9.0, the tag takt pins, so a crate depending on both resolves
one `b10x-canon-engineering`: the four manifests that name it (`loom-governor`, `loom-protocols`,
`loom-intake-slice`, `loom-intake-router`) name `tag = "0.9.0"`, and `Cargo.lock` holds one copy of
it and one `b10x-canon`, still at tag `0.6.0`. engineering-protocols 0.9.0 adds the built-in
protocol `eval.gated_change/1` and changes no existing one, so the bundled catalog the router and
`b10x-loom protocols list` read gains `eval-gated-change@1`, recording revision `0.9.0` as every
engineering protocol does. The slice does not perform it: a pick of it in `b10x-loom run` stops
`NoLocalExecutor` naming it before a case opens, as a pick of `incident-response@1` does.
Breaking: a host that links `b10x-canon-engineering` itself moves to tag `0.9.0` so it resolves one
copy; the classifier is offered one more protocol, `eval-gated-change@1`; `b10x-loom resume`
refuses a commission that `run --state` kept under 0.26.0, whose kept route names the catalog
without `eval-gated-change@1`, so run that intent again; `eval-gated-change@1` is now a bundled
name, so a definition of that name installed in the user protocol store under 0.26.0 fails every
command that reads the store as a duplicate registration and `protocols remove` refuses it: delete
its entry from `catalog.json` in that store (`$XDG_DATA_HOME/loom/protocols`, or
`~/.local/share/loom/protocols`) by hand; a host that adds its own `eval-gated-change@1` to
`ProtocolCatalog::bundled()` or `engineering()`, or registers one with
`CanonGovernor::with_protocol`, is refused because the name is taken.

A host reads the decision behind a held case's frontier (`story:governor-exposes-canon-decision`).
`CanonGovernor::decision_document(&case, &frontier_id)` returns `Some` of the whole
`canon-decision/1` document Canon wrote behind a frontier the governor issued, explanation
included, whose rendering gives that frontier id, as the generated JSON value
(`loom_sdk::governor::model::json::Value`) `evaluate` returns in `canon`. `frontier` keeps the
document it decided under the id it issues, in the governor's memory, so a host holding an id from
Commission's loop (`LoopEnd::last_frontier`, a step's observation) reads the document that frontier
was issued from, even when the trusted time moved since, as the host clock `b10x-loom` evaluates at
does. A lookup never evaluates the case: an id the governor did not issue, or no longer keeps, is
`None`. Per case the governor keeps only documents for the case as currently held, at most
`KEPT_DOCUMENTS_PER_CASE` (16), the most recently issued: they are dropped when a frontier or a
lookup finds the held case changed (a new artifact revision, evidence received, a decision
recorded) and when a read finds the store no longer holds the case. For the same protocol,
evidence, explicit decisions and time the document equals what `evaluate` returns. A case the
governor cannot read (its held inputs or independence declarations do not satisfy its protocol)
is refused `GovernorUnavailable`, and a case it does not hold `UnknownCase`, as `frontier` refuses
them. Breaking: none for callers; `CanonGovernor` only gains a method and the constant
`loom_governor::KEPT_DOCUMENTS_PER_CASE`.

## [0.26.0] - 2026-10-17

Loom 0.26.0 links llm 0.7.0, names the target in the routing record of a governed write, and
resumes a case kept with `b10x-loom run --config` on its configured models. The `routing_record`
the host evidence adapter records for an applied `ticket.route` names the queue or owner the
admitted arguments name, and a target that cannot be read records nothing. `run --config` now
takes `--state`: the resume context keeps the configuration's absolute path, and `resume` reloads
it and checks the agent's credential before any model call.
Breaking: a host that links llm itself moves to llm 0.7.0; a `ticket.route` whose admitted
arguments name no readable target records no `routing_record` and returns
`WriteEvidenceError::Unreadable`; a resume context gains the optional member `config`.

Loom links llm 0.7.0: every `b10x-llm-*` dependency names tag `0.7.0`, and `Cargo.lock` holds one
version of each. llm 0.7.0 adds `TurnRequest.controls`; Loom sets none, so every request it builds
stays the `llm.turn/3` envelope byte for byte and its recorded requests do not change. llm 0.7.0
also refuses an unknown key at every level of a Chat Completions or Responses request, naming its
path, where 0.6.0 passed over keys below the top level.
Breaking: a host that links llm itself moves to llm 0.7.0 so it resolves one copy of each
`b10x-llm-*` crate.

A routing record names the route target (`story:routing-record-names-target`). The `routing_record` `WriteEvidenceAdapter::support_triage_route` records for an applied `ticket.route` carries `target` in its facts: the `queue`, or for an owner route the `owner`, the admitted arguments name, never model output. A route whose target is absent, not text, empty or named twice (`queue` and `owner`), or whose answer from Connectors names another queue or owner, delivers no observation and records no evidence; `WriteEvidenceError::Unreadable` says why. `target` is not a `canon-evidence/1` field: the governor removes it before Canon reads the record, as it removes a facts `producer`, so the record still discharges `ticket.routed`. Breaking: a `ticket.route` whose admitted arguments name no readable target, which recorded a `routing_record` before, now records none and returns `WriteEvidenceError::Unreadable`.

A case kept with `b10x-loom run --config` resumes on its configured models (`story:cli-run-config-resume`). `run --config <PATH> --state <DIR>` is no longer refused: the resume context (`intake.routing.ResumeContext`) gains `config`, the configuration's absolute path, and keeps in `model` the name that serves the role `agent` (`--model`, when given); the classifier is not kept, since a resume classifies nothing. `resume` loads the configuration again through `loom_sdk::models::from_config`, resolves that name and checks its credential; a configuration that is gone, a name its routes document no longer serves, or a secret that does not resolve stops the resume, exit status 1, naming the file and the role `agent`, before any model call. A context kept before this change reads as having no configuration; one naming both `config` and `catalog` is refused when it is kept and when it is read. `ResumeHost` and `ResumeContextData` gain the field `config`. Breaking: a Rust caller that builds `b10x_loom_intake_slice::intent::ResumeHost` or the generated `ResumeContextData` by its fields adds `config`.

## [0.25.0] - 2026-10-17

Loom 0.25.0 lets a host perform a governed write and choose its models from a routes document. A
host writes through Connectors' command line with `ConnectorsCli::invoke_write`, bound to the
request the runtime admitted (its action and arguments) and to the person's Connectors approval
proof, passed as `--approval-file` with an idempotency key derived from the request id; a write
Connectors reports `unknown` is never retried, and the host evidence adapter records the protocol's
evidence for an applied write, `routing_record` for `support-triage@1`'s `ticket.route`. Models
come from a configuration naming an `llm.routes/1` document and a model per role, through
`loom_sdk::models` and `b10x-loom run --config`; Loom links llm 0.6.0.
Breaking: none for callers of existing paths; the SDK gains `loom_sdk::models`,
`loom_sdk::connectors::cli::ConnectorsCli::invoke_write` and `loom_sdk::connectors::evidence`, and a
host that links llm itself moves to llm 0.6.0 so it resolves one copy of each `b10x-llm-*` crate.

Loom links llm 0.6.0 (`story:llm-routes-pin`). Every `b10x-llm-*` dependency moves from tag `0.4.0` to `0.6.0` and `Cargo.lock` holds one version of each; llm 0.5.0 and 0.6.0 name no breaking change in the crates Loom uses. The specification gains the domain `loom.models` (`ess/domains/models.yaml`): `loom.models.ModelsConfig` (`routes`, the path of an `llm.routes/1` document relative to the configuration file, and `roles`, a list of `loom.models.RoleModel`), each naming a `loom.models.ModelRole` (`agent` or `classifier`) and the `model` that serves it, a route name or an `<endpoint>/<model-id>` name. Types only; nothing reads it yet. Breaking: a host that links llm itself moves to llm 0.6.0.

A host loads its models from a configuration or a routes document through the SDK alone (`story:sdk-models-from-config`). `loom_sdk::models::from_config(path)` reads a `loom.models.ModelsConfig` (YAML, parsed by `loom_sdk::models::parse_config`, which refuses an unknown field, an undeclared role and a role listed twice) and returns `Models`: the llm `Model` of each declared role, and `Models::model(name)` for another name of the same routes document, whose path is relative to the configuration's directory. `loom_sdk::models::from_routes(path, name)` returns the model one name of an `llm.routes/1` document serves, a route or `<endpoint>/<model-id>`. Both build the port with llm's `Routes::port` and resolve credentials from the document's `env:` and `file:` references (`~/` against `HOME`; `keychain:` is refused, that llm feature is off); Loom reads no secret itself and nothing is read or sent when the models are built; a missing secret is refused `unauthorized` at the first turn, unless the host asks first with `Models::check_credential(name)`, which resolves the secret of the endpoint the name reaches (the value is dropped) and names the endpoint and the reference kind in its refusal. `Models::name_of(role)` returns the name that serves a configured role. Files are read as regular files only, and every refusal names the file and the role or name. `b10x-loom-sdk` gains the dependencies `b10x-llm-models` (features `environment`, `file`), `b10x-llm-http` and the generated `loom` crate; the SDK stability page lists the new paths. Breaking: none for callers; the SDK only gains paths (`loom_sdk::models`).

`b10x-loom run` takes its models from a configuration file (`story:cli-run-config`). `run --config <PATH>` loads a `loom.models.ModelsConfig` through `loom_sdk::models::from_config`: the classifier and the agent run on the models of the roles `classifier` and `agent`, routes of the `llm.routes/1` document it names, with the credentials its `env:` and `file:` references name, so a route whose endpoint needs a credential runs, where `--catalog` refuses it. `--model` and `--classifier-model`, when given, override one role each with a route or `<endpoint>/<model-id>` name of the same document; with `--config` they have no default. Before any model call the run stops, exit status 1, on a configuration that cannot be loaded, a role neither configured nor given (naming the role and its flag), a name the document does not serve, a model that cannot take a forced tool call, and a model whose endpoint's `env:` or `file:` secret does not resolve (each naming the role; the last also the endpoint and the reference kind, never the value). `--config` conflicts with `--catalog`, and with `--state`, because a resume context records a Codex model or a catalog route and no configuration. Without `--config` nothing changes. Breaking: none for callers; `run` only gains `--config`.

A host performs a governed write through the Connectors command line (`story:connectors-governed-write`). `loom_connectors::cli::ConnectorsCli::invoke_write(&AdmittedRequest, &WriteOperation, input, &WriteApproval)` writes for a request only the runtime admitted, bound to its admitted action and arguments: `WriteOperation` names the protocol `action` it performs, and an action other than the admitted request's is refused `action-not-admitted`, and input other than its admitted arguments (compared as JSON values) `input-not-admitted`, before any process starts. Then one `operations describe`, which must report the operation with the `mutation` profile (any other is refused `not-a-write` and not invoked), then exactly one `operations invoke --input-stdin --approval-file <path> --idempotency-key <key>`, the input on stdin and never in argv. The `WriteApproval` names the absolute path of the proof `connectors approvals issue` wrote for the person's confirmation; Loom passes the path and never reads, writes or stores the proof, and an empty or relative path, or one holding a control character (`approval-not-a-path`), is refused before any process starts. The key is `loom_connectors::cli::idempotency_key` of the admitted request, derived from its id, so the same admitted request can only observe its first attempt. The answer is a `WriteResult` with Connectors' `mutation.classification` (`NotAttempted`, `Refused`, `Applied`, `Unknown`), `request_id`, `source_audit` and the provider's answer; an answer lost or unreadable after the invoke started is `Unknown`, and nothing is retried. `loom_connectors::evidence::WriteEvidenceAdapter` is the host evidence adapter: an applied write of its action becomes the protocol's evidence for the step through `submit_evidence`, naming the action and the admitted request (`WriteEvidenceAdapter::support_triage_route()`: a `routing_record` for `ticket.route` in `support-triage@1`, which is unchanged); a write not applied, or the write of another admitted request (another idempotency key, refused `WriteEvidenceError::OtherRequest`), records nothing. The specification's `loom.datasource` gains `WriteOperation`, `WriteApproval`, `MutationClassification`, `WriteResult` and `WriteRefusal`. Reads are unchanged: `invoke_read` still refuses a `mutation` operation and no read passes `--approval-file`; the service invoker, `ConnectorsInvoker`, is unchanged. The SDK stability page lists the new `loom_sdk::connectors` paths. Breaking: none for callers; the SDK only gains paths (`loom_sdk::connectors::cli` and `loom_sdk::connectors::evidence`).

## [0.24.0] - 2026-10-17

Loom 0.24.0 puts a model and custom protocols within reach of an embedding host. The SDK carries
an llm-backed model port and a Claude Code model port (`ClaudeCodePort`), so a host writes
neither. `b10x-loom run --action-bindings` runs an installed custom protocol through commands the
operator binds to its actions, and refuses to start unconfined programs unless the operator opts
in with `--unconfined-bindings` or `"unconfined_bindings": true` in that file
(`ConfinementUnavailable`). A host can pass explicit decisions on an
evaluation request and declare, when it opens a case, the independence dimensions of each evidence
producer. A model loop's budget now stops a run as `LoopBudget`, and Commission's runtime enforces
run and action budgets (`RunBudget`) over effects that report their usage. A workflow run survives
a restart through a durable run record. Loom links Canon 0.6.0 and engineering-protocols 0.8.0.
Breaking: `b10x_loom_cli::model_port` is gone (use `loom_sdk::llm::LlmModelPort`);
`SuspensionReason` gains `LoopBudget` and `RunBudget` and `EvaluationInput` gains `Decisions`, so
exhaustive matches handle them, as `OpenError` handles its four new variants; code that builds
`IntentRequest`, `EffectOutcomePerformed`, `LoopEnd`, `RunUsageReport`, `EvaluationRequest` or
`CaseState` with a struct literal sets the new fields (`bindings`, `usage`, `charges`, `effects`,
`decisions`, `independence`); and a host that links Canon itself moves to Canon 0.6.0.

A workflow run survives a restart (`story:dw-durable-run-state`). `WorkflowExecutor::with_store`
takes a `b10x_loom_executor::workflow::WorkflowRunStore`; bound to a Run with `begin(&run_id)`, the executor writes
the run's `loom.workflow.WorkflowRunRecord` after every performed step and every call: the steps
performed and the one proposed, the outputs taken, and the usage so far
(`loom.metering.CarriedUsage`). `resume(&run_id)` in another process continues the run at the step
after the last performed one, every output bound and no performed step proposed again, after an
approval (`resume_after_grant`) or a crash between steps, and `RunMeter::carry` makes the meter's
report cover every segment. `loom_governor::FileWorkflowRunStore` keeps one file per Run, under the
lock and atomic write `FileCaseStore` and `FileSuspensionStore` use, so the three may share one
directory; a record of another Run or of another definition is refused and nothing of it is read. A
request proposed and not recorded performed is proposed again after a restart: an effect must
tolerate one repeat. `b10x-loom-governor` now depends on `b10x-loom-executor` (it was a
dev-dependency), and the SDK stability page lists the new port,
`loom_sdk::loom::workflow::WorkflowRunStore`. A host that gives the executor no store sees no
change.

Loom links Canon 0.6.0 and engineering-protocols 0.8.0. `b10x-canon` moves from 0.4.0 to 0.6.0 and
`b10x-canon-engineering` from 0.7.0 to 0.8.0 in `loom-protocols`, `loom-governor`,
`loom-intake-slice` and `loom-intake-router`, and `Cargo.lock` holds one `b10x-canon`. A new
conformance test, `the_governor_decision_equals_the_pinned_canon_decision_bytes`, holds the
`canon-decision/1` document the governor returns equal to the one the pinned Canon release's
`evaluate_with` renders, for the software-change fixtures `chg-1842` and
`security-boundary-independent`. Breaking: a host that links Canon itself moves to Canon 0.6.0 so it
resolves one `b10x-canon`; the catalog's engineering protocols record revision `0.8.0` as their
source.

Breaking: `b10x_loom_cli::model_port` is gone; its `LlmModelPort` is `loom_sdk::llm::LlmModelPort`
(the CLI's module was no stable surface).

Breaking: `b10x_loom_intake_slice::intent::IntentRequest` gains the public field `bindings`,
so code that builds one with a struct literal must add `bindings: None`.

Breaking: `SuspensionReason` gains the variant `LoopBudget`, so a host that matches it
exhaustively no longer compiles until it handles the variant; and a governed run, a
`LoopExecutor` run or a workflow judgement step stopped by a budget of its model loop now ends
`Suspended` with it instead of `NoUsefulAction`.

Breaking: `EffectOutcomePerformed` gains the field `usage`, so a host that builds one with a struct
literal adds `usage: None`; `SuspensionReason` gains the variant `RunBudget`, so an exhaustive match
handles it; `LoopEnd` gains `charges` and `loom.metering.RunUsageReport` gains `effects`, so code
that builds either with a struct literal sets them.

### Added

- Two model ports in the SDK, so a host embedding it needs to write neither:
  `loom_sdk::llm::LlmModelPort`, the llm-backed port `b10x-loom` runs a plugin turn on, moved from
  the command line into the SDK; and `loom_sdk::loom::wire::ClaudeCodePort`, a port on an
  operator's existing Claude Code login. One turn of it is one `claude -p --output-format json`
  invocation in the port's directory with the port's model, the command's own tools switched off;
  the turn's tools and conversation go to its standard input, a call comes back as a JSON object
  the loop executes, and the turn's usage is the usage the command reports. It is single-attempt,
  bounded in output and time, and a missing or failing command is a typed refusal naming it. Loom
  code reads no credential for either: the `claude` command reads its own login.
- `b10x-loom run --action-bindings <FILE>` runs an installed custom protocol through commands the
  operator binds to its actions (`intake.bindings.ActionBindings`). Each admitted action starts
  its `argv` without a shell. The arguments go to standard input as JSON, the environment keeps
  only `PATH`, and the program is killed after `timeout_seconds` (60 by default). Exit 0 is
  `Performed`, with standard output (one JSON object) as the report. Any other end is `Refused`.
  The report may list evidence, and each kind must be one the action declares in `may_produce`.
  An action left unbound stops the run `NoLocalExecutor` naming it. A file that binds an action
  the protocol does not declare is refused before any model call. A binding confers no authority.
  Bound programs are not confined by Substrate, so `run` starts them only when the operator opts
  in, with `--unconfined-bindings` or with the file's own `"unconfined_bindings": true`
  (`intake.bindings.ActionBindings` gains the optional `unconfined_bindings`), and reports on
  standard error which one opted in; the flag wins when both are given. Without an opt-in the
  run is refused before any model call with `stopped: ConfinementUnavailable (CommandUnconfined: ...)`,
  exit status 3 (`intake.confinement.ConfinementRefusal` gains `CommandUnconfined`). `resume`
  still continues only `software-change@1`.
- Explicit decisions through the governor: an outcome that requires a decision
  (`requires: decision: <name>`, for example `inconclusive` once a budget is spent) can now become
  legitimate through Loom. `loom.evaluation.EvaluationRequest` gains `decisions`, a list of
  `canon-decisions/1` entries (`ExplicitDecision`: the decision, the outcome, the principal who
  took it and the case revision it was taken at), which `loom_governor::evaluate` and
  `b10x-loom evaluate` hand to Canon; a decision taken at another revision than the snapshot's is
  superseded, and Canon refuses a termination only it would make legitimate. Decisions Canon
  refuses are reported with the new refusal input `Decisions`. `CanonGovernor::decide(case,
  outcome, decision, principal)` records a decision on the held case at its current revision, so
  later `frontier` and `completion` calls evaluate with it until the case moves; Canon refuses one
  for an undeclared outcome or an outcome that does not require it (`DecisionError::Refused`), and
  `FileCaseStore` keeps the decisions in `StoredCase.decisions`, left out when there are none, so
  cases stored earlier read as cases with none. Loom decides nothing about who may take a decision.
  Breaking: `EvaluationRequest` gains the field `decisions` and `CaseState` the field `decisions`,
  so code that builds either with a struct literal adds `decisions: Vec::new()`; and
  `EvaluationInput` gains the variant `Decisions`, so a host that matches it exhaustively handles
  the variant.
- A host declares, when it opens a case on `CanonGovernor`, the independence dimensions on which
  each evidence producer is independent of the case's work (`CaseIndependence`,
  `CanonGovernor::open_with_independence`, `open_case_with_independence`, also
  `loom_sdk::CaseIndependence`), and the governor puts them in the case's `canon-case/1` snapshot.
  A claim whose evidence match demands independence (`independent_on`) is then decided on a
  declared producer's records instead of staying unknown, so an action it guards can become
  admissible. A producer that is not an identifier, or a dimension the protocol does not declare,
  is refused naming it before anything is stored; one listed twice is refused, never replaced.
  `CanonGovernor::independence` reports them. They are held with the case: `FileCaseStore` keeps
  them in `StoredCase.independence` and reads a case stored without them, as 0.23.0 wrote every
  case, as declaring none. `open` and `open_case` declare none, as before. Breaking:
  `CaseState` gains the field `independence`, so a store or test that builds one with a struct
  literal adds `independence: CaseIndependence::none()`; `OpenError` gains `InvalidProducer`,
  `UndeclaredDimension`, `RepeatedProducer` and `RepeatedDimension`, so a host that matches it
  exhaustively handles them.

- Run and action budgets in Commission's runtime. A host's `LoopContext::run_budget` (default
  none) returns a `RunBudget`: steps, input and output tokens, wall time and cost for the whole
  run, and per action (`ActionBudget`) a trial count and the same spending limits. An effect
  reports what it spent on its own model calls with `EffectOutcomePerformed.usage`
  (`EffectUsage`); the runtime charges every effect answer to the run and to its action
  (`LoopEnd::charges`, a `RunCharges`) and observes the usage. A trial limit stops the run before
  the request that would pass it is handed to the effect port; a spending limit stops it once an
  effect's usage took the charge past it, or left the cost unknown under a cost limit. The Run is
  suspended for `SuspensionReason::RunBudget`, naming the action, the limit, its value and the
  charge, distinct from a model-loop `Budget` stop. `RunMeter::charge_effects` puts the charges
  in the run's usage report as `RunUsageReport.effects`, beside the model-port usage.

### Changed

- A governed run stopped by one of its model loop's budgets (turns, input or output tokens, cost,
  wall time, or a ceiling the loop could no longer observe) reports that budget: `Loom::run_loop`,
  `LoopExecutor` and `WorkflowExecutor`'s judgement steps return
  `ExecutorOutcome::Suspended` with `SuspensionReason::LoopBudget`, carrying the loop's stop as
  `loom.run.LoopStop`'s wire writes it (for example
  `{"kind": "max-input-tokens", "limit": 100, "reported": 150}`), and the runtime suspends the Run
  for it; a `LoopExecutor` run whose case moved while it ran is still `CaseMoved`. A host no
  longer runs its budgeted loop outside Commission to read `LoopOutcome.stop`.
  A cancelled, unstructured or provider-incomplete stop, and one above the compaction trigger,
  still end `NoUsefulAction`.

## [0.23.0] - 2026-10-16

Loom 0.23.0 runs a workflow's data collection concurrently and hands a judgement step its inputs
as data. An executor proposes several reads at once (`ExecutorOutcome::ProposedReads`); the
runtime admits them only when each is a read the frontier lists admissible without authority and
none is listed twice, invokes them concurrently through their `Read` bindings, each once, and
ends the run `CaseMovedOn` when a stale read shows the case moved. A judgement step lists the
earlier outputs it is given (`inputs`), and the runtime appends exactly those values to its
instructions as one escaped data block, so the model no longer reads them out of the transcript.
A `RunMeter` counts overlapping and nested executor calls in their own steps. Loom requires ESS
0.60.0. Breaking: `ExecutorOutcome` gains the variant `ProposedReads`, so a host that matches it
exhaustively no longer compiles until it handles the variant; `loom.workflow.JudgementStep` gains
the field `inputs`, so code that builds one with a struct literal adds `inputs: Vec::new()`; and
specifications built with Loom's tasks need ESS 0.60.0.

### Added

- Read fan-out in Commission's runtime: an executor proposes several reads at once as
  `ExecutorOutcome::ProposedReads` (at most `MAX_FAN_OUT`, 16). The runtime admits the fan-out only
  when the effect port reads every listed action (`EffectPort::reads`, `false` by default) and the
  frontier the executor was handed lists each `Admissible`, needing no authority, and no two reads
  list the same action and arguments; otherwise nothing is invoked. Each read is its own action
  request, revalidated before any is invoked; the admitted reads are invoked concurrently through
  `EffectPort::invoke_reads` (a `Sync` port answers with `invoke_concurrently`), each once, joined,
  and recorded and observed in the order proposed. A stale read is not invoked and the others go
  on; the move it reveals is then judged as after a stale proposal, never adopted as the Run's
  own, so the run ends `CaseMovedOn`. An effect error fails the loop after every read answered,
  with nothing retried. A fan-out is one step. `ConnectorEffects` reads exactly the actions whose
  binding's effect is `Read`, through an invoker that opts in with `ConnectorInvoker::concurrent`,
  as `ConnectorsInvoker` does, and its `invoke_reads` invokes nothing when any request's binding is
  not a `Read`. Writes, and every action that needs authority, stay one per iteration.
- A workflow's judgement step lists the earlier outputs it is given (`inputs`, a list of the new
  `loom.workflow.OutputReference`: a step and an output). When the step is proposed, the runtime
  appends exactly those values to its instructions as one `<workflow-inputs>` data block, a JSON
  list naming each value's step, output and declared type, introduced as data and never as
  instructions, with `<`, `>` and `&` escaped so no value can close it; the model no longer has to
  read an earlier result out of the transcript. A listed output the run did not produce, or of
  another type, ends the call `NeedsHumanJudgment` naming the step and the output and asks the
  model nothing. `workflow::load` refuses an input naming no output of an earlier step, and one
  listed twice. Breaking: `loom.workflow.JudgementStep` gains the field `inputs`, so code that
  builds one with a struct literal adds `inputs: Vec::new()`.

### Changed

- A `RunMeter` counts overlapping and nested executor calls in their own steps. The step is
  carried by the executor call, on the thread it runs on and in every fork of a metered port made
  in it, rather than held as one open step per meter: two `MeteredExecutor` calls running at once
  on one meter, on two threads, each count their own model calls, and a `MeteredExecutor` over
  another `MeteredExecutor` of the same meter opens one step for the executor call, not two.
  Delegates side by side still count in their executor call's step, and calls outside any executor
  call in step 0. The contract that executor calls on one meter do not overlap and that an
  executor is wrapped by one `MeteredExecutor` is lifted. A plain metered port (not a fork) turned
  on a thread the executor call spawned now counts in step 0; a fork made in the call counts in
  its step. The report's shape and the `loom.metering` domain are unchanged.
- Loom requires ESS 0.60.0: the Loom, Commission and intake specifications require it, the
  conformance targets build on the 0.60.0 `ess-conformance` and `ess-primitives`, and CI installs
  the 0.60.0 `ess`. Every specification validates unchanged, and synthesis still writes its
  suites with no refusal. `not-in-frontier` stays an `external:` outcome: 0.60.0 has no predicate
  for membership in a list input.

## [0.22.0] - 2026-10-14

Loom 0.22.0 starts deterministic workflows. A run's model usage report (`RunMeter`,
`MeteredModelPort`, `MeteredExecutor`, the `loom.metering` domain) counts every model call of a
Commission run at the `ModelPort` boundary, with the tokens and the cost per step, each step
labelled with the workflow step it took. A workflow definition (`loom.workflow`,
`WorkflowExecutor`) proposes its deterministic steps without a model and asks one only in a
judgement step, among the actions that step and the host both admit, and a step's typed outputs
fill a later step's arguments without a model. `loom-xtask release-check` refuses an entry without
a `Breaking:` sentence, and `Breaking: none.` when a stable surface changed since the previous tag.
A bugfix's `regression_test_result` runs the test the run wrote (`--regression-test-command`), and
the governor takes a record's `observed_at` only as the instant of an observation it received.
rustls moves to 0.23.46 for RUSTSEC-2026-0346, RUSTSEC-2026-0347, RUSTSEC-2026-0348 and
RUSTSEC-2026-0349. Breaking: a bugfix run without `--regression-test-command` no longer discharges
`red_first.required`, `ResumeContextData` gains `regression_test_command`, and the governor
refuses a record that cites an observation it never received or gives it another instant.

### Added

- Workflow definitions (`b10x_loom_executor::workflow`, re-exported by the SDK): a host defines a
  workflow over one protocol as an ordered list of steps, the new `loom.workflow` domain
  (`ess/domains/workflow.yaml`). A deterministic step names one action and literal arguments and is
  proposed without a model; a judgement step names the actions a model may choose among and
  instructions, and runs the governed loop narrowed to those of them the host's own narrowing admits
  (a step never widens it); a step may wait on a frontier claim (`when`). `WorkflowExecutor`
  implements Commission's `AgentExecutor`: it takes the first step after the last performed one
  whose `when` holds, ends the call `NeedsHumanJudgment` naming the step when the frontier does not
  list its action, and never skips ahead on its own. `RecordingEffects` wraps the host's effect port
  so the workflow advances past a step only once its request was `Performed`. `workflow::load` reads
  a YAML definition and refuses, naming the step, an action outside its protocol, a repeated step
  name, a step name with surrounding whitespace or a judgement step with no action. Commission still
  revalidates every proposal and Canon decides admission and completion. `ProtocolEntry::actions`
  lists a protocol's actions for that check. `b10x-loom-executor` now depends on `serde_yaml_ng`. No
  break.
- Typed step outputs in workflow definitions: a step declares `outputs`, each a name, a JSON pointer
  into its performed effect's report and a type (`loom.workflow.StepOutput`,
  `loom.workflow.OutputType`: `String`, `Integer`, `Boolean`, `Json` or a list of one of them),
  and a later deterministic step's arguments name one as `{"$step": <step>, "$output": <name>}`;
  the executor fills it in when the step is proposed, with no model call, and Commission
  revalidates the resolved arguments like any other. An output absent from the report or of
  another type, or one its step did not produce in the run, ends the call `NeedsHumanJudgment`
  naming the step and the output (a new `output` member), and nothing is proposed.
  `workflow::load` refuses a reference to a later, unknown or undeclared output, a malformed
  reference or pointer and an output declared twice. `WorkflowExecutor::outputs` lists what the
  run produced; outputs are kept in process. Breaking for a caller that builds
  `loom.workflow.WorkflowStep` itself: it has the new field `outputs`. A definition that already
  used `$step` or `$output` as a member name in its arguments is now read as a reference.
- A per-run model usage report, counted at the `ModelPort` boundary. `RunMeter`,
  `MeteredModelPort` and `MeteredExecutor` (`b10x_loom_executor::metering`, re-exported by
  `b10x-loom-sdk`) count every `turn` of a Commission run, whoever makes it (the loop, the
  reasoning-model selector, an argument generator, compaction, a forked port), a failed turn
  included, and `RunMeter::report` returns the generated `loom.metering.RunUsageReport`: the
  model calls, the input, output and cached input tokens the provider reported and the cost at the
  run's rate card, in total and per step. A step is one executor call, counted from 1; calls made
  outside any executor call are step 0. A step's `label` names the workflow step the call took when
  a `WorkflowExecutor` is built `.with_meter(&meter)`, and is absent otherwise. A cost no rate
  card priced is absent, never zero; a turn that failed or unwound is a call whose cost is unknown,
  so its step's and the run's cost are absent. Executor calls on one meter do not overlap, and an
  executor is wrapped by one `MeteredExecutor`. New ESS domain `loom.metering`
  (`ess/domains/metering.yaml`). No break for callers: the types are new and nothing existing
  changes.

### Changed

- `loom-xtask release-check` refuses a release whose `CHANGELOG.md` entry has no `Breaking:`
  sentence, and one that says `Breaking: none.` while a stable surface (the pinned consumer
  `consumers/pinned/`, the host-port witness `crates/loom-sdk/tests/host_ports.rs` or the
  run-events contract `docs/contracts/run-events-v1.json`) differs between the previous
  bare-version tag and the tag, naming the entry and the changed files. With no previous tag only
  the sentence is required. `release.yml` checks out the full history and its tags for the diff.
  Breaking: none for consumers of the crates or the command; a release author now writes a
  `Breaking:` sentence in every entry.
- A bugfix's `regression_test_result` is the result of running the regression test the run wrote,
  never of the whole suite. `b10x-loom run --regression-test-command <CMD>` names the command,
  split at white space with `{path}` in at least one word; on a bugfix, after every `tests.run`
  that follows a committing `tests.write`, the slice runs it once per test path those writes
  committed (a path whose bytes at the run equal its bytes before the run's first write to it,
  written with what it held or written and then written back, is none), `{path}` filled, through the same
  confined runner, each run its own observation (`executor::REGRESSION_RUN_SOURCE`), and submits a
  `regression_test_result` from each. The suite's run is a `test_result` only. Without the flag
  no run is a regression result, so `red_first.required` stays open. The resume context keeps the
  command (`intake.routing.SliceRun.regression_test_command`,
  `ResumeContext.regression_test_command`); a context kept by 0.21.0 reads as having none.
  Library callers give it with `TestCommand::with_regression(RegressionTestCommand::new(..)?)`;
  a suite run's regression runs are `TestRun::regressions`. Breaking: a bugfix run without the
  flag no longer discharges `red_first.required`; `ResumeContextData` gains
  `regression_test_command`.
- The governor takes a record's `observed_at` only as the instant of the observation it cites:
  a record that cites an observation the governor never received, with or without an
  `observed_at`, or one of whose `observed_at` members is not the text of each cited
  observation's `observed_at`, is refused (`GovernorUnavailable`, as Commission's evidence port
  names no record) and nothing of it is kept. `CanonGovernor::check_evidence` names the record
  and the reason (`EvidenceRefusal`). Breaking: a host that submits a record before delivering
  the observation it cites, or with another instant, is refused.

## [0.21.0] - 2026-10-14

Loom 0.21.0 lets a bugfix show its regression test failing first, and clears approvals through
Mandate. The slice submits evidence that `software.change/1`'s order and age rules decide: on a
bugfix, a failing run after the first committing `tests.write` is also a
`regression_test_result`, and a committing edit is an `implementation_change`, each with the
host clock's `observed_at` (strictly increasing, whole seconds) and the trusted producer, so a
bugfix that runs its regression test red before it edits discharges `red_first.required`.
`b10x-loom-mandate` answers Commission's authority port with Mandate `v0.9.0`, and
`b10x-loom approve --mandate` and `resume --mandate` record a Mandate approval and consume it
once. The `run --output jsonl` records are a committed V1 contract, the SDK's stable surface
has its page, `task check` builds an external consumer of the SDK, and the migration guide maps
every Harness crate to Loom. Breaking: the slice's items that take a governor are generic over
`T: EvaluationTime`, `executor::Report::Edited` gains `change`, `ExecuteError` gains `Clock`,
and the slice's grant API (`grant::approve`, `GrantData`, `ApproveRefusal`,
`intent::ResumeState`) carries the Mandate approval.

### Added

- The SDK stability page (`website/docs/reference/sdk-stability.md`) names the stable surface of
  `b10x-loom-sdk`: the twenty ports a host implements with their `loom_sdk` paths, the types and
  functions an embedding calls, and every `loom_sdk` path control-plane imports. The surface stays
  stable within a release line and a break is named here under `Breaking:`; items not on the
  page, `#[doc(hidden)]` items and generated model types are outside the promise.
  `crates/loom-sdk/tests/host_ports.rs` implements every listed port through `loom_sdk` paths
  alone and fails when the page and the crate disagree. No API change.
- `b10x-loom-mandate` (`loom_mandate`), a new opt-in crate: `MandateAuthority` answers
  Commission's authority port with Mandate's `check` (Mandate `v0.9.0`). A host builds it with
  its own Mandate ports and puts Mandate's verified context in the commission's authority context
  (`{"verified": …}`, `loom.authority.HostContext`); the capability is the action and the case is
  the resource, of type `loom.case`. A case's resource id is derived from its case id, never
  parsed: the UUID version 5 of the case id's exact UTF-8 bytes under the namespace
  `735b2acd-620b-43ec-b1ab-144c2aec7ba7`, so every case id, the governor's `case-<n>` included, is
  its own resource, and two spellings of one UUID are two. A host computes the same id with
  `loom_mandate::case_resource_id(&CaseId)`, or any UUID version 5 implementation over that
  namespace, when it writes Mandate's graph or policy. Mandate's allow is a grant, its approval
  requirement is Commission's `ApprovalRequired` naming the decision id, so a run through
  `run_with_suspension` holds a Suspension, and its refusals are denials. A Mandate outage, an
  authority context that is no Mandate context and the empty case id grant nothing.
  `b10x-loom approve` and `resume` use it behind `--mandate` (below); `run` does not.
- `b10x-loom approve --mandate <FILE>` and `resume --mandate <FILE>` clear a resume's approval
  through Mandate v0.9.0's `check_with_approval`. `FILE` is a host-trusted Mandate file
  (`loom.authority.MandateHost`: the verified context, the approver, the approver policy, the
  relation, the capabilities and those needing an approval, and `approval_seconds`), from which
  Loom builds Mandate's own doubles for the case until Mandate ships a policy adapter. `approve`
  records a Mandate approval of the held request in the state directory's approval event log
  (`mandate-approvals.jsonl`, `intake.routing.MandateApprovalEntry`) and the grant names it
  (`intake.routing.Grant.mandate_approval`, a member only a grant carrying one has); `resume`
  performs the request only when Mandate clears and consumes that approval, appending
  `ApprovedDecisionRecorded` to the log under the state directory's lock before anything is
  performed. A second resume with the same approval, an expired approval, one naming another
  subject, action or resource, one recorded for another commission or request however its id is
  written, and a grant naming none are refused `NotGranted` (16) and perform nothing. The file's
  approver is neither its verified subject nor its verified actor. Only Mandate clears a grant
  naming an approval: `resume` without `--mandate` refuses it `NotGranted` and performs nothing.
  Without `--mandate` both commands are otherwise unchanged, and a grant written by 0.20.0 still
  reads. `b10x-loom-cli` now links `b10x-loom-mandate` (new `loom_mandate::approval`); the SDK
  does not. The flag and the log go when Mandate records approvals itself.
  Breaking: `b10x_loom_intake_slice::grant::approve` takes a `mandate` argument,
  `intent::ResumeState` gains a `mandate` field, `grant::GrantData` gains `mandate_approval` and
  `grant::ApproveRefusal` gains `Mandate`.
- The `loom.authority` domain (`ess/domains/authority.yaml`) declares that context, the case
  resource and its derivation (`loom.authority.CaseResource`), and the answer each outcome maps
  to. The `loom` specification moves to format `ess/24`; the generated `loom` model gains the
  `authority` module and nothing else changes.
- The `run --output jsonl` records and the exit statuses are a stated contract at `schema_version` 1,
  committed as `docs/contracts/run-events-v1.json` and checked by `task docs-check`: within the
  version only a new optional field of an existing record kind is allowed, and any other change is
  a break named here under `Breaking:` with a new schema version (reference: Run events,
  Stability).
- `task consumer-check`, run by `task check`: `consumers/pinned/`, a crate outside the workspace
  that depends on `b10x-loom-sdk` alone by path, compiles the SDK example
  (`crates/loom-sdk/examples/software_change.rs`) as an external crate would and runs it to
  `ApprovalRequired (repository.merge)`, over the workspace's `Cargo.lock`. A change that breaks
  what an embedder compiles against now fails Loom's own CI before a release. No break for
  callers.
- The guide "Move from Commission, the governor, intake or Harness"
  (`website/docs/guides/move-to-loom.md`) gains a "Move from Harness" section: for each of the 14
  Harness crates, its disposition in the Harness map, the `loom_sdk` path a program imports
  instead of a ported crate (`loom_sdk::loom::harness::wire`, `http`, `responses`, `messages`,
  `turn_loop`, and `loom_sdk::loom::session`) or `beyond10x/harness` as the owner of a crate Loom
  does not carry, and what changes when a program moves: tools are projected from the case's
  frontier, a call is selected and revalidated before Loom proposes it, a consequential call
  leaves Loom only as a proposal, and a wire's credential comes from an llm-credentials
  reference. `harness_map.rs` holds the section to the map, and `loom-sdk`'s `harness_paths.rs`
  compiles every path it names. No API changes.
- The slice submits evidence that `software.change/1`'s order, independence and age rules can
  decide (`intake.routing.SliceEvidence`): every `implementation.edit` that commits submits an
  `implementation_change` about the new `HEAD`, and on a `bugfix` case every `tests.run` after
  the run's first `tests.write` that committed submits a `regression_test_result` beside its
  `test_result`, with the same result and citing the same observation; a `tests.write` that
  wrote what the files already held wrote no regression test and does not count. Every record
  carries `observed_at`, the host clock's instant of its observation, and the verifier's producer. A
  bugfix that writes its regression test, runs it red and then edits now discharges
  `red_first.required`; one that edits before any regression test leaves it open.
- `b10x-loom run` and `resume` build their governor with `run::slice_governor`, which evaluates
  at the host instant (`clock::HostTime`, `clock::ClockTime`, `clock::SystemClock`), so an
  evidence kind's `max_age` (`operational_observation`, 30 minutes) expires an old observation.
- `run::run_with_clock` and `LocalExecutor::with_clock` take the host clock an observation's
  instant is read from; `run`, `run_with_options` and `LocalExecutor::new` read
  `clock::SystemClock`, as do the `intent` functions (the clock they are given stays the time
  query's) and a resume.
- An edit's observation (`executor::EDIT_SOURCE`) is delivered to the governor beside a test
  run's, and the step prints every record it submitted (`evidence: test_result fail,
  regression_test_result fail`; `evidence: implementation_change`).
- The executor's observation instants are strictly increasing in whole seconds, the unit Canon
  orders records by: before it delivers an observation in the same second as its previous one,
  or an earlier one, it waits on the clock (`Clock::wait`, a thread sleep by default) for that
  second to pass, at most `executor::CLOCK_WAIT` (2 seconds). A clock that still has not passed
  it (one that stepped back) fails the step with `ExecuteError::Clock`, naming both instants,
  before the observation is delivered, so no record is submitted for it; an edit already
  committed stays committed. A host clock stepping back no longer shows a regression test first
  for an edit made before it.

### Changed

- `task commission:deps-guard` also refuses any Mandate crate in `b10x-loom-commission`'s graph.
- The governor gives Canon the trusted producer `submit_evidence` attributed a record to
  (`EvidenceData.producer`), never a producer the record's facts name: a `producer` member of the
  facts is removed before Canon reads the record, so one Canon cannot read as a principal (a null,
  a number, an object) no longer refuses it. A trusted producer that is not a Canon identifier is
  given as none. A record's `observed_at` is still read from its facts.
- Breaking: the slice's public items that take a governor are generic over its evaluation time,
  `T: EvaluationTime`, defaulting to `NoEvaluationTime`: `LocalExecutor`, `TestResultVerifier`
  and `LocalEffects` gain a defaulted type parameter, and `run::run`, `run::run_with_options`,
  `case::open`, `case::open_with_catalog`, `case::report_head` and the `intent` functions take a
  `CanonGovernor<S, T>`. `loom-governor`'s public API is unchanged.
- Breaking: `executor::Report::Edited` gains `change: Option<Change>`, so a pattern names it
  `Edited { revision, .. }`; `TestRun` gains `observed_at()` and `after_tests_write()`;
  `ExecuteError` gains `Clock`.

## [0.20.0] - 2026-10-14

Loom 0.20.0 builds on engineering-protocols 0.7.0 and Canon 0.4.0, and every software change
case opens with the case inputs its protocol declares: `risk`, `change_kind` and
`affects_security_boundary`, given by the host and never by a model (`b10x-loom run --risk`,
`--change-kind`, `--affects-security-boundary`; `CaseInputs` for an embedding), checked at open
and on every read, and stored with the case. The slice performs `tests.write`, on the host's
test paths only (`--test-path`), and `implementation.edit` where it performed `repository.edit`,
so a bugfix edits its implementation only after a test failed; it never offers
`release.rollback`, which it cannot perform. An obligation the inputs do not select is not open.
Breaking: `CanonGovernor::open` and `open_case` take the inputs, `OpenError` gains four
variants, `CaseInputs` is built with `try_from_pairs`, `StoredCase` gains `inputs`, the action
`repository.edit` is gone, and a state directory 0.19.0 wrote is not resumed.

### Added

- Case inputs: a case opens with the value of every case input its protocol declares, given by
  the host as trusted context and never by a model (`loom_governor::CaseInputs`, re-exported by
  `loom_sdk`). `software-change@1` declares `risk` (`trivial`, `standard`, `elevated`,
  `critical`), `change_kind` (`feature`, `bugfix`, `refactor`) and `affects_security_boundary`
  (`true`, `false`); `incident-response@1`, `system-query@1` and `inbound-answer@1` declare none
  and open with `CaseInputs::none()`. Canon decides each evaluation with them, and
  `CanonGovernor::inputs` returns them.
- `b10x-loom run --risk <trivial|standard|elevated|critical>` (default `standard`),
  `--change-kind <feature|bugfix|refactor>` (default `feature`) and
  `--affects-security-boundary <true|false>` (default `false`) give a software change its inputs
  (`intake.routing.CaseInputs`; `b10x_loom_intake_slice::case::RUN_DEFAULTS`). With the defaults a
  run reaches the outcomes it reached in 0.19.0; with `--change-kind bugfix` an
  `implementation.edit` is admissible only once a test failed on the current revision.
- The specification's `intake.routing.SliceRun` declares the run's `risk`, `change_kind` and
  `affects_security_boundary`, the inputs of its request; the slice's `run::SliceRun` leaves them
  with the request the caller holds, as it leaves `run_id`. A resume's request carries the inputs
  the held case opened with, read from the governor.
- `tests.write` writes only test paths: the slice performs it only when every path it changes
  matches one of the host's test-path patterns, and otherwise refuses it before anything is
  written or committed, naming the first path outside them (`executor::ExecuteError::NotATestPath`,
  a refused step the agent is told about). The rule holds for every change kind, so a bugfix
  cannot change its implementation through `tests.write` before a test failed;
  `implementation.edit` is not restricted. The patterns are the host's trusted context:
  `b10x-loom run --test-path <GLOB>`, repeatable, replacing the defaults `tests/**`,
  `**/tests/**`, `test/**`, `**/test/**`, `**/*_test.*`, `**/test_*.*`, `**/*.test.*` and
  `**/*.spec.*` (`test_paths::TestPaths`, `LocalExecutor::with_test_paths`). The specification
  declares them as `intake.routing.SliceRun.test_paths`, and `intake.routing.ResumeContext` keeps
  them, so a resume holds `tests.write` to the same paths.
- `CanonGovernor::protocol` names the protocol a case is held under.

### Changed

- Loom builds on `b10x-canon-engineering` `0.7.0` and `b10x-canon` `0.4.0` (was `0.3.0` and
  `0.1.0`); `Cargo.lock` holds one copy of each. A consumer that adds Canon itself moves to
  `tag = "0.4.0"`. `loom-protocols` records revision `0.7.0`, and the bundled catalogue gains
  `ml-experiment@1`, which no slice executes (`NoLocalExecutor`).
- Breaking: `CanonGovernor::open` and `open_case` take the case inputs as a third argument, and
  refuse, before anything is stored, an input the protocol does not declare
  (`OpenError::UndeclaredInput`), a value its input does not list
  (`OpenError::UnlistedInputValue`) and a declared input left out (`OpenError::MissingInput`),
  each naming the input; an exhaustive match of `OpenError` adds the three. `CaseState` gains
  `inputs`. `case::open` and `case::open_with_catalog` take the host's `CaseInputs`, and
  `SliceRequest` and `IntentRequest` gain `inputs` and `test_paths`.
- Breaking: `CaseInputs` refuses a second value for one input, never replacing the first:
  `CaseInputs::with` returns `Result<CaseInputs, OpenError>`, `FromIterator` is replaced by
  `CaseInputs::try_from_pairs`, and `OpenError` gains `RepeatedInput` (an exhaustive match adds
  it). `case::governed_inputs` returns a `Result`.
- Breaking: a held case whose inputs do not satisfy its protocol (a declared input missing, an
  empty list included, an undeclared one, an unlisted value) is never decided: every read of it
  through the governor (`frontier`, `completion`, `current_revision`, `revisions`, `inputs`,
  `evidence`, evidence received) answers `GovernorUnavailable`, and `FileCaseStore` refuses to
  read such a case of a built-in protocol as a store failure naming the case and the input. A
  case of a protocol the governor no longer has registered is likewise `GovernorUnavailable` on
  every read.
- Breaking: `intent::resumed_request` takes the governor, and its request carries the held case's
  inputs (was the run defaults). A resume context without `test_paths`, as 0.19.0 wrote every
  one, is refused as a state failure, like the case 0.19.0 stored beside it.
- Breaking: `loom.governor`'s `StoredCase` gains `inputs`, a list of `{input, value}`.
  `FileCaseStore` refuses a case file without it as a store failure naming the case (`case <id>
  does not read: ...`), never filling in defaults: a state directory 0.19.0 wrote a case into is
  not resumed by this release; start a new run.
- Breaking: the slice performs `tests.write` and `implementation.edit`, the two write actions of
  `software.change/1`, where it performed `repository.edit`, with the same effect (a committed
  write reporting the new `HEAD`); the selector's prompt and argument schema name both, and
  `executor::EDIT` is replaced by `TESTS_WRITE` and `IMPLEMENTATION_EDIT`. Scripts and
  hosts that name `repository.edit` name one of them.
- An obligation is open only when Canon reports it `open`: one the case's inputs do not select
  (`not_applicable`) is not reported owed (`FrontierObligation.open`, `DecidedObligation.open`).
  With the default inputs `red_first.required` and `security_review.required` do not apply; where
  they apply they stay open, since the slice submits no regression test result or security
  review.
- The slice reads a software change's frontier without the actions it can never perform
  (`run::never_performed`, `run::PERFORMED`, `run::SliceReads` for an embedder's own loop):
  `release.rollback`, new on that frontier, is never offered to the agent, never awaited at an
  approval gate and never held as a Suspension; a proposal of it is refused as not in the
  frontier. `SliceReads` reads a `CanonGovernor` and filters only a `software-change@1` case's
  frontier: on any other protocol (an incident's `release.rollback`, `traffic.shift`) it passes
  the frontier through unchanged.

## [0.19.0] - 2026-10-13

Loom 0.19.0 lets a host settle a hold before anything is held. Commission's `HoldSuspension` gains
the outcome `settled`: a suspension port that answers it leaves no Suspension and nothing to
approve, and the run ends as a loop without a port ends it. `b10x-loom resume` answers it for a
merge the agent proposes again at a `HEAD` its commission already handed off, so the run stops
`MergeHandedOff`, nothing waits on an approval, and `approve` answers `NothingSuspended`. The
state directory and the governor's `FileCaseStore` and `FileSuspensionStore` read only regular
files as records; a symbolic link is refused, never followed. Breaking for exhaustive matches of
`HoldSuspensionOutcome` (`Settled`) and `LoopFailure` (`NotResumed`), and for a run store given to
`run_with_suspension` that does not implement `ResumeRunBehavior`.

### Added

- A suspension port can answer that the host already settled a hold: Commission's
  `HoldSuspension` gains the outcome `settled`, carrying `SuspensionSettled` (the Suspension, the
  action request and the host's reason). `run_with_suspension` and `resume_after_grant` then hold
  nothing, put the Run they suspended back `Running` through `ResumeRun`, and end the run as a
  loop without a port ends it, with no loop error; a resume of the commission finds nothing to
  resume. This is breaking for Rust callers: `HoldSuspensionOutcome` gains `Settled` and
  `LoopFailure` gains `NotResumed` (a `ResumeRun` that fails after a settled hold), which break an
  exhaustive match, and `run_with_suspension`'s run store must implement `ResumeRunBehavior`, as
  `RunStore` does.

### Changed

- A merge the agent proposes again, in `b10x-loom resume`, at a `HEAD` its commission already
  handed off holds no Suspension: the resume's suspension port
  (`b10x_loom_intake_slice::handoff::SettledHandoffs`) answers its hold `settled`, the run stops
  `stopped: MergeHandedOff (<request>, HEAD <sha>)`, the state directory keeps no Suspension and no
  resume context for the commission, and `approve` is refused `NothingSuspended` (10).
  `approve`'s `AlreadyHandedOff` refusal (23) stays as a defence.
- The slice's state-directory stores (`GrantStore`, `ResumeContextStore`, `MergeHandoffStore`)
  read only regular files as records: a grant, resume context or hand-off record that is a
  symbolic link (to a record elsewhere or to nothing), a directory or any other entry is refused as
  a store error and never followed. A symlinked hand-off record no longer settles a resume's hold
  of a merge; the hold is `storage-failed` and nothing is held. A symlinked resume context is
  listed as unreadable. The governor's `FileCaseStore` and `FileSuspensionStore` read their
  files the same way: a case or Suspension file that is not a regular file is a store error.
- Commission's conformance check (`task commission:conform`) also runs the authored scenarios
  `ess/commission/ess-inputs.yaml` lists, which read the stores after `HoldSuspension` `settled`
  and `storage-failed` and `StartRun` `storage-failed`, and fails when a scenario file under
  `ess/commission/scenarios/` is not listed.

## [0.18.0] - 2026-10-13

Loom 0.18.0 lets the command line keep a run that waits on an approval and continue it after the
grant. A host keeps a commission's Suspension in a directory with
`loom_governor::FileSuspensionStore`, beside the case in a `FileCaseStore`.
`b10x-loom run --state <DIR>` keeps the run there and holds the commission's Suspension when it
stops on a withheld approval; `b10x-loom approve` writes the grant and performs nothing, and
`b10x-loom resume` continues the run after it, performing the suspended request once. A granted
merge is handed off as a record of the commission, the request, the workspace and its `HEAD`; no
Git ref moves and merging happens outside Loom. Without `--state`, `run` prints and exits as
before. This is breaking for Rust callers of the slice and the command-line crate:
`SliceError` gains `State` and `Resume`, and `b10x_loom_cli::Command` gains `Approve` and
`Resume`, which break an exhaustive match; `SliceRun` gains `gate` and `RunArgs` gains `state`,
which break a struct literal. The slice's functions now take any `FallibleCaseStore`, which every
`CaseStore` is, and the new exit statuses belong to `run --state`, `approve` and `resume` only.

### Added

- A production suspension store: `loom_governor::FileSuspensionStore` keeps each commission's
  Suspension as one file in a directory, so a host can pass it to `run_with_suspension` and
  `resume_after_grant` and resume a Run after a restart. It takes the lock and makes the
  atomic write `FileCaseStore` makes, through the same code, so the two may share one state
  directory, in one process or in two. A commission id over `LONGEST_COMMISSION_ID` bytes is
  refused. The testkit's `FileSuspensionStore` now re-exports it, and
  `b10x-loom-commission-testkit` depends on `b10x-loom-governor`.
- `b10x-loom run --state <DIR>` keeps the run in a directory: the case in a `FileCaseStore`, the
  commission's Suspension in a `FileSuspensionStore`, and the commission's resume context
  (`intake.routing.ResumeContext`: the commission, case and agent revision, the intent, the
  accepted route, the workspace, the test command, `max_steps`, the confinement and cgroup root,
  the agent model and the `--catalog` path), kept before the commission runs and removed when the
  run ends without the commission's Suspension held (`intake.routing.DropResumeContext`). The
  commission runs through `run_with_suspension`, so a run that stops `ApprovalRequired` on a
  proposed merge leaves a `Pending` Suspension a later resume can continue. A state directory
  that cannot be written, a read-only one an earlier run used included, and a workspace, cgroup
  root or catalog path that is not UTF-8 stop the run before any model call. Otherwise output and
  exit status are unchanged, except that a run that stops `ApprovalRequired` holding no
  Suspension prints `stopped: ApprovalRequired (<action>), nothing suspended` and exits 4: only
  `stopped: ApprovalRequired (<action>)` leaves something `approve` can approve. Without `--state`
  `run` prints and exits as before. In the slice,
  `run_prepared_intent_with_state` takes a `RunState`, `ResumeHost::check` checks what a context
  keeps as text, and `b10x_loom_intake_slice::resume::ResumeContextStore` keeps the contexts under
  the stores' lock with their atomic write, removes one (`remove`), and lists them as
  `ResumeContexts`: the contexts it read and each file it could not. A process that dies before
  its run ends can leave a context with no Suspension, so a reader pairs each context with its
  commission's Suspension. `PreparedIntent` types its route (`accepted_route`, `restore_route`),
  and the slice's functions take any `FallibleCaseStore` instead of only a `CaseStore`.
  `SliceError` gains `State`, which breaks an exhaustive match.
- `b10x-loom approve <COMMISSION> <CAPABILITY> --state <DIR>` approves the request a
  commission's Suspension holds: it writes a grant record (`intake.routing.Grant`: the commission,
  the action, the capability, the request it grants and when, mirroring Commission's
  `AuthorityDecision`) into the directory and performs nothing. It refuses, writing nothing, when
  the commission holds no Suspension (exit 10), the Suspension is being resumed (11), the
  suspended request needs another capability (19), or the suspended request is a
  `repository.merge` that would hand off a `HEAD` the commission already handed off
  (`AlreadyHandedOff`, 23; `intake.routing.HeadAlreadyHandedOff`): the same work is not handed
  off twice, and only new work, a new `HEAD`, is approved again.
- `b10x-loom resume <COMMISSION> --state <DIR>` continues the suspended run after the approval,
  through `resume_after_grant`: it rebuilds the run from the commission's resume context, only for
  the intent, protocol catalog and context policy its route was accepted for, validates the
  workspace again and reports its `HEAD` to the case as its implementation revision, so a commit
  made after the Suspension is refused `CaseMoved` (14) with nothing handed off, reads the grant
  through an authority provider that allows only the request the commission's Suspension holds
  while it is held (`b10x_loom_intake_slice::grant`, `GrantAuthority`), performs that request
  once, releases the Suspension and runs on as `run` does; a run that ends without a Suspension
  removes its context. A granted `repository.merge` is
  a merge hand-off: a record of the commission, the request, the workspace and its `HEAD`
  (`intake.routing.MergeHandoff`, `merge-<hex>.json` in the state directory, recorded once per
  request and once per `HEAD` of a commission through `RecordMergeHandoff`, which refuses another
  request's record of a handed-off `HEAD` as `head-already-handed-off`, and only at the
  implementation revision the case holds); no Git ref moves and merging happens outside Loom
  (`b10x_loom_intake_slice::handoff`, `MergeHandoffStore::handed_off`). A resumed run that stops
  at its human gate says what is left to approve: `stopped: ApprovalRequired (<action>)` when the
  commission holds a Suspension, `stopped: ApprovalRequired (<action>), nothing suspended` (exit
  4) when it holds none, and `stopped: MergeHandedOff (<request>, HEAD <sha>)` (exit 0) when the
  case is at a `HEAD` the commission already handed off; `approve --help` and `resume --help` say
  which line can be approved. `SliceRun` gains `gate` (`ApprovalGate`), which breaks a struct
  literal, and `b10x_loom_cli::run_exit_status` gives a run's status. Each refusal of
  Commission's resume has its own exit status: `NothingSuspended` 10, `InDoubt` 11,
  `OtherRequest` 12, `OtherCommission` 13, `CaseMoved` 14, `NotAdmitted` 15, `NotGranted` 16,
  `NotPerformed` 17, `RunNotSuspended` 18; a suspension store that cannot be read is 20, a
  governor that cannot answer 21, and a resumed loop that stops without an outcome 22. The resumed model starts with an empty briefing.
  `SliceError` gains `Resume`, which breaks an exhaustive match, and `b10x-loom-cli` depends on
  `b10x-loom-commission`.
- `FileCaseStore::open` and `FileSuspensionStore::open` prove their directory takes a write:
  holding the lock, they create, write, sync and remove a temporary file, so a directory that
  has become read-only is refused when a store opens rather than at its first write.

## [0.17.0] - 2026-10-13

Loom 0.17.0 lets a Commission host keep a Run that waits on an approval across a process
restart. `ess/commission/` declares a commission's Suspension: the suspended Run, the pending
action request and the reason, held, taken once and released through a port the host
implements. Given the port, `run_with_suspension` holds the Suspension on a withheld approval and
`resume_after_grant`, in the same or a new process, performs the pending request once after the
grant, at the case revision it was suspended at; a resume that finds nothing, a moved case, a
Suspension already being resumed or one held for another request performs nothing. Callers of
`run_until_blocked` see no change. This is breaking for callers that match `ResumeRunOutcome`
or `LoopFailure` exhaustively: both gain variants.

### Added

- A suspension port for Commission's runtime (`b10x_loom_commission::ports::suspension`,
  `SuspensionPort`), which the host implements over durable storage. Given one,
  `run_with_suspension` suspends a Run whose request needs authority the provider withholds
  (`SuspendRun` with `SuspensionReason::Authority`) and stores the commission's Suspension: the
  Run, the pending action request and the reason, as `ess/commission/` now declares it. The
  caller still gets the Run's `NeedsAuthority` end. `resume_after_grant` continues the same Run
  from the stored Suspension, in a new process if need be: after the grant it performs the
  request once, at the case revision it was suspended at, and refuses, performing nothing, when
  nothing is stored or the case revision moved. Callers of `run_until_blocked`, which passes no
  suspension port, see no change in how a Run ends. The testkit's `FileSuspensionStore` is a
  file-backed store for tests, and `SuspensionStore` the port over memory.
- A Suspension is taken once, as `ess/commission/` declares it: the port answers
  `HoldSuspension`, `TakeSuspension` and `ReleaseSuspension`, each atomically. A resume takes the
  Suspension (`Pending` to `Resuming`) for the request it read and revalidated before it performs
  anything, and releases it for that request once the effect port answered and the governor
  observed it. Of two resumes racing over one Suspension, one takes it and performs the request;
  the other is refused (`ResumeRefusal::NothingSuspended` or `InDoubt`) and performs nothing. A
  take or a release naming another request than the Suspension holds is refused (`other-request`,
  `SuspensionRequestMismatch`) and changes nothing, so a resume that read a Suspension the
  commission has since resumed and replaced with one for another request is refused
  `ResumeRefusal::OtherRequest`, naming both requests, and performs nothing. A resume that stopped
  between take and release (a crash, a failing effect port or governor) leaves the Suspension
  `Resuming`: every later resume is refused `ResumeRefusal::InDoubt`, naming the request, and
  performs nothing; settling it is the host's. A release the port refuses after the request was
  performed ends the resume with `LoopFailure::SuspensionNotReleased`, naming the performed request
  and what the port answered. `run_with_suspension` starts no Run of a commission that holds a
  Suspension in either state (`LoopFailure::CommissionSuspended`) and performs nothing.

### Changed

- **Breaking:** `ResumeRun` declares a `storage-failed` outcome (`ResumeRunOutcome::StorageFailed`,
  `RunStorageFailed`), so it is no longer generated whole: a run store implements
  `ResumeRunBehavior` itself, as `RunStore` does, and `Generated` forwards it. A caller that
  matches `ResumeRunOutcome` exhaustively adds an arm; a run store of its own used through
  `Generated` for `ResumeRun` implements the obligation.
- **Breaking:** `LoopFailure` gains `CommissionSuspended`, `SuspensionStore`, `SuspensionNotHeld`
  and `SuspensionNotReleased`, which only `run_with_suspension` and `resume_after_grant` report. A
  caller that matches `LoopFailure` exhaustively adds the arms; `run_until_blocked` reports none
  of them.

## [0.16.0] - 2026-10-12

Loom 0.16.0 lets a Run continue past a step performed under an authority grant. When the
authority provider allowed a step's request and the effect port performed it,
`run_until_blocked` runs the executor again instead of ending the Run `AwaitingApproval`; the
step budget and idle bound still end a Run that only repeats granted steps, and a Run whose
budget runs out after a performed effect ends `Completed` when that step completed its case. A
plugin turn is one Run. Loom now requires ESS 0.59.0. This is breaking for callers that matched
`AwaitingApproval` after a granted effect and started another Run to get past it: they now get
the Run's own end.

### Changed

- **Breaking:** a step performed under an authority grant passes Commission's approval gate.
  When a step's request needed authority, the authority provider allowed it and the effect port
  performed it, `run_until_blocked` runs the executor again instead of ending the Run
  `AwaitingApproval`. Every other step reads the gate as before, and the step budget and idle
  bound still end a Run that only repeats granted steps. With the step budget used up after a
  performed effect, the Run first loads the case: a case that step completed ends the Run
  `Completed`, and only a case still open is suspended for its budget. A caller that matched
  `AwaitingApproval` after a granted effect, and started another Run to get past it, now gets the
  Run's own end. A plugin turn is one Run: the turn no longer starts another Run at a granted
  gate. The rule and its cases are in `docs/commission/design/approval-gate.md`.
- Loom requires ESS 0.59.0: the Loom, Commission and intake specifications require it, the
  conformance targets build on the 0.59.0 `ess-conformance` and `ess-primitives`, and CI installs
  the 0.59.0 `ess`. Every specification validates unchanged, and synthesis still writes its
  suites with no refusal. `not-in-frontier` stays an `external:` outcome: 0.59.0 has no predicate
  for membership in a list input.

## [0.15.0] - 2026-10-10

Loom 0.15.0 records how a selection was made. A confidence fallback is recorded as two
selections: the fast one, now `Overruled` and naming the selection that replaced it, and the
stronger selector's, each with the strategy of the selector that made it. Every selection gets
a `loom.run.SelectionRecord` (strategy, candidates, chosen action, confidence, the strategy a
fallback went to, latency and tokens), and a governed run counts the selections refused at the
execution boundary. Loom now requires ESS 0.57.0. This is breaking for callers that expect one
`Hybrid` selection on a fallback, and for code that builds `SessionData` or
`selection::Pick`.

### Added

- Selection telemetry for Metaharness. Every selection Loom records gets one
  `loom.run.SelectionRecord`, written with it and returned by `Loom::selection_records`: the
  strategy of the selector that made it, the number of candidates, the chosen action, its
  confidence, the selector's latency in milliseconds and the input and output tokens it reported.
  On a confidence fallback, `fell_back_to` is on the record of the overruled fast selection and
  names the strategy of the selector that replaced it; the replacement's record has none. A
  selection refused at the execution boundary after it was made, by Commission's admission or by
  revalidation, raises its governed run's `Session.boundary_refusals` by one and adds no record.
  Neither is evidence. `ess/domains/run.yaml` gains the entity `loom.run.SelectionRecord`, owned by
  `loom.run.Selection`, the commands `loom.run.RecordSelection` and `loom.run.CountBoundaryRefusal`,
  their events, the view `loom.run.SelectionRecords`, and `Session.boundary_refusals`.

### Changed

- **Breaking:** the generated `SessionData` gains `boundary_refusals`. A caller that builds the
  session it hands `Loom::run_loop` or `LoopExecutor` sets it to `0`; a resume compares the session
  without it. The filed session file keeps its format and does not carry the count.
- **Breaking:** `selection::Pick` gains `latency_ms`, `input_tokens` and `output_tokens`, and
  `selection::Resolved` gains `pick` and `overruled_pick`. A selector that overrides
  `ActionSelector::resolve` fills them; the default `resolve` times `select` and reports no tokens.

- **Breaking:** a selection on a confidence fallback is now recorded as two selections, each with
  its own selector's strategy, where there was one `Hybrid` selection. `Loom::selections` holds the
  fast selection first, in the new terminal state `Overruled` and naming the stronger selector's
  selection in the new field `replaced_by`, then that selection, which alone reaches argument
  generation and revalidation. A fast choice that is accepted is recorded as one selection with the
  fast selector's strategy and no replacement; a fast selector that errs or names an action outside
  the candidates leaves no fast selection. `loom.run.Selection` gains `replaced_by` and `Overruled`,
  and the new command `loom.run.OverruleSelection` emits `loom.run.SelectionOverruled`
  (`ess/domains/run.yaml`). `ActionSelector` gains the provided method `resolve`, which
  `HybridSelector` overrides; `RequestRecord::overrule` refuses, recording nothing, a replacement
  the record does not hold (`SelectionNotFound`), the selection itself, a replacement no longer
  `Selected` and one made from another catalogue (`OverruleRefused`). `Loom::select` returns the
  pick under the strategy of the selector that made it, as `Loom::run` records it, never `Hybrid`;
  `selection::select` still records the strategy of the selector it is handed. Callers that expect
  one `Hybrid` selection per run, or `Hybrid` from `Loom::select`, must read the strategies instead.

- Loom requires ESS 0.57.0: the Loom, Commission and intake specifications require it, the
  conformance targets build on the 0.57.0 `ess-conformance` and `ess-primitives`, and CI installs
  the 0.57.0 `ess`. Every specification validates under 0.57.0's new refusals (a wire name
  containing `/`, a refusal whose `when:` always holds) without change. Regenerating changed only
  documentation: a doc comment on the generated Commission `Generated` type, and the Commission
  domain graph page, which names the new compiler.

## [0.14.0] - 2026-10-09

Loom 0.14.0 generates `LoopStop` from its ESS declaration. `loom.run.LoopStop` in
`ess/domains/run.yaml` (format `ess/22`) declares every cause the governed loop stops for, and
the enum the loop uses is generated from it; a codec keeps the JSON of loop outcomes and events
byte for byte. This is breaking for embedders that match `LoopStop`: its variants are tuple
variants over payload structs, counts are `i64`, and `is_completed` is removed. A new test holds
the Laya selection slice end to end.

### Added

- `crates/loom-selector-laya/tests/laya_arguments_slice.rs` holds the Laya selection slice end to
  end: Loom runs as Commission's executor on a scripted three-action frontier, selects with the
  Laya selector behind the confidence fallback to the reasoning-model selector, generates
  arguments once and only for the finally selected action, and an action outside the frontier
  (`release.rollback`) never reaches argument generation or revalidation.

### Changed

- **Breaking:** `LoopStop` is generated from `loom.run.LoopStop` in `ess/domains/run.yaml` (format
  `ess/22`); the hand-written enum is gone. Its variants are tuple variants over payload structs
  (`LoopStopMaxTurns`, `LoopStopMaxInputTokens`, `LoopStopMaxOutputTokens`, `LoopStopMaxCost`,
  `LoopStopBudgetUnobservable`, `LoopStopDeadline`, `LoopStopCancelled`, `LoopStopAwaitingApproval`,
  `LoopStopProviderIncomplete`, `LoopStopUnstructured`, `LoopStopContextAboveTrigger`), re-exported
  from `harness::turn_loop`; `Completed` stays a unit variant. Every count field is `i64` (was
  `u64`, and `u32` for `asked_again`). `LoopStop::is_completed` is removed; compare with
  `LoopStop::Completed`. `LoopStop` no longer implements `Serialize`/`Deserialize` on its own;
  `LoopOutcome` and `LoopEvent` write the same JSON as before. Reading is stricter: a `completed`
  stop with extra fields is refused, as are negative, fractional or out-of-range numbers.

## [0.13.0] - 2026-10-09

Loom 0.13.0 adds a plugin layer: an unattended agent is a Loom plugin, hooks around a governed
run. `b10x-loom-plugin` hosts a plugin in a loop with state, retries and a model budget per turn;
every turn runs on the new read-only protocol `inbound-answer@1`; `loom_connectors::cli` reads data
sources through the `connectors` command line and refuses writes; and `b10x-loom plugin run
slack-handler` walks a bot's Slack channels and records proposed answers to unanswered messages,
posting nothing. New crates add no breaking change to existing ones; the minor version moves for
the new commands and domains.

### Added

- `protocols/inbound-answer/1.yaml`, registered `inbound-answer@1`: a read-only protocol that
  answers an inbound item from data sources. It offers `source.read` (`effect: read`, capability
  `datasource.read`), `reply.propose` (`effect: none`, capability `reply.propose`; records a
  proposal and never sends) and `reply.decline` (`effect: none`, no capability). It is `proposed`
  only with a proposed reply and at least one source read, and `declined` with a recorded decline;
  once one of the two is recorded, the other action is blocked.
- `ProtocolCatalog::plugins()`, the plugin host's catalog, holds `inbound-answer@1`.
  `ProtocolCatalog::bundled()`, which routing and `protocols list` read, does not.
- `b10x-loom-plugin`, the plugin host, and the `loom.plugin` ESS domain it is written against. A
  `Plugin` has the hooks `poll`, `classify`, `project`, `turn`, `result` and `objectives`, and
  `run_plugin(plugin, config, state, stop)` runs them in a loop until `stop` is set (a `stop`
  already set runs one cycle). The default `classify` makes one forced `classify_item` call; an
  item below `classify_threshold` (default 0.5) is recorded `unclassified` and gets no turn. The
  default `project` admits `source.read`, `reply.propose` and `reply.decline` over configured
  sources for ask, find and request, and nothing for a task, which records the protocol the
  router picks from `ProtocolCatalog::bundled()` as a proposed case. A turn runs Commission's
  `run_until_blocked` on `inbound-answer@1` with Loom's governed model loop, an authority
  provider granting only `datasource.read` and `reply.propose`, and `DataSourceEffects`, which
  reads through the Connectors command line, refuses a read outside the turn's projection and
  submits one `source_read` evidence per performed read. A turn sends at most 8 model requests,
  retries included. A proposal is written to the record and never sent. An item whose hook fails
  is kept in the state and retried each cycle, and its third unsuccessful attempt records it
  `stopped`; an outage that is not about the item counts against none. A poll that fails is an
  outage of the whole host: the cycle saves no cursor and the host polls again after
  `poll_interval_seconds`, returning the failure only when `stop` ends it after that cycle
  (`once`). A `stop` set while the host runs ends it after the item being handled. Each handled
  item adds one line to `record.jsonl`, which also decides what is handled, and a torn last line
  is dropped; `state.json` is written through a temporary file and a rename. One host holds a state
  directory at a time, and a state directory inside a configured workspace root or checkout is
  refused.
- `b10x-loom-plugin-slack`, the slack-handler plugin, and the `loom.slack` ESS domain of its
  configuration (`SlackConfig`, which holds the host's `PluginConfig`). Its poll reads through the
  Connectors command line only, with the configured Slack adapter, connection and three read
  operations (`conversations.list`, `conversations.history`, `conversations.replies`). It walks the
  channels the bot is a member of, ordered by the heaviest objective each serves, then by how long
  ago the newest message stored for it was posted, then by member count, then by a draw from the
  configured seed; reads each history from the channel's stored `ts` (a channel never read from
  `lookback_minutes` ago, default 1440); and reads a thread only for a message with replies. A
  message is an item once it has aged `min_age_minutes` and when it is no bot's, no system
  message, has no reaction by the bot and no thread reply from anybody but its poster; items
  naming the bot come first. A configuration without the bot's user id, the Slack adapter or
  connection is refused before anything runs.
- `b10x-loom plugin run <PLUGIN> --config <file> --state <dir> [--once]` hosts a registered plugin
  (`slack-handler`), with `--catalog`, `--model` and `--classifier-model` as `run` takes them;
  `b10x-loom plugin report <PLUGIN> --state <dir>` prints one line per recorded proposal. An unknown
  plugin name is refused naming the registered ones. A plugin turn runs an llm model through
  `model_port::LlmModelPort`, the bridge from an llm `Model` to the harness `ModelPort` Loom's
  governed loop calls. Without `--once`, SIGTERM or SIGINT ends the host after the item being
  handled; a second one exits at once with status 130.

## [0.12.0] - 2026-10-09

Loom 0.12.0 adds two action selectors and bounds compaction. `ReasoningModelSelector` asks any
model port to choose one of the given candidates; `HybridSelector` takes a fast selector's choice
at or above a confidence threshold the host supplies, else the stronger selector's; and the
experimental `b10x-loom-selector-laya` crate is a fast selector over a Laya endpoint. A compaction
of a declared context window now leaves the session at or below its target wherever removing what
may be removed can reach it, and a run whose session stays at or above its trigger ends with the
new `LoopStop::ContextAboveTrigger` instead of sending another request. Callers that match
`LoopStop` exhaustively add the arm.

### Added

- `ReasoningModelSelector`, an `ActionSelector` with strategy `ReasoningModel`: one turn of any
  `ModelPort`, held to a single tool whose only argument is a fixed choice of the candidate ids; an
  action outside them is refused as `not-in-catalogue`, and an answer naming none is
  `SelectorError::Unavailable`.
- `HybridSelector`, an `ActionSelector` with strategy `Hybrid`: the fast selector's choice when it
  names a candidate at or above a confidence threshold the host supplies (no default; refused as
  `InvalidThreshold` unless a decimal in [0, 1]), else the stronger selector's answer. `Confidence`
  compares a confidence as a number, so `0.9` equals `0.90`. `select` now drops, for every
  selector, a confidence that is not a decimal in [0, 1] and records the selection without one.
- `b10x-loom-selector-laya`, an experimental `FastTyped` action selector that asks a Laya
  endpoint (`POST /v1/systemone`) to choose one candidate and reports its `answer_confidence` as
  the selection's confidence; a choice outside the candidates, a failure or a timeout is a
  selection error, and neither `b10x-loom-cli` nor `b10x-loom-sdk` links it.

### Changed

- A compaction of a declared context window leaves the session at or below its target (50 % of
  the window) wherever removing what may be removed can reach it. Wherever a fold is not replaced
  by a summary (the summary turn failed on the wire, wrote nothing, or wrote a summary that would
  leave the session above its target, or the fold was too small to be worth a summary turn), its
  items are elided behind one `ELISION_MARKER` item that says how many, how many bytes and why. A
  summary is kept when the session it leaves is at or below the target; above it, only when it is
  smaller than that note. A failed or empty summary is still not a failed run.
- **Breaking:** `LoopStop` gains `ContextAboveTrigger { window, target, occupied }`, all in tokens.
  When what no compaction removes (the task, provider reasoning items, the newest turn group and
  protected tail, and the instruction and tool schemas the provider counts) leaves the session at
  or above its compaction trigger (80 % of the window), the run ends with it and sends no further
  request; its session is filed `Stopped`. Callers that match `LoopStop` exhaustively add the arm.

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
