---
format: aep.planning-md/3
id: story:silence-and-expiry-at-a-host-tick
kind: story
status: proposed
title: A host tick at a given instant expires unanswered requests or takes their default on silence
summary: 'tick(store, writers, revisions, at): no clock read, case revision from a host port, instants compared parsed, completes only what it took; decisions tick --at over --store'
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decision-inbox-commands
- depends_on: story:decided-record-written-then-recorded
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/tests/decisions_tick.rs
- confidence: inferred
  path: crates/loom-decision/src/lib.rs
- confidence: inferred
  path: crates/loom-decision/src/tick.rs
- confidence: cited
  path: crates/loom-decision/tests/tick.rs
- confidence: inferred
  path: crates/loom-sdk/src/decisions.rs
- confidence: cited
  path: ess/domains/decision.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:22Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`tick(store, writers, revisions, at)` in `b10x-loom-decision` ends every request nobody answered by
the instant `at`, which the host gives; nothing in it reads a clock. `revisions` is the host port
`CaseRevisions` that `story:decision-inbox-commands` declares and its `decide` and `take` already
take (one call per case, an integer as the governor numbers it). For each Open or Escalated request whose `expires_at` is at or before `at`,
compared as instants (both parsed as RFC 3339, not as strings):

- with a default on silence, it applies `TakeRecommendation` with principal `silence`, principal
  kind `silence`, `decided_at` = `at` and `case_revision` the request's case revision as
  `revisions` answers it (`TakeRecommendation` takes `case_revision`,
  `story:decision-request-entity`), then completes that request with the writers it was given
  (`story:decided-record-written-then-recorded`);
- without one, it applies `Expire` with `at`; the request is Expired ("the case stays stopped",
  `~/beyond10x/sdc/ess/domains/decision.yaml:845`).

An Escalated request keeps its `expires_at` and its default on silence, so it is handled as an
Open one. A request with no `expires_at`, or one after `at`, is not touched: the tick never sends
it a command the contract would refuse `NotDue` (`decision.yaml:669-674`, `:827-831`). A case
whose revision `revisions` cannot answer stops the tick with an error naming the case; requests it
already ended stay ended, and the next tick goes on from there. A tick answers a `TickReport`
(taken on silence, expired, completed, each by request id). The tick completes only the requests
it took on silence: a request that was already Decided without a record before the tick is not
its to complete and is not in its report. A second tick at the same instant changes nothing and
answers an empty report. `loom_sdk::decisions::tick` is the same call.

`b10x-loom decisions tick --at INSTANT --store DIR` runs it and prints the report as JSON; `--at`
is required and is an RFC 3339 instant. When this story lands, `decisions` has one store option,
`--store DIR` (`story:decision-inbox-commands`), and the tick runs over it with the writers the
inbox gives it: the governor's `ExplicitDecisionWriter` over the directory's `FileCaseStore`, and
the inbox's `CaseRevisions` for `--store`, that governor (the case revision of
`CanonGovernor::action_facts`, `crates/loom-governor/src/lib.rs:1166`, `:1199`). Like every `decisions` command, it first runs
`complete_pending` with those writers; what that completes is not the tick's and is not in its
report. `--state DIR` and `--tree STORE` are added to `decisions` later, each with its host's
writers and revisions (`story:approval-decisions-resume-held-runs`,
`story:question-and-outcome-decisions-hold-runs`, `story:tree-waits-open-decision-requests`), and
the tick runs over them unchanged.

A request taken on silence whose record kind has no writer in that set stays Decided without a
record (`AwaitsWriter`): the report lists it under taken on silence and not under completed, and
the next command with a writer for it completes it. The tick changes requests only; what an
Expired request means for a held run is the host's (`story:question-and-outcome-decisions-hold-runs`),
and what a tick over a tree's store writes into the run logs is the tree's
(`story:tree-waits-open-decision-requests`). A later flag, `decisions tick --state DIR --resume`,
resumes the runs of the requests a tick took on silence (`story:decisions-resume-the-held-run`);
without it the tick stays as this story says.

Its code goes in its own module of `crates/loom-decision` (`tick.rs`), and the subcommand in the
inbox's module of `crates/loom-cli` (`decisions.rs`, beside the inbox's other subcommands).

## Why

Item 3 of the request: "expiry with an optional default on silence (fired by a host tick that
passes the instant)" (`~/beyond10x/sdc/docs/decisions.md:308-309`). Session `sdc` answered on
2026-11-21 (point 8): a host tick with an explicit instant argument, a CLI command now and a
daemon later, and no clock read inside the transition; the contract now takes the instant as `at`
on `Expire` (`decision.yaml:821`) and refuses a tick before `expires_at` (`decisions.md:84-86`).
Commission sets no timer (`AGENTS.md:423`), and the contract names no one who fires `Expire` or
the silence `TakeRecommendation` at `expires_at` beyond the Driver actor (`decision.yaml:293-303`).
Loom's `TakeRecommendation` takes the case revision the record is written at (answer e), which
the SDC command has not, so the tick needs it from the host.

## Waits on

`story:decision-inbox-commands`: both change `crates/loom-cli` and `crates/loom-sdk`'s decisions
module, and it gives `--store` its writer and declares `CaseRevisions`.
`story:decided-record-written-then-recorded`: a silence
decision is completed, and both change `ess/domains/decision.yaml`.

## ESS first

`ess/domains/decision.yaml` gains `TickReport` (`CaseRevisions` is already there, from
`story:decision-inbox-commands`). On that commit
`task drift` fails, and `crates/loom-decision/tests/tick.rs` does not compile.

## Acceptance

In `crates/loom-decision/tests/tick.rs`, over the in-memory store and a fixed `CaseRevisions`
(case `C` at revision 4). Every instant is in 2099, after any host clock a test runs under, so a
result can come only from `at`:

- A request with a strong default on silence and `expires_at` `2099-01-01T10:00:00Z`, ticked at
  `2099-01-01T09:59:59Z`, is unchanged.
- The same request ticked at `2099-01-01T10:00:00Z` is Decided by `silence` with its recommended
  option, `decided_at` `2099-01-01T10:00:00Z` and `decided_case_revision` 4, and its record is
  written and recorded.
- A request without a default ticked at `2099-01-01T10:00:01Z`, past its expiry, is Expired and has
  no record.
- A request with a non-strong recommendation cannot carry a default on silence (the invariant of
  `decision.yaml:279`), so no tick decides it; ticked past its expiry it is Expired.
- A request with no `expires_at` is untouched by a tick at `2099-12-31T23:59:59Z`.
- A second tick at the same instant answers an empty report and changes no byte of the store.
- A store holding, beside a request the tick takes, a request Decided without a record before the
  tick: the report lists only the taken request, and the other stays Decided without a record.
- An Escalated request past its expiry is taken on silence or expired as an Open one is.
- An `expires_at` of `2099-01-01T11:00:00+01:00` (the same instant as `10:00:00Z`) is due at a tick
  at `2099-01-01T10:00:00Z`.
- A `CaseRevisions` that fails for case `C` stops the tick with an error naming `C`, and no
  request of `C` is changed.
- The tick module names no clock: a test reads `crates/loom-decision/src/tick.rs` and finds none
  of `SystemTime`, `Instant`, `Utc::now`, `Local::now` and `Clock`.

On the command line (`crates/loom-cli/tests/decisions_tick.rs`), over a fixture store directory:

- `decisions tick` without `--at` exits non-zero before it reads the store.
- `decisions tick --at 2099-01-01T10:00:00Z --store DIR` over an outcome request with a strong
  default `A` and `expires_at` `2099-01-01T10:00:00Z` decides it by `silence`, the directory's
  governor holds the explicit decision, and the printed report lists it under completed.
- The same command over a question request with a strong default and the same expiry decides it
  by `silence`, writes no record, and the report lists it under taken on silence and not under
  completed.
- `task check` exits 0.

## Out of scope

A timer or daemon. The tree daemon of `epic:tree-daemon-under-enforced-limits` calling this tick
is a later story of that epic. A held run's Suspension after expiry
(`story:question-and-outcome-decisions-hold-runs`). Run-log lines for a tree
(`story:tree-waits-open-decision-requests`). Resuming a run after a silence decision
(`story:approval-decisions-resume-held-runs`). A writer of an approval or question record. The
`--state` and `--tree` options of `decisions`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-decision` (the new module and `tests/tick.rs`) with `ess/domains/decision.yaml` for `TickReport` — cited (story § Outcome "Its code goes in its own module of `crates/loom-decision`", § ESS first, § Acceptance)
- **Files:** `ess/domains/decision.yaml` (gains `TickReport`) — cited (story § ESS first)
- **Files:** `crates/loom-decision/tests/tick.rs` — cited (story § ESS first)
- **Files:** `crates/loom-cli/src/decisions.rs`, the inbox's module, gains the `tick` subcommand — cited (story § Outcome "the subcommand in the inbox's module of `crates/loom-cli`"; the file name is `story:decision-inbox-commands` § Outcome, and it does not exist in the tree yet)
- **Files:** `crates/loom-decision/src/tick.rs` (the module's file name) — inferred; the story says "its own module" and gives no name
- **Files:** `crates/loom-decision/src/lib.rs` (one `mod` line) — inferred; the epic's Stories list says each story adds one `mod` line to a shared `lib.rs`
- **Files:** `crates/loom-sdk/src/decisions.rs` (`loom_sdk::decisions::tick`) — inferred; the story cites the module path `loom_sdk::decisions`, the file is created by `story:decision-inbox-commands`
- **Files:** `crates/loom-cli/src/lib.rs` (a `Tick` variant) — inferred; the other subcommand enums (`PluginCommand` `lib.rs:321`, `AgentCommand` `lib.rs:507`, `ProtocolCommand` `lib.rs:936`) live in `lib.rs`, not in module files. Needed only if the inbox puts `DecisionsCommand` there too.
- **Files:** `crates/loom-cli/tests/decisions_tick.rs` (CLI acceptance, new file) — inferred; the story names no CLI test file. It may instead extend the inbox's `crates/loom-cli/tests/decisions_inbox.rs`.
- **Symbols:** `tick`, `TickReport` (taken on silence, expired, completed, each by request id), the commands `TakeRecommendation` and `Expire`, and `complete_decision`/`complete_pending` as the completion call — cited (story § Outcome; the completion pair is `story:decided-record-written-then-recorded` § Outcome)
- **Symbols:** `ExplicitDecisionWriter`, `FileDecisionStore`, `FileCaseStore` — cited as callers only. They belong to `crates/loom-governor` and this story does not change them (`FileCaseStore` is at `crates/loom-governor/src/lib.rs:181`).
- **Also likely:** `crates/loom-cli/Cargo.toml` and `crates/loom-sdk/Cargo.toml` — inferred; the inbox adds the `b10x-loom-decision` dependency, so this story should need none.
- **Documents:** `website/docs/reference/cli.md` is regenerated (house file). No other document is named — cited (story § Acceptance names no guide page)
- **Confidence:** medium — the story names the module, the subcommand, the ESS file and one test file, but the crate and the CLI/SDK modules do not exist, so the file names are inferred
- **Would collide with (ordered by a `depends_on` path):**
  - `ess/domains/decision.yaml` — ordered before this story by `depends_on`:
    - `story:decision-request-entity` (creates it), `story:autonomy-grant-table`, `story:decider-port-and-grounding-check`, `story:decision-routing-by-holder`, `story:escalate-when-conditions` and `story:decided-record-written-then-recorded`.
    - They are ancestors through `decision-inbox-commands` → `decided-record-written-then-recorded` → `escalate-when-conditions` → `decision-routing-by-holder` → `decider-port-and-grounding-check` → `autonomy-grant-table` → `decision-request-entity`.
  - `ess/domains/decision.yaml` — ordered after this story by `depends_on`:
    - `story:approval-decisions-resume-held-runs` (direct), and through it `story:question-and-outcome-decisions-hold-runs`, `story:tree-waits-open-decision-requests`, and `story:non-independent-review-becomes-decision-request`. The last one's `crates/loom-decision` routing module and `decisions.rs` mention are also ordered through the same chain.
  - `crates/loom-decision/src/lib.rs` — the same ordered ancestors (stories 1, 3, 4, 7, 8, 9) and descendants.
  - `crates/loom-cli/src/decisions.rs`, `crates/loom-cli/src/lib.rs` (the `Command` variant) and `crates/loom-sdk/src/decisions.rs` — ordered after `story:decision-inbox-commands` (direct `depends_on`). Descendants `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:tree-waits-open-decision-requests` and `story:requests-from-canon-pending-decisions` add `--state`/`--tree` to the same module and are ordered after.
  - `crates/loom-cli/src/lib.rs` — **unordered** against nine draft/proposed stories outside the epic that cite this file, if the `DecisionsCommand` enum lands there:
    - `story:bound-commands-run-confined` (also `main.rs`)
    - `story:agent-node-run-records` (also `main.rs`)
    - `story:command-invocation-contract`
    - `story:engine-configuration-document`
    - `story:tree-run-command` (also `main.rs`)
    - `story:tree-daemon` (also `main.rs`)
    - `story:standing-tree-wakes-at-a-host-instant`
    - `story:standing-tree-rests-quiescent`
    - `story:secrets-subcommand` (also `main.rs`)
    - `story:tree-usage-and-model-budget`
    - `story:run-caps-per-arrival`
    - No `depends_on` path orders any of them with this story. Each one is a one-variant or one-flag edit. `story:decision-inbox-commands` carries the same exposure.
  - `crates/loom-sdk/src/lib.rs` — only if a `pub mod decisions` or re-export line is edited here (inferred, the inbox adds it). Unordered against `story:agent-selects-the-connectors-relay`, `story:child-launches-through-confinement-adapters`, `story:connectors-relay-client` and `story:engineering-registry-calls-replaced`, which also cite it.
  - `story:decision-suspension-reason` has no `depends_on` edge to this story and touches `crates/loom-commission` and `crates/loom-cli/tests/resume_after_grant.rs`; it shares no file with this story.
- **Safety fact:** nothing outside `crates/loom-decision` reads `TickReport` or the new `tick`, so adding both changes no existing caller — step 2 (`git grep -n 'loom-decision\|FileDecisionStore\|ExplicitDecisionWriter' -- crates` finds nothing in the tree), unproven
- **Safety fact:** the contract already refuses what a careless tick would send. `Expire` refuses `NotDue` (`~/beyond10x/sdc/ess/domains/decision.yaml:782-786`) and `SilenceHasDefault` (`:787-791`). `TakeRecommendation` refuses `NotDue` (`:625-630`) and `NoDefaultOnSilence` (`:619-624`). The tick's filter is an optimisation, not the guard — step 2, unproven
