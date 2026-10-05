---
format: aep.planning-md/3
id: architecture-design:effect-isolation
kind: architecture-design
status: approved
title: 'Confining Loom''s effects: Sandbox, Substrate or Mantle'
relations:
- serves: vision:O1
- informed_by: epic:effect-bindings
revision: 4
transitions:
- {from: "draft", to: "in_review", at: "2026-10-05T12:24:00Z", actor: "human:timo", revision: 2}
- {from: "in_review", to: "approved", at: "2026-10-05T12:34:20Z", actor: "human:timo", revision: 4}
---

Design only, 2026-10-05. No code, no commits. Sources were read at these `origin/main` heads: loom
`5621f4b`, sandbox `7a33b7b`, substrate `dc55907a` (tag `0.7.10`), mantle `9ab1cc8` (tag `0.2.0`),
harness `3169042f`, atlas (read only). `file:line` refers to those heads. **Verified** means read in
code or observed in a run on this machine. **Inferred** means reasoned from code without a run, and
is labelled each time.

## 1. Verdict

Confine `tests.run`, and the git operations that follow it, inside the effect adapter
(`LocalEffects` → `LocalExecutor`), not in `EffectPort`. Use Substrate's host driver embedded in the
process, the way Harness does (option A). Treat a host that cannot confine as a named refusal, not
as a fallback. Sandbox (B) is the smaller fallback if the operator wants zero setup, but it needs a
change to protect `.git` and it adds no resource bounds. Mantle (C) is about where a run is placed,
not about confining each effect. It cannot be layered on A or B as things stand.

## 2. What a model-chosen action can do today

Loom has three performed actions: `repository.inspect`, `repository.edit` and `tests.run`
(`crates/loom-intake-slice/src/executor.rs:78-83`, `effect.rs:190-193`). The model chooses the action
and its arguments. The operator chooses the test command (`crates/loom-cli/src/lib.rs:47-50`,
default `cargo test`).

| # | Path | What runs, with what rights | Source | Status |
|---|---|---|---|---|
| T1 | `tests.run` runs the test command on the model-edited tree | `Command::new(&test.program)` in the workspace; it removes only six `GIT_*` variables, so the operator's full environment, `HOME`, uid, filesystem and network are inherited | `executor.rs:638-653`, `case.rs:182-189` | verified (code) |
| T2 | model code reached through T1: `build.rs`, proc macros, test bodies, `.cargo/config.toml` (`runner`, `rustc-wrapper`), `rust-toolchain.toml` | arbitrary code as the operator. The model writes these files through `repository.edit`, which refuses only `.git`, symlinks, `..`, absolute and git-ignored paths | `executor.rs:23-32`, `executor.rs:817-860` | verified (code); the `.cargo/config.toml` route is inferred |
| T3 | credential theft | the model credential is the operator's Codex login, `~/.codex/auth.json`, read by llm in the same uid (loom `AGENTS.md` § Intake and the command line). A T2 process can read it, `~/.ssh` and other repositories, and send them out over the network | loom `AGENTS.md`, `README.md:61-63` | inferred from T1 (no run) |
| T4 | git hooks and git config | `repository.edit` runs `git add` and `git commit` on the host (`executor.rs:493`, `:502`), and `tests.run` runs `git status` (`:425`); hooks, `core.fsmonitor` and filter drivers run as the operator. A T2 process can write `.git/hooks/*` and `.git/config` because `.git` sits inside the writable work tree, and the next host-side git call then executes what it wrote | `executor.rs:909-921`, `case.rs:219-227` | verified: inside `b10x-sandbox` a command wrote `.git/hooks/pre-commit` and appended to `.git/config` (§ 7) |
| T5 | escape from the timeout | the kill takes the process group (`executor.rs:649-653`, `:697-707`); a child that calls `setsid` leaves the group and outlives the run | `executor.rs:697-707` | inferred |
| T6 | terminal injection | the child keeps the controlling terminal of `b10x-loom` (`process_group(0)` does not start a new session); with `dev.tty.legacy_tiocsti = 1` it can push keystrokes into the operator's shell | `executor.rs:649-653` | inferred |
| T7 | resource exhaustion | no memory, process or disk bound; only the 300 s timeout (`executor.rs:100`) | `executor.rs:98-118` | verified (code) |
| T8 | reads that leave through the model | `repository.inspect` returns any tracked, non-ignored file in the workspace to the model provider | `executor.rs:350-371` | verified (code); a policy question rather than an isolation one |

Loom says this itself: "There is no sandbox. `tests.run` and the workspace's git hooks run
model-edited code with the operator's rights and environment" (loom `AGENTS.md`;
`executor.rs:43-47`). `inspect` and `edit` execute no code. Their path checks are a lexical walk
plus `lstat` (`executor.rs:817-860`). Substrate's `openat2`-beneath I/O would also close the race
between that check and the write (inferred; not observed).

Where confinement belongs: `EffectPort` is domain-neutral by design. "How an action binds to an
operation, and what confines it, is the implementation's" (`crates/loom-commission/src/ports/effect.rs:13-15`).
The runtime calls `invoke` only after revalidation (`runtime.rs:525-532`). So confinement belongs
in `LocalEffects`/`LocalExecutor` (`loom-intake-slice`), and in a later Commission binding under
Atlas ADR 0082.

## 3. What each candidate actually provides

### Sandbox (`beyond10x/sandbox`, `b10x-sandbox` 0.1.0)

- **What it is.** A library and CLI that compute one `bwrap` (or `docker run`) argv and either exec
  it or hand back a `std::process::Command` (`src/confinement.rs`, `Confinement::command`). No
  daemon, no cgroups, no seccomp, no record (`src/confinement.rs:1-14`; `AGENTS.md` § What this
  repository owns).
- **Isolation, verified.** Own user, ipc, pid and uts namespaces, plus net unless `--net`;
  `--disable-userns`; `--die-with-parent`; only `/usr`, `/bin`, `/lib`, `/lib64` read-only;
  `--clearenv` with `HOME=/tmp`; the workspace writable (`src/confinement.rs`
  `bubblewrap_argv`). The tests that hold this: `tests/substrate_mirror.rs` (oracle transcribed
  from Substrate), `tests/bubblewrap.rs` (an undeclared sibling is absent; `--ro` is not writable),
  `tests/backend_refusal.rs` (a missing `bwrap` is refused and the command never runs). On this
  machine the suite ran green with no absent case (`task check`, exit 0).
- **Not provided.** Resource bounds, seccomp, egress filtering beyond all or nothing, a read-only
  `.git` inside a writable `--dir` (§ 7: the hooks and config were writable), and the
  `legacy_tiocsti` check on `command()` (only `run()` makes it; `src/confinement.rs` `run`).
- **API.** `Layout::plan(cwd, dirs)`, `Options { network, read_only_roots, env, … }`,
  `Confinement::new(..).command()`. Run on this machine against Git revision `7a33b7b`
  (embed-the-library guide, `website/docs/guides/embed-the-library.md` in the docs worktree).
- **Maturity.** Seven commits, no tag, workspace version 0.1.0; CI `Gate` green on `7a33b7b`
  (2026-09-18) and it installs bubblewrap, so the confinement cases execute in CI. One draft story.
  One consumer, the Atlas O6 loop. Atlas ADR 0052 Decision 1 admits it "as a tool derived from
  Substrate … not as a competitor to the seam Atlas already assigns to Substrate".

### Substrate (`beyond10x/substrate`, 0.7.10)

- **What it is.** An execution data plane with confined workspaces, bounded processes and a durable
  operation ledger. It ships a daemon, a Rust SDK (`b10x-substrate-sdk`: `Client`,
  `ManagedDaemon`), an MCP test adapter and the host driver (`substrate-host`).
- **Isolation, verified in code.** The bubblewrap set with `--new-session` and a `--seccomp`
  socket-family filter (`crates/substrate-host/src/process.rs:64`, `:1936-1962`); cgroup v2
  pids, memory+swap and observed CPU; whole-tree kill; timeout; cleared environment; no egress,
  with destination-bound egress apertures; read-only declared host roots (substrate ADR 0010);
  **explicit workspace write access**: read-only, or scoped writable sub-directories, with the
  workspace mounted read-only first (substrate ADR 0023). Its invariant 3 says a missing guarantee
  is a named refusal; for example, with no delegated cgroup root, exec is refused as unavailable
  (`process.rs:656-658`).
- **Tests.** `crates/substrate-daemon/tests/runtime_vectors.rs:929-1250` (clean environment, no
  egress, pids, memory, timeout, tree kill). They run only in the delegated lane
  (`scripts/delegated-lane.sh`, which uses `systemd-run --user -p Delegate=yes --scope`). CI runners
  have neither bubblewrap nor delegation, so the release workflow requires a recorded local lane
  run for the tagged commit (`STATUS.md`, CI row; that page is dated 2026-09-15, older than
  0.7.10).
- **Prerequisite on this machine.** The user manager delegates `cpu memory pids`
  (`/sys/fs/cgroup/user.slice/user-<uid>.slice/user@<uid>.service/cgroup.controllers`, read
  2026-10-05). So the delegated scope that exec needs is available here (verified, read only; no
  exec run).
- **Maturity.** 0.7.10 released 2026-10-02 and keyless-signed; wire bundles frozen to 0.17.0; 52
  stories implemented, 19 draft and 10 proposed; CI and docs runs green 2026-10-04. Its wire
  contract is still development, not stable (`STATUS.md`).
- **Boundary rule.** Substrate's README says cross-component consumers use the daemon and the wire
  contract, not the implementation crate. Harness crosses that rule on purpose and argues it in
  `crates/harness-substrate/src/embedded.rs:1-27`; Mantle follows it (`b10x-substrate-sdk` only,
  mantle `AGENTS.md` § Rules).

### Mantle (`beyond10x/mantle`, 0.2.0)

- **What it is.** It places a whole agent session (agent, workspace, toolchain) on a remote Linux
  worker (EC2 through SSM, or KubeVirt), confined by Substrate, behind a CONNECT egress proxy with
  a fixed allowlist (`crates/mantle-egress`). It talks to Substrate through the SDK.
- **What it confines.** The session as a whole. It does not confine each effect within it. The
  agent kinds are a closed set, `claude-code` and `codex` (`crates/mantle/src/domain/session.rs:29-32`).
  The sandbox refuses AF_UNIX sockets (mantle `AGENTS.md` § Layout, `mantle-launch`).
- **Maturity.** 0.2.0 released 2026-10-03; README: "first vertical slice, under construction";
  authenticated lifecycle qualification incomplete. 14 stories implemented, 5 active, 3 draft.

### Harness precedent

`b10x-harness-substrate` pins `b10x-substrate-host` and `b10x-substrate-wire` at one Git revision
with exact versions (`crates/harness-substrate/Cargo.toml`). It opens `HostDriver` in process.
Exec is served only with a delegated cgroup root (`embedded.rs:109-111`). It adopts an existing
directory as the workspace without copying (`embedded.rs:184-250`; the name must be one component
of `[A-Za-z0-9_-]`). It clears the exec environment ("an exec that saw this process's environment
would carry a credential into a confined workspace", `embedded.rs:430-437`). It mounts a declared
toolchain read-only (`src/toolchain.rs`). What it gives up, in its own words: the daemon's
peer-credential subject, i.e. "who asked" (`embedded.rs:5-14`). Harness `AGENTS.md` invariant 2
pins this crossing as a foundation dependency, never a sibling path.

## 4. Options

Every option keeps the Loom rule "fail toward less authority, less effect" (loom `AGENTS.md`
§ Rules) and Substrate invariant 3: a run that asked for confinement and cannot have it stops with
a named reason.

### A. Embed Substrate's host driver, as Harness does (recommended)

- **Plugs in.** A `TestRunner` (process-confinement) seam inside `loom-intake-slice`, replacing
  `run_bounded` (`executor.rs:638-694`). `LocalExecutor::new` takes it. `LocalEffects`,
  `EffectPort` and the runtime are unchanged. The CLI gains
  `b10x-loom run --confinement substrate|none` and the cgroup root (or Loom re-execs itself under
  `systemd-run --user -p Delegate=yes --scope`, as `delegated-lane.sh` does). `--confinement none`
  is explicit and printed on every step. The SDK re-exports the seam.
- **Changes.**
  1. `HostDriver` rooted at the workspace's parent; adopt the work tree (Harness `workspace_adopt`).
  2. `tests.run` becomes an exec with workspace access **scoped write** to the build output only
     (`target/`, created first, because ADR 0023 scopes must exist). The source tree and `.git`
     are read-only during the test run. That closes the T4 write path and stops tests from
     altering the revision they report on.
  3. No network; the toolchain is mounted as read-only roots (`~/.rustup`, a pre-fetched
     `~/.cargo` registry) with the env that names them, Harness `Toolchain` style.
  4. The `tests.run` observation payload (`executor.rs:436-454`) gains the driver's applied
     confinement record. The governor then sees under what confinement the evidence was produced.
  5. Host git calls get `-c core.hooksPath=…` per § 6 Q3 and `-c core.fsmonitor=false`
     (`executor.rs:909-921`, `case.rs:219-227`).
- **Closes.** T1–T7: environment, credential, filesystem, network, setsid escape (pid namespace and
  cgroup kill), `--new-session`, pids and memory bounds. T8 is untouched.
- **Cost (inferred).** Three or four Loom stories. Two Git dependencies at a pinned revision plus
  `tokio` in `loom-intake-slice`. A host prerequisite (bubblewrap and a delegated cgroup). Workspace
  directory names restricted to `[A-Za-z0-9_-]`, so `my.repo` must be renamed or symlinked (a
  symlinked root is refused; canonicalise). A capsule directory under `$XDG_STATE_HOME`.
- **Does not protect.** "Who asked": there is no daemon subject, which is acceptable on a
  single-operator machine (`embedded.rs:11-14`). Host-side git still runs as the operator. Reads
  that leave through the model (T8). The model credential inside `b10x-loom` itself (Loom is not
  confined; only its effects are).
- **Maturity risk.** Medium. Substrate is released and signed, but its contract is development and
  the confinement lane is not run in CI. Embedding crosses Substrate's own consumer rule. Pin
  `=0.7.x` at a revision, and move it on purpose (Harness pins 0.7.8 at `0569597`).

### A′. Substrate through its SDK (`b10x-substrate-sdk`, `ManagedDaemon`)

- Same seam as A, but over the daemon's wire. It keeps the peer-credential subject and follows
  Substrate's rule, like Mantle.
- **Blocker, verified in code.** The SDK creates workspaces only `empty` or from a Git source that
  needs a source authorization (`crates/b10x-substrate-sdk/src/lib.rs:552-575`). Nothing adopts an
  existing host directory. Loom would copy the tree in and the diff out, or Substrate would gain
  an "adopt host directory" source. Whether that is product-neutral is Substrate's decision.
- **Cost.** A plus a running (or linked) daemon, plus the copy or a Substrate change.

### B. Run effects through Sandbox (`b10x_sandbox` library)

- **Plugs in.** The same `TestRunner` seam, implemented as `Confinement::new(Layout::plan(root,
  &[]), Options { network: false, read_only_roots, env }, argv).command()`. Loom keeps its pipes,
  timeout and kill around the returned `Command`. CLI: `--confinement sandbox`.
- **Changes.** Loom: the seam and flag, `setsid` or the `legacy_tiocsti` check (because
  `command()` skips it), and observation facts (the argv digest, as the Atlas O6 loop records it).
  Sandbox: a read-only overlay for `.git` inside a `--dir` (`--ro-bind <ws>/.git /workspace/.git`
  after the bind). Its invariant 4 makes any added argument a design change recorded in its
  planning store.
- **Closes.** T1–T3 (environment, home, credential, network), T5 (pid namespace and
  `--die-with-parent`), and T4 once the `.git` overlay exists.
- **Cost.** Smallest: one Sandbox story, two Loom stories (inferred). It needs only `bwrap`.
- **Does not protect.** T7 (no cgroups), no seccomp, no egress allow-list, tests can still rewrite
  the source tree they report on, and there is no applied-confinement record beyond the argv.
- **Maturity risk.** High as a dependency: unreleased, no tag, one consumer. Architecturally, Atlas
  ADR 0052 places the confinement seam with Substrate, and governed-autonomy `invariants.md:16`
  says "Substrate owns confined effects". Adopting B in Loom would need the operator to accept a
  second confinement path.

### C. Mantle

- **Plugs in.** Nowhere inside Loom. `b10x-loom run` becomes the session a Mantle manifest starts on
  a remote worker.
- **Changes.** A Mantle agent kind for Loom: the closed set is in `session.rs:29-32` and
  `mantle-worker::AgentKind`. Loom's Codex login would live in the session's workspace home, as
  Codex's does.
- **Closes.** The operator's laptop is out of reach: placement, egress allow-list, Substrate around
  the whole session.
- **Does not protect.** Effects from Loom inside the session: the test process shares the
  session's confinement with Loom and its credential, so T3 holds inside the worker. Remote cost
  and latency, and AWS or KubeVirt.
- **Maturity risk.** High: 0.2.0, "under construction".

### Combinations

- **C with A or B nested: not viable as things stand.** Substrate passes `--disable-userns`, so a
  bubblewrap inside cannot create its namespace. Verified: inside `b10x-sandbox`, a nested `bwrap
  --unshare-user` failed with "Creating new namespace failed … (ENOSPC)". Mantle's sandbox also
  refuses AF_UNIX, so an in-session Loom cannot reach a local Substrate socket. Per-effect
  confinement inside Mantle needs a Substrate primitive (a child exec from inside a session), which
  is a Substrate decision.
- **A with B as a fallback: rejected.** A silent fallback to the weaker profile is the degradation
  Substrate invariant 3 forbids. An explicit, operator-chosen `--confinement sandbox` is possible,
  but it is the operator's call (§ 6 Q5).
- **0. Interim, no code.** Running all of `b10x-loom` under `b10x-sandbox --net --ro ~/.codex …`
  protects the rest of the filesystem. It still gives tests the network and a readable credential,
  and it breaks llm's token renewal on a read-only `~/.codex` (inferred). It is not recommended
  except as a stopgap.

### Comparison

| | A Substrate embedded | A′ Substrate SDK | B Sandbox | C Mantle |
|---|---|---|---|---|
| env, home and credential hidden from tests | yes | yes | yes | no (inside the session) |
| network off; allow-list | yes; apertures | yes; apertures | yes; all or nothing | allow-list via proxy |
| `.git` and source read-only during tests | yes (ADR 0023 scoped write) | yes | needs a Sandbox change | no |
| process, memory and CPU bounds | yes | yes | no | yes (session) |
| seccomp | socket-family filter | yes | no | yes (session) |
| setsid escape, terminal | closed | closed | closed; terminal needs `setsid` | closed |
| applied-confinement record | driver record | daemon ledger | argv only | Substrate (session) |
| host prerequisite | bwrap and a delegated cgroup | the same plus a daemon | bwrap | a remote worker |
| Loom stories (inferred) | 3–4 | 4–5 plus a Substrate change | 2 plus 1 in Sandbox | Mantle change plus packaging |
| maturity | 0.7.10, signed | same | 0.1.0, untagged | 0.2.0 |

## 5. Recommendation

1. **A now**, for `tests.run`: the embedded Substrate host driver behind a `TestRunner` seam in
   `loom-intake-slice`. Scoped write to `target/` only, no network, a read-only toolchain, and the
   applied confinement in the observation. `--confinement none` stays available, explicit and
   loud.
2. **Harden host git in the same change** (hooks per Q3, `core.fsmonitor=false`). It costs nothing
   and closes T4 for any option.
3. **Move to A′ or to Commission's binding later.** When Commission implements ADR 0082's invocation
   ("Connectors, inside Substrate"), the seam's Substrate implementation becomes that binding's
   containment. Mantle stays a placement question for later.

## 6. ESS nouns a later story needs

Loom's rule is that a new noun gets its ESS domain before a story is written around it (loom
`AGENTS.md` § ESS). Proposed home: a new domain `intake.confinement` in `ess/intake/`, beside
`intake.routing` (or `loom.confinement` in `ess/` if the seam leaves the slice).

| Noun | Kind | Fields (sketch) | Relation |
|---|---|---|---|
| `ConfinementProfile` | struct | backend (`substrate`, `none`), network (`none`, `apertures`), writable scopes, read-only roots, environment set, timeout, memory and pids limits | requested by a `SliceRun` |
| `ToolchainRoot` | struct | host path, mount point, environment it sets | many per profile |
| `AppliedConfinement` | struct | backend, profile digest, driver-reported applied record (mode, scopes, roots, network, limits) | one per `tests.run` observation |
| `ConfinementRefusal` | enum | `backend-missing`, `cgroup-undelegated`, `scope-invalid`, `workspace-name-invalid`, `capability-unserved` | ends a step as `EffectOutcomeRefused`, or the run |
| `StopReason::ConfinementUnavailable` | enum variant | — | extends `intake.routing.StopReason` |

Commission's `EffectOutcome` stays domain-neutral (`ess/commission/domains/responsibility.yaml:238-258`).
The applied record travels in the observation payload, not in Commission's types.

## 7. Observations made for this design (this machine, 2026-10-05)

| Check | Result |
|---|---|
| `b10x-sandbox -- sh -c 'echo "#!/bin/sh" > .git/hooks/pre-commit; echo "[core]" >> .git/config'` in a git work tree | both writes succeeded: T4 holds under B as it is today |
| `b10x-sandbox -- bwrap --unshare-user --ro-bind /usr /usr --symlink usr/bin /bin true` | `Creating new namespace failed: nesting depth … exceeded (ENOSPC)`, exit 1: nesting is refused |
| `b10x-sandbox -- ls /dev` | `tty` is present: T6 needs `setsid` or the sysctl check under B |
| delegated cgroup controllers of the user manager | `cpu memory pids`: A's prerequisite is available here (read, not exercised) |
| sandbox `task check` (docs worktree) | exit 0, no absent confinement case |

Not run: any Loom run, any Substrate exec, any Mantle session.

## 8. Decisions (operator, 2026-10-05)

The eight open questions were put to the operator in an interview on 2026-10-05; each answer below
is the option the operator chose.

| # | Question | Decision |
|---|---|---|
| 1 | The crossing | **Embed like Harness**: Loom depends on `substrate-host` and `substrate-wire` at a pinned release, as `crates/harness-substrate` does; Loom keeps the authority over what runs. |
| 2 | Default posture | **Refuse, opt out by flag**: `tests.run` stops with a named `ConfinementRefusal` when confinement is unavailable, unless the run passes `--confinement none`, which the run record names. |
| 3 | Git hooks | **Disable hooks on Loom's own git calls**: `core.hooksPath` set to an empty directory and `core.fsmonitor=false`. Gates still run when the operator or the bot pushes. |
| 4 | Network for tests | **None, prefetch first**: dependencies are fetched on the host before the run and mounted read-only; confined tests get no network. |
| 5 | Sandbox in Loom | **No**: Substrate is the only backend (`--confinement substrate` or `none`). |
| 6 | Delegation | **Loom re-execs itself** under `systemd-run --user -p Delegate=yes --scope` when no delegated cgroup is present. |
| 7 | Protocol | **Not yet, record only**: every `tests.run` observation carries the `AppliedConfinement` record; `software-change@1` does not require it until confinement is the default and proven. |
| 8 | Mantle | **Later**: remote placement of whole Loom runs is filed as a draft story, outside this design's work. |

## Stories this design leads to

- `story:confined-tests-run`: the `intake.confinement` ESS domain (§ 6), `tests.run` confined by the
  embedded Substrate host driver with no network and writes only to `target/`, the refusal by
  default, `--confinement none`, the `systemd-run` re-exec, and `AppliedConfinement` in the
  observation (decisions 1, 2, 4, 5, 6, 7).
- `story:host-git-hardening`: Loom's host-side git calls run with hooks and fsmonitor disabled
  (decision 3).
- `story:mantle-placement`: draft only (decision 8).
