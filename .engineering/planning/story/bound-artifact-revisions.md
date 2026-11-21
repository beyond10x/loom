---
format: aep.planning-md/3
id: story:bound-artifact-revisions
kind: story
status: active
title: After a bound action each placed artifact takes the content digest of its path as its revision
summary: Loom computes revisions; an observing action that changed an artifact records no evidence
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-program-result-table
- depends_on: story:hash-chained-run-log
scope:
- confidence: inferred
  path: crates/loom-bindings
- confidence: cited
  path: crates/loom-bindings/tests/bound_revisions.rs
- confidence: inferred
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-governor/tests/step_write.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-11-21T21:13:21Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w8"}
---
## Outcome

After a bound action ends, every artifact the bindings place at a path (`artifacts` in
`loom.bindings/1`) takes as its revision the content digest of what is at that path: `t` and the
first 16 hex digits of the SHA-256 over the file, or over the relative paths and bytes of a tree
in path order; `none` while the path is absent. Loom computes it in `b10x-loom-bindings`; no
executor names a revision. The step's revision moves and the step's evidence are written to the
case in one store write, and the evidence names the moved revisions, so a crash leaves the case
either as it was before the step or complete after it. An action whose effect class is not
`write` that changed an artifact records no evidence for that step. The same computation runs
before every evaluation, so a person's edit in the workspace or another case's write reaches the
case as a new revision.

## Why

`~/beyond10x/sdc/docs/boundary.md` row 4: the revision counter and per-artifact revisions are
Loom's (`CanonGovernor::update_revision`, `crates/loom-governor/src/lib.rs:1017-1046`: the case
revision moves only when an artifact's revision changes), and which digest counts is the
application's. Today a host calls `update_revision` itself with a value it chose. The driver's
rule (`~/beyond10x/sdc/driver/src/main.rs:148-240`, `749-751`) is what makes an LLM's output "only
an artifact": the next evaluation sets aside every record about the earlier revision, and only a
program that observes the new one produces evidence about it. The driver writes evidence before
the revision (`driver.md` gap 13) because its case and evidence are separate files; Loom's case
file holds both (`crates/loom-governor/src/file_store.rs:136-146`), so one write carries both.

## ESS first

`ess/domains/bindings.yaml` first: `ArtifactPlacement`, `RevisionMove` (artifact, from, to) and
the step outcome `Discarded` with its reason. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_revisions.rs` does not compile.

## Acceptance

- A placed file whose content is the three bytes `abc` has revision `tba7816bf8f01cfea`; an
  absent path has `none`.
- A `write` action whose program changes a placed file moves that artifact's revision to the
  digest and the case revision by one; a program that changes nothing moves neither.
- The digest of a tree does not depend on directory listing order, and two trees that differ in
  one byte or one file name differ in revision.
- A record submitted by a writing step names the moved revision, and Canon counts it as about the
  current revision at the next evaluation.
- An action with effect class `read` (or none) whose program changed a placed artifact records no
  evidence, the step is recorded as `Discarded`, and the artifact still moves to the digest.
- A file changed in the workspace between two steps moves the artifact's revision at the next
  evaluation, before the frontier is read.
- A crash injected at the store write of a step leaves a case file that opens and holds either
  neither the moved revision nor the step's evidence, or both.
- A symbolic link under a placed path is digested as its target string and never followed outside
  the workspace.
- `task check` exits 0.

## Out of scope

A digest other than the content digest (an ESS `spec_digest`, a git commit). Files outside the
placed paths. One workspace per child case.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (new crate from `story:bindings-document`; this story adds the digest, the revision moves and the discard rule) — cited
- **Files:** `crates/loom-bindings/tests/bound_revisions.rs` — cited
- **Files:** `ess/domains/bindings.yaml` (`ArtifactPlacement`, `RevisionMove`, `Discarded`; file exists only after `story:bindings-document`) — cited
- **Files:** `crates/loom-governor/src/lib.rs` (`update_revision` 1017-1046, `EvidencePort::receive` 1415-1432, `applies` 1779-1791, the `FallibleCaseStore` trait 382-393) — cited
- **Symbols:** `CanonGovernor::update_revision`, `EvidencePort::receive`, `applies`, `FallibleCaseStore::update`, `HeldEvidence` — cited
- **Symbols:** a new public `CanonGovernor` method that applies revision moves and appends evidence inside one `FallibleCaseStore::update` closure — inferred, no such method exists
- **Also likely:** `crates/loom-governor/src/file_store.rs` — inferred, read only. Its `update` (280-293) already holds the lock, applies the closure and does one `write_case` that renames a synced temporary file. That is already atomic, so the file should need no change. It is listed because the crash-injection acceptance has to write through it.
- **Also likely:** `crates/loom-governor/tests/` gets one new test file for the combined write, for example `step_write.rs` — inferred, name guessed
- **Documents:** none required. The `lib.rs` module doc lines 10 and 100-110 describe `update_revision` and evidence intake and should gain a sentence — inferred, and it sits in a file already listed. `website/docs/concepts/governor-and-intake.md` does not name `update_revision`, so no hand-written page needs a change.
- **Confidence:** medium — the bindings half is cited. The governor half is cited for where the current behaviour lives and inferred for the shape of the change.
- **Would collide with:** `crates/loom-governor/src/lib.rs` (the `CanonGovernor` impl and the evidence-intake region), `crates/loom-governor/src/locked_dir.rs`, and `crates/loom-bindings` plus `ess/domains/bindings.yaml`. That means any unit that edits the governor's write path or opens a case through it, and any unit extending the bindings crate or domain.
- **Safety fact:** `update_revision` and `receive` are infallible-closure writes, so a combined method that validates before it mutates inside one `update` closure leaves the case file either as it was or complete after the step — `file_store.rs:280-293` and `file_store.rs:180-195`, unproven
- **Safety fact:** `applies` needs no change. It only asks whether Canon evaluates without refusal (`lib.rs:1779-1791`). A record bound to a superseded revision is kept, applies and is excluded by Canon's own binding (`tests/adversary_governor.rs:75-112`), so the order of moves and evidence does not change `applies` — unproven for a revision ahead of the case's, which no test covers.
- **Safety fact:** `ess/domains/governor.yaml` is not touched. `StoredCase` and `StoredEvidenceData` hold the same fields before and after (`subject_revision` is an Integer at `governor.yaml`/`file_store.rs:607`), and the new method adds no stored member — inferred from the story naming no new stored field, unproven
