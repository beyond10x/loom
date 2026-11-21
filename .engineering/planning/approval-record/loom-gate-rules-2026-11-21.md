---
format: aep.planning-md/3
id: approval-record:loom-gate-rules-2026-11-21
kind: approval-record
status: approved
title: 'Three gate stories ordered by the folder rules of 2026-11-21: the test budget, planning-only pull requests, the local gate equal to the forge'
relations:
- decides: story:test-step-within-budget
- decides: story:planning-only-pull-requests-skip-build
- decides: story:local-gate-equals-forge
- decides: story:one-test-binary-per-crate
revision: 5
transitions:
- {from: "draft", to: "approved", at: "2026-11-21T22:00:13Z", actor: "agent:host-operator", revision: 3, executor: "agent:loom", correlation: "loom-gate-rules-2026-11-21"}
---
## What is ordered

The folder rules file `~/beyond10x/AGENTS.md`, which binds every repository under that folder,
holds three sections that each order a change of the repository's gate as a story ahead of the
next feature. It records the operator's words:

- § Tests stay fast, 2026-11-21: "I don't want any test to run 53 minutes. So these tests must
  run like in a minute or two." And: "this seems to be a mistake you are making ALL the time - FIX
  THIS". The section: "A step over the budget is a defect. It gets a story and is fixed before the
  next feature."
- §§ Planning-only pull requests skip the tests, and The local gate equals the forge: proposals 3
  and 4 of five that the session `host-operator` put to the operator, approved by him on
  2026-11-21 22:42 CET with "ROLL the 5". The first of the two sections: "This is built as a story
  ahead of the next feature, like a test-budget defect."

Session `host-operator` sent that order to session `loom` the same evening: "Confirm in one line
with the story ids for 2-4."

## The three stories

- `story:test-step-within-budget`: the unit on branch `impl/w7-test-budget`, begun as a
  correction of wave 2026-11-21-w7 after its test step ran 54 minutes and was stopped.
- `story:local-gate-equals-forge` and `story:planning-only-pull-requests-skip-build`: one
  implementation agent in one worktree forked from the branch that holds the first, in that
  order, because all three change `.github/workflows/check.yml`.

They merge into the integration branch `wave/2026-11-21-w7` and are read by the review of the
batch they merge with (`approval-record:loom-wave-2026-11-21-w8`, "How the batches run").

Design choices taken by session `loom` where the drafts asked: `pages.yml` keeps running on a
planning-only pull request; a test of the `slow` lane may run longer than 60 s, because that lane
is outside the test step; the workflows' `ess` version is held equal to the pin in
`ess/ess-inputs.yaml`.

## Acceptance

Session `host-operator`, 2026-11-21, to session `loom`: "A. I accept all three:
story:test-step-within-budget, story:planning-only-pull-requests-skip-build,
story:local-gate-equals-forge. Actor `agent:host-operator`. The page cites Timo's "ROLL the 5"
(2026-11-21 22:42 CET, typed in host-operator's terminal) and this message as my command. Your
build order stands."

## A fourth story, added the same evening

`story:one-test-binary-per-crate`, from the same section's rule "Test binaries are few: one
integration-test binary per crate (`tests/main.rs` with modules)". The first whole-workspace
build of the new test step linked 516 test executables, 33 GB under `target/debug/deps`.

Session `host-operator`, 2026-11-21, to session `loom`: "A. I accept
`story:one-test-binary-per-crate` (actor `agent:host-operator`, page
`approval-record:loom-gate-rules-2026-11-21`): one unit, one commit per crate, from the merged
head of batch w8, merged before wave 2 opens; target `main`; the two ess-gate replica tests run
`--test main ess_gate::`, to `slow` if over 60 s." Its condition: the units of batch w8 keep
building and merging beside it; only the next batch waits.
