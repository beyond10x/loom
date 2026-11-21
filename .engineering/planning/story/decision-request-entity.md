---
format: aep.planning-md/3
id: story:decision-request-entity
kind: story
status: active
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
- confidence: cited
  path: Taskfile.yml
- confidence: inferred
  path: crates/loom-conformance/Cargo.toml
- confidence: cited
  path: crates/loom-conformance/src/decision.rs
- confidence: inferred
  path: crates/loom-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-conformance/tests/adversary2_w2_conformance_status.rs
- confidence: cited
  path: crates/loom-conformance/tests/conform.rs
- confidence: cited
  path: crates/loom-decision
- confidence: cited
  path: crates/loom-decision/tests/decision_request.rs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: crates/loom-executor/tests/ess_gate.rs
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/decision
- confidence: cited
  path: ess/scenarios/decision/merge-a5-veto.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: inferred
  path: website/data/ess/loom-decision.domain-graph.json
revision: 42
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:18Z", actor: "agent:sdc", revision: 10, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T13:30:58Z", actor: "agent:sdc", revision: 35, decided_on: {"recorded":{"review_outcome":2}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w7"}
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

`story:ess-format-37` is delivered (wave 2026-11-21-w6) and `ess/system.yaml` declares `ess/37`:
the dependency is met. This story needs `ess/37` for `ordered: true` on `Strength` and
`AutonomyLevel` (from `ess/36`); both stories change `ess/system.yaml`. The two stories
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

## As built, wave 2026-11-21-w7

Where the unit's commits (`c1a1f443..838536f2`, fourteen) differ from the text above, the commits
are what holds:

- The story does not mention the ESS gate; the unit changed it (`sdc`, option A; `9450c100`).
  Step 3 of `crates/loom-executor/tests/ess_gate.rs` synthesizes with `--suite-format 5` (`:97`)
  and passes when every refusal occurrence is covered by an authored `covers:` scenario
  (`synthesize_holds`, `:219`); `task conform` fails when a cover is skipped, unsupported or
  unreported (`unexecuted_covers`, `conform.rs:430`). Rule: AGENTS.md:206-217.
- The domain makes ess 0.96.0 refuse 17 occurrences: 10 at `3be62683` (8 ESS-SYNTH-004: the seven
  `Expired` refusals and `supersede`; 2 ESS-SYNTH-001 on `RecordProduced/recorded`, as in the SDC
  file) and 7 more after `kind-not-principal` (ESS-SYNTH-003, which the two SYNTH-001 also read
  after it; commit bodies of `54cf9f95` and `86313d78`). Four authored scenarios in
  `ess/scenarios/decision/` cover them: `expired-request-refuses-every-command`,
  `escalated-request-is-superseded`, `produced-record-is-recorded-once`,
  `decide-kind-guards-under-a-rule` (`4d75156b`, `54cf9f95`). `COVER_TABLE` pins each occurrence
  to its cover (`conform.rs:69`; `0357f12a`).
- `Decide` and `TakeRecommendation` have a guard the story does not name: `kind-not-principal`
  refuses `NotEligible` when `principal_kind` disagrees with the form of `principal`
  (`decider:<kind>`, `rule:<name>`, `silence`, any other name a person), first in both commands
  (`decision.yaml:603`, `:700`; `apply.rs:411`, `:527`). Decided by `sdc` after pass 1, F1
  (`54cf9f95`, `b6dfb3a3`, `0357f12a`); the SDC's contract lacks it.
- One open request per record key is an obligation of `DecisionStore::write`, not only of the
  lookup in `Request` (`store.rs:133-137`; the in-memory store holds it in one step, `:229-250`).
  `StoreConflict` carries `record_key_held_by` (`store.rs:41`); `Request` answers `RecordKeyOpen`
  naming that holder (`apply.rs:291-298`) and reads the store before `open_for_record`
  (`:222-223`). F3; `0357f12a`. A `Request` for an id already stored is `StoreConflict`, a Rust
  error, not an ESS outcome (`apply.rs:205`; AGENTS.md:1302-1303).
- The view `DecisionRequests` carries `record_key` and `decided_case_revision`
  (`decision.yaml:1023`, `:1053`), which the list of changes does not name (F6; kept).
  `decided_case_revision` is stored as given (`apply.rs:491`, `:588`), compared with nothing.
- The conformance run checks no invariant after an authored scenario's steps (a mutant; `838536f2`,
  its commit body); the two `invariant/after` occurrences have their own test in
  `crates/loom-decision/tests/decision_request.rs:675`,
  `the_stored_request_keeps_every_invariant_after_recorded_and_already_recorded` (`838536f2`).
- Changed outside Scope: the six scenarios beside `merge-a5-veto.yaml` and their lines in
  `ess/ess-inputs.yaml`; `Taskfile.yml`; AGENTS.md; `conform.rs` and `ess_gate.rs`;
  `crates/loom-conformance/tests/adversary2_w2_conformance_status.rs` (`--scenarios ess` added
  to its synthesize call, no assertion changed, `2a008cf2`); the reviewers' test files in
  `crates/loom-decision/tests/`: `security_w20261121w7_u1.rs` (`732fd2e9`) and
  `security_w20261121w7_u1_pass2.rs` (`638213b5`).
- Not closed (`review-result:decision-request-entity-security-1`, `-2`): the fact report names
  the trigger of the 8 ESS-SYNTH-004, not the cause; pass 2 did not check whether a
  behaviour-keeping change of the specification removes the 7 later occurrences.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w7 from the unit's commits (`git diff --stat
c1a1f443 838536f2`: 60 files, 13136 insertions, 151 deletions). Every line is cited from that diff.

- **Primary surface:** `ess/domains/decision.yaml` and `crates/loom-decision` — cited, 1 and 9 of
  the 60 changed files (the crate: `Cargo.toml`, five `src/` files, three test files); the rest are
  3 ESS files in all with the domain, 7 scenarios, 25 generated files, 4 site files, 5 under
  `crates/loom-conformance`, 2 under `crates/loom-executor`, `Cargo.lock`, `Taskfile.yml` and 3
  documents
- **Files (ESS sources):** `ess/domains/decision.yaml` (new, 1057 lines: domain `loom.decision`, 71
  declarations among them the entity `DecisionRequest`, the actors `Runtime`, `Person` and
  `Decider`, the eight commands, nine events and three views), `ess/system.yaml` (1 line:
  `loom.decision`), `ess/ess-inputs.yaml` (8 lines: the domain and seven `scenarios:` entries)
- **Files (scenarios):** `ess/scenarios/decision/` — new, 7 files, 714 lines: `merge-a5-veto.yaml`
  (133), `expired-request-refuses-every-command.yaml` (170), `escalated-request-is-superseded.yaml`
  (82), `produced-record-is-recorded-once.yaml` (111), `decide-kind-not-principal.yaml` (62),
  `take-recommendation-kind-not-principal.yaml` (61), `decide-kind-guards-under-a-rule.yaml` (95);
  four of them carry `covers:` for 17 refusal occurrences synthesis cannot reach
- **Files (generated output):** `generated/rust/loom/**` — cited, 25 files from `ess synthesize
  --layout crate` (`task generate`): new `src/decision.rs` (2009 lines) and `src/actor.rs` (83),
  `plan.json` (659 and 29), `PLAN.md` (85 and 2), `src/primitives.rs` (400 and 1),
  `src/behaviour.rs` (177 and 1), `src/lib.rs` (3 and 1), and 18 files of 1 and 1 each (`Cargo.toml`
  and 17 `src/*.rs`: the contract digest on line 2); also
  `website/docs/reference/ess/loom-decision.md` (new, 986 lines),
  `website/data/ess/loom-decision.domain-graph.json` (new, 251),
  `website/docs/reference/ess/index.mdx` (47 and 38), `website/docs/reference/crates.md` (3 and 2)
- **Files (source, `crates/loom-decision`):** new crate, `Cargo.toml` (14 lines, depends on the
  generated `loom` crate alone), `src/apply.rs` (958, the eight commands in ESS's precedence, 2 unit
  tests), `src/store.rs` (291, the `DecisionStore` port and `InMemoryDecisionStore`),
  `src/instant.rs` (167, RFC 3339 reading, 2 unit tests), `src/views.rs` (91, the three views),
  `src/lib.rs` (51)
- **Files (source, `crates/loom-conformance`):** `src/decision.rs` (new, 1203 lines: the adapter
  that applies the eight commands and three views to the scenario's store), `src/lib.rs` (26 lines:
  `mod decision;`, a `decisions` field on `Live`, eight command arms, one view arm), `Cargo.toml` (3
  lines: the `b10x-loom-decision` path dependency and the description)
- **Files (new tests):** `crates/loom-decision/tests/decision_request.rs` (1657 lines, 26 `#[test]`,
  the story's cases), `crates/loom-decision/tests/security_w20261121w7_u1.rs` (1258 lines, 6
  `#[test]`, the security reviewer's pass-1 cases) and
  `crates/loom-decision/tests/security_w20261121w7_u1_pass2.rs` (329 lines, 3 `#[test]`, pass 2),
  committed unchanged
- **Files (existing tests):** `crates/loom-conformance/tests/conform.rs` (328 lines, still 3
  `#[test]`: the pinned `COVER_TABLE` of 17 rows, the check that every cover is executed and that
  the suite's cover map equals the table, self-checks on moved copies),
  `crates/loom-executor/tests/ess_gate.rs` (222 lines, 10 `#[test]` become 12: gate step 3 reads
  `--suite-format 5` counts and fails an uncovered refusal,
  `gate_over_a_copy_synthesize_refuses_with_exit_0_fails_at_synthesize` is renamed
  `gate_over_a_copy_with_an_uncovered_refusal_fails_at_synthesize`),
  `crates/loom-executor/tests/crate_names.rs` (7 lines: `EXPECTED` from 30 to 31 entries, gains
  `b10x-loom-decision`), `crates/loom-conformance/tests/adversary2_w2_conformance_status.rs` (2
  lines: `--scenarios ess`) — cited
- **Files (manifests and lock):** `Cargo.lock` (8 lines: the `b10x-loom-decision` package, and the
  dependency line in `b10x-loom-conformance`), `Taskfile.yml` (2 lines: the `ess-gate` description)
  — cited
- **Files (documents):** `AGENTS.md` (75 lines: § ESS item 3 and the rule on covering a refusal, the
  new § Decision requests, the `commission:ess-gate` sentence), `CHANGELOG.md` (1 line: an `Added`
  bullet), `README.md` (1 line: the crate's row) — cited
- **Symbols (added, `b10x_loom_decision`):** functions `request`, `recommend`, `decide`,
  `take_recommendation`, `record_produced`, `escalate`, `supersede`, `expire`,
  `open_decisions_for_principal`, `artifact_decision_history`, `decision_requests`; trait
  `DecisionStore` (`read`, `write`, `list`, `open_for_record`, `append_record`, `records`) and
  `InMemoryDecisionStore` (`new`, `with_requests`); types `DecisionError`, `Undecided`,
  `StoreFailure`, `StoreConflict`, `WriteError`, `AppendError`; re-exports `model`
  (`loom::decision`) and `Timestamp` — cited
- **Symbols (added, generated `loom`):** modules `decision` and `actor` (`Actor`, `Caller`, `may`),
  the trait `DecisionRequestStorage` in `behaviour`, and the helper module `invariant` in
  `primitives` — cited
- **Symbols (changed, none public):** `crates/loom-conformance/src/lib.rs` `LoomTarget`'s command
  and view dispatch (new arms), `Live` (new field); `crates/loom-executor/tests/ess_gate.rs`
  `refusals` now returns the counts `Refusals { occurrences, covered }` and gains
  `synthesize_holds`; no public item removed — cited
- **Not predicted and touched:** `AGENTS.md`, `Taskfile.yml`,
  `crates/loom-executor/tests/ess_gate.rs`, `crates/loom-conformance/tests/conform.rs`,
  `crates/loom-conformance/tests/adversary2_w2_conformance_status.rs` (the gate now counts covered
  refusals), the six scenario files besides `merge-a5-veto.yaml`, the two security test files (under
  the typed `crates/loom-decision` row but unnamed), and `generated/rust/loom/src/actor.rs`,
  `src/behaviour.rs`, `src/primitives.rs` (the Scope named `generated/rust/loom/**` as a whole)
- **Predicted and not touched:** none; every row of the typed scope is in the diff, and the
  generated, document and lock files the Scope inferred are all in it
- **Review:** `decision-request-entity-security-1` (red at `2a008cf2`, six findings: a listed
  decider naming itself a person decides under a rule because `principal_kind` is the command's own
  claim; the doc says who may decide is never read from the command; two open requests for one
  record key through the gap between check and write; a scenario covering a refusal it does not test
  passes the gate; the `broken_invariant` check in `write` is untested; the view `DecisionRequests`
  gains `record_key` and `decided_case_revision`, which the story does not list) and `-security-2`
  (green at `0357f12a`: F1, F3, F4 and F5 closed by `kind-not-principal`, the record-key check
  inside the store's write, the pinned `COVER_TABLE` and a test of the invariant check, F2 holds as
  fixed; one note left, the docs said synthesis arranges no Decided request for `RecordProduced`
  although `option-produces-none` is still synthesized, which the later commits `86313d78` and
  `e1373728` reword); the plan-time results `decision-request-entity-e3r15-acceptance` and
  `-e3r15-design` are not read for this section
