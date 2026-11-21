---
format: aep.planning-md/3
id: review-result:confinement-opens-from-engine-config-acceptance-c1
kind: review-result
status: active
title: Plan critic acceptance, on the narrowed story:confinement-opens-from-engine-config and story:child-launches-through-confinement-adapters
relations:
- reviews: story:confinement-opens-from-engine-config
- reviews: story:child-launches-through-confinement-adapters
revision: 1
---
needs-revision

story:confinement-opens-from-engine-config — The Outcome doesn't say how the test runner's `run` builds its command under `local`. Today `run` always adds read-only toolchain and cache roots mounted under `/toolchain/...`, and sets PATH and CARGO_HOME to point at those mounts. By the story's own rule `local` refuses those roots (`filesystem_view`), and the paths don't exist on the host. So bullet 2's `run` case either gets refused or passes only because the roots were silently dropped — .engineering/planning/story/confinement-opens-from-engine-config.md:66-69, :124; crates/loom-intake-slice/src/confinement.rs:994-997, :3164-3209 (compare `run_bounded`, executor.rs:1171)
story:confinement-opens-from-engine-config — The Outcome says read-only roots and files are refused under `local` (`filesystem_view`), but no acceptance bullet checks that refusal. Only egress has a bullet — .engineering/planning/story/confinement-opens-from-engine-config.md:66-69, :139
story:confinement-opens-from-engine-config — The Outcome never says what `local` does with a child's default no-network request (`NetworkMode::None`). `local` can't keep it because it proves `host_network`. Admission refuses it if Loom sends it as `Isolated`. Bullets 2 and 3 quietly assume it is dropped — .engineering/planning/story/confinement-opens-from-engine-config.md:66-69; crates/loom-intake-slice/src/confinement.rs:1091; substrate local.rs:118, mod.rs:616-629
story:confinement-opens-from-engine-config — The deadline bullets cover `run_child` and `spawn_child` but not the test runner's `run`. The Outcome puts `run` under `local` too and says Loom holds every `local` deadline — .engineering/planning/story/confinement-opens-from-engine-config.md:71, :130-135
story:confinement-opens-from-engine-config — The bullet "finds no member naming an adapter or a reason in `ConfinementSpec`" is already true today. It reads the same before the story as after — .engineering/planning/story/confinement-opens-from-engine-config.md:149-150; ess/intake/domains/confinement.yaml:132-147
story:confinement-opens-from-engine-config — The refusal bullet holds two outcomes. Its host-policy half says only "naming the policy" and names no refusal variant. Substrate refuses with `LocalRefused` / `confinement.local-refused`, and Loom's `ConfinementRefusal` has no such variant. The body doesn't adopt the § Open 2 default (`CapabilityUnserved`) — .engineering/planning/story/confinement-opens-from-engine-config.md:136-138; ess/intake/domains/confinement.yaml:12; substrate mod.rs:227, :609-610
story:confinement-opens-from-engine-config — The body doesn't say which `Backend` a `local` record carries. Its own citation points to a record with `backend: Backend::None`, while the § Open 1 default is `Substrate`. The `confinement:` line is printed from `backend_name`, which can only print `substrate` or `none`, so the CLI bullet's "names `local`" needs a decision the body doesn't make — .engineering/planning/story/confinement-opens-from-engine-config.md:60, :155; crates/loom-intake-slice/src/executor.rs:1180, :1202-1207; crates/loom-intake-slice/src/intent.rs:711, :860
story:confinement-opens-from-engine-config — The CLI bullet joins five outcomes in one bullet. It also reads a clock query's `confinement:` line that no code prints today: the runner is built, and the line printed, only for `software-change@1`, and an existing test asserts that a clock query prints no `confinement:` line — .engineering/planning/story/confinement-opens-from-engine-config.md:151-157; crates/loom-cli/src/main.rs:1143-1151; crates/loom-cli/tests/adversary_slice_run.rs:639
story:confinement-opens-from-engine-config — Bullets at :124, :134, :136, :139, :145 and :147 name no test. Bullet :124 also joins two outcomes (the write succeeds; the record names `local` and the reason), and :145 joins three — .engineering/planning/story/confinement-opens-from-engine-config.md:124-148
story:child-launches-through-confinement-adapters — Acceptance 7 (two `spawn_child` children in workspaces of two parents run at once) is already true today, because today's runner opens one driver per parent. It reads the same before the story as after — .engineering/planning/story/child-launches-through-confinement-adapters.md:137-138; crates/loom-intake-slice/src/confinement.rs:116-124, :321-359

What I read: 2 of 2 ids in full, with `aep plan artifact show` on each, plus `story:bound-commands-run-confined`. I also read the ledger and design check, ran `aep plan artifact validate` (prints `valid`), and read confinement.rs, executor.rs, child.rs, main.rs, intent.rs, adversary_slice_run.rs and the ESS domain. In Substrate at `adfde40` I read `local.rs` in full and `mod.rs` and `profile.rs` in part.

What checked out:
- **The kill claim holds.** `local`'s `signal` sends SIGKILL to `-pid` while the leader is unreaped (local.rs:437-452). That is the same reach as `kill_group`'s `killpg` while the leader is unreaped (executor.rs:1402-1415): every process still in the group, none that left it.
- **Blank reason and host policy at construction both hold** (profile.rs:234-238; local.rs:92-99, :117).
- **No `linux` sentence in the narrowed story is false**, and it claims nothing only the second story delivers.
- **`story:bound-commands-run-confined` has what it needs.** Everything it reads exists after the narrowed story: `run_child` under `local` with a reason, `run_child` under `linux`, and the process-group kill at the deadline.
- **Second story:** its body agrees with the reversal, it owns the per-key `linux` open, and it waits on the first story. The edge graph has no cycle.
- **§ Open 1 and § Open 2:** no source contradicts either default. The two related findings above are about the body never stating the choice.

What I could not establish:
- Whether `ChildResult.driver_record` (a non-optional `substrate_wire::AppliedConfinement`, child.rs:70) can be filled for a `local` `spawn_child`. If not, a public type has to change and the story doesn't say so. This is design, out of my lane.
- How bullet :121 keeps the test file itself out of its own grep: `engine_confinement.rs` sits under `crates/` and has to contain `AdapterRequest::Linux` as its search pattern.
- Setting `SUBSTRATE_CONFINEMENT_LOCAL` inside a test binary that runs other `local` opens in parallel may be racy (edition 2024, `set_var`).
- Out of my lane (design / parallel-safety): some story text still describes the old order.
  - `story:substrate-daemon-runner` :86-89 and :115-118, and `epic:tree-daemon-under-enforced-limits` :61-65, still say they come after the second story.
  - `story:connectors-relay-client` :364 says the first story depends on the second. The ledger's § Open 5 list misses this one.
  - None of their acceptance bullets reads anything that won't exist when they land.

```findings
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 124
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome does not say how the test runner's run builds its command under local: today it always adds read-only toolchain and cache roots mounted under /toolchain and points PATH and CARGO_HOME at them (confinement.rs:3164-3209), which the story's own rule refuses under local and which do not exist at host paths, so bullet 2's run case is refused or passes only by silently dropping them"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 66
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome refuses read-only roots and files (filesystem_view) under local, but no acceptance bullet checks that refusal; only egress has one"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 67
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome does not say what local does with a child's default no-network request (NetworkMode::None), which local cannot keep and which admission refuses if sent as Isolated (mod.rs:616-629); bullets 2 and 3 assume it is dropped"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 130
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the deadline bullets cover run_child and spawn_child but not the test runner's run, which the outcome also puts under local with Loom holding its deadline"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 149
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet that finds no adapter or reason member in ConfinementSpec is already true today (confinement.yaml:132-147), so it reads the same before the story as after"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 136
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the refusal bullet holds two outcomes, and its host-policy half names no refusal variant ('naming the policy'), while Substrate refuses LocalRefused and Loom's ConfinementRefusal has no such variant; the body does not adopt the Open 2 default"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 60
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body names no Backend for a local record; its cited record has Backend::None (executor.rs:1180) against the Open 1 default Substrate, and the confinement: line comes from backend_name, which prints only substrate or none, so the CLI bullet's 'names local' rests on an undecided choice"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 151
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the CLI bullet joins five outcomes and reads a clock query's confinement: line that no code prints today (runner and line only for software-change@1, main.rs:1143-1151; asserted absent at adversary_slice_run.rs:639)"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 124
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "bullets at :124, :134, :136, :139, :145 and :147 name no test, and :124 and :145 each join two or three outcomes"
- file: .engineering/planning/story/child-launches-through-confinement-adapters.md
  line: 137
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "acceptance 7 (two spawn_child children in workspaces of two parents run at once) is already true today, since the runner opens one driver per parent (confinement.rs:321-359), so it reads the same before the story as after"
```
