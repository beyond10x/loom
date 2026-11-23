---
title: Bind a protocol's actions
sidebar_position: 6
description: Write a loom.bindings/1 document that says, per action of a protocol, who performs it, how its result becomes evidence, and what it may spend; Loom refuses a file that does not fit the protocol when it loads it.
lede: A bindings file is the one place an operator declares how each action of one protocol is performed. Loom decodes it, holds it against the compiled protocol and names the action and the member of every defect.
source: crates/loom-bindings/src/lib.rs, crates/loom-bindings/src/decode.rs, crates/loom-bindings/src/load.rs, crates/loom-bindings/src/program.rs, crates/loom-bindings/src/copy.rs, crates/loom-bindings/tests/bindings_document.rs, crates/loom-bindings/tests/bound_program.rs, crates/loom-bindings/tests/bound_copy.rs, ess/domains/bindings.yaml
---

# Bind a protocol's actions

Unreleased. `b10x-loom-bindings` decodes and checks a `loom.bindings/1` document, and its program
effect port, `ProgramEffects`, performs an action bound to a `program` executor (see
[Running a program binding](#running-a-program-binding)), and `CopyEffects` performs one bound to
`copy` (see [Copying a record from another case](#copying-a-record-from-another-case)). Decoding and checking start no process,
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
- A `person` may also name an `argv`: the program `tree approve --action` runs once when the
  principal takes the action. Each entry that is exactly `{principal}` is replaced by the
  approving principal, as one argument whatever it holds (a space, a `;` or `$(…)` adds no
  argument and no command); nothing else is replaced. The placeholder is refused `InvalidArgv`
  as `argv[0]`, inside a longer entry (`--by={principal}`), and in a `program`'s `argv`, an
  `arguments_from` or a `handoff_when`. Quote it in YAML (`"{principal}"`): unquoted in a flow
  list it is a mapping, and the argv is refused. A person takes no `arguments`, `arguments_from`
  or `exits` (`MemberNotForExecutor`). The program reads `{}` on standard input and the
  environment of a `program` step (`PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS` and, for a child
  case, the opening names); the principal is never in it. It starts through the runner
  `--confinement` names, as `tree run` starts its programs: `substrate` by default, `tree approve`
  then starting itself again in a delegated scope unless `--cgroup-root` names one; `local` for
  `--confinement local`. It runs under the binding's `timeout_seconds`, and every run counts
  against `budget.max_runs`.
  - The order: the runner is admitted (`ConfinementRefused`, `NoRunner` without one), the run
    count is below `max_runs`, and the placed artifacts are digested, all before anything is
    written; a refusal there is `PersonProgramFailed`, its detail naming `RunsSpent` or
    `ArtifactUnreadable`. Then the decision is recorded, and the program runs inside the effect
    the decision admitted.
  - Only an exit of 0 records the person's evidence, with the placed artifacts' new revisions,
    in one store write. Any other end (another exit, killed at its timeout, refused by its
    runner, a placed artifact changed by an action whose effect is not `write`
    (`ChangedByNonWriter`), a placed artifact that cannot be digested after the run) records
    nothing, keeps the approval and the run, and refuses `PersonProgramFailed`, naming the end
    and the blob of the program's output by its digest, never what it printed.
  - The refusal's `command` is the argv of the refused call, its paths resolved, in this word
    order: `b10x-loom tree approve --store <dir> --case <id> [--capability <c>] --principal <p>
    --action <a> [--result <r>] [--reasons <text> | --reasons-file <file>] [--at <instant>]
    --confinement <c> [--cgroup-root <dir>]`, each as the call named it. Once the cause is fixed,
    running it takes the action again with the same result, within `max_runs`. The
    `ApproveAction` decision `tree run` prints for such an action carries the run's own
    `--confinement` and `--cgroup-root`.
  - The program, and any program it starts, is found on the `PATH` of the `tree approve`
    process, which may differ from the `PATH` of `tree run`: start both with one `PATH`, or a
    bare name may resolve to another file, or to none.
- `arguments` is a literal object; `arguments_from` is an argv whose standard output is the
  object. They are alternatives.
- Each `evidence` entry says what the action records: the `kind` (one the action `may_produce`),
  the `subject` and `upstream` artifacts, the `producer`, and where the value comes from. `from:
  exit` reads the program's exit code; `from: effect` is Loom's own report that it performed the
  action, whose value is `performed`, and an effect that was refused or failed records nothing (a
  `person` executor's table may also list named values, never `*`: the result `tree approve
  --result` names);
  a JSON pointer such as `/summary/state` reads the program's report; `from: runs` is Loom's
  count of the action's runs, whose value is `spent` on the run that reaches `budget.max_runs`
  and which records nothing on any other run (a cancelled run counts and records nothing, so a
  cancelled last run writes no `spent` record); its table lists `spent` and nothing else, and the
  record is written after the run's other records, about the revision the run left. A protocol
  reads it through a claim over the entry's kind, such as an outcome's `requires`; the kind alone
  never makes the writer useful again. A `program` executor's entries read `exit`, a pointer or
  `runs` and never `effect`; a `person` or `open` executor's entries read `effect` alone; an
  `agent` or `copy` executor's read anything but `runs`. `results` maps each value, or `*` for any
  other, to the evidence result; `null` records no result.
- `tools.allow` lists the tool names an `agent` may be offered. An entry is never empty and holds
  no comma and no white space, because the session executor joins the names with commas.
- `budget` holds `max_runs` (8 when absent) and, for an `agent`, `max_turns`, `max_tokens` and
  `max_budget_micro_usd`, each a whole number from 1. `timeout_seconds` is from 1 to 3600.
- `exits`, on a `program` alone, holds `error` and `not_called`, each a list of exit codes from 0
  to 255 (a list left out is empty). An exit `error` lists is a program error and one
  `not_called` lists a run that did nothing; neither goes through the `results` tables. Loom
  ships no default list: a binding with no `exits` puts every exit through its tables, but an
  exit from 129 to 192, which Substrate gives a program a signal ended, is refused
  `ProgramSignalled` before either is read (below). A code in
  both lists, or in a list and as a key of a `from: exit` table of the same binding, is refused
  `ExitClassConflict`.
- `program_root`, at the top of the document, is `workspace` (the default) or `bindings`. With
  `bindings`, every `argv[0]`, `arguments_from[0]` and `handoff_when[0]` that is relative and
  holds a `/`, such as `tools/lint.sh`, names a file relative to the directory of the bindings
  file, as an `open` executor's `bindings` does; a bare name such as `sh` is still found on `PATH`, and an absolute
  path runs as written. The host calls `resolve_programs(document, directory)` with that
  directory before it builds a port: it answers the document with each such entry replaced by the
  absolute real path of its file, and refuses `ProgramNotBesideBindings`, naming the action and
  the member (`executor.argv[0]`, `arguments_from[0]` or `handoff_when[0]`), an entry with a
  `..` segment, one that names no regular file, and one whose real path, links resolved, lies outside the directory. A
  port given a document with `program_root: bindings` that was not resolved starts nothing.
- `retry_when` maps an evidence kind of the protocol to the results that send the action back to
  work; a result is never empty. A retried `program` is given those records as
  `LOOM_RETRY_FEEDBACK` (below).
  `handoff` is the text a run shows when this writer has done what it may and a person continues.
- `handoff_when`, on a `program` binding beside its `handoff`, is the argv of a check, held to the
  rules of any bound program's argv. Before each step of the action, and when the case is otherwise
  stuck, `tree run` runs it through the run's runner with the whole workspace read-only (`target/`
  included), `PATH` and the `LOOM_*` variables alone, no retry feedback, empty standard input and
  the binding's `timeout_seconds`. Exit 0 offers the handoff and takes no step; any other exit lets
  the writer run in that step, or leaves a stuck case `Stuck`. A check that cannot be started
  (exit 126 or 127 included), is killed at its timeout or by a signal is never a skip: the run
  stops `HandoffCheckFailed`, naming the action, and the writer does not run. Under `local` a
  signal leaves no exit status; under Substrate it is exit 128 plus the signal (137 for SIGKILL, as
  an out-of-memory or cgroup kill ends it), so an exit from 129 to 192 is refused too, one the
  check gives itself included (`SIGNAL_EXITS`). Each evaluation is
  one `HandoffChecked` line of the case's run log, with the check's exit or refusal and the blob of
  its output. Without `handoff` it is refused `MissingMember`, on another executor
  `MemberNotForExecutor`.
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
list of strings, an `effect` entry whose `results` has a key other than `performed` (but a `person` executor's
named value other than `*`), a key named
twice, a tag, a merge key, text nested or expanded past the bounds above, and a tool name that is
empty or holds a comma or white space. Loading against the protocol refuses an action the protocol does not declare, a duplicate
action, an evidence `kind` outside the action's `may_produce`, a `subject`, `upstream` entry or
`artifacts` key that is not a declared artifact, a `tools` member, an agent budget member, `inputs`
or `outputs` on an executor that is not an `agent`, a `timeout_seconds` outside 1 to 3600, an
`artifacts`, `inputs`, `outputs` or `open` `bindings` path that is absolute or leaves the
workspace, a `retry_when` kind the protocol does not declare, an evidence `from` the executor does
not take (`MemberNotForExecutor`), a `from: runs` table that lists anything but `spent`
(`InvalidEvidence`, naming `evidence[<i>].results`), `exits` on an executor that is not a `program`, an exit code in both `exits` lists or in
one and as a key of a `from: exit` table (`ExitClassConflict`), and an `independence` dimension the
protocol does not declare. Decoding also refuses a `program_root` other than `workspace` or
`bindings` and an `exits` entry that is not a code from 0 to 255. Each refusal names the action and the
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
  `argv` (no shell), the arguments as one JSON object on standard input, `PATH`, `LOOM_CASE`,
  `LOOM_CASE_NAME` (a case a parent opened), `LOOM_PARENT_CASE_NAME` (a case whose parent a case
  opened) and `LOOM_TIMEOUT_SECONDS` alone in its environment, the binding's `timeout_seconds` (60
  when absent), and the workspace root as its start directory. Under a runner that is not `local`, a program at an absolute path
  outside `/usr`, `/bin`, `/lib` and `/lib64` is mounted read-only at its own path. A host `PATH`
  that is not UTF-8 refuses the step, `<program> was not started: the host's PATH is not UTF-8`,
  and starts nothing.
- `PATH` has the host process's value. `LOOM_CASE` is the id of the case the port was built for,
  as the governor holds it (a child case as `<parent id>.<name>`), so a program reads the case it
  runs for from it. The port alone sets it: no member of the request's arguments, of the
  `arguments_from` output, of a program's report or of the bindings document, and no variable of
  the host process, changes it; a document cannot name an environment. A case id longer than 4096
  bytes refuses the step, `<program> was not started: the case id is longer than 4096 bytes`, and
  nothing starts or is recorded. The commands of `b10x-loom run --action-bindings` keep `PATH`
  alone.
- `LOOM_TIMEOUT_SECONDS` is the whole seconds of that run's timeout, the binding's
  `timeout_seconds` or 60. `LOOM_CASE_NAME` is the name the case's opening gave it, as the
  governor stored it in the case's `parent` link: the `open` executor's `name` or the entry of its
  `each` argument, without the repeat suffix, so the child `P.story.2` reads `story` as `P.story`
  does. `LOOM_PARENT_CASE_NAME` is the name stored in the `parent` link of that parent, one level
  up and no further: in a tree `P` -> `P.epic` -> `P.epic.story` the programs of `P` get neither,
  those of `P.epic` get `LOOM_CASE_NAME=epic`, and those of `P.epic.story` get
  `LOOM_CASE_NAME=story` and `LOOM_PARENT_CASE_NAME=epic`. Like `LOOM_CASE`, the port alone sets
  each, after the child is built: no request, argument, report, program output, bindings document
  or host variable changes one. A stored name that is not one child name (ASCII letters, digits,
  `-` and `_`) of at most 4096 bytes refuses the step, `<program> was not started: the case name
  is not a child name` (or `the parent case name`), and a parent whose stored link cannot be read
  refuses it naming the case and the parent; nothing starts or is recorded.
- The arguments are the request's, else the binding's `arguments`, else the object the
  `arguments_from` argv prints, else `{}`. A request has no arguments when they are the empty object
  `{}`; any other value, `null` included, reaches the program as written. An `arguments_from` argv
  that does not exit 0, or prints anything but one JSON object of at most 1 MiB, refuses the step
  `ArgumentsUnreadable`, and the bound program does not start. The `arguments_from` program
  starts on every step that reaches it, leaves no observation and does not count against
  `budget.max_runs`, unless its cancel ends it (below).
- An `open` executor with `each` reads the names of its children from the same object: the
  request's arguments, else the binding's `arguments`, else the object its `arguments_from`
  program prints. That program runs as a `program` binding's does, with the same environment
  (never `LOOM_RETRY_FEEDBACK`), on the request's arguments in the workspace root, under the
  binding's `timeout_seconds`. `OpenEffects::new` takes the workspace, the store directory and the
  host's runner, which only a document with `arguments_from` needs, and refuses `arguments_from`
  on an action whose protocol declares no `arguments` schema (`ArgumentsUndeclared`). When the
  action declares a schema, the object is held to it with Canon's `check_arguments`
  (`ArgumentsMismatched`, naming the JSON pointer and the keyword). The member `each` names must be
  a list (`EachNotAList`) of strings (`EachItemNotText`) with at least one entry (`NoChildNamed`),
  each given once (`ChildNameRepeated`) and one plain segment of ASCII letters, digits, `-` and `_`
  (`ChildNameInvalid`); a refusal names the entry's index, never its text. A program that cannot
  give its object refuses the step `ArgumentsUnreadable`. A refused step opens no child and writes
  nothing.
- Once the names are checked, the program's output is kept as `blobs/<sha256>` in the store
  directory and the names as one `loom/open` observation (`loom.bindings.ChildNames`: the case,
  the action, the case revision, the source, the names in order and the blob digest); then the
  children open. A step cut before the governor's write takes that list again at the same case
  revision and does not run the program twice; the governor takes over each child already written.
  At another case revision the program runs again. The step's report holds `names`, `source` and
  `blob_sha256` beside `children` and `evidence`.
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
- An exit the binding's `exits` lists is read before the tables. One in `error` records nothing
  and refuses the step `ProgramError`, naming the exit code and the blob's SHA-256 and never what
  the program printed, and the refusal ends the run (`EffectRefused`). One in `not_called` records
  nothing and the step is performed. The run's observation holds `class` (`error` or
  `not_called`), and neither class counts against `budget.max_runs`. A run killed at its timeout
  reads `timeout`, whatever the lists hold. Each step starts the bound program once. Under
  `tree run` a writer whose run was not called leaves the due set, as a refused step does, until
  the case is evaluated again for a reason other than its own moves (a refused line or one that
  names no move and records nothing is no other reason): `tree run` starts it once per
  evaluation and goes on to the next useful action or the handoff check. Any other change ends
  the set-aside, so a not-called writer that moves an artifact and a writer of evidence on that
  artifact alternate, each one's line ending the other's wait, and only the driving loop's step
  budget bounds them (`max_steps`; in a suite cycle, its 60 s wall bound).
- A program a signal ended never answers through its tables. Under Substrate such a run reports
  exit 128 plus the signal (137 for SIGKILL, as an out-of-memory or cgroup kill ends it), so a
  run that ends with an exit from 129 to 192 (`SIGNAL_EXITS`) and was not killed at its timeout
  records nothing and refuses the step `ProgramSignalled: action <action>: …`, naming the exit,
  the signal and the blob's SHA-256 and never what the program printed; the refusal ends the run.
  The check comes before `exits` and the tables, so an `exits` list or a `from: exit` row keyed
  by a code from 129 to 192 is accepted at load and never read under Substrate, and the run holds
  no `class` and counts against `budget.max_runs`. Under `local` a signal leaves no exit status
  and the runner refuses the run, as above.
- A port built `with_cancel(cancel)` (`confinement::Cancel`) reads the handle after the run count:
  set then, the step is refused `Cancelled: action <action>: …`, nothing starts and nothing is
  observed. Every program the step starts, the `arguments_from` program included, holds it. A
  handle set after the port read it and before the runner starts the program starts nothing, but
  the runner refuses that run `Cancelled`, so it is kept and counted as a cancelled run below. Set
  while one runs, its runner kills it within 100 ms, with no grace: under `linux` with every
  process it started (its cgroup), a write of its arguments the program does not read dropped;
  under `local` with its process group, then waiting at most `KillBounds::kill_settle`, so
  a process that left the group (`setsid`, a daemon) is not killed and survives the cancel.
  `run_child` returns within 7 s. The step is then refused
  `Cancelled: action <action>: <program> was ended by its cancel: …`, and the refusal ends the run
  (`EffectRefused`). The run's observation is kept with `cancelled: true`, no `exit` and a
  `runner_refusal` naming `Cancelled`, and counts against `budget.max_runs`; the placed
  artifacts' moves are written alone; no table is read, no blob is kept and no record is written,
  so no retry record comes from the step. A handle set after the program ended changes nothing.
  `OpenEffects::with_cancel` gives its `arguments_from` program the handle the same way.
- Otherwise the port records one record per entry (`loom.bindings.RecordedEvidence`), of the
  entry's `kind` about its `subject`, with the entry's `producer`, the observation's instant, the
  revision the case holds for the subject once the step is written and, as `upstream_revisions`,
  for each `upstream` artifact. The record cites the observation. A program never names a result,
  a revision, a producer or an instant: members of its report with those names change nothing.

- An action whose `retry_when` is not empty is sent back to work by its retry records
  (`retry_records`): each record the case holds whose `kind` and `result` `retry_when` lists, about
  the revision the case holds for its subject, in the order the case received them. A record
  about a subject at `none`, a record of another case and a record of another kind or result are
  none. Each names the record's evidence id, kind, subject, subject revision, result, producer and
  instant and, read from the bound program's run the record cites in that case, the `action` that
  run was for, its `exit` code and the SHA-256 of its blob. A record that cites no such run names
  none of the three (`null`); a run killed at its timeout or refused by its runner has no `exit`.
- After the run count and before anything starts, a step with retry records keeps one
  `loom.retry-feedback/1` document, `loom.bindings.RetryFeedback`, as `blobs/<sha256>`: the case,
  the action, the retry records and, per cited blob that still hashes to its name, its last 60
  lines, at most 65536 bytes, invalid UTF-8 replaced by U+FFFD. The bound program then has one
  more variable, `LOOM_RETRY_FEEDBACK`, the canonical absolute path of that document, mounted
  read-only at that path under a runner that is not `local`. With no retry records nothing is
  written and the variable is not set; the `arguments_from` program is never given it, and no
  argument, report or bindings member sets it. A retried run is a run: at `budget.max_runs` the
  step is refused `RunsSpent` before any record is read. A document that cannot be kept (a
  `blobs` that is a link) fails the step before anything starts, with nothing observed. The run's
  observation names the records it was given in `retry_of`.

The feedback document has this shape (one record, ids and digests shortened here):

```json
{
  "format": "loom.retry-feedback/1",
  "case": "case-1",
  "action": "draft.write",
  "records": [
    {
      "evidence_id": "5f0c…",
      "kind": "draft_check",
      "subject": "draft",
      "subject_revision": "t3e1f…",
      "result": "invalid",
      "producer": "checker",
      "observed_at": "2026-11-21T22:14:03Z",
      "action": "draft.check",
      "exit": 1,
      "blob_sha256": "9b2a…"
    }
  ],
  "logs": [{"blob_sha256": "9b2a…", "tail": "the check failed\n"}]
}
```

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

## Copying a record from another case

A host builds `CopyEffects` per case, from the governor that holds the case, the document and the
host's protocol catalog. It performs the actions bound to `copy` and no other. Loom makes the copy:
no program runs.

- `from` is a registration of the catalog, or the protocol id of registrations in it. One that
  is neither is refused `ProtocolMismatch` at `executor.from` when the port is built.
- The source is a case of the receiving case's tree (`CanonGovernor::tree`), other than the
  receiving case, held under a registration `from` names. A case of another tree in the same store
  directory is never a source. With no such case the step is refused `NoSourceCase`.
- The port copies one record: of the binding's `evidence` kind, one that applies in its case,
  about the revision its case holds now of its subject. Of several, it takes the one with the latest
  `observed_at`. A record with none counts as the earliest. With no such record the step is refused
  `NoSourceRecord`. A record about an artifact the receiving protocol does not declare is refused
  `SubjectUndeclared`.
- The source and the kind come from the bindings file alone. The request's arguments select
  nothing.
- The copy keeps the record's kind, subject, subject revision, result, instant and producer. It
  keeps the `upstream_revisions` entries of artifacts the receiving protocol declares and drops the
  others. The receiving case judges the copy by its own `independence` map: a producer that map
  does not list satisfies no match that demands independence, whatever the source case's map says.
- The copy cites one observation of source `loom/copy` (`loom.bindings.CopiedRecord`). It names
  the receiving case, the action, the source case, the source record's evidence id and the dropped
  artifacts. Its instant is the source record's. A restart after that observation and before the
  record leaves the observation, and the next step uses it and writes one record.
- `copy_due(action)` says whether a step would write a copy. It is false once the receiving case
  holds the copy of the record the step would take. A step while it is false is performed and
  writes nothing. A refused step writes nothing.

The step refusals are the members of `loom.bindings.CopyRefusalReason` in the
[reference](../reference/ess/loom-bindings.md).
