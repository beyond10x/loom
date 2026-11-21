---
title: Bind a protocol's actions
sidebar_position: 6
description: Write a loom.bindings/1 document that says, per action of a protocol, who performs it, how its result becomes evidence, and what it may spend; Loom refuses a file that does not fit the protocol when it loads it.
lede: A bindings file is the one place an operator declares how each action of one protocol is performed. Loom decodes it, holds it against the compiled protocol and names the action and the member of every defect.
source: crates/loom-bindings/src/lib.rs, crates/loom-bindings/src/decode.rs, crates/loom-bindings/src/load.rs, crates/loom-bindings/src/program.rs, crates/loom-bindings/tests/bindings_document.rs, crates/loom-bindings/tests/bound_program.rs, ess/domains/bindings.yaml
---

# Bind a protocol's actions

Unreleased. `b10x-loom-bindings` decodes and checks a `loom.bindings/1` document, and its program
effect port, `ProgramEffects`, performs an action bound to a `program` executor (see
[Running a program binding](#running-a-program-binding)). Decoding and checking start no process,
and the crate opens no network connection. The commands that run the other executors take the
checked document in later releases, and `b10x-loom run --action-bindings` still reads the older
`intake.bindings` format.

A bindings file belongs to one protocol, registered as `name@major`. It holds one binding per
action it performs. An action with no binding is not an error: `unbound` lists those. The file is
YAML, and JSON is read as YAML. The protocol and the file below are the ones
`crates/loom-bindings/tests/bindings_document.rs` decodes and checks from this page.

## The protocol

```yaml
format: protocol/1
protocol:
  id: review
  revision: 1
artifacts:
  draft: {}
  brief: {}
evidence_kinds:
  lint_result: {}
  check_result: {}
  draft_change: {}
  approval: {}
  translation: {}
  house_style: {}
independence_dimensions:
  different_principal: {}
actions:
  draft.lint:
    may_produce:
      - evidence: lint_result
  draft.check:
    may_produce:
      - evidence: check_result
  draft.write:
    may_produce:
      - evidence: draft_change
  draft.approve:
    may_produce:
      - evidence: approval
  draft.translate:
    may_produce:
      - evidence: translation
  draft.import:
    may_produce:
      - evidence: house_style
```

## The file

```yaml
format: loom.bindings/1
protocol: review@1
artifacts:
  draft: out/draft.md
  brief: in/brief.md
independence:
  editor: [different_principal]
actions:
  - action: draft.lint
    executor:
      kind: program
      argv: [tools/lint.sh, out/draft.md]
    timeout_seconds: 120
    evidence:
      - kind: lint_result
        subject: draft
        producer: lint
        from: exit
        results: {"0": passed, "*": failed}
  - action: draft.check
    executor:
      kind: program
      argv: [tools/check.sh, out/draft.md]
    evidence:
      - kind: check_result
        subject: draft
        producer: check
        from: /summary/state
        results: {clean: passed, "*": failed}
        upstream: [brief]
  - action: draft.write
    executor:
      kind: agent
      adapter: claude-code
      inputs: [in/brief.md]
      outputs: [out/draft.md]
    tools:
      allow: [Read, Write]
    budget: {max_runs: 3, max_turns: 40, max_tokens: 200000, max_budget_micro_usd: 2000000}
    timeout_seconds: 1500
    arguments: {tone: plain}
    retry_when:
      check_result: [failed]
    handoff: The draft is written; an editor reads it and approves.
    evidence:
      - kind: draft_change
        subject: draft
        producer: writer
        from: effect
        results: {performed: applied}
        upstream: [brief]
  - action: draft.approve
    executor:
      kind: person
      principal: editor
    evidence:
      - kind: approval
        subject: draft
        producer: editor
        from: effect
        results: {performed: approved}
  - action: draft.translate
    executor:
      kind: open
      each: language
      bindings: translate.bindings.yaml
    evidence:
      - kind: translation
        subject: draft
        producer: loom
        from: effect
        results: {performed: opened}
  - action: draft.import
    executor:
      kind: copy
      evidence: house_style
      from: house-style@1
```

## What each member means

- `executor.kind` is one of `program`, `agent`, `person`, `open` and `copy`. A `program` runs an
  `argv`, a list of strings and never a shell string. An `agent` names an `adapter`, a `model`
  (`claude-haiku-5-5` when it names none), and `inputs` and `outputs` as paths in the workspace. A
  `person` names the `principal` who alone takes the action. An `open` takes a `name` or an `each`
  argument and the child's `bindings` file, a path relative to the directory of the bindings file that names it, naming a
  file (not `.`, not ending in `/`). A `copy` names the evidence
  kind it copies as `evidence` and the protocol id of the case it copies from as `from`.
- `arguments` is a literal object; `arguments_from` is an argv whose standard output is the
  object. They are alternatives.
- Each `evidence` entry says what the action records: the `kind` (one the action `may_produce`),
  the `subject` and `upstream` artifacts, the `producer`, and where the value comes from. `from:
  exit` reads the program's exit code; `from: effect` is Loom's own report that it performed the
  action, whose one value is `performed`, and an effect that was refused or failed records nothing;
  a JSON pointer such as `/summary/state` reads the program's report. A `program` executor's
  entries read `exit` or a pointer and never `effect`; a `person` or `open` executor's entries read
  `effect` alone; an `agent` or `copy` executor's are not restricted. `results` maps each value, or
  `*` for any other, to the evidence result; `null` records no result.
- `tools.allow` lists the tool names an `agent` may be offered. An entry is never empty and holds
  no comma and no white space, because the session executor joins the names with commas.
- `budget` holds `max_runs` (8 when absent) and, for an `agent`, `max_turns`, `max_tokens` and
  `max_budget_micro_usd`, each a whole number from 1. `timeout_seconds` is from 1 to 3600.
- `retry_when` maps an evidence kind of the protocol to the results that send the action back to
  work; a result is never empty.
  `handoff` is the text a run shows when this writer has done what it may and a person continues.
- `artifacts` maps an artifact id of the protocol to a path in the workspace; `independence` maps
  a producer to dimensions the protocol declares. The `artifacts`, `inputs` and `outputs`
  paths are relative to the workspace root, and an `open` executor's `bindings` is relative to the
  directory of the file that names it; none is absolute, empty or holds a `..` segment.
- `protocol` is the registration the protocol catalog holds, `name@major`: lowercase letters,
  digits and hyphens in `name`, starting with a lowercase letter and at most 100 bytes, then a
  positive major written without leading zeros. A protocol whose id is `software.change` at
  revision 1 is registered as `software-change@1`: dots and underscores become hyphens.
- A mapping key is read as written, so `0` and `"0"` are the same key and `1.50` is not `1.5`; a
  key named twice (also through two spellings), a `<<` merge key and a tag such as `!x` are
  refused; the standard `!!` tags (`!!str`, `!!int`) are resolved by the parser and load. Keys of
  a `results` table are kept as written, so `01` and `1` are two rows, and an exit status matches
  the row written as its decimal number. YAML aliases are read, and a document that nests deeper
  than 64 levels, expands to more than 100,000 values or to more than 1 MiB of text, or is larger
  than 1 MiB, is refused. As a guard against a parser cost, text that holds more than 2048 open
  brackets or braces net of closers is refused before it is parsed; the count includes brackets in
  comments and strings.

## What Loom refuses

Decoding refuses an undeclared member at any level, a `format` that is missing or is not
`loom.bindings/1`, a `protocol` that is not a registration, an empty `argv` or one that is not a
list of strings, an `effect` entry whose `results` has a key other than `performed`, a key named
twice, a tag, a merge key, text nested or expanded past the bounds above, and a tool name that is
empty or holds a comma or white space. Loading against the protocol refuses an action the protocol does not declare, a duplicate
action, an evidence `kind` outside the action's `may_produce`, a `subject`, `upstream` entry or
`artifacts` key that is not a declared artifact, a `tools` member, an agent budget member, `inputs`
or `outputs` on an executor that is not an `agent`, a `timeout_seconds` outside 1 to 3600, an
`artifacts`, `inputs`, `outputs` or `open` `bindings` path that is absolute or leaves the
workspace, a `retry_when` kind the protocol does not declare, an evidence `from` the executor does
not take, and an `independence` dimension the protocol does not declare. Each refusal names the action and the
member, such as `action draft.write, member tools.allow[1]`. The reasons are the members of
`loom.bindings.BindingsRefusalReason` in the [reference](../reference/ess/loom-bindings.md).

## Running a program binding

A host builds `ProgramEffects` per case, from the governor that holds the case, the checked
document, the case's workspace root, the store directory and its own runner, which it may not give.
The port is a Commission effect port: it performs the actions bound to `program` and no other.

- The runner is admitted when the port is built, before anything starts. No runner is refused
  `NoRunner`. A runner that launches through Substrate's `local` adapter is admitted only when it
  was opened with the reason the `--unconfined-bindings` flag gives, and is refused
  `CommandUnconfined` for any other. The runner then checks it can start a child in the workspace
  root, and its refusal is passed on: a runner that runs test commands only is refused `NoRunner`.
- Every program starts through that runner alone, the `arguments_from` argv included, with its
  `argv` (no shell), the arguments as one JSON object on standard input, `PATH` alone in its
  environment, the binding's `timeout_seconds` (60 when absent), and the workspace root as its start
  directory. Under a runner that is not `local`, a program at an absolute path outside `/usr`,
  `/bin`, `/lib` and `/lib64` is mounted read-only at its own path. A host `PATH` that is not UTF-8
  refuses the step, `<program> was not started: the host's PATH is not UTF-8`, and starts nothing.
- The arguments are the request's, else the binding's `arguments`, else the object the
  `arguments_from` argv prints, else `{}`. A request has no arguments when they are the empty object
  `{}`; any other value, `null` included, reaches the program as written. An `arguments_from` argv
  that does not exit 0, or prints anything but one JSON object of at most 1 MiB, refuses the step
  `ArgumentsUnreadable`, and the bound program does not start. The `arguments_from` program
  starts on every step that reaches it, leaves no observation and does not count against
  `budget.max_runs`.
- A case held under another protocol than the document's `protocol` refuses the step
  `CaseOfAnotherProtocol`, naming both, before anything starts, and nothing is recorded.
- A port hands an action's program to its runner at most `budget.max_runs` times in a case,
  whatever the runner answers, unless the case store refuses that run's observation (the step
  then answers the store's error, and the run is not counted): the count is the observations the port kept for that action
  in that case, kept in the case's store, so it survives a reopen. One more is refused
  `RunsSpent`, and nothing starts. The cap holds for one port that drives the case; two hosts that
  drive one case store at the same time can each start the action once more.
- Each run's standard output followed by its standard error is kept as one file, `blobs/<sha256>`
  in the store directory, written to a temporary file there and renamed; a link at that name is
  replaced, never written through, and a `blobs` that is a link fails the step with no blob
  written and no evidence recorded. The port writes one observation (`loom.bindings.ProgramRun`)
  for every call of the runner, whatever it answered: the case, the action, the exit code, the
  blob's SHA-256 (absent when the blob was not kept), whether the program was killed at its
  timeout and, when the runner refused the child (a program ended by a signal under `local`
  is one), the refusal's text. That run counts, and the step then answers the runner's refusal or
  the blob failure.
- For each `evidence` entry the port reads the value its `from` names: the exit code, `timeout`
  for a program killed at its timeout, or the value at the JSON pointer of the report, the program's
  standard output read as one JSON object of at most 1 MiB, nested at most 64 deep, that names no
  member of an object twice; a text value is read as its text, any other as its JSON. The
  `arguments_from` output is read by the same rule. It looks the value up in `results`,
  falling to `*`. A value neither lists refuses the step `ResultNotInTable`; a report that cannot be
  read, or that lacks the pointer, refuses it `ReportUnreadable`. A refused step records no
  evidence.
- Otherwise the port records one record per entry (`loom.bindings.RecordedEvidence`), of the
  entry's `kind` about its `subject`, with the entry's `producer`, the observation's instant, the
  revision the case holds for the subject once the step is written and, as `upstream_revisions`,
  for each `upstream` artifact. The record cites the observation. A program never names a result,
  a revision, a producer or an instant: members of its report with those names change nothing.

The step refusals are the members of `loom.bindings.ProgramRefusalReason` in the
[reference](../reference/ess/loom-bindings.md).

## Revisions of placed artifacts

Loom computes the revision of every artifact the document places in `artifacts`; no program, report,
argument or bindings document names one. The revision is `t` and the first 16 hex digits of a
SHA-256, or `none` while nothing is at the path:

- a file: over its bytes, so a file holding `abc` is `tba7816bf8f01cfea`;
- a symbolic link: over its target string; the link is never followed;
- a directory: over the header `loom.bindings tree/1` and a line feed, then, for each entry below it
  in path order (each directory's names in byte order), the entry's kind (`d`, `f` or `l`), its
  path relative to the placed one, a NUL byte, the length of its bytes as eight big-endian bytes,
  and the bytes. Listing order changes nothing; one byte or one name changes the revision.

A placed path is read from the workspace root one segment at a time, never following a link. A link
among the path's directories, wherever it points, an absolute path and a `..` segment are refused
`OutsideWorkspace` and nothing is read. More than 64 MiB, 10,000 entries or 64 levels under one
placed path is `TooLarge`; a file or directory that changes while it is read is
`ChangedWhileRead`; a pipe, a socket or a device is `Unreadable` and is never opened
(`loom.bindings.RevisionRefusalReason`).

The program effect port digests every placed artifact before the program starts and again once it
ended:

- A placed path that cannot be digested refuses the step `ArtifactUnreadable`: before the run
  nothing starts, after it no revision moves and nothing is recorded.
- Otherwise each placed artifact whose digest differs from the revision the case holds moves to it,
  and the case revision rises by one per artifact moved, in the same store write as the step's
  records (`CanonGovernor::record_step`): a crash leaves the case as before the step or with all
  of it. An artifact the document does not place keeps its revision. A runner refusal, a blob
  failure, `ReportUnreadable` and `ResultNotInTable` write the moves alone.
- An action whose effect class is not `write` (`read` or none) whose program changed a placed
  artifact records nothing: the step is `Discarded` with `ChangedByNonWriter`, and the moves are
  still written. The report of a performed step names the outcome, the placements and the moves
  (`loom.bindings.BoundStep`).

`PlacedRevisions` wraps the governor a host hands Commission's loop: before every `frontier` and
`completion` it digests every placement and writes the moves, so a person's edit in the workspace
or another case's write reaches the case as a new revision before the frontier is read. A placed
path that cannot be digested makes that evaluation `GovernorUnavailable`.
