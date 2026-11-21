---
format: aep.planning-md/3
id: story:decision-request-entity
kind: story
status: proposed
title: loom.decision declares the decision request and its eight commands, and b10x-loom-decision applies them
summary: 'The final SDC contract moved into Loom''s ESS at ess/37, ordered Strength and AutonomyLevel kept: class ids, the decided case revision, one open request per record id, and the DecisionStore port with the records kept beside a request'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:ess-format-31
- depends_on: story:ess-096-upgrade
- depends_on: story:ess-format-37
scope:
- confidence: inferred
  path: crates/loom-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-conformance/src/decision.rs
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-decision
- confidence: cited
  path: crates/loom-decision/tests/decision_request.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/scenarios/decision/merge-a5-veto.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: inferred
  path: website/data/ess/loom-decision.domain-graph.json
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:18Z", actor: "agent:sdc", revision: 10, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

Loom's ESS has a domain `loom.decision` (`ess/domains/decision.yaml`) whose entity
`DecisionRequest` is the decision a run asks of a person, a decider or a rule: its case, its
`question_key` and `record_key`, its subject at a revision, its question and options, its autonomy
level, class and holder, the eligible principals and the one it is addressed to, its
recommendation and grounds, its escalation, its decision with the case revision it was taken at
(`decided_case_revision`), the Canon record the decision produced, and its supersession. The
domain is taken from `~/beyond10x/sdc/ess/domains/decision.yaml` (at `00e3557`) as session `sdc` declared it
final on 2026-11-21, with its guards as written, changed only where listed below. It has eight
commands: `Request`, `Recommend`, `Decide`, `TakeRecommendation`, `RecordProduced`, `Escalate`,
`Supersede` and `Expire`.

A new crate, `b10x-loom-decision` (`crates/loom-decision`), applies the commands to a request and
answers each outcome the domain declares, over a `DecisionStore` port with an in-memory
implementation, and the three views (`OpenDecisionsForPrincipal`, `ArtifactDecisionHistory`,
`DecisionRequests`) as functions over a store. The port:

- reads a request;
- writes it when its state is still the one read, and otherwise refuses `StoreConflict`, naming
  the request id, the state read (absent for a new request) and the state found; a new request
  whose id was stored since the read is refused the same way;
- lists requests;
- finds the Open or Escalated request for a record key;
- appends an `EscalationSuppressedRecord` beside a request, refused `NoSuchRequest` for an unknown
  one;
- reads the records beside a request in the order they were appended.

A store that cannot read or write answers `StoreFailure`, naming where and why. `StoreConflict`
and `StoreFailure` are the port's Rust result types in `b10x-loom-decision`, not ESS errors: no
command of the domain answers them. The crate depends on the generated `loom` crate only: no
Commission, executor, Canon, model provider or Mandate.

Taken from the SDC file as written, each stated by session `sdc`, 2026-11-21, beyond its earlier
answers (third message):

- An option's consequence names the kind of Canon record it produces, `produces`, absent when it
  produces none (`decision.yaml:95`); an outcome's "leave it untaken" produces none (`:67`).
- The denying option is the one whose `consequence.grant` is `denied` (`:595`); no other field
  marks it.
- `Escalate` always sets the holder to `person` (`:760-764`), so a decider can no longer decide
  the request.
- `RecordProduced` with the record already stored changes nothing (`:708-713`); a different
  record is refused `ProducedRecordConflict` (`:714-718`); on a request whose decided option
  produces no record it is refused `NoRecordToProduce` (`:719-723`).
- `Expire` takes the tick's instant `at` (`:819`); `Expire`, and `TakeRecommendation` by
  `silence`, before `expires_at` or on a request with none are refused `NotDue` (`:667-672`,
  `:825-829`).
- `Request` in class `escalation` is refused `NotARequestClass` (`:416-419`).

Changes from the SDC file:

- A decision class is `DecisionClassId`, a newtype over `String`, not the enum of
  `decision.yaml:176-178`; `budget_overrun` and `limit_hit` are built in (session `sdc`,
  2026-11-21, point 9; the file says so in prose, `:180`). The guard `decision_class ==
  escalation` (`:417`) compares with the text `"escalation"`. The domain checks no class beyond
  `escalation`; `story:autonomy-grant-table` validates it against the host's classes document.
- `Decide` and `TakeRecommendation` take `case_revision`, the case revision at the decision, kept
  as `decided_case_revision` (answer e); the SDC inputs (`:547-554`, `:633-638`) have none. The
  record is written at that revision (`story:decided-record-written-then-recorded`).
- `Request` takes `record_key`, the record id the request's decision would produce at the case
  revision when it is opened (`<capability>@r<N>` for an approval with no scope and
  `<capability>[<key>=<value>,…]@r<N>` for one with a scope, `<outcome>/<decision>@r<N>`
  for an outcome, `question:<question id>@<subject revision>` for a question, N the governor's
  case revision; `story:decision-routing-by-holder` states the form and computes it), and stores
  it. It also takes
  `open_for_record`, the Open or Escalated request the runtime found in the store for that key, if
  any. With `open_for_record` given, `Request` is refused `RecordKeyOpen` naming that request, and
  nothing is opened (answer g; the coordinator chose refusal over supersession). The SDC file
  states this rule in prose only (`:546`; `~/beyond10x/sdc/docs/decisions.md:263-264`), so Loom
  enforces it here.
- `EscalationSuppressedRecord` is declared: the request id, class, subject at revision, reasons,
  `best` if any, and the cause (`SuppressionCause`: `escalate`, `failure`, `withheld`, `veto`,
  `rule_escalate`), with the instant it was kept. The SDC file names the event
  `escalation_suppressed` only in prose (`decision.yaml:186`, `:759`); the type is named apart
  from the error `EscalationSuppressed` (`:366-369`). `story:decision-routing-by-holder` writes
  it through the port; `story:escalate-when-conditions` adds the cause `condition`.
- The actor `Driver` (`decision.yaml:291-301`) is `loom.decision.Runtime`; `Person` and `Decider`
  keep their commands.

Format. Loom pins `ess` 0.96.0 since `story:ess-096-upgrade` (wave 2026-11-21-w5;
`ess/ess-inputs.yaml:2`, `.github/workflows/check.yml:60`), and `story:ess-format-37` raises
`ess/system.yaml` from `ess/31` to `ess/37` before this story and changes nothing else. The SDC
file declares `format: ess/37` and needs `ess 0.96` (`~/beyond10x/sdc/ess/system.yaml:1`,
`~/beyond10x/sdc/ess/ess-inputs.yaml:2`), so the copy is made at the format of its source and
keeps, as the SDC file writes them:

- `ordered: true` on `Strength` (`~/beyond10x/sdc/ess/domains/decision.yaml:109`, variants
  `none, weak, moderate, strong` at `:110`) and on `AutonomyLevel` (`:171`, variants `A0` to `A5`
  at `:172`), valid from `ess/36` (0.91.1; `website/docs/reference/spec-versions.md:78` in
  `~/beyond10x/ess` at tag `0.96.0`);
- `[]` stored by `sets:` into the required lists `grounds` and `decision_reasons`
  (`:446-447`, `:507`), valid from `ess/31` (0.69.0);
- `coalesce(produced_record.record, "") == input.produced.record` in `already-recorded`
  (`:710`), a call on the left of a comparison, valid from `ess/29` (0.67.0).

No construct of the SDC file is adapted or dropped for the format: `ess` 0.96.0 refuses a copy of
`~/beyond10x/sdc/ess/` declaring `format: ess/31` at the two `ordered:` keys only
(`unsupported_format_version`, naming `ess/36`) and validates it once they are removed, and the
SDC file itself validates at `ess/37`. The changes listed above each rest on an answer of session
`sdc`, none on the format.

Session `sdc` decided on 2026-11-21 that the copy keeps `ordered: true` on both enums: the SDC
compares both (an autonomy level at or above A5, a strength at or above a threshold), and a copy
that drops the order is a second definition that can disagree with the source. This replaces its
earlier choice, made while Loom pinned `ess` 0.86.0, which cannot read `ess/36`: stay at `ess/31`
and drop `ordered: true` (`.engineering/drafts/e3-sdc-answers-6.md:9-11`).

What the order changes at `ess` 0.96.0 (`~/beyond10x/ess`, tag `0.96.0`): a `min` or `max`
measure over a field of the enum is admitted and reads the variant declared last as the greatest,
and is refused as `type_mismatch` without the key
(`crates/specify/ess-domain/src/view.rs:1084-1089`, `:1693-1709`); below `ess/36` the key is
refused (`crates/specify/ess-domain/src/types.rs:1249-1270`); the compiled model carries
`ordered: true` on the type (`crates/specify/ess-compiler/src/ir.rs:402-407`); and
`ess verify diff` reports a flipped flag as `ordered-changed`
(`crates/verify/ess-diff/src/diff.rs:748-753`). Nothing else changes. A guard's comparison reads
no `ordered` flag (`crates/specify/ess-domain/src/expression.rs:3065`, `compare`), and the Rust
generator writes every enum with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`, flag or no flag
(`crates/generate/ess-synth/src/rust/items.rs:150-153`), so the generated `Strength` and
`AutonomyLevel` have no `PartialOrd` and no guard of the copy changes. The SDC file uses neither
order in its own declarations: its guards test strength by equality (`decision.yaml:277`, `:475`)
and its views (`:892-983`) declare no `min` or `max`. What holds the copy's order to the source is
the copy's variant lists, which a test holds to the SDC file's (Acceptance).

Its code goes in its own modules of `crates/loom-decision`, so a later story adds a module and a
`mod` line, not edits to this story's code.

## Why

Item 4 of the request: the entity "as the contract, moved into Loom's ESS when it is taken"
(`~/beyond10x/sdc/docs/decisions.md:351-353`). Loom's three ESS systems hold no decision-request
entity; Commission's `HumanDecisionRequest` is untyped JSON, "Commission's sources declare no
structure for it" (`ess/commission/domains/responsibility.yaml:106-109`). Every later story of
the epic reads or writes this entity, and the routing, file-store and inbox stories share the
records kept beside a request, so the port that carries them is declared here. Session `sdc`
decided on 2026-11-21 that a format story first raises Loom's main system to `ess/37`
(`story:ess-format-37`) and that the copy keeps `ordered: true`, `[]` and `coalesce` as written.
It replaced its earlier choice, `ess/31` with `ordered: true` dropped
(`.engineering/drafts/e3-sdc-answers-6.md`), which the 0.86.0 pin forced.

## Waits on

`story:ess-format-37` (wave 2026-11-21-w6): `ess/system.yaml` at `ess/37`, which `ordered: true` on `Strength`
and `AutonomyLevel` needs (from `ess/36`); both stories change `ess/system.yaml`. The two stories
it named before are implemented at `9b4170fb`: `story:ess-format-31` (`ess/31`, which `[]` in
`sets:` and the `coalesce` guard need) and `story:ess-096-upgrade` (the `ess` 0.96.0 pin).

## ESS first

`ess/domains/decision.yaml`, declared in `ess/system.yaml` and listed in `ess/ess-inputs.yaml`:
the types (with `DecisionClassId`, `EscalationSuppressedRecord` and `SuppressionCause`, and
`Strength` and `AutonomyLevel` with `ordered: true`), entity, actors, errors (the fourteen of the
SDC file at `00e3557`, `OptionOrAnswer` and `AnswerNotAllowed` included, and `RecordKeyOpen`),
commands, events (the nine of the SDC file) and views above, under `format: ess/37`, which
`story:ess-format-37` sets; this story adds only a `domains:` line to `ess/system.yaml`. The copy
holds every declaration of the contract, its guards included: the goals ground source, the
citation's `entry` and `digest`, the optional `Ground.supports`, and the free answer
(`recommended_answer`, `decided_answer`, `Decide.answer`) with its two guards. The crate answers
those guards as it answers every outcome. The behaviour behind the goals and answer members is
`story:goals-grounds-and-free-answers`, whose tests hold the free-answer guards and add only
Loom's own declarations to this file. On that commit `task drift` fails, and
`crates/loom-decision/tests/decision_request.rs` and `crates/loom-conformance/src/decision.rs` do
not compile.

## Acceptance

- `task conform` runs the synthesized `loom.decision` scenarios against
  `b10x-loom-decision` and every scenario of the domain executes; `ess/SKIPPED.md` gains no line
  for it.
- SDC's scenario `~/beyond10x/sdc/ess/scenarios/merge-a5-veto.yaml`, copied to
  `ess/scenarios/decision/merge-a5-veto.yaml` and listed under `scenarios:` in
  `ess/ess-inputs.yaml`, passes against `b10x-loom-decision`. The copy changes only: `domain:` and
  every `sdc.decision.` name to `loom.decision.` (`:2`, `:8`, `:36`, `:44`, `:61`, `:68`, `:80`,
  `:89`, `:100`, `:107`, `:114`, `:120`, `:128`); `actor: sdc.decision.Driver` to
  `loom.decision.Runtime` (`:9`, `:45`, `:108`, `:121`) and `sdc.decision.Decider` to
  `loom.decision.Decider` (`:69`, `:81`, `:90`); the `Request` input (`:10-33`) gains
  `record_key: "change.merge@r9"`; both `Decide` inputs (`:70-76`, `:91-97`) gain
  `case_revision: "r9"`; and the produced record `change.merge@c9` with `case_revision: "c9"`
  (`:111`, `:117`, `:124`, `:130`) becomes `change.merge@r9` with `case_revision: "r9"`, Loom's
  record id form.
- `Request` with one option is refused `TooFewOptions`, and `Request` in class `escalation` is
  refused `NotARequestClass`; neither stores anything.
- `Request` with `open_for_record: DR-1` is refused `RecordKeyOpen` naming `DR-1`, and the store
  is unchanged; after `DR-1` is Decided, the in-memory store finds no open request for the key and
  a new `Request` for it is opened.
- A stored request carries the `question_key` and `record_key` its `Request` gave, and a Decided
  one the `decided_case_revision` its `Decide` gave.
- Two writes of `DR-1` from the same read, into the in-memory store: the first is kept, the second
  is refused `StoreConflict` naming `DR-1`, and the store holds the first.
- `Recommend` with `check_verdict: refused` clears any earlier recommendation and sets
  `no_recommendation_reason` to the report; a verified one with no ground or strength `none` is
  refused `UngroundedRecommendation`; a verified non-strong one with `default_on_silence` is
  refused `NoDefaultOnSilence`; neither invariant of `decision.yaml:275`, `:277` can be broken by any
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
- On a request with a strong recommendation, `default_on_silence: true` and `expires_at` 12:00,
  `TakeRecommendation` by `silence` at 11:59 is refused `NotDue`; on the same request with no
  `expires_at` it is refused `NotDue` at any instant.
- On a request with no default on silence and `expires_at` 12:00, `Expire` at 11:59 is refused
  `NotDue` and `Expire` at 12:00 answers `expired`; on a request with no `expires_at`, `Expire` is
  refused `NotDue` at any instant.
- `Supersede` of a Decided request keeps `decided_option` and `produced_record`.
- The in-memory store keeps two `EscalationSuppressedRecord`s appended beside `DR-1` and reads
  them back in order; reading the records of a request with none gives an empty list; appending
  beside an unknown request is refused `NoSuchRequest`.
- `OpenDecisionsForPrincipal` lists Open and Escalated requests addressed to the principal and
  nothing else.
- `crates/loom-decision/tests/decision_request.rs` reads `ess/domains/decision.yaml` and
  `ess/system.yaml` from the repository and asserts that the declaration of
  `loom.decision.Strength` carries `ordered: true` and the variants `none, weak, moderate, strong`
  in that order, that the declaration of `loom.decision.AutonomyLevel` carries `ordered: true` and
  the variants `A0, A1, A2, A3, A4, A5` in that order, and that `ess/system.yaml` declares
  `format: ess/37`. The two lists are those of `~/beyond10x/sdc/ess/domains/decision.yaml:110`
  and `:172` at `00e3557`, written into the test, since CI has no SDC checkout. The test fails
  when either key is removed, two variants of either enum swap places, or the format is lowered.
- `ess specify compile --path ess --format json` on the commit carries `"ordered": true` on
  `types."loom.decision.Strength"` and `types."loom.decision.AutonomyLevel"` and on no other type
  of `loom.decision`. That is the one place the order shows in generated output: the generated
  `loom` crate declares both enums with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`, as it
  declares every enum (`crates/generate/ess-synth/src/rust/items.rs:150-153` in `~/beyond10x/ess`
  at tag `0.96.0`), and no code of this story compares a strength or a level by order.
- A `Request` stores `grounds` and `decision_reasons` as empty lists, and a `Recommend` withheld
  stores `grounds` as an empty list (the `[]` of the SDC file's `sets:`).
- `crates/loom-decision/Cargo.toml` names no Commission, Canon, executor, `llm` or Mandate crate
  (a test reads the manifest), and `crates/loom-executor/tests/crate_names.rs` lists the crate.
- `task docs-check` holds the generated ESS reference page of the domain; `task check` exits 0.

## Out of scope

Durable storage (`story:decision-request-file-store`). Resolving a holder or validating a class
(`story:autonomy-grant-table`). The grounding check (`story:decider-port-and-grounding-check`).
Computing the record key of a stop, and deciding when a suppressed escalation is kept
(`story:decision-routing-by-holder`). Writing any Canon record. Any CLI command. The format
raise (`story:ess-format-37`), an ESS toolchain change, and any `min` or `max` measure or Rust
order over `Strength` or `AutonomyLevel`. A guide or a `website/data/status.json` entry: the
crate is an internal library with no command.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `ess/domains/decision.yaml` (new domain `loom.decision`) and `crates/loom-decision` (new crate `b10x-loom-decision`) — cited (story Outcome, ESS first)
- **Files (ESS):** `ess/domains/decision.yaml`, `ess/system.yaml` (one `domains:` line, after `loom.bindings` at `ess/system.yaml:24`), `ess/ess-inputs.yaml` (one `specification:` line, `ess/ess-inputs.yaml:17`) — cited
- **Files (crate):** `crates/loom-decision/{Cargo.toml,src/**,tests/decision_request.rs}` — cited (`Cargo.toml` and `src` are inferred as the crate's own layout, as `crates/loom-bindings` has)
- **Files (conformance):** `crates/loom-conformance/src/decision.rs` — cited; `crates/loom-conformance/src/lib.rs` — inferred, needs a `mod decision;` line, a field on `Live` (`lib.rs:142-163`), command arms beside `run::RUN_ONCE` (`lib.rs:220-256`) and a view arm beside `resource::LOOM_AGENTS` (`lib.rs:375`); `crates/loom-conformance/Cargo.toml` — inferred, gains `b10x-loom-decision = { path = "../loom-decision" }`
- **Files (house test):** `crates/loom-executor/tests/crate_names.rs` — cited (story acceptance). `EXPECTED` is `[..; 29]` at `:18`, so the length becomes 30 and a tuple `("b10x-loom-decision", "loom-decision", Some("b10x_loom_decision"))` is added
- **Files (scenario):** `ess/scenarios/decision/merge-a5-veto.yaml` plus its `ess/ess-inputs.yaml` line — inferred path (acceptance says "copied into Loom"; sibling scenarios sit in `ess/scenarios/<group>/`)
- **Files (generated, regenerated not hand-edited):** `generated/rust/loom/**`, `website/docs/reference/ess/loom-decision.md`, `website/data/ess/loom-decision.domain-graph.json` — inferred, from AGENTS.md § Generated files and the `loom.bindings` commit `501c332a`
- **Symbols:** `DecisionRequest`, `DecisionStore`, `EscalationSuppressedRecord`, `SuppressionCause`, `DecisionClassId`, `OpenDecisionsForPrincipal`, `ArtifactDecisionHistory`, `DecisionRequests`, actor `loom.decision.Runtime`, errors `RecordKeyOpen` and `NotARequestClass` — cited (story). None exists in `ess/`, `generated/` or `crates/` (`git grep` found no `loom.decision`).
- **Documents:** none beyond the generated reference page. The story ships no guide, so `website/data/status.json` is not scoped (see "could not establish").
- **Confidence:** high for the ESS files and the crate, which the story names. Medium for `loom-conformance/src/lib.rs` and the scenario path: the dispatch is a closed `match` over command ids that returns `unavailable` for an unknown command (`lib.rs:256-261`), so the new commands cannot pass without an arm.
- **Would collide with:**
  - `ess/system.yaml`: `story:ess-format-37` (wave 2026-11-21-w6) changes its `format:` line from `ess/31` to `ess/37`, and this story adds a `domains:` line — ordered, this story `depends_on` it.
  - `ess/system.yaml`, `ess/ess-inputs.yaml`: `story:hash-chained-run-log` (`loom.runlog`) — no `depends_on` path; `story:tree-run-command` (`loom.tree`) — no path; `story:engine-configuration-document` (`loom.engine`) — no path; `story:agent-selects-the-connectors-relay`, `story:send-input-in-the-declared-shape`, `story:boundary-admits-declared-http-egress` (scenario lines in `ess-inputs.yaml`) — no path; `story:autonomy-grant-table` (`loom.autonomy`) — ordered, it `depends_on` this story.
  - `crates/loom-executor/tests/crate_names.rs`: `story:hash-chained-run-log`, `story:tree-run-command` — no path. A merge of any two of them must both bump `[..; 29]` and add a tuple.
  - `crates/loom-conformance/src/lib.rs`: `story:tree-run-command`, `story:agent-selects-the-connectors-relay`, `story:send-input-in-the-declared-shape`, `story:revalidation-membership-conformance` — no path (the first three also touch `loom-conformance/Cargo.toml`, except `revalidation-membership-conformance`, which touches `lib.rs` only).
  - `ess/domains/decision.yaml`, `crates/loom-decision/{Cargo.toml,src/lib.rs}`: every other story of the epic except `story:decision-suspension-reason` is ordered after this one by a `depends_on` path (graph: 19 of 20 siblings; the edges are direct only for `autonomy-grant-table` and `decision-request-file-store`). `decision-suspension-reason` touches only `ess/commission/` and Commission crates, so it does not collide.
  - `Cargo.lock`, `generated/rust/loom/**`: every story that changes `ess/` or adds a crate.
- **Safety fact:** the change adds a domain and a crate and edits no existing declaration, so no existing caller changes. `loom.bindings` landed as the same three `ess/` files (`501c332a`) plus a crate depending on `loom = { path = "../../generated/rust/loom" }` (`crates/loom-bindings/Cargo.toml:16`) — step 2, unproven. Whether `task crd-drift` or `task docs-check` move beyond the new domain's own page was not walked.
