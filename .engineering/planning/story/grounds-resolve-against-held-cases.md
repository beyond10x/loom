---
format: aep.planning-md/3
id: story:grounds-resolve-against-held-cases
kind: story
status: proposed
title: Grounds resolve against the governor-held case, its records and the decision store
summary: Revision still current, evidence still present, record ids in session sdc's forms
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
- depends_on: story:decision-request-file-store
scope:
- confidence: inferred
  path: crates/loom-governor/src/held_case_grounds.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/held_case_grounds.rs
- confidence: cited
  path: ess/domains/governor.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:19Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`HeldCaseGrounds` in `b10x-loom-governor` implements the `CitationResolver` port of
`b10x-loom-decision` for the citations a pending recommendation cites in a governor-held case. It
applies Canon's two rules for a ground to a recommendation that is not yet a Canon record: the
cited revision is still the current one, and the cited evidence record is still present. It is a
module of the governor crate, `held_case_grounds`, and reads a held case through the governor's
private case read (`CanonGovernor::held`, `crates/loom-governor/src/lib.rs:1294`); it adds no
public reader.

A case revision in a citation is written `r<N>`, N the governor's integer case revision
(`CaseState.revision`, `lib.rs:193`), as `CanonGovernor::decide` stamps it on an explicit
decision (`format!("r{}", state.revision)`, `lib.rs:1074`).

- `artifact_revision`: resolves when the revision equals the current revision of the artifact in
  the held case, or in the held case the citation's `case` names; the artifact `case` means the
  case revision, `r<N>` (`~/beyond10x/sdc/tools/decision-grounds.sh:47-52`).
- `record_id` per store:
  - `canon_evidence`: the evidence id is held by the case;
  - `canon_decisions`: `<outcome>/<decision>@r<N>` matches an explicit decision the governor
    holds for the case, recorded with `CanonGovernor::decide` at that case revision
    (`AGENTS.md:461-463`);
  - `canon_authority`: `<capability>@r<N>` matches a record the host's `AuthorityRecords` port
    reports for the case (capability, decision, principal, case revision); Loom keeps authority
    records outside Canon today (the slice's grant store,
    `crates/loom-intake-slice/src/grant.rs:116`). This story declares the port; the port is
    optional: `HeldCaseGrounds::new` takes none and `with_authority_records` adds one. Without
    one, a `canon_authority` citation is refused "no authority records for case <id>", so a ground
    nobody can resolve never counts. The slice offers its records from
    `story:approvals-supplied-to-canon-evaluation` on, the tree from
    `story:tree-waits-open-decision-requests` on;
  - `sdc_decisions`: not asked of this resolver. `check_grounds` resolves it against the decision
    store (`story:decider-port-and-grounding-check`); asked anyway, `HeldCaseGrounds` refuses it
    naming the store;
  - `aep_artifact`: `kind:name` names the file `<kind>/<name>.md` under the AEP root the host
    names (`decision-grounds.sh:44`). An absent file is refused naming the id. A `kind` or `name`
    holding `/`, `\` or `..` is refused naming the form: this check is Loom's own, the script tests
    only that the file exists.
- `path_line` is delegated to `PathLineResolver::new(case_dir, repository)` with the two
  directories of `GroundRoots`. A held case has no directory of its own (`CaseState`,
  `lib.rs:187-206`, has no path, and `FileCaseStore` keeps one file per case,
  `crates/loom-governor/src/file_store.rs:136`), so the host names the directory that stands for
  the SDC script's `case_dir` (`decision-grounds.sh:20-30`): the slice its workspace, the tree its
  store directory (`story:approval-decisions-resume-held-runs`,
  `story:tree-waits-open-decision-requests`).

A bare `capability` or `decision@outcome` id, the forms `decision-grounds.sh:38` and `:40-42` read
today, is refused naming the form: the record ids are those of session `sdc`, 2026-11-21, point 7.

## Why

The grounding check of `story:decider-port-and-grounding-check` needs these citations resolved,
and only `b10x-loom-governor` evaluates protocols with Canon and holds cases (`AGENTS.md:440-441`).
Canon judges grounds only on a decided entry (`story:decision-grounds`, session `canon`,
2026-11-21); before the entry exists Loom checks the same two rules itself (session `sdc`,
2026-11-21). Canon 0.8.0 already carries artifact revisions and the case revision an explicit
decision binds to (`~/beyond10x/canon/website/docs/reference/documents.md:23-35`, `:141-148`), so
nothing here waits on a Canon release.

## Waits on

`story:decider-port-and-grounding-check`: the port and the refusal types.
`story:decision-request-file-store`: the store the prior-decision acceptance row reads, and both
change `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`.

## ESS first

`ess/domains/governor.yaml` gains `GroundRoots` (case directory, repository root, AEP root) and
`HeldAuthorityRecord` (case, capability, decision, principal, case revision). On that commit
`task drift` fails, and `crates/loom-governor/tests/held_case_grounds.rs` does not compile.

## Acceptance

With a fixture protocol held by `CanonGovernor` over a `FileCaseStore`, and fixture directories
for the case directory, the repository root and the AEP root:

- `roadmap@<current>` resolves; after `update_revision` moves `roadmap`, the same citation is
  refused "`roadmap@<old>` is not current (`<new>`)".
- `case@r1` resolves on a case just opened, and `case@r2` is refused "`case@r2` is not current
  (`r1`)"; a citation whose `case` names a second case held by the same governor resolves against
  that case, looked up by id (no parent-child link of `story:child-case-runtime` is read).
- A held evidence id resolves; an id the case does not hold is refused naming the store and id.
- `close/taken@r1` resolves after `CanonGovernor::decide(case, "close", "taken", p)` on a case at
  revision 1, and `close/taken@r2` is refused.
- `owner.approve@r1` resolves against a fixture `AuthorityRecords` holding that record, and the
  bare `owner.approve` is refused naming the form.
- An `sdc_decisions` citation asked of `HeldCaseGrounds` is refused naming the store.
- An `aep_artifact` id `story:x` with `story/x.md` under the fixture AEP root resolves; `story:y`
  with no file is refused naming the id.
- An `aep_artifact` id `../story:x` and one `story:a/b` are each refused naming the form.
- A `path_line` citation of a file in the case directory resolves, and one of a file only in the
  repository root resolves against the root.
- With no `AuthorityRecords` port, `owner.approve@r1` is refused "no authority records for
  case <id>".
- The check of `story:decider-port-and-grounding-check` over this resolver and a
  `FileDecisionStore` gives the four rows of the hand-made requests of
  `~/beyond10x/sdc/docs/decisions.md:148-153`, rebuilt as a fixture case: `roadmap-valid`
  verified with "5 grounds, all resolve"; `roadmap-dangling` refused with its three reasons (lines
  outside the file, no evidence record, `roadmap@<old>` not current); a reply built from a
  Decided prior in the store whose grounds resolve verified with "; reuses <prior id>", and one
  built from a prior whose `vision` ground is not current refused naming `vision@<old>`; a ground
  with a wrong digest refused "changed: lines 20-27 of work/aep/vision/product.md differ from the
  cited digest".
- The resolver evaluates no protocol and reads no clock: the governor is built with
  `with_evaluation_time` (`lib.rs:878`) over a counting `EvaluationTime`, whose `at` is called once
  per evaluation (`lib.rs:422-424`), and the count read just before each resolution above equals
  the count read just after it (the fixture's setup, which receives evidence through
  `EvidencePort::receive`, calls `at`, `lib.rs:1512-1515`, so the count is not 0).
- `task check` exits 0.

## Out of scope

Writing grounds onto a Canon record, and reading `stale_grounds`
(`story:decided-records-carry-grounds-to-canon`). A record store of Loom's own for authority
records. Resolving `sdc_decisions` ids (`story:decider-port-and-grounding-check`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (Canon-linked crate; the story names `HeldCaseGrounds` in `b10x-loom-governor`) — cited
- **Files:** `crates/loom-governor/tests/held_case_grounds.rs` (new; named in "ESS first") — cited
- **Files:** `ess/domains/governor.yaml` (gains `GroundRoots`, `HeldAuthorityRecord`; file has 153 lines, `loom.governor` domain) — cited
- **Files:** `crates/loom-governor/src/lib.rs` (a `mod` line and re-exports; "Waits on" says this story changes it) — cited
- **Files:** `crates/loom-governor/src/held_case_grounds.rs` (new module for `HeldCaseGrounds` and `AuthorityRecords`; the epic's Stories paragraph says each story's code goes in its own module) — inferred
- **Symbols:** `HeldCaseGrounds`, `HeldCaseGrounds::new`, `with_authority_records`, `AuthorityRecords`, `GroundRoots`, `HeldAuthorityRecord` — cited (all new)
- **Symbols used, existing:** `CanonGovernor::decide` (`crates/loom-governor/src/lib.rs:1055`), `update_revision` (`:1020`), `CaseState` (`:187-206`: `revision` i64, `artifacts`, `evidence: Vec<HeldEvidence>`, `decisions: Vec<ExplicitDecision>`) — cited
- **Symbols used, other crates:** `CitationResolver`, `PathLineResolver`, `DecisionStore`, `check_grounds` of `b10x-loom-decision` (not in tree yet: `crates/loom-decision` does not exist; they arrive with `story:decider-port-and-grounding-check` and `story:decision-request-entity`) — cited
- **Also likely:** reads `CaseState` through the private `CanonGovernor::held` (`lib.rs:1294`), which a child module may call; no `pub` accessor exists for the case revision or for `decisions` (`revisions()` at `:1097` returns artifacts only) — inferred
- **Also likely:** `crates/loom-governor/Cargo.toml` needs a `b10x-loom-decision` path dependency; `story:decision-request-file-store` (a dependency) needs the same and adds it first, so this story probably edits no manifest — inferred
- **Documents:** none beyond house-rule files; the story's acceptance is code and ESS — cited
- **Confidence:** high — the story names crate, type, test file and ESS file; the only guess is the module file name
- **Would collide with (same file, ordered by `depends_on`):**
  - `story:decision-request-file-store`: `crates/loom-governor/src/lib.rs` (`mod` line) and `ess/domains/governor.yaml`; this story depends on it — cited
  - `story:decided-record-written-then-recorded`: `lib.rs`; it depends on this story directly — cited
  - `story:approvals-supplied-to-canon-evaluation`: `lib.rs`, `ess/domains/governor.yaml` (extends `HeldAuthorityRecord`); it depends on this story through `story:approval-decisions-resume-held-runs` and directly, per the graph — cited
  - `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:tree-waits-open-decision-requests`, `story:decided-records-carry-grounds-to-canon`, `story:authority-records-bound-and-grounded-in-canon`, `story:requests-from-canon-pending-decisions`: use `HeldCaseGrounds`, and each depends on this story (transitively) — cited
- **Would collide with (same file, NOT ordered by any `depends_on` path to or from this story):**
  - `story:canon-0-9-0-pin` (active, wave w2): `crates/loom-governor/src/lib.rs`, `Cargo.toml` — cited; it edits a `match` at `lib.rs:1963-1990`, away from a `mod` line
  - `story:child-case-runtime`: `lib.rs`, `ess/domains/governor.yaml` (adds the case link to `CaseState`/`StoredCase`) — cited; also changes the case this resolver reads
  - `story:child-case-own-workspace`: `ess/domains/governor.yaml` — cited
  - `story:record-copy-between-cases`: `lib.rs`, `ess/domains/governor.yaml` — cited
  - `story:bound-artifact-revisions`: `lib.rs` (`update_revision`), `ess/domains/governor.yaml` — cited; changes how an artifact revision is computed, which this resolver compares with
  - `story:hash-chained-run-log`: `lib.rs` (`mod`/`pub use` near `:175-179`) — cited
  - `story:standing-tree-rests-quiescent` (`next_expiry`), `story:engineering-registry-calls-replaced`, `story:frontier-names-movers`: `lib.rs` — cited
  - `story:engineering-protocols-dependency-removed`: `crates/loom-governor/Cargo.toml` — cited
  - Only a `mod` line and re-exports hit `lib.rs`, so these are trivial merges; the two `governor.yaml` appends are the real overlap — inferred
- **Contradicted by the code:** see "Story defects" 1 to 4 — cited
- **Safety fact:** the resolver only reads `CaseState`, held evidence and decisions through the existing store and runs no evaluation. `EvaluationTime::at` (`lib.rs:424`) is "refreshed for every evaluation", so a counting closure passed to `with_evaluation_time` can test "evaluates no protocol" — step 2 (`lib.rs:422-426`), unproven
