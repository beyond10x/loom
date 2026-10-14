---
format: aep.planning-md/3
id: story:regression-evidence-integrity
kind: story
status: draft
title: A regression result runs the written test, and evidence instants match their observations
relations:
- informed_by: story:change-evidence-order
- serves: vision:O1
revision: 1
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

To be scoped: `ess/intake/domains/routing.yaml` (`SliceEvidence`) and `ess/domains/governor.yaml` where the stored record changes.

## Not in scope

The ordering and clock rules story:change-evidence-order settled.
