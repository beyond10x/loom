---
title: System queries and custom protocols
sidebar_position: 2
description: Read the local clock and install protocol definitions from files or pinned Git sources.
lede: Loom bundles a system-query protocol alongside engineering protocols. Each run uses one catalog for routing, case artifacts and governor admission.
source: protocols/system-query/1.yaml, crates/loom-protocols/src/lib.rs, crates/loom-intake-slice/src/clock.rs, crates/loom-intake-slice/tests/system_query.rs, crates/loom-intake-slice/src/bindings.rs, crates/loom-cli/tests/custom_protocol_commands.rs
---

# System queries and custom protocols

System queries and custom protocols ship in 0.4.0. From a checkout of tag `0.25.0`, `task install`
builds and installs `b10x-loom` into `~/.local/bin`; that directory must be on your `PATH`.

## Read the time

With the Codex login used by [run an intent](run-an-intent.md):

```console
b10x-loom run --context-policy=bounded "need to know the current time"
```

Loom selects `system-query@1`, invokes `system.time.read {}`, prints local time with its numeric
UTC offset and the corresponding UTC instant, and exits 0 after Canon confirms completion.
The time comes from the host clock. Model-generated time or success text cannot create evidence.
The local timezone is the machine's configured timezone; named timezone conversion is not supported.

A system query needs no workspace, Git repository, test command or Substrate confinement. A supplied
`--workspace` is ignored by the clock binding, including a nonexistent path. Software changes still
require an existing Git worktree and retain test confinement and the merge approval gate.

`--context-policy` defaults to `legacy`; `bounded` limits each serialized model request to 64 KiB,
including classification. Add `--context-report PATH` for request bytes, model calls, usage and elapsed
time without source payloads. See [bounded context](../concepts/working-context.md).

## Provider overload

Loom retries a transient model failure at most twice, on the same model and account.
The waits are one and two seconds; a provider-requested delay can extend either wait up to
thirty seconds. Context reports count each attempt, including failed attempts and any reported usage.

A persistent overload stops the run with `Unavailable` and the attempt count. This is a provider
capacity failure. Earlier builds could misreport `server_is_overloaded` as `Refused`.
Actual refusals, invalid requests and authorization failures stop immediately. Once a model has
streamed output, its failed request is final. Tool effects are not retried by this policy.

## Install a local definition

The CLI includes engineering definitions from engineering-protocols and Loom's
[system-query definition](https://github.com/beyond10x/loom/blob/main/protocols/system-query/1.yaml).
Additional definitions can come from a local YAML file. From a Loom source checkout:

```console
b10x-loom protocols add team-clock@1 --file protocols/system-query/1.yaml
b10x-loom protocols list
b10x-loom protocols add team-clock@1 --file protocols/system-query/1.yaml --replace
b10x-loom protocols remove team-clock@1
```

Installation validates and snapshots the exact YAML bytes, their SHA-256 digest and provenance.
Changing or deleting the source file cannot change an installed definition. Replacing a name
requires `--replace`; bundled names cannot be replaced or removed. The registration name has the
form `lower-case-name@MAJOR`, and its major must match the definition's revision.

The store is `$XDG_DATA_HOME/loom/protocols`, or `~/.local/share/loom/protocols` when that variable is
absent. Its manifest is atomically replaced. Invalid, missing or digest-mismatched installed records
stop startup explicitly. Definitions are limited to 1 MiB each and the manifest to 64 MiB.

## Install from pinned Git

Use `protocols add NAME@MAJOR --source LOCATOR --path RELATIVE_PATH`. A locator has the form
`git+https://host/repository.git#FULL_COMMIT_SHA`; SSH and local `git+file://` repositories also work.
The commit pin must contain all 40 hexadecimal characters. Branches and tags are refused.

Installation fetches the selected regular YAML blob without a checkout. It runs no repository hooks,
filters or executable plugins. Remote URLs must not contain passwords or tokens. SSH can use the
host's authentication agent. Later `run` commands use only the installed snapshot, without fetching
or contacting its source.

## Definitions and available tools

`protocols list` shows each name, source, digest and available execution binding. Installing a
definition makes it visible to routing and governor admission. It does not install tools or confer
authority. This CLI provides the software-change binding and the clock binding. A custom definition
with the same clock semantics can use the clock binding under its own name. Any other custom
definition returns `NoLocalExecutor`, unless its actions are bound to commands (below).

## Bind a custom protocol's actions to commands

`run --action-bindings FILE` drives an installed protocol through programs you declare, one per
action, so a custom protocol needs no embedding host. The file is JSON:

```json
{
  "protocol": "team-ops@1",
  "timeout_seconds": 30,
  "bindings": [
    {"action": "note.record", "argv": ["./record-note"]},
    {"action": "summary.publish", "argv": ["./publish-summary"]}
  ]
}
```

When the run picks that protocol, each action the governor admits starts its `argv` directly,
without a shell. The action's arguments arrive as one JSON document on standard input. The
environment is cleared except for `PATH`, and a program still running after `timeout_seconds`
(60 when absent, at most 3600) is killed. Exit status 0 means `Performed`: standard output, one
JSON object of at most 1 MiB, becomes the action's report. Any other exit means `Refused`, citing
the tail of standard error.

A report may list evidence about the intent as `"evidence": [{"kind": ..., "result": ...}]`.
Each kind must be one the action declares in `may_produce`; Loom submits it, and the governor
decides from it whether the case is complete. A report that states an undeclared kind fails the
run. Nothing else in the report becomes evidence.

The protocol must be installed, not bundled, and declare only the `intent` artifact and no case
input. `record-note` here reads its arguments and prints its report:

```sh
#!/bin/sh
arguments=$(cat)
echo "$(date -u +%FT%TZ) $arguments" >> notes.log
printf '%s\n' '{"note":"n-1","evidence":[{"kind":"note_recorded","result":"recorded"}]}'
```

The transcript below comes from a run against a loopback model catalog that replays recorded
answers. With your Codex login, leave out `--confinement`, `--catalog`, `--classifier-model` and
`--model`.

```console
$ b10x-loom protocols add team-ops@1 --file team-ops.yaml
installed team-ops@1
$ b10x-loom run --confinement none --catalog lab-catalog.toml --classifier-model lab-classifier \
    --model lab-agent --action-bindings team-ops.bindings.json --unconfined-bindings \
    "record a note that the deploy finished and publish the summary"
bound commands: unconfined (--unconfined-bindings)
picked team-ops@1 (confidence 0.99)
  reason: a team note
frontier: note.record (admissible), summary.publish (blocked)
step 1: note.record {"text":"deploy finished"}
  effect: {"note":"n-1","evidence":[{"kind":"note_recorded","result":"recorded"}]}
  evidence: note_recorded recorded
frontier: note.record (admissible), summary.publish (admissible)
step 2: summary.publish {"channel":"ops"}
  effect: {"published":true,"evidence":[{"kind":"summary_published","result":"published"}]}
  evidence: summary_published published
stopped: Completed (done)
```

An action with no binding stops the run before a case opens, with exit status 3:

```console
stopped: NoLocalExecutor (no command is bound to summary.publish)
```

A file that binds an action the protocol does not declare, binds one twice or gives an empty
`argv` is refused with exit status 1 before any model call:

```console
$ b10x-loom run --action-bindings wrong.json "record a note"
b10x-loom: --action-bindings wrong.json: team-ops@1 declares no action note.erase
```

A binding confers no authority. The governor still admits each action first, and an action that
needs a capability still needs its grant. `resume` continues `software-change@1` runs only.
Bound programs are not confined by Substrate, because `--confinement` applies only to a software
change's tests. A bound program runs with the permissions of the user who runs `b10x-loom`, so
`run` starts one only when you pass `--unconfined-bindings`, and then says so on standard error.
Without the flag the run is refused with exit status 3 before any model call:

```console
$ b10x-loom run --action-bindings team-ops.bindings.json "record a note"
confinement: bound commands
stopped: ConfinementUnavailable (CommandUnconfined: Substrate does not confine the commands --action-bindings binds; pass --unconfined-bindings to run them with the operator's rights)
```

Embedders can construct `loom_protocols::ProtocolCatalog`, add in-memory YAML with `add_yaml`, and
pass the same catalog to `classify_with_catalog`, `CanonGovernor::with_catalog` and
`run_intent_with_options`. The SDK re-exports the catalog. Existing `run`, `run_with_options` and
`SliceRequest` callers retain their engineering-only behavior.

A Jira key is currently extracted as a reference; it does not fetch a ticket. System queries do not
add a Jira connector or turn Loom into an unrestricted general-purpose assistant.
