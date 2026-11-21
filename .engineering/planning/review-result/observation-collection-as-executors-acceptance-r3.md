---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-acceptance-r3
kind: review-result
status: active
title: Plan critic acceptance, confirming pass (round 3), on epic:observation-collection-as-executors
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
needs-revision

All 9 round-2 acceptance findings hold in the stored bodies:
- **Story 8:** the gate-command bullets are at :301-311, the before/after preamble at :223-226, and `GatesRefused` at :268-271.
- **Story 1:** the step-path refusal is at :268-270 and the 1 MiB needle bullet at :240-242.
- **Other stories:** story 2 :197-199, story 7 :181-182, epic § Refusals :142-146, story 10 :102 and :110.

The admission unit is held in every caller:
- **Program port:** story 2 :180-189 and :208-213.
- **git:** story 6 :205-209.
- **Gate commands:** story 8 :288-315, including the link-swap and link-name cases.

Story 7 matches Canon 0.9.0: request order, `validate_bindings`, the `eval.rs` reasons, `evidence-time` and the identity check, all read at the tag. The catalog is JSON, with 26 functions and the stated sha256. Story 8's bundle, the narrowed source guarantee and the fake clock each have bullets. Stories 9 and 10 put the cases loopback cannot produce on a test double or on the blocker's surface. Substrate accepts a file of exactly 1 MiB and refuses anything larger (`size > max_file_bytes`), so the 1 MiB needle bullet is reachable. The three findings below are new.

story:observe-executor-reads-workspace-files — the bullet expects `code` `workspace.path-escape` from all three operations on a link as the last component, but `fs.file.exists` and `fs.dir.exists` only read the parent's page (Outcome :124-126). No Substrate call returns a code in that case, which the preamble's "Substrate's code where it gave one" (:249) admits. The bullet has to drop the code for those two operations — .engineering/planning/story/observe-executor-reads-workspace-files.md:252-254

story:observe-executor-reads-workspace-files — every classification reads "a directory page of the parent" (:120). Substrate 0.21.0 returns at most 1,000 items per page and a `next_cursor` (`substrate-wire/src/lib.rs:23` `MAX_LIST_ITEMS`, `:1349`). No bullet puts the name past the first page, so a reader that stops after one page passes every bullet and gives `false` for an existing file — .engineering/planning/story/observe-executor-reads-workspace-files.md:120-132

story:gate-run-collects-a-gates-document — neither the Outcome nor any bullet says how a file or document collector's step refusal in a gate request is recorded (`PathRefused`, `FileOverBound`, `NotRegular`, `ChangedDuringRead`, `DocumentMalformed`). Examples: a link or a file over 1 MiB read by `text.contains`, or an absolute path in a request. So nothing shows such a refusal refuses the run rather than becoming an `Unavailable` observation and an `unknown` gate. The epic promises "a gate input over 1 MiB is refused" (epic :108-109), and story 1 :268-270 points at story 8 as where such a path arrives from — .engineering/planning/story/gate-run-collects-a-gates-document.md:72-79

**What I read:** all 11 artifacts in full from `.engineering/planning/` (the epic and 10 stories), plus the round-2 acceptance verdict, `e7r2-changes.md` and the three briefs. I diffed the acceptance sections against the e7r1 drafts. I read Substrate 0.21.0 `substrate-guarded/src/fs.rs` (open flags, `read_complete_from`, `list_directory`, `classify_open_error`), `HostConfig::minimum` and the wire constants, and `DirectoryPage`. I read Canon 0.9.0 `eval.rs:30-120`, `compiler.rs:690-800` and `model.rs:395-404`. I checked the catalog at engineering-protocols 0.16.0 (sha256, JSON, 26 functions), Loom `confinement.rs:220-240` and the reasons in `ess/domains/bindings.yaml:203-231`.

**What I could not establish:**
- Story 1's 300 s observe timeout bullet (:308-309) against `MAX_TIMEOUT_SECONDS` 3600 (`crates/loom-bindings/src/lib.rs:34`): the Outcome states no per-executor bound. e7r2 did not touch it, so I did not raise it.
- Out of my lane (scope/design), not in the verdict: the epic's story list still describes admission as "a name or an absolute path, and a search path", without the workspace root (epic :157-160). The scoper's symbol line in story 8 still names `NoExitCode` (:418).
- I opened no takt or engineering-protocols test files; no new bullet depends on them.

```findings
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 252
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the link-as-last-component bullet expects code workspace.path-escape from all three operations, but fs.file.exists and fs.dir.exists only read the parent's page (:124-126), so no Substrate call gives that code and the bullet asserts a value no probe returns"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 120
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the classification reads one directory page of the parent, while Substrate pages hold at most 1,000 items with a next_cursor (substrate-wire lib.rs:23, :1349), and no bullet puts the name past the first page, so a one-page reader passes every bullet and gives false for an existing file"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 72
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "neither the Outcome nor any bullet says how a file or document collector's step refusal in a gate request (PathRefused, FileOverBound, NotRegular, ChangedDuringRead, DocumentMalformed) is recorded, so nothing shows a link, an over-1 MiB input or an absolute request path refuses the run rather than becoming an Unavailable observation and an unknown gate (epic :108-109, story 1 :268-270)"
```
