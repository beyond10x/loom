# System query and protocol sources qualification — 2026-10-07

This qualifies the development implementation of `story:system-query`, after 0.3.0. It does not
claim a tagged release, published documentation or general assistant behavior.

## Contract and implementation

ESS-first commit `9346427` introduces `intake.protocols` and `intake.query`. On that commit,
`task intake-drift` failed because the generated protocol/query models were absent. Subsequent
commits generated those models, composed one catalog across routing/case opening/admission, and
bound `system.time.read` to verified host clock observations. Canon still owns completion.
Engineering protocol definitions remain in engineering-protocols; the time definition lives in Loom.

Installed YAML is data. Local snapshots and full-commit Git snapshots carry SHA-256 and provenance;
runs load their atomic manifest offline. Host code supplies tools, so an unsupported custom protocol
returns `NoLocalExecutor` even when its YAML validates.

## Live acceptance

The candidate was built with `cargo build --locked -p b10x-loom-cli` and run against the configured
hosted model using the exact previously refused intent:

```console
b10x-loom run --context-policy=bounded --context-report=metrics.json "need to know the current time"
```

The run exited 0, selected `system-query@1` with confidence 0.99 and printed:

```text
frontier: system.time.read (admissible)
step 1: system.time.read {}
  effect: Local time: 2026-10-07T13:05:57+02:00
    | UTC: 2026-10-07T11:05:57Z
  evidence: system_time observed
stopped: Completed (answered)
```

It initialized no workspace or confinement. This smoke used one model/provider configuration;
it does not establish general routing quality or a cost comparison between context policies.

| Phase | Request bytes | Input tokens | Output tokens | Cache-read tokens |
|---|---:|---:|---:|---:|
| Classification | 2,198 | 429 | 60 | 0 |
| Selection | 4,127 | 631 | 20 | 0 |
| Arguments | 5,273 | 778 | 14 | 0 |

Three calls, 11,598 cumulative request bytes, no retrievals or checkpoints, 7,662 ms elapsed.
Cache-write counters were unknown. All requests met the 65,536-byte ceiling.

## Recorded acceptance and review

`tests/system_query.rs` drives the actual governor/runtime with recorded models and an injected
clock. It covers both context policies with no workspace, `/tmp` and a nonexistent workspace,
blank unused test configuration, local and pinned-Git custom definitions after deleting their
sources, forged arguments, write selections, clock failures, low-confidence/unknown routes,
unsupported custom semantics and oversized Unicode/escaped catalog or intent requests.

Catalog tests cover duplicates, explicit replacement, digest corruption, invalid sources,
symlinks, missing manifests, regular pinned blobs and concurrent installation. Clock verifier
tests bind evidence to case and intent revisions, reject duplicate/stale observations and verify
local/UTC agreement across day and daylight-saving boundaries.

Independent code review identified three defects: early software-only test-command validation,
silent timezone fallback and omitted delegation startup time. These were assigned separate fixes
and regression tests. The review also required a software-change fixture through the new API;
legacy SliceRequest tests alone could not establish that the new path preserved merge approval.

## Live software compatibility

A separate temporary Git fixture started with `check.txt` containing `broken`. The new CLI ran
in bounded mode with default Substrate confinement, test command `/usr/bin/grep -qx fixed check.txt`,
and intent "Change check.txt from broken to fixed, run the provided test, and stop before merging."
It committed `fixed`, observed two passing test runs on the new revision and stopped at
`ApprovalRequired (repository.merge)` with exit 0. No merge or push occurred.

The report retained one classification and six selection/argument calls, a maximum request size
of 8,299 bytes, and 31,831 ms elapsed. The substrate probe's read-only filesystem diagnostics
appeared as expected. This run qualifies real confined execution; private process handoff timing
and route restoration are additionally held by regression tests.
