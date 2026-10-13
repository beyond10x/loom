---
format: aep.planning-md/3
id: decision-blocker:cli-merge-effect
kind: decision-blocker
status: cleared
title: Nobody has decided what b10x-loom resume performs for a granted repository.merge
relations:
- blocks: story:cli-resume-after-grant
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-13T07:40:52Z", actor: "human:timo", revision: 3}
---
## Question

What does `b10x-loom resume` perform when the suspended request is `repository.merge`? No effect
binding performs it: `LocalEffects::performs` covers only `repository.inspect`, `repository.edit`
and `tests.run` (`crates/loom-intake-slice/src/effect.rs:202-205`), the slice executor refuses the
merge (`crates/loom-intake-slice/src/executor.rs:3-4`), and `resume_after_grant` then refuses
`NotPerformed` (`crates/loom-commission/src/runtime.rs:1168`). The slice commits each edit onto the
workspace's current branch, so there is no work branch to merge. `story:cli-resume-after-grant`
asks that resume "performs the suspended request exactly once".

## Options

- A: a merge hand-off. The slice binds `repository.merge` to an effect that writes a merge record
  into the state directory (commission, the granted request, the workspace and its `HEAD` as the
  implementation revision) and touches no Git ref. Performed names that record; a person or a
  later tool merges outside Loom. Cost: M, inside U3; no change to where edits land.
- B: a local Git merge. `run` creates a work branch, edits commit there, and the merge effect
  fast-forwards the base branch to it after the grant, through the recorded host Git. Cost: L,
  changes every software-change run (a new branch per run), new confinement surface; its own story.
- C: U3 delivers `approve` and `resume` with no merge binding; a resume of a merge ends
  `NotPerformed` with its own exit code, and a binding comes later. Cost: S; the acceptance's
  "performs once" is dropped.

## Recommendation

A: it keeps a consequential action leaving Loom as a record, needs no change to the run's Git
model, and meets the acceptance.

## Decision (2026-10-13)

A: resume of a granted `repository.merge` performs a merge hand-off, an effect that writes a merge
record (commission, the granted request, the workspace and its `HEAD`) into the state directory.
No Git ref moves; merging happens outside Loom. Inside `story:cli-resume-after-grant`, spec first:
the merge record is declared in the specification before code, and "performs the suspended request
exactly once" holds for the record. Option B (a work branch fast-forwarded after the grant) is not
in this wave.
