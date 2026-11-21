---
format: aep.planning-md/3
id: story:decision-request-entity
kind: story
status: draft
title: loom.decision declares the decision request and its eight commands, and b10x-loom-decision applies them
summary: 'The final SDC contract moved into Loom''s ESS: class ids, the decided case revision, one open request per record id, and the DecisionStore port with the records kept beside a request'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
revision: 5
---
## Outcome

Loom's ESS has a domain `loom.decision` (`ess/domains/decision.yaml`) whose entity
`DecisionRequest` is the decision a run asks of a person, a decider or a rule: its subject at a
revision, its question and options, its autonomy level, class and holder, the eligible principals
and the one it is addressed to, its recommendation and grounds, its escalation, its decision, the
Canon record the decision produced, and its supersession. The domain is taken from
`~/beyond10x/sdc/ess/domains/decision.yaml` as session `sdc` declared it final on 2026-11-21, with
its guards as written, changed only where listed below. It has eight commands: `Request`,
`Recommend`, `Decide`, `TakeRecommendation`, `RecordProduced`, `Escalate`, `Supersede` and
`Expire`.

A new crate, `b10x-loom-decision` (`crates/loom-decision`), applies the commands to a request and
answers each outcome the domain declares, over a `DecisionStore` port with an in-memory
implementation, and the three views (`OpenDecisionsForPrincipal`, `ArtifactDecisionHistory`,
`DecisionRequests`) as functions over a store. The port reads a request, writes it when its state
is still the one read, lists requests, finds the Open or Escalated request for a record key,
appends an `EscalationSuppressedRecord` beside a request, and reads the records beside a request
in the order they were appended. The crate depends on the generated `loom` crate only: no
Commission, executor, Canon, model provider or Mandate.

Taken from the SDC file as written, each stated by session `sdc`, 2026-11-21, beyond its earlier
answers (third message):

- An option's consequence names the kind of Canon record it produces, `produces`, absent when it
  produces none (`decision.yaml:95`); an outcome's "leave it untaken" produces none (`:67`).
- The denying option is the one whose `consequence.grant` is `denied` (`:553-555`); no other field
  marks it.
- `Escalate` always sets the holder to `person` (`:720-724`), so a decider can no longer decide
  the request.
- `RecordProduced` with the record already stored changes nothing (`:665-670`); a different
  record is refused `ProducedRecordConflict` (`:671-675`); on a request whose decided option
  produces no record it is refused `NoRecordToProduce` (`:676-680`).
- `Expire` takes the tick's instant `at` (`:776`); `Expire`, and `TakeRecommendation` by
  `silence`, before `expires_at` or on a request with none are refused `NotDue` (`:625-630`,
  `:782-786`).
- `Request` in class `escalation` is refused `NotARequestClass` (`:401-404`).

Changes from the SDC file:

- A decision class is `DecisionClassId`, an identifier, not the enum of `decision.yaml:174-176`;
  `budget_overrun` and `limit_hit` are built in (session `sdc`, 2026-11-21, point 9; the file
  says so in prose, `:178`). The domain checks no class beyond `escalation`;
  `story:autonomy-grant-table` validates it against the host's classes document.
- `Decide` and `TakeRecommendation` take `case_revision`, the case revision at the decision, kept
  as `decided_case_revision` (answer e); the SDC inputs (`:518-524`, `:591-596`) have none. The
  record is written at that revision (`story:decided-record-written-then-recorded`).
- `Request` takes `record_key`, the record id the request's decision would produce at the case
  revision when it is opened, and `open_for_record`, the Open or Escalated request the runtime
  found in the store for that key, if any. With `open_for_record` given, `Request` is refused
  `RecordKeyOpen` naming that request, and nothing is opened (answer g; the coordinator chose
  refusal over supersession). The SDC file states this rule in prose only (`:517`;
  `~/beyond10x/sdc/docs/decisions.md:225-226`), so Loom enforces it here.
- `EscalationSuppressedRecord` is declared: the request id, class, subject at revision, reasons,
  `best` if any, and the cause (`SuppressionCause`: `escalate`, `failure`, `withheld`, `veto`,
  `rule_escalate`), with the instant it was kept. The SDC file names the event
  `escalation_suppressed` only in prose (`decision.yaml:184`, `:716`); the type is named apart
  from the error `EscalationSuppressed` (`:351-354`). `story:decision-routing-by-holder` writes
  it through the port; `story:escalate-when-conditions` adds the cause `condition`.
- The actor `Driver` (`decision.yaml:286-296`) is `loom.decision.Runtime`; `Person` and `Decider`
  keep their commands.

Its code goes in its own modules of `crates/loom-decision`, so a later story adds a module and a
`mod` line, not edits to this story's code.

## Why

Item 4 of the request: the entity "as the contract, moved into Loom's ESS when it is taken"
(`~/beyond10x/sdc/docs/decisions.md:313-315`). Loom's three ESS systems hold no decision-request
entity; Commission's `HumanDecisionRequest` is untyped JSON, "Commission's sources declare no
structure for it" (`ess/commission/domains/responsibility.yaml:106-109`). Every later story of
the epic reads or writes this entity, and the routing, file-store and inbox stories share the
records kept beside a request, so the port that carries them is declared here.

## ESS first

`ess/domains/decision.yaml`, declared in `ess/system.yaml` and listed in `ess/ess-inputs.yaml`:
the types (with `EscalationSuppressedRecord` and `SuppressionCause`), entity, actors, errors (the
twelve of the SDC file and `RecordKeyOpen`), commands, events (the nine of the SDC file) and views
above. On that commit `task drift` fails, and `crates/loom-decision/tests/decision_request.rs` and
`crates/loom-conformance/src/decision.rs` do not compile.

## Acceptance

- `task conform` runs the synthesized `loom.decision` scenarios against
  `b10x-loom-decision` and every scenario of the domain executes; `ess/SKIPPED.md` gains no line
  for it.
- SDC's scenario `~/beyond10x/sdc/ess/scenarios/merge-a5-veto.yaml`, copied into Loom with the
  domain renamed, passes against `b10x-loom-decision`.
- `Request` with one option is refused `TooFewOptions`, and `Request` in class `escalation` is
  refused `NotARequestClass`; neither stores anything.
- `Request` with `open_for_record: DR-1` is refused `RecordKeyOpen` naming `DR-1`, and the store
  is unchanged; after `DR-1` is Decided, the in-memory store finds no open request for the key and
  a new `Request` for it is opened.
- `Recommend` with `check_verdict: refused` clears any earlier recommendation and sets
  `no_recommendation_reason` to the report; a verified one with no ground or strength `none` is
  refused `UngroundedRecommendation`; a verified non-strong one with `default_on_silence` is
  refused `NoDefaultOnSilence`; neither invariant of `decision.yaml:270-272` can be broken by any
  sequence of the eight commands (a test drives random sequences over a fixed request).
- Under `holder: rule_and_decider_final`, `Decide` by `decider:x` on the option whose
  `consequence.grant` is `denied` is Decided, and on the option whose `grant` is `granted` is
  refused `NotEligible`.
- Under `holder: decider`, `Escalate` to `timo` leaves the request Escalated with holder
  `person`, and a following `Decide` by `decider:x` is refused `NotEligible`. `Escalate` from
  Escalated moves `addressed_to` to the new person; under `rule_final` it is refused
  `EscalationSuppressed` and nothing changes.
- `Decide` on an option that produces a record leaves `produced_record` absent and keeps
  `decided_case_revision`; `RecordProduced` sets it; the same `RecordProduced` again answers
  `already-recorded` and changes no byte of the stored request; a different record is refused
  `ProducedRecordConflict`.
- `Decide` on an outcome's "leave it untaken" option (no `produces`) is Decided, and
  `RecordProduced` on it is refused `NoRecordToProduce`.
- On a request with `expires_at` 12:00, `Expire` at 11:59 and `TakeRecommendation` by `silence`
  at 11:59 are refused `NotDue`; on a request with no `expires_at` both are refused `NotDue` at any
  instant; `Expire` at 12:00 on a request with no default on silence answers `expired`.
- `Supersede` of a Decided request keeps `decided_option` and `produced_record`.
- The in-memory store keeps two `EscalationSuppressedRecord`s appended beside `DR-1` and reads
  them back in order; reading the records of a request with none gives an empty list; appending
  beside an unknown request is refused `NoSuchRequest`.
- `OpenDecisionsForPrincipal` lists Open and Escalated requests addressed to the principal and
  nothing else.
- `crates/loom-decision/Cargo.toml` names no Commission, Canon, executor, `llm` or Mandate crate
  (a test reads the manifest), and `crates/loom-executor/tests/crate_names.rs` lists the crate.
- `task docs-check` holds the generated ESS reference page of the domain; `task check` exits 0.

## Out of scope

Durable storage (`story:decision-request-file-store`). Resolving a holder or validating a class
(`story:autonomy-grant-table`). The grounding check (`story:decider-port-and-grounding-check`).
Computing the record key of a stop, and deciding when a suppressed escalation is kept
(`story:decision-routing-by-holder`). Writing any Canon record. Any CLI command.
