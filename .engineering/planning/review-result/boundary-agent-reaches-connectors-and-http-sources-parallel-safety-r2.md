---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-parallel-safety-r2
kind: review-result
status: active
title: Plan critic parallel-safety, round 2, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:connectors-relay-client
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
approve

Nothing I found would change what was drafted. The set's one unordered pair, stories 1 and 2, shares only `CHANGELOG.md` and different sections of `AGENTS.md`, and the epic says so.

**What I read.**
- 5 artifacts: `aep plan artifact show` on the epic and the four stories, `graph --format json`, and `waves`. `waves` lists the stories unassessed, because no scope entries exist yet.
- The round-1 verdict and `e6r1-changes.md`.
- Source at the tree's HEAD `61fd500b`. I opened:
  - `admission.rs:1355-1365`, `:1470-1482` and `:1600-1618`, and `cluster_service` at `:1203`
  - `render.rs:1176-1208`
  - `crates/loom-conformance/src/lib.rs:240-262`
  - `crates/loom-connectors/src/lib.rs:70` and `:205-230`
  - `tests/render.rs:1834-1868`
  - the `Cargo.toml` files of `loom-agent`, `loom-conformance`, `loom-plugin`, `loom-plugin-slack`, `loom-cli`, `loom-boundary` and `loom-connectors`
  - the `AGENTS.md` headings, and `CHANGELOG.md`

**Round-1 fixes of my charter.** All four hold in the stored bodies and in the graph.
- Stories 3 (shape) and 4 (agent selection) now have edge 4→3. Its reasons are `ess/ess-inputs.yaml`, the command match at `lib.rs:245-256`, the `fake-connectors` fixture and `AGENTS.md § Connectors`. Story 3 (`:62-65`) and story 4 (`:194-199`) name each one, and so does the epic (`:75-79`).
- The epic's one-wave sentence is gone.
- The egress story's Files now list `ess/ess-inputs.yaml` (`boundary-admits-declared-http-egress.md` `:293`).
- The end-to-end assertion is owned by story 4: `an_auto_run_sends_its_reply_through_the_relay_in_the_declared_shape`, in the new `crates/loom-agent/tests/connectors_relay.rs`. That file does not exist yet and only story 4 creates it. The loom-connectors file of the same name is a different path.
- Story 4's describe schema is now stated, and edge 4→3 covers the fixture it reads.

**The unordered pair, stories 1 and 2.**
- Story 1 lands on `ess/`, `generated/rust/loom/`, `loom-operator`, `charts/`, `Taskfile.yml` and `operator-cli.md`. It adds no crate dependency, so it touches no `Cargo.lock` (cited).
- Story 2 lands on `loom-connectors` (new `relay.rs`, `Cargo.toml`, `Cargo.lock`), `loom-sdk/src/lib.rs` and `sdk-stability.md`. It has no ESS change.
- The shared files are `CHANGELOG.md` (story 1 under `### Changed`, story 2 under `### Added`) and `AGENTS.md` (§ Operator and § Resource against § Connectors, headings at `AGENTS.md:1155`, `:965` and `:528`). The epic names both (`:84-85`).

**Surfaces.**
- 4 stories placed cited, 0 inferred, 0 unplaced. The epic holds none of its own.
- Sub-surface still inferred: the conformance `Cargo.toml` dependency (the body marks it inferred).
- Acceptance reads traced to a producer in the set: 5. Edges record 5:
  - story 3 reads story 2's relay write and `tests/connectors_relay.rs` (3→2)
  - story 4 reads story 2's `ConnectorsRelay` (4→2)
  - story 4 reads story 3's shaped write and describe fixture (4→3)
  - story 4 reads story 1's admission, scenarios and `render.rs` (4→1)
  - story 3's `ess/ess-inputs.yaml` and plan-file collision with story 1 (3→1). This one is a shared file, not a read.

**What I could not establish.**
- Whether `generated/rust/loom/plan.json` is written when only comments change. This no longer matters, because story 2 has no ESS change.
- Whether story 1's new golden `credential-free-http-egress.yaml` is built from the `render.rs` helpers that story 4 changes (drops the `runtime.connectors` line). Edge 4→1 orders the two either way. If it is, story 4 may have to re-bless a file that is not in its Files list.

**The egress option (a), reasons for the operator (the brief asks for them).**
- For:
  - The default list is empty, and each opening is one operator-listed entry.
  - Condition 1 (`boundary-admits-declared-http-egress.md:143-145`) now covers every allowed upstream's namespace, so it keeps the fix `Reason::BoundaryEgressNamespace` made at `admission.rs:1473-1480`.
  - Option (b) lets through the same traffic. A NetworkPolicy selects pods, not containers, and the relay adds no token.
- Against:
  - The destination is admitted by network position alone. The refusal's own detail says a boundary resource "reaches only DNS and its boundary's upstreams", and this entry is an exception to that.
  - The opening is pod-wide and layer 4. Only `HttpGet` in process stops a write on the same port, and the sidecar gets the opening too.
  - Condition 3 ties an entry to a binding by namespace only. The story states this (`:166-170`), so the operator's list is the whole gate.
- The invariant is a failing test in stories 1 to 3:
  - Story 1: `an_http_egress_adds_no_credential_variable_or_volume` (`:267-270`).
  - Story 2: `the_relay_client_sends_no_credential`.
  - Story 3: `the_shaped_relay_write_sends_no_credential_and_starts_no_process`.
- Story 4 has `the_upstream_sees_only_the_boundarys_token` and `no_connectors_program_is_started_under_the_relay`.
- The operator should decide with the three exposures in front of them. `e6r1-changes.md` § Open 5 lists them.

**Out of my lane, which does not set the verdict.**
- Acceptance and scope, story 4:
  - `PluginConfig.connectors` becomes optional, which changes how it is constructed at `crates/loom-plugin-slack/src/config.rs:341`, `crates/loom-plugin-slack/tests/support/mod.rs:161`, `crates/loom-plugin/tests/support/mod.rs:134` and `held_record.rs:14`. None is in Files, yet `tests/poll.rs` is listed "unchanged".
  - `crates/loom-agent/Cargo.toml` has no `b10x-loom-boundary` dev-dependency, and the Files list omits it.
  - `credential_free_agent` asserts `agent.lines().count() + 4` against the example (`tests/render.rs:1848-1851`), which dropping a line breaks.
- Scope, story 3: Files omit `crates/loom-connectors/src/lib.rs`, which `loom_connectors::shape_write_input` needs. Stories 2 and 4 edit that file and are ordered with story 3, so this is not a collision.
- Design: story 2 has no ESS change, so its red commit is a test file. The rule is in `AGENTS.md § ESS`, which I did not re-read for this.

```findings
[]
```
