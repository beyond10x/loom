---
format: aep.planning-md/3
id: story:substrate-0-24-0-pin
kind: story
status: active
title: 'Loom pins Substrate 0.24.0: one compile break fixed, bounds 4 to 8, the ioctl floor shown for a bound program'
summary: =0.21.0 adfde402 to =0.24.0 85da2e2b; proof(machine, &ExecListProof::default()); fifth-root test to ninth; e4defrag EACCES test; replaces substrate-pin-at-the-deny-list (D1 a)
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:drop-the-substrate-git-feature
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/bound_command_confined.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T15:08:36Z", actor: "agent:loom", revision: 13}
- {from: "proposed", to: "active", at: "2026-11-22T15:09:58Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "runbook-batch4-2026-11-22"}
---
## Outcome

Loom pins Substrate `0.24.0` and builds and passes its lanes on it. Cites are at `23f1f4b3`.

- **The pin.**
  - `b10x-substrate-host` and `b10x-substrate-wire` are at the tag `0.24.0`, commit
    `85da2e2be46b690993007990905d6d3934650774`, with `version = "=0.24.0"`.
  - Today they are at `=0.21.0`, rev `adfde402` (`crates/loom-intake-slice/Cargo.toml:30-31`).
  - The feature setting stays (`default-features = false`).
  - `Cargo.lock` holds one copy each of `b10x-substrate-host`, `-wire` and `-guarded`
    (`Cargo.lock:826-870`).
- **Breaks.** Every break is fixed, and each is named below by its Loom site.
- **Conformance.** Loom's Substrate conformance is unchanged.
- **The ioctl floor.** A confined bound program gets `EACCES` for `EXT4_IOC_MOVE_EXT`, the one of
  Substrate's three new refusals this host can run.

The tag contains `b280c4ca`, the deny-list merge that `story:substrate-pin-at-the-deny-list`
waits for (`git merge-base --is-ancestor b280c4ca 0.24.0` holds). D1 says how the two stories
meet.

### What changed for an embedder, 0.22.0 to 0.24.0 (Substrate `CHANGELOG.md` at `0.24.0`, read only)

- **0.22.0.**
  - The contract bundle moves from `substrate-wire/0.26.0` (Loom's today) to `0.27.0`.
  - Three bounds rise: `MAX_EGRESS_APERTURES` 4 → 8, `MAX_READ_ONLY_ROOTS` 4 → 8, and a
    deployment's apertures 6 → 12 (`substrate-wire` `lib.rs:193`, `:383` at `0.24.0`).
- **0.23.0.**
  - `confinement::linux::proof` takes a second argument, `&ExecListProof`.
  - `ConfinementError::FactUnserved` and `AdapterFacts` gain `withheld`.
  - `HostConfig` gains `exec_gate`.
  - `substrate_exec_gate::place` changes.
- **0.24.0.**
  - `FACT_ORDER` is `[Fact; 14]`, with `exec_denied_paths` after `exec_subpaths`.
  - `ConfinementError` gains `ExecDeniedPathsWithoutSubpaths` and `ExecDeniedPathsExceeded`.
  - New fields: `ConfinementProfile.exec_denied_paths`, `ExecListProof.denied_paths`,
    `seatbelt::BuildProof.exec_denied_paths` and `substrate_exec_gate::Lists.deny`.
  - Every confined run answers `EXT4_IOC_MOVE_EXT`, `XFS_IOC_SWAPEXT` and
    `F2FS_IOC_SEC_TRIM_FILE` with `EACCES` (Security, ADR 0050).
  - A daemon refuses a read-only source that names its own private paths.
  - `url` moves to 2.5.9, for RUSTSEC-2026-0592.
  - The wire, the bundle (`0.27.0`) and the daemon are otherwise unchanged.

### Each site in Loom (cited, read at `23f1f4b3`)

- **A compile break.** `crates/loom-intake-slice/src/confinement.rs:3655` calls
  `substrate_host::confinement::linux::proof(machine)`. It takes `&ExecListProof::default()` as
  its second argument, since Loom sets no `exec_gate` and proves no exec-list fact.
  `ExecListProof` derives `Default` (Substrate `confinement-public-items.txt:34`).
- **No break: every Substrate `ConfinementProfile` literal ends `..Default::default()`.**
  - `crates/loom-intake-slice/src/confinement/engine.rs:235-238`;
  - `:348-355`.
  - `confinement.rs:3740-3766` is Loom's own `ConfinementProfile`, not Substrate's.
- **No break: the one `match` on Substrate's `ConfinementError` has a `_` arm**
  (`engine.rs:254-263`). The two new variants map to `CapabilityUnserved`.
- **No break: `FACT_ORDER` is read by iteration, never by its type**:
  - `confinement.rs:37`, `:3675-3679`;
  - `engine.rs:28`, `:304-308`;
  - `crates/loom-intake-slice/tests/security_w20261121w1_u1.rs:19`, `:49`.
  - `Fact::LocalPermitted` is named once (`engine.rs:296`), and no `match` on `Fact` is
    exhaustive.
- **No break: the test fake `Counting` implements Substrate's `ConfinementAdapter`**
  (`engine.rs:1104-1150`, `impl Adapter for Counting` at `:1120`). The trait's methods are the
  same at `0.21.0` and `0.24.0` (a diff of their `fn` lines in Substrate
  `crates/substrate-host/src/confinement/mod.rs`, trait at `:815` at `0.24.0`), and the
  `AdapterUnserved { adapter, detail }` variant it builds (`:1112-1117`) has the same fields.
- **No break: `HostConfig` is built by `HostConfig::minimum`** (`confinement.rs:431`).
- **Behaviour: the bounds that follow Substrate's constants.**
  - The re-export `MAX_EGRESS_APERTURES` (`confinement.rs:41`) and
    `ChildCommand::MAX_READ_ONLY_ROOTS` (`:1385`) become 8.
  - `confinement_spec.rs:363` and `:400` derive from them, and so stay right.
  - `confined_child.rs:2025-2050`, `a_fifth_root_is_refused_before_launch`, builds 5 roots and
    expects a refusal, so it goes red. It becomes `a_ninth_root_is_refused_before_launch`.
  - The probe snapshot in `confinement.rs:4023` names `max_roots: 4`. It is a fixture value of
    the driver's facts and changes to 8 with the test's comment.
- **Behaviour: the facts test.**
  - `confinement.rs:4031-4041` lists the proved facts exactly.
  - No exec-list fact is proved without a gate, so the list is expected unchanged. If the probe
    proves one, the test names it (inferred).
- **The lock-file test.** `crates/loom-intake-slice/tests/host_git_hardening.rs:578-581` pins
  `0.21.0` and the `adfde402` source. It reads `0.24.0` and the tag's commit.
- **Documents.**
  - `website/docs/reference/sdk-stability.md:144` says "`MAX_EGRESS_APERTURES` (4)", "more than
    four apertures", "a second to fourth aperture" and "more than 4 roots";
  - `AGENTS.md:170` says "past 4 roots";
  - `AGENTS.md:2693-2694` names the facts "on Substrate 0.21.0";
  - `AGENTS.md:2833-2834` says "Substrate 0.21.0 binds read-only where the host has it" and
    "more than 4 roots". No change to those binds is in Substrate's `CHANGELOG.md` for 0.22.0
    to 0.24.0, so the release named moves to `0.24.0`, and the bound to 8 under D2 a;
  - `AGENTS.md:1813` names 0.21.0 as history and stays.
  - These are the lines at `23f1f4b3`. At `f0a9ca8b` they were `:2687`, `:2826` and `:1806`; a
    merge between the two heads added seven lines above them.
  - Under D2 a, each bound follows the new value.

## Decisions

- **D1. This story and `story:substrate-pin-at-the-deny-list` (proposed).** Default **a**.
  - **a.** This story replaces it. Its Outcome says the pin names a tag that holds `b280c4ca`
    once one exists, and `0.24.0` is that tag. Its acceptance (mount tests, eight roots, a
    Runbook-style step) is carried here in the Acceptance below. The coordinator withdraws or
    supersedes it; this draft does not touch it.
  - **b.** It lands first at the commit, and this story moves from there to the tag. That makes
    two pin moves through the same files.
- **D2. The two bounds that rise to 8.** Default **a**.
  - **a.** Loom follows Substrate: 8 apertures and 8 read-only roots, as the deny-list story
    already assumed ("limits 4 to 8"). The tests and documents change with the value.
  - **b.** Loom keeps its own bound of 4 (a Loom constant, no longer the re-export). That needs
    no document change, but `MAX_EGRESS_APERTURES` stops being Substrate's and the SDK page must
    say so.
- **D3. The ioctl test.** Default **a**.
  - **a.** A confined bound program runs `e4defrag` on a file of its ext4 workspace; the test
    reads its `EACCES` (below). It skips, naming what is missing, without `/usr/bin/e4defrag`,
    without an ext4 workspace or without delegation, and fails instead under
    `LOOM_REQUIRE_DELEGATION=1`. This host has `/usr/bin/e4defrag` (e2fsprogs 1.47.4) and an
    ext4 tree.
  - **b.** No Loom test. Cite Substrate's own cases, which ran `EXT4_IOC_MOVE_EXT` on the
    `linux` adapter and the host driver. `XFS_IOC_SWAPEXT` and `F2FS_IOC_SEC_TRIM_FILE` are not
    reachable on this host under either option (no XFS or f2fs).

## ESS first

Exempt: no noun or rule of Loom's specification changes. The first commit holds the tests below,
red on `0.21.0`. The second moves the pin and `Cargo.lock` (`cargo update -p
b10x-substrate-host -p b10x-substrate-wire`).

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no model call, no network. No test
fetches. Fixture programs are POSIX sh, never Python. A `linux` test runs in a delegated scope or
skips, naming what is missing; the unit's report says which ran on this host. A slot job ends
within 10 minutes, or names its time with `slot --timeout` (at most 30 minutes). A script that
only measures runs without the `slot` prefix.

1. **The lock.** `host_git_hardening.rs` `every_host_git_command_goes_through_a_hardened_helper`
   (`:444`, the lock lines `:578-581`) reads one each of `b10x-substrate-host`, `-wire` and
   `-guarded` at `0.24.0`, from `rev=85da2e2b…`.
   - `git grep -n adfde402 -- ':!.engineering' ':!CHANGELOG.md'` prints nothing.
   - Failing first: the lock holds `0.21.0`.
2. **The compile break.** `cargo check -p b10x-loom-intake-slice` passes on the new pin, with
   the proof call at `confinement.rs:3655`.
3. **The facts.** `a_driver_missing_a_family_is_refused_naming_it` (`confinement.rs:3993`) lists
   the proved facts exactly in the new `FACT_ORDER`, with `max_roots: 8` in its snapshot.
4. **The bounds, under D2 a.**
   - `confined_child.rs` `a_ninth_root_is_refused_before_launch`: 8 roots get past every check,
     and a ninth is refused `ScopeInvalid`, naming it.
   - `confinement_spec.rs:363` and `:400` pass unchanged.
   - Failing first: on `0.21.0` the eighth root is refused.
5. **The ioctl floor, under D3 a.** In `crates/loom-intake-slice/tests/bound_command_confined.rs`,
   beside `under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace` (`:406`, and
   its `support::delegated` gate):
   - Test `under_linux_a_bound_program_cannot_move_file_blocks`.
   - A bound program, POSIX sh, writes a 1 MiB file in its workspace and runs
     `e4defrag <file>`. It prints e4defrag's exit status and standard error.
   - The kept output names `Permission denied`, the `EACCES` of `EXT4_IOC_MOVE_EXT`, and the
     file's SHA-256 is unchanged.
   - Failing first: on `0.21.0` the ioctl is not filtered. The unit records what `e4defrag`
     printed there.
6. **Lanes.** These pass:
   - `cargo test -p b10x-loom-intake-slice --test confinement --test confinement_spec --test
     confined_child --test bound_command_confined --test engine_confinement`;
   - `-p b10x-loom-bindings --test bound_program`.

   Each new test is timed and named with its time when over 5 s.
7. **Conformance.** `task conform` and `task intake-conform` give the same counts as at
   `23f1f4b3`.
8. **Documents.**
   - `sdk-stability.md:144`, `AGENTS.md:170`, `AGENTS.md:2693-2694` and
     `AGENTS.md:2833-2834` name the new values. `AGENTS.md:1813` is unchanged.
   - `CHANGELOG.md` `[Unreleased]` (`:13`) names the move, the two bounds, the ioctl floor and
     the `url` advisory.
   - It also says the deny-list is not yet reachable from Loom.
9. **From `story:substrate-pin-at-the-deny-list`, under D1 a.**
   - Its four mount tests, taken over with the admission half only. Their denial halves need a
     deny-list Loom cannot pass (Out of scope). Each is admitted on the new pin, where `0.21.0`
     has no `ExecListProof` for the proof call (compile-red first). They live in
     `crates/loom-intake-slice/tests/confined_child.rs` (inferred), each under the
     `support::delegated` gate of `bound_command_confined.rs`:
     - `the_toolchain_root_is_admitted_on_the_new_pin`: the toolchain read-only root
       (`confinement.rs:3493-3511`);
     - `the_three_cargo_caches_are_admitted_on_the_new_pin`: `registry`, `git/db` and
       `git/checkouts` (`CACHE_DIRS`, `confinement.rs:75`, mounted at `:3521-3545`);
     - `the_bound_program_binary_is_admitted_on_the_new_pin`: the binary through `at_host_path`
       (`crates/loom-intake-slice/src/bindings.rs:445`);
     - `the_retry_feedback_file_is_admitted_on_the_new_pin`: the feedback file through
       `at_host_path` (`crates/loom-bindings/src/retry.rs:331`).
   - Its confined step:
     `a_runbook_style_confined_step_runs_through_bound_program_on_the_new_pin`, in
     `bound_command_confined.rs` beside
     `under_linux_a_bound_command_reads_no_host_file_and_writes_its_workspace` (`:406`).
     A bound program reads one read-only root and writes its workspace, runs through
     `ProgramEffects` and exits 0.

**Gate.** `task check` exits 0.

## Out of scope

- Passing `exec_subpaths` or `exec_denied_paths` from Loom, and `HostConfig.exec_gate`.
- `oci` and `pod`, which Substrate's floor does not cover.
- Using the higher bounds for anything new.

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`; Substrate was read at tag `0.24.0`, read
only. **Cited** means read at the line named; **inferred** means a reading that could be wrong.

- `crates/loom-intake-slice/Cargo.toml` (`:30-31`) and `Cargo.lock` (`:826-870`) — cited.
- `crates/loom-intake-slice/src/confinement.rs` (`:37`, `:41`, `:431`, `:1385`, `:3652-3679`,
  `:3740-3766`, `:3993-4045`) — cited.
- `crates/loom-intake-slice/src/confinement/engine.rs` (`:28`, `:235-238`, `:254-263`,
  `:296-308`, `:348-355`), read and not changed — cited.
- `crates/loom-intake-slice/tests/`: `host_git_hardening.rs` (`:444`, `:578-581`),
  `confined_child.rs` (`:2025-2050`), `confinement_spec.rs` (`:363`, `:400`),
  `bound_command_confined.rs` (`:406`) and `security_w20261121w1_u1.rs` (`:19`, `:49`) — cited.
- `website/docs/reference/sdk-stability.md` (`:144`), `AGENTS.md` (`:170`, `:1813`,
  `:2693-2694`, `:2833-2834`) and `CHANGELOG.md` (`:13`) — cited.

## Edges

- `depends_on story:drop-the-substrate-git-feature` (active): the feature setting this pin
  keeps.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below except where a region is named.
- **Lands before `story:child-launches-through-confinement-adapters` (draft).** That story
  deletes the host-driver route and reads Substrate 0.21.0 (its `depends_on
  story:substrate-0-21-pin`). It shares these files with this story:
  - `crates/loom-intake-slice/src/confinement.rs`;
  - `crates/loom-intake-slice/tests/confined_child.rs`;
  - `crates/loom-intake-slice/Cargo.toml` and `Cargo.lock`;
  - `website/docs/reference/sdk-stability.md`.

  This pin lands first, and that story then reads its sites on 0.24.0. It is an outcome order:
  that story's linux route is built against the adapter API this pin brings (`linux::proof`
  with `&ExecListProof`). The edge belongs on that story, `depends_on
  story:substrate-0-24-0-pin`. This draft does not touch it; the coordinator records it.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `story:substrate-pin-at-the-deny-list` (proposed), the same files. Under D1 a it is
    replaced; under D1 b it merges first.
  - `story:canon-0-11-0-pin` (active). Textual merge, no order:
    - `Cargo.lock`, other entries;
    - `AGENTS.md`: it edits § Governor `:717-735`, this story `:170`, `:2693-2694` and
      `:2833-2834`;
    - `sdk-stability.md`: it edits `:72`, this story `:144`.

    The manifests differ (`loom-governor`'s against `loom-intake-slice`'s).
  - `Cargo.lock`, active: `canon-0-10-0-pin`, `bound-program-result-table`,
    `decision-request-entity`, `engineering-protocols-dependency-removed`,
    `planning-only-pull-requests-skip-build`, `protocol-argument-schema`,
    `tree-library-exports-what-an-engine-needs` and `tree-run-command`; draft:
    `docs-check-builds-once-per-fresh-tree` and `secrets-subcommand`. Textual merge, no
    order; a lock conflict is resolved by `cargo update -p` of this story's two crates after
    the other's merge.
  - `crates/loom-intake-slice/Cargo.toml`. Textual merge, no order:
    - `concurrent-confined-runs` and `engineering-protocols-dependency-removed` (active);
    - `approval-decisions-resume-held-runs`, `decisions-resume-the-held-run`,
      `gate-run-collects-a-gates-document` and `observe-executor-reads-workspace-files`
      (proposed).
  - `crates/loom-intake-slice/src/confinement.rs`. This story edits the proof call (`:3655`),
    the facts test (`:4023-4041`) and the comments of the bounds (`:41`, `:1385`):
    - active: `bound-program-case-id`, `bound-program-result-table`,
      `bound-program-retry-records`, `cancel-a-running-bound-program`,
      `concurrent-confined-runs`, `drop-the-substrate-git-feature`,
      `handoff-check-holds-the-cancel`, `local-gate-equals-forge`,
      `suite-runs-cycles-in-parallel`, `suite-runs-cycles-under-one-cgroup-root`,
      `tree-library-exports-what-an-engine-needs` and `tree-run-takes-a-delegated-cgroup`.
      Textual merge, no order;
    - proposed: `bound-program-model-credential-through-a-secret-slot`,
      `dns-resolution-through-a-resolver-aperture`, `ess-rules-of-the-runtime`,
      `fewer-git-starts-per-command`, `gate-program-identity-bound-to-spawn`,
      `gate-run-collects-a-gates-document`, `observe-executor-reads-workspace-files`,
      `observe-git-changed`, `observe-git-in-a-linked-worktree`, `per-run-program-path` and
      `release-binaries-macos`. Textual merge, no order;
    - `mixed-egress-launches` and `tcp-reachability-through-an-aperture` (proposed) read the
      aperture bound. Under D2 a they merge after this story and see 8;
    - draft: `python-test-profile`. Textual merge, no order;
    - draft: `run-cgroup-root-holds-the-root`, only if its hold moves into `SubstrateRunner`
      (inferred). Textual merge, no order.
  - `crates/loom-intake-slice/tests/` (`confined_child.rs`, `bound_command_confined.rs`,
    `host_git_hardening.rs`): `one-test-binary-per-crate` (active) turns them into modules of
    `tests/main.rs`. The files keep their paths and test names. Textual merge, no order.
  - `host_git_hardening.rs`, the lock lines: `drop-the-substrate-git-feature` (active), which
    merges first, and the proposed observe and gate stories named above. Textual merge.
  - `website/docs/reference/sdk-stability.md`, proposed: `agent-selects-the-connectors-relay`,
    `decision-inbox-commands`, `goals-entries-from-a-resolved-document` and
    `goals-grounds-and-free-answers`. Other rows than `:144`. Textual merge, no order.
- Security-relevant: yes. This moves the confinement floor (the ioctl refusal) and a daemon
  refusal of private read-only sources. The unit gets a security review of the diff in
  `confinement.rs`.
