---
title: Run events
sidebar_label: Run events
sidebar_position: 3
description: The JSON Lines record b10x-loom run writes with --output jsonl, its schema version, every record kind and the terminal record.
source: ess/intake/domains/events.yaml, docs/contracts/run-events-v1.json, crates/loom-docs/src/run_events.rs, crates/loom-cli/src/events.rs, crates/loom-cli/src/main.rs, crates/loom-cli/tests/run_events.rs, crates/loom-cli/tests/session_events.rs, crates/loom-session-executor/src/lib.rs
---

# Run events

`b10x-loom run --output jsonl` writes one JSON object per line on standard output instead of the
human lines, for a process that supervises the run. Standard error is unchanged. Without the flag,
or with `--output human`, the output is the human lines described in
[Run an intent](../guides/run-an-intent.md), byte for byte.

The records are declared in ESS as the `intake.events` domain
([`ess/intake/domains/events.yaml`](https://github.com/beyond10x/loom/blob/main/ess/intake/domains/events.yaml),
reference: [Run events domain](ess/intake-events.md)), and the writer builds each line from the
types generated from it. `loom-docs generate --check` (`task docs-check`) fails when this page
leaves out a record kind, one of its fields or the schema version.

A record is a projection of what the run does. It grants no authority and is not evidence: a
`ToolCall` record is what a model asked for, not what happened.

## Lines

Every line is one JSON object with these two members, beside the record's own fields:

| Member | Value |
|---|---|
| `schema_version` | The line format's version, `1` (`intake.events.SchemaVersion` `V1`). A consumer refuses a version it does not know. |
| `kind` | The record kind: `Route`, `Turn`, `ToolCall`, `Usage`, `Approval`, `Session` or `Terminal`. |

A field the run does not have is absent, never `null` or zero. Members are written in alphabetical
order; read them by name. Each line is flushed when it is written.

## Record kinds

### Route

The protocol the router picked, written once after classification. A run that restarts itself in
a delegated cgroup writes it before the restart; the delegated process does not repeat it.

| Field | Type | Meaning |
|---|---|---|
| `protocol` | string | The pick, as `name@major`; for a refused pick, the one refused |
| `accepted` | boolean | `true` when the run goes on to execute the pick; `false` when it stopped at routing (`Refused`, `NoLocalExecutor`) |

### Turn

One model turn the run received, for the classifier and the agent alike.

| Field | Type | Meaning |
|---|---|---|
| `turn` | integer | The turn's number in the run, from 1 |
| `phase` | string | The request it answered: `Classification`, `Selection` or `Arguments` |
| `model` | string | The model of the binding that answered |
| `turn_stop` | string | Why the provider ended the turn: `EndTurn`, `ToolCalls`, `MaxOutputTokens` or `Incomplete` |

### ToolCall

One tool call a model made in a turn.

| Field | Type | Meaning |
|---|---|---|
| `turn` | integer | The turn it was made in |
| `call_id` | string | The provider's id for the call |
| `name` | string | The tool: `pick_protocol`, `select_action` or `action_arguments` |
| `arguments` | string | The arguments as the model wrote them, as JSON text |

### Usage

The counters a provider reported for one turn. A turn with no reported usage has no `Usage`
record.

| Field | Type | Meaning |
|---|---|---|
| `turn` | integer | The turn they belong to |
| `model` | string | The model of the binding that answered |
| `input_tokens` | integer, optional | Input tokens |
| `output_tokens` | integer, optional | Output tokens |
| `cached_input_tokens` | integer, optional | Input tokens read from a cache |
| `cache_creation_input_tokens` | integer, optional | Input tokens written to a cache |
| `reasoning_output_tokens` | integer, optional | Output tokens spent on reasoning |
| `final_usage` | boolean | `false` marks a partial snapshot, never a final cost |

### Approval

Written just before the terminal record when the run stopped `ApprovalRequired`.

| Field | Type | Meaning |
|---|---|---|
| `actions` | list of strings | The actions the run awaits approval for, in frontier order |

### Session

One agent session metaharness ran for the run, through the session executor
(`b10x-loom-session-executor`), written once after the session ended. `b10x-loom run` itself runs
no session executor, so its stream has no `Session` line; a host that runs its agent work through
one writes the line with `EventStream::sessions` (`b10x_loom_cli::events`). It names the session's
`trace-ir/1` document by its digest and holds metaharness's `trace-ir-summary/1` of it
(`metaharness::protocol::summarize`, metaharness 0.13.0). The session's own turns, tool calls and
usage are in that document: they have no `Turn`, `ToolCall` or `Usage` record, and the stream's
`Turn` numbers count Loom's own model calls only.

Every field after `source` is the summary's field of the same name, and metaharness owns what it
means; a field the summary leaves `null` is absent. A record metaharness cannot project, and a
summary the line cannot hold as stated (a cost that is no finite number, a count past the largest
integer, 9223372036854775807), leaves every summary field absent and says why in
`projection_error`; the run goes on.

| Field | Type | Meaning |
|---|---|---|
| `transcript_digest` | string, optional | The lowercase hex SHA-256 of the session's record: the `trace-ir/1` document's own digest and the digest on the executor's step; absent when the session left no record |
| `source` | string, optional | What the record was read as: `claude-code-stream-json`, `codex-rollout`, `harness-json`, or `metaharness.event/1` for the events the executor received when metaharness kept no vendor record |
| `adapter` | string, optional | The adapter the session's opening record names |
| `model` | string, optional | The model the session opened with |
| `models` | list, optional | Every model the record names, in first-seen order, each with `name` and the figures the record states for it alone: `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` (integers) and `cost_usd` (number) |
| `turns` | integer, optional | The session's own turn count, as its terminal record states it |
| `tokens_basis` | string, optional | Where the four token counts come from: `session` (the terminal record's totals) or `sum-of-requests` (each request's usage, summed) |
| `input_tokens` | integer, optional | Input tokens, read as `tokens_basis` says, not normalized |
| `output_tokens` | integer, optional | Output tokens |
| `cache_read_input_tokens` | integer, optional | Input tokens read from a cache |
| `cache_creation_input_tokens` | integer, optional | Input tokens written to a cache |
| `cost_usd` | number, optional | The session's total cost in US dollars, the vendor's figure passed through |
| `duration_ms` | integer, optional | The terminal record's `duration_ms` |
| `duration_api_ms` | integer, optional | The terminal record's `duration_api_ms` |
| `first_timestamp_ms` | integer, optional | The first timestamp the record states, in Unix milliseconds |
| `last_timestamp_ms` | integer, optional | The last timestamp the record states |
| `is_error` | boolean, optional | The terminal record's error flag |
| `stop_reason` | string, optional | The terminal record's stop reason |
| `tool_calls` | list, optional | One entry per tool call in the record, in its order: `call_id` and `name` (strings), `answered` (boolean: a result names the call) and `error` (boolean: that result's error flag) |
| `unk` | integer, optional | The document's unk count: the sum over its `metaharness.unk_kinds` |
| `projection_error` | string, optional | Why the record could not be projected or its summary not written, naming no directory of the machine; present only then. A session that left a record has `transcript_digest` and `source` beside it; one that left none (`the session left no record`) has neither |

A session refused before it started (its scratch copy could not be made, for one) has no record
and no `Session` line.

### Terminal

The last line, exactly once per run.

| Field | Type | Meaning |
|---|---|---|
| `stop_reason` | string, optional | Why the run stopped (`intake.routing.StopReason`); absent when the run failed |
| `exit_status` | integer | The exit status the process returns |
| `protocol` | string, optional | The protocol the run worked |
| `steps` | integer, optional | The actions performed, refused ones included |
| `error` | string, optional | Why the run failed; present only when it did |
| `unperformed_actions` | list of strings, optional | On a `NoLocalExecutor` stop, the actions no local executor performs, each once: every action of the picked protocol the run has no executor for, in the protocol's order, or, once a case opened, the actions of the frontier it stopped on, in frontier order; absent on every other stop |

## The terminal record

`exit_status` is the status `b10x-loom` exits with: `0` for `Completed` and `ApprovalRequired`, `3`
for every other stop reason, and `1` for a run that failed (the exit statuses are in the
[CLI reference](cli.md)). A command line that is not valid exits `2` before any record is written.
A run that restarts itself in a delegated cgroup returns the delegated process's status, and that
process writes the terminal record.

## Stability

The records and the exit statuses are a contract at `schema_version` 1. Within it:

- a record kind or a field is never removed, renamed or retyped, a field's optionality never
  changes, and a value is never added to or removed from a field's listed values;
- an exit status never changes meaning;
- a new optional field of an existing record kind, or of a record one of its fields holds (an entry
  of `models` or `tool_calls`), is not a break: a reader that reads members by name ignores it;
- anything else, a new record kind included, is a break. It is named in that release's CHANGELOG
  entry, in a bullet that starts `**Breaking:**`, and the committed contract moves with it.

The `Session` kind was added at `schema_version` 1 as such a break: a reader that refuses a record
kind it does not know refuses a `Session` line.

The contract is committed as
[`docs/contracts/run-events-v1.json`](https://github.com/beyond10x/loom/blob/main/docs/contracts/run-events-v1.json):
the record kinds, their fields with type and optionality, the schema versions and the exit
statuses, written by `loom-docs generate` from the declaration. `loom-docs generate --check`
(`task docs-check`) compares the declaration with it, names each difference by kind and field, and
fails on a break, so the stream cannot change under a reader unless the contract file moves with
it.

## A recorded run

The acceptance test drives a recorded run, with scripted models and no network, the way the binary
does, and prints the stream it wrote. The models are fixtures, so `model` reads `recorded`:

```console
$ cargo test -p b10x-loom-cli --locked --test run_events every_line_is_one_object_carrying_the_schema_version -- --nocapture

running 1 test
{"kind":"Turn","model":"recorded","phase":"Classification","schema_version":1,"turn":1,"turn_stop":"ToolCalls"}
{"arguments":"{\"confidence\":0.99,\"protocol\":\"software-change@1\",\"reasons\":[\"change code\"]}","call_id":"call-1","kind":"ToolCall","name":"pick_protocol","schema_version":1,"turn":1}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":1}
{"accepted":true,"kind":"Route","protocol":"software-change@1","schema_version":1}
{"kind":"Turn","model":"recorded","phase":"Selection","schema_version":1,"turn":2,"turn_stop":"ToolCalls"}
{"arguments":"{\"action\":\"tests.run\"}","call_id":"call-1","kind":"ToolCall","name":"select_action","schema_version":1,"turn":2}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":2}
{"kind":"Turn","model":"recorded","phase":"Arguments","schema_version":1,"turn":3,"turn_stop":"ToolCalls"}
{"arguments":"{}","call_id":"call-2","kind":"ToolCall","name":"action_arguments","schema_version":1,"turn":3}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":3}
{"kind":"Turn","model":"recorded","phase":"Selection","schema_version":1,"turn":4,"turn_stop":"ToolCalls"}
{"arguments":"{\"action\":\"implementation.edit\"}","call_id":"call-3","kind":"ToolCall","name":"select_action","schema_version":1,"turn":4}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":4}
{"kind":"Turn","model":"recorded","phase":"Arguments","schema_version":1,"turn":5,"turn_stop":"ToolCalls"}
{"arguments":"{\"files\":[{\"contents\":\"fixed\\n\",\"path\":\"check.txt\"}],\"message\":\"update the check\"}","call_id":"call-4","kind":"ToolCall","name":"action_arguments","schema_version":1,"turn":5}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":5}
{"kind":"Turn","model":"recorded","phase":"Selection","schema_version":1,"turn":6,"turn_stop":"ToolCalls"}
{"arguments":"{\"action\":\"tests.run\"}","call_id":"call-5","kind":"ToolCall","name":"select_action","schema_version":1,"turn":6}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":6}
{"kind":"Turn","model":"recorded","phase":"Arguments","schema_version":1,"turn":7,"turn_stop":"ToolCalls"}
{"arguments":"{}","call_id":"call-6","kind":"ToolCall","name":"action_arguments","schema_version":1,"turn":7}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":7}
{"kind":"Turn","model":"recorded","phase":"Selection","schema_version":1,"turn":8,"turn_stop":"ToolCalls"}
{"arguments":"{\"action\":\"repository.merge\"}","call_id":"call-7","kind":"ToolCall","name":"select_action","schema_version":1,"turn":8}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":8}
{"kind":"Turn","model":"recorded","phase":"Arguments","schema_version":1,"turn":9,"turn_stop":"ToolCalls"}
{"arguments":"{}","call_id":"call-8","kind":"ToolCall","name":"action_arguments","schema_version":1,"turn":9}
{"cached_input_tokens":0,"final_usage":true,"input_tokens":120,"kind":"Usage","model":"recorded","output_tokens":7,"schema_version":1,"turn":9}
{"actions":["repository.merge"],"kind":"Approval","schema_version":1}
{"exit_status":0,"kind":"Terminal","protocol":"software-change@1","schema_version":1,"steps":3,"stop_reason":"ApprovalRequired"}
test every_line_is_one_object_carrying_the_schema_version ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.13s
```

## A recorded session

The session acceptance test runs a commission whose agent is the session executor over a committed
recorded Claude Code session (`crates/loom-cli/tests/fixtures/session/claude-code.jsonl`, synthetic
ids), replayed by metaharness's `ScriptedRunner`: no harness program runs, no model is called and
nothing reaches the network. The session read one file and proposed one edit, which the run
performed; its two turns and its `Read` call are in the `Session` line, not in `Turn` or
`ToolCall` lines:

```console
$ cargo test -p b10x-loom-cli --locked --test session_events a_session_event_holds_the_summary_of_the_recorded_session -- --nocapture

running 1 test
{"accepted":true,"kind":"Route","protocol":"software-change@1","schema_version":1}
{"adapter":"claude","cache_creation_input_tokens":0,"cache_read_input_tokens":1900,"cost_usd":0.0123,"duration_api_ms":4100,"duration_ms":5200,"first_timestamp_ms":1792659600000,"input_tokens":2500,"is_error":false,"kind":"Session","last_timestamp_ms":1792659605000,"model":"a-model","models":[{"cache_creation_input_tokens":0,"cache_read_input_tokens":1900,"cost_usd":0.0123,"input_tokens":2500,"name":"a-model","output_tokens":78}],"output_tokens":78,"schema_version":1,"source":"claude-code-stream-json","tokens_basis":"session","tool_calls":[{"answered":true,"call_id":"call-0001","error":false,"name":"Read"}],"transcript_digest":"9cbac1617fed75f100f71e426282bcc0fab2f36c372077fecd8c459991b8f6b6","turns":2,"unk":0}
{"exit_status":0,"kind":"Terminal","protocol":"software-change@1","schema_version":1,"steps":1,"stop_reason":"Completed"}
test a_session_event_holds_the_summary_of_the_recorded_session ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.01s
```

The test prints each line as it parsed it back. Its assertions hold the `Session` line's fields to
metaharness's `summarize` of the same record; `loom_s_own_records_are_as_without_the_session_record`
holds the run's other lines to those the same run writes without `EventStream::sessions`.

## A failed run

A run that fails before it has a stop reason still ends with the one terminal record. Here the
Codex login cannot be located; the error goes to standard error as well:

```console
$ HOME=$PWD/empty CODEX_HOME=empty b10x-loom run --output jsonl --confinement none "What time is it?"; echo "exit: $?"
b10x-loom: CODEX_HOME is the relative path empty, so the Codex login cannot be located; set CODEX_HOME to an absolute directory
{"error":"CODEX_HOME is the relative path empty, so the Codex login cannot be located; set CODEX_HOME to an absolute directory","exit_status":1,"kind":"Terminal","schema_version":1}
exit: 1
```
