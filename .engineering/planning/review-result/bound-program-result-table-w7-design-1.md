---
format: aep.planning-md/3
id: review-result:bound-program-result-table-w7-design-1
kind: review-result
status: active
title: Plan critic (design), wave 7 round 1, on story:bound-program-result-table
relations:
- reviews: story:bound-program-result-table
revision: 1
---
needs-revision

story:bound-program-result-table — The Outcome lets the port start a bound program through any runner the host passes in. It never says whether the port refuses `CommandUnconfined` when that runner launches through `local` for a reason that is not an opt-in, as `select_runner` does on the `run --action-bindings` path. It also never says what tells a reader which of the two crates runs a given binding. Wave w6's own text says "an SDK host that sets bindings is held to the same rule". — .engineering/planning/story/bound-program-result-table.md:34-38, crates/loom-intake-slice/src/bindings.rs:366, story:bound-commands-run-confined § "As built, wave 2026-11-21-w6"

story:bound-program-result-table — The port is said to build its own `ChildCommand`, but the Outcome is missing three things that `CommandRunner::child` and `select` decide today:
- the directory the child starts in (`ChildCommand::new` needs one);
- the root `preflight_child` checks;
- what happens to a program outside the system view, which `child` mounts with `at_host_path` using the private `SYSTEM_VIEW`. Without that mount, a stub program outside `/usr`, `/bin`, `/lib` or `/lib64` cannot be seen under `linux`.

The story waiting on this one already expects the port to be "built per case ... for the case's workspace root". — .engineering/planning/story/bound-program-result-table.md:34-38, crates/loom-intake-slice/src/confinement.rs:112,1204, crates/loom-intake-slice/src/bindings.rs:83,323-358, story:gate-program-identity-bound-to-spawn § Outcome

story:bound-program-result-table — The wave shares more files than `Cargo.lock`. § Scope changes `AGENTS.md` and `CHANGELOG.md`, but the typed scope lists neither. `story:engineering-protocols-dependency-removed` edits both, and `story:decision-request-entity` edits `CHANGELOG.md` and regenerates `generated/rust/loom/**`. "Would collide with" names neither sibling. — .engineering/planning/story/bound-program-result-table.md:15-25,156,164, story:engineering-protocols-dependency-removed § Scope, story:decision-request-entity § Scope "Would collide with"

**What I read:** 4 artifacts (this story, `story:bound-commands-run-confined`, `story:decision-request-entity`, `story:engineering-protocols-dependency-removed`), plus the Outcome of `story:gate-program-identity-bound-to-spawn` and the scope lines of `story:tree-run-command` and `story:bound-artifact-revisions`.
- Commands: `aep plan artifact show/relations/graph/validate` (validate: valid), `git diff` of the story, `git grep` over the manifests, and reads of `confinement.rs`, `engine.rs`, `bindings.rs` and `loom-sdk/src/lib.rs`.
- Edges: I walked this story's 4 outgoing edges and the 5 `depends_on` edges into it, and about 45 `depends_on` edges around it, outside the set too.
- No cycle. No manifest depends on `b10x-loom-bindings`. `loom-intake-slice` depends on `loom-commission` and `loom-governor`, `loom-governor` depends on `loom-commission`, and none of them depends on `loom-bindings`.
- Acceptance reads traced to a producer in the set: 0. The siblings produce nothing this story reads, so no edge is needed.

**Visibility:** every item the story needs is public in `crates/loom-intake-slice`:
- `ChildCommand::new` :1204, `with_stdin` :1443, `with_timeout` :1432, `with_environment` :1333, `at_host_path` :1303
- `ExecEnvironment` (`pub use` :56), `TestRunner`, `TestExecution` :197, `ConfinementError`, `ConfinementRefusal` (`pub use` :19-21)
- `INHERITED` (`bindings.rs:72`)

Not public, and needed only if the port reuses them: `CommandRunner` :255 and `select_runner` :366 (`pub(crate)`), `SYSTEM_VIEW` :83 and `SOURCE` :84 (private).

**Checked and true:** every claim in "Not in this story" holds:
- `engine.rs:89,134`
- `confinement.rs:197-207`
- `:3100` (`output_bytes: 1_048_576`)
- `loom-sdk/src/lib.rs:120,123`
- the child writes only under `target/` (`confinement.rs:1142-1144`)

It promises nothing from the neighbour story that this story depends on. The moved cites `bindings.rs:604-650` and `:653-684` match.

**What I could not establish:**
- Out of my lane (acceptance): the Acceptance has no line for a host that passes no runner and gets `NoRunner`, which the brief requires.
- I did not check `~/beyond10x/sdc/driver/src/main.rs:1441-1462`.

```findings
- file: .engineering/planning/story/bound-program-result-table.md
  line: 34
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The Outcome lets the port start a bound program through any runner the host passes in. It never says whether the port refuses CommandUnconfined when that runner launches through local for a reason that is not an opt-in, as select_runner does on the run --action-bindings path. It also never says what tells a reader which of the two crates runs a given binding."
- file: .engineering/planning/story/bound-program-result-table.md
  line: 34
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The port is said to build its own ChildCommand, but the Outcome does not name the directory the child starts in, the root preflight_child checks, or what happens to a program outside the system view (at_host_path). CommandRunner::child and select decide all three today with pub(crate) or private items."
- file: .engineering/planning/story/bound-program-result-table.md
  line: 164
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The wave shares more files than Cargo.lock. The story changes AGENTS.md and CHANGELOG.md but its typed scope lists neither, story:engineering-protocols-dependency-removed edits both, story:decision-request-entity edits CHANGELOG.md and regenerates generated/rust/loom, and Would collide with names neither sibling."
```
