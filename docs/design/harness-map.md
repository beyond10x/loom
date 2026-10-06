# Harness → Loom module map

Where each crate of `beyond10x/harness` goes when Loom takes over the model/tool loop (Atlas ADR
0071). One row per crate under `crates/` at Harness `798325f0` (release 0.13.3). "Crate" is the
directory; "package" is the name in that crate's `Cargo.toml`, which is what `cargo tree` prints.
"Owns today" is Harness README § Layout at that revision. "Serves" names the responsibilities of
[`loom-design.md`](loom-design.md) § Owns the crate covers, or `none`.

Dispositions:

- `depend`: a git dependency at a pinned Harness revision, as Harness's current consumers already
  use it. "Loom target" names the revision; "licence" and "owner instead" are `—`.
- `port`: the source is carried into Loom at the path in "Loom target", relicensed Apache-2.0
  (operator, 2026-10-04). Harness keeps its own licence. "Owner instead" is `—`.
- `not carried`: Loom does not take the crate; "owner instead" names the one repository that keeps
  it, and "Loom target" and "licence" are `—`.

Harness is not changed by any of this. agentide, agent-platform, metaharness and uilab keep their
pinned Harness revisions until each moves (ADR 0071, operator decision 2026-10-04).
`story:harness-loop-port` acts on the `port` rows from `harness-wire` to `harness-loop`;
`story:session-transcript-streaming` acts on the `harness-cli` row, which ports one file of that
crate (see § Why the rows fall this way).

## Map

| crate | package | owns today | serves | disposition | Loom target | licence | owner instead |
|---|---|---|---|---|---|---|---|
| `harness-wire` | `b10x-harness-wire` | neutral values plus `ModelPort`, `ToolPort` and `BearerSource`; no I/O, no clock, no vendor field name | model API invocation; tool round trips; streaming; interruption/recovery | port | `crates/loom-executor/src/harness/wire/` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-http` | `b10x-harness-http` | the transport half of a wire: bounded SSE framing, the retry rule and its back-off, the witnessed sink, the status mapping and the one blocking `POST` | model API invocation; streaming | port | `crates/loom-executor/src/harness/http/` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-responses` | `b10x-harness-responses` | the Responses projection: its request body, its stream decoder, its three conversation headers | model API invocation; streaming | port | `crates/loom-executor/src/harness/responses/` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-messages` | `b10x-harness-messages` | the Messages projection: its request body, its content-block decoder, and the two header names one secret travels under | model API invocation; streaming | port | `crates/loom-executor/src/harness/messages/` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-loop` | `b10x-harness-loop` | the loop: turn assembly, tool round trips, approvals, budgets, cancellation; the four tools it owns itself (`answer`, `delegate`, `skill`, `recall`) and the hook port | prompt/context construction; model turn loop; tool round trips; compaction; turn/token/time/cost budgets; interruption/recovery; model-facing approval/suspension mechanics | port | `crates/loom-executor/src/harness/turn_loop/` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-cli` | `b10x-harness-cli` | the `b10x-harness` binary, the terminal approver, the hook runner, session transcripts and the environment block. Carried: `src/transcript.rs`, the cross-wire refusal of `open_session` (`src/lib.rs:2540-2550`) and `close_session` with `persist` (`src/lib.rs:2573-2614`) | session/transcript state | port | `crates/loom-executor/src/session.rs` | `LicenseRef-B10x-Proprietary` → `Apache-2.0` | — |
| `harness-credential` | `b10x-harness-credential` | credential sources that read exactly what a caller pointed them at; how a credential is presented belongs to the wire | model API invocation | not carried | — | — | `beyond10x/harness` |
| `harness-flow` | `b10x-harness-flow` | the workflow notation `workflow run` walks: a DAG of sub-trees, validated before anything runs, a group as a context scope, and a boundary a caller can refuse | none | not carried | — | — | `beyond10x/harness` |
| `harness-substrate` | `b10x-harness-substrate` | a client of the substrate wire: what this machine can confine, and the tools that answer | none | not carried | — | — | `beyond10x/harness` |
| `harness-tools` | `b10x-harness-tools` | one catalogue, published flat or under three verbs | none | not carried | — | — | `beyond10x/harness` |
| `harness-toolchain` | `b10x-harness-toolchain` | versioned declarative toolchain providers with read-only discovery and a typed argv compiler | none | not carried | — | — | `beyond10x/harness` |
| `harness-mcp` | `b10x-harness-mcp` | a reviewed `b10x-mcp` tool snapshot as a `ToolPort`, pinned by registry and snapshot digests | none | not carried | — | — | `beyond10x/harness` |
| `harness-app-server` | `b10x-harness-app-server` | the Codex-format JSON-RPC server, and the wire-backed `ToolPort` | none | not carried | — | — | `beyond10x/harness` |
| `harness-xtask` | `b10x-harness-xtask` | the executable gate and the independent provider and CLI contract validators | none | not carried | — | — | `beyond10x/harness` |

## Why the rows fall this way

- **No `depend` row.** Every Harness crate except `harness-toolchain` reaches `harness-wire`
  (each crate's `Cargo.toml` at `798325f0`): `harness-xtask` through `harness-cli`, every other one
  directly. Once `harness-wire` is ported, depending on any of them would put Harness's
  `ModelPort`, `ToolPort` and `Item` into `b10x-loom-executor` beside Loom's own copies: two incompatible
  sets of the same types. `story:harness-loop-port` acceptance 5 forbids it, since
  `cargo tree -p b10x-loom-executor` may name no package this map marks `port`. `harness-toolchain` has no
  such dependency but serves no § Owns responsibility.
- **The ported set is closed.** `harness-http`, `harness-responses` and `harness-messages` depend
  only on `harness-wire` and `harness-http`; `harness-loop` only on `harness-wire`;
  `src/transcript.rs` of `harness-cli` only on `harness-loop` and `harness-wire`. Nothing ported
  needs a crate this map leaves behind. The two `harness-cli` functions below are carried as
  behaviour, not verbatim: they take the command line's `RunOptions` and `Prepared`, which stay
  behind.
- **`harness-loop` lands in `turn_loop/`.** `loop` is a Rust keyword, so a module of that name
  would need a raw identifier at every use.
- **`harness-cli` is ported in part.** Loom is driven through Commission's `AgentExecutor`, not a
  command line, so the binary, the terminal approver, the hook runner and the workspace environment
  block stay in `beyond10x/harness`. Three pieces are carried into `crates/loom-executor/src/session.rs`,
  the file `story:session-transcript-streaming` names, because that story keeps their behaviour
  (all at `798325f0`):
  - session filing and resume by id: `src/transcript.rs` (`Session` line 47, `save` line 184, `load` line 219);
  - the refusal, before any request is sent, of a session recorded on another wire, naming both
    wires: the wire check of `open_session`, `src/lib.rs:2540-2550`;
  - filing the session however the run ended, answered or died: `close_session`,
    `src/lib.rs:2587-2614`, and the `persist` it calls, `src/lib.rs:2573-2581`.
- **Credentials stay with the embedder.** Loom does not own connector credentials
  (`loom-design.md` § Does not own; `AGENTS.md` § Boundary). The ported `harness-wire` carries the
  `BearerSource` trait (`src/bearer.rs:58`); the embedder hands Loom a source, and Loom reads no
  credential file and no credential environment variable itself. (The ported `transcript.rs` does
  read `XDG_STATE_HOME` and `HOME`, at lines 356 and 369, to place the session directory.)
- **Tools, substrate and MCP are not Loom's.** `harness-tools`, `harness-substrate` and
  `harness-mcp` each supply a toolset for the loop to publish, and run its calls. In Loom the
  model-visible catalogue is projected from the current frontier, not registered at startup (ADR
  0072), and a selected action is executed by a trusted adapter after Commission revalidates it
  (`loom-design.md` § Commission integration). Loom has no place for a tool source of its own or
  for running an effect, so these crates stay in Harness, which still serves their consumers.
- **Flows, toolchains, the app-server and the gate are not Loom's either.** Loom serves one bounded
  run (`loom-design.md` § Purpose); a `harness-flow` document walks several sessions above that.
  Toolchain providers for Rust, Go, Taskfile and npm are engineering-domain semantics, which
  `loom-design.md` § Does not own excludes. Loom is driven through `AgentExecutor`
  (`AGENTS.md` § Boundary), not `harness-app-server`'s JSON-RPC. `harness-xtask` is Harness's own
  gate.

## Seams a port reuses

All at Harness `798325f0`; a port keeps each one's shape so the loop that calls it needs no
rewrite.

- **`ModelPort`** (`harness-wire/src/port.rs:91`): one documented model API; `turn` runs exactly
  one turn into a `StreamSink`. The two provider projections implement it.
- **`ToolPort`** (`harness-wire/src/port.rs:135`): where the loop's tools come from; `specs` is the
  complete set published for the next turn. In Loom it publishes the turn's catalogue and runs
  nothing: every published tool asks first (see the approval seam below), and Loom's answer is
  the selection, argument generation and revalidation a call goes through before it becomes a
  `ProposedAction` (`crates/loom-executor/src/harness/governed.rs`).
- **`TurnEnvironmentProvider`** (`harness-loop/src/environment.rs:47`): refreshed before every
  turn; a snapshot can narrow the attached `ToolPort`, never widen it. `story:frontier-projection`
  feeds the projected catalogue through it and `story:harness-loop-port` wires it.
- **The approval checkpoint** (`harness-loop/src/approval.rs`): `ApprovalPort::decide` (line 43)
  returns `ApprovalDecision::Deferred { checkpoint_id }` (line 13); the loop captures an
  `ApprovalCheckpoint` (`harness-loop/src/lib.rs:223`) immediately before the gated effect and
  continues it through `AgentLoop::resume_approval` (`lib.rs:2181`). This is Loom's model-facing
  approval suspension.

## New in Loom

No Harness crate covers these § Owns responsibilities; Loom builds them.

- dynamic model-visible action catalogue — projected from the governed frontier per turn
  (`story:frontier-projection`); Harness publishes the toolset attached at startup, which a
  `TurnEnvironmentProvider` can only narrow.
- action selection strategy — a selector choosing only among admissible candidates
  (`story:action-selector`); in Harness the model's tool call is the choice.
- action argument generation — filling the selected action's schema (`story:argument-generator`);
  in Harness the model writes the arguments within the tool call itself.
