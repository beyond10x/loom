---
format: aep.planning-md/3
id: approval-record:loom-kubernetes-filing
kind: approval-record
status: draft
title: Filing the Kubernetes design's milestones M0-M3 and M5; the orchestrator's answers to its questions
relations:
- decides: epic:loom-sdk-for-products
revision: 1
---
# Filing the Kubernetes design's milestones under epic:loom-sdk-for-products

Session `b10x-chef-orchestrator` decided, in these words (2026-10-22):

> "New design for you: ~/.cache/b10x-layers/design-2026-10-22/loom-kubernetes.md (Timo's ask in
> timo-2026-10-22-operator-agent.md). It covers a spec-driven agent run on Kubernetes plus a first
> operator agent. My decisions: Q1 A (Loom only projects; `ess` builds and pushes; CI applies, as
> your AGENTS.md:1130 requires), Q2 A if Connectors supports standing scoped approval, else C, Q4 C,
> Q5 A, Q6 A, Q7 A, Q8 A, Q9 C. Name: `b10x-loom agent {check, serve, project}`, ESS domain
> loom.agent, merged with the loom-sdk Q1 AgentSpec, not a second domain. File M0, M1, M2, M3, M5
> as draft stories under epic:loom-sdk-for-products. Check the design's file:line claims against
> your code. Then propose one SDK wave covering both designs. Nothing gets deployed today. Record
> this in your store."

| Q | Decision | Where it lands |
|---|---|---|
| Q1 | A: Loom projects, `ess` builds and pushes, CI applies | `story:agent-project-kubernetes` |
| Q2 | A if Connectors has standing scoped approval, else C; resolved C (D2) | `story:agent-serve` |
| Q3 | not answered; the design recommends a Connectors file-backed custody, outside Loom | none |
| Q4 | C: plain manifests now, ESS chart later | `story:agent-project-kubernetes` |
| Q5 | A: `emptyDir`, cursors from `lookback_minutes` | `story:agent-project-kubernetes` |
| Q6 | A: no confinement of the `connectors` child, `runtime.confinement: none` | `story:agent-domain`, `story:agent-check` |
| Q7 | A: Loom adapter `loom-a2a` | `story:agent-a2a` |
| Q8 | A: model id `claude-haiku-5-5` as given; M1's manual run checks it | `story:agent-serve` |
| Q9 | C: classifier optional, absent threshold means one state | `story:agent-domain` |

## Decisions taken while filing

- D1 `loom.agent` is one domain: `story:agent-domain` (SDK Q1) now carries the `loom.agent/1`
  document's fields; M0's "ESS domain, generated types" is that story, and M0's check, example and
  protocol are `story:agent-check`.
- D2 Q2 resolves to C. `connectors approvals issue --help` (connectors 0.54.0 installed) says
  "Approve the exact reconstructed subject" and requires one `--input-json`, `--input-file` or
  `--input-stdin`: a proof covers one write's input. `auto` mode waits on a Connectors story, which
  is not filed here.
- D3 M2 depends on M1 (it dispatches inside the serve effects); M5 on M1 and `story:runtime-events`.
- D4 Gap: Loom publishes no release binary (`release.yml` creates no artifact), which both the
  image's `bins` node (M3) and `story:confine-an-agent-tutorial` need. No story filed for it.

## Claim check against Loom at `c5228ee0`

| Design claim | Result |
|---|---|
| `crates/loom-plugin/src/lib.rs:218-306,333-352` (`Plugin`, `run_plugin`) | holds |
| `crates/loom-plugin-slack/src/lib.rs:148-176` implements `name, poll, classifier, turn_model` | holds |
| `walk.rs:5-8`: every member channel walked, `channels` only weights | holds (`walk.rs:1-9`) |
| `protocols/inbound-answer/1.yaml:7-8` sends nothing | holds |
| `crates/loom-connectors/src/cli.rs:1-4,119,193` | holds (`ConnectorsCli` :120, `invoke_read` :194) |
| `authority.rs:14-20`, `effects.rs:76-109,186,354-373` | holds |
| `crates/loom-protocols/src/lib.rs:106-126` (`plugins()`) | holds |
| `ess/domains/plugin.yaml:81-121`, `datasource.yaml:8-11,77-91`, `models.yaml:21-42` | holds |
| `crates/loom-cli/src/lib.rs:217-236,241-256,327-353` | holds |
| `crates/loom-executor/src/workflow.rs:220` (`load`) | holds |
| CHANGELOG 0.30.0 `RunMeter`/`RateCard` | holds (lines 173, 182) |
| `loom/AGENTS.md:1130` and the other AGENTS.md lines (570-660, 1093-1105) | content holds; lines moved: the Never rule is at 1154, `<earlier-steps>` 586, `TURN_STEP_BUDGET` 591, the one-host lock 622, SIGTERM 652, Laya 658, Releases 1093 |
| § 6 "the refusal is final (`EffectOutcome::Refused`)" | needs `EffectOutcomeRefused.ends_run: true`; `story:agent-serve` says so |

Claims about other repositories (llm, ess, connectors, a2a, substrate) were not checked here.

Nothing is implemented and nothing is deployed.
