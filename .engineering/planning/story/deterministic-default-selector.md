---
format: aep.planning-md/3
id: story:deterministic-default-selector
kind: story
status: active
title: A deterministic selector takes useful admissible actions, reads before writes, then identifier order
summary: Calls no model; the frontier and catalogue gain effect class and open evidence kinds, the history half comes from a host port
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-conformance/src/codec.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/deterministic_selector.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-governor/tests/action_facts.rs
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: inferred
  path: docs/design/loom-design.md
- confidence: cited
  path: ess/domains/run.yaml
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:44Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":2}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
---
## Outcome

Loom ships a deterministic selector that calls no model. From the candidates it is given it keeps
only admissible actions that are useful, orders them with every action whose effect class is not
`write` before every `write` action and then by action identifier, and takes the first. What it
needs beyond the candidate's id and status comes from two ports the host passes to it, never
from the model and never from the candidate list:

- `ActionFacts`: per action its effect class and its open evidence, the kinds it may produce
  that Canon counted no record of at the subject's current revision. The Canon governor answers
  it from the IR and from the decision of the current evaluation.
- `StepHistory`: an action is not useful once `performed_at(action, case revision)` is true,
  and any action is useful again while `due(action)` is true (a host uses it for a retry or a
  pending copy).

An action not yet performed at the current case revision is useful if it may produce nothing,
or while its open evidence is not empty. An action already performed at the current case
revision is not useful again unless the host reports it due; it becomes useful again when an
artifact moves, and `max_runs` stays a ceiling in the effect (decided by session `sdc` on
2026-11-21 after adversary pass 2, `review-result:adv-w20261121w1-u3-pass-2`: under the first
rule, "useful while a kind it may produce is open", the bundled `software-change@1` selected
`tests.run` at every step, because a run without a regression command never produces
`regression_test_result`). A program that fails without writing its evidence therefore needs
`retry_when` to run again. With no
useful candidate the selector answers that nothing is admissible to do, and the run ends on its
ordinary outcome. `story:tree-run-command` is the run that uses it and implements `StepHistory`.

## Why

`FirstAdmissibleSelector` is "used only for tests/examples"
(`crates/loom-executor/src/selection.rs:140-163`), and the production selectors are models
(`crates/loom-executor/src/model_selector.rs:175`, `crates/loom-selector-laya/src/lib.rs:222`).
Admissible is not enough: the SDC driver's first run looped on an action that stayed admissible
after its evidence existed (`~/beyond10x/sdc/docs/driver.md` § "Fixed policy"). A candidate today
is `{action, status}` (`generated/rust/loom/src/run.rs:38-52`); Canon's IR has the effect class
and `may_produce` per action (`ir/mod.rs:141-143` at the pinned tag) and the governor already
walks `ir.actions` (`crates/loom-governor/src/lib.rs:1895`). `~/beyond10x/sdc/docs/boundary.md`
rows 9 and 10: the pure half of usefulness is a Canon request; until Canon ships it Loom derives
it, and the history half is the runtime's.

Two readings of the tree on 2026-11-21 decide the shape. `FrontierAction` and `CatalogueEntry`
are built by 205 struct literals in 166 files, so a new member on either breaks 202 sites; the
facts therefore come through a port and both types stay as they are. And a held record's
`applies` flag is decided once, when the record arrives (`crates/loom-governor/src/lib.rs:1428,
1779`), while revision binding is Canon's at each evaluation (`lib.rs:105-107`); open evidence
therefore comes from the current decision, not from `applies`.

## ESS first

`ess/domains/run.yaml` first: `ActionFacts` (action, optional effect, open evidence kinds) and
`SelectionStrategy` gains `Deterministic`. On that commit `task drift` fails,
`crates/loom-conformance/src/codec.rs` no longer compiles (it matches `SelectionStrategy`
exhaustively at lines 89 and 137), and `crates/loom-executor/tests/deterministic_selector.rs`
does not compile. `ess/commission/` is not changed.

Shared files: `story:child-case-runtime` and `story:canon-0-9-0-pin` also change
`crates/loom-governor/src/lib.rs`. This story goes first; both carry a `depends_on` edge to it.

## Acceptance

- `CanonGovernor` answers the action facts of a case: after a record of a kind arrives at the
  subject's current revision the kind leaves that action's open evidence, and after the
  subject's revision moves it is back, with the earlier record still held.
- An action whose `effect` the protocol does not declare has none in its facts and sorts with
  the non-write actions.
- Over a fixture frontier with two reads and two writes, all with open evidence, the order taken
  across four steps is the two reads by identifier, then the two writes by identifier.
- An admissible action whose kinds all have counted records is never selected; with only such
  actions the selector returns `NothingAdmissible`.
- With a fixture `StepHistory`: an action, with or without `may_produce` and whatever evidence
  is still open, is selected while `performed_at` is false for the current case revision and
  not selected once it is true; it is selected again at the next case revision; an action with
  no open evidence, or one already performed at this revision, is selected while `due` is true.
- On the bundled `software-change@1` with a feature change and no regression command,
  `tests.run` is taken once per case revision and `implementation.edit` is reached.
- An `ApprovalRequired` candidate is never selected, and an action the facts name that is not
  among the candidates is never returned (`AGENTS.md` § Rules).
- The same candidates, facts and history give the same choice on 100 repeated calls, and the
  selection record names strategy `Deterministic`, 0 tokens and no confidence.
- `git diff --stat` of the story names no file under `ess/commission/` or
  `generated/rust/commission/`, and no existing test file changes.
- `task check` exits 0.

## Out of scope

Usefulness computed by Canon. Effect class and open evidence as members of `FrontierAction`
(the draft `story:frontier-action-names-its-claims` would add its claims there). Changing the
selector of `b10x-loom run <intent>`. `max_runs` (enforced by the effect,
`story:bound-program-result-table`). An implementation of `StepHistory` over a store
(`story:tree-run-command`).

## Scope

Derived 2026-11-21. The `story-scoper` report was made against the earlier design (new members
on `FrontierAction` and `CatalogueEntry`); its count of 205 struct literals in 166 files is why
the story now uses a port. This section is the coordinator's reading for the port design; every
line is **cited** (read from the scoper's report or the story) or **inferred**.

- **Primary surface:** `crates/loom-executor/src/selection.rs` (the selector, the `ActionFacts`
  and `StepHistory` ports, beside `FirstAdmissibleSelector` at line 140) — cited
- **Files:** `ess/domains/run.yaml` (`ActionFacts`; `SelectionStrategy` at line 46 gains
  `Deterministic`) — cited
- **Files:** `crates/loom-governor/src/lib.rs` (a method that answers a case's action facts;
  `project` at line 1874 already walks `ir.actions`) — cited
- **Files:** `crates/loom-conformance/src/codec.rs:89,137` (exhaustive matches over
  `SelectionStrategy`) — cited
- **Files:** `crates/loom-executor/src/lib.rs:64` (the `pub use selection::{…}` re-export) — cited
- **Files:** `crates/loom-executor/tests/deterministic_selector.rs` (new) — cited
- **Also likely:** a governor test for the action facts, `crates/loom-governor/tests/action_facts.rs`
  (name guessed) — inferred
- **Documents:** `docs/contracts/loom-action-selection.md`, `docs/design/loom-design.md:74` (the
  selection-strategies list) — inferred
- **Not touched:** `ess/commission/domains/responsibility.yaml`, `crates/loom-executor/src/projection.rs`
  and the 166 files that build `FrontierAction` or `CatalogueEntry` — cited, the story's last
  acceptance line holds it
- **Confidence:** medium — the files are named by the story and the scoper's reading; that the
  current decision exposes which records Canon counted per kind is read from the story's Why and
  not from the code
- **Would collide with:** any unit touching `crates/loom-governor/src/lib.rs`,
  `crates/loom-executor/src/selection.rs`, `SelectionStrategy` in `ess/domains/run.yaml`, or
  `crates/loom-conformance/src/codec.rs`
- **Safety fact:** the selector returns only an id from the candidate list it was given
  (`select`/`resolve` at `selection.rs:309,347` refuse any other), so a facts port that names an
  unknown action cannot widen the choice — unproven
