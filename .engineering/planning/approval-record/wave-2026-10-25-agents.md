---
format: aep.planning-md/3
id: approval-record:wave-2026-10-25-agents
kind: approval-record
status: draft
title: 'Wave 2026-10-25-agents: agent-serve, agent-http-binding, agent-a2a under the operator''s standing word'
tags:
- wave
relations:
- decides: story:agent-serve
- decides: story:agent-http-binding
- decides: story:agent-a2a
revision: 1
---
# Wave 2026-10-25-agents: agent-serve, agent-http-binding, agent-a2a

Skill: aep:implementing 0.24.1, wave mode. Coordinator: the orchestrator session (b10x-chef-orchestrator). Run kind: interactive operator present, with standing approval: Timo, 2026-10-25: "you will now implement the design about deployable loom agents"; "use /aep:implementing for implementing stuff, use up to 3 sub-agents, move in bigger batches of work, use full gates wisely". This record is the stage-1 proposal accepted under that standing word; the page is kept current through stage 2.

## Units, in order (one integration branch, sequential units, one full gate)

| # | unit | serves | depends on | scope confidence | dispatch type |
|---|---|---|---|---|---|
| 1 | story:agent-serve | vision:governed-autonomy | story:agent-check (in flight on its own PR, merged to main first) | high, cited | aep:implementor, then aep:adversary |
| 2 | story:agent-http-binding | vision:governed-autonomy | unit 1 | medium | aep:implementor, then aep:adversary |
| 3 | story:agent-a2a | vision:governed-autonomy | unit 1; story:runtime-events (implemented) | medium | aep:implementor, then aep:adversary |

Design: docs/design/loom-agent-crd-deployment.md (PR 127) § 3-5, § 9 steps 2-4. Decisions taken by the orchestrator on § 10: 1 = A, 2 = A, 6 = A; Timo: 3 = B, 5 = A, 7 = takt later.

## Selection path

`aep plan artifact waves --kind story --status draft --format json` (aep 0.77.1), run after the typed scope entries were written from the story-scoper's sections (one aep:story-scoper agent scoped the three stories in sequence; the sub-agent cap of 3 held two implementation units at the time). The verb placed the three in different waves: wave 1 holds story:agent-serve (with story:managed-composition, story:mantle-placement, story:revalidation-membership-conformance, not selected: outside the design), wave 5 story:agent-a2a, wave 6 story:agent-http-binding. The verb and the scoper agree: no two of the three are disjoint. So this wave runs them as sequential units on one integration branch, each unit branched from the integration head after the previous unit merged, and the full gate runs once at the end. That is the batch Timo asked for; it is not parallel.

Collisions the verb printed among the three (verbatim ids and paths): agent-a2a × agent-http-binding: AGENTS.md, CHANGELOG.md, Cargo.lock (inferred), crates/loom-agent, ess/domains/agent.yaml. agent-a2a × agent-serve: AGENTS.md, CHANGELOG.md, README.md, crates/loom-agent, crates/loom-plugin/src/turn.rs (inferred), ess/domains/agent.yaml. agent-http-binding × agent-serve: AGENTS.md, CHANGELOG.md, crates/loom-agent, crates/loom-plugin/src/effects.rs, ess/domains/agent.yaml. Collisions with stories outside this wave (doc-dependency-tags-match-lock, frontier-action-names-its-claims, python-test-profile, secrets-subcommand) are on AGENTS.md, CHANGELOG.md, Cargo.lock and the loom-cli entry files; none of those stories is in this wave.

Unassessed after scoping: story:agent-check (in flight elsewhere), story:agent-project-kubernetes (superseded by the design: the controller renders manifests), story:confine-an-agent-tutorial (not a candidate).

Left out and why: story:agent-project-kubernetes (superseded, PR 127 § 3); the LoomAgent domain and the operator crate (own units, steps 5-6, the domain in flight on its own PR; the operator follows it as the next wave).

## Pre-flight

- Base: origin/main 858643a6 (loom 0.40.0). Integration branch `wave/2026-10-25-agents` in managed tree `loom-agents-w2` (`~/.local/state/worktree/trees/b10x/loom/loom-agents-w2`), its build directory `target/` inside the tree.
- Primary checkout `~/beyond10x/loom`: on main, one untracked path `.agents/` (not this wave's; left alone).
- Other managed loom trees present: loom-agent-domain (domain unit, in flight), loom-crd-design (design PR, its owner finished it), loom-w18-u2-breaks, loom-w18-u3-clicontract, loom-wave-20261018-w1, wt-4b0b390aadb9, wt-8082f40b5998 (older sessions; kept by the operator's earlier hold). Not refused: none is this wave's branch.
- Disk free at proposal: 166G (`df -h /`). Floor: 30G (builds pause). The b10x-disk-guard stops compilers under 15G.
- One measured loom build: not measured this wave; loom trees reached 30G on 2026-10-24 (footprint). sccache is installed (/usr/bin/sccache); loom sets no RUSTC_WRAPPER in `.cargo/config.toml`, so builds are cold per tree.
- Sub-agent cap: 3 at once (Timo). Units run one at a time here, so the cap is not reached by this wave alone.
- Gate: `task check` (AGENTS.md:221), run once on the integration branch at close; units gate package-scoped (`cargo test -p <crate>`, clippy on the crate).

## Commits this wave makes

One opening store commit on `wave/2026-10-25-agents` (this record, the Scope sections, the story moves); one commit per unit on its branch; the merge of each unit branch into the integration branch; the closing store commit (evidence, moves, Scope rewrites); the merge of the integration branch into main through the bot route. No tag, no release: the release is a separate step after the wave, batched with the other agent units, and announced through brain-main.

## Unit records (kept current)

| unit | branch | head | worktree | build dir | scratch | stage |
|---|---|---|---|---|---|---|
| agent-serve | | | | | ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/w2/agent-serve | waits for agent-check on main |
| agent-http-binding | | | | | ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/w2/agent-http-binding | waits for unit 1 |
| agent-a2a | | | | | ~/.cache/scratch/fbf1604f-db7b-423e-bed5-b9a4dc96d66a/w2/agent-a2a | waits for unit 1 |
