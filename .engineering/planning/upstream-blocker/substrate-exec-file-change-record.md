---
format: aep.planning-md/3
id: upstream-blocker:substrate-exec-file-change-record
kind: upstream-blocker
status: open
title: Substrate keeps no file-change record per exec
summary: Requested by session sdc on 2026-11-21; SDC's sdc-watch is the interim
tags:
- sdc
relations:
- blocks: story:run-log-references-exec-file-changes
revision: 1
---
## What is missing

A per-exec file-change record in Substrate: for each process Substrate starts, the effective
changes it made to the files under its workspace, keyed by the exec id, which a client can read
back. No Substrate release has one. SDC records changes in its own prototype, `sdc-watch`, which
watches a run directory with inotify and attributes each change by time window; its interim
format is `runs/<run>/fs/changes.jsonl` (`sdc.fs-change/1`), with content in `fs/blobs/<sha256>`
(`~/beyond10x/sdc/docs/fs-changes.md:9-51`). The target owner named there is Substrate, per exec
id, exact for concurrent processes too, and `sdc-watch` retires then (`fs-changes.md:13-15`).

## What it blocks

`story:run-log-references-exec-file-changes`: a run log line can reference an exec's change record
only once Substrate keeps one.

## What clears it

A Substrate release whose daemon keeps a change record per exec id (paths, operation, content
digests before and after) and serves it through `b10x-substrate-sdk`, so a client holding an exec
id reads the record or a reference to it.

## Who

Session `sdc` sent the request to a `substrate` session on 2026-11-21 (`fs-changes.md:14`). No
story id in Substrate's planning store is known here.
