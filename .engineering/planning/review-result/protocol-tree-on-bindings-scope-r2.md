---
format: aep.planning-md/3
id: review-result:protocol-tree-on-bindings-scope-r2
kind: review-result
status: active
title: Plan critic scope, round 2, on epic:protocol-tree-on-bindings
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
approve

**What you read:** I re-read 15 bodies from the store with `aep plan artifact show`: the epic (revision 2) and 14 stories. Of those, 13 are rewritten and `story:substrate-0-21-pin` is unchanged. I did not re-read `upstream-blocker:engineering-protocols-on-canon-0-9-0`, `story:child-launches-through-confinement-adapters` or the graph. I re-extracted the promises from the epic and the seven requests, 25 in all, and traced all 25 to an item. Each round-1 finding is closed:

- **Open-action evidence.** `story:child-case-runtime` now records the opening action's `from: effect` entries in the parent. It cites `sdc.protocol.yaml:25-30` and tests a `child_opened` record in its acceptance.
- **`arguments_from`.** `story:bound-program-result-table` now runs it in its Outcome. It adds `ArgumentsUnreadable`, an acceptance bullet and the `stories.open` rationale.
- **`independence`.**
  - `story:tree-run-command` carries the file's map at `tree open` and tests it.
  - `story:child-case-runtime` does the same for children.
  - `story:bindings-document` has a "Who uses each member" paragraph.
- **Agent confinement narrowing.** The epic's first Not in scope bullet now records it. The Outcome says "a bound program" and the Acceptance says "Every program of that run".
- **`handoff`.** `story:tree-run-command` prints it when it waits, and tests that. `story:bindings-document` names that story as its user.
- **`tree status`.** The epic Outcome now says "the tree's state is printed as JSON for a live view", and `story:tree-run-command` has a Why line for it.
- **Bound commands claimed twice.** `story:confinement-opens-from-engine-config` now says "Bound commands reach this runner in `story:bound-commands-run-confined`, not here", and lists them under Out of scope.

The revision also created no new reach or double claim. I checked in particular:
- `--max-agent-calls`, which now sits only in `story:agent-node-run-records`, with `story:tree-run-command` excluding it.
- `--action-bindings` acceptance of `loom.bindings/1`, which the epic now excludes by name.
- `terminate(...)` in `story:child-case-runtime` against `tree close` in `story:tree-run-command`.

**What you could not establish:**
- The shape of the claim that remains outside the lane: how the SDC session binds its agent nodes as stub programs. The epic assigns that conversion to the SDC and I did not check it.
- Whether the `tree` command registers any agent adapter. That is a design question, outside my lane.
- Anything about the acceptance run itself. It belongs to the SDC session.

```findings
[]
```
