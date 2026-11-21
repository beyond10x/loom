---
format: aep.planning-md/3
id: story:bound-program-declares-its-environment
kind: story
status: draft
title: A bound program declares its environment, read-only roots and writable directories
summary: A program executor in loom.bindings/1 gains environment, read_only_roots and writable; credential and loader names, unsafe roots and paths leaving the workspace are refused; only the operator widens reach
tags:
- sdc
relations:
- depends_on: story:bound-program-result-table
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
revision: 2
---
## Outcome

A `program` executor in a `loom.bindings/1` document may declare three members. The effect port
of `story:bound-program-result-table` passes them to the public function of
`crates/loom-intake-slice` that builds a bound program's `ChildCommand`, which gains them as
parameters, and the port starts that child through `run_child` of the admitted runner, as that
story does:

- `environment`: variables by name and literal value. The program's environment is these
  variables and `PATH`; `PATH` keeps the host's value unless the binding declares it, and then
  the declared value replaces it. No other variable reaches the program.
- `read_only_roots`: host directories by absolute path. Under `linux` each is mounted read-only
  at its own host path, so a path in `environment` or `argv` names the same directory under both
  adapters.
- `writable`: directories of the workspace by relative path. Under `linux` the program writes
  there beside `target/`, which stays writable as today; the rest of the workspace stays
  read-only.

What the program sees:

- Under `linux`: exactly the declared variables and `PATH`; the system view, `/proc`, `/dev`, a
  private `/tmp` and the workspace at `/workspace`; each declared root, read-only; writes under
  `target/` and the declared directories alone; no network. The applied confinement names exactly
  the roots and writable paths requested, or the result is refused, as today.
- Under `local`: exactly the declared variables and `PATH`, checked by Substrate's local adapter;
  the host's file system and network as the operator. The build function adds no root when the
  runner's `local_reason` is set, as it adds no program mount then, since a `local` runner
  refuses any read-only root; the program already sees them. The writable
  declarations confine nothing there: the workspace is opened read-write. The load and start
  checks below still run, so a document has one verdict under both adapters.

A program under a declared root (for example the toolchain's own `cargo`) is not mounted again at
its own path: the build function skips the program mount for it, as it skips a program under the
system view today.

Refused at load, by `decode` or `check`, naming the action and the member (new
`BindingsRefusalReason` variants unless stated):

- `InvalidEnvironment`: `environment` that is not a mapping of string to string; a name that is
  not an upper-case ASCII letter followed by upper-case letters, digits and `_`, or is longer than
  128 bytes; a value longer than 4096 bytes or holding NUL; more than 64 variables with `PATH`
  counted. This is Substrate's environment shape, checked at load.
- `EnvironmentNameRefused`: a name that holds `AUTHORIZATION`, `BEARER`, `CREDENTIAL`,
  `PASSWORD`, `PROXY`, `SECRET` or `TOKEN` (Substrate's fragments); a name with `KEY` as one of its
  `_`-separated words (`ANTHROPIC_API_KEY`, `AWS_ACCESS_KEY_ID`); a name starting `LD_` or
  `DYLD_` (the dynamic loader's).
- `InvalidReadOnlyRoot`: a root that is not UTF-8, not absolute (`~` is not expanded), or not
  canonical (a `.`, `..` or empty component, a trailing `/`, more than 64 components, `/`
  itself); a root on, under or holding a mount Substrate owns (`/usr`, `/bin`, `/lib`, `/lib64`,
  `/etc/alternatives`, `/proc`, `/dev`, `/tmp`, `/workspace`, `/runtime`, `/scratch`,
  `/etc/hosts`, the certificate bundle path); two roots where one equals or lies under the other;
  more than 4 roots.
- `PathNotInWorkspace` (existing): a `writable` path that is empty, absolute, holds NUL or has a
  `..` segment.
- `InvalidWritable`: a `writable` path with a `.` or empty component or a trailing `/`; `.git`
  or a path under it; a path under `target`; two paths where one lies under the other; more than
  63 paths.
- `MemberNotForExecutor` (existing): any of the three on an `agent`, `person`, `open` or `copy`
  executor.

Refused at the start of the run, before the program starts, with no record and no blob: the
step is refused `ReachRefused`, its detail naming the path and the rule. The port checks, under
both adapters:

- a root that does not exist, is no directory, or is reached through a symbolic link in any
  component;
- a root that is, holds or lies inside the workspace, or the store directory the port writes
  blobs to;
- a root that is the host's home directory or holds it, or is, holds or lies inside one of
  `~/.ssh`, `~/.gnupg`, `~/.aws`, `~/.azure`, `~/.kube`, `~/.docker`, `~/.config/gh`,
  `~/.config/gcloud`, `~/.netrc`, `~/.git-credentials`, `~/.npmrc`, `~/.pypirc`,
  `~/.cargo/credentials` and `~/.cargo/credentials.toml`;
- a `writable` path that does not exist in the workspace, is no directory, or passes through a
  symbolic link.

Under `linux` the runner then applies its own checks again and adds two the port does not make:
a writable directory holding a socket, device or FIFO, or a file hard-linked outside the writable
paths. Its refusal is the runner's `ScopeInvalid`, and the step is refused naming it.

Only the operator widens a program's reach. The bindings document is host context written by the
operator: `ess/domains/bindings.yaml` says so at its head ("the one file in which an operator
declares", lines 1-2) and its summary gains the sentence that the document is host context, never
model output, as `ess/intake/domains/bindings.yaml:26-27` says of the intake file
("Configuration is trusted host context, never model output"). An action's arguments go to the
program's standard input alone and never name an environment variable, a root or a writable
path. Environment values are not secret: a name that carries a credential is refused, and no
value is written to a record or a step line by this story.

## Why

The SDC wants a work node to run the product tree's own test command (`cargo test`, `task
check`, `npm test`) as a bound program, confined, and turn its exit code into evidence. That
cannot run today:

- A bound program's environment is `PATH` alone (`crates/loom-intake-slice/src/bindings.rs:72`,
  `:329-349`), and `story:bound-program-result-table` keeps that: its public build function sets
  "an environment that holds `PATH` alone at the host's value" (its Outcome), pinned by
  `a_bound_program_starts_with_path_alone_in_its_environment` (its Acceptance). `HOME`,
  `CARGO_HOME` and `RUSTUP_HOME` are absent.
- Under `linux` a child sees the system view, `/proc`, `/dev`, a private `/tmp`, its workspace
  and only the roots the host names (`crates/loom-intake-slice/src/confinement.rs:1157-1164`);
  "no toolchain or cache is mounted" (`:1144-1145`). The bound path names no root beyond the
  program file (`bindings.rs:350-356`), so `~/.rustup`, `~/.cargo/bin` and the registry are
  absent.
- It writes only `target/` (`confinement.rs:1846-1852`); the bound path names no other writable
  directory.

The runner serves all three already: `ChildCommand::with_environment`, `with_read_only_root` and
`with_writable` (`confinement.rs:1328-1338`, `:1239-1274`, `:1308-1326`), each refused
`ScopeInvalid` before anything starts when out of shape. What is missing is a place where the
operator declares them per action. `SubstrateRunner::run` (`confinement.rs:1109-1126`) shows the
shape for Rust alone: `RustClosure` (`:3257-3365`) mounts the installed toolchain and only the
`registry`, `git/db` and `git/checkouts` parts of the cargo home (`:75`, `:3306-3339`), never the
cargo home itself, and sets `PATH`, `HOME=/tmp`, a private `CARGO_HOME`, `CARGO_NET_OFFLINE=true`
(`:3345-3359`). This story lets a binding state the same for any toolchain.

Which document. The `loom.bindings/1` document gains the three members, in `ExecutorSpec`, where
each member belongs to one executor kind (`ess/domains/bindings.yaml:30-63`;
`crates/loom-bindings/src/load.rs:228-254`). The intake run configuration file stays `PATH`-only:

- it binds only a protocol that declares the `intent` artifact alone and no case input
  (`bindings.rs:183-202`), and the SDC's protocols declare several artifacts;
- the program port of `story:bound-program-result-table` lives in `crates/loom-bindings`, which
  reads the `loom.bindings/1` document, and builds each `ChildCommand` through the public build
  function of `crates/loom-intake-slice` that `CommandRunner` also uses;
- the intake decoder refuses an `env` member and a test pins it (`bindings.rs:711-714`), and
  `AGENTS.md` § Bindings document says `intake.bindings` is not changed by the new format.

Where the declarations enter. The build function of `story:bound-program-result-table` gains
the three declarations as parameters, and the port passes the decoded `ExecutorSpec` members.
Chaining onto the returned `ChildCommand` is possible in part: `with_read_only_root` and
`with_writable` are public and append (`confinement.rs:1264-1274`, `:1323-1326`), and
`with_environment` is public and replaces the whole environment with the one it is given
(`:1328-1338`, "exactly `environment` and no other variable"), so the port would read
`environment()` (`:1471`), merge, and set the result. Chaining cannot skip the program mount:
`at_host_path` adds a host path (`:1303-1306`), no public method removes one, and a file mount
under a root mount is refused (`check_mounts` step 4, `:2392`). Inside the build function the
environment is merged before `with_environment` is called once, and the program mount is
skipped for a program under a declared root. The intake path keeps `PATH` alone after this story:
`CommandRunner` passes no declarations to the shared function, and
`a_bound_program_receives_the_arguments_alone_on_stdin_and_path_alone_in_its_environment`
(`crates/loom-intake-slice/tests/bound_command_confined.rs:134-162`) pins it today.

Why each rule:

- The credential fragments are Substrate's (pinned at `crates/loom-intake-slice/Cargo.toml:31-32`;
  Substrate's `crates/substrate-guarded/src/environment.rs:19-57`, which its `linux` driver and
  its `local` adapter both apply). Checked at load, a document is refused before any model
  call. Substrate's list misses `ANTHROPIC_API_KEY` and
  `OPENAI_API_KEY`, which the session executor treats as credentials
  (`crates/loom-session-executor/tests/endpoint_secret.rs:253`); the `KEY` word rule covers them.
  A credential reaches a confined program only through Substrate's secret slots, and Loom
  requests none (`confinement.rs:1693`).
- `LD_` and `DYLD_`: the program's `argv` is what the operator reviewed; a loader variable makes
  every process it starts load other code, and a writable or workspace path in
  `LD_LIBRARY_PATH` would load files the model wrote. Substrate refuses `DYLD_` on macOS only.
- Root text rules are `canonical_text` (`confinement.rs:1750-1773`) and `check_mounts` steps 1, 3
  and 4 (`:2386-2392`), with the owned mounts of `OWNED_MOUNTS` (`:2341-2356`), moved to load
  because a root is mounted at its own path. The root bound of 4 is Substrate's
  (`ChildCommand::MAX_READ_ONLY_ROOTS`, `confinement.rs:1231-1233`).
- Root disk rules repeat `with_read_only_root` (`confinement.rs:1242-1259`) in the port so that a
  `local` run, which takes no roots (`crates/loom-intake-slice/src/confinement/engine.rs:343-351`),
  gets the same verdict. The credential locations are new: a root is read by a program whose
  input the model chose, and its report and blob reach the case and the next model call.
- Writable rules are `check_writable` (`confinement.rs:1860-1908`) and
  `check_writable_directory` (`:3164-3190`), moved to load and start. `.git` is refused because
  "a child that may write `.git` may also plant hooks or configuration that a later git call on
  the host runs" (`confinement.rs:1310-1312`).
- Under `local` the runner checks writable paths as text only (`engine.rs:352`) and opens the
  workspace read-write (`engine.rs:308-309`); `local` runs as the operator with the host's file
  system and network (`engine.rs:10-13`). The story says so and promises no confinement there.
- Who may widen: `AGENTS.md` § Rules, "The model is not trusted context", and "fail toward less
  authority". If an action's arguments could name a root, the model could mount `~/.ssh` and read
  it back through the program's output. `IntentRequest.bindings` is "Trusted host context, never a
  model's" (`crates/loom-intake-slice/src/intent.rs:65-69`).

Open questions for the planner (the draft takes the first option of each):

1. Reach as binding data or engine data: binding alone; engine lists allowed roots and a binding
   picks; engine alone (`ess/domains/bindings.yaml:7-8` makes an aperture engine data).
2. Paths in roots and values: absolute literals only; `~` and `$HOME` expanded at load; a
   `{workspace}` token in values (the workspace is `/workspace` under `linux`, the host path under
   `local`, `confinement.rs:1708`).
3. `arguments_from` program: `PATH` alone; the same three declarations as the executor's program.
4. An absent writable directory: refused `ReachRefused`; created by the port before the start.
5. Credential locations: a fixed list in `crates/loom-bindings`; a list in engine configuration;
   every root under the home directory refused unless the engine allows it.
6. Root mount point: its own host path; a mount the binding names under `/opt`.
7. The `KEY` word rule: kept; Substrate's seven fragments alone.
8. Where the declarations enter: the build function gains them as parameters; the port chains
   them onto the returned `ChildCommand` and refuses at load a program at an absolute path under
   a declared root, whose mount would overlap the root.

## Waits on

- `story:bound-program-result-table`: the program port, the public function of
  `crates/loom-intake-slice` that builds its `ChildCommand`, the result table that records the
  exit code, and the blob that keeps the program's output. Both stories change
  `ess/domains/bindings.yaml`, `crates/loom-bindings` and
  `crates/loom-intake-slice/src/bindings.rs`.

No other story is needed. Same files, ordered by the wave planner: `story:bound-artifact-revisions`,
`story:bound-agent-node`, `story:capacity-given-to-an-admission-program` and
`story:gate-program-identity-bound-to-spawn` change `ess/domains/bindings.yaml` and
`crates/loom-bindings`. The last mounts the program file read-only under
`/opt/loom/programs/<n>/`; if it lands first, a declared root that equals, holds or lies under
`/opt/loom/programs` is refused `InvalidReadOnlyRoot`. Substrate 0.21.0 is pinned
(`crates/loom-intake-slice/Cargo.toml:31-32`) and serves read-only roots and writable subtrees.

## ESS first

`ess/domains/bindings.yaml` first: `loom.bindings.EnvironmentVariable` (`name`, `value`);
`ExecutorSpec` gains `environment` (`Optional<List<loom.bindings.EnvironmentVariable>>`),
`read_only_roots` and `writable` (`Optional<List<String>>`), each a `Program` member;
`BindingsRefusalReason` gains `InvalidEnvironment`, `EnvironmentNameRefused`,
`InvalidReadOnlyRoot` and `InvalidWritable`; the run refusal `ReachRefused` is declared beside
the refusals `story:bound-program-result-table` adds; the summary gains the sentence that the
document is host context written by the operator, never model output, and that an action's
arguments never widen a program's reach. On that commit `task drift` fails (`generated/rust/loom/`
is stale), `every_refusal_reason_is_reachable_from_a_document`
(`crates/loom-bindings/tests/bindings_document.rs:769`) does not compile, and the story's new
test `crates/loom-bindings/tests/bound_program_environment.rs` does not compile.

## Acceptance

Each in `crates/loom-bindings/tests/bound_program_environment.rs` unless named, with a fixture
protocol and no model and no network:

- End to end, `linux`: a fixture crate with no dependencies and a committed `Cargo.lock`
  (`crates/loom-bindings/tests/fixtures/environment/adder/`) is copied into a workspace under
  `CARGO_TARGET_TMPDIR` with an empty `build/`. The binding runs `<sysroot>/bin/cargo test
  --offline --locked`, where the test reads `<sysroot>` from `rustc --print sysroot`; it declares
  that sysroot as its one root, `writable: [build]` and `PATH=<sysroot>/bin:/usr/bin:/bin`,
  `HOME=/tmp`, `CARGO_HOME=/tmp/cargo`, `CARGO_TARGET_DIR=build`, `CARGO_NET_OFFLINE=true`. With
  `results: {"0": passed, "*": failed}` it records `passed`; `build/debug` exists after the run;
  the applied confinement names the root and `build`; network none. The same with a failing
  `#[test]` records `failed`. The case runs in a delegated scope or skips naming what is missing,
  as `crates/loom-intake-slice/tests/bound_command_confined.rs:387-390` does; a skip shows
  nothing, and the wave's evidence names a run that printed `ran delegated`.
- `linux`: a program that reads a host file outside its workspace and its declared roots fails;
  one that writes outside `target/` and its declared directories fails.
- `local`: a program running `/usr/bin/env` leaves a blob that holds exactly the declared
  variables and `PATH`; with declared roots the run is not refused `CapabilityUnserved`.
- With no `environment`, the program's environment is `PATH` alone, as in
  `story:bound-program-result-table`.
- A request whose arguments hold `environment`, `read_only_roots` and `writable` members changes
  nothing in the program's environment, mounts or writable paths; the members reach its standard
  input alone.
- Load refusals, one case each, with the reason, action and member: a lower-case name, a
  4097-byte value, 65 variables (`InvalidEnvironment`); `GITHUB_TOKEN`, `ANTHROPIC_API_KEY`,
  `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES` (`EnvironmentNameRefused`); `~/.rustup`, `opt/x`,
  `/opt/a/../b`, `/`, `/usr/local`, `/tmp/x`, `/opt/a` beside `/opt/a/b`, five roots
  (`InvalidReadOnlyRoot`); `/abs`, `../x` (`PathNotInWorkspace`); `./build`, `build/`, `.git`,
  `.git/hooks`, `target/x`, `build` beside `build/x`, 64 paths (`InvalidWritable`);
  `environment` on an `agent` executor (`MemberNotForExecutor`).
- Every name Substrate's environment rule refuses for a fragment is refused at load: a case
  builds one name per fragment and checks each is `EnvironmentNameRefused`.
- Start refusals, each `ReachRefused` under both adapters, the program not started, no record
  and no blob: an absent root; a root that is a file; a root through a symbolic link; a root that
  holds the workspace; a root holding the store directory; with `HOME` set for the test process
  to a fixture home under `CARGO_TARGET_TMPDIR` holding an empty `.ssh/`, the root `<home>`,
  `<home>/.ssh` and `<home>/.ssh/x`; an absent writable directory; a writable path through a
  symbolic link.
- `linux`: a writable directory holding a FIFO is refused by the runner, `ScopeInvalid`, and the
  step is refused naming it.
- `every_refusal_reason_is_reachable_from_a_document`
  (`crates/loom-bindings/tests/bindings_document.rs`) reaches the four new reasons, and the
  complete file on `website/docs/guides/bindings.md` holds the three members and decodes.
- The intake path keeps `PATH` alone and its decoder still refuses `env`:
  `a_bound_program_receives_the_arguments_alone_on_stdin_and_path_alone_in_its_environment`
  (`crates/loom-intake-slice/tests/bound_command_confined.rs`) and
  `a_file_with_an_undeclared_member_is_refused_naming_it`
  (`crates/loom-intake-slice/src/bindings.rs`) pass unchanged.
- `task check` exits 0.

## Out of scope

Egress apertures for a bound program. A `timeout_seconds` above 3600. Returning the program's
exit code or output to an SDK host as a value: an SDK host does that today through
`EngineRunner::run_child` and `SubstrateRunner::run_child` (`confinement.rs:593-627`, re-exported
at `crates/loom-sdk/src/lib.rs:101` and `:120-123`). The intake run configuration file
(`intake.bindings`). Credentials for a bound program (Substrate's secret slots). A toolchain
shortcut that discovers roots, as `RustClosure` does for `TestRunner::run`.
