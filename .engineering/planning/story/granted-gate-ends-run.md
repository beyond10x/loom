---
format: aep.planning-md/3
id: story:granted-gate-ends-run
kind: story
status: active
title: A granted gated action ends a Commission run as awaiting approval
relations:
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/tests/runtime_effect.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-plugin/src/turn.rs
- confidence: cited
  path: crates/loom-plugin/tests/turn.rs
- confidence: inferred
  path: docs/commission/design/approval-gate.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-12T15:59:29Z", actor: "human:timo", revision: 11}
- {from: "proposed", to: "active", at: "2026-10-12T15:59:29Z", actor: "human:timo", revision: 12}
---
## Why

Found by wave 2026-10-09-w2 U3 (plugin host): after one granted `source.read` on `inbound-answer@1`,
`run_until_blocked` ends the Run `AwaitingApproval [reply.propose, source.read]`. The gate (step 5
of `crates/loom-commission/src/runtime.rs`, module doc; the check at line 514) fires because the
handed frontier's actions needing approval are unchanged after the step, although the authority
provider granted the capability of the action the step performed. The plugin host works around it:
`at_a_granted_gate` (`crates/loom-plugin/src/turn.rs:285`) starts another Run on the same case when
the Run performed an effect and every awaited action needs a capability `PluginAuthority` grants.

The gate is deliberate for a step that performed an action needing no authority:
`a_step_that_leaves_the_approval_gate_unchanged_ends_awaiting_approval`
(`crates/loom-commission-testkit/tests/runtime_effect.rs:1047`) performs `inspect`, which needs
none, beside two gated actions, and ends `AwaitingApproval [deploy, merge]`; a Run that only works
around a gate stops there.

## Design

Decided in `decision-blocker:wave-2026-10-12-w1`; the design note
`docs/commission/design/approval-gate.md` records the decision and the cases below.

Option A (recommended): a step whose request needed authority, was granted by the provider
(`AuthorityVerdict::Allow`) and was performed by the effect port (`EffectOutcome::Performed`)
passes the gate: the next iteration does not read the gate (step 5) and runs the executor. Every
other step reads the gate as today: a step that performed an action needing no authority, a step
that proposed nothing, a refused or denied one. The step budget and the idle bound still end a Run
that only repeats granted reads.

## ESS first

The gate is not a command of the specification; it is described by the comment on
`commission.responsibility.RunOutcome` in `ess/commission/domains/responsibility.yaml` (line 193:
"a step left a frontier listing actions that need approval unchanged"). The first commit changes
only that comment to say a step performed under a grant is not one, with no declaration changed;
`task commission:ess-gate` stays green on it, and the story's new test
`a_step_performed_under_a_grant_passes_the_gate` fails on it (the run ends `AwaitingApproval`), and
the run is recorded. Later commits make it pass without changing `ess/`.

## Acceptance

- `crates/loom-commission-testkit/tests/runtime_effect.rs`: a new test
  `a_step_performed_under_a_grant_passes_the_gate` drives a frontier listing `read` (capability
  `datasource.read`) and `propose` (capability `reply.propose`), a provider that allows both, and
  an executor proposing `read` then `propose`; the Run performs both effects and does not end
  `AwaitingApproval` after the first. The case at line 1047 is unchanged and still ends
  `AwaitingApproval [deploy, merge]`, and a control in the new test with a provider that denies
  `read`'s capability still stops as today.
- The step-5 paragraph of the module doc of `crates/loom-commission/src/runtime.rs` states the rule.
- `docs/commission/design/approval-gate.md` exists and names the decision, the three cases (an
  ungated performed step, a granted performed step, a denied step) and the tests that hold them.
- `at_a_granted_gate` and the re-run loop in `crates/loom-plugin/src/turn.rs` are removed: a turn
  is one Run. `a_turn_that_only_reads_stops_at_its_budget` (`crates/loom-plugin/tests/turn.rs:82`)
  and every other test in `crates/loom-plugin/tests/` and `crates/loom-plugin-slack/tests/` pass;
  AGENTS.md § Plugin host no longer describes the re-run rule.
- CHANGELOG (Unreleased) names the change for callers that match `AwaitingApproval` after a
  granted effect.

## Scope

- `ess/commission/domains/responsibility.yaml` (cited, line 193; comment only)
- `crates/loom-commission/src/runtime.rs` (cited, lines 15, 34-40, 455-522, 593-600)
- `crates/loom-commission-testkit/tests/runtime_effect.rs` (cited, line 1047)
- `docs/commission/design/approval-gate.md` (new)
- `crates/loom-plugin/src/turn.rs` (cited, lines 14-21, 224, 285-305), `crates/loom-plugin/tests/turn.rs`
  (cited, line 82)
- `AGENTS.md` § Plugin host, `CHANGELOG.md` (cited)

## Shared surface

`CHANGELOG.md` (Unreleased) with `story:ess-059-upgrade`; lands second in its wave.
