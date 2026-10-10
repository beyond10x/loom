# AGENTS.md — loom

This file is for an agent changing the repository. What Loom is, how to install it and how to embed
it are in [README.md](README.md) and on the site, <https://beyond10x.github.io/loom/>; neither is
repeated here. The architecture behind it is in Atlas (private): ADRs 0066–0076, 0080, 0082, 0089
and 0090 under `architecture/adr/`, and `docs/design/governed-autonomy/`. ADR 0090 made Loom the
one runtime repository, with Commission, the governor and intake moved in.

## Serves

- **O1**, governed reach.
- **O3**, any harness, observed and compared.

§ Boundary says what each asks of Loom.

## Boundary

Loom serves two outcomes of the governed-autonomy vision. **O1, governed reach:** the model sees
and can execute only what the current frontier admits, and authority is rechecked before every
effect. **O3, any harness, observed and compared:** Loom is the native executor Metaharness compares
against other harnesses on the same case.

Loom owns the model/tool loop (ADR 0071): prompt and context assembly, model turns, the projected
action catalogue, action selection, argument generation, tool round trips, streaming, compaction,
sessions and transcripts, budgets, interruption and recovery, and model-facing approval suspension.
Since ADR 0090 it also holds Commission's contracts and runtime, the governor and intake (sections
below).

It does not own protocol semantics (Canon), engineering protocol definitions
(`b10x-canon-engineering`, beyond10x/engineering-protocols), case truth, organizational authority,
connector credentials, final completion, provider wires or credentials (llm), or the engineering
record of a case (AEP).

`beyond10x/harness` is the predecessor. Port its implementation instead of rewriting from zero, and
do not break its consumers while they still depend on it. Code ported from Harness into Loom is
Apache-2.0 (operator, 2026-10-04); Harness keeps its own `LicenseRef-B10x-Proprietary` licence.

## Protocol composition and system queries

Loom owns `protocols/system-query/1.yaml`, a read-only local clock query. Canon still owns protocol
semantics, and engineering-protocols still owns engineering definitions. `b10x-loom-protocols`
composes bundled and explicitly installed definitions. Intake routing, artifact declarations and
host admission must use the same immutable catalog; a remote package never installs executable
code or authority. Protocol installation is explicit and runs read verified snapshots offline.

`run --workspace` is optional for system queries and required for software changes. Route before
protocol-specific setup: a clock query must not touch Git, a workspace or Substrate. Software
changes retain confinement and merge approval. Clock evidence comes from one trusted reading,
not model output; only Canon completion makes a query successful. The integration test
`crates/loom-intake-slice/tests/system_query.rs::clock_query_runs_in_both_policies_without_workspace_resources`
holds this path in both context modes. `protocols add/list/remove` manage user installations;
`--replace` is explicit and bundled definitions cannot be shadowed. The SDK exports the catalog.

Loom also owns `protocols/inbound-answer/1.yaml` (`inbound-answer@1`), the read-only protocol a
plugin turn answers an inbound item under: `source.read` reads (capability `datasource.read`),
`reply.propose` records a proposal and `reply.decline` a decline, and neither sends anything. It
is the one exception to a single catalog: a plugin host admits on `ProtocolCatalog::plugins()`,
which holds it, while routing, `protocols list` and a plugin's task path read `bundled()`, which
does not. A plugin protocol is never routed to, and a routed protocol is never run by a plugin
turn. `crates/loom-protocols/tests/inbound_answer.rs` holds both catalogs.

## Rules

- Model-visible actions derive from the current frontier and runtime capability. Nothing
  consequential is granted because it was registered at startup (ADR 0072).
- A selector picks only from the candidate set it was given. Unknown action ids are rejected,
  confidence never grants authority, and low confidence falls back to a stronger path (ADR 0073).
- Revalidate every selected action against case revision, frontier and authority before
  execution. A proposal carries no case revision of its own; the runtime reads the current one.
- A trace is not evidence (ADR 0074). Evidence comes from a trusted verifier, never from what a
  model says happened.
- The model is not trusted context: it never supplies identity, authority, case revision, trusted
  time, approval results or tenant context.
- When a selector, integration, verifier or authority provider fails, fail toward less authority,
  less effect and more explicit uncertainty. Never silently broaden capability.
- Do not make Loom semantically dependent on one selector vendor or model.
- Commission core never depends on Loom's executor, on Canon or on a model-provider crate
  (ADR 0075). Loom depends on Commission and implements its `AgentExecutor`.
- Anything that runs is Rust; command lines use clap derive.

Commission's code and tests cite this section (`AGENTS.md` § Rules). Keep the heading and the
phrases above; `adversary_agents_rules_carry_the_rules_commission_cites` fails without them.

## Checks that hold the rules

| Claim | Held by |
|---|---|
| The catalogue follows the frontier | `crates/loom-executor/tests/frontier_projection.rs` (`projection_follows_frontier`), `crates/loom-executor/tests/harness_loop_port.rs` (`ported_loop_round_trip`) |
| A selector cannot leave the catalogue | `crates/loom-executor/tests/action_selector.rs` (`selector_cannot_leave_catalogue`) |
| The reasoning-model selector offers the model only its candidates, through any `ModelPort`, and an answer outside them is never a selection | `crates/loom-executor/tests/reasoning_model_selector.rs` (`an_action_outside_the_candidate_set_is_a_selection_error_and_never_a_selection`) |
| Low confidence falls back to the stronger selector, at a host-supplied threshold compared as a number | `crates/loom-executor/tests/confidence_fallback.rs` (`the_fast_choice_is_returned_only_at_or_above_the_supplied_threshold`) |
| A confidence fallback is recorded as two selections, each with its own selector's strategy: the fast one `Overruled`, naming its replacement, which alone reaches argument generation and revalidation; a replacement that is unknown, the selection itself, no longer `Selected` or from another catalogue is refused and nothing is recorded; `Loom::select` returns the pick under its own selector's strategy | `crates/loom-executor/tests/fallback_selection_recording.rs` (`an_overruled_fast_selection_is_recorded_overruled_naming_its_replacement`, `overrule_selection_naming_an_unknown_replacement_is_refused_with_selection_not_found`); `crates/loom-executor/tests/adversary_w4_fallback_overrule.rs`; `crates/loom-executor/tests/adversary_w4_fallback_pipeline.rs` (`adversary_w4_loom_select_carries_the_strategy_of_the_selector_that_made_the_pick`) |
| Every selection gets one `SelectionRecord` equal to it, with `fell_back_to` on the overruled fast selection naming the replacement's strategy; a selection refused at the execution boundary raises its session's `boundary_refusals` by one and adds no record; neither is evidence | `crates/loom-executor/tests/selection_telemetry.rs` (`selection_telemetry_is_recorded`) |
| Laya selects behind the confidence fallback; arguments are generated once, only for the finally selected action, and an action outside the frontier never reaches argument generation or revalidation | `crates/loom-selector-laya/tests/laya_arguments_slice.rs` (`a_choice_outside_the_frontier_is_rejected_and_never_reaches_arguments_or_revalidation`) |
| A blocked or merge-seeking pick is never proposed | `crates/loom-executor/tests/adversary_executor_admission.rs` |
| A stale or unlisted request is refused at revalidation | `crates/loom-commission-testkit/tests/action_request.rs`, `crates/loom-executor/tests/adversary_run_revalidation.rs`, `crates/loom-executor/tests/selection_revalidation.rs` |
| Unknown capabilities and panicking authority providers yield no grant | `crates/loom-commission-testkit/tests/adversary_authority_fail_closed.rs` |
| Commission names no executor, Canon, `model-provider-deny.txt` or Connectors crate | `task commission:deps-guard` (`crates/loom-commission-testkit/tests/executor_port.rs`) |
| An executor's report that the case moved is decided by the governor and judged on the case's current frontier, reloaded once | `crates/loom-commission-testkit/tests/moved_case_outcome.rs`, `crates/loom-executor/tests/adversary_w1_runtime_stale.rs`, `crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs` |
| An executor is never handed an unperformed action that needs no authority; an effect is invoked once, through its binding | `crates/loom-commission-testkit/tests/effect_invocation.rs` (`effect_invoked_only_through_its_binding`) |
| A consequential action leaves Loom only as a proposal, and the executor links no Connectors crate | `crates/loom-executor/tests/connector_boundary.rs` (`consequential_actions_leave_loom_only_as_a_proposal`) |
| A Connector invocation is made once, and only Connectors' own "nothing changed" is a refusal | `crates/loom-connectors/tests/connectors_invoker.rs` |
| A plugin turn reads only the sources and kinds of its projection, and a proposal is recorded, never sent | `crates/loom-plugin/tests/effects.rs` (`effects_refuse_an_undeclared_read`, `propose_writes_the_record_only`) |
| A slack-handler run describes and invokes only the three configured Slack reads and the turns' data-source reads, and records one line per item it proposes for; nothing is sent | `crates/loom-plugin-slack/tests/run.rs` (`fixture_run_invokes_no_write`, `fixture_run_records_three_proposals`) |
| A read through Connectors is performed naming its audit record; a write without a recorded attempt is an error | `crates/loom-connectors/tests/connectors_invoker.rs` (`an_admitted_read_is_performed_with_its_result_and_names_its_audit_record`, `a_write_without_a_recorded_attempt_is_still_an_error`), `crates/loom-commission-testkit/tests/effect_invocation.rs` (`a_performed_that_names_another_record_than_its_binding_requires_is_not_answered`) |
| The executor answers the commands of `ess/` as their synthesized scenarios specify, except `not-in-frontier`, which the conformance target answers | `task conform` (`crates/loom-conformance/tests/conform.rs`, `ess_conformance_report`); the membership rule: `crates/loom-executor/tests/adversary_run_revalidation.rs` (`not_in_frontier_follows_the_frontier_actions`) |
| The router refuses a pick outside the registry or below the threshold | `crates/loom-intake-router/tests/adversary_classify.rs` |
| Model or provider JSON nested past 128 levels is refused | `crates/loom-executor/tests/json_depth.rs` |
| No hand-written type shadows an ESS-declared one | `task no-hand-model`, `task commission:no-hand-model` |
| Ported Harness modules keep the import limits their crates had; `turn_loop` alone may also name the generated model (`crate::model`), for `LoopStop` | `crates/loom-executor/tests/adversary_harness_port_boundaries.rs` (`the_boundaries_harness_enforced_by_crate_still_hold_between_modules`), `crates/loom-executor/tests/adversary2_harness_port.rs` (`each_ported_module_names_only_what_its_harness_manifest_allowed`) |
| Package and library names are the `loom-` names | `crates/loom-executor/tests/crate_names.rs` |
| No manifest names an archived Commission, governor or intake repository | `crates/loom-executor/tests/governor_import.rs`, `intake_import.rs`, `commission_import.rs` |
| A release tag equals the workspace version and has a CHANGELOG entry | `loom-xtask release-check`, run by `release.yml` (`crates/loom-xtask/tests/release_check.rs`) |

## ESS

Loom is specified in ESS under `ess/`. The domain is drafted and validated before any story
introduces a noun, and `task check` runs its conformance suite once synthesized. Change the
specification first.

The specification is a hard gate (Atlas ADR 0076). `task check` enforces it through
`task ess-gate`, which runs `crates/loom-executor/tests/ess_gate.rs`; a failure of any of the four
conditions fails `task check`:

1. `ess specify validate --path ess --strict-requires` exits 0;
2. `ess specify compile --path ess --format json` exits 0;
3. `ess verify conform synthesize` exits 0 with 0 refusals;
4. no file under `ess/` contains `UNMAPPED:`, so the specification carries no open question.

No story is implemented while the gate is red, whether or not it edits `ess/`. The `UNMAPPED:`
scan stands in for ESS until ESS refuses open entries itself: the `UNMAPPED:` scan is removed when
the ESS release that refuses open entries (beyond10x/ess `epic:typed-open-questions`) is pinned.

Commission's specification is its own ESS system under `ess/commission/`, held to the same four
conditions with `--path ess/commission` by `task commission:ess-gate`
(`crates/loom-commission/tests/ess_gate.rs`), which `task check` also runs. Intake's
`intake.routing` domain is `ess/intake/`, validated by `task intake-spec`. The governor's nouns are
Commission's and Canon's, except its stateless evaluation request, decision and refusal, which are
the `loom.evaluation` domain (`ess/domains/evaluation.yaml`), and its held case as a durable store
writes it, which is the `loom.governor` domain (`ess/domains/governor.yaml`); a noun it introduces
gets a declaration before a story is written around it.

An open question is settled before the specification changes, in a story or in a
`decision-blocker` when nobody has decided it, and is never written into `ess/` as an `UNMAPPED:`
marker.

Spec first, then red, then implement (Atlas ADR 0080). A unit's first commit changes only `ess/`;
on it a named test fails (a conformance scenario, `task drift`, or the story's own new test when
its declarations already landed), and the run is recorded; later commits make it pass without
changing `ess/`. Every story names that change and that test in its `## ESS first` section. Only a
change with no behaviour change is exempt, and its story says so.

`ess_gate.rs` (both copies) reads this section and fails when a phrase it checks is gone; reword
with the tests open. CI installs `ess` 0.57.0 (`.github/workflows/check.yml`); move that pin when a
newer ESS ships.

## Gate

Needs Rust, Task, `ess` and bubblewrap (`/usr/bin/bwrap`). CI installs the backend before
running the delegation-refusal tests. Run `cargo fetch --locked` before the gate, as CI does:
the offline dependency guard inspects the full graph, including platform-specific crates.
Before a change is reported done, in this order:

1. `task check`: the three specification validations, both ESS gates, all three drift checks, both
   no-hand-model checks, Loom's and Commission's conformance suites, the dependency guard,
   `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
   `cargo test --workspace --locked`, then `task docs-check` and `task commission:docs-drift`.
   CI runs exactly this (`check.yml`, job `loom / task check`).
2. `task plan` (`aep plan artifact validate`) when the planning store changed. `task check` does
   not run it.
3. `task website` when anything under `website/` changed, or a generated page did. CI's
   `Documentation validation` (`pages.yml`) builds the site and runs the `loom-docs` and
   `loom-commission-docs` tests.

There is no `rust-toolchain.toml`: CI builds on `stable` (`dtolnay/rust-toolchain`), and a local
toolchain newer than CI's `stable` can add Clippy lints CI does not have. Run the gate on CI's
version (`RUSTUP_TOOLCHAIN=<version> task check`) when Clippy disagrees.

`task install` builds and installs the current checkout into `~/.local/bin` through locked
`cargo install --path`; it does not fetch or move a branch. Run the gates before installing a
candidate for dogfooding.

`task --list` names every step, and each runs alone (`task drift`, `task conform`, `task commission:conform`, …).
`task commission:check` is Commission's own gate, limited to its crates. While iterating, prefer
`cargo test -p <package> --locked` on the crate you touched.

Every work tree builds into its own `target/`. The Taskfiles set no `CARGO_TARGET_DIR`; do not set
one. Two trees that share a build directory can run each other's test binaries: cargo judges the
other tree's binary fresh, and the gate exits 0 on code it did not build
(`gate_checks_the_tree_it_runs_in_when_worktrees_share_a_build_directory` in
`crates/loom-executor/tests/adversary_ess_gate.rs`). Before a gate's result counts as evidence,
check with `cargo test --workspace --locked -- --list` in the gated tree that the tests the run
printed exist there. Check `df -h /` before a full gate and run one full gate at a time. End a
managed worktree with `worktree finish --discard-cache --archive <tree>`, which removes its build
cache; never delete a `target/` by hand.

## Generated files

| Path | Generator | Regenerate | Drift check |
|---|---|---|---|
| `generated/rust/loom/` | `ess generate synthesize` over `ess/`, via `loom-xtask` | `task generate` | `task drift` |
| `generated/rust/commission/` | the same over `ess/commission/`, via `loom-commission-xtask` | `task commission:generate` | `task commission:drift` |
| `generated/rust/intake/` | synthesis over `ess/intake/`, via `loom-xtask` | `task intake-generate` | `task intake-drift` |
| `website/docs/reference/cli.md` | `loom-docs`, from `b10x_loom_cli::Cli` (`crates/loom-cli/src/lib.rs`) | `task docs-generate` | `task docs-check` |
| `website/docs/reference/crates.md` | `loom-docs`, from `cargo metadata` | `task docs-generate` | `task docs-check` |
| `website/docs/reference/ess/`, `website/data/ess/` | `loom-docs`, from `ess/` and `ess/intake/` | `task docs-generate` | `task docs-check` |
| `website/docs/reference/commission/` | `loom-commission-docs`, from `ess/commission/` | `task commission:docs` | `task commission:docs-drift` |

Never edit these by hand; change the source and regenerate. The generated Rust is never formatted:
`generated/rustfmt.toml` sets `disable_all_formatting`, so `cargo fmt --all` leaves it alone. Do not
add a `rustfmt.toml` inside a generated tree; `task drift` reports it and `task generate` deletes
it. A flag change in `b10x-loom-cli` regenerates the CLI reference. Every package needs a Cargo
`description`, and the crate page publishes it, so it names no story id or internal name.

`website/data/status.json` is maintained by hand. A change that ships, decides or drops a
capability updates it, the page that describes it and `CHANGELOG.md` (**Unreleased**) in the same
commit. A change to a command, a crate or a rule updates README.md and this file in that commit too.

## Commission

Commission's history was merged into Loom, and its files keep their Commission paths so
`git log --follow` reaches it. Packages: `b10x-loom-commission` (the contracts and runtime, with
`run_until_blocked`), `b10x-loom-commission-testkit` (port fakes and adapter kits),
`b10x-loom-commission-conformance` (holds the contracts crate to its synthesized suite),
`loom-commission-docs` and `loom-commission-xtask`. Design and contract notes are under
`docs/commission/`. Its tasks are in `Taskfile.commission.yml`, included under the `commission:`
namespace.

The frontier Loom reads is Commission's generated `Frontier`. Loom crates may depend on
`b10x-canon`; the Commission contracts crate may not.

## Governor

`b10x-loom-governor` (`crates/loom-governor`, library `loom_governor`) puts Canon behind the
governor and evidence ports Commission defines (ADR 0089). It decides and never acts: it evaluates
a case's protocol, reports the frontier and completion, and executes nothing. It is the only crate
that evaluates protocols with Canon, and it adds no clock, network or model call to an evaluation.
It depends on Commission by path, never the reverse.

Host-reviewed protocols enter through `CanonGovernor::with_protocol`; compile and frontier
representability checks remain in the governor. `EvaluationTime` is a trusted host callback,
never a model argument. Durable stores implement `FallibleCaseStore` atomically and report
failures explicitly; the legacy `CaseStore` bridge is only for infallible adapters.
`FileCaseStore` is one: a file per case, written as `loom.governor`'s `StoredCase` to a temporary
file and renamed, under a per-operation lock; `crates/loom-governor/tests/restart_durability.rs`
holds a case across a real process restart. The host
restores the same admitted protocol definitions on recovery and authenticates evidence before
`submit_evidence`. The governor supplies no authority decision.

`evaluate` (`loom_governor::evaluate`, `b10x-loom evaluate`) decides a caller's `canon-case/1`
snapshot and `canon-evidence/1` records under a protocol of the host catalog, never raw YAML from
the caller. Its request, decision and refusal are the `loom.evaluation` domain
(`ess/domains/evaluation.yaml`); its signature names no Commission type. It goes through the
private `decide_case` and `project` that `CanonGovernor` uses, never a second evaluator, and a
refusal names the input (protocol, snapshot, evidence record by position, time). A readable
record that does not apply is set aside as `CanonGovernor` sets it aside; an unreadable record or
a repeated id is refused. The time and the records are judged once each, never by re-running Canon
on prefixes. `crates/loom-governor/tests/evaluate.rs` and `evaluate_adversary.rs` hold it against
`CanonGovernor`; `crates/loom-cli/tests/evaluate.rs` holds the subcommand against the library.

Canon is named by the reference `b10x-canon-engineering` uses (`tag = "0.1.0"`), pinned by
`Cargo.lock`. A different reference builds a second Canon whose types do not match. Move Canon with
`cargo update -p b10x-canon -p b10x-canon-engineering` together with the `b10x-canon-engineering`
tag (now `0.3.0`); any other crate that adds Canon uses the same reference.

## Connectors

`b10x-loom-connectors` (`crates/loom-connectors`, `loom_sdk::connectors`) is the one crate that
links Connectors: `connectors-client` and `connectors-core` at tag `v0.35.0`. Move both together.
`ConnectorsInvoker` fills Commission's `ConnectorInvoker` from the host's `ConnectorEndpoint`
records (`ess/commission/domains/responsibility.yaml`), one per instance, and a
`CredentialResolver` the host injects; it reads no credential itself. It describes on
`GET /v1/describe` and invokes once on `POST /v1alpha2/invoke`, never `/v1/invoke`, and never
resends. A binding declares its operation's `effect`: `Write` for the `mutation` profile the
service describes (Connectors records an attempt), `Read` otherwise; a binding described otherwise
is `Err` and not invoked. A write's `Performed` names its attempt, a read's the execution audit
record Connectors completed (`audit`) and no attempt, and `ConnectorEffects` refuses any other
`Performed` as a failure to answer. `Refused` is only what Connectors states as nothing changed
(`mutation.classification` `refused` or `not_attempted` on a valid response); every other failure,
a write without a recorded attempt included, is `Err`. The sync port runs on a Tokio runtime the
invoker owns. `crates/loom-connectors/tests/connectors_invoker.rs` holds it against a fake service
on a loopback port.

`loom_connectors::cli::ConnectorsCli` is the other path: a plain read client over the operator's
`connectors` command line, for a host polling a `loom.datasource` source outside a run and for a
plugin turn's `source.read` inside one. It needs no `AdmittedRequest` and no binding. Each read is
`operations describe`, then exactly one `operations invoke --input-stdin`, with `--output json` and
the configured `--config` and `--state-dir`; argv holds only those flags and their ids. It reads
no credential: the CLI's own keyring custody holds it, and the program starts with only the
variables `cli::INHERITED` names, never a token. Writes are refused twice. The client refuses an
operation `operations describe` reports with the `mutation` profile (`cli::WRITE_PROFILES`) before
any invoke; `describe` prints no effect, and adapters publish other profiles, `resource` among
them, for reads, which are read. Connectors refuses any write without `--approval-file` (connectors
`v0.38.0`, `crates/connectors-host/src/local/owner/mutation/execution.rs:161`), which this client
never passes (`no_invoke_ever_carries_an_approval_file`); that second check is what holds a write an
adapter publishes under another profile. A kind the source declares no operation for starts no
process. Each command is bounded by the configuration's `timeout_seconds` (60 when absent) and the
process it started is killed and reaped past it; a process that process forked is not killed, and
outlives the read. A Connectors refusal is a
`ReadRefusal` naming the source, never retried; `not_granted`'s names the remedy, `connectors
connections revalidate --adapter <alias>`, which the client never runs.
`crates/loom-connectors/tests/connectors_cli.rs` and `connectors_cli_adversary.rs` hold it against
the fake `tests/fixtures/connectors/fake-connectors`, which logs its argv.

## Plugin host

`b10x-loom-plugin` (`crates/loom-plugin`) hosts a plugin (ADR `plugin-hooks`): a crate linked at
build time implementing `Plugin` (`poll`, `classify`, `project`, `turn`, `result`, `objectives`),
run by `run_plugin(plugin, config, state, stop)`. Its nouns are the `loom.plugin` domain
(`ess/domains/plugin.yaml`); a plugin's own configuration extends `PluginConfig`. The plugin hooks
sit outside a run and grant nothing. A turn is a thin caller of `run_until_blocked` through
`loom_sdk`, on `inbound-answer@1` from `ProtocolCatalog::plugins()`, with Loom's governed model loop
(`LoopExecutor`), `PluginAuthority` (grants `datasource.read` and `reply.propose`, denies every
other capability) and `DataSourceEffects`, which reads through `loom_connectors::cli`, refuses a
`{source, kind}` outside the projection before any command, and submits one `source_read` evidence
per performed read. Commission ends a Run `AwaitingApproval` after every performed read there, since
both gated actions stay gated; the turn starts the next Run only when every awaited action needs a
capability `PluginAuthority` grants and the Run performed an effect, within `TURN_STEP_BUDGET`.
Model requests are bounded per turn: `TURN_MODEL_BUDGET` counts every request sent, wire retries
included, across all Runs; the port the turn lends refuses any past it. A classifier hint that
names no configured source is dropped before anything prints it. Nothing is sent: a reply is a
proposal in the record. A task gets no turn: the router's pick from `ProtocolCatalog::bundled()`
is recorded as a proposed case. An attempt on an item is counted in the state, which keeps the
item, before it starts; a classify or turn hook that fails leaves it unhandled, each cycle
retries the failing items from the state before it polls, and the third attempt that does not
succeed records it `stopped`. A failure that is not about the item (`PluginError::Unavailable`:
sources that cannot be described, a read whose connectors program timed out or that Connectors
refused `not_granted`, a model that cannot be reached, the router's model or catalog) counts
against none and ends the cycle without saving the poll's cursors. A poll that fails is such an
outage of the whole host: the cycle saves no cursor and the host polls again after its interval;
`run_plugin` returns the poll's failure only when `stop` ends it after that cycle (`once`). A
`stop` set while the host runs ends it after the item being handled, leaving the poll's other items
and its cursors to the next host; a `stop` already set at start (`once`) runs its whole cycle
(`crates/loom-plugin/tests/host.rs`, `a_poll_outage_does_not_stop_the_host`,
`a_stop_ends_the_host_after_the_current_item`). The state directory
holds `state.json`, written to a temporary file and renamed, and `record.jsonl`, the source of
truth for handled items, whose torn last line is dropped under the lock. One host holds it at a
time: `run_plugin` takes an exclusive lock on the record before anything else, and a second host
is refused once the lock stayed held for `LOCK_WAIT`. One inside a configured workspace root or
checkout is refused by comparing paths, with no git command. The tests in
`crates/loom-plugin/tests/` use a recorded classifier, a scripted model port and the fake
`connectors` of `loom-connectors`. The command line hosts registered plugins through
`b10x-loom plugin run|report` (§ Slack handler).

## Slack handler

`b10x-loom-plugin-slack` (`crates/loom-plugin-slack`) is the slack-handler plugin, the one
`b10x-loom plugin run` registers (`PluginName`, held to the crate's `NAME` by a unit test of
`b10x-loom-cli`). Its configuration is the `loom.slack` domain's `SlackConfig`
(`ess/domains/slack.yaml`), which holds the host's `PluginConfig`; `config::read` decodes it from
JSON, refuses an unknown member, and `config::check` refuses one without the bot's user id, the
Slack adapter or connection before anything runs. Its own hook is the poll; classification,
projection, the turn and the record are the host's defaults. The poll reads only through
`ConnectorsCli::invoke_read` on the configured adapter and connection, with the three configured
read operations, and never a write: the channel list, each member channel's history from its
stored `ts`, and a thread only for a message with replies. The walk orders channels by objective
weight, staleness, member count, then a seeded draw. A message younger than `min_age_minutes`, a
bot's, a system message (`subtype` other than `file_share`), one the bot reacted to or one with a
thread reply from anybody but its poster is no item; mentions of the bot come first. A channel's
cursor never passes a message too young to judge. The tests in `crates/loom-plugin-slack/tests/`
reuse `crates/loom-plugin/tests/support/mod.rs` by path and answer Slack-shaped JSON from
`tests/fixtures/` through the fake `connectors`; ids there are synthetic.

`b10x-loom plugin run` settles both models before any read: with `--catalog` from route aliases
(`model_catalog`), otherwise Codex models, as `run` does. A turn runs on
`model_port::LlmModelPort`, the bridge from an llm `Model` to the harness `ModelPort`: one
`Model::turn` per port turn, no retry and no other target, an opaque item carried back under the
wire id `llm-model` only. Without `--once`, the first SIGTERM or SIGINT sets the host's stop, so
a systemd stop ends it after the item being handled; a second one exits at once with 130
(`plugin_run_stops_on_sigterm`). `plugin report` reads `record.jsonl` without the lock and drops a torn
last line. `crates/loom-cli/tests/plugin.rs` runs the binary against the fake `connectors` and the
fixture catalog at a loopback socket replaying recorded responses.

## Laya selector

`b10x-loom-selector-laya` (`crates/loom-selector-laya`) is an experimental `FastTyped`
`ActionSelector`, never the default. It sends the goal and the candidate action ids as one
`choice` question to a Laya endpoint (`POST <base>/v1/systemone`, the wire of the Laya README at
commit `1adc59f`) through llm's `b10x-llm-http` `HttpClient::post_json`, on a runtime the selector
owns, and reports `answer_confidence` as the confidence, never `confidence`. A choice outside the
candidates, a non-2xx status, a timeout, malformed or over-deep JSON, or a probability outside
[0, 1] is `SelectorError::Unavailable`; more than 100 candidates is refused before sending. The
wire stays in this crate until Laya becomes a default selector, when it moves to llm. No product
crate (`b10x-loom-cli`, `b10x-loom-sdk`) depends on it, so Loom builds and runs with no Laya code;
`crates/loom-selector-laya/tests/laya_selector.rs` holds both, the dependency rule over the
resolved graph `cargo metadata` reports.

## Intake and the command line

The CLI's `Briefing` owns single-intent working context. `--context-policy bounded` is opt-in;
`run` and existing `SliceRequest` callers retain legacy behavior. `run_with_options` also accepts
a measurement-report path. Preserve typed revision-bound test state, exact intent and instructions,
complete history records, and the 64 KiB serialized request ceiling including schemas and lookups.
`crates/loom-intake-slice/tests/bounded_context.rs` compares recorded workflows in both policies.
This does not change the separate governed-loop compactor. Reports contain counters only and retain
unknown provider counters as unknown; the catalog-based CLI also reports routing and startup failures.

Result capture and selection belong to `loom-intake-slice::results` and the shared `Briefing`.
Resolve references before returning a `ProposedAction`, never inside an already-admitted effect.
Keep original statuses and capture completeness explicit; stored text is data, not authority or
verified evidence. The store expires with its briefing and grants no cross-run access. The
`result_reference_workflow` integration suite holds the inspect-to-edit path and bounded context.

- `b10x-loom-intake-router` classifies an intent against the host catalog (legacy wrappers use engineering definitions) and
  refuses a pick outside it or below the confidence threshold. A routing proposal is never
  authority; whoever opens the case checks it.
- `b10x-loom-intake-references` extracts tracker keys, chat permalinks, merge and pull requests and
  URLs from an intent, deterministically.
- `b10x-loom-intake-slice` is the local effect adapter (`LocalEffects`) and a thin caller of
  `run_until_blocked`. It has no loop of its own; keep it small. It executes `software.change/1`
  actions inside the given workspace and `system.query/1` through the host clock, never merges, pushes or deploys, and never supplies
  authority on the operator's behalf. It submits evidence from the test command it runs itself.
- `b10x-loom-cli` is `b10x-loom`. Its clap definition is the library (`src/lib.rs`, `Cli`), which
  `loom-docs` renders. The library also holds `evaluate`, the JSON codec of `b10x-loom evaluate`
  over the generated `loom.evaluation` types, and `exit_status` and `events`, the writer of
  `run --output jsonl`: one `intake.events` record per line (`ess/intake/domains/events.yaml`),
  built from the generated types, the terminal record last and exactly once.
  `crates/loom-cli/tests/run_events.rs` holds it; `loom-docs` checks the hand-written
  `website/docs/reference/run-events.md` against the declaration.

The injected `TestRunner` defaults to Substrate 0.7.10, with host and wire pinned together.
Tests get no network, a cleared environment, read-only source and Rust toolchain, writes only
under `target/`, and resource bounds. Mount dependency-cache subdirectories only; never Cargo's
credential/config files or the operator's home. Dependencies are prefetched explicitly by the
operator; the private Cargo home is offline. A missing guarantee stops with a typed refusal;
never silently switch to `UnconfinedRunner`. Test fixtures opt out explicitly.
Validate writable artifacts before every launch: external hardlink aliases and special files
are invalid scopes; internal Cargo hardlinks remain supported.
The CLI attempts one delegated user systemd scope and guards against re-exec loops.
Every executed test observation includes the actual applied confinement; refused launches produce
no test observation or passing evidence. Real delegated tests must run to qualify confinement;
a skip with a named missing prerequisite establishes no qualification. The slice's own git calls
go through one helper, `crates/loom-intake-slice/src/git.rs`: no workspace hook, `core.fsmonitor`
command or signing program runs, a case does not open on a workspace whose own configuration
names a program, and a call is refused once that configuration, its includes or the git directory
changed since the case opened. `tests/host_git_hardening.rs::every_host_git_command_goes_through_a_hardened_helper` rejects
other Git call sites except the separately hardened pinned-source helper at
`crates/loom-protocols/src/git.rs`. That helper fetches into a private bare repository, never a
run workspace, with hooks, fsmonitor, helpers and global/system configuration disabled. Gates still run when the operator or the bot commits and pushes.

Model calls go through llm's crates at release tag `0.4.0`, never a hand-written HTTP client.
`model_retry.rs` owns caller retries using llm's retry
classification and `RetryPolicy`: three identical attempts, no retry after a sink event,
cancellation-aware backoff and per-attempt context metrics. Model ports stay single-attempt;
effects remain outside this loop. `system_query.rs` exercises recovery in all three request phases. The Codex preset and the forced tool call are
`codex_model` (the CLI builds its model with it) and `call_tool` (the router calls it). With
`run --catalog`, the CLI builds both models instead with `b10x-llm-models` `port` from the
route aliases `--model` and `--classifier-model` name (`crates/loom-cli/src/model_catalog.rs`,
held by `crates/loom-cli/tests/catalog_route.rs`): one target per route, no fallback, and no
credential resolver, so a credentialed account is refused before any model call. The
credential is the operator's Codex login (`~/.codex/auth.json`), read and renewed by llm, never by
Loom code. The ported wires take theirs from a `loom.run.WireCredential` reference through
`credentials::ResolvedBearer`, which asks a `SecretResolver` the embedder injects at each call.

## Planning and waves

- The plan is the AEP store under `.engineering/`, written only through `aep plan artifact`. Body
  drafts go in `.engineering/drafts/` (git-ignored). Draft stories are planned, never shipped.
- A wave runs on `wave/<date>-w<n>`: `plan: open wave … (story:…)`, one `impl/<story>` branch per
  story merged into the wave branch, then `plan: close wave …`, and `main` fast-forwards to the
  wave head. Work outside a wave lands as a bot pull request.
- Use a managed worktree (`worktree create --repo ~/beyond10x/loom --purpose …`); keep the primary
  checkout clean.

## Commits, pushes and GitHub writes

Every commit and push is `b10x-bot[bot]`'s through `b10x-gates bot`. Every other GitHub write
(pull request, comment, issue, release, workflow dispatch, re-run) goes through `b10x-gates api`.
`gh` is read-only here. Loom is enrolled in common Gates (`shared-gates.yml`); its
`common / Security and privacy` check scans every commit since the policy baseline, so a
`/home/<name>/` path literal in any commit fails it and admits no exception.

## Releases

Loom releases from source at bare-version tags (`0.1.0`, no `v`); every crate inherits the
workspace version and nothing is on a registry. Consumers pin `tag = "<version>"`, and
`CHANGELOG.md` collects under **Unreleased** between releases. A release is:

1. A release commit on `main` (through a wave or a bot pull request): `[workspace.package] version`
   in `Cargo.toml`, `Cargo.lock`, and **Unreleased** turned into `## [<version>] - <date>` under an
   empty **Unreleased**, opening with a one-paragraph summary fit for the release notes. README,
   AGENTS.md and the site pages that name the version or a dependency line move with it.
   `cargo run -q --locked -p loom-xtask -- release-check --tag <version>` passes on it.
2. `check` and `Shared source gates` green on that commit.
3. An annotated tag by `b10x-bot[bot]` on that commit: `b10x-gates bot … -- tag -a <version> -m
   "Loom <version>" <commit>`, then `-- push origin <version>`.
4. `release` (`.github/workflows/release.yml`) green on the tag. It runs `loom-xtask release-check`
   (the tag equals the workspace version and `CHANGELOG.md` has its entry) and then `check.yml` on
   the tagged commit. It is read-only, needs no secret and creates no release.
5. The GitHub Release for the tag, created by the bot (`b10x-gates api --method POST --path
   /repos/beyond10x/loom/releases`), named `<version>`, its notes the version's CHANGELOG entry.

A release is finished when the tag, the green `release` run and the bot's GitHub Release with its
notes are verified. A pushed tag whose `release` run is not green is queued, not released; never
move or delete a pushed tag, fix forward with the next version.
`crates/loom-xtask/tests/release_check.rs` holds the check, the workflow's shape, and that the
README and site pin the workspace version as a tag.

## Documentation

The site (`website/`, Docusaurus on `@beyond10x/docs-system`) is written with the workspace `docs`
skill (`~/beyond10x/.agents/skills/docs/SKILL.md`). It is independent of the unified site:
`pages.yml` (`Documentation validation`) builds it and binds it to its commit
(`loom-docs provenance`), and on a bot push to `main` `b10x-docs-site.yml` (`Documentation site`)
deploys it to <https://beyond10x.github.io/loom/> through Website's `project-site.yml`.

Every command on a page is run in the tree before it is written, with its output pasted from that
run. Never make a live model call for a page; cite a record under `docs/qualification/`.

## Never

- Never make a model or network call in a test (tests use recorded responses and local fixtures),
  and never for a documentation page.
- Never read a credential file in Loom code or tests.
- Never edit a generated file by hand, and never write an `UNMAPPED:` marker into `ess/`.
- Never implement a story while either ESS gate is red.
- Never add a dependency from `b10x-loom-commission` to the executor, Canon or a model-provider
  crate.
- Never let the slice or the command line merge, push or deploy.
- Never commit or push as anything but the bot, and never use a `gh` write.
- Never write a `/home/<name>/` path literal anywhere; write `~/`.
