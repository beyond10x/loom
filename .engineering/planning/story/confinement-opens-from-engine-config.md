---
format: aep.planning-md/3
id: story:confinement-opens-from-engine-config
kind: story
status: proposed
title: A host names Loom's confinement once in engine configuration, and local runs through Substrate's contract with the operator's reason
summary: EngineConfinement and one constructor; local, with a reason, opened through confinement::open; linux stays on today's host-driver route until story:child-launches-through-confinement-adapters
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/child.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: generated/rust/intake
- confidence: inferred
  path: website/data/ess/intake-confinement.domain-graph.json
- confidence: inferred
  path: website/docs/concepts/governor-and-intake.md
- confidence: inferred
  path: website/docs/guides/run-an-intent.md
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
---
## Outcome

A host configures Loom's confinement once, as engine configuration: `EngineConfinement`, the name
of one Substrate adapter (`linux`, or `local` with a reason) and what that adapter needs (for
`linux` the optional delegated cgroup root `--cgroup-root` gives today). One constructor,
`EngineRunner::open(&EngineConfinement)`, builds the runner from that value. `EngineRunner`
implements `TestRunner` and has `run_child` (standard input included) and `spawn_child`, which then
run under it. A request never names an adapter. Bound commands reach this runner in
`story:bound-commands-run-confined`, not here.

With `local`, the constructor opens Substrate's `local` adapter through
`substrate_host::confinement::open` with `AdapterRequest::Local`, and the three launches run on the
adapter it returns: start, `write_stdin` and `close_stdin`, `read_output` for a live child,
`signal`, `wait` and `output`. Each launch starts, reads and waits inside one runtime it owns,
because `local` spawns a run's drain and reaper tasks on the caller's runtime
(`crates/substrate-host/src/confinement/local.rs:299-340`): `run_child` and `run` on the runtime
they build per call (`crates/loom-intake-slice/src/confinement.rs:570`, `:606`), a live child on
the runtime `ConfinedChild` owns (`crates/loom-intake-slice/src/confinement/child.rs:157`). The
adapter is shared by every launch of the runner; a run is not.

"Unconfined, declared insecure" means, under `local`:
- The child runs as the operator with the host's file system and the host's network. Every
  `ChildCommand` asks for no network by default (`NetworkMode::None`, `confinement.rs:1091`), which
  `local` cannot keep: it proves `host_network` (`local.rs:118`), and admission refuses an
  isolated network on such an adapter (`confinement/mod.rs:616-629`). So Loom sends no network
  request under `local`, and the record says what the child had: its `facts` name `host_network`
  and `declared_insecure`, and `network` is absent, never an empty list.
- The operator's reason goes in the profile's `local_reason`. Each launch records adapter `local`,
  the reason and the facts `local` proved, with `filesystem` `unconfined`.
- The workspace is opened read-write, since `local` refuses a scoped one (`mod.rs:897-911`).
- No process or output bounds are sent: `local` serves no `limits` and admission would refuse them
  (`profile.rs:202`, `mod.rs:621-630`).
- What `local` cannot serve is refused before launch, `CapabilityUnserved` naming the option and
  its family, and never dropped: egress apertures and their bundle (`network_egress`), and
  read-only roots and files, those given through `at_host_path` included (`filesystem_view`).

The constructor refuses a blank reason, `LocalReasonMissing`, by Substrate's own rule
(`ConfinementProfile::reason`, `profile.rs:234-238`). It refuses a host whose policy forbids
`local` by reading the opened adapter's `local_permitted` fact (`local.rs:92-99`, `:117`), as
`CapabilityUnserved` naming `SUBSTRATE_CONFINEMENT_LOCAL` and Substrate's code
`confinement.local-refused`, with no new refusal variant. Both refusals come before any child
starts.

Paths under `local` are host paths. Nothing Loom sends to `local` names `/workspace` or
`/toolchain`:
- The test runner's `run` builds its command as the unconfined runner does (`run_bounded`,
  `crates/loom-intake-slice/src/executor.rs:1271`). It runs in the workspace directory with the
  host's toolchain, mounts no read-only root, and creates no private cargo home. Today's `run`
  discovers mounts and sets sandbox paths (`RustClosure::discover`, `confinement.rs:994`,
  `:3135-3233`); under `local` the same variables take host values instead. `PATH` and `HOME` are
  the host's. `RUSTUP_HOME` and `CARGO_HOME` are the host's where set. `CARGO_TARGET_DIR` is the
  workspace's `target` as a host path. `CARGO_NET_OFFLINE`, `CARGO_BUILD_JOBS` and
  `RUSTUP_AUTO_INSTALL` keep today's values.
- `local` clears the environment and sets only what it is given (`local.rs:265-270`), and holds
  it to Substrate's closed shape (`crates/substrate-guarded/src/environment.rs:19-41`). Unlike
  `run_bounded`, which inherits the host's environment, nothing else is inherited. A value that
  shape refuses, a `PATH` over 4096 bytes say, is refused `exec.environment-refused` and never cut.
- `with_workdir` names the directory as the child sees it under `linux`: `/workspace` or a path
  beneath it (`confinement.rs:1423`). `local` takes a workdir relative to the workspace
  (`relative_workdir`, `mod.rs:913-929`), so `/workspace/<relative>` is sent as `<relative>` and
  `/workspace` as no workdir. A workdir under a read-only root's mount cannot reach `local`,
  because the root is refused.
- `ChildCommand::sandbox_path` (`confinement.rs:1446`) keeps answering the `linux` view. The
  runner gains `EngineRunner::child_path(&ChildCommand, &Path)`, which answers `sandbox_path`'s
  value under `linux` and, under `local`, the canonical host path itself wherever `sandbox_path`
  maps one, so a host that builds arguments through the runner passes the child a path it can
  open.

A `local` run is ended at its deadline by Loom, since `local` serves no limits. Loom holds every
launch's timeout itself, for `run_child` with and without standard input, `spawn_child` and `run`
alike. At the deadline it sends `Signal::Kill` through the adapter, and `local` sends `SIGKILL` to
the run's process group, which the leader's pid names while the leader is unreaped
(`local.rs:17-23`, `:274-276`, `:427-453`). `wait` then returns once the leader is reaped and the
group is empty, or after `LOCAL_GROUP_SETTLE`, 1 s (`local.rs:58-61`, `:312-340`). The result has
no exit status and `timed_out` set. That reaches what today's kill of an unconfined command
reaches (`kill_group`, `crates/loom-intake-slice/src/executor.rs:1386-1418`; `finish` and `stop`,
`crates/loom-intake-slice/src/bindings.rs:565`, `:653`): every process still in the group, none
that left it.

What a `local` record carries, and what it changes in the public types:
- `backend` is `Backend::None`, the value an unconfined record has (`executor.rs:1180`), because
  `Backend` says whether confinement was applied and `local` applies none. Nothing that reads
  `Backend` can take a `local` run for a confined one. `adapter` `local` says that Substrate's
  contract ran it. `EngineRunner::backend()` answers `None` under `local` and `Substrate` under
  `linux`.
- The `confinement:` line `software-change@1` prints (`crates/loom-intake-slice/src/intent.rs:711`,
  `:860`) comes from a new `TestRunner::label` method. Its default is today's
  `backend_name(self.backend())` (`executor.rs:1201-1207`), so `confinement: substrate` and
  `confinement: none` are unchanged. `EngineRunner` under `local` answers
  `local (<reason>)`, with control characters escaped, so the line reads
  `confinement: local (<reason>)`. A clock query builds no runner (`crates/loom-cli/src/main.rs:1151`)
  and still prints no `confinement:` line.
- `AppliedConfinement` gains `local_reason`. `ChildResult.driver_record`
  (`confinement/child.rs:70`) becomes `Option<substrate_wire::AppliedConfinement>`, `None` under
  `local`, which has no host-driver record, as `TestExecution.driver_record` already is
  (`confinement.rs:84`). Both are breaking changes to public types: `CHANGELOG.md` gets a
  `**Breaking:**` bullet for each, and the `ChildResult` row of
  `website/docs/reference/sdk-stability.md` (`:137`) says the record is absent under `local`.

Resume: the resume context keeps only `Backend` (`crates/loom-intake-slice/src/resume.rs:509-512`),
and `resume` rebuilds a runner from it (`crates/loom-cli/src/main.rs:670-671`). A `local` run kept
for resume would come back under another adapter with no word. So `b10x-loom run` refuses
`--confinement local` together with `--state`, exit 2, naming both flags: a `local` run is not
kept for resume, and an operator who wants one runs again with the flags and the reason.

With `linux`, the constructor wraps today's `SubstrateRunner` unchanged. Launches stay on the
host-driver route: `HostDriver::open` (`crates/loom-intake-slice/src/confinement.rs:402`), one
driver per workspace parent, egress set and certificate bundle, opened at the first launch that
needs it (`driver`, `:321-359`), as today. The constructor opens nothing for `linux`, so building a
runner ends no run under the cgroup root (`:116-124`).

After this story two launch routes exist, a stated intermediate state: `local` on Substrate's
confinement contract, `linux` on the host driver.
`story:child-launches-through-confinement-adapters` moves `linux` onto the contract route this
story builds and deletes the host-driver route in the same change; it waits on this story.

## Why

`~/beyond10x/sdc/docs/layers.md` § 3: the substrate is injected today only as a `TestRunner`
implementation, and `SubstrateRunner` builds a `HostDriver` itself
(`crates/loom-intake-slice/src/confinement.rs:402`). The unconfined route is a second type,
`UnconfinedRunner` (`crates/loom-intake-slice/src/executor.rs:1163-1197`), which has no
`run_child`. So "unconfined" is a type a host picks, not a value of the engine's configuration,
and a tree of cases whose tools need host paths and a Unix socket (the SDC's `slot`, `ess`, `aep`
calls) cannot run through `run_child` at all. Substrate's `local` adapter exists for this: it
requires `declared_insecure` and a non-blank `local_reason`, and host policy can refuse it
(`substrate-host` `confinement/local.rs`, read at 0.21.0). This story makes the choice one engine
value and puts `local` behind `run_child`. It does not take `HostDriver::open` off the `linux`
launch path.

It comes first because the `linux` contract route is held by
`upstream-blocker:substrate-run-observation-names-the-limit`: moving `linux` launches loses the
limit reason Loom reads in process until a Substrate release names it. `local` serves no `limits`
family (`local.rs:3-6`), so the blocker does not concern it. Session `sdc` accepted the narrowing
and the two-route state on 2026-11-21.

## ESS first

`ess/intake/domains/confinement.yaml` first: an `EngineConfinement` type with the adapter name
(`Linux`, `Local`), an optional `local_reason` and an optional `cgroup_root`; the refusal
`LocalReasonMissing` in `ConfinementRefusal` (`:12`); and the reason as the member `local_reason`
of the applied confinement record, beside its existing `adapter` (`:70`). The comment above
`adapter` and `facts` (`:67-69`) says both are absent on an unconfined run. It changes in the same
commit: both are present on a `local` run, which is unconfined, and absent on an `UnconfinedRunner`
run. `Backend` keeps its two values. On that commit `task intake-drift` fails; the story's new test
`crates/loom-intake-slice/tests/engine_confinement.rs` does not compile.

## Acceptance

Tests named `engine_confinement.rs::<name>` are in `crates/loom-intake-slice/tests/engine_confinement.rs`.
Those named `loom_cli.rs::<name>` are in `crates/loom-cli/tests/loom_cli.rs`. A test marked "gated"
is skipped with a named missing prerequisite, as the delegated tests are.

The constructor and the source:
- `engine_confinement.rs::only_the_constructor_opens_an_adapter` reads every file under the
  `src/` directory of each crate in `crates/`, as
  `every_host_git_command_goes_through_a_hardened_helper` does
  (`crates/loom-intake-slice/tests/host_git_hardening.rs:444-452`). It never reads `tests/`, so
  its own patterns are outside its match. It finds `confinement::open(` in exactly one function,
  the constructor, called there with `AdapterRequest::Local`.
- The same test finds no `AdapterRequest::Linux` in those files.
- `engine_confinement.rs::only_the_constructor_takes_an_engine_confinement`, over the same files,
  finds `EngineConfinement` as a parameter type of exactly one public function, the constructor.

Launches under `local`, with a reason:
- `engine_confinement.rs::each_local_launch_writes_outside_its_workspace`: each of `run_child`
  without standard input, `run_child` with `with_stdin`, `spawn_child` and the test runner's
  `run` starts a program that writes a file outside its workspace, and the file exists afterwards.
- `engine_confinement.rs::each_local_record_names_the_adapter_and_reason`: each of those four
  launches records adapter `local` and the reason.
- `engine_confinement.rs::a_local_record_claims_no_confinement`: each of those records has
  `backend` `None`, `filesystem` `unconfined` and no `network`.
- `engine_confinement.rs::a_local_record_names_the_host_network`: each of those records has
  `facts` holding `host_network` and `declared_insecure`.
- `engine_confinement.rs::a_local_test_run_sees_host_paths`: the test runner's `run` of
  `sh -c 'printf "%s\n" "$PATH" "$HOME" "$CARGO_TARGET_DIR"'` prints the host's `PATH`, the host's
  `HOME` and the workspace's `target` directory as a host path, and no printed value starts with
  `/toolchain` or `/workspace`.
- `engine_confinement.rs::a_local_child_starts_in_the_host_directory`: `run_child` of `pwd` with
  `with_workdir("/workspace/sub")` prints the host path of the workspace's `sub` directory.
- `engine_confinement.rs::a_local_live_child_starts_in_the_host_directory`: `spawn_child` of `pwd`
  with `with_workdir("/workspace/sub")` reads the host path of the workspace's `sub` directory as
  its output.
- `engine_confinement.rs::child_path_under_local_is_the_host_path`: `child_path` of a file in the
  workspace answers that file's canonical host path, and `run_child` of `cat` given that answer
  exits 0 with the file's bytes as its output.
- `engine_confinement.rs::a_local_live_child_outlives_another_launch`: a `spawn_child` child of
  one runner still delivers output after a `run_child` on the same runner returned.
- `engine_confinement.rs::a_local_live_result_has_no_driver_record`: a `local` `spawn_child`
  child's `ChildResult.driver_record` is `None`.

Deadline under `local` (each with a 1 s timeout, a program that starts a background `sleep 30`
and then sleeps 30 s):
- `engine_confinement.rs::a_local_child_past_its_timeout_is_killed_with_its_group`: `run_child`
  returns within 3 s with `timed_out` set and no exit status, and no process of its group remains
  (`kill(-pgid, 0)` answers `ESRCH`).
- `engine_confinement.rs::a_local_child_with_stdin_past_its_timeout_is_killed_with_its_group`: the
  same for `run_child` with `with_stdin`.
- `engine_confinement.rs::a_local_live_child_past_its_timeout_is_killed_with_its_group`: the same
  for a `spawn_child` child left to its timeout.
- `engine_confinement.rs::a_local_test_past_its_timeout_is_killed_with_its_group`: the same for the
  test runner's `run` of a `TestCommand` with that timeout.

Refusals under `local`:
- `engine_confinement.rs::a_blank_reason_is_refused_before_any_child`: the constructor given
  `Local` with a reason of white space only is refused `LocalReasonMissing`, and no child starts.
- `engine_confinement.rs::host_policy_refusal_runs_in_its_own_process`: the test starts its own
  binary again for this one case, with `SUBSTRATE_CONFINEMENT_LOCAL=refuse` set on that child
  process only, so no other test sees the variable. There the constructor is refused
  `CapabilityUnserved`, and the detail names `SUBSTRATE_CONFINEMENT_LOCAL` and
  `confinement.local-refused`.
- `engine_confinement.rs::a_local_child_with_egress_is_refused`: a `ChildCommand` with
  `with_egress` is refused `CapabilityUnserved` naming `network_egress`, and the file its program
  would write does not exist.
- `engine_confinement.rs::a_local_child_with_a_read_only_root_is_refused`: a `ChildCommand` with
  `with_read_only_root` is refused `CapabilityUnserved` naming `filesystem_view`, and no child
  starts.
- `engine_confinement.rs::a_local_child_with_a_read_only_file_is_refused`: a `ChildCommand` with
  `with_read_only_file` is refused `CapabilityUnserved` naming `filesystem_view`, and no child
  starts.

Under `linux`:
- `engine_confinement.rs::building_a_linux_runner_ends_no_run` (gated): a second runner built from
  the same value while a first runner's `spawn_child` child runs leaves that child running, and it
  ends with its own exit status.
- `engine_confinement.rs::a_linux_record_has_no_reason` (gated): a constructor-built runner's
  `run_child` record names adapter `linux` and has no `local_reason`.
- `engine_confinement.rs::an_undelegated_root_is_refused_and_nothing_falls_back`: with a
  `cgroup_root` that is not a delegated cgroup, a constructor-built runner's `run_child` is
  refused `CgroupUndelegated`, and no child starts.

The run line and the command line:
- `engine_confinement.rs::a_local_run_prints_its_adapter_and_reason`: `run_intent_with_options`
  for `software-change@1`, with an `EngineRunner` on `local` with the reason `x` and a recorded
  classifier and agent, writes the line `confinement: local (x)`.
- `loom_cli.rs::local_confinement_is_offered_with_a_reason`: `b10x-loom run --help` lists `local`
  among the values of `--confinement` and lists `--local-reason`.
- `loom_cli.rs::local_without_a_reason_is_refused`: `b10x-loom run --confinement local` without
  `--local-reason` exits 2, and its standard error names `--local-reason`.
- `loom_cli.rs::a_local_run_is_not_kept_for_resume`: `b10x-loom run --confinement local
  --local-reason x --state <dir>` exits 2, and its standard error names `--state` and
  `--confinement local`.
- `loom_cli.rs::none_prints_its_deprecation_naming_local`: `b10x-loom run --confinement none` with
  no credentials prints one line on standard error naming `local` as the replacement before it
  fails on the missing login, and makes no model call.
- `grep -n` on `CHANGELOG.md` finds `UnconfinedRunner` under `### Deprecated` (it stays exported
  for one release, the SDK stability page rule).
- `grep -n` on `CHANGELOG.md` finds a `**Breaking:**` bullet naming `AppliedConfinement.local_reason`.
- `grep -n` on `CHANGELOG.md` finds a `**Breaking:**` bullet naming `ChildResult.driver_record`.
- `task check` exits 0.

Kept as it is. These guard behaviour that holds today; they pass before the story and must still
pass after it:
- `a_routed_query_completes_without_software_test_configuration`
  (`crates/loom-cli/tests/adversary_slice_run.rs:596-640`): a clock query prints no
  `confinement:` line (`:639`).
- The delegated cases of `crates/loom-intake-slice/tests/confinement.rs` and
  `crates/loom-intake-slice/tests/confined_child.rs` (the scoped `target` write, a refused write
  outside it), on `SubstrateRunner`, which the `linux` runner wraps.

## Out of scope

Bound commands (`story:bound-commands-run-confined`). Opening `linux` through
`confinement::open`, one open point for every adapter, one internal launch route, and removing
`HostDriver::open` from the launch path (`story:child-launches-through-confinement-adapters`).
Memory, CPU and process limits under `local` and their advisory form
(`story:action-limits-enforced`). Resuming a `local` run. The `macos`, `systemd`, `oci` and `pod`
adapters and remote placement (`story:confines-on-macos-through-substrate`,
`story:mantle-placement`). The session executor's harness spawn and the Connectors invoker.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-intake-slice/src/confinement.rs` — cited. `SubstrateRunner` (`:126`), `run_child` (`:475`), `spawn_child` (`:525`), `impl TestRunner` (`:984`), `HostDriver::open` (`:402`), `adapter = Some("linux")` (`:256`)
- **Files:** `crates/loom-intake-slice/src/executor.rs:1163-1197` — cited. `UnconfinedRunner`, plus `backend_name` (`:1201`) and `observation_confinement` (`:1208`), where `local_reason` is written
- **Files:** `crates/loom-intake-slice/src/confinement/child.rs:118-241` — cited. `ConfinedChild` holds an `OpenDriver` and calls `HostDriver` pipe sessions; a `local` child needs another route behind the same return type of `spawn_child`
- **Files:** `crates/loom-intake-slice/src/intent.rs:711,860` — cited. The two `confinement:` lines, from `backend_name(runner.backend())`; `ResumeHost.confinement: Backend` at `:356`
- **Files:** `crates/loom-cli/src/lib.rs:777,985` — cited. `--confinement`, `enum Confinement`; `--local-reason` goes here
- **Files:** `crates/loom-cli/src/main.rs:670,1017,1152,1331` — cited. Resume runner, `ResumeHost` mapping, run runner, `eprintln!("confinement: substrate")`
- **Files:** `ess/intake/domains/confinement.yaml:12,70` — cited. `ConfinementRefusal` variants, `adapter` member; `EngineConfinement`, `LocalReasonMissing` and `local_reason` go here
- **Files:** `crates/loom-intake-slice/tests/engine_confinement.rs` — cited, new, named by the story
- **Files:** `CHANGELOG.md` — cited. `### Deprecated` for `loom_sdk::UnconfinedRunner`; the new `AppliedConfinement` field also wants a `**Breaking:**` bullet (precedent `d56d248b`)
- **Generated:** `generated/rust/intake/` — inferred. `task intake-drift` forces it. Any intake ESS change rewrites the model digest in every file there (`d56d248b` touched all 16)
- **Generated:** `website/docs/reference/ess/intake-*.md` (8 pages), `index.mdx`, `website/data/ess/intake-confinement.domain-graph.json` — inferred. `task docs-check` forces them, by the same digest
- **Symbols:** `EngineConfinement` (new), `SubstrateRunner`, `UnconfinedRunner`, `TestRunner`, `AppliedConfinement`, `Backend`, `ConfinementRefusal`, `substrate_host::confinement::{open, AdapterRequest::Local, LocalRequest}` — cited
- **Also likely:** `crates/loom-intake-slice/src/resume.rs:510,587` — inferred. `Backend` is kept as text "Substrate" or "None"; whether a `local` run can resume is not in the story
- **Also likely:** `crates/loom-sdk/src/lib.rs:117,123` — inferred. The new type and constructor are exported beside `SubstrateRunner`
- **Documents:** `website/docs/reference/sdk-stability.md:134` — inferred. The row listing the runners
- **Documents:** `website/docs/guides/run-an-intent.md:131`, `website/docs/concepts/governor-and-intake.md:134` — inferred. Both say only `--confinement none` runs with the operator's rights; false after this story
- **Documents:** `AGENTS.md` (confinement section, `:2018-2100`) — inferred. Durable context; `d56d248b` updated it
- **Confidence:** medium. The code surface is read in the tree and the Substrate side verified at 0.21.0 (`adfde40`). The resume and SDK ripple and the clock-query acceptance are not settled by the story
- **Would collide with (no `depends_on` path orders these with this story):**
  - `story:decision-suspension-reason` (active) on `crates/loom-cli/src/lib.rs`; it edits `:83,116-150`, this story `:777,985` — cited
  - `story:engineering-protocol-lock` (active) on every file under `generated/rust/intake/` (`protocols.rs` included) and the `intake-*.md` pages, through the digest headers; its `protocols.yaml` change regenerates them — inferred
  - `story:hash-chained-run-log` (active): no shared path; both touch only the house-rule files `CHANGELOG.md` and `AGENTS.md` — cited
  - `story:approval-decisions-resume-held-runs`: `cli/src/lib.rs`, `main.rs`, `intent.rs`, and the intake ESS — cited
  - `story:decisions-resume-the-held-run`: `cli/src/lib.rs`, `main.rs`, `resume.rs`, `sdk-stability.md`, and the intake ESS — cited
  - `story:question-and-outcome-decisions-hold-runs`: `cli/src/lib.rs`, `main.rs`, `resume.rs`, and the intake ESS — cited
  - `story:secrets-subcommand`: `cli/src/lib.rs`, `main.rs`, `generated/rust/intake`, and the intake ESS — cited
  - `story:approvals-supplied-to-canon-evaluation`: `cli/src/lib.rs`, `main.rs` — cited
  - `story:decision-inbox-commands`: `cli/src/lib.rs`, `main.rs`, `sdk/src/lib.rs`, `sdk-stability.md` — cited
  - `story:agent-selects-the-connectors-relay`: `cli/src/lib.rs`, `main.rs`, `sdk/src/lib.rs`, `sdk-stability.md` — cited
  - `story:connectors-relay-client`: `sdk/src/lib.rs`, `sdk-stability.md` — cited
  - `story:silence-and-expiry-at-a-host-tick`: `cli/src/lib.rs` — cited
  - `story:engineering-protocols-dependency-removed`: `sdk-stability.md` — cited
  - `story:mantle-placement`: `crates/loom-cli` (a whole crate) — cited
  - `story:python-test-profile`: `confinement.rs` — cited
  - Ordered by a `depends_on` path (they depend on this story; transitively except the three direct ones): `story:bound-commands-run-confined` (direct), `story:child-launches-through-confinement-adapters` (direct), `story:substrate-daemon-runner` (direct, no shared path), `story:tcp-reachability-through-an-aperture`, `story:observe-executor-reads-workspace-files`, `story:child-case-runtime`, `story:tree-run-command`, `story:agent-node-run-records`, `story:tree-waits-open-decision-requests`, `story:tree-usage-and-model-budget`, `story:concurrent-cases-in-the-tree-daemon`, `story:daemon-continues-a-decided-request`, `story:flow-limits-recorded-as-events`, `story:run-caps-per-arrival`, `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant`, `story:goals-entries-from-a-resolved-document`, `story:goals-grounds-and-free-answers` — cited
- **Safety fact:** a launch must start, read and wait inside one runtime, because `local` spawns its drain and reaper tasks on the caller's runtime (`local.rs:300-340`) and the constructor-built adapter is shared across launches; `run_child` and `run` use a runtime per call (`confinement.rs:570,606`), `ConfinedChild` owns its own (`child.rs:157`) — step 2, unproven
