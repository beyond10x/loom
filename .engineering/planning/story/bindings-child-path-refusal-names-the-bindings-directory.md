---
format: aep.planning-md/3
id: story:bindings-child-path-refusal-names-the-bindings-directory
kind: story
status: draft
title: A child bindings path refused for leaving the bindings file's directory says so in its detail
summary: The check is right; PathNotInWorkspace says 'relative to the workspace' for a rule about the bindings file's directory
tags:
- from-finding
- sdc
relations:
- depends_on: story:bindings-document
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
revision: 1
---
## Outcome

A child `executor.bindings` path refused for leaving the bindings file's directory is refused with
a detail that names that rule.

## Found

Found 2026-11-21 while revising `story:child-case-runtime`, after wave 2026-11-21-w1 closed; the
wording came in with `story:bindings-document` (`2fe0743f`).

- The rule, as the doc comment at `crates/loom-bindings/src/load.rs:20-22` states it and as the
  requester (session `sdc`) accepted it: `executor.bindings` is read relative to the bindings
  file's directory, relative only, no `..`.
- A path with `..` is refused `PathNotInWorkspace` through `in_workspace`
  (`crates/loom-bindings/src/load.rs:203`, `:286-294`) with the detail "a path is relative to the
  workspace".
- The check is right; the refusal's name and detail describe a different rule than the one
  enforced for this member.

## Acceptance

- A test in `crates/loom-bindings/tests/` loads a document whose child `executor.bindings` holds
  `..` and asserts the refusal's detail names the bindings file's directory.
- Every existing case of `crates/loom-bindings/tests/` passes with no assertion on a refusal
  variant changed, or the story names each changed assertion and the `**Breaking:**` CHANGELOG
  bullet for a renamed or added variant in `ess/domains/bindings.yaml`.

## Out of scope

- The path rule itself.
- Paths of other members that are held to the workspace.
