---
format: aep.planning-md/3
id: story:tree-run-takes-a-delegated-cgroup
kind: story
status: proposed
title: tree run in the default substrate confinement gets a delegated cgroup of its own, and two runs never share a parent slice
summary: tree run moves into its own delegated leaf as run does or takes --cgroup-root, a refusal names tree run's own remedy, two runs never share one parent; security-relevant
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:56Z", actor: "agent:loom", revision: 5}
---
## Outcome

`tree run --confinement substrate` (the default) gets a delegated cgroup of its own, as `run`
does, and never shares a parent slice with another run.

- `--cgroup-root DIR` is new on `tree run`. When it is given, that root is used, and it must
  qualify as Substrate requires: empty, writable, delegated, with the process in a child.
- When it is absent and the process is not yet in its own scope, `tree run` re-execs under
  `systemd-run --user --scope -p Delegate=yes`, as `run` does
  (`crates/loom-cli/src/main.rs:1230-1250`, `:1500-1510`). The child moves into its own leaf
  with `prepare_delegated_cgroup` (`crates/loom-intake-slice/src/confinement.rs:3631-3641`), and
  its scope is the root.
- `tree run` never discovers an ancestor. Today `SubstrateRunner::new(None)`
  (`crates/loom-cli/src/tree.rs:123-125`) falls back to `discover_cgroup_root`
  (`confinement.rs:3615-3628`, used at `:487-490`). That returns the first empty, writable
  ancestor, which under an outer `systemd-run --scope` is the parent slice, `app.slice`
  (inferred). Two runs then share it, and one run's driver open kills the other's launches
  (`:508-519`; the count is per process).
- Two `tree run` processes that name the same `--cgroup-root`: the second is refused
  `ConfinementRefused` naming the root, before any step. The form of that host-wide hold (a lock
  file keyed by the canonical root) is inferred; the unit names it.
- A refusal names `tree run`'s own remedy. Today's text, "use systemd-run --user -p Delegate=yes
  --scope or --cgroup-root" (`confinement.rs:501-506`), names a flag `tree run` lacks
  (`crates/loom-cli/src/lib.rs:364-382`). From `tree run`, the refusal names `tree run
  --cgroup-root DIR`, or the re-exec that failed with its error. It is printed as the
  `ConfinementRefused` the tree path already prints (`crates/loom-tree/src/run.rs:935-946`).
- `tree run --help` (`TREE_RUN_HELP`, `lib.rs:328-340`) and the tree guide page state the
  precondition: Linux, bubblewrap, delegated cgroup v2, or `--confinement local`. The guide for
  `run` already does (`website/docs/guides/run-an-intent.md:21`).

Security-relevant: yes. This chooses where confined processes run, and which driver may kill
which launches. The unit gets a security review.

## Why

Runbook's tools run under `tree run`, and the refusal without a delegated root stops every
confined step. Every tree test and the tree guide therefore use `--confinement local`
(`crates/loom-cli/tests/tree_run.rs:160`).

Under a scope, `tree run` does not move into a leaf, because `prepare_delegated_cgroup` is
`run`'s alone. Two runs scoped by hand would share `app.slice`.

## ESS first

`ess/` does not change. The refusal stays `ConfinementRefused`, with `CgroupUndelegated` in its
detail. No noun changes, only the text and the route.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. A `linux` test runs in a delegated scope or skips, naming what is missing. The unit's report
says which tests ran on this host.

- A counted delegated root: two `tree run` processes, each with a confined step, run at the same
  time. Each process's confined launches lie under its own root, and the two roots differ. Neither
  run sees a launch of the other killed: both steps exit as their programs do. Test:
  `two_tree_runs_never_share_a_cgroup_root`.
- `tree run --cgroup-root DIR` with a delegated, empty root runs a confined step under `DIR`. Test:
  `tree_run_uses_the_cgroup_root_it_is_given`.
- A second process naming the same root is refused `ConfinementRefused`, naming the root, and
  writes nothing. Test: `a_second_tree_run_on_the_same_root_is_refused`.
- Without a root and without `systemd-run`: the refusal's detail names `--cgroup-root` of
  `tree run` and the re-exec's error, and the old hint is not printed. Test:
  `the_tree_refusal_names_tree_runs_own_remedy`. It runs with `PATH` lacking `systemd-run`, so no
  delegation is needed.
- `tree run` does not call `discover_cgroup_root`. A unit test on `tree_runner` shows the runner
  it builds always holds a root. Test: `the_tree_runner_never_discovers_an_ancestor`.
- `tree run --help` names the precondition and `--cgroup-root`. Test:
  `tree_run_help_names_the_confinement_precondition`.
- The guide page names the same precondition (the docs check).

## Not covered

- What a confined program may read, write or reach: `story:bound-program-declares-its-environment`,
  `story:per-run-program-path` and `story:named-egress-per-action`.
- Several drivers inside one process: `story:concurrent-confined-runs`.
- Launches of different egress inside one process: `story:mixed-egress-launches`.
- A non-Linux confinement.

## Scope

- `crates/loom-cli/src/tree.rs:120-135` (`tree_runner`). Cited.
- `crates/loom-cli/src/lib.rs:328-340` and `:364-382`. Cited.
- `crates/loom-cli/src/main.rs:1230-1250` and `:1490-1520`: the re-exec, to share with `run`.
  Cited.
- `crates/loom-intake-slice/src/confinement.rs`: `:480-520` and `:3613-3641`. Cited.
- `crates/loom-cli/tests/tree_run.rs`, with the new tests. Cited.
- The tree guide page: `website/docs/guides/run-a-case-tree.md`, added by the unmerged
  `impl/tree-run-command`. Inferred.

Edges:

- depends_on `story:tree-run-command`.
- Neighbours: `story:concurrent-confined-runs` and `story:mixed-egress-launches`, which both touch
  `SubstrateRunner::open` (`confinement.rs:480-540`). This story changes `tree_runner` and adds a
  hold across processes, not the in-process table.
