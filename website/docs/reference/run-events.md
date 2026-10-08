---
title: Run events
sidebar_label: Run events
sidebar_position: 3
description: The JSON Lines record b10x-loom run writes with --output jsonl, its schema version, every record kind and the terminal record.
source: ess/intake/domains/events.yaml, crates/loom-cli/src/events.rs, crates/loom-cli/src/main.rs, crates/loom-cli/tests/run_events.rs
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
| `kind` | The record kind: `Route`, `Turn`, `ToolCall`, `Usage`, `Approval` or `Terminal`. |

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

### Terminal

The last line, exactly once per run.

| Field | Type | Meaning |
|---|---|---|
| `stop_reason` | string, optional | Why the run stopped (`intake.routing.StopReason`); absent when the run failed |
| `exit_status` | integer | The exit status the process returns |
| `protocol` | string, optional | The protocol the run worked |
| `steps` | integer, optional | The actions performed, refused ones included |
| `error` | string, optional | Why the run failed; present only when it did |

## The terminal record

`exit_status` is the status `b10x-loom` exits with: `0` for `Completed` and `ApprovalRequired`, `3`
for every other stop reason, and `1` for a run that failed (the exit statuses are in the
[CLI reference](cli.md)). A command line that is not valid exits `2` before any record is written.
A run that restarts itself in a delegated cgroup returns the delegated process's status, and that
process writes the terminal record.

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
{"arguments":"{\"action\":\"repository.edit\"}","call_id":"call-3","kind":"ToolCall","name":"select_action","schema_version":1,"turn":4}
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

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.10s
```

## A failed run

A run that fails before it has a stop reason still ends with the one terminal record. Here the
Codex login cannot be located; the error goes to standard error as well:

```console
$ HOME=$PWD/empty CODEX_HOME=empty b10x-loom run --output jsonl --confinement none "What time is it?"; echo "exit: $?"
b10x-loom: CODEX_HOME is the relative path empty, so the Codex login cannot be located; set CODEX_HOME to an absolute directory
{"error":"CODEX_HOME is the relative path empty, so the Codex login cannot be located; set CODEX_HOME to an absolute directory","exit_status":1,"kind":"Terminal","schema_version":1}
exit: 1
```
