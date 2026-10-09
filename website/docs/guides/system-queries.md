---
title: System queries and custom protocols
sidebar_position: 2
description: Read the local clock and install protocol definitions from files or pinned Git sources.
lede: Loom bundles a system-query protocol alongside engineering protocols. Each run uses one catalog for routing, case artifacts and governor admission.
source: protocols/system-query/1.yaml, crates/loom-protocols/src/lib.rs, crates/loom-intake-slice/src/clock.rs, crates/loom-intake-slice/tests/system_query.rs
---

# System queries and custom protocols

System queries and custom protocols ship in 0.4.0. From a checkout of tag `0.14.0`, `task install`
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
with the same clock semantics can use the clock binding under its own name; changes to actions,
artifacts, evidence, authority or completion requirements return `NoLocalExecutor`.

Embedders can construct `loom_protocols::ProtocolCatalog`, add in-memory YAML with `add_yaml`, and
pass the same catalog to `classify_with_catalog`, `CanonGovernor::with_catalog` and
`run_intent_with_options`. The SDK re-exports the catalog. Existing `run`, `run_with_options` and
`SliceRequest` callers retain their engineering-only behavior.

A Jira key is currently extracted as a reference; it does not fetch a ticket. System queries do not
add a Jira connector or turn Loom into an unrestricted general-purpose assistant.
