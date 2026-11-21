---
format: aep.planning-md/3
id: story:autonomy-grant-table
kind: story
status: draft
title: A run's level, overrides and the host's presets, classes and ceilings resolve into one grant table
summary: Holders partially ordered; a lowering always, any other move a raise up to the admitted ceiling; limit_hit takes no override; an expiry duration per class; autonomy_resolved
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-request-entity
revision: 5
---
## Outcome

`b10x-loom-decision` resolves a run's autonomy into one grant table: for every decision class,
the holder, and for the run, the escalation mode (`none`, `person` or `final`). The inputs are
the run's level (`A0` to `A5`), and three documents the host passes in: the level presets
(`loom.autonomy-presets/1`: per level, a holder per class and the escalation mode), the decision
classes (`loom.decision-classes/1`: per class id its kind, `approval`, `question`, `outcome`,
`review` or `control`, the actions and capability it covers, and optionally `expires_after`, a
whole number of seconds after which an unanswered request of the class may expire), and the
admitted ceilings
(`loom.autonomy-admitted/1`: per class the highest admitted holder), plus the run's overrides
(class to holder) and whether the run is an eval run. Loom ships no presets for host classes and
no default level: a host that names no level is refused.

Holders are ordered partially, as session `sdc` answered on 2026-11-21 (answer a):
`person < advised < rule_and_decider < rule` and `rule_and_decider < decider`, with `rule` and
`decider` incomparable; each non-final holder is below its final variant (`rule < rule_final`,
`decider < decider_final`, `rule_and_decider < rule_and_decider_final`); and
`rule_and_decider_final < rule_final`, `rule_and_decider_final < decider_final`. The order is the
transitive closure of these pairs. A move from the preset to an override is a lowering when the
override is below the preset in this order; every other move, a move between incomparable holders
included, is a raise.

The resolver applies the rules of `~/beyond10x/sdc/docs/autonomy-levels.md` § 2.3:

1. An override that lowers a class is taken.
2. An override that raises a class is taken only when it is at or below the class's admitted
   holder in the order; with no admitted entry the preset is the ceiling, so every raise is
   refused. A refused raise is `AboveCeiling`, naming the class, the override and the ceiling.
3. `limit_hit` takes no override (`NoOverride`), at any level (`autonomy-levels.md` § 2.3, rule 3, answer a).
4. `budget_overrun` takes no override to a holder that involves the decider (`decider`,
   `decider_final`, `rule_and_decider`, `rule_and_decider_final`), and an admitted entry naming one
   for it is refused (`NoOverride`): no LLM raises a limit (`autonomy-levels.md` § 2.2, "Limits are never the decider's").
5. An eval run is resolved at `A5`, and an override naming `person` or `advised` is refused
   `EvalOverride`.

`budget_overrun` and `limit_hit` are built in (session `sdc`, 2026-11-21, point 9): they are valid
with no classes document, and Loom holds their rows, read from `autonomy-levels.md` § 2.2, rows "budget overrun" and "limit hit" with
"R, then P" as `rule` and "R: end" as `rule_final` (session `sdc`, 2026-11-21, point 1):
`budget_overrun` person, person, person, rule, rule, rule_final; `limit_hit` rule at `A0` to `A4`
and rule_final at `A5`. A host presets document may give `budget_overrun` and may not give
`limit_hit`. `escalation` is a policy row, not a class: the escalation mode is `none` at `A0` and
`A1`, `person` at `A2` to `A4`, and `final` at `A5` (`autonomy-levels.md` § 2.2, row "escalation", and § 3.1, `escalation.mode`), read
from the presets document.

A resolution gives an `AutonomyResolved` record (level, eval flag, the digest of each input
document, the overrides, and the table). The grant table answers `holder(class)` and
`expires_after(class)`, and refuses a class the classes document does not declare and that is not
built in (`UnknownClass`). `expires_after` is absent for the built-in classes and for a class
whose entry gives none; a value that is not a positive whole number is refused (`BadExpiry`),
naming the class. The grant table only carries the duration: `story:decision-routing-by-holder`
sets a request's `expires_at` from it. A run that is not an eval run and names no level is
refused `NoLevel`.

Its code goes in its own module of `crates/loom-decision`; `lib.rs` gains one `mod` line.

## Why

Item 2 of the request (`~/beyond10x/sdc/docs/decisions.md:304-306`). § Ownership gives Loom
"the resolver: preset ⊕ overrides ⊕ admitted ceilings into one holder per class"
(`decisions.md:257-258`), and session `sdc` answered that Loom owns
the types and the resolver while the preset table, the classes and the admitted ceilings are
documents the host passes in (point 9). Nothing in Loom resolves a holder: "autonomy" appears in
no crate, and `StopPolicy` (`responsibility.yaml:432-446`) is a per-run stop configuration.
The order is answer a, now also written in `autonomy-levels.md:299-309`. Expiry is "fired by a
host tick that passes the instant" (`decisions.md:84-86`), and a request's `expires_at` needs a
source a host run can give: the coordinator chose a per-class duration in the classes document
over a CLI flag.

## Waits on

`story:decision-request-entity`: `Holder` is declared there; both stories add a domain to
`ess/system.yaml` and `ess/ess-inputs.yaml` and change `crates/loom-decision/src/lib.rs`.

## ESS first

A new domain `loom.autonomy` in `ess/domains/autonomy.yaml`, declared in `ess/system.yaml`:
`AutonomyLevel`, `EscalationMode`, `AutonomyPresets`, `DecisionClasses`, `DecisionClassKind`,
`AdmittedCeilings`, `AutonomyOverrides`, `GrantTable`, `AutonomyResolved`, and the refusals
`AboveCeiling`, `NoOverride`, `EvalOverride`, `UnknownClass`, `NoLevel`, `BadExpiry`. The domain
declares types only, no command, so no conformance scenario is added. On that commit
`task drift` fails, and `crates/loom-decision/tests/autonomy.rs` does not compile.

## Acceptance

With fixture documents that copy the table of `autonomy-levels.md` § 2.2, with class ids as in
`~/beyond10x/sdc/ess/domains/decision.yaml:176`:

- At `A3`, `holder(roadmap)` is `decider` and `holder(merge)` is `person`; at `A4`,
  `holder(merge)` is `rule_and_decider`; at `A5`, `rule_and_decider_final`; the escalation mode is
  `person` at `A3` and `final` at `A5`.
- At `A4`, `vision: person` is taken; `merge: decider` at `A3` is refused `AboveCeiling` with no
  admitted entry and taken with an admitted entry `merge: decider`.
- A move between incomparable holders is a raise: at `A3`, `roadmap: rule` (preset `decider`) is
  refused `AboveCeiling` with no admitted entry, refused with an admitted entry `roadmap: decider`,
  and taken with an admitted entry `roadmap: rule`; at `A5`, `roadmap: rule` (preset
  `decider_final`) is refused `AboveCeiling` with no admitted entry.
- At `A5`, `roadmap: rule_and_decider_final` (preset `decider_final`) and `roadmap: advised` in a
  run that is not an eval run are lowerings and are taken with no admitted entry.
- A test checks every ordered pair of the eight holders against the pairs of answer a: a pair is a
  lowering exactly when the closure of those pairs puts the override below the preset.
- `limit_hit: person` and `limit_hit: rule_final` are refused `NoOverride` at every level.
- `budget_overrun: decider` and `budget_overrun: rule_and_decider_final` are refused `NoOverride`;
  `budget_overrun: person` at `A3` is a lowering and is taken.
- An eval run with no level resolves at `A5`; with `vision: advised` it is refused
  `EvalOverride`. A run that is not an eval run and names no level is refused `NoLevel` and gives
  no `AutonomyResolved`.
- `holder(deploy)`, a class neither the classes document nor the built-ins declare, is refused
  `UnknownClass` naming `deploy`.
- With `roadmap: {expires_after: 86400}` in the classes document, `expires_after(roadmap)` is
  86400 seconds and `expires_after(vision)` (no member) and `expires_after(limit_hit)` are absent;
  `expires_after: 0` is refused `BadExpiry` naming the class.
- `budget_overrun` and `limit_hit` resolve with an empty classes document; a presets document that
  names `limit_hit` is refused, naming it.
- A classes document with a member the format does not declare (`escalate_when` included) is
  refused, naming the member.
- `AutonomyResolved` carries the SHA-256 of each document's bytes; the same inputs give the same
  record bytes.
- `task check` exits 0.

## Out of scope

The run's `escalate_when` conditions (`story:escalate-when-conditions`); they are not a member of
the classes document. A change of level within a run, and any record of one: no story changes a
run's level. Setting `expires_at` on a request (`story:decision-routing-by-holder`). Mapping an action to its
class at a stop (`story:decision-routing-by-holder`). SDC's own preset and class files.
