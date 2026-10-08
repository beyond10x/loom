---
format: aep.planning-md/3
id: story:run-event-stream
kind: story
status: active
title: b10x-loom run writes a versioned JSONL event stream on stdout when asked
relations:
- decomposes: epic:downstream-adoption
- serves: vision:O3
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/
- confidence: inferred
  path: crates/loom-docs/
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: generated/rust/intake/
- confidence: inferred
  path: website/docs/reference/cli.md
- confidence: inferred
  path: website/docs/reference/run-events.md
revision: 18
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T07:47:45Z", actor: "human:timo", revision: 17}
- {from: "proposed", to: "active", at: "2026-10-08T07:47:45Z", actor: "human:timo", revision: 18}
---
## Outcome

`b10x-loom run` prints human lines only (`crates/loom-cli/src/main.rs` hands stdout to
`prepare_intent` and `run_prepared_intent`, which write through `crates/loom-intake-slice/src/run.rs`
and `intent.rs`). A process supervisor driving a downstream factory needs a machine-readable
record of the run. With a flag (for example `--output jsonl`), `run` writes one JSON object per
line on stdout instead: a schema version on every record, records for routing, turns, tool calls,
approvals and usage, and exactly one terminal record carrying the stop reason and the exit status.
Without the flag the human output is unchanged. Gap 1 of `epic:downstream-adoption`.

## Domain relations

- Run -> stream record, one-to-many, the run owns its records, none outlives the run that wrote
  it; the terminal record is exactly one per run - inferable (inferred from
  `crates/loom-intake-slice/src/run.rs:20-36`, where one `SliceRun` ends with one `StopReason`;
  no ess/1 document declares a stream record yet).
- Terminal record -> `intake.routing.StopReason`, many-to-one - inferable from
  `ess/intake/domains/routing.yaml` (`intake.routing.StopReason`, `intake.routing.SliceRun`).
- Usage record -> `loom.run.ReportedUsage` - inferable from `ess/domains/run.yaml`
  (`loom.run.ReportedUsage`).

## Acceptance

On a recorded run (scripted model, no network), the last line `b10x-loom run --output jsonl`
writes is the one terminal record, and its `exit_status` field equals the process exit code.

## Checks

- Every line parses as one JSON object carrying the declared schema version.
- The terminal record carries the stop reason as a separate `stop_reason` field; `exit_status` is
  the value the existing `exit_status(stop_reason)` function in `crates/loom-cli/src/main.rs`
  maps it to.
- The same run without the flag prints the current human lines byte for byte.
- The recorded run emits at least one turn, one tool-call and one usage record.
- `website/docs/reference/run-events.md` documents every record kind, the schema version and the
  terminal record, and is generated or checked by `task docs-check` so it cannot drift from the
  ESS declaration.

## ESS first

Declare the stream record in `ess/intake/domains/routing.yaml` (or a new `ess/intake` domain
listed in `ess/intake/system.yaml`): a schema-version constant, a tagged union of record kinds
(route, turn, tool call, approval, usage, terminal), and the terminal record over
`intake.routing.StopReason`. Validate with the newest `ess`, run `task intake-generate`, and
implement the writer against `generated/rust/intake/`. The red test of the first commit is the
new stream test in `crates/loom-cli/tests/`. If ESS cannot express a versioned line envelope,
stop and report it; do not hand-write the record types.

## Notes

- The record stream is a projection of what the run already prints; it adds no authority and is
  not evidence (AGENTS.md § Rules).
- Documentation: the event schema, its version and the terminal record are documented on
  `website/docs/reference/run-events.md`, alongside the regenerated `website/docs/reference/cli.md`
  (`task docs-generate`).
- Shared surface: `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/main.rs` and
  `website/docs/reference/cli.md` with `story:governor-evaluate`, which depends on this story.

