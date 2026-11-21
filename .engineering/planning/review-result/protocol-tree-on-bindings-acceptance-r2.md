---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:protocol-tree-on-bindings
relations:
- reviews: epic:protocol-tree-on-bindings
- reviews: story:canon-0-9-0-pin
- reviews: story:substrate-0-21-pin
- reviews: story:confinement-opens-from-engine-config
- reviews: story:bound-commands-run-confined
- reviews: story:bindings-document
- reviews: story:bound-program-result-table
- reviews: story:bound-artifact-revisions
- reviews: story:bound-agent-node
- reviews: story:deterministic-default-selector
- reviews: story:child-case-runtime
- reviews: story:record-copy-between-cases
- reviews: story:hash-chained-run-log
- reviews: story:tree-run-command
- reviews: story:agent-node-run-records
revision: 1
---
needs-revision

- story:bindings-document — `from: effect` is defined only as "Loom's own report of what it did" and names no value for `results` to key on. The only value, `performed`, appears in `story:bound-agent-node`'s acceptance. `story:child-case-runtime` relies on a `from: effect` entry (its Outcome and acceptance) without depending on that story, so its fixture cannot write the entry. This is a new finding of round 2, from the revised text. — .engineering/planning/story/bindings-document.md:25

All 26 round-1 findings are resolved. I checked each against the revised bodies.

- **story:confinement-opens-from-engine-config:** the `--confinement` mapping is now explicit (lines 54-58). The single call site is decided by a grep (44). `spawn_child` and the test runner are each exercised (46-48).
- **story:bound-commands-run-confined:** the JSON-stdin and `PATH`-only environment bullet is added (43-45). The existing tests are named (48-53), and I confirmed all four files exist. The file route and the reason naming the flag or file are in the opt-in bullet (56-59).
- **story:bindings-document:** the crate is named (14-15, 62). The model and `max_runs` defaults are decode-level observables (78-79). `--action-bindings` is moved out of scope (93), with a `git diff --stat` check (80-82). The reference page is decided by `task docs-generate` and `task docs-check` plus a decoded code block (83-85).
- **story:bound-program-result-table:** the revision contradiction is resolved. Lines 28-31 and 69-71 now agree with `story:bound-artifact-revisions` lines 51-52. The stdin and environment bullet is added (64-65).
- **story:bound-artifact-revisions:** the format is pinned by a known value (45). The SHA-256 of `abc` begins `ba7816bf8f01cfea`, so `tba7816bf8f01cfea` is correct.
- **story:bound-agent-node:** `inputs` and `outputs` are now in the `agent` executor (`bindings-document.md:19-22`). The refusal is observable in the observation payload (64-65).
- **story:deterministic-default-selector:** the `retry_when` and no-evidence rules are now tested through a fixture `StepHistory` (62-64). The "default for a run driven by bindings" claim is gone from the Outcome.
- **story:child-case-runtime:** the trigger is now `terminate(case, outcome, instant)`, called directly by this story's tests (32-35, 79-81). The shared workspace is tested at lines 91-92.
- **story:record-copy-between-cases:** each refusal is named (57-61), and `copy_due` is tested (55-56).
- **story:hash-chained-run-log:** the canary check moved to `story:tree-run-command` (106-107) and `story:agent-node-run-records` (63-64).
- **story:tree-run-command:**
  - `tree verify` and `tree status` are covered (101-103).
  - The store lock is checked (99-100).
  - The confinement claim has a grep and a Linux read test (111-114).
  - The agent-call cap moved to `story:agent-node-run-records`, with its own fixture at lines 65-67 and the dependency on the tree story added.
- **story:agent-node-run-records:** turns, duration and transcript digest are checked (58-59).

What I read: 16 of 16 ids with `aep plan artifact show` (the 14 stories, `upstream-blocker:engineering-protocols-on-canon-0-9-0`, and the dependency `story:child-launches-through-confinement-adapters`), plus `epic:protocol-tree-on-bindings` for context. I re-read every body from the store, not from memory. I also ran `git grep` and `sed` in the Loom tree for `INHERITED`, the named test files and the crates, and `git rev-parse` in the canon and substrate repositories for the pinned tags.

What I could not establish:
- Whether the new crates `loom-bindings`, `loom-runlog` and `loom-tree` can depend on `loom-intake-slice`'s runner. That is design.
- The epic's own acceptance, which the SDC session runs and reports. I judged only the story set.
- Out of my lane, not counted in the verdict:
  - `story:confinement-opens-from-engine-config` offers `--confinement substrate|local|none` on `run`, while `story:tree-run-command` offers `linux|local`. The two commands name the Linux value differently (design).
  - `story:deterministic-default-selector`'s summary line still says "The default for a run driven by bindings", though its Outcome no longer claims it.

```findings
- file: .engineering/planning/story/bindings-document.md
  line: 25
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "`from: effect` is defined only as \"Loom's own report of what it did\" and names no value for `results` to key on; the one value, `performed`, appears only in story:bound-agent-node's acceptance, so story:child-case-runtime, which relies on a `from: effect` entry without depending on it, cannot write its fixture"
```
