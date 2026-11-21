---
format: aep.planning-md/3
id: story:autonomy-grant-table
kind: story
status: proposed
title: A run's level, overrides and the host's presets, classes and ceilings resolve into one grant table
summary: Three document formats with their members; holders partially ordered; a lowering always, any other move a raise up to the admitted ceiling; limit_hit takes no override; an eval run is A5 and refuses every non-final override; refusal order stated; an expiry duration per class; no escalation mode; autonomy_resolved
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-request-entity
scope:
- confidence: inferred
  path: crates/loom-decision/Cargo.toml
- confidence: inferred
  path: crates/loom-decision/src/autonomy.rs
- confidence: cited
  path: crates/loom-decision/src/lib.rs
- confidence: cited
  path: crates/loom-decision/tests/autonomy.rs
- confidence: cited
  path: ess/domains/autonomy.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:19Z", actor: "agent:sdc", revision: 16, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom-decision` resolves a run's autonomy into one grant table: for every decision class,
the holder. The inputs are the run's level (`A0` to `A5`), and three documents the host passes in
as bytes: the level presets, the decision classes, and the admitted ceilings, plus the run's
overrides (class to holder) and whether the run is an eval run. Loom ships no presets for host
classes and no default level: a host that names no level is refused.

The three formats, each a YAML document with a `format` member; any member a format does not
declare is refused `BadDocument`, naming the document and the member:

- `loom.autonomy-presets/1`: `presets`, per level `A0` to `A5` a map of class id to holder. It may
  give `budget_overrun`; an entry for `limit_hit` or `escalation` is refused `BadDocument` naming
  it.
- `loom.decision-classes/1`: `classes`, per class id the members `kind` (one of `approval`,
  `question`, `outcome`, `review`, `control`), `actions` (the protocol actions the class covers),
  `capability` (optional, the capability whose grant decides an approval) and `expires_after`
  (optional, a whole number of seconds after which an unanswered request of the class may expire).
  The key is `kind`, not the `type` of SDC's own file (`~/beyond10x/sdc/docs/autonomy-levels.md:398-413`);
  SDC's `close` type is the kind `outcome` here (the outcome `close`), and SDC's `when` and
  `independent` are not members. An entry for `budget_overrun`, `limit_hit` or `escalation` is
  accepted when it is `{kind: control}` alone and changes nothing (SDC lists them "so its table is
  complete", `autonomy-levels.md:416-419`); any other member on one of them is refused
  `BadDocument`.
- `loom.autonomy-admitted/1`: `admitted`, a map of class id to the highest admitted holder. The
  document is optional: an absent one admits nothing.

`AutonomyLevel` and `Holder` are the `loom.decision` types of `story:decision-request-entity`;
this story declares neither again. The resolver uses no order of levels (a preset row is looked up
by its level), and the holder order below is the resolver's own code, not an ESS ordered enum
(Loom's `ess/31` has none; `ordered:` needs `ess/36`, `story:ess-format-31`).

Holders are ordered partially, as session `sdc` answered on 2026-11-21 (answer a):
`person < advised < rule_and_decider < rule` and `rule_and_decider < decider`, with `rule` and
`decider` incomparable; each non-final holder is below its final variant (`rule < rule_final`,
`decider < decider_final`, `rule_and_decider < rule_and_decider_final`); and
`rule_and_decider_final < rule_final`, `rule_and_decider_final < decider_final`. The order is the
transitive closure of these pairs. An override equal to the preset is no move and is taken. A move
from the preset to an override is a lowering when the override is below the preset in this order;
every other move, a move between incomparable holders included, is a raise.

The resolver applies the rules of `~/beyond10x/sdc/docs/autonomy-levels.md` § 2.3:

1. An override that lowers a class is taken.
2. An override that raises a class is taken only when it is at or below the class's admitted
   holder in the order. With no admitted entry the preset is the ceiling, so every raise is
   refused; an admitted entry at or below the preset admits no raise. A refused raise is
   `AboveCeiling`, naming the class, the override and the ceiling.
3. `limit_hit` takes no override (`NoOverride`), at any level (`autonomy-levels.md` § 2.3, rule 3, answer a).
4. `budget_overrun` takes no override to a holder that involves the decider (`decider`,
   `decider_final`, `rule_and_decider`, `rule_and_decider_final`), refused `NoOverride`: no LLM
   raises a limit (`autonomy-levels.md` § 2.2, "Limits are never the decider's"). An admitted entry
   naming one for it is a document fault, refused `BadDocument` naming `budget_overrun` and the
   holder.
5. An eval run is resolved at `A5`: it names no level or `A5`, and one naming another level is
   refused `EvalLevel`, naming the level. An override naming a holder that is not final (`person`,
   `advised`, `decider`, `rule` or `rule_and_decider`) is refused `EvalOverride`, naming the class
   and the holder: "Eval runs (`--eval`, § 3.4) refuse any override naming a holder that is not
   final (`person`, `advised`, `decider`, `rule`, `rule_and_decider`), exit 64: at A5 every cell is
   a final holder, so no decision waits on a person" (`autonomy-levels.md:317`). An override to a
   final holder is resolved by rules 1 to 4.

Where two rules refuse one override, one refusal is reported, the first of `NoOverride`,
`EvalOverride`, `AboveCeiling`. Any refusal refuses the whole resolution: no `AutonomyResolved`
and no grant table. Every refusal is returned, in this order: document refusals (`BadDocument`,
`BadExpiry`, and `UnknownClass` for a class a presets or admitted document names that the classes
document does not declare and that is not built in), then the level (`NoLevel`, `EvalLevel`),
then the overrides in class-id order.

`budget_overrun` and `limit_hit` are built in (session `sdc`, 2026-11-21, point 9): they are valid
with no classes document entry, and Loom holds their rows, read from `autonomy-levels.md` § 2.2, rows "budget overrun" and "limit hit" with
"R, then P" as `rule` and "R: end" as `rule_final` (session `sdc`, 2026-11-21, point 1):
`budget_overrun` person, person, person, rule, rule, rule_final; `limit_hit` rule at `A0` to `A4`
and rule_final at `A5`. A host presets document may give `budget_overrun` and may not give
`limit_hit`.

`escalation` is a policy row, not a class (point 9): it gives the holder of a request after
Escalate, and the contract fixes that holder as `person` (`~/beyond10x/sdc/ess/domains/decision.yaml:760-764` at `00e3557`,
third message of session `sdc`). The grant table carries no escalation mode. The document's
`escalation.mode` (`person` at A2 to A4, `final` at A5, `autonomy-levels.md:385-387`) is
redundant with the holders: the contract's guards read the request's holder, not the level
(`~/beyond10x/sdc/docs/decisions.md:90-92`); suppression is what `decider_final`, `rule_final`
and `rule_and_decider_final` mean (`decision.yaml:186`); and every A5 cell of the presets is one of
those holders, the rollback cell included, now "R: end" (`autonomy-levels.md:253`). With rule 5,
no escalation in an eval run reaches a person. Where a run's holder and the document's mode would
disagree, a lowering at A5 in a run that is not an eval run or a raise at A1, the holder governs
(`story:decision-routing-by-holder`). A presets document that names `escalation` is refused
`BadDocument`, naming it.

A resolution gives an `AutonomyResolved` record (level, eval flag, the SHA-256 of the presets and
classes documents' bytes, the SHA-256 of the admitted document's bytes or none when it is absent,
the overrides, and the table). `story:escalate-when-conditions` later adds the run's
`escalate_when` list to the resolver's input, the grant table and `AutonomyResolved`; an empty list
is not written, so the record of a run without one keeps its bytes. The grant table answers `holder(class)` and `expires_after(class)`,
and refuses a class the classes document does not declare and that is not built in
(`UnknownClass`). `expires_after` is absent for the built-in classes and for a class whose entry
gives none; a value that is not a positive whole number is refused (`BadExpiry`), naming the
class. The grant table only carries the duration: `story:decision-routing-by-holder` sets a
request's `expires_at` from it. A run that is not an eval run and names no level is refused
`NoLevel`.

Its code goes in its own module, `autonomy`, of `crates/loom-decision`; `lib.rs` gains one `mod`
line, and the crate's manifest gains `serde_yaml_ng` (to read the documents) and `sha2` (their
digests), at the versions the workspace already pins (`crates/loom-bindings/Cargo.toml:17`,
`crates/loom-governor/Cargo.toml:21`).

## Why

Item 2 of the request (`~/beyond10x/sdc/docs/decisions.md:304-306`). § Ownership gives Loom
"the resolver: preset ⊕ overrides ⊕ admitted ceilings into one holder per class"
(`decisions.md:257-258`), and session `sdc` answered that Loom owns
the types and the resolver while the preset table, the classes and the admitted ceilings are
documents the host passes in (point 9). Nothing in Loom resolves a holder: "autonomy" appears in
no crate, and `StopPolicy` (`ess/commission/domains/responsibility.yaml:432-442`) is a per-run
stop configuration. The order is answer a, now also written in `autonomy-levels.md:299-309`.
Expiry is "fired by a host tick that passes the instant" (`decisions.md:84-86`), and a request's
`expires_at` needs a source a host run can give: the coordinator chose a per-class duration in the
classes document over a CLI flag. The request's item 2 names "escalation mode person or final"
(`decisions.md:305-306`); it is met by the final holders, as above, and no story read a mode. The
eval rule for non-final holders and the rollback cell are session `sdc`'s fifth message
(2026-11-21, point 3), now in `autonomy-levels.md:253` and `:317`. "Every eval run runs at A5"
(`autonomy-levels.md:456`) does not say what an eval run naming another level gets; refusing it
fails toward less authority.

## Waits on

`story:decision-request-entity`: `Holder` and `AutonomyLevel` are declared there; both stories add
a domain to `ess/system.yaml` and `ess/ess-inputs.yaml` and change `crates/loom-decision/src/lib.rs`
and `crates/loom-decision/Cargo.toml`.

## ESS first

A new domain `loom.autonomy` in `ess/domains/autonomy.yaml`, declared in `ess/system.yaml`:
`AutonomyPresets`, `DecisionClasses`, `DecisionClassKind`, `AdmittedCeilings`,
`AutonomyOverrides`, `GrantTable`, `AutonomyResolved`, and the refusals `AboveCeiling`,
`NoOverride`, `EvalOverride`, `EvalLevel`, `UnknownClass`, `NoLevel`, `BadExpiry`, `BadDocument`.
`AutonomyResolved`'s admitted digest is optional. The domain declares types only, no command, so
no conformance scenario is added. On that commit `task drift` fails, and
`crates/loom-decision/tests/autonomy.rs` does not compile.

## Acceptance

With fixture documents that copy the table of `autonomy-levels.md` § 2.2, with class ids as in
`~/beyond10x/sdc/ess/domains/decision.yaml:176`:

- At `A3`, `holder(roadmap)` is `decider` and `holder(merge)` is `person`; at `A4`,
  `holder(merge)` is `rule_and_decider`; at `A5`, `rule_and_decider_final`, and
  `holder(rollback)` is `rule_final`.
- At `A4`, `vision: person` is taken; `merge: decider` at `A3` is refused `AboveCeiling` with no
  admitted entry and taken with an admitted entry `merge: decider`.
- A move between incomparable holders is a raise: at `A3`, `roadmap: rule` (preset `decider`) is
  refused `AboveCeiling` with no admitted entry, refused with an admitted entry `roadmap: decider`,
  and taken with an admitted entry `roadmap: rule`; at `A5`, `roadmap: rule` (preset
  `decider_final`) is refused `AboveCeiling` with no admitted entry.
- At `A3` with an admitted entry `roadmap: advised` (below the preset `decider`), `roadmap: decider`
  is taken as no move and `roadmap: decider_final` is refused `AboveCeiling` naming the ceiling
  `advised`.
- At `A5`, `roadmap: rule_and_decider_final` (preset `decider_final`), `roadmap: decider` and
  `roadmap: advised` in a run that is not an eval run are lowerings and are taken with no admitted
  entry.
- A test checks every ordered pair of the eight holders against the pairs of answer a: a pair is a
  lowering exactly when the closure of those pairs puts the override below the preset.
- `limit_hit: person` and `limit_hit: rule_final` are refused `NoOverride` at every level.
- `budget_overrun: decider` and `budget_overrun: rule_and_decider_final` are refused `NoOverride`;
  `budget_overrun: person` at `A3` is a lowering and is taken.
- An admitted document with `budget_overrun: decider` is refused `BadDocument` naming
  `budget_overrun` and `decider`.
- An eval run with no level resolves at `A5`, and so does one naming `A5`; one naming `A3` is
  refused `EvalLevel` naming `A3`. A run that is not an eval run and names no level is refused
  `NoLevel` and gives no `AutonomyResolved`.
- In an eval run, `vision: person` is refused `EvalOverride` naming `vision` and `person`.
- In an eval run, `vision: advised` is refused `EvalOverride` naming `advised`.
- In an eval run, `roadmap: decider` is refused `EvalOverride` naming `decider`.
- In an eval run, `rollback: rule` is refused `EvalOverride` naming `rule`.
- In an eval run, `merge: rule_and_decider` is refused `EvalOverride` naming `rule_and_decider`.
- In an eval run, `roadmap: rule_and_decider_final` is a lowering and is taken.
- In an eval run, `limit_hit: person` is refused `NoOverride`, not `EvalOverride`.
- At `A3`, the overrides `merge: decider` and `vision: decider_final` with no admitted entries
  give two `AboveCeiling` refusals, `merge` first, and no `AutonomyResolved`.
- `holder(deploy)`, a class neither the classes document nor the built-ins declare, is refused
  `UnknownClass` naming `deploy`; a presets document naming `deploy` at `A2` is refused
  `UnknownClass` naming `deploy`.
- With `roadmap: {kind: approval, actions: [...], expires_after: 86400}` in the classes document,
  `expires_after(roadmap)` is 86400 seconds and `expires_after(vision)` (no member) and
  `expires_after(limit_hit)` are absent; `expires_after: 0` is refused `BadExpiry` naming the
  class.
- `budget_overrun` and `limit_hit` resolve with an empty classes document (`classes: {}`); a
  presets document that names `limit_hit` or `escalation` is refused `BadDocument`, naming it.
- A classes document with `budget_overrun: {kind: control}` resolves as one without it; one with
  `budget_overrun: {kind: control, actions: [x]}` is refused `BadDocument`.
- A classes document with a member the format does not declare (`escalate_when`, `type` and `when`
  each tried) is refused `BadDocument`, naming the member.
- With no admitted document, every raise is refused `AboveCeiling` and `AutonomyResolved` carries
  no admitted digest.
- `AutonomyResolved` carries the SHA-256 of each given document's bytes and no escalation mode;
  the same inputs give the same record bytes.
- `task check` exits 0.

## Out of scope

The run's `escalate_when` conditions (`story:escalate-when-conditions`); they are not a member of
the classes document. A change of level within a run, and any record of one: no story changes a
run's level. Setting `expires_at` on a request (`story:decision-routing-by-holder`). Mapping an action to its
class at a stop (`story:decision-routing-by-holder`). SDC's own preset and class files. A
run-wide escalation mode. A class condition (`when`) and a class's independence flag: Loom reads
neither from the classes document.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (new module `autonomy`) plus the ESS domain `loom.autonomy` — cited (story: "Its code goes in its own module of `crates/loom-decision`"; § ESS first)
- **Files:** `ess/domains/autonomy.yaml` (new) — cited (§ ESS first)
- **Files:** `ess/system.yaml` (one `loom.autonomy` line under `domains:`, `ess/system.yaml:10-23`) — cited (§ Waits on)
- **Files:** `ess/ess-inputs.yaml` (one `domains/autonomy.yaml` line under `specification:`) — cited (§ Waits on)
- **Files:** `crates/loom-decision/src/lib.rs` (one `mod` line) — cited (§ Outcome, last line)
- **Files:** `crates/loom-decision/tests/autonomy.rs` — cited (§ ESS first)
- **Files:** `crates/loom-decision/src/autonomy.rs` — inferred, the module name is not given; `crates/loom-bindings/src/{decode,load}.rs` is the precedent
- **Files:** `crates/loom-decision/Cargo.toml` — inferred. The crate must decode YAML documents and hash their bytes (`serde_yaml_ng = "0.10"` as `crates/loom-bindings/Cargo.toml:17`, `sha2 = "0.11.0"` as `crates/loom-executor/Cargo.toml:16`). Neither is in the entity story's "generated `loom` crate only".
- **Symbols (declared in ESS, generated into `loom`):** `AutonomyLevel`, `AutonomyPresets`, `DecisionClasses`, `DecisionClassKind`, `AdmittedCeilings`, `AutonomyOverrides`, `GrantTable`, `AutonomyResolved`, refusals `AboveCeiling`, `NoOverride`, `EvalOverride`, `UnknownClass`, `NoLevel`, `BadExpiry` — cited (§ ESS first)
- **Symbols (hand-written):** `holder(class)`, `expires_after(class)`, the holder order, the resolver — cited (§ Outcome). The names of the Rust functions are inferred.
- **Symbols (taken, not declared here):** `Holder` from `story:decision-request-entity` — cited (§ Waits on). `crates/loom-decision` does not exist at `61fd500b` (`ls crates/` has no `loom-decision`), and `git grep -il autonomy -- crates ess` finds only a string in `crates/loom-executor/tests/adversary_w19_history.rs`. The story's "autonomy appears in no crate" is true.
- **Documents:** none in this repo. Inputs are the host-passed `loom.autonomy-presets/1`, `loom.decision-classes/1` and `loom.autonomy-admitted/1`; fixtures live in `crates/loom-decision/tests/` — inferred.
- **Confidence:** high for the file set (the story names every file; `story:bindings-document` is the same shape: `ess/domains/bindings.yaml`, `ess/system.yaml`, `ess/ess-inputs.yaml` and a crate, with no command). Medium for `Cargo.toml` and the module file name, which are inferred.
- **Would collide with:**
  - `crates/loom-decision/src/lib.rs`:
    - `story:decision-request-entity`: creates the crate. Ordered, `depends_on` direct.
    - `story:decider-port-and-grounding-check`: adds a `mod`. Ordered, it depends on this story.
    - `story:decision-routing-by-holder`, `story:escalate-when-conditions`, `story:decided-record-written-then-recorded`, `story:silence-and-expiry-at-a-host-tick`, `story:non-independent-review-becomes-decision-request`: all add modules. Ordered by the chain autonomy → decider-port → routing → escalate-when → decided-record → inbox → silence.
    - `story:decision-request-file-store`: **not ordered**. It depends only on `story:decision-request-entity`, says "own module", and so needs a `mod` line too.
    - `story:grounds-resolve-against-held-cases`: **not ordered**. It depends only on the file store and puts its code in `crates/loom-governor`, so the `lib.rs` overlap is unproven.
  - `crates/loom-decision/Cargo.toml`: `story:decision-request-entity` (ordered); `story:decider-port-and-grounding-check` (ordered, line 114 asserts its content); `story:decision-request-file-store` (**not ordered**, inferred).
  - `ess/domains/autonomy.yaml`: `story:escalate-when-conditions` extends it (ordered, it comes later in the chain).
  - `ess/system.yaml` and `ess/ess-inputs.yaml` (one-line additions, which merge but are the same file):
    - `story:decision-request-entity`: ordered.
    - `story:hash-chained-run-log` (`loom.runlog`), `story:tree-run-command` (`loom.tree`), `story:engine-configuration-document` (`loom.engine`): **none has a `depends_on` path to or from this story**. Among themselves they are ordered: hash-chained-run-log → tree-run-command → engine-configuration-document.
    - `ess/ess-inputs.yaml` only: `story:boundary-admits-declared-http-egress`, `story:send-input-in-the-declared-shape`, `story:agent-selects-the-connectors-relay` (scenario lines; ordered among themselves, **not ordered** against this story).
  - `crates/loom-decision/tests/autonomy.rs`: no other story names it.
- **Safety fact:** a types-only domain adds no conformance scenario, so `task conform` is unaffected — `ess/domains/bindings.yaml` has no `commands:` and its first commit `501c332a` touched only that file, `ess/system.yaml` and `ess/ess-inputs.yaml` — step 2, unproven
- **Contradicted by the code:** nothing found. Every `autonomy-levels.md`, `decisions.md` and `decision.yaml` line the story cites says what it claims (checked at 253, 273, 299-309, 317, 385-387, `decisions.md:84-92,257-258,304-306`, `decision.yaml:176,720-724`). One exception is under Story defects.
