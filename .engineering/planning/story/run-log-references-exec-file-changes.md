---
format: aep.planning-md/3
id: story:run-log-references-exec-file-changes
kind: story
status: draft
title: A run log line references the Substrate file-change record of the exec it ran
summary: Exec id and change-record reference on action finished; under epic:tree-daemon-under-enforced-limits; blocked until Substrate keeps per-exec records
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:hash-chained-run-log
- depends_on: story:limit-hits-in-run-log
- depends_on: story:budget-and-limit-stops-raise-decision-requests
revision: 2
---
## Outcome

A case's run log references the file changes of every process the run started through the
Substrate daemon. The `action finished` line of a step whose process ran through the daemon
carries the exec id and a reference to Substrate's change record for that exec (its id and the
digest of the record), never the changes themselves. A reader resolves the reference through
`b10x-substrate-sdk`. A step whose process did not run through the daemon, and a step that ran
none, carry no reference; a record Substrate reports as absent is noted as absent, never as empty.

## Why

Session `sdc`, 2026-11-21: Substrate will produce per-exec file-change records; SDC's interim
format is `~/beyond10x/sdc/docs/fs-changes.md`; Loom's run log should be able to reference them.
`fs-changes.md:13-15` names the target: every process runs through Substrate, the change record
belongs there per exec id, and "Loom's run log then references the exec's change record".
The run log is `story:hash-chained-run-log`'s; exec ids exist only on the daemon path
(`story:substrate-daemon-runner`).

## Waits on

- `upstream-blocker:substrate-exec-file-change-record`: no per-exec file-change record exists.
- `story:hash-chained-run-log`: the run log and its `action finished` event.
- `story:limit-hits-in-run-log`: it also changes `RunLogEvent` in `ess/domains/runlog.yaml` from
  the daemon's observation, and depends on `story:substrate-daemon-runner`, which gives the exec
  id.
- `story:budget-and-limit-stops-raise-decision-requests`: the last story before this one to change
  `ess/domains/runlog.yaml` and `crates/loom-tree`.

## ESS first

`ess/domains/runlog.yaml`: the `action finished` event gains `exec` (exec id) and `fs_changes`
(`FileChangeReference`: record id, digest, or `absent`). On that commit `task drift` fails, and
`crates/loom-tree/tests/fs_change_reference.rs` does not compile.

## Acceptance

Through the Substrate daemon runner with a recorded daemon (no network):

- A stub program that creates one file and modifies another gives an `action finished` line with
  its exec id and a reference whose digest equals the recorded change record's.
- A step under the in-process adapter gives a line with neither member.
- A daemon that reports no record for an exec gives `fs_changes: absent`.
- No run log line contains a path or diff from the change record (a test greps a canary file name).
- `tree verify` exits 0; `docs/contracts/run-log-v1.json` is regenerated.
- `task check` exits 0.

## Out of scope

Reading or rendering the changes (the SDC view's). `sdc-watch` and its `changes.jsonl`. Recording
changes in Loom itself.
