---
format: aep.planning-md/3
id: story:confinement-opens-from-engine-config
kind: story
status: implemented
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
  path: crates/loom-cli/tests/loom_cli.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/child.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement/engine.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/engine_confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w1_u1.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w4_u1.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261121w4_u1_pass2.rs
- confidence: inferred
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: ess/intake/domains/confinement.yaml
- confidence: inferred
  path: generated/rust/intake
- confidence: inferred
  path: website/docs/concepts/governor-and-intake.md
- confidence: inferred
  path: website/docs/guides/run-an-intent.md
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 30
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-11-21T07:20:39Z", actor: "agent:sdc", revision: 18, decided_on: {"recorded":{"review_outcome":6}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
- {from: "active", to: "implemented", at: "2026-11-21T09:11:55Z", actor: "agent:sdc", revision: 30, decided_on: {"recorded":{"test_result":1,"review_outcome":8,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w4"}
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
`--confinement local` together with `--state`, exit 2, naming both flags (the command line's usage refusal: no refusal variant is declared for it): a `local` run is not
kept for resume, and an operator who wants one runs again with the flags and the reason.

The library refuses as well (`review-result:confinement-opens-from-engine-config-security-1`,
finding 2): `HostText::of` (`crates/loom-intake-slice/src/intent.rs`), which serves
`ResumeHost::check` and the code that stores a resume context, answers
`SliceError::State("a local run is not kept for resume: …")` for a runner whose
`TestRunner::local_reason` answers a reason, and stores nothing. That method has a default,
`None`. A host's own runner that launches through a `local` runner and does not forward it has
its run kept for resume as an unconfined run; forwarding it is the host's duty, stated in the
method's documentation and on the SDK stability page
(`review-result:confinement-opens-from-engine-config-security-2`, finding A, declined by the
coordinator at the close of wave 2026-11-21-w4: a required method breaks every implementor of a
listed trait).

The reason is escaped wherever it is printed or recorded (`confinement::escape_reason`): control
characters, the line and paragraph separators, every format character (Unicode category Cf, the
bidirectional controls and the tag block among them) and the backslash, so a reason cannot forge
or hide a line and two different reasons never record as the same text. The record holds the
escaped text.

Substrate permits `local` when `SUBSTRATE_CONFINEMENT_LOCAL` is unset: on a host that sets
nothing, the flag and the reason are the whole opt-in.

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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w4 from the unit's commits (`git diff --stat 63668a97 281013d8`: 46 files, 3534 insertions, 100 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-intake-slice` — cited, 11 of the 46 changed files are under it (six in `src/`, one of them the new `src/confinement/engine.rs`; five in `tests/`). Beside it: `crates/loom-cli` 3 files, `crates/loom-sdk` 1, `generated/rust/intake` 16, `website` 12, `ess/intake` 1, and `AGENTS.md`, `CHANGELOG.md`
- **Files (ESS sources):** `ess/intake/domains/confinement.yaml` (21 insertions, 3 deletions) — cited, the new enums `intake.confinement.ConfinementAdapter` (`Linux`, `Local`) and the struct `intake.confinement.EngineConfinement` (`adapter`, `local_reason`, `cgroup_root`), the variant `LocalReasonMissing` in `ConfinementRefusal`, the member `local_reason` of `AppliedConfinement`, and the comment on `adapter` and `facts` that says both are present on a `local` run
- **Files (generated):** `generated/rust/intake/` (16 files: `Cargo.toml`, `PLAN.md`, `plan.json` and 13 files of `src/`) — cited, from the intake ESS generator; `src/confinement.rs` (26 insertions, 2 deletions) carries the new types, the other 12 `src/` files change by 2 lines each, the model digest
- **Files (generated docs):** `website/docs/reference/ess/intake-{bindings,confinement,context,events,protocols,query,results,routing}.md` (8 pages; `intake-confinement.md` 17 insertions, 4 deletions, the others 1 line each), and `website/docs/reference/cli.md` (2 insertions, 1 deletion: the `--confinement` row and the new `--local-reason` row) — cited, from the ESS and CLI generators
- **Files (source):** `crates/loom-intake-slice/src/confinement/engine.rs` — cited, new, 922 lines: `EngineRunner` with `open`, `preflight`, `run_child`, `spawn_child`, `child_path` and `impl TestRunner`, the `local` route (`LocalRoute`, `LocalChild`), the refusal mapping from Substrate's contract errors, and the deadline kill of the run's process group
- **Files (source):** `crates/loom-intake-slice/src/confinement.rs` — cited, 95 insertions, 2 deletions: `mod engine`, the re-exports of `EngineRunner`, `ConfinementAdapter` and `EngineConfinement`, `TestRunner::label` and `TestRunner::local_reason` with defaults, `local_label`, `escape_reason` and the escape of format characters, line and paragraph separators and the backslash, `local_reason: None` on `SubstrateRunner`'s records, `Some(driver_record)` in `live_result`
- **Files (source):** `crates/loom-intake-slice/src/confinement/child.rs` — cited, 120 insertions, 17 deletions: `ChildResult.driver_record` becomes `Option`; `ConfinedChild` becomes a route over the host-driver session and a `local` child, its five methods moved onto the route
- **Files (source):** `crates/loom-intake-slice/src/executor.rs` — cited, 20 insertions, 1 deletion: `applied_label`, `local_reason` on the `UnconfinedRunner` record, `local_reason` in the observation JSON, and `TestRun::summary` printing the label
- **Files (source):** `crates/loom-intake-slice/src/intent.rs` — cited, 15 insertions, 4 deletions: both `confinement:` lines print `runner.label()`; `HostText::of` refuses a runner that answers `local_reason`, so `ResumeHost::check` and `keep` refuse a `local` run with `SliceError::State`
- **Files (source):** `crates/loom-intake-slice/src/run.rs` — cited, 1 insertion, 6 deletions: the `confinement:` line prints `runner.label()`
- **Files (source):** `crates/loom-cli/src/lib.rs` — cited, 11 insertions, 2 deletions: `--local-reason` (required with `local`) and `Confinement::Local`, `none` marked deprecated
- **Files (source):** `crates/loom-cli/src/main.rs` — cited, 70 insertions, 7 deletions: `usage_refused` (exit 2) for `--state` with `local` and for `--local-reason` without it, the `none` deprecation warning on standard error, the run runner built through `EngineRunner::open` with `preflight`, `confinement_refused` takes the label, resume refuses `local`
- **Files (source):** `crates/loom-sdk/src/lib.rs` — cited, 6 insertions: exports `ConfinementAdapter`, `EngineConfinement`, `EngineRunner`
- **Files (new tests):** `crates/loom-intake-slice/tests/engine_confinement.rs` (1250 lines, 32 cases, the story's test), `crates/loom-intake-slice/tests/security_w20261121w4_u1.rs` (589 lines, 11 cases) and `crates/loom-intake-slice/tests/security_w20261121w4_u1_pass2.rs` (156 lines, 4 cases), the two security passes' cases, committed
- **Files (existing tests):** `crates/loom-cli/tests/loom_cli.rs` (87 insertions: the helper `refused` and four new cases, `local_confinement_is_offered_with_a_reason`, `local_without_a_reason_is_refused`, `a_local_run_is_not_kept_for_resume`, `none_prints_its_deprecation_naming_local`; 4 cases before, 8 after), `crates/loom-intake-slice/tests/confined_child.rs` (17 insertions, 4 deletions: reads `driver_record` as an `Option` with `.expect`), `crates/loom-intake-slice/tests/security_w20261121w1_u1.rs` (2 insertions, 1 deletion: the same)
- **Files (documents):** `AGENTS.md` (23 insertions, 1 deletion: the engine-confinement paragraph, `UnconfinedRunner` named in the unconfined-record sentence), `CHANGELOG.md` (10 insertions: three Added bullets, three Changed bullets marked **Breaking:** for `AppliedConfinement.local_reason`, `ChildResult.driver_record` and `ConfinementRefusal::LocalReasonMissing`, and a new `### Deprecated` for `UnconfinedRunner` and `--confinement none`), `website/docs/reference/sdk-stability.md` (4 insertions, 3 deletions: the `TestRunner`, runners, `ConfinedChild` rows and a new row for `EngineRunner`), `website/docs/guides/run-an-intent.md` (11 insertions, 2 deletions), `website/docs/concepts/governor-and-intake.md` (5 insertions, 2 deletions)
- **Symbols:** added — `loom_sdk::EngineRunner` with `open(&EngineConfinement)`, `preflight`, `run_child`, `spawn_child`, `child_path`; `EngineConfinement` and `ConfinementAdapter` (generated, re-exported by `confinement` and `loom_sdk`); `TestRunner::label` and `TestRunner::local_reason` (defaulted); `confinement::local_label`; `confinement::escape_reason`; `executor::applied_label`; `ConfinementRefusal::LocalReasonMissing`; `AppliedConfinement.local_reason`; in the CLI `RunArgs.local_reason` and `Confinement::Local`. Changed — `ChildResult.driver_record` from `substrate_wire::AppliedConfinement` to `Option<substrate_wire::AppliedConfinement>`. Removed — none; `UnconfinedRunner` and `--confinement none` stay, deprecated — cited
- **Not predicted and touched:** `crates/loom-cli/tests/loom_cli.rs`, `crates/loom-intake-slice/src/confinement/engine.rs`, `crates/loom-intake-slice/src/run.rs`, `crates/loom-intake-slice/tests/confined_child.rs`, `crates/loom-intake-slice/tests/security_w20261121w1_u1.rs`, and the two security test files; `AGENTS.md`, `CHANGELOG.md`, `website/docs/reference/cli.md` and the eight `website/docs/reference/ess/intake-*.md` pages are house-rule files, the pages predicted as a group
- **Predicted and not touched:** `crates/loom-intake-slice/src/resume.rs` (inferred; the library refusal went into `intent.rs`, and `resume.rs` keeps only `Substrate` and `None`), `website/data/ess/intake-confinement.domain-graph.json` (inferred; the generated graph did not change), `website/docs/reference/ess/index.mdx` (named in the story's Scope text, not in its typed scope)
- **Review:** `review-result:confinement-opens-from-engine-config-security-1` (head `3cc79cbd`, red: three cases failed, four findings, the confinement line of `run.rs` and `executor.rs`, the resume refusal of the library, the Unicode line separators U+2028 and U+2029 in the reason, the scan for a second open point); `review-result:confinement-opens-from-engine-config-security-2` (head `37de3e91`, red: three cases failed, the first three findings answered for `EngineRunner`; open were a runner that wraps a `local` one without forwarding `local_reason`, invisible characters in the reason, and the backslash left unescaped, the subject of the last commit `281013d8`); plan-critic reviews of the story exist too (`-acceptance-c1`, `-design-c1`, `-design-edge-check`)
