---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:observation-collection-as-executors
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

All 12 round-1 acceptance findings are fixed in the stored bodies: epic :200-203; story 7 :120-127, :130-133, :139-145 and :154-158; story 5's ESS first section and its bullets; story 2 :113-119 together with the epic's § Refusals; story 1 :172-174 and :194-197; story 8 :186-225 and :247-257; story 10 :92 and :101-103. The findings below are new. Paths are relative to `~/.local/state/worktree/trees/b10x/loom/loom-sdc-plan`.

story:gate-run-collects-a-gates-document — the acceptance has no case where a project command's link is retargeted between admission at preflight and launch. The delegated case checks only network and environment: it does not check that the child gets the admitted file as its one read-only file, or what it may write. The Outcome never says the command runs the admitted real file mounted read-only. So the guarantee the taken-over story is named for (story 2 :153-158, :200-202) is tested only for the program port and never for gate commands — .engineering/planning/story/gate-run-collects-a-gates-document.md:242-243

story:gate-run-collects-a-gates-document — the "Refused X" preamble requires that "a marker file the first command would write does not exist". That cannot hold for refusals raised after commands have run: `SourceChanged` (:223-224, :261-262) and `ProgramChanged` (:225). This is the same preamble-versus-bullet clash as round 1's story 1 finding — .engineering/planning/story/gate-run-collects-a-gates-document.md:161-163

story:gate-run-collects-a-gates-document — a gates document that the governor's `plan` refuses (`GatesRefusal` `UnsupportedFormat`, `CompileFailed` and the others, story 7 :91-92) has no `ObserveRefusal` member and no bullet. No test can say how that refusal is recorded — .engineering/planning/story/gate-run-collects-a-gates-document.md:138-139

story:observe-executor-reads-workspace-files — only a literal path that is absolute or holds `..` is refused, and only at load. No bullet covers such a path arriving at the step from a request's arguments (a gates document, story 8). Nothing shows that case is refused rather than read as `false`. Story 3 :90-92 already shows the pattern for `pointer` — .engineering/planning/story/observe-executor-reads-workspace-files.md:196-197

story:observe-executor-reads-workspace-files — the 3 MiB bullet passes even for a reader that stops after its first slice. Substrate's first `DigestedFileSlice` already names the whole file's `size` and `sha256` (substrate 0.21.0 `crates/substrate-wire/src/lib.rs:1302-1314`). The bullet needs something that only a full read shows, such as a `text.contains` needle after the first 1 MiB — .engineering/planning/story/observe-executor-reads-workspace-files.md:222-224

story:gate-program-identity-bound-to-spawn — the Outcome's rule for a relative name with a `/` (it runs as written from `/workspace` and `ProgramRun.program` is absent, :45-46, :113-114) has no acceptance bullet. The epic promises SDC readers exactly that behaviour (epic :176) — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:131-218

story:gates-document-evaluated-in-the-governor — `GatesRefusal` `CatalogMismatch` is declared, but no acceptance bullet reaches it — .engineering/planning/story/gates-document-evaluated-in-the-governor.md:92

epic:observation-collection-as-executors — § Refusals says every step refusal is a run record with a declared member, and that every bullet names one. But `ResultNotInTable` and `ReportUnreadable` are step refusals with no member (bound-program-result-table.md:63-64), and these bullets read them: story 3 :86-87, story 5 :134-135 and :142-144, story 8 :263. Either the rule names these table refusals or the bullets say which member they read — .engineering/planning/epic/observation-collection-as-executors.md:121-129

story:dns-resolution-through-a-resolver-aperture — two Outcome rules have no bullet: an unresolved CNAME chain is `Unavailable` (:22-23), and a declared resolver may be named by its exact `IP:port` (:28). The bullet at :93 covers only timeout, SERVFAIL, referral and truncated replies — .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md:93

**What I read:** all 13 artifacts in full (the epic, 10 stories, 2 upstream blockers), from `.engineering/planning/` and `aep plan artifact lifecycle upstream-blocker`. Also the round-1 acceptance verdict, `e7r1-changes.md`, both briefs, `bound-program-result-table.md:55-94` and `crates/loom-intake-slice/tests/host_git_hardening.rs:444-505`. I grepped the tree for `TimeoutOutOfRange`, `MemberNotForExecutor`, `UndeclaredMember`, `PathNotInWorkspace`, `every_refusal_reason_is_reachable_from_a_document`, `TestExecution`, `try_observations` and `with_read_only_file`; all exist. I read `crates/loom-bindings/src/load.rs:20-150` and substrate 0.21.0 `DigestedFileSlice`.

**What I could not establish:**
- Whether story 8 builds Canon's observation or plan types in `loom-bindings`. If it names `canon_expr` there, story 7's `canon_expr_is_the_governors_alone` fails once story 8 lands; a re-export through `loom_governor::gates` would avoid that. Story 7 :92-93 says only "Canon's plan and observation types stay Canon's".
- Out of my lane (design), not in the verdict: story 2's `admission::admit` takes only a program and a search path (:24-27), yet it must refuse entries and real paths inside the workspace (:33-37). The workspace root is not among its inputs.
- Out of my lane (design), not in the verdict: `every_host_git_command_goes_through_a_hardened_helper` counts a `Command::new(` only when its argument text contains "git" (`host_git_hardening.rs:469-475`). Story 6's call that starts the admitted git counts as "exactly one place" only if that argument's text names git.
- I did not open the takt citations or the engineering-protocols test files named in story 8, and no acceptance bullet depends on them.

```findings
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 242
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has no case where a project command's link is retargeted between preflight admission and launch, and the delegated case checks only network and environment, not that the child gets the admitted file as its one read-only file or what it may write, so the bound-to-spawn guarantee is tested for the program port only"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 161
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Refused-X preamble requires that the first command's marker file does not exist, which cannot hold for SourceChanged and ProgramChanged, raised after commands have run (:223-225, :261-262)"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 138
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "a gates document the governor's plan refuses (GatesRefusal UnsupportedFormat, CompileFailed and the others) has no ObserveRefusal member and no bullet, so no test can say how that refusal is recorded"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 196
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "only a literal absolute or dot-dot path is refused, and only at load; no bullet covers such a path reaching the step from a request's arguments (a gates document), so nothing shows it is refused rather than read as false"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 222
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the 3 MiB bullet passes for a reader that stops after its first slice, because Substrate's first DigestedFileSlice already names the whole file's size and sha256; it needs something only a full read shows, such as a text.contains needle after the first 1 MiB"
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 131
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome's rule that a relative name with a slash runs as written from /workspace with ProgramRun.program absent (:45-46, :113-114) has no acceptance bullet, though the epic promises SDC readers exactly that"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 92
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "GatesRefusal CatalogMismatch is declared but no acceptance bullet reaches it"
- file: .engineering/planning/epic/observation-collection-as-executors.md
  line: 121
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Refusals rule says every step refusal is a run record with a declared member, but ResultNotInTable and ReportUnreadable have no member, and bullets in stories 3, 5 and 8 read them"
- file: .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md
  line: 93
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "two Outcome rules have no bullet: an unresolved CNAME chain is Unavailable (:22-23), and a declared resolver may be named by its exact IP:port (:28)"
```
