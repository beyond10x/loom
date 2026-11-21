---
format: aep.planning-md/3
id: review-result:observation-collection-as-executors-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:observation-collection-as-executors
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

epic:observation-collection-as-executors — the check "`git grep -n 'canon_expr::evaluate' -- crates` prints only that crate" gives the same result before the work as after, because no crate names `canon_expr` today and an empty output already counts as "only that crate"; the acceptance should require at least one line, all in `crates/loom-governor`, or check the manifests that depend on `b10x-canon-expr` — .engineering/planning/epic/observation-collection-as-executors.md:156-157

story:gates-document-evaluated-in-the-governor — the bullet "`git grep … 'canon_expr::evaluate\|canon_expr::compile'` prints lines of `crates/loom-governor` only" already passes on today's tree, where the grep prints nothing; it also fails to show the work if the code imports the names with `use canon_expr::{compile, evaluate}` — .engineering/planning/story/gates-document-evaluated-in-the-governor.md:97-98

story:gates-document-evaluated-in-the-governor — the expired / plan-mismatch / identity-mismatch bullet names no expected result ("the result Canon gives is not read here; the test pins it"); Canon reports `unknown` either way, so the test cannot tell an observation Loom passed on from one Loom dropped; the bullet should name Canon's reasons (`<id>: expired`, `<id>: plan mismatch`, `source or context identity mismatch`, canon 0.9.0 `crates/canon-expr/src/eval.rs:91-96`, `:115`) — .engineering/planning/story/gates-document-evaluated-in-the-governor.md:89-91

story:gates-document-evaluated-in-the-governor — the Outcome says `loom-xtask protocols-vendor` copies the catalog with the protocol files, but no acceptance bullet checks what the command writes; the digest test checks only the committed file — .engineering/planning/story/gates-document-evaluated-in-the-governor.md:37-38

story:gates-document-evaluated-in-the-governor — engineering-protocols' implemented `a-gate-sees-the-change-and-may-not-apply` guarantees in its acceptance item 4 that "Every existing gates fixture keeps its result" (`fixtures/assertions/gates.yaml`); this acceptance carries only the `applies-when.fixture.yaml` cases and does not name that guarantee as not carried — .engineering/planning/story/gates-document-evaluated-in-the-governor.md:84-85

story:report-formats-checked-before-the-table — on the program path the acceptance expects a `ProgramRun` with outcome `Refused` carrying `ReportInconsistent` / `ExitContradictsReport`, and outcome `Unavailable` for a Codegate `Incomplete`; but ESS first adds those reasons only to `ObserveRefusal`, and `ProgramRun` has no outcome member (action, exit, blob digest, timed out: `bound-program-result-table.md:63`), so no test can check these bullets against the declared records — .engineering/planning/story/report-formats-checked-before-the-table.md:97-98

story:gate-program-identity-bound-to-spawn — "Every refusal above is a `ProgramRun` with its `ProgramRefusal` and the effect refused naming it" cannot be checked: ESS first gives `ProgramRun` only `program`, with no member for a `ProgramRefusal` (:82-85), and the admission refusals happen "when the port is built" (:104), before any step exists to refuse — .engineering/planning/story/gate-program-identity-bound-to-spawn.md:136-137

story:observe-executor-reads-workspace-files — the bullet "a path that is absolute or holds `..` is refused at load, `PathNotInWorkspace`" sits under a preamble that requires every refusal to be an `ObserveRun` with an `ObserveRefusal` and Substrate's code; a refusal at load produces no run, so the two cannot both pass — .engineering/planning/story/observe-executor-reads-workspace-files.md:142-146

story:observe-executor-reads-workspace-files — the acceptance gives no outcome for `text.contains` on an absent path; engineering-protocols gives `Unavailable` (`files.rs` `read` I/O error, mapped by `lib.rs:152-161` at 0.16.0), while this story's rule that "every other Substrate refusal is `Refused`" would make it a refusal — .engineering/planning/story/observe-executor-reads-workspace-files.md:171-172

story:gate-run-collects-a-gates-document — the Outcome and § Why name guarantees that no acceptance bullet checks: no base in the context gives `Unavailable` (:25), the 4 MiB bound on reads at the base (:24), the glob preflight (:29), the project's `search_path` before the host's (:33, engineering-protocols `process_program.rs::a_runtime_default_search_path_is_used`), `valid_until` from `freshness_seconds` (:37), the project limits behind `ProjectInvalid` (:66-67), and `GatesInputUnreadable`; no test reaches either of the two declared reasons (:81-82) — .engineering/planning/story/gate-run-collects-a-gates-document.md:92-124

story:dns-resolution-through-a-resolver-aperture — "a positive answer contradicted by a later NXDOMAIN … is refused" names no `ObserveRefusal`, and ESS first declares only `ResolverRefused` and `HostNameInvalid`, so the refusal record this bullet expects cannot be checked — .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md:63-65

story:dns-resolution-through-a-resolver-aperture — the Outcome's rule that `valid_until` is the lesser of `freshness_seconds` and the TTL in a gate run (carried from `canon-engineering-assertions/src/lib.rs:651-658`) has no acceptance bullet, though `observe/gates.rs` is in § Files for it — .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md:22-23

**What I read:** all 13 artifacts I was given, 13 read in full (the epic, 10 stories, 2 upstream blockers), using `aep plan artifact show` on each, the files under `.engineering/planning/`, `aep plan artifact kinds` and `aep plan artifact lifecycle story|upstream-blocker`. I also read `bound-program-result-table.md`, AGENTS.md:436-444, :1125-1145, :2112-2180, canon 0.9.0 `eval.rs:34-125`, substrate 0.21.0 `fs.rs:1545-1578` and `lib.rs:684-690`, engineering-protocols 0.16.0 (`OPERATIONS`, `files.rs` collect, the catalog's sha256, which matches the story, and the acceptance of the four implemented gate stories), and `prepare_with`.

**What I could not establish:**
- That the catalog holds exactly 26 functions. I counted 20 `"operation"` entries, and `document.get` appears three times; I did not count function names.
- The takt citations, which I did not open.
- Out of my lane (design / parallel-safety), not in the verdict: story 6 adds a git call site in `crates/loom-bindings`, and the existing `tests/host_git_hardening.rs::every_host_git_command_goes_through_a_hardened_helper` rejects other git call sites (AGENTS.md:2175-2177). Story 6 does not say how it gets past that test.
- Out of my lane (design), not in the verdict: story 8 makes `b10x-loom-bindings` depend on `b10x-loom-governor`.

```findings
- file: .engineering/planning/epic/observation-collection-as-executors.md
  line: 156
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the check that `git grep 'canon_expr::evaluate' -- crates` prints only the governor gives the same result before the work as after, because no crate names canon_expr today and an empty output already counts as 'only that crate'"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 97
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the grep for canon_expr::evaluate/compile 'prints lines of crates/loom-governor only' already passes on today's tree, where it prints nothing, and fails to show the work if the names are imported with use"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 89
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the expired / plan-mismatch / identity-mismatch bullet names no expected result, so it cannot tell an observation Loom passed on from one Loom dropped; it should name Canon's reasons (eval.rs:91-96, :115)"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 37
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says loom-xtask protocols-vendor copies the catalog, but no acceptance bullet checks what the command writes"
- file: .engineering/planning/story/gates-document-evaluated-in-the-governor.md
  line: 84
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "engineering-protocols' guarantee that every existing gates fixture keeps its result (a-gate-sees-the-change-and-may-not-apply acceptance item 4, fixtures/assertions/gates.yaml) is neither carried nor named as not carried"
- file: .engineering/planning/story/report-formats-checked-before-the-table.md
  line: 97
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "on the program path the acceptance expects a ProgramRun with outcome Refused carrying ObserveRefusal reasons, and outcome Unavailable, but ProgramRun has no outcome member and these reasons are declared only on ObserveRefusal, so no test can check them"
- file: .engineering/planning/story/gate-program-identity-bound-to-spawn.md
  line: 136
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "every refusal is to be a ProgramRun with its ProgramRefusal, but ESS first gives ProgramRun no member for it, and admission refusals happen when the port is built, before any step exists to refuse"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 145
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the load-time PathNotInWorkspace bullet sits under a preamble requiring every refusal to be an ObserveRun with an ObserveRefusal and Substrate's code; a refusal at load produces no run, so both cannot pass"
- file: .engineering/planning/story/observe-executor-reads-workspace-files.md
  line: 171
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance gives no outcome for text.contains on an absent path; engineering-protocols gives Unavailable, while this story's rule would make it Refused"
- file: .engineering/planning/story/gate-run-collects-a-gates-document
  line: 92
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the Outcome and Why name guarantees no acceptance bullet checks: no base gives Unavailable, the 4 MiB base-read bound, glob preflight, the project search_path before the host's, valid_until from freshness_seconds, project limits behind ProjectInvalid, and GatesInputUnreadable"
- file: .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md
  line: 64
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the refusal of a positive answer contradicted by a later NXDOMAIN names no ObserveRefusal, and ESS first declares only ResolverRefused and HostNameInvalid"
- file: .engineering/planning/story/dns-resolution-through-a-resolver-aperture.md
  line: 22
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome's rule that valid_until is the lesser of freshness_seconds and the TTL in a gate run has no acceptance bullet"
```
