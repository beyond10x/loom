---
format: aep.planning-md/3
id: story:deterministic-default-selector
kind: story
status: implemented
title: A deterministic selector takes useful admissible actions, reads before writes, then identifier order
summary: Calls no model; the frontier and catalogue gain effect class and open evidence kinds, the history half comes from a host port
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-conformance/src/codec.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w20261121w1_u3.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w20261121w1_u3_pass2.rs
- confidence: cited
  path: crates/loom-executor/tests/deterministic_selector.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-governor/tests/action_facts.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w1_u3.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: inferred
  path: docs/design/loom-design.md
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
- confidence: cited
  path: website/docs/reference/ess/
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:44Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":2}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
- {from: "active", to: "implemented", at: "2026-11-21T01:32:44Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"test_result":1,"review_outcome":4,"verification":1}}}
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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w1 from the unit's commits (`git diff --name-status ecefe180 ba343770`). Every line is cited from that diff.

- **Primary surface:** `crates/loom-executor/src/selection.rs` — cited, 274 lines: the `ActionFactsPort` and `StepHistory` ports, `CaseFacts`, `DeterministicSelector` beside `FirstAdmissibleSelector`, and `resolve_for_case` on `ActionSelector`, `HybridSelector` and as a free function
- **Files:** `ess/domains/run.yaml` — cited, `SelectionStrategy` gains the variant `Deterministic`; new struct `loom.run.ActionFacts` (`action`, `effect`, `may_produce`, `open_evidence`)
- **Files:** `crates/loom-governor/src/lib.rs` — cited, 257 lines: `CanonGovernor::action_facts` and its `ActionFactsPort` impl, the private `counted_kinds` rule for which records count, and `decided_at` split out of `decided`
- **Files:** `crates/loom-executor/src/lib.rs` — cited, the `pub use selection::{…}` list gains `ActionFactsPort`, `CaseFacts`, `DeterministicSelector` and `StepHistory`; the run's selection goes through `selection::resolve_for_case` with the frontier's case id
- **Files:** `crates/loom-conformance/src/codec.rs` — cited, the two matches over `SelectionStrategy` gain a `Deterministic` arm (2 lines)
- **Files:** `crates/loom-executor/tests/deterministic_selector.rs` — new, cited, the acceptance tests (449 lines)
- **Files:** `crates/loom-governor/tests/action_facts.rs` — new, cited, the governor's action-facts tests (290 lines)
- **Files:** `crates/loom-executor/tests/adversary_w20261121w1_u3.rs` and `crates/loom-executor/tests/adversary_w20261121w1_u3_pass2.rs` — new, cited, the adversary's two rounds against the selector
- **Files:** `crates/loom-governor/tests/adversary_w20261121w1_u3.rs` and `crates/loom-governor/tests/adversary_w20261121w1_u3_pass2.rs` — new, cited, the adversary's two rounds against the action facts
- **Files:** `docs/contracts/loom-action-selection.md` — cited, a new `## Deterministic selection` section (48 lines)
- **Files:** `docs/design/loom-design.md` — cited, 7 lines: the selection-strategies list names `DeterministicSelector` and links the contract section
- **Files:** `generated/rust/loom/` — cited, regenerated: `src/run.rs` gains `ActionFacts` and the `Deterministic` variant; `PLAN.md` goes from 297 to 298 capabilities; `Cargo.toml`, `plan.json` and the other 17 `src/*.rs` files take the new model and contract digests
- **Files:** `website/docs/reference/ess/` — cited, regenerated: `loom-run.md` and the other 12 domain pages
- **Files:** `AGENTS.md` — cited, 14 lines: the selector's table row and a paragraph on `CanonGovernor::action_facts` and which records count
- **Files:** `CHANGELOG.md` — cited, one `### Added` line, marked **Breaking**: `SelectionStrategy` gains `Deterministic`
- **Files:** `README.md` — cited, a sentence on `DeterministicSelector` beside the `b10x-loom-selector-laya` crate
- **Symbols:** `b10x_loom_executor::selection::DeterministicSelector<F, H>` — new, cited, `DeterministicSelector::new(case: CaseId, facts: F, history: H)`, implements `ActionSelector` with strategy `Deterministic`
- **Symbols:** `b10x_loom_executor::selection::ActionFactsPort` — new, cited, `action_facts(&self, case: &CaseId) -> Result<CaseFacts, String>`, with a blanket impl for `&P`
- **Symbols:** `b10x_loom_executor::selection::StepHistory` — new, cited, `performed_at(&self, action: &str, case_revision: i64) -> Result<bool, String>` and `due(&self, action: &str) -> Result<bool, String>`, with a blanket impl for `&H`
- **Symbols:** `b10x_loom_executor::selection::CaseFacts` — new, cited, `pub case_revision: i64` and `pub actions: Vec<ActionFacts>`
- **Symbols:** `b10x_loom_executor::selection::resolve_for_case` — new, cited, the free function; `ActionSelector::resolve_for_case` — new, cited, a provided trait method that ignores the case by default; `HybridSelector` overrides it
- **Symbols:** `loom_governor::CanonGovernor::action_facts` — new, cited, `action_facts(&self, case: &CaseId) -> Result<CaseFacts, GovernorError>`
- **Symbols:** `loom::run::SelectionStrategy::Deterministic` and `loom::run::ActionFacts` — new, cited, generated from `ess/domains/run.yaml`
- **Not touched:** `ess/commission/domains/responsibility.yaml`, `crates/loom-executor/src/projection.rs` and the files that build `FrontierAction` or `CatalogueEntry` — cited, none is in the diff, as the story's last acceptance line required
- **Predicted and not touched:** none
