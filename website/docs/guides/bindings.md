---
title: Bind a protocol's actions
sidebar_position: 6
description: Write a loom.bindings/1 document that says, per action of a protocol, who performs it, how its result becomes evidence, and what it may spend; Loom refuses a file that does not fit the protocol when it loads it.
lede: A bindings file is the one place an operator declares how each action of one protocol is performed. Loom decodes it, holds it against the compiled protocol and names the action and the member of every defect.
source: crates/loom-bindings/src/lib.rs, crates/loom-bindings/src/decode.rs, crates/loom-bindings/src/load.rs, crates/loom-bindings/tests/bindings_document.rs, ess/domains/bindings.yaml
---

# Bind a protocol's actions

Unreleased. `b10x-loom-bindings` decodes and checks a `loom.bindings/1` document. It executes
nothing, starts no process and opens no network connection; the commands that run a bound action
take the checked document in later releases, and `b10x-loom run --action-bindings` still reads the
older `intake.bindings` format.

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
  argument and the child's `bindings` file. A `copy` names the evidence `kind` it copies and the
  protocol id of the case it copies from.
- `arguments` is a literal object; `arguments_from` is an argv whose standard output is the
  object. They are alternatives.
- Each `evidence` entry says what the action records: the `kind` (one the action `may_produce`),
  the `subject` and `upstream` artifacts, the `producer`, and where the value comes from. `from:
  exit` reads the program's exit code; `from: effect` is Loom's own report that it performed the
  action, whose one value is `performed`, and an effect that was refused or failed records nothing;
  a JSON pointer such as `/summary/state` reads the program's report. `results` maps each value,
  or `*` for any other, to the evidence result; `null` records no result.
- `tools.allow` lists the tool names an `agent` may be offered. An entry is never empty and holds
  no comma and no white space, because the session executor joins the names with commas.
- `budget` holds `max_runs` (8 when absent) and, for an `agent`, `max_turns`, `max_tokens` and
  `max_budget_micro_usd`, each a whole number from 1. `timeout_seconds` is from 1 to 3600.
- `retry_when` maps an evidence kind to the results that send the action back to work.
  `handoff` is the text a run shows when this writer has done what it may and a person continues.
- `artifacts` maps an artifact id of the protocol to a path in the workspace; `independence` maps
  a producer to dimensions the protocol declares.

## What Loom refuses

Decoding refuses an undeclared member at any level, a `format` that is missing or is not
`loom.bindings/1`, an empty `argv` or one that is not a list of strings, an `effect` entry whose
`results` has a key other than `performed`, and a tool name that is empty or holds a comma or white
space. Loading against the protocol refuses an action the protocol does not declare, a duplicate
action, an evidence `kind` outside the action's `may_produce`, a `subject`, `upstream` entry or
`artifacts` key that is not a declared artifact, a `tools` member, an agent budget member, `inputs`
or `outputs` on an executor that is not an `agent`, a `timeout_seconds` outside 1 to 3600, an
`artifacts`, `inputs` or `outputs` path that is absolute or leaves the workspace, and an
`independence` dimension the protocol does not declare. Each refusal names the action and the
member, such as `action draft.write, member tools.allow[1]`. The reasons are the members of
`loom.bindings.BindingsRefusalReason` in the [reference](../reference/ess/loom-bindings.md).
