---
format: aep.planning-md/3
id: story:engineering-protocols-070-pin
kind: story
status: implemented
title: Loom builds on engineering-protocols 0.7.0 and Canon 0.4.0, and every software change case gives its inputs
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests
- confidence: cited
  path: crates/loom-commission-testkit/tests
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/file_store.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests
- confidence: cited
  path: crates/loom-intake-router/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/case.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/selector.rs
- confidence: cited
  path: crates/loom-intake-slice/tests
- confidence: cited
  path: crates/loom-protocols/Cargo.toml
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: cited
  path: crates/loom-sdk/examples/software_change.rs
- confidence: cited
  path: crates/loom-sdk/tests/example_runs.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: ess/domains/evaluation.yaml
- confidence: cited
  path: ess/domains/governor.yaml
- confidence: cited
  path: ess/intake/domains/routing.yaml
- confidence: cited
  path: generated
- confidence: cited
  path: website/docs
revision: 34
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T18:27:47Z", actor: "human:timo", revision: 32}
- {from: "proposed", to: "active", at: "2026-10-13T18:27:47Z", actor: "human:timo", revision: 33}
- {from: "active", to: "implemented", at: "2026-10-14T01:53:11Z", actor: "human:timo", revision: 34, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

Loom builds on `b10x-canon-engineering` `0.7.0` from `beyond10x/engineering-protocols` and on the
one Canon that release names, `b10x-canon` `0.4.0`. Every `software.change/1` case Loom writes
gives the case inputs the protocol declares (`risk`, `change_kind`, `affects_security_boundary`),
from the host and never from a model, and the slice performs the protocol's two write actions,
`tests.write` and `implementation.edit`, where it performed `repository.edit`.

## Why

- engineering-protocols `0.5.0` made `risk` and `change_kind` required inputs of
  `software.change/1` and split `repository.edit` into `tests.write` and `implementation.edit`;
  `0.7.0` made `affects_security_boundary` required and moved Canon to `0.4.0`. Canon refuses a
  case that leaves out a declared input (`missing-input`), so a governor that opens a case with
  revisions only can no longer decide one.
- Canon `0.4.0` reports an obligation `not_applicable` beside `open` and `discharged`; the
  governor's projection counts every status other than `discharged` as open, which would report
  every obligation the case's risk does not select as owed.
- `b10x-canon-engineering` names Canon by tag; a second Canon reference builds types that do not
  match (`AGENTS.md` § Governor), so Loom's Canon reference moves with the pin.

## Decision

- **Where the inputs come from.** The host, as trusted context: the governor's case opening takes
  them as a typed value; `b10x-loom run` takes `--risk` (default `standard`), `--change-kind`
  (default `feature`) and `--affects-security-boundary` (default `false`); the SDK and
  `case::open` take them as arguments. The defaults keep today's run: an edit is admissible
  without a red test first. No model, classifier or router output sets an input.
- **Checked at open.** Opening a case refuses an input the protocol does not declare, a value it
  does not list, and a declared input left out, each naming the input, before anything is stored.
  A protocol that declares no inputs (`incident.response/1`, `system.query/1`, `inbound-answer/1`)
  opens with none.
- **Stored.** `loom.governor`'s `StoredCase` keeps the inputs; reading a case stored without them
  is refused as a store failure naming the case, never filled with defaults. A state directory
  written by 0.19.0 that holds a case is not resumed by 0.20.0; the CHANGELOG says so.
- **`not_applicable` is not open.** The projection reports an obligation `open` only when Canon
  reports it `open`; `FrontierObligation.open` and `DecidedObligation.open` stay Booleans, and
  their ESS descriptions say a not-applicable obligation is not open.
- **The two write actions.** The slice performs `tests.write` and `implementation.edit` with the
  effect `repository.edit` had, and reports the workspace `HEAD` after each, as before. The
  selector's prompt and argument schema name both.
- **Actions Loom cannot perform** (`release.rollback` is new on the software-change frontier): they
  follow the rule the slice already has for an action its effects do not perform; a run never
  holds a Suspension that no resume can perform. The implementor states which rule that is, with
  the test that holds it.
- **Not in this story:** submitting `implementation_change` or `regression_test_result`, an
  `observed_at` or `producer` on the slice's evidence, an evaluation instant for `max_age`, and a
  security review. `red_first.required` and `security_review.required` therefore stay open where
  they apply; with the default inputs they do not apply.

## Acceptance

- Every `b10x-canon-engineering` dependency names tag `0.7.0` and every `b10x-canon` dependency
  tag `0.4.0`; `cargo tree --locked -i b10x-canon` and `-i b10x-canon-engineering` each show one
  copy, from that tag. `loom-protocols`' recorded revision is `0.7.0` and its pin test lists
  `ml-experiment@1`.
- The fixtures copied from the release (`crates/loom-governor/tests/fixtures/`) equal the
  `0.7.0` files byte for byte and pass.
- A governor test opens `software-change@1` with each of: all three inputs (opens), one left out,
  an undeclared one, and an unlisted value (each refused naming the input, nothing stored).
- `FileCaseStore` holds the inputs across a real process restart
  (`crates/loom-governor/tests/restart_durability.rs`), and a stored case without them is refused.
- A case at `risk: trivial` reports no obligation the protocol leaves `not_applicable` as open.
- `b10x-loom run` with no input flags runs the software-change slice to the outcomes it reaches
  today, with `implementation.edit` in place of `repository.edit`; with `--change-kind bugfix`, an
  `implementation.edit` before a failing test on the current revision is not admissible.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `cargo test --workspace --locked --no-fail-fast` exit 0 on the integration head.
- `AGENTS.md`, `README.md`, the site pages that name the pins or `repository.edit`, the CLI
  reference and `CHANGELOG.md` (**Unreleased**) say the above, naming the breaking changes for
  callers: the opening signature, the new open refusals, the `StoredCase` field and the action
  ids.

## ESS first

First commit, `ess/` only: `loom.governor.StoredCase` gains the case inputs (a list of
`{input, value}`); `intake.routing.SliceRun` gains the three inputs the host supplies; the
descriptions of `FrontierObligation.open` (`ess/commission/`) and `DecidedObligation.open`
(`ess/domains/evaluation.yaml`) say a not-applicable obligation is not open. On it `task drift`
(and `task intake-drift`) fail; later commits regenerate and implement without changing `ess/`.

## Not in scope

Changes to Canon or engineering-protocols; the evidence kinds and order or independence data
named under Decision, which are a later story.
