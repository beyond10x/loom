---
format: aep.planning-md/3
id: story:open-record-for-a-running-child
kind: story
status: draft
title: Opening a child copies the parent's records its protocol requires at open (Canon required_at_open), or opens nothing
summary: 'a downstream consumer A13: the open case_outcome record for a running child already exists; Canon 0.15.0 required_at_open is a copy of required parent records into the child at open; needs a Canon pin upgrade from 0.11.0 first (not drafted); D1 a the governor''s open_children copies, D4 a refuses the opening with MissingAtOpen; no bindings ESS change; Added'
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/tests/bound_open.rs
- confidence: cited
  path: crates/loom-governor/src/children.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/child_cases.rs
revision: 10
---
## Outcome

When Loom opens a child case whose protocol marks an evidence kind `required_at_open: true`
(Canon 0.15.0), it copies a current record of that kind from the parent into the child, or it
opens no child and says which kind is missing. That is the runner's half of Canon's
`story:supplied-evidence-required-at-open`: "Runner at case open: copies a current record of the
kind into the child, or refuses to open it; Canon offers `eval::missing_at_open`" (Canon `0.15.0`,
`.engineering/planning/story/supplied-evidence-required-at-open.md:96`). Asked by a downstream consumer
(A13). Cites are at `03f64d4e` (the wave 2026-11-23-w8 plan head, after 0.60.0) unless they name
Canon.

**Before.**

- **Loom builds on Canon 0.11.0.** Five `b10x-canon` lines name `tag = "0.11.0"`
  (`crates/loom-governor/Cargo.toml:18`, `crates/loom-bindings/Cargo.toml:14`,
  `crates/loom-executor/Cargo.toml:10`, `crates/loom-protocols/Cargo.toml:14`,
  `crates/loom-tree/Cargo.toml:14`), `Cargo.lock:179-181` pins `e81f970d`, and `AGENTS.md:790-812`
  states the rule and what Loom reads of each Canon release. Canon 0.11.0 has no
  `required_at_open` and refuses unknown keys of a protocol (`#[serde(deny_unknown_fields)]`,
  Canon `0.11.0` `crates/canon/src/model/mod.rs:73`, `:112`, `:127`). So a protocol of
  a downstream consumer that uses the key does not register in Loom at all today.
- **In Canon 0.15.0** the key is an evidence-kind field that needs `supplied_by` on the same kind
  (`crates/canon/src/model/mod.rs:222`, `ir/mod.rs:225`, `:352`). `eval::missing_at_open(ir, case,
  evidence, at)` returns the required kinds of which no record applies to the case at `at`, read
  through the revision-binding and freshness stages; Canon opens nothing (`eval/mod.rs:416-430`,
  `:574-601`). `canon check-set` proves statically that an opener's precondition implies a
  current record (`unsupplied-at-open`, Canon `CHANGELOG.md` 0.15.0).
- **Loom opens a child with no evidence.** `CanonGovernor::open_children` builds each child with
  the parent's revision of every artifact the child declares, `UNREVISED` (`r0`) for the rest, no
  case input and its independence (`crates/loom-governor/src/children.rs:510-527`, `opened`
  `crates/loom-governor/src/lib.rs:2052`), writes the children, then the parent once
  (`children.rs:589-639`). Nothing of the parent's evidence reaches the child.
- **The "open record for a running child" is already written** (that half of the ask is served
  since `story:child-case-runtime`): the parent's one write holds, per child, a `case_outcome`
  record with result `open`, producer `loom-child-case`, about the `case_open` subject
  (`children.rs:5-7`, `:383-386`, `:611-626`; `OPEN_RESULT` `:33`), replaced by the child's outcome
  when it terminates. A composer reads a running child through it: a `children` claim stays not
  true while a child is open (`crates/loom-governor/tests/child_cases.rs:345`, `:457`). The open
  step also writes the binding's own `from: effect` records in the parent, such as a
  `<child>_opened` record (`crates/loom-bindings/src/open.rs:480-528`), which is the guard Canon
  0.15.0 advises for an opener of counted children. No change here.

**After.**

1. **Predecessor.** Loom builds on Canon 0.15.0 or later, by a story of its own that moves the
   five lines and `Cargo.lock` together (`AGENTS.md:790-795`). Not drafted here (Edges).
2. **Copy at open (D1 a, D2 a).** In `open_children`, after a child's protocol is checked and
   before anything is written (`children.rs:509-532`), for each evidence kind the child's IR marks
   `required_at_open`: take the parent's held record of that kind the parent received last, about
   an artifact the child declares, at the child's revision of it (the parent's own revision,
   `:516-527`). Copy it into the new child's state: a new evidence id derived from the child id and
   the source record's id (as `outcome_evidence` is derived, `:549-552`), so a restart writes the
   same; the source's kind, subject, `subject_revision`, producer, result, `observed_at` and cited
   observations kept (D3 a); `provenance` naming the parent case and the source record.
3. **Refuse what is missing (D4 a).** Then `b10x_canon::eval::missing_at_open(child_ir,
   child_case, copied, at)` with the opening's instant `at`. A kind it returns refuses the whole
   opening with a new `ChildError::MissingAtOpen { name, kinds }` ("child `<name>` needs a current
   record of `<kind>`, … at open; the parent holds none") before the first child is written:
   nothing is written. The open step's refusal carries that text (`open.rs:561`).
4. **Take-over.** A child a restart left unlinked (`children.rs:576-583`) is taken over as today;
   it already holds its copies.
5. **Docs.** `AGENTS.md` § Child cases (`:814-835`) gains one sentence on the copy and the refusal.

## Decisions

- **D1. Who copies.** Default **a**: the governor, in `open_children`, where the child's state is
  built and the write order is kept. **b**: the open effect port in `open.rs`: it would need the
  child's IR and a second write into the child, breaking the "children first, parent once" order.
  **c**: `tree run` after the step: the child would be readable without its record.
- **D2. Which record.** Default **a**: per kind, the one the parent received last of those about
  an artifact the child declares at the child's revision of it; `missing_at_open` then decides
  whether it applies (freshness at `at`). **b**: every such record: a copied pair that disagrees
  would make the child's claim `unknown`. **c**: the opener names what it supplies
  (`case_open.supplies` or a binding member): Canon chose the precondition proof instead (its
  D9 a, `supplied-evidence-required-at-open.md`), and Loom would check what Canon proves.
- **D3. The copy's facts.** Default **a**: the source's `observed_at` and producer, so `max_age`
  runs from the original observation. **b**: the opening's instant: a stale record would arrive
  fresh in the child, defeating `max_age`.
- **D4. A missing record.** Default **a**: refuse the opening, nothing written, as Canon's
  contract says ("or opens nothing", `eval/mod.rs:419-420`). **b**: open the child without it:
  the case would wait for good, the gap Canon closed.
- **D5. The refusal's form.** Default **a**: a `ChildError` variant whose text the open step
  carries, as every other `ChildError` today (`open.rs:561`): no ESS change. **b**: a variant
  `MissingAtOpen` of `loom.bindings.OpenRefusalReason` (`ess/domains/bindings.yaml:690-711`): an
  ESS change of the bindings domain, for a refusal no consumer reads by reason yet.

## ESS first

None under D5 a. Under D5 b: the `OpenRefusalReason` variant and its comment, then
`task generate`, `task suite-generate` (`SLOT_TIMEOUT: 30m`) and `task docs-generate`, and the
story then depends on `story:results-row-records-nothing` (Edges).

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call. Stubs are POSIX
sh, never Python. A slot job ends within 10 minutes, or names its time with `slot --timeout` (at
most 30 minutes). No test binary is added. All tests need the Canon 0.15.0 pin; before it, every
fixture protocol below is refused at registration (Canon 0.11.0, `unknown field
required_at_open`), which is the failing-first state.

**`crates/loom-governor/tests/child_cases.rs`** (the fixture's `catalog`, `root` and
`open_children` helpers, `:52`, `:96`, `:161`), with a child protocol whose kind `tickets_read`
declares `supplied_by` and `required_at_open: true`, about an artifact `brief` both protocols
declare:

1. `a_required_record_is_copied_into_the_child_at_open`: the parent holds a current
   `tickets_read` record about its revision of `brief`; the opening writes the child with exactly
   one `tickets_read` record, of the source's producer, result, `observed_at` and
   `subject_revision`, under an id derived from the child and the source; Canon admits the
   child's action that needs it. Failing first (after the pin): the child holds no record.
2. `a_missing_required_record_refuses_the_opening_and_writes_nothing`: the parent holds none:
   `ChildError::MissingAtOpen` naming the child and `tickets_read`; no child case is held, the
   parent's file is unchanged (no link, no `open` record).
3. `a_stale_required_record_refuses_the_opening`: the parent's record is older than the kind's
   `max_age` at `at`: refused as in test 2 (D3 a: a copy stamped `at` would pass).
4. `a_record_about_another_revision_is_not_copied`: the parent's only record is about an earlier
   revision of `brief`: refused.
5. `a_restart_after_the_child_was_written_keeps_one_copy`: the child is written and the parent's
   write is cut (as
   `a_restart_between_the_child_opened_and_the_link_recorded_adopts_the_child_and_opens_no_second`,
   `:737`, cuts it); the next opening takes the child over, which holds one copy.
6. Guard: `performing_the_opening_action_opens_the_child_and_writes_one_open_record_in_the_same_write`
   (`:345`) passes unchanged: a child of a protocol with no required kind opens as today.

**`crates/loom-bindings/tests/bound_open.rs`** (through the open port, `:124`, `:191`):

7. `an_opening_without_a_required_record_is_refused_naming_the_kind`: the step is refused with a
   text naming the child and the kind, and the parent holds no new record and no link.

**Documents.** Read in the diff: `AGENTS.md` § Child cases says that a kind marked
`required_at_open` is copied from the parent at open, or the opening is refused naming it;
`CHANGELOG.md` holds the bullet below.

**Gate.** `task check` exits 0.

**CHANGELOG** (`### Added`; not Breaking: no protocol Loom registers today can declare the key,
and a child without it opens as before): "Opening a child whose protocol marks an evidence kind
`required_at_open` copies the parent's current record of that kind into the child, or refuses the
opening naming the kind (`story:open-record-for-a-running-child`)."

## Does not do

- Move the Canon pin (the predecessor story, Edges).
- D1 b: copy in the open port. D1 c: copy in `tree run`.
- D2 b: copy every record of a required kind. D2 c: an opener-named supply list.
- D3 b: stamp a copy with the opening's instant.
- D4 b: open a child without its required record.
- D5 b: an `OpenRefusalReason` variant (an ESS change).
- Write another record for a running child: the `case_outcome` `open` record already exists
  (Before); a composer reads it today.
- Read `runs_only_when` or the `children` `count` form of Canon 0.15.0 beyond what Canon's own
  evaluation does.

## Scope

Derived 2026-11-24 from the source at `03f64d4e` and Canon at tags `0.11.0` and `0.15.0`.
**Cited**: read at the line named. **Inferred**: a reading that could be wrong.

- `crates/loom-governor/src/children.rs` (`:373-400`, `:509-532`, `:549-552`, `:576-583`,
  `:589-639`) — cited; the copy, the check and `ChildError::MissingAtOpen` (`:63-89`).
- `crates/loom-governor/src/lib.rs` (`opened` `:2052`, `HeldEvidence` `:394`, `applies` `:2442`)
  — cited; the child's state takes the copies (inferred: `opened` or the caller pushes them).
- `crates/loom-bindings/src/open.rs` (`:538-561`) — read, not changed under D5 a.
- Tests: `crates/loom-governor/tests/child_cases.rs` (`:52`, `:96`, `:161`, `:345`, `:737`),
  `crates/loom-bindings/tests/bound_open.rs` (`:124`, `:191`) — cited.
- `AGENTS.md` (`:790-835`) — cited; `CHANGELOG.md` — one `### Added` bullet under
  `[Unreleased]`.
- Under D5 b only: `ess/domains/bindings.yaml` (`:690-711`), `generated/ess/loom`,
  `generated/rust/loom`, `website/docs/reference/.generated.json` — inferred.

## Edges

- **Needs a Canon pin story first** (suggested id `story:canon-0-15-pin`, not drafted here): move
  the five `b10x-canon` lines and `Cargo.lock` from 0.11.0 to 0.15.0, update
  `crates/loom-governor/tests/canon_0_10_0.rs:327-334` (it asserts the decision document names
  `0.11.0`) and `AGENTS.md:790-812`. Measured cost from Canon's changelog: 0.12.0, 0.13.0 and 0.14.0
  change neither `protocol/1`, `canon-ir/1`, `canon-case/1` nor the `canon-decision/1` model;
  0.14.0 breaks `canon simulate` output only; 0.15.0 adds three optional keys and writes no new
  `canon-ir/1` key for a protocol without them. Each release changes
  `computed_from.semantics.canon`, so every decision document and frontier id changes (no store
  holds one, `AGENTS.md:810-812`). Library changes Loom calls (`eval`, `ir::check_openings`,
  `model::parse`) between 0.11.0 and 0.15.0 are not read here: the pin story measures them.
  Loom builds no Canon struct literal (`git grep` over `crates/`, none found).
- **ESS.** None under D5 a, so no `depends_on story:results-row-records-nothing`. Taking D5 b
  adds that edge, and the story then merges after the 0.61.0 ESS-changing stories (timeout,
  wildcard, program-error, spent) and that story, as one inputs digest
  (`generated/ess/loom/synthesis.txt:5`).
- **Shared files.** `children.rs` with no open story; `AGENTS.md` and `CHANGELOG.md` with every
  story of its wave, one bullet each.
