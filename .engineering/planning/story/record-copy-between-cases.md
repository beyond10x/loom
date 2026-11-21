---
format: aep.planning-md/3
id: story:record-copy-between-cases
kind: story
status: active
title: A copy binding copies the latest current record of a kind from another case of the same tree
summary: Evidence from parent or sibling into a case, made by Loom and cited to its source
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:child-case-runtime
scope:
- confidence: inferred
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_copy.rs
- confidence: inferred
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-21T21:13:22Z", actor: "agent:sdc", revision: 9, decided_on: {"recorded":{"review_outcome":2}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
---
## Outcome

An action bound to a `copy` executor copies one evidence record from another case of the same
tree into the case that performs it: the latest record of the binding's `kind`, about the current
revision of its subject, held by the case under the protocol id the binding names. Loom makes the
copy in `b10x-loom-bindings`; no program runs. The copied record keeps its kind, subject, subject
revision, result, producer and instant, cites an observation that names the source case and the
source record's id, and loses only the `upstream_revisions` entries for artifacts the receiving
protocol does not declare. With no such record in the tree the step is refused and nothing is
written. One function, `copy_due`, says whether the source holds such a record that the receiving
case lacks; a run uses it to decide whether the action is worth taking.

## Why

Canon declares evidence flowing from child to parent (`case_outcome`) and nothing from parent to
child (`~/beyond10x/sdc/docs/boundary.md` § 2, limitation 4, and row 28). The SDC models it as an
action, `roadmap.read`, bound in four protocols to "copy the `roadmap_approval` record from the
`sdc.direction` case" (`~/beyond10x/sdc/driver/examples/sdc/sdc-epic.bindings.json`,
`~/beyond10x/sdc/driver/src/main.rs:303-320`, `693-711`). Only the runtime can honour that, and
without it the SDC's epic, spec and plan cases never reach their first precondition. It was not
among the seven requests; the epic's acceptance needs it.

## ESS first

`ess/domains/bindings.yaml` first: `CopiedRecord` (source case, source evidence id, dropped
upstream artifacts) and the refusals `NoSourceRecord`, `NoSourceCase` and `SubjectUndeclared`. On
that commit `task drift` fails, and `crates/loom-bindings/tests/bound_copy.rs` does not compile.
This is the fifth story that changes `ess/domains/bindings.yaml`; it depends on
`story:bound-agent-node` only to take the file after it.

## Acceptance

- In a fixture tree of a parent and two children, a record of the kind held by the first child at
  its subject's current revision is copied into the second child by the bound action, with the
  same result, producer, instant and subject revision.
- A source record about an earlier revision of its subject is not copied; with only such records
  the step is refused `NoSourceRecord`.
- With two source records at the current revision the later one is copied.
- An `upstream_revisions` entry for an artifact the receiving protocol does not declare is
  dropped and named in `CopiedRecord`; the others are kept.
- The copy cites an observation whose source is Loom's copy effect, whose payload names the
  source case and record, and whose instant is the source record's instant (the governor refuses
  a record whose `observed_at` differs from its observation's,
  `crates/loom-governor/src/lib.rs:1363-1411`).
- A source record the source case holds as not applying is not copied.
- The receiving case judges the copied record by its own `independence` map: a copied record
  whose producer that map does not list does not satisfy a match that demands independence,
  whatever the source case's map says.
- The cases of a tree and a case's held records with their applying state are read through
  `crates/loom-governor`: the tree walk comes from `story:child-case-runtime`, and this story
  adds the read of held records if that story left none.
- `copy_due` is true before the copy and false after it; performing the action while it is false
  writes nothing.
- A copied record's subject that the receiving protocol does not declare is refused
  `SubjectUndeclared`.
- A source protocol with no case in the tree is refused `NoSourceCase`.
- With a second, unrelated tree in the same store directory that holds a matching record, the
  step is still refused `NoSourceRecord`.
- `task check` exits 0.

## Out of scope

A declared imported kind or case input in Canon. Copying between trees. Copying without a bound
action.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (new crate from `story:bindings-document`; the `copy` executor, `copy_due`, the refusals) — cited
- **Files:** `crates/loom-bindings/tests/bound_copy.rs` — cited, named in `## ESS first`
- **Files:** `ess/domains/bindings.yaml` (`CopiedRecord`, `NoSourceRecord`, `NoSourceCase`, `SubjectUndeclared`) — cited, the fifth story to change it
- **Symbols:** `copy_due`, `CopiedRecord`, `NoSourceRecord`, `NoSourceCase`, `SubjectUndeclared` — cited
- **Also likely:** `crates/loom-bindings/src/lib.rs` (module declaration for the copy code; the story names no source file) — inferred
- **Also likely:** `crates/loom-governor/src/lib.rs` — inferred. The copy needs a source case's held evidence with its `applies` flag, and a walk from a case to its tree. `CanonGovernor::evidence` (lib.rs:1118) returns `EvidenceData` only. `HeldEvidence { data, applies }` (lib.rs:349) is reachable only through the store, which `CanonGovernor` owns privately (lib.rs:702) and neither store type clones. The parent and children links come from `story:child-case-runtime`, which names no read accessor for them. If that story ships none, this one adds a read to `crates/loom-governor/src/lib.rs`. A new method returning the already-public `HeldEvidence` changes no ESS type, so `ess/domains/governor.yaml` would not need to change.
- **Also likely:** a test file under `crates/loom-governor/tests/` for that read — inferred; no name to give
- **Not needed:** `crates/loom-commission/src/ports/evidence.rs` — inferred from reading it, see Safety fact. A copy keeps the producer and the instant through the existing `submit_evidence` signature.
- **Documents:** none the acceptance names. The bindings guide page under `website/docs/` comes from `story:bindings-document` and already covers the `copy` member. — inferred
- **Confidence:** medium — the crate, test and ESS file are cited. The governor read is inferred, because the story reads another case's evidence and walks the tree but names no governor function.
- **Would collide with:** `ess/domains/bindings.yaml` and `crates/loom-bindings` (the other bindings stories, ordered by `depends_on`); `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml` (`story:child-case-runtime`, and any story adding a governor read).
- **Safety fact:** Commission's evidence port needs no change, because `submit_evidence(port, producer, data)` takes the producer as the trusted caller's argument (`evidence.rs:116-135`). Loom can pass the source record's producer and keep it. `check_evidence` (lib.rs:1363-1411) compares only an `observed_at` member of the facts with the cited observation's `observed_at`. A copy therefore keeps its instant if Loom first observes a new observation, with source the copy effect, whose `observed_at` is the source instant. Pointed at `file:line`, unproven.
