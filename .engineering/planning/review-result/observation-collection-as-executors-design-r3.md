---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-design-r3
kind: review-result
status: active
title: Plan critic design, confirming pass (round 3), on epic:observation-collection-as-executors
relations:
- reviews: epic:observation-collection-as-executors
- reviews: story:observe-executor-reads-workspace-files
- reviews: story:gate-program-identity-bound-to-spawn
- reviews: story:observe-reads-documents
- reviews: story:observe-compares-generated-trees
- reviews: story:report-formats-checked-before-the-table
- reviews: story:observe-git-changed
- reviews: story:gates-document-evaluated-in-the-governor
- reviews: story:gate-run-collects-a-gates-document
- reviews: story:tcp-reachability-through-an-aperture
- reviews: story:dns-resolution-through-a-resolver-aperture
revision: 1
---
approve

No findings. All four round-2 design findings are fixed in the stored bodies, and e7r2 broke nothing in the shape of the set.

Round-2 findings, rechecked:
- **Blocker, the spawn binding was only the program port's: fixed.**
  - Story 2 now makes `admission.rs` provide `admit`, `launch` and `verify` together for every caller (`.engineering/planning/story/gate-program-identity-bound-to-spawn.md:38-54`).
  - Story 6 calls all three for git (`observe-git-changed.md:74-79`), and one bullet holds each: `git_not_on_the_search_path_refuses_the_port`, `git_runs_the_admitted_file` and `a_git_rewritten_after_admission_refuses_the_step` (`:205-209`).
  - Story 8 calls all three for each project command (`gate-run-collects-a-gates-document.md:102-113`). `admit` is held by the preflight bullets (`:288-291`, `:295-300`). `launch` is held by `a_gate_command_gets_only_its_program` (`:307`). `verify` is held by the `ProgramChanged` bullet (`:314`). The link-swap case is held at `:301-304` and the link-name case at `:305-306`.
- **The workspace root as an input of `admit`: fixed.** It is an input (story 2 `:42-45`). The program port is built per case with that case's root (`:72-73`). Story 6 passes the case's root for git (`:76`).
- **`engineering.gates` on a reader-only port: fixed.** It is refused `NoLauncher` at port build (story 8 `:48-50`), with the bullet `a_reader_only_port_refuses_a_gate_run` (`:256-257`).
- **`ExitContradictsReport` for a gate command: fixed.** Story 8 declares it on `ObserveRefusal` (`:195`) and has a bullet for it (`:330-331`).

Points the brief put under review, checked in my lane:
- **1 MiB per-file bound.** Substrate gives it: `HostConfig::minimum` at `confinement.rs:229`, `MAX_FILE_BYTES` = 1,048,576 (substrate-wire `lib.rs:21`), refused `workspace.file-limit` (substrate-guarded `fs.rs:523-528`). `read_digested` goes through that path (`fs.rs:338`). Every bullet uses a file of at most 1 MiB or exactly 1 MiB + 1 byte, and `FileOverBound` is declared in story 1 (`:191`).
- **Cases Substrate's codes cannot tell apart.** A link, a FIFO and a wrong kind all answer `workspace.path-escape`. Stories 1 and 4 state where two cases get one refusal (story 1 `:124-132`, story 4 `:61-65`), and they tell the rest apart by the parent's page, which matches `fs.rs:843-870`.
- **Story 7 against Canon `0.9.0`.** The plan id and the request order come from `build_plan` (`compiler.rs:725-740`, `:795-802`). The empty-identity check is `validate_bindings` (`:696-697`). Canon's `Plan` and `EvalContext` can be serialized, so `GateBundle.plan` as JSON can be built.
- **Stories 9 and 10.** Story 10's bullets rest on a resolver declaration that story 10 itself is to write when it is rewritten (`dns-resolution-through-a-resolver-aperture.md:55-61`). That is § Open 20, and no source contradicts its default.

What I read: the epic, the 10 stories and the 2 upstream blockers from the store, plus `e7r2-changes.md`, both earlier briefs and my round-2 verdict. Commands: `aep plan artifact relations`, `aep plan artifact graph --format json` and `aep plan artifact validate`, which printed `valid` with only old review-outcome reminders outside the set. I walked 25 ordering edges out of the set's nodes: 23 `depends_on` and 2 `blocks`. There is no new edge since round 2 (the ledger's § Edges says none), so the chain 1 → … → 10 and round 2's no-cycle walk outside the set still hold; I did not walk outside the set again. I traced 14 acceptance reads that are new or changed in e7r2. Each has a producer in the set or upstream, and an edge records each, directly or through the chain.

What I could not establish:
- **Outside my lane (scope):** story 8's `## Scope` section is stale. It still lists `NoExitCode` and leaves out `GatesRefused`, `ExitContradictsReport` and `ObserveRun.gates_refusal` (`gate-run-collects-a-gates-document.md:418` against `:193-198`).
- **Outside my lane (acceptance):** `ProgramMountsExceeded` is placed only at the program port's build (story 2 `:87-90`). A git or gate launch over 16 read-only files would hit the runner's `ScopeInvalid` instead and be refused `LaunchRefused`, a declared member, but no bullet covers that case.
- **Outside my lane (acceptance):** story 6 admits git whenever the observe port is built with a launcher (`:74-77`). So a document binding only file operations, on a host without git, is refused `GitNotAdmitted` at port build. The body neither says this nor limits admission to documents that bind `git.changed` or `engineering.gates`.
- **Governor placement:** still § Open 7, for session `canon`. No source contradicts the default.

```findings
[]
```
