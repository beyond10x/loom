---
format: aep.planning-md/3
id: story:substrate-pin-at-the-deny-list
kind: story
status: proposed
title: Loom links the Substrate release that holds the executable deny-list, and every read-only mount it passes still works
summary: pin move from 0.21.0 to the tag (or commit b280c4ca) with the executable deny-list; one compile break, a lock-file test, limits 4 to 8; passing a deny-list is a follow-up
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:canon-0-10-0-pin
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T04:57:14Z", actor: "agent:loom", revision: 5}
---
## Outcome

Loom builds on a Substrate that holds the executable deny-list:

- `b10x-substrate-host` and `b10x-substrate-wire` are pinned at or after commit
  `b280c4ca7706338e869e5ec43903aecd1ce7bf0d`, the merge of Substrate PR 243 ("confinement: a
  profile names executables that never run").
- If Substrate has a tag containing that commit when the unit starts, the pin names the tag.
  Otherwise the pin names the commit, and moving to the tag is a second step once Substrate cuts
  one. Tag `0.23.0` does not contain the commit.
- At the commit, the manifest's version reads `0.23.0`, so both lines read `version = "=0.23.0"`.
- `default-features = false, features = ["git"]` stays as it is. `Cargo.lock` holds one copy of
  each Substrate crate.
- Every compile break is fixed, the confinement tests pass, and one confined step that looks like
  a Runbook step runs through `bound_program` on the new pin.

What the deny-list is, and what Loom can reach of it at this commit:

- It is `ConfinementProfile.exec_denied_paths` in `substrate_host::confinement`: paths whose
  execution fails, even beneath an `exec_subpaths` entry. It denies execution; it does not deny
  reads. A read-only mount stays readable.
- Loom reaches it through no route today:
  - the wire start Loom's `ChildCommand` writes (`ExecStartInput`) has no field for either list
    ("a wire start has no field for one", `crates/substrate-host/src/process.rs:242`);
  - Loom's adapter-route profiles (`confinement/engine.rs:234`, `:347`) set no `exec_subpaths`;
  - a deny-list beside an empty allow-list is refused
    `confinement.exec-denied-paths-without-subpaths`;
  - the gate needs `HostConfig.exec_gate`, the path of the helper, which Loom does not set.
- Default (a): this story moves the pin, proves Loom's mounts under the new Substrate, and stops.
  Passing a deny-list is a following story, either through the `linux` adapter route with
  `exec_gate` set or through a Substrate change that adds the lists to the wire start.
- Alternative (b): this story also passes the lists. That is a change of architecture, so it waits
  for an answer.

## Why

Runbook needs the pin at or after `b280c4ca`. Loom pins `=0.21.0` at rev `adfde402`
(`crates/loom-intake-slice/Cargo.toml:30-31`, `Cargo.lock:821-868`), and Loom has asked Substrate
for a tag. `adfde402` is an ancestor of `b280c4ca`, with 73 commits between them.

The changes between the two revisions that touch Loom
(`git -C ~/beyond10x/substrate diff adfde402 b280c4ca`, read only):

- `confinement::linux::proof(machine)` becomes `proof(machine, exec_lists: &ExecListProof)`. Loom
  calls it at `confinement.rs:3444`, so this is the one compile break. The fix passes
  `&ExecListProof::default()`, because Loom has no gate and so proves no exec-list fact.
- `ConfinementProfile` gains `exec_denied_paths`. Loom builds it only with `..Default::default()`
  (`engine.rs:234-237`, `:347-354`), so nothing breaks.
- `HostConfig` gains `exec_gate`. Loom builds it with `HostConfig::minimum` (`confinement.rs:347`),
  so nothing breaks.
- `ConfinementError::FactUnserved` gains `withheld`, and two variants are new:
  `ExecDeniedPathsWithoutSubpaths` and `ExecDeniedPathsExceeded`. Loom matches neither, so nothing
  breaks (inferred from `git grep`).
- `FACT_ORDER` grows from 13 to 14 facts, with `ExecDeniedPaths` after `ExecSubpaths`. Loom reads
  it by name (`confinement.rs:3464`). The exact list in the facts test (`:3686-3697`) holds no
  exec-list fact, so it is unchanged unless the probe proves one.
- In the wire, `MAX_READ_ONLY_ROOTS` and `MAX_EGRESS_APERTURES` go from 4 to 8, and the contract
  bundle goes from `substrate-wire/0.26.0` to `0.27.0`. `ExecStartInput` gains no field, so the
  projection at `confinement.rs:1678-1694` builds unchanged. `ChildCommand::MAX_READ_ONLY_ROOTS`
  (`:1233`) and the re-exported `MAX_EGRESS_APERTURES` (`:41`) follow the new values.
  `confinement_spec.rs:343`, `:363` and `:400` derive from them. The probe snapshot in
  `confinement.rs:3680` names `max_roots: 4`.
- `ChildCommand::with_read_only_root`, `with_read_only_file` and `at_host_path` (`:1264-1304`) are
  Loom's own, and their bodies do not change.
- The `git` feature: the features table is the same at both revisions
  (`git = ["dep:curl", "dep:git2", "dep:gix"]`). No libgit2-check story is in the store at this
  head. The feature setting is kept unless such a story removes it.
- `rust-version` is `1.97` at both revisions.

## ESS first

Exempt: the move changes no noun and no rule of Loom's specification. The first commit adds the
mount tests below, red on `adfde402` because `ExecListProof` and `exec_denied_paths` are absent.
The second commit moves the pin and `Cargo.lock` (`cargo update -p b10x-substrate-host -p
b10x-substrate-wire`).

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no model call, no
network. No test fetches. A `linux` test runs in a delegated scope or skips, naming what is
missing, and the unit's report says which ran on this host.

- `Cargo.lock` holds exactly one `b10x-substrate-host`, one `b10x-substrate-wire` and one
  `b10x-substrate-guarded`, from the new source. The test pins the version and source, and it
  reads the new pair. Test: `host_git_hardening.rs:574-580`, updated.
- `git grep -n adfde402 -- ':!.engineering' ':!CHANGELOG.md'` prints nothing.
- The facts the `linux` adapter proves for Loom's snapshot are listed exactly, in the new
  `FACT_ORDER`. Test: `confinement.rs:3680-3697`, updated, which names any fact that is new.
- Each mount Loom passes has its own test. Without a deny-list it is admitted on the new pin. With
  a deny-list naming the program it holds, it is refused on Loom's driver with the refusal text
  (inferred: the `FactUnserved` text naming `exec_denied_paths`, or
  `confinement.exec-denied-paths-without-subpaths`):
  - the toolchain root (`confinement.rs:3282-3298`), test
    `the_toolchain_root_is_admitted_and_a_denied_compiler_is_refused`;
  - the three Cargo cache directories `registry`, `git/db` and `git/checkouts`
    (`confinement.rs:3305-3335`, `CACHE_DIRS` `:75`), test
    `the_three_cargo_caches_are_admitted_and_a_denied_entry_is_refused`;
  - the bound program binary through `at_host_path` (`bindings.rs:441-446`), test
    `the_bound_program_binary_is_admitted_and_its_denial_is_refused`;
  - the retry feedback file through `at_host_path` (`retry.rs:209`), test
    `the_retry_feedback_file_is_admitted_and_its_denial_is_refused`.
- Eight read-only roots are admitted and a ninth is refused, the bound on the new pin. Test:
  `confinement_spec.rs:343`, updated.
- A confined bound program that reads one read-only root and writes its workspace runs through
  `ProgramEffects` on the new pin and exits 0. Test, beside
  `under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace`
  (`bound_command_confined.rs:406`):
  `a_runbook_style_confined_step_runs_through_bound_program_on_the_new_pin`.
- These pass: `cargo test -p b10x-loom-intake-slice --test confinement --test confinement_spec
  --test confined_child --test bound_command_confined --test engine_confinement` and
  `-p b10x-loom-bindings --test bound_program`. Each new test is timed and named with its time
  when over 5 s.
- `CHANGELOG.md` `[Unreleased]` names the Substrate move. It says that `MAX_EGRESS_APERTURES` and
  `ChildCommand::MAX_READ_ONLY_ROOTS` are now 8 and that the deny-list is not yet reachable from
  Loom. `AGENTS.md:1624` is kept as history.

## Out of scope

- Passing `exec_subpaths` or `exec_denied_paths` from Loom, and setting `HostConfig.exec_gate`.
  That is alternative (b), or a following story.
- Removing the `git` feature or libgit2.
- Using the higher root and aperture bounds for anything new.

## Scope

- `crates/loom-intake-slice/Cargo.toml:30-31` and `Cargo.lock:821-868`. Cited.
- `crates/loom-intake-slice/src/confinement.rs`, cited:
  - the proof call `:3444`;
  - the facts test `:3680-3697`;
  - the mounts `:3282-3335`;
  - the projection `:1678-1694`;
  - the bounds `:41` and `:1233`;
  - `HostConfig::minimum` `:347`.
- `crates/loom-intake-slice/src/confinement/engine.rs:234-237` and `:347-354`. Read only, cited.
- `crates/loom-intake-slice/src/bindings.rs:441-446` and `crates/loom-bindings/src/retry.rs:209`.
  Read only, cited.
- `crates/loom-intake-slice/tests/`, cited: `host_git_hardening.rs:574-580`,
  `confinement_spec.rs:343-400`, `bound_command_confined.rs:406`. The mount tests' file is
  inferred.
- `CHANGELOG.md`. Cited.

Edges:

- depends_on `story:canon-0-10-0-pin`.
- Collides with `story:canon-0-10-0-pin` on `Cargo.lock`.
