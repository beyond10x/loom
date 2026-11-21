---
format: aep.planning-md/3
id: story:decision-inbox-commands
kind: story
status: proposed
title: b10x-loom decisions lists a principal's open requests and decides, takes, escalates and supersedes them
summary: The inbox as CLI and SDK calls over the file store, completing what it decides
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decided-record-written-then-recorded
- depends_on: story:decision-suspension-reason
scope:
- confidence: inferred
  path: crates/loom-cli/Cargo.toml
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/decisions_inbox.rs
- confidence: cited
  path: crates/loom-decision
- confidence: inferred
  path: crates/loom-sdk/Cargo.toml
- confidence: cited
  path: crates/loom-sdk/src/decisions.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: inferred
  path: crates/loom-sdk/tests/decisions.rs
- confidence: inferred
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: ess/domains/decision.yaml
- confidence: inferred
  path: website/docs/guides/decisions.md
- confidence: inferred
  path: website/docs/reference/sdk-stability.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:22Z", actor: "agent:sdc", revision: 9, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom decisions` is the inbox, over a store directory (`--store DIR`) that holds a
`FileDecisionStore` and the governor's `FileCaseStore`:

- `decisions list --principal P` prints `OpenDecisionsForPrincipal` for `P` as JSON, one entry per
  request: id, case, question, subject at revision, recommendation and strength or the reason for
  none, opened and expiry instants. It lists Open and Escalated requests only, as the view does; a
  Decided request still waiting for its record is not listed.
- `decisions show ID` prints the request as JSON, with the `EscalationSuppressedRecord`s kept
  beside it; `decisions history --artifact A` prints `ArtifactDecisionHistory`.
- `decisions decide ID OPTION --as P [--reason TEXT]...` and `decisions take ID --as P` apply
  `Decide` and `TakeRecommendation` with principal `P` of kind `person`, passing as
  `case_revision` the current revision of the request's case, `r<N>`, as the host port
  `CaseRevisions` answers it.
- `CaseRevisions` is declared here, in `b10x-loom-decision` and in `ess/domains/decision.yaml`: a
  host port that answers a case's current revision, one call per case, an integer as the governor
  numbers it. `decide` and `take` take it, and so does `story:silence-and-expiry-at-a-host-tick`'s
  `tick`. For `--store` it is the directory's governor, the case revision of
  `CanonGovernor::action_facts` (`crates/loom-governor/src/lib.rs:1166`, `case_revision` at
  `:1199`); `--state` and `--tree` give their own (`story:approval-decisions-resume-held-runs`,
  `story:tree-waits-open-decision-requests`). A `CaseRevisions` that cannot answer for the case
  stops the command with exit `20`, naming the case, and nothing changes.
- `decisions escalate ID --to P --reason TEXT` and `decisions supersede ID --by ID2 --reason TEXT`
  apply `Escalate` and `Supersede`. Both take no `--as`, and nobody is recorded as escalating or
  superseding: the contract's `Escalate` takes `request_id`, `escalate_to` and `reason`, and
  `Supersede` takes `request_id`, `superseded_by` and `reason`, with no principal
  (`~/beyond10x/sdc/ess/domains/decision.yaml:748-751`, `:787-790`).
- Every command first runs `complete_pending` with the writers the command line has for the
  store (in this story the governor's `ExplicitDecisionWriter`), and `decide` and `take` complete
  the request they decided, so a decision made here is recorded before the command exits. A
  Decided request whose record kind has no writer on this command line (an approval's
  `canon_authority`, a question's `artifact_revision`) is answered `AwaitsWriter`, left as it is,
  and the command goes on.
- Every instant comes from `--at` when given, otherwise from the machine's clock, as `approve`'s
  grant instant does (`crates/loom-intake-slice/src/grant.rs:234-237`). The principal named by
  `--as` is host-trusted input, as the principal of `b10x-loom approve` is; authenticating it is
  not Loom's.
- Exit status: `0` done; `10` the domain refused the command (the error, such as `NotEligible`,
  `DecisionStateConflict` or `NoRequest`, and the request are named on standard error, and nothing
  changed); `11` `StoreConflict`, another process wrote the request first, nothing changed; `12`
  the decision was taken but its record could not be completed (`CompletionRefusal`, named), so
  the request is Decided without a record; `20` a store could not be read or written; `2` the
  command line is not valid; `1` anything else.

When this story lands, `decisions` has `--store` and `--at` and no other store flag.
`decisions tick` comes with `story:silence-and-expiry-at-a-host-tick`, `--state DIR` with
`story:approval-decisions-resume-held-runs`, `decisions record` with
`story:question-and-outcome-decisions-hold-runs`, and `--tree STORE` with
`story:tree-waits-open-decision-requests`, each with its host's writers.

The subcommand's code goes in its own module of `crates/loom-cli`, `src/decisions.rs`, which holds
its clap `Args` and a `run` function. `lib.rs` gains the `pub mod decisions;` line
(`crates/loom-cli/src/lib.rs:184-192`) and a `Decisions` variant in `Command` (`:220-243`), and the
exhaustive match of `main` (`crates/loom-cli/src/main.rs:61-85`) gains the arm that calls
`decisions::run`. `crates/loom-cli/Cargo.toml` gains `b10x-loom-decision`.

`b10x-loom-sdk` exposes the same calls (`loom_sdk::decisions`: open requests for a principal,
show, history, decide, take, escalate, supersede), each over a `DecisionStore` and a set of
writers the host gives, `decide` and `take` also over the host's `CaseRevisions`, in `crates/loom-sdk/src/decisions.rs` with a `pub mod decisions;` line in
`crates/loom-sdk/src/lib.rs` and `b10x-loom-decision` in `crates/loom-sdk/Cargo.toml`. The calls are
listed in the control-plane table of `website/docs/reference/sdk-stability.md`, and
`crates/loom-sdk/tests/host_ports.rs` gains their `use` lines, which that test requires of every
path on the page (`host_ports.rs:1-10`).

A guide page, `website/docs/guides/decisions.md`, shows the commands and the exit statuses with no
pasted output, so nothing on it can go stale unchecked; the `guides` sidebar is generated.

## Why

Item 3 of the request: the open-decisions query per principal and escalation to a named person
(`~/beyond10x/sdc/docs/decisions.md:307-309`). The inbox is a store plus CLI and SDK calls, with
no service of its own (coordinator's decision, accepted by session `sdc` on 2026-11-21, and now
`decisions.md:264-265`). Under `person` or `advised` an
Escalate is a person re-addressing the request (session `sdc`, 2026-11-21, point 4), which is this
command. The command line today has `approve` and `resume` for a held capability only
(`crates/loom-cli/src/lib.rs:235-242`).

## Waits on

`story:decided-record-written-then-recorded`: `decide` and `take` complete what they decide.
`story:decision-request-file-store` through it (by way of `story:grounds-resolve-against-held-cases`):
the two-process bullet needs `FileDecisionStore`'s lock. `story:decision-suspension-reason`: both
edit `crates/loom-cli/src/lib.rs` (that story adds a `ResumeRefusal` exit status there), and no
other path orders them. The stories outside this epic that edit `crates/loom-cli/src/lib.rs`,
`main.rs`, the two manifests or the SDK files are in the typed scope, and
`aep plan artifact waves` keeps them apart.

## ESS first

`ess/domains/decision.yaml` gains the port `CaseRevisions`; the commands otherwise apply
`loom.decision` as declared. On that commit `task drift` fails. The red test is
`crates/loom-cli/tests/decisions_inbox.rs`, which fails before the subcommand exists. The
generated CLI reference (`website/docs/reference/cli.md`) is regenerated in the implementing
commits.

## Acceptance

In `crates/loom-cli/tests/decisions_inbox.rs`, which runs the binary over a fixture store holding a
governor case and requests opened by `route_stop`:

- `decisions list --principal timo` prints the two requests addressed to `timo` and not the one
  addressed to `ana` or the Decided one.
- `decisions decide <outcome request> A --as timo` prints the produced record; `decisions show`
  then shows `produced_record` `canon_decisions` and the governor holds the explicit decision with
  principal `timo`.
- `decisions decide` by a principal not in `eligible` exits 10 naming `NotEligible`, and the
  request file is byte-identical.
- `decisions take` on a request with a `moderate` recommendation exits 10 naming `NotEligible`;
  `decisions decide` with the same option succeeds (session `sdc`, 2026-11-21, point 5).
- `decisions escalate ID --to ana` moves the request to `ana`'s list; a second escalate from
  Escalated to `ben` moves it again.
- Under holder `decider`, `decisions escalate ID --to ana --reason "needs ana"` leaves the request
  with holder `person`; `decisions show ID` prints it so.
- `decisions supersede DR-1 --by DR-2 --reason "spec moved"` leaves `DR-1` Superseded with
  `superseded_by: DR-2`; `decisions list` no longer shows it, and a second supersede exits 10
  naming `DecisionStateConflict`.
- `decisions history --artifact roadmap` prints the requests whose subject is `roadmap`, at every
  revision and in every state, and none whose subject is `vision`.
- `decisions show ID` on a request with two `EscalationSuppressedRecord`s prints both, in the
  order they were appended, with class, subject at revision, reasons, `best` and cause.
- `decisions decide ID A --as timo --at 2099-01-01T00:00:00Z` records `decided_at`
  `2099-01-01T00:00:00Z`, an instant only `--at` can give.
- `decisions decide ID A --as timo` without `--at` records a `decided_at` at or after the instant
  the test read before starting the command and at or before the one it read after it exited.
- A store left with a Decided outcome request without a record is completed by the next
  `decisions list`: `decisions show` then prints its `produced_record`.
- A store holding a Decided approval request without a record: `decisions list` exits 0 and leaves
  that request file byte-identical (`AwaitsWriter`).
- Two `decisions decide` processes on one request leave one Decided request; the second exits 11
  naming `StoreConflict` or 10 naming `DecisionStateConflict`.
- The SDK calls give the same store bytes as the commands for the same sequence
  (`crates/loom-sdk/tests/decisions.rs`).
- The SDK's `decide` given a fixed `CaseRevisions` answering 7 for the request's case leaves the
  request with `decided_case_revision` `r7` (`crates/loom-sdk/tests/decisions.rs`).
- `crates/loom-sdk/tests/host_ports.rs` passes with the `loom_sdk::decisions` paths on
  `website/docs/reference/sdk-stability.md`.
- `task docs-check` exits 0 with `website/docs/reference/cli.md` regenerated to show `decisions`.
- `task check` exits 0.

## Out of scope

Expiry (`story:silence-and-expiry-at-a-host-tick`). Approval records and resuming a run
(`story:approval-decisions-resume-held-runs`). The question writer and `decisions record`
(`story:question-and-outcome-decisions-hold-runs`). A web inbox, notifications, a service.
Resolving or authenticating principals. Recording who escalated or superseded a request.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-cli` (the `decisions` subcommand: `src/decisions.rs`, new) — cited, the Outcome names the module and the `lib.rs` edit
- **Files:** `crates/loom-cli/src/decisions.rs` (new) — cited
- **Files:** `crates/loom-cli/src/lib.rs` — cited. A `pub mod decisions;` line among `lib.rs:184-192` and a `Decisions` variant in `Command` (`lib.rs:220-243`), after `Resume`.
- **Files:** `crates/loom-cli/tests/decisions_inbox.rs` (new, the red test) — cited
- **Files:** `crates/loom-cli/src/main.rs` — inferred, not named by the story. `main` matches `Command` exhaustively (`main.rs:61-85`) and every command's execution is a function there (`approve_command` `main.rs:520`), so the variant needs an arm.
- **Files:** `crates/loom-sdk/src/decisions.rs` (new) and `crates/loom-sdk/src/lib.rs` (a `pub mod decisions;` beside `lib.rs:60-62`) — cited. The story names `loom_sdk::decisions`; the paths are inferred from the existing module layout.
- **Files:** `crates/loom-sdk/tests/decisions.rs` (new, for "same store bytes as the commands") — inferred
- **Files:** `website/docs/guides/decisions.md` (new guide page with pasted `list`, `show`, `decide`) — inferred, the story names "a guide page" and no path. `website/sidebars.ts:15` autogenerates `guides`, so no sidebar edit.
- **Symbols (cited):** `OpenDecisionsForPrincipal`, `ArtifactDecisionHistory`, `EscalationSuppressedRecord`, `DecisionStore`, `FileDecisionStore`, `Decide`, `TakeRecommendation`, `Escalate`, `Supersede`, `complete_pending`, `ExplicitDecisionWriter`, `route_stop`, `FileCaseStore`. None exists in the tree yet; they come from `story:decision-request-entity`, `story:decision-request-file-store`, `story:decided-record-written-then-recorded` and `story:decision-routing-by-holder`.
- **Symbols (existing, cited):** `CanonGovernor::decide` (`crates/loom-governor/src/lib.rs:1055`) behind `ExplicitDecisionWriter`; `FileCaseStore` (`crates/loom-governor/src/file_store.rs:92`); `HostClock` (`crates/loom-intake-slice/src/clock.rs:41`); `grant::now` (`crates/loom-intake-slice/src/grant.rs:235`).
- **Also likely:** `crates/loom-cli/Cargo.toml` and `crates/loom-sdk/Cargo.toml` (a dependency on `b10x-loom-decision`; `loom-sdk` already depends on `b10x-loom-governor`) — inferred
- **Also likely:** `website/docs/reference/sdk-stability.md` and `crates/loom-sdk/tests/host_ports.rs` — inferred. The page lists each `loom_sdk::models`, `agent`, `connectors` and `session` path, and `host_ports.rs` asserts the page and its `use` lines agree (`host_ports.rs:1-9`). A `loom_sdk::decisions` row needs both; the story does not say whether the module is on the page.
- **Documents:** `website/docs/guides/decisions.md`; `website/docs/reference/cli.md` is regenerated (a house-rule file, left out of the paths). No `ess/` change: "No new noun" is cited in the story.
- **Confidence:** medium. The code surface is high (the story names the crate, module and edited file), but every domain symbol it uses is in an undelivered dependency, so the SDK file names, the `main.rs` edit and the guide path are inferred.
- **Would collide with:**
  - `crates/loom-cli/src/lib.rs`, `decisions.rs` and `crates/loom-sdk` decisions module, all ordered after this story by `depends_on`: `story:silence-and-expiry-at-a-host-tick` (direct), `story:approval-decisions-resume-held-runs` (through silence), `story:question-and-outcome-decisions-hold-runs` (through approval-decisions), `story:tree-waits-open-decision-requests` (through question-and-outcome), `story:flow-limits-recorded-as-events` (inbox is its ancestor).
  - `crates/loom-cli/src/lib.rs` and `main.rs`, **no `depends_on` path with this story**: `story:tree-run-command` (proposed; its store scope lists `crates/loom-cli/Cargo.toml`, and its body edits `lib.rs` and `main.rs`), `story:confinement-opens-from-engine-config` (proposed; `lib.rs` cited, `main.rs:670,1017,1152`), `story:bound-commands-run-confined` (proposed; `lib.rs` and `main.rs:983-1003`), `story:agent-node-run-records` (proposed; `lib.rs` and `main.rs` inferred), `story:secrets-subcommand` (draft; `loom-cli/Cargo.toml`, `lib.rs` and `main.rs`), `story:engine-configuration-document` (edits `lib.rs`), `story:command-invocation-contract` (reads `lib.rs`), `story:tree-daemon`, `story:run-caps-per-arrival`, `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant` and `story:tree-usage-and-model-budget` (each names `lib.rs`; the last four sit behind `tree-run-command`, not behind this story). Each is one `mod` or `Command` line apart from us in `lib.rs`, so the conflict is textual and small; `main.rs` is the same.
  - `crates/loom-cli/Cargo.toml` and `crates/loom-sdk/Cargo.toml`: `story:tree-run-command` and `story:secrets-subcommand` (loom-cli, unordered); `story:engineering-protocol-lock` (loom-sdk, unordered).
  - `crates/loom-sdk/src/lib.rs`, `sdk-stability.md`, `host_ports.rs`, no `depends_on` path: `story:connectors-relay-client`, `story:agent-selects-the-connectors-relay`, `story:child-launches-through-confinement-adapters`, `story:engineering-registry-calls-replaced`, `story:consumer-sdk-paths`, `story:generated-types-release-surface`, `story:co-pinned-consumer-check` (draft or proposed, each names one of them).
  - `story:decision-suspension-reason` shares `crates/loom-cli/tests` files only (`resume_after_grant.rs`, `run_state.rs`, `adversary_w10_u3.rs`); this story adds a new test file and edits none of those, so there is no file collision.
- **Safety fact:** this story only adds a subcommand, so `b10x-loom approve`, `resume` and `run` are untouched. A new `Command` variant changes no existing arm; `lib.rs:26-35` and the `approve` and `resume` exit-status tables stay as they are — step 2 (`lib.rs:220-243`, `main.rs:61-85`), unproven
- **Contradicted by the code:** nothing at `file:line` yet. `lib.rs:235-242` is `Approve` and `Resume` as the Why says. The domain types are absent from the tree (dependencies undelivered), so none can be contradicted.
