---
format: aep.planning-md/3
id: story:run-cgroup-root-holds-the-root
kind: story
status: draft
title: b10x-loom run holds the cgroup root it is given, so run and tree run never share a root
summary: run/resume --cgroup-root take hold_cgroup_root once before any model call; held root refused, never re-executed; tests in tree_run/ delegated style; security-relevant
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-takes-a-delegated-cgroup
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_run/run_holds_the_root.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement.rs
revision: 8
---
## Outcome

`b10x-loom run --cgroup-root R` holds `R` the way `tree run` does. Two runs on one root are
refused. `run` and `tree run` refuse each other's root. Cites are at `23f1f4b3`.

**Today.**

- `tree run` and the embedded tree runner hold a root before they use it:
  - `TreeRunner::on_root` (`crates/loom-tree/src/embed.rs:84-95`) calls `hold_cgroup_root`
    (`crates/loom-intake-slice/src/confinement.rs:3878-3912`);
  - that takes an advisory `flock` on the root's directory, released when the process ends,
    killed or not (`:3859-3867`), then checks that the root qualifies (`check_cgroup_root`).
- `run` builds `SubstrateRunner::new(root)` with no hold:
  - for the protocol's runner, `crates/loom-cli/src/main.rs:1238-1255`;
  - for a bindings file's commands, `:1176`;
  - for `resume`, `:683`.
- So a `run --cgroup-root R` can launch into a root that a `tree run` holds, and two `run`s
  can share one. That is review lane 3's finding, and it predates the w7 batch.

**After, under D1 a.**

- **One hold per process.**
  - These call `hold_cgroup_root(R)` once, before any model call and before any runner is
    built:
    - `run --cgroup-root R` (`RunArgs`, `crates/loom-cli/src/lib.rs:1172`);
    - `resume` of a run whose kept context names `R` (`main.rs:1061` keeps it, `:683` reads
      it).
  - Both come after the checks that run today before any model call: the non-UTF-8 refusal of
    `run_with_state_refuses_a_path_that_is_not_utf8_before_any_model_call`
    (`crates/loom-cli/tests/run_state.rs:165-215`) still names the path, not the hold.
  - Every `SubstrateRunner` of that command gets the held root (`hold.root()`) and keeps the
    hold alive for as long as it runs. A process that held `R` twice would refuse itself
    (`embed.rs:68-70`), so the bindings runner and the protocol's runner share one hold.
- **The refusal.**
  - A root that another process holds, or that does not qualify, ends the command with
    `ConfinementUnavailable` naming the root and Substrate's text ("held by another process:
    two runs never share a root", `confinement.rs:3895-3898`).
  - Nothing is written and no model is called.
  - The refusal is `CgroupUndelegated` (`:3880`), and `run` re-executes into a delegated scope
    on that reason when the marker is unset (`main.rs:1256-1267`). A root named on the command
    line is never re-executed: the hold's refusal is final.
- **The re-executed child keeps its order.** Under the re-exec marker the child reads its
  handoff first (`main.rs:1182`). So
  `reexec_without_private_route_handoff_fails_before_model_access`
  (`crates/loom-cli/tests/loom_cli.rs:21-40`) still fails on the handoff, not on the hold.
- **No `--cgroup-root`** (D1 b). A `run` that re-executes into its own scope
  (`prepare_delegated_cgroup`, `main.rs:1241-1250`) does not hold it. That scope is a fresh
  systemd scope no other process names. A `run` with neither keeps Substrate's own root
  discovery, unheld. Both as today.

Security-relevant: yes. Two runs in one cgroup root share its limits and its kill. A kill of one
run's cgroup tree, or one run's memory limit, can then end or starve the other's confined
programs. The unit gets a security review.

## Decisions

- **D1. Which roots `run` holds.** Default **b**.
  - **a.** Also the scope a re-executed `run` prepared, as `TreeRunner::in_own_scope` holds its
    own (`embed.rs:105-108`). Its test would need a `run` re-executed by a real `systemd-run`
    and a second process naming that fresh scope, which no host process does today.
  - **b.** Only a root an operator named: `run --cgroup-root` and the `resume` of a context that
    keeps one. Each has its test below (1 to 6). The re-executed scope is its own systemd scope
    and no other process names it, so a hold adds nothing there today.

## ESS first

No noun changes. The refusal reuses `ConfinementUnavailable` and Substrate's text.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call, no credential
file read. Fixture programs are POSIX sh, never Python. A slot job ends within 10 minutes, or
names its time with `slot --timeout` (at most 30 minutes).

**`crates/loom-cli/tests/tree_run/run_holds_the_root.rs`**, a new `#[path]` module of
`tree_run.rs`:

- It works in the style of `delegated_cgroup.rs`: `Scope::start` (`:66-105`), `Running`, the
  `cell@1` fixture whose stub waits for `target/release`, and `refused_before_any_step`.
- It skips without a delegated scope, naming what is missing, and fails instead when
  `LOOM_REQUIRE_DELEGATION=1`.
- `run` gets no model. Its `HOME` is an empty scratch directory, so a test that reached a model
  call would fail on the missing route, not on the network.
- The run-first case uses the loopback replay of `custom_protocol_commands.rs` (the fixture
  catalog at a socket the test opens, `:12-16`), with one bound command that waits for
  `target/release`.

1. `run_is_refused_a_root_a_tree_run_holds`.
   - A `tree run` on `R` waits in its step.
   - `b10x-loom run --cgroup-root R` exits non-zero, naming `R` and "held by another process".
     Its scratch `HOME` holds no state afterwards.
   - Failing first: `run` goes on past the hold.
2. `tree_run_is_refused_a_root_a_run_holds`.
   - A `run --cgroup-root R` waits in its bound command.
   - `tree run --cgroup-root R` is refused before any step, naming `R` and "held by another
     process" (`refused_before_any_step`).
   - Failing first: `run` holds nothing, so `tree run` takes `R`.
3. `two_runs_on_one_root_are_refused`.
   - A second `run --cgroup-root R` while the first waits is refused as in test 1.
   - Failing first: as test 2.
4. `the_hold_of_run_is_released_on_exit_and_on_kill`.
   - After a `run` on `R` ends, and after one is killed with SIGKILL, a `tree run` on `R` ends
     `Terminal`, as `the_root_hold_is_released_on_exit_and_on_kill` does for `tree run`
     (`delegated_cgroup.rs:360-409`).
   - Failing first: none. It is a guard against a hold kept in a file that outlives its process.
5. `a_named_root_is_never_reexecuted`.
   - `PATH` starts with a scratch directory holding a `systemd-run` stub, POSIX sh, that touches
     a marker file and exits 1.
   - A `tree run` holds `R` and waits in its step. `run --cgroup-root R`, with the re-exec
     marker unset:
     - exits non-zero;
     - names `R` and "held by another process" on standard error, and not `RUN_HINT`
       (`delegated_cgroup.rs:26`);
     - leaves the marker file absent.
   - Failing first: at the base `run` holds nothing. With `R` held and not empty, either its
     preflight refuses `CgroupUndelegated` and `run` re-executes through the stub
     (`main.rs:1256-1267`), which leaves the marker, or `run` starts its program in `R`. Neither
     prints "held by another process".
6. `resume_is_refused_a_root_a_tree_run_holds`.
   - A `run --state <dir> --cgroup-root R` leaves a commission suspended at its approval gate,
     as `resume_after_grant.rs` makes one (`:1-20`, the same fixture catalog at a loopback
     socket and the same failing check). Its kept context names `R` (`main.rs:1061`).
   - A `tree run` then holds `R` and waits in its step.
   - `b10x-loom resume <commission> --state <dir>` exits non-zero, naming `R` and "held by
     another process". It makes no model call (the socket records none), and the Suspension is
     still held.
   - Failing first: `resume` builds `SubstrateRunner::new` on `R` with no hold (`main.rs:683`)
     and goes on.

**Unchanged:** `crates/loom-cli/tests/loom_cli.rs:21-40` passes as it is, and so does
`run_state.rs:165-215`.

**Gate.** `task check` exits 0. The unit's report names which tests ran with delegation on this
host.

## Out of scope

- `tree test --cgroup-root` (`story:suite-runs-cycles-under-one-cgroup-root`).
- Two cases of one `tree run` (`story:concurrent-confined-runs`).
- A hold on the scope a re-executed `run` prepares (D1 a).

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-cli/src/main.rs` (`:627`, `:683`, `:1061`, `:1139`, `:1176`, `:1182`,
  `:1238-1267`) — cited.
- `crates/loom-cli/src/lib.rs` (`:1152-1172`, `RunArgs`), read and not changed — cited.
- `crates/loom-cli/tests/run_state.rs` (`:165-215`) and
  `crates/loom-cli/tests/resume_after_grant.rs` (`:1-20`), read — cited.
- `crates/loom-intake-slice/src/confinement.rs` (`:3840-3912`), read; whether
  `SubstrateRunner` takes a hold in its constructor or the CLI keeps it beside the runner is
  inferred.
- `crates/loom-tree/src/embed.rs` (`:64-108`), read and not changed — cited.
- `crates/loom-cli/tests/tree_run.rs` (the `#[path]` list, `:38-46`),
  `crates/loom-cli/tests/tree_run/run_holds_the_root.rs` (new),
  `crates/loom-cli/tests/tree_run/delegated_cgroup.rs` (read) and
  `crates/loom-cli/tests/custom_protocol_commands.rs` (read) — cited.
- `AGENTS.md` (the `--cgroup-root` lines, `:1708`, `:2676`) and `CHANGELOG.md` — cited.


## Edges

- `depends_on story:tree-run-takes-a-delegated-cgroup` (active): `hold_cgroup_root` and the
  test harness.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `crates/loom-cli/src/main.rs`:
    - `story:tree-run-takes-a-delegated-cgroup` and `story:cancel-a-running-bound-program`
      (active) edit the runner sites. This story merges after both;
    - `story:tree-run-command`, `story:tree-run-tests-fit-the-budget` and
      `story:tree-suite-format` (active) edit other regions. Textual merge, no order;
    - `approval-decisions-resume-held-runs` and `decisions-resume-the-held-run` (proposed) edit
      the resume path (`:627-690`). This story merges first; they keep the hold when they move
      the runner;
    - proposed, other regions: `agent-node-run-records`, `agent-selects-the-connectors-relay`,
      `approvals-supplied-to-canon-evaluation`, `daemon-continues-a-decided-request`,
      `decision-inbox-commands`, `fewer-git-starts-per-command`,
      `goals-entries-from-a-resolved-document`, `question-and-outcome-decisions-hold-runs` and
      `tree-waits-open-decision-requests`. Textual merge, no order.
  - `crates/loom-cli/tests/tree_run.rs` (the `#[path]` list) and `tests/tree_run/`:
    - `bound-program-signal-exit-is-refused` (draft) adds a module beside this one. Textual
      merge, no order;
    - `tree-run-tests-fit-the-test-budget` (draft) merges first. This module starts
      `b10x-loom`, since the binary's hold is what it tests, as that story's rule allows;
    - `concurrent-confined-runs`, `person-action-runs-a-program`,
      `suite-runs-cycles-under-one-cgroup-root`, `tree-library-exports-what-an-engine-needs`
      and `tree-suite-stop-resume-reopen-steps` (active), and the other `tree_run` stories named
      in `story:tree-run-tests-fit-the-test-budget`. Textual merge, no order.
  - `crates/loom-intake-slice/src/confinement.rs`. Only if the hold moves into
    `SubstrateRunner` (inferred); the default keeps it in the CLI:
    - `concurrent-confined-runs` and `suite-runs-cycles-under-one-cgroup-root` (active) also
      edit how runners share a root. This story merges after both;
    - `drop-the-substrate-git-feature` (active), and `substrate-0-24-0-pin`,
      `child-launches-through-confinement-adapters` and `python-test-profile` (draft). Textual
      merge, no order;
    - `substrate-pin-at-the-deny-list`, `mixed-egress-launches` and
      `tcp-reachability-through-an-aperture` (proposed). Textual merge, no order.
