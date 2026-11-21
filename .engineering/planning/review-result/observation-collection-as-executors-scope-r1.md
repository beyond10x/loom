---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:observation-collection-as-executors
relations:
- reviews: epic:observation-collection-as-executors
- reviews: story:dns-resolution-through-a-resolver-aperture
- reviews: story:gate-program-identity-bound-to-spawn
- reviews: story:gate-run-collects-a-gates-document
- reviews: story:gates-document-evaluated-in-the-governor
- reviews: story:observe-compares-generated-trees
- reviews: story:observe-executor-reads-workspace-files
- reviews: story:observe-git-changed
- reviews: story:observe-reads-documents
- reviews: story:report-formats-checked-before-the-table
- reviews: story:tcp-reachability-through-an-aperture
revision: 1
---
needs-revision

1. `epic:observation-collection-as-executors` — "Processes (`process.run`, `process.succeeded`, `ess.run`, `tests.run`): the existing `program` executor through the confined launch" is promised, but no item claims the observation value of `process.run`, `process.succeeded` or `ess.run` — `.engineering/planning/epic/observation-collection-as-executors.md:28`. Story 8 is the natural owner, and it only says "run as a bound program" (`story/gate-run-collects-a-gates-document.md:31-33`). Upstream these return `{exit_code (int or null), stdout, stderr}`, and `process.succeeded` is a boolean that is Unavailable when no exit code exists (`assertion-providers/src/lib.rs:182-191` at `0.16.0`; README table). The program port keeps stdout and stderr as one blob and reads `exit` or a pointer (`story:bound-program-result-table` Outcome). The catalog types the result (`process.run`, `ess.run` return a record; `process.succeeded` a boolean). The set has no mapping from a program run to that record, no signal-or-timeout-to-Unavailable rule and no acceptance for it.

2. `story:report-formats-checked-before-the-table` — the story says `ess.run` takes a program `report_format`, but `ess.run` has no report format upstream — `story/report-formats-checked-before-the-table.md:28-30`, `:7`.
   - Upstream `ess.run` is plain `process::collect` (`lib.rs:182`). Only `tests.run` reads a program's report (`reports.rs:34`, format fixed to `engineering-tests/1` at `:51-57`).
   - The epic contradicts itself: line 109 lists `(tests.run, ess.run)`, while line 147 says `report_format` for `tests.run` only.
   - The `ess.run` label should go from the story and epic line 109, and `ess.run` should follow finding 1's process value.
   - Offering the other two formats on a program stays, as the SDC-readers paragraph (epic line 133) intends.

3. `story:gate-run-collects-a-gates-document` — the `engineering.gates` run narrows the repository's gate commands without saying so — `story/gate-run-collects-a-gates-document.md:31-35`. Three upstream behaviours from the implemented story `gate-commands-name-a-program-portably` are neither in the acceptance nor in § Open 10:
   - The project's own `search_path` takes precedence over the host's. The outcome says so, but no bullet tests it.
   - A command named by `executable:` (an absolute path) still runs, and the run records the resolved path and sha256 (upstream acceptance 1, 2 and 6).
   - A `search_path` entry inside the workspace is refused in project data.
   Add the three as acceptance bullets, or list them as not carried.

All three findings are `origin: introduced`.

**What I checked, and found fine**
- Coverage by group, with no overlap between items:
  - Files, documents, generated trees and report files are claimed by stories 1, 3, 4 and 5.
  - `git.changed` is claimed by story 6.
  - `story.id` and `story.has_tag` are host context in stories 6 and 8.
  - Program identity is claimed by story 2.
  - Evaluation in the governor is claimed by story 7.
  - The gates run is claimed by story 8.
  - `net.*` is claimed by stories 9 and 10 behind their blockers.
  - Providers are refused as `ExtensionNotServed` by story 8.
- Nothing already in the store overlaps. I grepped `.engineering/planning` for gate-run, observe, assertion-providers, canon-evidence and vendor. `story:engineering-protocols-vendor-command` owns the vendor command, and story 7 only extends it for the catalog.
- No item reaches beyond the parent. Each is traceable to an epic sentence: story 2's launch change by the SDC-readers paragraph, story 8's blob by "Not in scope: offline replay", and story 7's `compile` grep as a stricter form of the epic's `evaluate` grep.
- The four implemented gate stories (`a-gate-sees-the-change-and-may-not-apply`, `gate-commands-name-a-program-portably`, `gate-executable-link-keeps-its-name`, `gate-executable-identity-bound-to-spawn`) are carried as follows:
  - `git.changed` and `applies_when`, including `not_applicable_is_never_pass`, are carried by stories 6 and 7.
  - Link name and identity are carried by story 2 (acceptance 1 and 4 of `gate-executable-link-keeps-its-name` and `-identity-bound-to-spawn`; the others fall under finding 3).
  - Not named as carried or dropped, and I do not file it: `a-gate-sees-the-change` acceptance 4, "every existing gates fixture keeps its result".

**Counts.** I extracted 21 promises from the parent and traced 20 to an item. The one untraced promise is the process observation values, which is finding 1.

**What I read**
- 12 artifacts: the epic, 10 stories and 2 blockers. I also read `story:bound-program-result-table` and `story:engineering-protocols-dependency-removed` for context.
- Commands run: `aep plan artifact show` on each, `aep plan artifact list`, and `grep` over `.engineering/planning`.
- Drafts and sources: `e7-request.md`, `e7-plan.md`, `e5-observation-collection-note.md` and the drafter's brief.
- Upstream at `0.16.0`: `lib.rs`, `reports.rs`, `process.rs` and `canon-engineering-assertions/src/lib.rs` (lines 340-445 and 620-700), the 26-function catalog and the four gate stories.
- `takt-comparison.md` lines 66-97.

**What I could not establish**
- I did not re-open the Substrate, Canon or Loom `file:line` citations: they are other lanes' to check, and the takt ones the drafter marked as read by a helper.
- `story:json-depth-bound` exists in the store, but I did not read its body.

**Out of my lane (not setting the verdict)**
- Acceptance: story 8 has no bullet for `story.id` or for absent story context being Unavailable.
- Acceptance: the epic's `observe_operations_cover_the_catalog` holds 26 catalog functions, not the 20 `OPERATIONS`, so `json.read` and `yaml.read` are held only by story 3's own tests.
- Acceptance: story 8 tests the total 300 s budget but not each request's timeout being cut to what is left.
- Design: story 3 says "five of the catalog's functions" and lists four.
- Design and parallel-safety: story 7 edits `crates/loom-xtask/src/`, whose command `story:engineering-protocols-vendor-command` owns (proposed). Story 7 does not depend on that story directly, only through `story:engineering-protocols-dependency-removed`.

```findings
- file: .engineering/planning/epic/observation-collection-as-executors.md
  line: 28
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the epic promises process.run, process.succeeded, ess.run and tests.run through the program executor, but no item claims the observation value of process.run, process.succeeded or ess.run (exit_code integer-or-null, stdout, stderr; process.succeeded Unavailable without an exit code, as assertion-providers lib.rs:182-191 at 0.16.0 and the vendored catalog type it); story 8 only says 'run as a bound program' and the program port keeps one blob and reads exit or a pointer, so a story (most naturally story:gate-run-collects-a-gates-document) must carry the mapping and an acceptance bullet for it"
- file: .engineering/planning/story/report-formats-checked-before-the-table.md
  line: 28
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story attaches report_format to ess.run, but upstream ess.run is plain process::collect with no report format (lib.rs:182) and only tests.run reads a program report (reports.rs:34, format fixed to engineering-tests/1 at :51-57); the epic also disagrees with itself (line 109 lists tests.run and ess.run, line 147 lists tests.run only), so drop ess.run from the report_format list in the story and the epic and give it the process value of the other item"
- file: .engineering/planning/story/gate-run-collects-a-gates-document.md
  line: 31
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the engineering.gates run silently narrows the gate commands of engineering-protocols' implemented story gate-commands-name-a-program-portably: the project's own search_path taking precedence over the host's, a command named by executable: with its path and sha256 recorded, and a search_path entry inside the workspace refused in project data have no acceptance bullet and are not listed as not carried in the work order's Open 10, so add the bullets or name them as not carried"
```
