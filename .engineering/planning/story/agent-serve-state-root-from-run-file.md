---
format: aep.planning-md/3
id: story:agent-serve-state-root-from-run-file
kind: story
status: implemented
title: b10x-loom agent serve takes its state root from the run file's runtime.state_dir when --state is absent
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:16:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-27T12:16:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-27T12:33:29Z", actor: "human:timo", revision: 4, decided_on: {"asserted":{"test_result":1}}}
---
## Outcome

`b10x-loom agent serve` and `agent resume` take their state root from the run file's
`runtime.state_dir` when `--state` is absent. `--state` still wins when given. The host's cursors,
`record.jsonl`, `sends.jsonl`, cases, Suspensions and grants then land under the directory the
document names, the same root a delivery program reads from the same document (takt writes its host
records under its state directory, `story:the-state-directory-holds-every-host-record` in
integration/takt).

## Why

The orchestrator, 2026-10-27 (decision D90), from the takt runtime-data assessment, story 6:
`loom.agent/1` already declares `runtime.state_dir` and Loom decodes it, but `agent serve` read only
`runtime.connectors` and took its root from `--state`. A document that names one state directory
for the run therefore gave the agent server no way to find the records takt writes there.

## Acceptance

- `serve_takes_its_state_root_from_the_run_file`: `agent serve --once` given no `--state` runs a
  cycle and writes `record.jsonl` and the held send under the spec's `runtime.state_dir`; the
  proposal line printed is the one `--state` gives (`crates/loom-cli/tests/agent_serve.rs`).
- `an_explicit_state_wins_over_the_run_file`: with both, the records land under `--state` and
  `runtime.state_dir` is not created.
- `resume_takes_its_state_root_from_the_run_file`: `agent resume` given no `--state` performs the
  held send of a host that served from the document's `runtime.state_dir`.
- `a_blank_runtime_state_dir_is_refused_whether_or_not_state_is_given`: a blank `runtime.state_dir`
  is refused by the spec check (`BlankText`, exit 3) and starts no read; the root is never blank.
- `agent serve --help` and the CLI reference state the default; `task docs-check` is green.
- The ESS contract is unchanged in shape: `loom.agent.RuntimeSpec.state_dir` stays a required
  `String`; only its documented meaning gains the default, and `ess specify validate` stays green.

## Scope

- `crates/loom-cli/src/lib.rs` (`AgentServeArgs`, `AgentResumeArgs`, help text) and
  `crates/loom-cli/src/main.rs`, `crates/loom-cli/src/agent.rs` (the root's resolution).
- `ess/domains/agent.yaml` (the comment on `RuntimeSpec`), `website/docs/reference/cli.md`.

A relative `runtime.state_dir` is relative to the working directory, as a relative `--state` is.
`approve` keeps its required `--state`; it is not part of this story.
