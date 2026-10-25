---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-25-w1
kind: approval-record
status: draft
title: Wave 2026-10-25-w1 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:agent-check
revision: 1
---
# Wave 2026-10-25-w1 proposal, accepted with no operator turn

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved it under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-25): "implement
and land loom `story:agent-check` (step 1 of docs/design/loom-agent-crd-deployment.md § 9 [...])
Approved under the grant in ~/beyond10x/CLAUDE.md; Timo, 2026-10-25: 'the operational k8s bot is
to be implemented with loom sdk' and 'you will now implement the design about deployable loom
agents'", and "no release in this unit (the orchestrator batches releases)".

Skill `aep:implementing` 0.24.1, wave mode, N = 1: the orchestrator named one story.

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:agent-check` | `vision:governed-autonomy` | `ess/domains/agent.yaml`, `ess/scenarios/agent-check/`, `ess/ess-inputs.yaml`, `generated/rust/loom/`, `protocols/operator-reply/1.yaml`, `crates/loom-protocols/`, new `crates/loom-agent/`, `crates/loom-cli/`, `crates/loom-conformance/`, `website/docs/reference/`, README.md, AGENTS.md, CHANGELOG.md | adversary |

`aep plan artifact waves --kind story --status draft` (aep 0.77.1), before the scope entries were
written: waves `[doc-dependency-tags-match-lock, managed-composition, mantle-placement,
revalidation-membership-conformance, secrets-subcommand]`, `[frontier-action-names-its-claims]`,
`[python-test-profile]`; collisions: doc-dependency-tags-match-lock × frontier-action-names-its-claims
(AGENTS.md), doc-dependency-tags-match-lock × python-test-profile (AGENTS.md),
frontier-action-names-its-claims × managed-composition (ess/commission/domains/responsibility.yaml),
frontier-action-names-its-claims × python-test-profile (AGENTS.md, CHANGELOG.md); unassessed:
agent-a2a, agent-check, agent-http-binding, agent-project-kubernetes, agent-serve,
breaks-between-releases, co-pinned-consumer-check, command-invocation-contract,
confine-an-agent-tutorial, connector-effect-in-substrate, consumer-sdk-paths, dw-bugfix-baseline,
dw-bugfix-workflow, dw-incident-workflow, dw-workflow-into-protocol, frontier-names-movers,
generated-types-release-surface, needs-authority-before-host-stop, protocol-argument-schema,
reference-enrichment, release-binaries, tool-decisions-on-step, unperformable-names-the-claim,
writable-host-root, yaml-only-plugin. `story:agent-check` was unassessed; its 21 scope entries
(10 cited from the story's Scope section, 11 inferred) were written before this record. The other
candidates are left out because the orchestrator's item names one story.

Commits this record authorises: the opening store commit, the unit's commits on
`impl/agent-check` (the `ess/`-only first commit, then the implementation), the merge into
`wave/2026-10-25-w1`, the closing store commit, the pull request and its merge into `main` through
the bot route. No tag, no release, no other branch.

Dispatch: `aep:implementor` (opus, high) on the unit tree, then `aep:adversary` (opus, high).
Pre-flight: `df -h /` 169G free; one unit, one build directory inside the unit tree;
`git worktree list` holds other sessions' trees (`loom-agent-domain`, `loom-crd-design`,
`loom-w18-u2-breaks`, `loom-w18-u3-clicontract`, `loom-wave-20261018-w1`), left as they are.

## Decisions taken in planning

- D1: the check is an ESS command, `loom.agent.CheckAgentSpec`, with the document's `path` as
  its one input. The document and the plugin catalog are outside the input, so each refusal is an
  `external:` branch and `serveable` the default; one error, `loom.agent.SpecRefused`
  (`reason`, `member`, `detail`), with `reason` sourced as a literal per branch, so a target
  answering one reason for every branch fails. The conformance target plays the file system, as it
  plays the governor for `not-in-frontier`: a path that exists under the repository root is read as
  it is; for a synthesized path, the fixture of the forced branch (`ess/scenarios/agent-check/specs/`)
  or, forced nothing, `examples/agents/operator.yaml`.
- D2: the story's scenarios are authored `ess-scenario/4` documents under
  `ess/scenarios/agent-check/`, listed in `ess/ess-inputs.yaml`; ESS admits only lower-case letters,
  digits and hyphens in a scenario name (`ESS-AUTHOR-001`), so `a_valid_spec_checks` is
  `a-valid-spec-checks`, and so on. `crates/loom-conformance/tests/conform.rs` passes
  `--scenarios ess` so they run.
- D3: "a binding naming a program other than `runtime.connectors.program`" is not declared: no
  binding of the `loom.agent` domain names a program (`ConnectorsBinding` has `name`, `adapter`,
  `connection`, `list`, `search`, `get`, `write`), so the refusal has no member to name. It is
  declared with the story that gives a binding a program.
- D4: a secret literal is a string of the document that carries a credential: a URL with a user
  name and password or a query component, or a value starting with a known token prefix
  (`xoxb-`, `xoxp-`, `xapp-`, `sk-`, `sk_live_`, `sk_test_`, `ghp_`, `github_pat_`, `AKIA`,
  `Bearer `, `-----BEGIN`). The refusal names the member and never repeats the value.
- D5: with ess 0.69.2 on `PATH` the gate's `--strict-requires` refuses `requires: ess 0.69.0`;
  the gate runs with the ESS 0.69.0 release binary (`~/.cache/ess-release-0.69.0/smoke/`) first on
  `PATH`, and the pin does not move in this unit (no release here).
- D6: `malformed` carries `member: document` and the decoder's message as `detail`;
  `loom_sdk::agent::AgentSpecError` exposes the message only.
