---
format: aep.planning-md/3
id: review-result:confinement-opens-from-engine-config-design-c1
kind: review-result
status: active
title: Plan critic design, on the narrowed story:confinement-opens-from-engine-config and story:child-launches-through-confinement-adapters
relations:
- reviews: story:confinement-opens-from-engine-config
- reviews: story:child-launches-through-confinement-adapters
revision: 1
---
needs-revision

story:confinement-opens-from-engine-config — The body never says how the test runner's sandbox-path input maps onto `local`, where paths are host paths. `run` always mounts read-only roots (the toolchain, the cargo caches) and sets `CARGO_HOME`, `CARGO_TARGET_DIR` and `PATH` to sandbox paths. So the Outcome's own rule refuses `run` under `Local` on any host with rustup or a cargo home, and the `run` part of Acceptance 2 cannot pass as written — .engineering/planning/story/confinement-opens-from-engine-config.md:66-69, :124-127; crates/loom-intake-slice/src/confinement.rs:994, :3164-3205, :3217-3231, :3275; ~/.cargo/git/checkouts/substrate-29ab43bbe6970bfc/adfde40/crates/substrate-host/src/confinement/profile.rs:195-198, local.rs:104-119

story:confinement-opens-from-engine-config — § ESS first adds only `local_reason`, but a `local` record carries `adapter` and `facts`, and the ESS comment on those members says they are absent on an unconfined run. That comment has to change in the ESS commit — .engineering/planning/story/confinement-opens-from-engine-config.md:58-60, :113-116; ess/intake/domains/confinement.yaml:67-69

story:child-launches-through-confinement-adapters — Acceptance 7 has two `spawn_child` children in two workspace parents run at once. The Outcome keeps the `running()` guard, which refuses opening a second adapter on the cgroup root while the first child runs. So the bullet must say the second parent's adapter opens before the first child starts, or it fails by design — .engineering/planning/story/child-launches-through-confinement-adapters.md:70-72, :137-138; crates/loom-intake-slice/src/confinement.rs:116-124, :392-401

story:limit-hits-in-run-log — The story now lands while in-process `linux` still runs on the host-driver route, where `settle` reads `exec.memory-limit`. But its in-process kinds and its "no `memory` line" bullet are argued from the adapter route, which arrives only with the held story, and no edge orders the two. Rewrite those lines for the host-driver route (the coordinator's planned revision) or add the edge — .engineering/planning/story/limit-hits-in-run-log.md:40-42, :59-62, :119-120; crates/loom-intake-slice/src/confinement.rs:836-839; .engineering/planning/epic/tree-daemon-under-enforced-limits.md:61-65

**Checks from the brief**
- **Landing on today's code:** the story can land without the held story. It opens only `AdapterRequest::Local` (:121-123), and `git grep` finds no `confinement::open(` in `crates/` today, only `linux::proof` (confinement.rs:3316). Every bullet fails before the story, because no constructor or `EngineConfinement` exists yet. :147-148 is a regression guard on behaviour that holds today.
- **Security:**
  - The reason check matches `ConfinementProfile::reason` (profile.rs:234-238).
  - Host policy matches `local_permitted` (local.rs:92-99) and admission (mod.rs:606-611).
  - Egress is refused, not dropped (:139-140).
  - No fallback (:147-148), and requests name no adapter (:149-150).
  - Bounds are not sent: `Limits` is implied by bounds (profile.rs:202) and `local` does not serve it.
- **Deadline kill:** the claim holds. `local`'s `signal(Kill)` sends `kill(-pid, SIGKILL)` while the leader is unreaped (local.rs:427-453). `kill_group` sends `killpg` to the unreaped leader's group (executor.rs:1386-1418). Both reach every process still in the group and none that left it.
- **Intermediate state:**
  - Title :6, summary :7, Outcome :81-90 and Why :103-104 say nothing false for `linux`, and nothing claims what only the second story makes true.
  - The Scope prose at :137 still says "predecessor"; the ledger already notes it.
- **`story:bound-commands-run-confined`:** everything it reads exists after story 1.
  - `run_child` under `local` with a per-source reason: two runners from two values, and `local_reason` on the record.
  - `run_child` under `linux`: today's route.
  - Its stdin-carrying commands go through `run_child` with `with_stdin`, which Acceptance 2 covers.
- **Second story:** agrees with the reversal (:43-58, :104-107, :160-163), owns the keyed open (:60-72), and assumes nothing about landing first, apart from Acceptance 7 above.
- **Ledger § Open 1 (`Backend` of a `local` record):** no source contradicts the default. One consequence is unstated: resume stores only `Backend` (resume.rs:510-511), and `Backend::Substrate` rebuilds a `linux` `SubstrateRunner` (main.rs:670-671). So a resumed `local` run comes back under `linux`. That fails closed, and no bullet covers it.
- **Ledger § Open 2 (type of the host-policy refusal):** no source contradicts the default. Substrate's own variant is `LocalRefused` / `confinement.local-refused`, 403 (mod.rs:228, :247, :258), and the default names that code in its detail.
- **Edges:** no cycle. Story 1 has no `depends_on`. Story 2 depends on story 1 and `story:substrate-0-21-pin`. The `blocks` edge still holds story 2 only.
- **Other stories naming the old order:**
  - `story:limit-hits-in-run-log` and `epic:tree-daemon-under-enforced-limits`: the finding above.
  - `story:substrate-daemon-runner` :84-86 ("opens `local` or `linux` in process") and :115-118 ("comes after `story:child-launches-through-confinement-adapters`"), and `epic:protocol-tree-on-bindings` :58-60: now false in wording only. Nothing they read is missing.
- **Ledger disagrees with the store:** c1-changes.md:25, :61 and § Open 6 record a `limit-hits-in-run-log` edge that `aep plan artifact graph` does not show. The brief says it was dropped on purpose.

What I read: 4 artifacts in full, both stories as they are now (and `story:bound-commands-run-confined` from the earlier pass), `story:limit-hits-in-run-log` lines 13-16, 36-66 and 115-122, the old-order lines of 3 more artifacts, the brief and c1-changes.md, plus confinement.rs (:116-124, :392-401, :1495-1576, :3125-3284), executor.rs:1380-1418, the ESS confinement domain, and Substrate `adfde40` `local.rs`, `profile.rs` and `mod.rs`. Commands: `aep plan artifact graph`, `git status`. I walked 7 edges touching the set (6 `depends_on`, 1 `blocks`), going outside the set. I traced 5 acceptance reads to a producer: 4 are recorded by an edge, and the `limit-hits-in-run-log` one is not.

Could not establish:
- How `with_workdir` and `sandbox_path` (paths under `/workspace`) map under `local`, whose workdir must be relative (mod.rs:914-929). Same gap as the first finding, but for `run_child`. I did not test it.
- Whether the deadline bullets cover `run_child` with `with_stdin`, the path bound commands take (:130-135). Out of my lane, for the acceptance critic.
- For the parallel-safety critic: story 2's frontmatter scope lacks `ess/intake/domains/confinement.yaml` and `tests/engine_confinement.rs`, though its § Waits on (:160-163) says they join its files. Ledger § Open 4 (`story:substrate-daemon-runner` and story 2 share the constructor, with no edge between them) is also theirs.

```findings
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 124
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the body does not say how the test runner's sandbox-path input (read-only toolchain and cache roots, CARGO_HOME, CARGO_TARGET_DIR and PATH under /workspace) maps onto local, so the Outcome's own rule refuses run under Local on any host with rustup or a cargo home and the run part of acceptance 2 cannot pass as written"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 113
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "ESS first adds only local_reason, but a local record carries adapter and facts while ess/intake/domains/confinement.yaml:67-69 says both are absent on an unconfined run, so that comment must change in the ESS commit"
- file: .engineering/planning/story/child-launches-through-confinement-adapters.md
  line: 137
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "acceptance 7 runs two spawn_child children in two workspace parents at once while the Outcome keeps the running() guard that refuses opening a second adapter on the cgroup root during the first child, so the bullet must say the second parent's adapter opens before the first child starts or it fails by design"
- file: .engineering/planning/story/limit-hits-in-run-log.md
  line: 119
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story now lands while in-process linux stays on the host-driver route where settle reads exec.memory-limit, but its in-process kinds and the no-memory-line bullet are argued from the adapter route only the held story brings, and no edge orders them; rewrite those lines for the host-driver route or add the edge"
```
