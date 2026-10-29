---
title: SDK stability
sidebar_label: SDK stability
sidebar_position: 4
description: The stable surface of b10x-loom-sdk, the ports a host implements and the loom_sdk paths control-plane imports, with the promise Loom makes for them.
source: crates/loom-sdk/src/lib.rs, crates/loom-sdk/src/runtime.rs, crates/loom-sdk/src/models.rs, crates/loom-sdk/src/agent.rs, crates/loom-sdk/tests/host_ports.rs, crates/loom-session-executor/src/lib.rs, control-plane's crates/control-plane-runtime (its loom_sdk imports), CHANGELOG.md
---

# SDK stability

`b10x-loom-sdk` (library `loom_sdk`) is the one crate an embedding depends on. This page names
the part of it Loom keeps stable: the ports a host implements, the types and functions an
embedding calls, and every `loom_sdk` path control-plane imports. It is hand-written and held by
[`crates/loom-sdk/tests/host_ports.rs`](https://github.com/beyond10x/loom/blob/main/crates/loom-sdk/tests/host_ports.rs):
that test implements every port below through `loom_sdk` paths alone, has one `use` per
control-plane path, and fails when either table here and the code disagree.

## The promise

- The surface on this page stays stable within a release line. A release that breaks any of it
  names each break in its `CHANGELOG.md` entry, in a bullet that starts `**Breaking:**`.
- Loom releases from source at bare-version tags and nothing is on a registry. A consumer pins a
  tag, `tag = "<version>"`, and moves by changing it:

  ```toml
  [dependencies]
  b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.49.0" }
  ```

- Outside the promise:
  - items not on this page, even where `loom_sdk` reaches them through a module;
  - `#[doc(hidden)]` items;
  - generated model types (`loom_sdk::commission::model`, `loom_sdk::loom::model`,
    `loom_sdk::session::model`, `loom_sdk::governor::model`). They change with their ESS declaration under `ess/`, and a
    release names such a change as a break when it makes one.

- Deprecation window: a path this page lists is deprecated for at least one release before it is
  removed. The release that deprecates it names it in a bullet under `### Deprecated` of its
  `CHANGELOG.md` entry; a later release may then remove it.
- Tag rule: a release whose entry carries a `**Breaking:**` bullet moves the minor version before
  1.0 (`0.35.0` to `0.36.0`); a tag that moves only the patch version (`0.35.1`) breaks nothing.

`loom-xtask release-check` holds both rules on every release tag
([`crates/loom-xtask/tests/release_check.rs`](https://github.com/beyond10x/loom/blob/main/crates/loom-xtask/tests/release_check.rs)):
it refuses an entry that removes a path the previous tag's version of this page lists, when no
earlier release deprecated it; and it refuses a `**Breaking:**` bullet on a tag with the same
major and minor version as the previous tag. There is no 1.0 promise.

- A listed path is a backticked `loom_sdk::` path in a table row of this page, outside a code
  fence, in a row not marked *generated*. Environment variables and other spans are not paths.
- A removal is any bullet under `### Removed`, and any `**Breaking:**` bullet that says
  `removed`, `deleted`, `renamed` or `moves to`. It removes every backticked item before its
  first `;`, wherever the verb stands. A rename or move removes only the old name, the first item
  before the verb.
- A deprecation is a bullet under `### Deprecated`. It deprecates every `loom_sdk::` path before
  its first `;`.
- An item covers the path it names and every path in its module: removing or deprecating
  `loom_sdk::ports` covers `loom_sdk::ports::Gate`.

## The Canon release a Loom release links

A Loom release links exactly one Canon release, `b10x-canon` (`Cargo.lock` holds one), and the
engineering protocols built on it, `b10x-canon-engineering`. The governor's decision for a case is
the same JSON document that Canon release renders for the same case
(`crates/loom-governor/tests/canon_decision_bytes.rs`).

| Loom | `b10x-canon` | `b10x-canon-engineering` |
|---|---|---|
| 0.49.0 | 0.7.0 | 0.13.0 |

A release that moves either pin names it in its `CHANGELOG.md` entry.

## Ports a host implements

Each port is a trait the host implements and hands to the runtime. The path is the one the test
implements it through.

| Port | Path | What the host supplies |
|---|---|---|
| `AuthorityProvider` | `loom_sdk::commission::ports::authority::AuthorityProvider` | Whether a commission holds a capability for the action request the runtime revalidated (its action, arguments and case revision), from organizational authority |
| `SuspensionPort` | `loom_sdk::commission::ports::suspension::SuspensionPort` | Durable storage of a run suspended on a withheld approval |
| `FallibleCaseStore` | `loom_sdk::governor::FallibleCaseStore` | A case store whose reads and writes can fail |
| `CaseStore` | `loom_sdk::governor::CaseStore` | A case store that cannot fail |
| `EvaluationTime` | `loom_sdk::governor::EvaluationTime` | The trusted time a case is evaluated at |
| `EffectPort` | `loom_sdk::commission::ports::effect::EffectPort` | Performing an admitted action |
| `ConnectorInvoker` | `loom_sdk::commission::ports::connector::ConnectorInvoker` | Invoking the connector operation an action is bound to |
| `CredentialResolver` | `loom_sdk::connectors::CredentialResolver` | The service credential a Connectors endpoint names, which `ConnectorsInvoker` asks for at each invocation; the invoker reads and keeps none |
| `ObservationPort` | `loom_sdk::commission::ports::evidence::ObservationPort` | Receiving the observations a run reports |
| `EvidencePort` | `loom_sdk::commission::ports::evidence::EvidencePort` | Receiving evidence attributed to its producer |
| `AgentExecutor` | `loom_sdk::commission::ports::executor::AgentExecutor` | What an executor proposes for one frontier of its commission (`run`); the runtime calls the provided `run_with_usage`, which returns `run`'s outcome with no usage unless the host overrides it to report what the call spent |
| `Governor` | `loom_sdk::commission::ports::governor::Governor` | A case's current revision, the frontier issued for it and whether the case is complete, which only the governor determines; `loom_sdk::CanonGovernor` implements it over Canon |
| `ActionSelector` | `loom_sdk::loom::selection::ActionSelector` | Picking one action from the catalogue |
| `WorkflowRunStore` | `loom_sdk::loom::workflow::WorkflowRunStore` | Durable storage of a workflow run's record, so the run resumes after a restart |
| `ArgumentGenerator` | `loom_sdk::loom::arguments::ArgumentGenerator` | Arguments for the selected action only |
| `Briefing` | `loom_sdk::loom::model_selector::Briefing` | What `ModelSelector` and `ModelArguments` tell the model: the text every request carries, the standing instructions of a selection request and of an argument request, and each action's argument schema, a `loom_sdk::serde_json::Value`; the provided `send`, `select` and `arguments` make one request each, and a host overrides them to ask otherwise |
| `LoopContext` | `loom_sdk::commission::runtime::LoopContext` | Ids, the time and the step budget, which no model supplies, and the event sink a run reports to (`events`, `NoEvents` by default) |
| `Ids` | `loom_sdk::commission::runtime::Ids` | The new ids a run needs (`action_request_id`, `observation_id`), a part of a `LoopContext`: a type implementing `Ids`, `Clock`, `Limits` and `HostControl` is one, each method answering as its part does |
| `Clock` | `loom_sdk::commission::runtime::Clock` | The trusted time a run reads (`now`), a part of a `LoopContext` |
| `Limits` | `loom_sdk::commission::runtime::Limits` | The step budget, run budget and stop policy the host chose, a part of a `LoopContext` a host may keep per agent; `run_budget` and `stop_policy` default as on `LoopContext` |
| `HostControl` | `loom_sdk::commission::runtime::HostControl` | The host's own stop (`host_stop`) and the event sink (`events`), a part of a `LoopContext` a host may keep per process; both default as on `LoopContext` |
| `EventSink` | `loom_sdk::commission::ports::events::EventSink` | Receiving every `RuntimeEvent` of a run as it happens: step started, action selected, proposed, authority decided, effect invoked, session closed and stopped; an event is never evidence and never changes the loop |
| `TestRunner` | `loom_sdk::intake::slice::confinement::TestRunner` | Running the slice's test command in a workspace and reporting the confinement it observed applied, not the one it was asked for; the slice's `LocalExecutor` takes one through `with_runner`, and `loom_sdk::SubstrateRunner` is Loom's |

`LoopContext`, its four parts and `EffectPort` are also at the top (`loom_sdk::LoopContext`,
`loom_sdk::Ids`, `loom_sdk::Clock`, `loom_sdk::Limits`, `loom_sdk::HostControl`,
`loom_sdk::EffectPort`), as are `ActionSelector`, `ArgumentGenerator`, `CaseStore` and
`TestRunner`.

## Types and functions an embedding calls

| Item | What it is |
|---|---|
| `loom_sdk::run_until_blocked` | Drives a commission until it is blocked |
| `loom_sdk::LoopEnd` | What `run_until_blocked` returns |
| `loom_sdk::Runtime` | A run's governor, executor, authority provider and effect port, held by reference, and its run store and loop context, owned (`new`, `runs`, `runs_mut`, `context`, `context_mut`, `into_parts`); `run`, `run_suspending` and `resume` call `run_until_blocked`, `run_with_suspension` and `resume_after_grant` with them and return what those return, which stay public and unchanged |
| `loom_sdk::EventSink`, `loom_sdk::NoEvents` | The event port at the top, and the sink that receives nothing, `LoopContext::events`' default |
| `loom_sdk::Loom` | The executor: proposes one action per frontier |
| `loom_sdk::CanonGovernor` | The governor over a case store |
| `loom_sdk::CompletionRule` | How the governor completes a case when several declared outcomes are legitimate at once (`CanonGovernor::with_completion_rule`): `ExactlyOne`, the default, or `FirstDeclared`, generated from `loom.governor` under `ess/` |
| `loom_sdk::MemoryCaseStore` | An in-memory `CaseStore` |
| `loom_sdk::CaseInputs` | The case inputs a host gives when it opens a case |
| `loom_sdk::CaseIndependence` | The independence dimensions a host declares for each evidence producer when it opens a case |
| `loom_sdk::ProtocolCatalog` | The protocol catalog shared by routing, case opening and admission |
| `loom_sdk::models::from_config` | Loads a host's models from a models configuration (YAML): the model of each declared role, over the `llm.routes/1` document it names, relative to the configuration's directory; every endpoint of the routes document must resolve, including ones no role uses ([llm issue 36](https://github.com/beyond10x/llm/issues/36)) |
| `loom_sdk::models::from_routes` | The llm `Model` one name of an `llm.routes/1` document serves: a route, or `<endpoint>/<model-id>` of a declared endpoint; every endpoint of the document must resolve, including ones no role uses ([llm issue 36](https://github.com/beyond10x/llm/issues/36)) |
| `loom_sdk::models::parse_config` | Parses a models configuration into a `ModelsConfig`, the parser `from_config` uses |
| `loom_sdk::models::Models` | What `from_config` returns: the model of each declared role (`get`, and `name_of` for the name that serves it), any other name of the same routes document (`model`), and `check_credential`, which resolves the `env:` or `file:` secret of the endpoint a name reaches and drops the value, so a host can refuse a missing secret before any model call |
| `loom_sdk::models::ModelsError` | Why models could not be loaded; its message names the file and the role or name |
| `loom_sdk::models::ModelsConfig`, `loom_sdk::models::ModelRole`, `loom_sdk::models::RoleModel` | The models configuration, generated from `loom.models` under `ess/`; it changes with that declaration |
| `loom_sdk::agent::parse`, `loom_sdk::agent::AgentSpecError`, `loom_sdk::agent::MAX_FLOW_OPENERS` | Decodes one `loom.agent/1` document, YAML or JSON, into an `AgentSpec`; a member the domain does not declare, a required member missing, a value of the wrong type, empty text, an undeclared enum spelling, a YAML tag, a `bearer_env` that is no environment variable name, a port outside 1 to 65535, a deadline that is not an RFC 3339 instant, more than `MAX_FLOW_OPENERS` `[` or `{` or another `format` is an `AgentSpecError` naming the member; its `reason()` is the `RefusalReason` of a value rule, `Malformed` otherwise |
| `loom_sdk::agent::AgentSpec`, `loom_sdk::agent::ToolPolicy`, `loom_sdk::agent::DecisionMode` | An agent as data: its identity, models, harness session with its tool policy (tools allowed and denied, egress apertures, and the metaharness decision mode `frame`, `ask` or `observe`), loops, states, bindings, channel, writes, capabilities, exposure, runtime and deployment, generated from `loom.agent` under `ess/`; they change with that declaration |
| `loom_sdk::agent::Decimal`, `loom_sdk::agent::Timestamp`, `loom_sdk::agent::SessionKind`, `loom_sdk::agent::Intent`, `loom_sdk::agent::SourceName`, `loom_sdk::agent::AdapterAlias`, `loom_sdk::agent::ConnectionId`, `loom_sdk::agent::OperationId`, `loom_sdk::agent::ConnectorsCliConfig` | The types of other domains an `AgentSpec` holds, re-exported so a host builds one in code through `loom_sdk::agent` alone; they change with their declarations under `ess/` |
| `loom_sdk::TestRunner`, `loom_sdk::SubstrateRunner`, `loom_sdk::UnconfinedRunner`, `loom_sdk::TestExecution`, `loom_sdk::ConfinementError` | The slice's test runners; a `TestExecution` carries the child's two streams apart (`child_output`), each whole up to Substrate's output limit and flagged cut past it (the unconfined runner keeps an 8 KiB tail of each) |
| `loom_sdk::ChildCommand`, `loom_sdk::ReadOnlyMount`, `loom_sdk::ReadOnlyFile` | A child process the host names, run confined by `SubstrateRunner::run_child` with a test's guarantees: its program and arguments, never a shell line; the directory it runs in, writable only under its `target/` and the workspace directories it names with `with_writable` (`.git` only when named); exactly the environment the host names; and the profile and network it asks for, the workspace profile with no network unless it names others; with `with_egress`, called once per aperture and at most `MAX_EGRESS_APERTURES` (4) times, its network is the egress apertures it names and nothing else, each host reached by its name, and with `with_ca_bundle` it trusts the certificate bundle the host names there: the bytes read from the file once at launch, through an open with its last component not followed, handed to Substrate as a private read-only copy and bound where `SSL_CERT_FILE` names, the one variable it gets beside the named environment. The bundle is the trust store the child gets through `SSL_CERT_FILE`; beside it, the environment may not name the listed variables, so a host does not override by accident the bundle it named. That is no guarantee about what the child trusts: configuration under `HOME`, a program's own flags and variables not listed can still point a client elsewhere or turn verification off; what the child reaches is bounded by the aperture. The last three are Substrate's `ExecEnvironment`, `SandboxProfile` and `NetworkMode`, and the aperture Substrate's `EgressAperture`, at `loom_sdk::intake::slice::confinement`. A guarantee Substrate does not serve, an aperture declaration Substrate's daemon never makes (no host name or numeric host, no port, a pinned address other than the named one) or that names a numeric host (any spelling `inet_aton` reads as an IPv4 address: hexadecimal, octal or fewer than four labels) other than `127.0.0.1` spelled so, which the child reaches only by name, more than four apertures, one aperture name given twice, two apertures naming one host (ASCII case aside, a trailing `.` ignored), a second to fourth aperture naming a numeric host or `localhost`, a writable path that is absolute, climbs out with `..`, passes a symbolic link, is absent or no directory, holds a special file or a hard link from outside, or overlaps another, a certificate bundle named without an aperture, by a path that is not UTF-8 or not absolute, or that is absent, a symbolic link, no regular file, empty, larger than 1 MiB or unreadable, or beside an environment naming a variable that replaces or widens a TLS client's trust store (`SSL_CERT_FILE`, `SSL_CERT_DIR`, `CURL_CA_BUNDLE`, `CURL_CA_PATH`, `CURL_HOME`, `REQUESTS_CA_BUNDLE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`, `GIT_SSL_NO_VERIFY`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_PARAMETERS`, or a `GIT_CONFIG_KEY_<n>` naming Git's `http.sslCAInfo`, `http.sslCAPath` or `http.sslVerify`), or a program containing `=` or starting with `-`, is a `ConfinementError` and nothing runs; a result whose applied confinement differs from the request, a wider network or writable set included, or with no exit status that neither its timeout nor its aperture's byte ceiling explains, is refused after the child ran; otherwise the `TestExecution` it returns carries the applied confinement, listing each aperture it reached (`AppliedConfinement::network`, a list of `AppliedAperture` generated from `intake.confinement`, with the address each of several listens on, empty for no network and absent only for an unconfined run) and the writable paths. Launches through one `SubstrateRunner` and its clones share one driver per directory holding their workspaces, set of egress apertures (in order) and certificate bundle they name (its path and the bytes read), so launches in workspaces of one directory run side by side; a launch that needs a driver for another directory, aperture set or bundle while a launch of the process runs under the same cgroup root is refused `CapabilityUnserved`. With `with_stdin`, `run_child` writes the bytes to the child's standard input once and closes it, through a Substrate pipe session; more than `ChildCommand::MAX_INPUT_BYTES` (16 MiB) is refused before launch. With `with_read_only_root(host, mount)`, `with_read_only_file(host, mount)` and `at_host_path(host)` the child reads host directories and files read-only where named (`ReadOnlyMount`), and nothing else of the host beside Substrate's own mounts; a host path that is not UTF-8, absolute and canonical, is absent, reached through a symbolic link or of the wrong kind, a mount on, under or containing one Substrate owns (`/`, `/usr`, `/bin`, `/lib`, `/lib64`, `/proc`, `/dev`, `/tmp`, `/workspace`, `/runtime`, `/scratch`, `/etc/hosts`, the bundle path), two mounts equal or nested, more than 4 roots or 16 files, or a host path that is, holds or lies inside the workspace, holds the bundle the child names or reaches the driver's state is a `ConfinementError` naming the path and nothing runs; files on a driver that does not publish `exec.read-only-files` are refused `CapabilityUnserved`. The applied confinement names the roots and files (`AppliedConfinement::read_only_files`, `ReadOnlyFile` generated from `intake.confinement`), a record naming others refused. `sandbox_path` says where the child sees a host path: the workspace at `/workspace`, a root or file at its mount, else `None`. The child starts in `/workspace` unless `with_workdir` names another directory, as the child sees it, at or beneath `/workspace` or a read-only root's mount; one elsewhere (`/scratch` and `/runtime` included), relative, with a `.`, `..` or empty component, not UTF-8, longer than `MAX_WORKDIR_BYTES` (4096), at or beneath a read-only file, holding a control character, absent, no directory, reached through a symbolic link or not searchable is a `ConfinementError` naming it and nothing runs, a run bubblewrap could not enter it in is refused rather than reported as the program's exit, and a driver that does not publish `exec.workdir` refuses it `CapabilityUnserved`; the applied confinement names it (`AppliedConfinement::workdir`). Substrate offers writable roots; Loom requests none, and a record naming one, another working directory than the one requested, apertures other than those requested, a capsule or a single aperture's `listen` address is refused |
| `loom_sdk::ConfinementSpec`, `loom_sdk::ChildEnvironment`, `loom_sdk::BaselineVariable`, `loom_sdk::ChildProfile`, `loom_sdk::ChildNetwork`, `loom_sdk::ChildAperture`, `loom_sdk::WritableRoot` | A child's confinement as one value, applied by `ChildCommand::confined(&spec)`: each field goes through the `ChildCommand` builder of the same meaning (read-only roots and files, host paths, writable workspace directories, environment, profile, network, egress apertures, certificate bundle, timeout in milliseconds, standard input, working directory), so the launch checks it exactly as it checks those builder calls, with the same limits and refusals; a spec naming nothing leaves the command as it was. A non-empty `writable_roots` is refused `CapabilityUnserved` naming it, because Loom requests no writable root; a value the builder's type cannot carry (an aperture port outside 0 to 65535, a `pinned` that is no socket address, a negative `max_bytes` or `timeout_ms`, one variable set twice) is refused `ScopeInvalid` naming the field. The read-only roots and environment entries are `ToolchainRoot` and `EnvironmentEntry` at `loom_sdk::intake::slice::confinement`. All are generated from `intake.confinement` under `ess/intake/`; they change with that declaration |
| `loom_sdk::ConfinedChild`, `loom_sdk::ChildFrame`, `loom_sdk::ChildSignal`, `loom_sdk::ChildResult`, `loom_sdk::ChildError`, `loom_sdk::ChildOutput`, `loom_sdk::ChildExit`, `loom_sdk::ChildStream` | A child running confined and live, started by `SubstrateRunner::spawn_child` with every check `run_child` makes before launch, on a driver that publishes what a Substrate pipe session needs (else `CapabilityUnserved`): the host writes its standard input (`write_stdin`, `close_stdin`), reads its output frame by frame, each naming its stream (`read`), signals it (`signal`: an interrupt or termination it has not ended from 5 s later is followed by a kill) and waits for it (`wait`), which returns its exit, its two streams whole and apart, and the applied confinement, checked against the request as `run_child` checks it; Substrate ends a child at its timeout (`ChildExit::timed_out`). A child dropped before it ended is killed and released from the driver. Its methods block on a runtime the child owns, so they are called outside an async runtime. `ChildOutput`, `ChildExit` and `ChildStream` are generated from `intake.confinement` under `ess/intake/`; they change with that declaration |
| `loom_sdk::connectors::cli::ConnectorsCli` | The client over the operator's `connectors` command line: reads (`invoke_read`, `read`) and, for a request the runtime admitted, one governed write (`invoke_write`) with the path of the person's approval proof, invoked once and never retried |
| `loom_sdk::connectors::cli::idempotency_key` | The `--idempotency-key` a write of an admitted request carries, derived from its request id: the same request, the same key |
| `loom_sdk::connectors::cli::WriteError` | Why a governed write was not invoked: refused (no absolute proof path, an operation not described as a write, a describe Connectors refused) or failed |
| `loom_sdk::connectors::cli::datasource::WriteOperation`, `loom_sdk::connectors::cli::datasource::WriteApproval`, `loom_sdk::connectors::cli::datasource::WriteResult`, `loom_sdk::connectors::cli::datasource::MutationClassification`, `loom_sdk::connectors::cli::datasource::WriteRefusal` | The governed write's operation, approval proof path, result (Connectors' classification, `request_id` and audit record) and refusal, generated from `loom.datasource` under `ess/`; they change with that declaration |
| `loom_sdk::connectors::evidence::WriteEvidenceAdapter`, `loom_sdk::connectors::evidence::WriteEvidenceError` | The host evidence adapter: an applied write of its action becomes the protocol's evidence for the step (for `support-triage@1`, a `routing_record` for `ticket.route` naming its target, the `queue` or `owner` the admitted arguments name, in its facts as `target`) through `submit_evidence`; a write not applied records none, nor does a route whose target cannot be read or whose answer names another one (`WriteEvidenceError::Unreadable`) |
| `loom_sdk::session::SessionExecutor` | The session executor, an `AgentExecutor`: each call runs one harness, Claude Code or Codex session through metaharness in a scratch copy of the workspace's named files and returns the change as a proposed action whose arguments are `{files}` only, a removed file's content null; it never writes the workspace, which an effect changes once Commission admits the proposal. It mints a session id per call and records the session (`sessions`, `proposed`); built `continuing` a recorded session, the next session it records names that one, a predecessor of another commission refused before any session runs, and the request to the harness is unchanged; the session's summary and transcript digest travel on the `ExecutorStep`, never as evidence, and `run_with_usage` reports its tokens, time and cost; built `with_observer`, it hands the host each metaharness event and the session's copy while the session runs. A session that fails suspends with `ExternalAvailability` |
| `loom_sdk::session::Proposal`, `loom_sdk::session::ProposedFile` | A proposal as an effect reads it back from the admitted request's arguments (`Proposal::from_outcome`), and one file of it |
| `loom_sdk::session::Kind`, `loom_sdk::session::CredentialSource` | The harness a session runs, and where its credential comes from: metaharness's types |
| `loom_sdk::serde_json` | The `serde_json` crate Loom links, re-exported for the `Value` a `Briefing` returns as an action's argument schema |
| `loom_sdk::commission`, `loom_sdk::connectors`, `loom_sdk::loom`, `loom_sdk::session`, `loom_sdk::governor`, `loom_sdk::protocols`, `loom_sdk::intake::router`, `loom_sdk::intake::slice` | The re-exported crates, under these module names |

## Used by control-plane

control-plane embeds `loom_sdk` in `crates/control-plane-runtime`. These are the `loom_sdk` paths
it names, its imports and the items it names inline, that this release still exports.
Associated functions and enum variants are covered by their type. Rows marked *generated* are
generated model types or modules, outside the promise above.

control-plane also names `loom_sdk::loom::harness` paths, `loom_sdk::loom::session::SessionFile`
and the session types of `loom_sdk::loom::model::run`, which 0.30.0 listed here. Loom no longer
ships its port of Harness (Atlas ADR 0098), so they are removed, each named in a `**Breaking:**`
bullet of `CHANGELOG.md`; control-plane moves off them when it moves its pin past 0.30.0.

| Path | Kind |
|---|---|
| `loom_sdk::ActionSelector` | port |
| `loom_sdk::ArgumentGenerator` | port |
| `loom_sdk::CanonGovernor` | type |
| `loom_sdk::Loom` | type |
| `loom_sdk::LoopContext` | port |
| `loom_sdk::commission::model::behaviour::Generated` | generated |
| `loom_sdk::commission::model::json` | generated |
| `loom_sdk::commission::model::primitives::Timestamp` | generated |
| `loom_sdk::commission::model::primitives::Uuid` | generated |
| `loom_sdk::commission::model::responsibility` | generated |
| `loom_sdk::commission::outcome::RunStore` | type |
| `loom_sdk::commission::ports::authority::AuthorityProvider` | port |
| `loom_sdk::commission::ports::authority::AuthorityProviderError` | type |
| `loom_sdk::commission::ports::effect::AdmittedRequest` | type |
| `loom_sdk::commission::ports::effect::EffectError` | type |
| `loom_sdk::commission::ports::effect::EffectPort` | port |
| `loom_sdk::commission::ports::evidence::submit_evidence` | function |
| `loom_sdk::commission::ports::executor::AgentExecutor` | port |
| `loom_sdk::commission::ports::governor::Governor` | port |
| `loom_sdk::governor::CaseState` | type |
| `loom_sdk::governor::FallibleCaseStore` | port |
| `loom_sdk::loom::arguments::ArgumentContext` | type |
| `loom_sdk::loom::model::primitives::Uuid` | generated |
| `loom_sdk::loom::model::run::CatalogueEntry` | generated |
| `loom_sdk::loom::model::run::CommissionRunId` | generated |
| `loom_sdk::loom::model::run::SelectionStrategy` | generated |
| `loom_sdk::loom::selection::Choice` | type |
| `loom_sdk::loom::selection::SelectionContext` | type |
| `loom_sdk::loom::selection::SelectorError` | type |
| `loom_sdk::run_until_blocked` | function |

The [embedding guide](../guides/embed-the-runtime.md) walks through a whole embedding with the
ports it supplies.
