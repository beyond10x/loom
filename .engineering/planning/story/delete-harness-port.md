---
format: aep.planning-md/3
id: story:delete-harness-port
kind: story
status: implemented
title: Loom deletes its port of harness and releases the break
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:chooser-executor-for-plugin-turns
- depends_on: story:judgement-step-any-executor
- depends_on: story:metering-on-llm-model
- depends_on: story:authority-sees-the-request
- depends_on: story:agent-executor-host-port
- depends_on: story:executor-reports-usage
- depends_on: story:governor-exposes-canon-decision
- depends_on: story:sdk-confines-any-child
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-executor/src
- confidence: cited
  path: crates/loom-executor/tests
- confidence: cited
  path: crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: crates/loom-sdk/src/llm.rs
- confidence: cited
  path: crates/loom-sdk/tests/harness_paths.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: crates/loom-sdk/tests/llm_model_port.rs
- confidence: cited
  path: crates/loom-sdk/tests/sdk_model_ports.rs
- confidence: cited
  path: ess/domains/metering.yaml
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/guides/embed-the-runtime.md
- confidence: cited
  path: website/docs/guides/move-to-loom.md
- confidence: cited
  path: website/docs/reference/crates.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T00:00:05Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-10-21T00:00:05Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "active", to: "implemented", at: "2026-10-21T12:21:00Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"review_outcome":7}}}
---
## Outcome

`crates/loom-executor/src/harness/` and its port test suites are deleted, together with what only
the port used: compaction, the session transcript, interruption recovery, the wire credentials and
the two SDK model ports built on it (`loom_sdk::llm::LlmModelPort`, `crates/loom-sdk/src/llm.rs`;
`ClaudeCodePort`, `crates/loom-executor/src/wire/claude_code.rs`). The release that ships it is
breaking: its `CHANGELOG.md` entry names, under `Breaking:`, the eight ports the stability page
lists from `loom_sdk::loom::harness` (`ModelPort`, `ToolPort`, `BearerSource`, `StreamSink`,
`ApprovalPort`, `TurnEnvironmentProvider`, `HookPort`, `LoopSink`;
`website/docs/reference/sdk-stability.md:52-79`), the other `loom_sdk::loom::harness` paths the page
names, and the two model ports. The README sentence that Loom "speaks no provider wire"
(`README.md:33-34`) is then true.

## Why

ADR 0098 point 4. The port is 26,826 lines (2026-10-20) with no caller outside Loom; its callers
inside Loom moved in the stories this one depends on. Claude Code is reached through metaharness
(ADR 0096), not through a `claude -p` model port.

## Acceptance

- `crates/loom-executor/src/harness/`, `crates/loom-executor/src/wire/`,
  `crates/loom-executor/src/{compaction,credentials,recovery,session}.rs` and
  `crates/loom-sdk/src/llm.rs` do not exist, and `git grep -n 'harness::' crates/` prints nothing
  (`crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs` included).
- The suites that test only the port are deleted (`adversary_harness_port_boundaries.rs`,
  `adversary2_harness_port.rs`, `harness_loop_port.rs`, `harness_port.rs`, `harness_port_contract.rs`
  and the others `git grep -l 'harness::' crates/loom-executor/tests` lists once the stories this one
  depends on have landed); a suite that tests workflow, metering or selection behaviour was migrated
  by the story that owns it and is not deleted here. The frontier projection keeps a test that
  holds `projection_follows_frontier`.
- `crates/loom-sdk/tests/host_ports.rs::the_stability_page_lists_exactly_the_ports_this_test_implements`
  passes with the eight rows gone.
- For each of the eight ports and the two model ports above, and for each `loom_sdk::loom::harness`
  path the stability page lists at the previous release tag (`git show <tag>:website/docs/reference/sdk-stability.md`;
  32 lines today, the ports table and `:143-163`), `grep -F` finds the name in the release's
  `CHANGELOG.md` entry, after `Breaking:`; `loom-xtask release-check` refuses `Breaking: none.` for it.
- The guides that describe the port describe llm's `Model` instead:
  `git grep -nE 'harness::|MeteredModelPort|ModelPort' website/docs/guides ess/domains/metering.yaml website/data/status.json`
  prints nothing.
- `git grep -nE 'harness::|ModelPort|Port its implementation|beyond10x/harness. is the predecessor' AGENTS.md README.md`
  prints nothing.
- `task consumer-check` passes: the documented embedding uses no removed path.
- `task check` and `task website` exit 0.

## ESS first

Declarations only the port used are removed from `ess/` first (inferred candidates: `loom.run`'s
`WireCredential` and the loop-stop types in `ess/domains/run.yaml`; Commission's `LoopBudget`
suspension reason, `ess/commission/domains/responsibility.yaml:135`, stays unless no executor can
raise it). The red step is `task drift` over the regenerated code, failing until the code that used
them is gone.

## Depends on

`story:chooser-executor-for-plugin-turns`, `story:metering-on-llm-model`,
`story:judgement-step-any-executor`; and, because they edit `host_ports.rs`, `crates/loom-sdk/src/lib.rs`,
the stability page or `metering.rs` first, `story:authority-sees-the-request`,
`story:agent-executor-host-port`, `story:executor-reports-usage`,
`story:governor-exposes-canon-decision` (after `story:engineering-protocols-090`, which changes the
page's pin table) and `story:sdk-confines-any-child`.

## Not in scope

- Changing harness, which ADR 0096 keeps as the in-house harness behind metaharness.
- The session executor (`story:metaharness-session-executor`).

## Scope

Crates: `loom-executor`, `loom-sdk`, `loom-governor` (test only), `loom-docs`.

- `crates/loom-executor/src/harness/`, `crates/loom-executor/src/wire/`,
  `crates/loom-executor/src/{compaction,credentials,recovery,session,lib}.rs` (deleted or edited)
- the port's suites under `crates/loom-executor/tests/`,
  `crates/loom-governor/tests/adversary_w8_arbitrary_precision.rs`
- `crates/loom-sdk/src/llm.rs`, `crates/loom-sdk/src/lib.rs`, `crates/loom-sdk/tests/harness_paths.rs`,
  `llm_model_port.rs`, `sdk_model_ports.rs`, `host_ports.rs`
- `ess/domains/run.yaml`, `generated/rust/loom/` (regenerated)
- `website/docs/reference/sdk-stability.md`, `website/docs/reference/crates.md` (regenerated),
  `website/docs/guides/embed-the-runtime.md`, `website/docs/guides/move-to-loom.md`,
  `ess/domains/metering.yaml` (a comment line, `:3`),
  `README.md`, `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`

`README.md` and the loom-sdk files are also edited by `story:metaharness-session-executor`, which
depends on this story.

## Shipped

Merged into wave 2026-10-21-w5 as 6855dbaf, after two adversary passes (review-result:adv-w20261021w5-u2-pass-1, -pass-2). The design documents moved with it as story:design-docs-after-the-port (00d8fdff). After the release commit moved the removals into the 0.31.0 entry, d6561144 makes the pass-2 test read the Breaking bullets of every CHANGELOG entry.
