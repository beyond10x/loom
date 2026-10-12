---
format: aep.planning-md/3
id: review-result:adv-w5-u2-pass-1
kind: review-result
status: active
title: Adversary, loom w5 U2 granted gate ends run, pass 1
relations:
- reviews: story:granted-gate-ends-run
revision: 1
---
unit: U2 `story:granted-gate-ends-run`, tree `lw5-u2` on `impl/granted-gate-ends-run` (HEAD `895e4eb`)
verdict: NEEDS-CHANGE; 2 red cases
cases: executed 211→217, red 2
origin: introduced 3 / pre-existing 0 / undecided 1

**Cases added** (untracked, committed with the fix): `crates/loom-commission-testkit/tests/adversary_granted_gate.rs` (5), `crates/loom-plugin/tests/adversary_granted_gate.rs` (1).

Red:

| case | failed at |
|---|---|
| `a_granted_step_that_completes_the_case_at_the_budget_ends_completed` | a granted step that completes the case on the last budgeted step ends `Suspended` (budget), not `Completed` |
| `a_proposal_on_the_last_budgeted_step_ends_the_turn_proposed` | a plugin turn whose proposal is performed on step 8 ends `Stopped` with no proposal |

Green, holding rules of the design note the unit's suite did not drive: Allow with a Refused effect reads the gate; an ungated step after a granted one reads the gate; a grant for one action passes the gate of another for one step; the idle bound ends a run that repeats a granted step.

Suites after the cases: `cargo test -p b10x-loom-commission-testkit --no-fail-fast` 182 passed, 1 failed; `cargo test -p b10x-loom-plugin --no-fail-fast` 33 passed, 1 failed.

**Findings**

| # | file:line | finding | verdict | origin |
|---|---|---|---|---|
| F1 | `crates/loom-commission/src/runtime.rs:528` | a granted step leaves no gate pending, so at the budget the Run is suspended without loading the case; a completed case ends `Suspended` | CONFIRMED, NEEDS-CHANGE | introduced |
| F2 | `crates/loom-plugin/src/turn.rs:251` | a turn proposing on its eighth step ends `stopped` with no proposal; reached through F1 | CONFIRMED, NEEDS-CHANGE | introduced |
| F3 | `docs/qualification/plugin-layer-measurement.md:61` | the table still lists the re-run rule the unit removed | NEEDS-CHANGE (note) | introduced |
| F4 | `docs/commission/design/approval-gate.md:12` | "the grant is the approval the gate waits for" is untrue when the gate also awaits an action the grant does not cover; no caller in this repository reaches it | INFEASIBLE (note) | undecided |

Decided for the unit: F1 is fixed in the runtime (after a step that performed an effect, a used-up budget first loads the case, so a completed case ends `Completed`), which also fixes F2; F3 updates the measurement row; F4 rewords the design note's rationale to state the rule as it is (a granted step passes the whole gate for one iteration), the rule itself unchanged.

**Could not break:** Allow with a stale request (no verdict asked); a denied control; a case moved after a granted step (taken as the loop's own move, as for any performed effect); the plugin's other `RunOutcome` endings; CHANGELOG, AGENTS.md, the site pages and the specification comment against the code.
