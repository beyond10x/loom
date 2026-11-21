---
format: aep.planning-md/3
id: review-result:confinement-opens-from-engine-config-design-edge-check
kind: review-result
status: active
title: Plan critic design, on the edge from story:confinement-opens-from-engine-config to story:child-launches-through-confinement-adapters
relations:
- reviews: story:confinement-opens-from-engine-config
- reviews: story:child-launches-through-confinement-adapters
revision: 1
---
needs-revision

story:confinement-opens-from-engine-config — The edge is a real order edge as the story is written now, but it ties all nine stories to a Substrate blocker that concerns only the `linux` route. Narrow this story to engine configuration plus the `local` route on the contract, leave `linux` on today's host-driver route, and reverse the edge (`story:child-launches-through-confinement-adapters depends_on` this story) — .engineering/planning/story/confinement-opens-from-engine-config.md:51-52, :81-83, :102-103; `aep plan artifact show upstream-blocker:substrate-run-observation-names-the-limit`

story:confinement-opens-from-engine-config — Acceptance 1 puts every `confinement::open(` call in one constructor. A `linux` adapter is fixed when it opens to one workspace parent, one set of egress apertures and one certificate bundle, and today's runner opens one per (parent, egress) pair. So one engine-wide open cannot serve the delegated cases that bullet 4 keeps, and neither story says who changes the per-pair cache — .engineering/planning/story/confinement-opens-from-engine-config.md:79-80, :87-88; crates/loom-intake-slice/src/confinement.rs:116-124, :229, :321-359; ~/.cargo/git/checkouts/substrate-29ab43bbe6970bfc/adfde40/crates/substrate-host/src/confinement/linux.rs:67-72, :144, :342-349

**1. What the first story needs that only the second gives**
- Not the removal of `HostDriver::open`. It needs a launch route through `ConfinementAdapter` (`start`/`wait`/`output`/`write_stdin`/live events) behind `run_child`, `spawn_child` and the test runner's `run`, with the record built from `RunObservation`.
  - Which lines need it: story 1 Outcome :51-52 ("opens the adapter through `confinement::open`; the test runner, `run_child` and `spawn_child` then run under that adapter") and Acceptance :81-83 (all three run under `local`).
  - Who creates it: story 2 Outcome :42-45 ("launch through Substrate's confinement contract … and the adapter it selects").
- Today all three launches go through `self.driver()` to `HostDriver::open` (confinement.rs:490, :543, :993, :402).
- `settle` needs the driver's own record, with a cgroup (:828-833, :851-852).
- `ConfinedChild` calls `HostDriver` directly (story 2 Scope :130).
- `local` is not a `HostDriver`. It is a `ConfinementAdapter` returned by `local::open` (local.rs:89-96, mod.rs:159-168).
- So with the edge in place, story 1 is "engine config plus a second `AdapterRequest` on story 2's route". The Out of scope line :102-103 names the half it can do without, not the half it needs.

**2. If the edge is reversed (story 1 lands first)**
- Story 1, Outcome :51-52: false for `linux`. Launches run on the driver `SubstrateRunner::open` opens per (parent, egress) (:321-359, :402), not on the opened adapter.
- Story 1, summary :7 ("not a second code path") and Why :59-63: false. `local` becomes a contract route beside `linux`'s host-driver route, and the :402 problem the Why cites remains.
- Acceptance :79-80: either vacuous or harmful.
  - If the constructor does not open `linux`, the bullet must be limited to `Local`.
  - If it does open it, `linux::open` runs `HostDriver::open` (linux.rs:144). That is a second driver whose open ends every run under the cgroup root (confinement.rs:120-124), outside the `running()` guard (:392-401).
- Acceptance :81-83: still testable, but story 1 must build the adapter launch code itself (launch, stdin, live child, record from `RunObservation`, mod.rs:696-706). That is story 2's code without `linux`.
- Acceptance :87-88 ("each applied record names adapter `linux`"): already true today (confinement.rs:256), so it tests nothing.
- Story 2, Outcome :42-45: becomes "move `linux` onto the contract route story 1 built, and delete the host-driver route". The constructor and `EngineConfinement` join its Scope :133.
- Story 2, Acceptance :84-87: unchanged.

**3. Story 1 first, with launches still through `HostDriver::open`**
- Security regression: none.
  - `linux` launches keep every check (:475-551).
  - `local` is opt-in with a reason, nothing falls back to it (local.rs:8-10), and it replaces `none`, which was already unconfined (executor.rs:1163-1197).
- False claims meanwhile: Outcome :51-52 for `linux`, title :6 ("every child … runs under it"), and summary :7.
- One operational hazard, only if the constructor opens `linux` through the contract: it can end another runner's live launches on the same cgroup root (:120-124).

**4. Narrowing the first story**
- It would deliver:
  - `EngineConfinement` in ESS, the constructor, the refusals `LocalReasonMissing` and host policy, and the `--confinement local --local-reason` flags;
  - `run_child` (stdin included), `spawn_child` and `run` under `local` through `confinement::open(AdapterRequest::Local)`. `local` proves `stdin_pipe` and `output_live` (local.rs:3-5);
  - for `Linux`, the constructor wraps today's `SubstrateRunner` unchanged, and :402 stays.
- It would not deliver: `linux` through `confinement::open`, a single open point, or one internal launch route.
- `story:bound-commands-run-confined` can build on it. It reads `run_child` under `local` with a reason (bound-commands-run-confined.md:57, :95-97) and `run_child` under `linux` (:85-86). The second already works on the host-driver route.
- The blocker does not apply: its cause is lost limit reasons on the `linux` route (upstream-blocker § What is missing), and `local` serves no `limits` family (local.rs:3-5).

**5. Recommendation: narrow the first story and reverse the edge**
- The edge is a real order edge only because story 1 claims `linux` through the contract. The part the chain reads is `local` as a setting of the one runner, plus engine config, and that needs no `linux` adapter launch.
- Ledger Open 1 (e2r4-changes.md:182) and line 109 justify why story 2 comes before the tree-daemon epic, through story 1. They do not say why story 1 needs story 2.
- Narrowing frees seven stories from a release Substrate has not shipped.
- Story 2 shrinks to "`linux` joins the route, `HostDriver` goes". That is an expand-then-contract pair, each step leaves the tree green, not a split abstraction.
- Keeping the edge is right only if the requester accepts waiting on Substrate for the whole chain.

What I read: 4 artifacts in full (both stories, `story:bound-commands-run-confined`, the upstream blocker), the epic's story order, design reviews r1 and r2, e2r4-changes.md:109 and :180-185, confinement.rs:112-1007 and :3300-3360, executor.rs:1160-1197, and Substrate `adfde40` confinement `mod.rs`, `linux.rs` and `local.rs`. Commands: `aep plan artifact show/relations/graph/validate` (it prints `valid`). I walked 13 `depends_on` edges and 1 `blocks` edge around these stories, and went outside the set. I traced 3 acceptance reads to a producer: 2 recorded by an edge, the `linux` one already met by today's code.

Could not establish:
- How a `local` run times out. `local` has no limits, so Loom's own deadline is needed. Bound-commands' tests keep "the kill of the whole process group at the timeout" (bound-commands-run-confined.md:88-92). This is for the acceptance critic.
- Whether `story:bound-agent-node` and later stories read `spawn_child` under `linux` through the contract. I did not read them.

```findings
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 51
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its depends_on edge to story:child-launches-through-confinement-adapters is an order edge only because it claims linux through the contract, which holds all nine stories behind an upstream blocker about linux alone; narrow it to engine configuration and the local route on the contract with linux on the host-driver route, and reverse the edge"
- file: .engineering/planning/story/confinement-opens-from-engine-config.md
  line: 79
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "acceptance puts every confinement::open call in one constructor, but a linux adapter is fixed at open to one workspace parent and one egress and bundle set while the runner opens one per pair (confinement.rs:321-359), and neither story says who changes that cache"
```
