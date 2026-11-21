---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:observation-collection-as-executors
relations:
- reviews: epic:observation-collection-as-executors
- reviews: story:dns-resolution-through-a-resolver-aperture
- reviews: story:observe-executor-reads-workspace-files
- reviews: story:gate-run-collects-a-gates-document
- reviews: story:gates-document-evaluated-in-the-governor
- reviews: story:gate-program-identity-bound-to-spawn
- reviews: story:observe-compares-generated-trees
- reviews: story:observe-reads-documents
- reviews: story:observe-git-changed
- reviews: story:report-formats-checked-before-the-table
- reviews: story:tcp-reachability-through-an-aperture
revision: 1
---
needs-revision

story:gate-program-identity-bound-to-spawn — admission is now one unit, but binding the admitted file to the spawn is described only for the program port's child: the read-only mount at `/opt/loom/programs/<n>/<name>`, `argv[0]` set to the mount path, a second digest after each run and `ProgramChanged`. Story 6 states it again for git and lists `admission.rs` as "called, not changed". Story 8 runs project commands through `ObserveLauncher` and never says it mounts and runs the admitted real file. So the unit should also build the launch of an `AdmittedProgram` and run its after-run digest, called by all three, and story 8 should carry the link-swap and link-name cases for a gate command — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:40-57, :238-239, .engineering/planning/story/observe-git-changed.md:45-49, :194-195, .engineering/planning/story/gate-run-collects-a-gates-document.md:34-38, :57-64, :225

story:gate-program-identity-bound-to-spawn — the unit's input is a program name and a search path only, yet its checks need the workspace: entries outside it, a real path inside it refused. The body names no workspace root as an input, and does not say which workspace the program port uses when it is built for a bindings document before any action runs — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:24-27, :33-37, :43-46

story:gate-run-collects-a-gates-document — `engineering.gates` needs the observe port's launcher (for `git cat-file` and the project commands). But story 6 gives a reader-only port the `NoLauncher` refusal only for a document that binds `git.changed`, and this body does not extend that refusal to `engineering.gates` or say where a gate run on a reader-only port is refused — .engineering/planning/story/observe-git-changed.md:37-40, .engineering/planning/story/gate-run-collects-a-gates-document.md:22-24, :34-38, :132-146

story:gate-run-collects-a-gates-document — `tests.run` in a gate run goes through story 5's `engineering-tests/1` check, which refuses an exit that contradicts the report. But `ExitContradictsReport` is declared only on `ProgramRunRefusal`; story 5 adds only `ReportFormatUnsupported` and `ReportInconsistent` to `ObserveRefusal`, and this story adds only `NoExitCode`. A gate command whose exit contradicts its report therefore has no declared refusal member, which breaks the epic's § Refusals rule — .engineering/planning/story/report-formats-checked-before-the-table.md:31-33, :81-87, .engineering/planning/story/gate-run-collects-a-gates-document.md:138-139, .engineering/planning/epic/observation-collection-as-executors.md:121-127

Round-1 findings, rechecked:
- Admission at port build only: partly fixed. `admission::admit` and its three callers are now in story 2 (`:21-49`); the spawn half is still open, as the first finding above.
- Observe as a launcher: fixed. Story 1 `:42-46` and story 6 `:25-43` add `ObserveLauncher`, `NoLauncher` and the counting launcher. The gap that remains is `engineering.gates`, the third finding.
- TCP gate-run half: fixed. Story 9 `:13`, `:32-43`, `:68-72` and Files `:132-135`. Story 10 does the same for DNS (`:31-41`, Files `:117-121`).
- `WorkspaceReader` crate cycle: fixed. Story 1 `:51-60`; story 4 `:30-32` follows it.

What I read: the epic, the 10 stories and the 2 upstream blockers (whole bodies, read from the store files the CLI's graph reports), plus `e7r1-changes.md`, my round-1 verdict and the scope of `story:bound-program-result-table`. Commands run: `aep plan artifact relations`, `aep plan artifact graph --format json` and `aep plan artifact validate`, which printed `valid` with only old review-outcome reminders outside the set. I walked 69 `depends_on` edges over 39 nodes, outside the set too. There is no cycle, and the ten stories form one chain, 1 → 2 → … → 10. I traced 21 acceptance reads. 20 have a producer in the set or upstream, and an edge (direct or through the chain) records each of them. The 21st is the gate-run `ExitContradictsReport` refusal, which no story declares (the fourth finding). I checked `run_child` (`confinement.rs:475`) and `with_read_only_file` (`:1157`): both are `pub`.

What I could not establish:
- **The chain of all ten.** The set is a queue. Every edge has a shared-file reason written beside it (`generated/rust/loom/`, `CHANGELOG.md`, the ESS files), so it is not a finding. The trade-off is unchanged: keep the order, or split those shared files. The reviser declined the closed-list split (`e7r1-changes.md:160-163`).
- **The crate direction.** The epic and story 1 rest the `loom-bindings` → `loom-intake-slice`/`loom-governor` direction on `bound-program-result-table.md:113`. That line is marked "Inferred: the story names no manifest". This is a dependency outside the set, so not mine to call, but the premise is not stated as a fact anywhere.
- **Outside my lane (scope):** story 10 checks a resolver against "a resolver the engine declares for the action" (`:27-29`, `:99`). No story in the set or upstream adds a resolver declaration to the engine configuration; `story:named-egress-per-action` declares apertures only.
- **Outside my lane (acceptance):** story 10 says load-time refusals use `BindingsRefusalReason` (`:43-44`), but declares none and has no load bullet; story 9 has `ApertureNotNamed` at load. Story 1's no-process test speaks of "a counting runner" (`:209-212`) on a port that has no runner member in that story.
- **Governor placement:** still § Open 7, for session `canon`. No source contradicts the default.

```findings
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 51
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "admission is now one unit, but binding the admitted file to the spawn (read-only mount at /opt/loom/programs/<n>/<name>, argv[0] the mount path, digest again after each run, ProgramChanged) is stated only for the program port's child; story 6 restates it for git with admission.rs called, not changed, and story 8 runs project commands through ObserveLauncher without saying the admitted real file is mounted and run, so the unit needs to also give the launch of an AdmittedProgram and its after-run digest, called by all three, with story 8 carrying the link-swap and link-name cases for a gate command"
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 24
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the unit's input is a program and a search path only, yet its checks (entries outside the workspace, a real path inside it refused) need the workspace root, and the body names neither that input nor which workspace the program port uses when it is built for a bindings document before any action runs"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 34
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "engineering.gates needs the observe port's launcher (git cat-file, the project commands), but story 6 makes a reader-only port refuse NoLauncher only for a document binding git.changed, and this body neither extends that refusal to engineering.gates nor says where a gate run on a reader-only port is refused"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 138
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "tests.run in a gate run applies story 5's engineering-tests/1 check, which refuses an exit that contradicts the report, but ExitContradictsReport is declared only on ProgramRunRefusal; ObserveRefusal gains only ReportFormatUnsupported and ReportInconsistent (story 5) and NoExitCode (here), so a contradicting gate command has no declared refusal member, against the epic's Refusals rule"
```
