# Loom

Loom is the agent runtime: it runs governed agents, and `b10x-loom-sdk` is its library for building
on it. It lets a model act as an agent for one bounded run, and only through the actions a governed
frontier admits at that moment.

**Documentation: <https://beyond10x.github.io/loom/>**, starting at
[Getting started](https://beyond10x.github.io/loom/docs/getting-started).

Since `0.4.0`, Loom includes workspace-free system queries and an extensible protocol
catalog. Loom owns `system-query@1`; engineering definitions and installed custom definitions
remain separate sources. [System queries and custom protocols](https://beyond10x.github.io/loom/docs/guides/system-queries)
describes the clock tool, pinned installation and offline loading. `run --action-bindings <FILE>`
drives an installed protocol through commands the operator binds to its actions: each admitted
action starts its program without a shell, with its arguments as JSON on standard input, and the
program's JSON report becomes the action's result; a binding confers no authority, and the
commands run unconfined only with `--unconfined-bindings` or the file's own
`"unconfined_bindings": true`. Transient model failures get
at most three attempts on the same binding; reports count every attempt. Persistent overload
is reported as unavailable, and tool effects are never retried by this policy.

Each step of a run goes the same way. Loom turns the frontier into the catalogue the model sees,
selects one action, asks for that action's arguments and returns a `ProposedAction`. Commission's
runtime, which lives in this repository, rechecks the proposal against the frontier, the case
revision and the authority it needs, and only then hands it to an effect port. A selector can pick
the wrong admissible action. It cannot pick one the frontier does not contain.

## What it is not

Loom does not decide what is legitimate: protocols are evaluated by
[Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)), and
`software-change@1` comes from
[engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)). It holds no model credentials and
speaks no provider wire. Its decisions (routing an intent, selecting an action, generating its
arguments, a workflow's judgement step) are structured requests to a model through
[llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)), and agent
work in a harness such as Claude Code goes through
[Metaharness](https://beyond10x.github.io/metaharness/)
([GitHub](https://github.com/beyond10x/metaharness)) behind an executor the host supplies; Loom
runs no model loop of its own (Atlas ADR 0098). It grants no authority, and it never merges,
pushes or deploys. [Where Loom ends](https://beyond10x.github.io/loom/docs/concepts/where-loom-ends)
draws the whole boundary.

## Status

Version `0.42.0`, released from source at the tag `0.42.0`
([release](https://github.com/beyond10x/loom/releases/tag/0.42.0)). Nothing is on a registry: you
install from the tag or depend on it with `tag = "0.42.0"`. `b10x-loom run` completed a live run
against a hosted model on 2026-10-05
([record](docs/qualification/2026-10-05-b10x-loom-live-run.md)). The
[status page](https://beyond10x.github.io/loom/docs/status) marks every capability shipped,
decided or planned; [CHANGELOG.md](CHANGELOG.md) lists the changes.

Since `0.3.0` the slice keeps inspected file contents as immutable, run-local results. The model
sees bounded previews and can select ranges or reuse content in edits through verified references,
which are expanded before Commission admission;
[result references](docs/design/result-references.md) describes the contract and limits.

Since `0.5.0` an executor can report that the case moved while it worked
(`ExecutorOutcome::CaseMoved`), and Commission judges the run on the case's current frontier
instead of the one it left. Callers that match `ExecutorOutcome` exhaustively handle the new
variant.

Since `0.6.0` a run whose case moved to a revision whose frontier still admits an action ends
`RunOutcome::CaseMovedOn`, naming the Run's revision and the current one; start a new Run at the
current revision. Callers that match `RunOutcome` exhaustively handle the new variant. Loom
requires ESS 0.69.2.

Since `0.7.0` a process can supervise a run through `b10x-loom run --output jsonl`, decide a case
it keeps itself through `b10x-loom evaluate`, and drive a run with a model from an llm catalog
through `b10x-loom run --catalog`.

Since `0.8.0` `b10x-loom-connectors` (`loom_sdk::connectors`) performs a commission's bound
actions through a Connectors service at `v0.35.0`, answering a write with the Connector attempt it
produced.

Since `0.9.0` a read answers with the execution audit record Connectors completed for it: an
`ActionBinding` declares its operation's `effect` (`Read` or `Write`), and `EffectOutcomePerformed`
carries the read's `audit`. Hosts that build either record add the new field. `task conform`
passes only when ESS rates the run `passed`.

Since `0.10.0` a run store can report a failed write: `StartRun` and `SuspendRun` declare a
`storage-failed` outcome, and `run_until_blocked` reports it as `LoopFailure::RunStorage` before any
executor or effect call. `FileCaseStore` keeps a governor case across a process restart. Callers
that match `StartRunOutcome`, `SuspendRunOutcome` or `LoopFailure` exhaustively add an arm.

Since `0.11.0` Loom builds on engineering-protocols `0.3.0` and Canon `0.1.0` (`tag = "0.1.0"`;
a consumer that adds Canon itself pins the same tag). `incident.response/1` keeps the obligation
`investigate_cause` open after `emergency.leave` becomes admissible, until a cause analysis
identifies the cause.

Since `0.19.0` a suspension port can answer that the host already settled a hold: Commission's
`HoldSuspension` has the outcome `settled`, and the run then holds nothing and ends as a loop
without a port ends it. `b10x-loom resume` uses it for a merge the agent proposes again at a
`HEAD` its commission already handed off: the run stops `MergeHandedOff`, nothing is held, and
`approve` has nothing to approve. The state directory and the governor's file stores read only
regular files as records; a symbolic link is refused, never followed. Callers that match
`HoldSuspensionOutcome` or `LoopFailure` exhaustively add `Settled` and `NotResumed`, and the run
store given to `run_with_suspension` implements `ResumeRunBehavior`, as `RunStore` does.

Since `0.20.0` Loom builds on engineering-protocols `0.7.0` and Canon `0.4.0` (`tag = "0.4.0"`;
a consumer that adds Canon itself pins the same tag), and every `software-change@1` case opens
with the case inputs the protocol declares: `risk`, `change_kind` and
`affects_security_boundary`, given by the host and never by a model (`b10x-loom run --risk`,
`--change-kind`, `--affects-security-boundary`; `CaseInputs` for an embedding). The governor
refuses an undeclared input, an unlisted value or a missing one before anything is stored, and
`CanonGovernor::open` and `open_case` take the inputs as a third argument. The slice performs
`tests.write` and `implementation.edit` where it performed `repository.edit`, and never offers
`release.rollback`, which it cannot perform. An obligation the case's inputs do not select is not
open. A state directory 0.19.0 wrote a case into is not resumed: its case has no inputs.

Since `0.18.0` the command line keeps a run that waits on an approval and continues it after the
grant: `b10x-loom run --state <DIR>` keeps the case, the commission's Suspension (in a
`loom_governor::FileSuspensionStore`) and its resume context in a directory, `b10x-loom approve`
writes the grant, and `b10x-loom resume` performs the suspended request once and runs on. A
granted merge is handed off as a record; no Git ref moves. Without `--state`, `run` is unchanged.
Callers that match `SliceError` or `b10x_loom_cli::Command` exhaustively, or build `SliceRun` or
`RunArgs` as struct literals, add the new variants and fields.

Since `0.17.0` a Commission host can keep a Run waiting on an approval across a process restart:
given a suspension port, `run_with_suspension` holds the commission's Suspension on a withheld
approval and `resume_after_grant` performs the pending request once, after the grant, at the case
revision it was suspended at. Callers of `run_until_blocked` see no change; callers that match
`ResumeRunOutcome` or `LoopFailure` exhaustively add the new arms.

Since `0.16.0` a step performed under an authority grant passes Commission's approval gate:
`run_until_blocked` runs the executor again instead of ending the Run `AwaitingApproval`, so a
caller that matched `AwaitingApproval` after a granted effect now gets the Run's own end, and a
plugin turn is one Run. Loom requires ESS 0.69.2.

Since `0.15.0` a confidence fallback is recorded as two selections, the overruled fast one
naming its replacement, each with the strategy of the selector that made it, and every
selection carries a telemetry record (`loom.run.SelectionRecord`) for Metaharness; Loom
requires ESS 0.57.0.

Since `0.13.0` Loom hosts plugins (ADR `plugin-hooks`): `b10x-loom-plugin` runs an unattended
agent as hooks around a governed run on the read-only protocol `inbound-answer@1`, reading data
sources through the `connectors` command line, and `b10x-loom plugin run slack-handler` walks the
Slack channels a bot belongs to and records proposed answers to unanswered messages; nothing is
posted.

Since `0.12.0` the executor offers two more action selectors: a reasoning-model selector chooses
one of the given candidates, and `HybridSelector` takes a fast selector's choice at or above a
confidence threshold the host supplies, else the stronger selector's. The experimental
`b10x-loom-selector-laya` crate is a fast selector over a Laya endpoint.

## Run the command line

Since `0.4.0`, Loom supports opt-in `--context-policy bounded` and `--context-report PATH`.
It keeps typed working state and a recent event tail, with retrievable run-local history and a
64 KiB serialized request ceiling. The default remains `legacy`; recorded byte reductions do not
establish live quality or cost savings. See [working context](website/docs/concepts/working-context.md).

You need a Rust toolchain that builds edition 2024, and `git`.

```console
git clone --branch 0.42.0 https://github.com/beyond10x/loom.git
cd loom
cargo install --locked --path crates/loom-cli
b10x-loom --version
```

```text
b10x-loom 0.42.0
```

On a development checkout, `task install` rebuilds the checked-out source and replaces
`~/.local/bin/b10x-loom`. Keep `~/.local/bin` on your `PATH`. It does not fetch or switch branches.

`b10x-loom run` routes an intent to a protocol, opens a governed case and runs until the run is
blocked:

```console
b10x-loom run --workspace <git work tree> --test-cmd "cargo test" "make the failing test pass"
```

For a software change, it prints each step with its effect and evidence and ends on a line such as
`stopped: ApprovalRequired (repository.merge)`. Exit status 0 means a query completed or a software change stopped at that human gate,
3 another stop, 1 a failure, 2 an invalid command line. With `--output jsonl` standard output is
one JSON record per line instead, the last one the terminal record with the stop reason and that
exit status ([run events](https://beyond10x.github.io/loom/docs/reference/run-events)).
A software change opens with the case inputs `--risk <trivial|standard|elevated|critical>`
(default `standard`), `--change-kind <feature|bugfix|refactor>` (default `feature`) and
`--affects-security-boundary <true|false>` (default `false`); with the defaults an edit needs no
failing test first, and with `--change-kind bugfix` `implementation.edit` waits for a test that
failed on the current revision. `tests.write` writes only test paths, matched by the repeatable
`--test-path <GLOB>` (by default `tests/**`, `**/tests/**`, `test/**`, `**/test/**`,
`**/*_test.*`, `**/test_*.*`, `**/*.test.*` and `**/*.spec.*`); `implementation.edit` is not
held to them. A bugfix's regression test result comes only from `--regression-test-command
<CMD>`, a command with `{path}` in a word, run once per test path the run's `tests.write`
committed after each test run; without it no run is a regression result, so
`red_first.required` stays open.
`--model` and `--classifier-model` name Codex models (default `gpt-5.6-sol`). With
`--catalog <PATH>`, an [llm](https://beyond10x.github.io/llm/) catalog file, both name route
aliases of that catalog instead, such as a self-hosted endpoint whose account needs no
credential; an unknown alias or an unreadable catalog stops the run before any model call.
With `--config <PATH>`, a models configuration naming an `llm.routes/1` document and the model of the roles `agent` and `classifier`, each role runs on its route with the credential the document's `secret:` reference names; `--model` or `--classifier-model`, when given, overrides its role with another name of that document.
A role neither configured nor given, or a model that cannot take a forced tool call, stops the run before any model call; `--config` takes neither `--catalog` nor `--state`.
With `--state <DIR>` the run keeps its case, its commission's Suspension and a resume context in
that directory, so a run that stops for an approval leaves what a later resume continues from;
a run that ends without a Suspension removes its resume context, and a directory that cannot be
written stops the run before any model call. Without it nothing outlives the run.
`b10x-loom approve <COMMISSION> <CAPABILITY> --state <DIR>` then writes a grant for the request
that Suspension holds, performing nothing, and `b10x-loom resume <COMMISSION> --state <DIR>`
performs that request once and runs on. A granted `repository.merge` is a merge hand-off: a record
of the commission, the request, the workspace and its `HEAD` in the state directory, once per
`HEAD` of a commission; no Git ref moves, and merging happens outside Loom. A merge the agent
proposes again at a `HEAD` its commission already handed off holds no Suspension and leaves no
resume context, so there is nothing to approve. Each refusal of a
resume has its own exit status (10 to 22, listed by `b10x-loom resume --help`). Only a last line
`stopped: ApprovalRequired (<action>)` leaves something to approve: `..., nothing suspended`
(exit 4) and `stopped: MergeHandedOff (<request>, HEAD <sha>)` leave nothing.
With `--mandate <FILE>`, a host-trusted Mandate file (`loom.authority.MandateHost`: the verified
context, the approver, the approver policy, the relation, the capabilities and those needing an
approval), `approve` also records a Mandate approval of the request in the state directory's
approval log (`mandate-approvals.jsonl`), which the grant names, and `resume` performs the request
only when Mandate's `check_with_approval` clears and consumes that approval, keeping the
consumption in the log before anything is performed. A second resume with the same approval, an
expired one and a grant naming none are refused `NotGranted` (16) and perform nothing. The file's
approver is neither its verified subject nor its actor. Only Mandate clears a grant naming an
approval: `resume` without `--mandate` refuses it `NotGranted` (16) and performs nothing.

Since `0.2.0` tests run confined. A software-change run needs a Codex
login and Linux with bubblewrap and delegated cgroup v2 controllers. Tests default to Substrate:
no network, source and toolchain read-only, workspace writes only under `target/`, a 300-second
timeout, 8 GiB memory and 2,048 processes. Fetch Rust dependencies explicitly before starting;
Loom uses a private offline Cargo home and never fetches them. Without delegation it attempts
one user systemd scope, then stops `ConfinementUnavailable` (exit 3) if confinement is unavailable.
`--confinement none` explicitly opts out and is named in every test observation.
Loom's own git calls run none of the work tree's hooks, and
a run does not start on a work tree whose own git configuration names a program (a filter driver,
a credential helper, an SSH command and the like).
[Run an intent](https://beyond10x.github.io/loom/docs/guides/run-an-intent) explains every line of
the output; the [CLI reference](https://beyond10x.github.io/loom/docs/reference/cli) lists every
flag.

`b10x-loom plugin run slack-handler --config <file> --state <dir>` hosts the slack-handler plugin:
it walks the Slack channels the bot is a member of through the Connectors command line, read only,
and for each message nobody answered records a proposed reply read from the configured data
sources, or a proposed case for a task. Nothing is sent. `--once` runs one cycle and prints one
line per proposal; `b10x-loom plugin report slack-handler --state <dir>` prints them again. The
configuration is `loom.slack.SlackConfig` as JSON; `plugin run --help` names its members, and
`--catalog` takes route aliases as `run` does.

`b10x-loom agent check --spec <FILE>` says whether a `loom.agent/1` spec can be served, against
the plugin catalog (`inbound-answer@1`, `operator-reply@1`), and runs nothing. The first refusal
is the answer: the text is not one YAML document; a value or key anywhere in it holds a credential
(a URL with a user name or a query, or a token prefix such as `xoxb-`), named without repeating
it, before the document is decoded; the document does not decode; the inner loop's
protocol is not in the catalog, or states have no inner loop; a state names an action its
protocol lacks; a state, the outer loop or the channel names a source no binding declares; an
`http` binding's method is not `GET`; `reply.send` is a capability while writes are `none`; the
channel allows no channel id. It exits 0 when the spec is serveable, 3 when it is refused, 1 when
the file cannot be read:

```console
$ b10x-loom agent check --spec examples/agents/operator.yaml
checked platform-operator: 3 states, 3 bindings, protocol operator-reply@1
$ b10x-loom agent check --spec ess/scenarios/agent-check/specs/unknown-action.yaml
b10x-loom: refused: UnknownAction at states[0].actions[2]: reply.escalate
```

`b10x-loom agent serve --spec <FILE> --state <DIR> [--once]` hosts the agent a checked spec
declares: the plugin host over the spec, polling only the channels `channel.allow` lists through
its `connectors` channel binding, classifying each item and answering it in a governed turn on the
inner loop's protocol under the state whose `when` names the intent, with exactly
`policy.capabilities` as its authority. `reply.send` is a governed write of the recorded proposal
to the item's channel, at most `writes.max_per_hour` an hour. In `writes.mode: approval` a send is
held: the turn ends proposed naming its commission, `b10x-loom approve <COMMISSION> reply.send
--state <DIR>` grants it, and the host performs it at its next poll, or `b10x-loom agent resume
<COMMISSION> --spec <FILE> --state <DIR>` performs it now, once. `writes.mode: none` sends nothing
and `auto` is not served yet. The models are the spec's `model` section, or `--catalog` route
aliases as `plugin run` takes them. SIGTERM ends the host after the item it is handling. An
`http` binding of the spec is read by `source.read` with `kind: get` and `input: {path, query?}`:
one GET of a path exactly in its `allow_paths` under its `base_url`, carrying the bearer the
variable `bearer_env` names, following no redirect, the body cut at `max_bytes` and marked; a path
outside the list (or holding `\`, a `.` or `..` segment or an encoded separator, which
`agent check` also refuses in the list), a method other than GET, a bearer variable unset, empty
or blank, a redirect or a failing status is a refused read the model reads as data.

`b10x-loom agent stage-files --from <DIR> --to <DIR>` copies each key of a mounted Kubernetes
Secret volume into a directory the caller owns (mode 0700, each file 0600, a file an earlier run
left replaced, never followed, a file no current key names removed). The controller runs it in
an init container for `spec.secretFiles`, so the files are those of the Secret when the pod
started; a changed Secret reaches the agent in a new pod. Exit 1 names the path it could not read
or write.

## Deploy agents to Kubernetes

A `LoomAgent` resource carries a `loom.agent/1` document as its `spec.agent`, and the controller
`b10x-loom-operator` (`crates/loom-operator`) runs each resource as one pod. The Helm chart
`charts/loom-agents`, versioned with Loom, installs the CustomResourceDefinition, the controller,
its ServiceAccount and a Role limited to its namespace, which grants nothing on Secrets. The values
name the controller image (`image`), the agent image a resource gets when it names none
(`agentImage`), the model route, an in-cluster proxy given by `modelRoute.url`, `.protocol` and
`.model` that holds no credential, and the Secret names and egress hosts a resource may use
(`allowedSecretNames`, `allowedEgressHosts`; an empty list allows none). `helm install` takes the
release name, chart, namespace and `--set` values shown here; `--include-crds` is only for
`helm template`, since `helm install` installs `crds/` itself:

```console
$ helm template loom-agents charts/loom-agents --namespace agents --include-crds \
    --set modelRoute.url=http://model-route.agents.svc:8080/v1 \
    --set modelRoute.protocol=messages --set modelRoute.model=small-model | grep '^kind:'
kind: CustomResourceDefinition
kind: ServiceAccount
kind: ConfigMap
kind: Role
kind: RoleBinding
kind: Deployment
```

Helm installs the chart's `crds/` once and never upgrades it; `b10x-loom-operator crd` prints the
CRD of the installed version. For each resource the controller first runs the check of
`b10x-loom agent check` over `spec.agent`, looks for a credential in `spec.routes` and
`spec.connectorsConfig`, and holds the Secret, the namespace and the egress hosts to its
configuration. A refusal is the resource's status (`state: Refused`, `check` naming the member)
and no child is created. Otherwise it applies a ServiceAccount, a ConfigMap, a Deployment that
checks the spec again in an init container and runs `b10x-loom agent serve`, a NetworkPolicy and,
with `expose.a2a`, a Service, all owned by the resource, and publishes `state`
(Pending, Running, Degraded or Refused), `check`, `deployment`, `variant` and the conditions
`Checked` and `Available`.

A resource with `spec.command` is the `Command` variant: its pod runs the image's own program
(`spec.command.entrypoint` and `args`, each left to the image when absent) instead of
`b10x-loom agent serve`, with no check before it, and reads `spec.agent` at `/config/agent.yaml`.
That is how a takt delivery runs as a `LoomAgent`. `spec.workspace` (`mountPath`, optional
`claimName`) adds a read-write volume, a PersistentVolumeClaim the values allow in
`allowedClaimNames` or else an emptyDir. On either variant `spec.secretRef` gives one Secret as
the environment and `spec.secretFiles` (`name`, `mountPath`) another as files: an init container
runs `b10x-loom agent stage-files` to copy its keys into a directory the pod user owns (mode 0700,
files 0600), since a tool such as `b10x-gates` refuses a key file another user owns or others may
read. The values must allow both names in `allowedSecretNames`. For this variant the controller checks `spec.agent` only for a
credential and against the `loom.agent/1` decoder, not the plugin catalog, and walks
`spec.command` for a credential too.

`b10x-loom-operator render` prints what the controller would apply for one resource and talks to no
cluster. Install it with `cargo install --locked --path crates/loom-operator`. Here the example
spec is wrapped as a resource that asks for the default routes (`routes: {}`), which needs a model
route, so the built-in configuration refuses it (exit 3) and `operator.yaml`, written in the
chart's `config.yaml` format, admits it:

```console
$ (printf 'apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata: {name: platform-operator, namespace: agents}\nspec:\n  routes: {}\n  agent:\n'; sed 's/^/    /' examples/agents/operator.yaml) > platform-operator.yaml
$ b10x-loom-operator render --resource platform-operator.yaml
b10x-loom-operator: refused: NoModelRoute at spec.routes: it is empty and the controller configures no model route
$ cat operator.yaml
namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.42.0'}
defaultResources: {cpu: 250m, memory: 256Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: []
allowedEgressHosts: [chat.example, models.example]
$ b10x-loom-operator render --resource platform-operator.yaml --config operator.yaml | grep -E '^kind:|^  name:'
kind: ServiceAccount
  name: platform-operator
kind: ConfigMap
  name: platform-operator-spec
kind: Deployment
  name: platform-operator
kind: NetworkPolicy
  name: platform-operator
kind: Service
  name: platform-operator
```

The [operator CLI reference](https://beyond10x.github.io/loom/docs/reference/operator-cli) lists
every flag and exit status, and
[the deployment design](docs/design/loom-agent-crd-deployment.md) draws the pod.

## Embed the runtime

An application depends on one crate, `b10x-loom-sdk` (library `loom_sdk`). It re-exports
Commission's contracts and runtime, the executor, the governor and intake. The application supplies
the selector, the argument generator, the authority provider and the effect port. For actions
bound to Connector operations, the effect port is Commission's `ConnectorEffects` over
`loom_sdk::connectors::ConnectorsInvoker`, which invokes each admitted request once on the host's
Connectors endpoint for the operation's instance and reports the attempt Connectors recorded.
`loom_sdk::Runtime` holds those parts with the governor, a run store and the loop context, and its
`run`, `run_suspending` and `resume` call `run_until_blocked`, `run_with_suspension` and
`resume_after_grant` with them; the free functions stay public.
A host that wants to watch a run returns its own `loom_sdk::EventSink` from
`LoopContext::events` and receives every step of it as it happens (step started, action selected,
proposed, authority decided, effect invoked, session closed, stopped); the default, `NoEvents`,
receives nothing. An event is never evidence and never changes the run.

An agent can be written as data: a `loom.agent/1` document, YAML or JSON, which
`loom_sdk::agent::parse` decodes into an `AgentSpec` (identity, models, session and tool policy,
loops, states, bindings, channel, writes, capabilities, exposure, runtime and deployment),
refusing a member the domain does not declare. `examples/agents/operator.yaml` is one.
`b10x-loom agent check` (library `b10x-loom-agent`) says whether such a spec can be served, and
`b10x_loom_agent::SpecPlugin` is the plugin `b10x-loom agent serve` hosts over it: `SpecAuthority`
grants exactly `policy.capabilities`, and `admit_send` decides each `reply.send` as
`loom.agent.AdmitSend` declares.

To deploy such an agent to Kubernetes, the `LoomAgent` custom resource carries the document as its
`spec.agent`. Its CustomResourceDefinition, `deploy/crd/loomagents.loom.beyond10x.dev.yaml`, is
generated: its schema is projected from the specification (`ess/domains/resource.yaml` and the
`loom.agent` domain) and `task crd-drift` fails when it falls behind. The controller that
reconciles the resource is `b10x-loom-operator` (§ Deploy agents to Kubernetes).

Since `0.2.0`, `CanonGovernor::with_protocol` admits host-reviewed Canon protocols;
`with_evaluation_time` supplies trusted freshness time. Durable hosts implement
`governor::FallibleCaseStore`, while existing `CaseStore` users remain compatible. Protocol
admission, durable storage and authenticated evidence remain the embedding application's duties.
A supervisor that keeps its own case record calls `loom_governor::evaluate` (or runs
`b10x-loom evaluate`, JSON in and out) with a catalog protocol, a `canon-case/1` snapshot and its
`canon-evidence/1` records (and any `canon-decisions/1` explicit decisions taken for the case), and
gets Canon's decision without Commission's types; `CanonGovernor::decide` records such a decision
on a held case ([the governor](https://beyond10x.github.io/loom/docs/concepts/governor-and-intake)).

```toml
[dependencies]
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.42.0" }
```

The example below
is a whole embedding: it opens a case on `software-change@1` over a scratch git repository, drives
it with scripted fake models (no network, no login) and stops before the merge. The commit id in
its output changes on every run.

```console
cargo run --locked -p b10x-loom-sdk --example software_change
```

```text
workspace: target/loom-sdk-example
step 1: implementation.edit {"files":[{"path":"check.txt","contents":"fixed\n"}],"message":"fix the check"}
  effect: committed; HEAD is 6c1b876de066f060c9d564596f480534df880904
  evidence: implementation_change
step 2: tests.run {}
  effect: the test command exited with 0
    | confinement: none
  evidence: test_result pass
stopped: ApprovalRequired (repository.merge)
```

A second example loops: a scripted agent rewrites `fizzbuzz.sh` until the slice's verifier
records a passing check, which is the only thing that puts `repository.merge` in its catalogue.
Its early merge is refused at selection, its merge after the passing check stops for approval, and
the same agent under a step budget of 4 is suspended for its budget.

```console
cargo run --locked -p b10x-loom-sdk --example governed_loop
```

A workflow whose steps are known needs no model for them. `loom_sdk::loom::workflow::load` reads a
YAML definition over a catalog protocol, checked against `ProtocolEntry::actions`, and
`loom_sdk::WorkflowExecutor` runs it as the executor: a deterministic step proposes its action
with its literal arguments and asks no model, a judgement step asks the model only among the
actions it names, and the workflow advances past a step only once the effect port, wrapped in
`loom_sdk::RecordingEffects`, performed it. Commission still revalidates every proposal.
A step may declare typed `outputs` from its performed effect's report (a name, a JSON pointer and
a type), and a later deterministic step's arguments name one as `{$step: <step>, $output:
<name>}`: the runtime fills it in with no model call, and an output absent from the report or of
another type ends the run `NeedsHumanJudgment` naming the step and the output, with nothing
proposed. A judgement step lists the earlier outputs it is given as `inputs` (`{step: <step>,
output: <name>}`): their values go into its instructions as one data block, typed, never as
instructions, and a listed output the run did not produce asks the model nothing.
Given a run store (`.with_store(store)`, the governor's `FileWorkflowRunStore` beside its
`FileSuspensionStore`, or the host's own `WorkflowRunStore`) and bound to a Run with
`begin(&run_id)`, the executor writes the run's record after every performed step and every call:
the steps performed and proposed, the outputs and the usage so far. `resume(&run_id)` in another
process continues it, after an approval (`resume_after_grant`) or a crash, at the step after the
last performed one with every output bound, and the meter's report covers every segment; a record
of another Run or another definition is refused. A request proposed and not recorded performed is
proposed again after a restart, so an effect must tolerate one repeat.

```yaml
name: change
protocol: software-change@1
steps:
  - {name: inspect, kind: Deterministic, action: repository.inspect, arguments: {}}
  - name: edit
    kind: Judgement
    actions: [implementation.edit]
    instructions: Edit the implementation so the failing test passes.
  - {name: test, kind: Deterministic, action: tests.run, arguments: {}}
```

[Embed the runtime](https://beyond10x.github.io/loom/docs/guides/embed-the-runtime) walks through
its code. [SDK stability](https://beyond10x.github.io/loom/docs/reference/sdk-stability) names the
surface kept stable within a release line, every port a host implements among it.

## Crates

| Package | What it is |
|---|---|
| `b10x-loom-protocols` | The shared protocol catalog and verified offline installations |
| `b10x-loom-cli` | The `b10x-loom` command line |
| `b10x-loom-agent` | Checks a `loom.agent/1` spec before anything runs: the first reason it cannot be served, naming the member; `b10x-loom agent check` runs it. `SpecPlugin` serves a checked spec on the plugin host, holding each `reply.send` for approval and reading its `http` bindings by bounded GET (`HttpGet`); `b10x-loom agent serve` hosts it |
| `b10x-loom-a2a` | The A2A listener of a served agent (`expose.a2a`): its agent card, one governed turn per `SendMessage` answered with the turn's record, `GetTask` following the turn's runtime events, readiness and liveness probes, and a refusal of every other request without the agent's bearer; `b10x-loom agent serve` opens it |
| `b10x-loom-sdk` | The one crate an embedding depends on |
| `b10x-loom-executor` | The executor: catalogue, action selection and argument generation, each model request one turn of an llm `Model` |
| `b10x-loom-session-executor` | An `AgentExecutor` over a harness, Claude Code or Codex session run through metaharness (`0.15.0`) in a scratch copy: it proposes the session's change, never applies it, and summarizes the session's record as `trace-ir-summary/1`, with each tool call's decision and who made it; `SessionExecutor::from_spec` builds it from a `loom.agent/1` spec, offering only the spec's allowed tools; a b10x-harness session can read its endpoint's API key from the secret store by name (`api_key_secret`) |
| `b10x-loom-commission` | Commission's contracts and runtime loop, generated from its ESS specification |
| `b10x-loom-connectors` | Commission's `ConnectorInvoker` over a Connectors service (Connectors `v0.35.0`) |
| `b10x-loom-plugin` | The plugin host: polls a plugin, classifies each item and answers it in a governed turn over Connectors data sources, one record line per item; a turn given a `SendPolicy` may send the reply it proposed, once admitted |
| `b10x-loom-plugin-slack` | The slack-handler plugin: walks the Slack channels the bot is a member of, read only, and proposes answers to the messages nobody answered; `b10x-loom plugin run slack-handler` hosts it |
| `b10x-loom-resource` | The `LoomAgent` Kubernetes resource: its lifecycle record over the generated `loom.resource` behaviour, and the CRD projected from the specification; talks to no cluster |
| `b10x-loom-operator` | The `b10x-loom-operator` controller: reconciles a `LoomAgent` into a pod and its children, and renders them; installed by `charts/loom-agents` |
| `b10x-loom-governor` | Evaluates a case's protocol with Canon and issues the frontier; executes nothing |
| `b10x-loom-mandate` | Opt-in: Commission's authority port answered by Mandate's `check` (Mandate `v0.9.0`), with the host's Mandate ports; a Mandate failure grants nothing. `b10x-loom approve` and `resume` use it behind `--mandate`; the SDK does not link it |
| `b10x-loom-selector-laya` | Experimental: a fast typed action selector over a local or hosted Laya endpoint (`POST /v1/systemone`); no product crate links it |
| `b10x-loom-intake-router`, `-references`, `-slice` | Route an intent to a protocol, extract its references, and run the local slice over a git work tree |

The [crate reference](https://beyond10x.github.io/loom/docs/reference/crates) lists all
workspace packages, test kits and repository tools included. If you depended on the archived
`commission`, `governor` or `intake` repositories,
[Move to Loom's crates](https://beyond10x.github.io/loom/docs/guides/move-to-loom) maps the old
names to the new ones.

## Contributing

Changes go through `task check`, which needs Rust, [Task](https://taskfile.dev/), bubblewrap
(`/usr/bin/bwrap`) and the `ess` command line of [ESS](https://beyond10x.github.io/ess/)
([GitHub](https://github.com/beyond10x/ess)). Run `cargo fetch --locked` first to prepare the
full dependency graph for offline checks.
[Run the checks](https://beyond10x.github.io/loom/docs/guides/run-the-checks) explains each step.
Agents read [AGENTS.md](AGENTS.md).

## Licence

Apache-2.0 ([LICENSE](LICENSE)).
