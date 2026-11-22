---
format: aep.planning-md/3
id: story:person-action-records-a-named-result
kind: story
status: proposed
title: A person action can end with a named result and its reasons, and a writer's retry_when reruns on it
summary: 'tree approve --action A --result R --reasons: R from the binding''s declared results, reasons kept as a blob the retry feedback carries; --deny unchanged; security-relevant'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:tree-run-command
- depends_on: story:every-printed-decision-command-runs
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/retry.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-tree/src/approve.rs
- confidence: cited
  path: crates/loom-tree/src/person.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:57Z", actor: "agent:loom", revision: 5}
---
## Outcome

A person action can end with a result other than `performed`, give its reasons, and send a writer
back to work with those reasons as feedback.

- `tree approve --action A --result R` records each `from: effect` entry of `A`'s person binding
  with the result its `results` table maps `R` to. Today the result is always `performed`:
  `PERFORMED` at `crates/loom-tree/src/person.rs:28` is looked up at `:102-106`.
- If any `from: effect` entry of `A` lists no rule whose value is `R`, the approve is refused
  `NotAPersonAction`, naming `A` and `R` and the values the entry lists, and nothing is written:
  no authority entry, no observation, no record, no log line. The binding's declared results are
  the only accepted values.
- Without `--result`, the result is `performed`, as today.
- `--result` requires `--action` and conflicts with `--deny`. A `rejected` result is not a
  denial: `--deny` stays as it is, an authority `Deny` with no action taken
  (`crates/loom-tree/src/approve.rs:119-123`).
- The producer of each record is the principal, as today (`submit_evidence` with the principal,
  `person.rs:145-147`). A model decider that approves as `decider:<name>` uses the same path.
- `--reasons TEXT` or `--reasons-file PATH`, which conflict with each other, keep the person's
  reasons with the step:
  - the reasons are at most 65536 bytes and 60 lines of UTF-8, the bounds of a retry log
    (`RETRY_LOG_BYTES` and `RETRY_LOG_LINES`, `crates/loom-bindings/src/retry.rs:35-38`), so
    they arrive whole;
  - `PATH` is a regular file inside the workspace, opened without following a symbolic link;
  - anything else is refused `NotAPersonAction` with nothing written;
  - the text is kept as `blobs/<sha256>` of the store, as a program's output is. The person
    observation's payload names that digest as `blob_sha256` (`person.rs:82-86` writes `case`,
    `action` and `principal` today).

The record counts for `retry_when` as a program's result does. `retry_records` already takes any
record whose `kind` and `result` a binding's `retry_when` lists (`retry.rs:58-83`). It reads the
cited run's `action`, `exit` and `blob_sha256` only from an observation of source
`PROGRAM_SOURCE` (`retry.rs:88-97`).

- The reasons fit that record without a new member: `retry_records` also reads a cited
  observation of source `loom/person` (`PERSON_SOURCE`, `person.rs:26`). The record then names the
  person's `action`, no `exit`, and the reasons blob as `blob_sha256`.
- The writer's feedback document (`LOOM_RETRY_FEEDBACK`, set at `retry.rs:207-209`) then holds the
  reasons as that record's log.
- Alternative: a member `reasons: Optional<String>` on `RetryRecord`. It is cleaner to read, and it
  is a contract change of `loom.retry-feedback/1`. The default keeps the record as it is.

Security-relevant: yes. This puts a person's text into a later program's input, and it lets a
principal name a result.

## Why

Runbook's person actions `context.approve`, `vision.approve`, `roadmap.approve` and
`merge.approve` must be able to record `rejected` (Runbook `bindings/PENDING.md:114-117`). So must
every refusal by a model decider. Today no step yields `rejected`, and `--deny` records authority
and takes no action.

## ESS first

`ess/` does not change for the default. The observation payload is not a typed noun, and
`RetryRecord` keeps its members. The first commit holds the failing tests below. If the
alternative is chosen, `ess/domains/bindings.yaml` gains `RetryRecord.reasons` first.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call. The tests use the shared tree fixture and run no whole tree per test where a store copy does.

- `tree approve --action A --result rejected` records `rejected` for each `from: effect` entry,
  produced by the principal, and the run log names the step. Test:
  `a_person_records_the_named_result`.
- `--result R` for an `R` the entry does not list is refused `NotAPersonAction` naming the listed
  values, and the store is byte for byte unchanged. Test:
  `an_undeclared_result_is_refused_and_writes_nothing`.
- Without `--result`, the result is `performed`. Test: `without_a_result_the_person_performed`.
- `--result` with `--deny` is refused by the command line (exit 2). Test:
  `a_result_with_deny_is_refused`.
- A writer whose `retry_when` lists `rejected` runs again after the person's `rejected` result.
  Its `LOOM_RETRY_FEEDBACK` document holds the record with the person's action and the reasons
  text whole. Test: `a_rejected_result_sends_the_writer_back_with_the_reasons`.
- Reasons over 65536 bytes or 60 lines, a `--reasons-file` outside the workspace, and a symbolic
  link are each refused, and nothing is written. Test:
  `reasons_out_of_bounds_or_outside_the_workspace_are_refused`.
- The reasons appear in the blob and the feedback only, never in a run-log line. Test:
  `the_reasons_are_kept_as_a_blob_not_in_the_log`.

## Not covered

- A person action with no capability: `story:every-printed-decision-command-runs`.
- Open decision requests: `story:tree-waits-open-decision-requests`.
- A case directory in the environment, which is a decision between Runbook and Loom, not a story.

## Scope

- `crates/loom-tree/src/person.rs`: `:25-28`, `:55-150`. Cited.
- `crates/loom-tree/src/approve.rs`: `:110-145`, `take_person_action` at `:174`. Cited.
- `crates/loom-cli/src/lib.rs`: `TreeApproveArgs` at `:419-442`. Cited.
- `crates/loom-bindings/src/retry.rs`: `:35-38`, `:58-110`, `:207-209`. Cited.
- `crates/loom-cli/tests/tree_run.rs`, for the new tests. Cited.
- The help of `tree approve` and `CHANGELOG.md`. Inferred.

Edges:

- depends_on `story:tree-run-command`.
- Collides with `story:every-printed-decision-command-runs` on `TreeApproveArgs` and `approve.rs`.
