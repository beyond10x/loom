---
format: aep.planning-md/3
id: story:regression-evidence-integrity
kind: story
status: implemented
title: A regression result runs the written test, and evidence instants match their observations
relations:
- informed_by: story:change-evidence-order
- serves: vision:O1
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T13:31:52Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-14T13:31:52Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-14T21:39:17Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A bugfix's `regression_test_result` is the result of running the regression test the run wrote, not of the whole suite after any committing `tests.write`; and the governor applies a record's `observed_at` only when it matches the observation the record cites, which the governor received.

## Why

The review of story:change-evidence-order found two gaps it left by decision:

- `crates/loom-intake-slice/src/verifier.rs` (~164): after any committing `tests.write`, a failing suite is submitted as `regression_test_result`, though the test command need not run the written file. A one-byte change to an existing test makes an unrelated failing suite count as red first; the story's own acceptance test writes `tests/regression.txt`, which its test command never reads. `software.change/1` defines the kind as running a bugfix's regression test.
- `crates/loom-governor/src/lib.rs` (~1054): `canon_record` reads `observed_at` from the record's facts without checking it against the cited observation, or that the observation was received. No model path reaches it (only host code calling `submit_evidence`), so it is integrity hardening, not an open authority path.

## Acceptance

- A bugfix whose test command does not run the file its `tests.write` wrote submits no `regression_test_result` (how a run names the files it ran is the design question of this story; options: the host test command names its test selection, or the written file is run on its own).
- A record whose `observed_at` differs from its cited observation's instant, or that cites an observation the governor never received, is refused by the governor, naming the record.

## ESS first

The first commit changes only `ess/intake/domains/routing.yaml` (the optional field); on it
`task intake-drift` fails. Later commits regenerate `generated/rust/intake/` and make the
acceptance tests pass without changing `ess/`.

## Not in scope

The ordering and clock rules story:change-evidence-order settled.

## Decision (2026-10-14)

The host names its regression-test command: `run --regression-test-command <cmd>`, with a `{path}`
placeholder the slice fills with each path the run's `tests.write` committed. Only that command's
result is submitted as `regression_test_result`; the suite's `tests.run` result stays a
`test_result` only. A host that gives no such command gets no regression result, so
`red_first.required` stays open for it. The flag and its field are optional and kept in the resume
context like `test_paths`.

The governor applies a record's `observed_at` only when it equals the instant of the observation
the record cites and that observation was received by the governor; otherwise the record is
refused naming it.

## Design

- `ess/intake/domains/routing.yaml`: `SliceRun` and `ResumeContext` gain
  `regression_test_command: Optional<String>`; the comment states the `{path}` rule.
- `crates/loom-intake-slice/src/verifier.rs` (around the `RegressionTestResult` submit, ~189):
  after a `tests.run` on a bugfix whose `tests.write` committed, the slice runs the regression
  command once per committed test path, through the same confined `TestRunner`, and submits
  `regression_test_result` from that run only, citing its own observation; without the command,
  none is submitted.
- `crates/loom-cli/src/lib.rs`: the flag beside `--test-path`; `website/docs/reference/cli.md`
  regenerated.
- `crates/loom-governor/src/lib.rs` (`canon_record`, ~1057): the cited observation is looked up
  among the observations the governor received; a missing one or a different instant refuses the
  record, naming it. Its declaration in `ess/domains/governor.yaml` if the stored record changes.

## Scope

- `ess/intake/domains/routing.yaml`, `generated/rust/intake/` (regenerated)
- `crates/loom-intake-slice/src/verifier.rs`, `crates/loom-intake-slice/src/run.rs`,
  `crates/loom-intake-slice/src/resume.rs`, `crates/loom-intake-slice/tests/`
- `crates/loom-cli/src/lib.rs`, `crates/loom-cli/tests/slice_run.rs`
- `crates/loom-governor/src/lib.rs`, `crates/loom-governor/tests/`
- `website/docs/reference/cli.md`, `docs/contracts/` only if `task docs-generate` changes them
- `AGENTS.md` (§ Intake and the command line), `CHANGELOG.md`, `website/data/status.json`

Touches none of `ess/domains/metering.yaml`, `ess/domains/workflow.yaml`, `ess/system.yaml`,
`generated/rust/loom/`, `crates/loom-executor`, `crates/loom-sdk`, `crates/loom-xtask`,
`Cargo.toml`, `Cargo.lock`.
