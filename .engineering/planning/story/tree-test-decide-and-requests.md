---
format: aep.planning-md/3
id: story:tree-test-decide-and-requests
kind: story
status: draft
title: A suite cycle decides a tree's decision request and reads the requests
summary: decide step and scripted decide entry decide a tree decision request in process, as decisions --tree; the requests source lists a case's requests and decisions; the three SUITE_AVAILABILITY entries become Available; first-cut acceptance
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-test-command
- depends_on: story:tree-waits-open-decision-requests
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/src/suite/select.rs
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
revision: 2
---
## Outcome

A `loom.tree-suite/1` cycle can decide a tree's decision request and read the requests and their
decisions. The format already declares all three; this story makes them run:

- the `decide` step;
- the scripted `decide` entry;
- the `requests` source.

They move here from `story:tree-test-command` (decision D123). They run through the decision
requests that `story:tree-waits-open-decision-requests` adds to the tree's store, and decide
them as its `decisions --tree` does.

```yaml
steps:
  - {do: open}
  - {do: run, exit: 3}
  - {do: decide, case: ".", option: approve, principal: "person:owner"}
  - {do: run}
```

- **The step.** `{do: decide, case, option, principal}` decides one open decision request of the
  cycle's tree.
  - The request is the one on `case`; `.` is the root, as for the other steps.
  - With `option` it decides as that request's holder, through the function `decisions --tree`
    calls, in process.
  - It exits 0 when the decision is recorded. A refusal gives exit 1 and its reason, compared
    with the step's `refusal` as for any step (`crates/loom-tree/src/suite/run.rs:830-853`).
- **The scripted entry.** `{take: decide, case, option, principal, times, optional}`
  (`loom.tree.ScriptedChoice`, `ess/domains/tree.yaml:615-628`) answers a wait at exit 3 that is
  a decision request, in the order and with the use counts of the other entries.
- **The source.** `{requests: {case}}` gives the tree's decision requests on that case in the
  order they were opened. Each holds its members and Loom's decision on it, with the shape that
  `story:tree-waits-open-decision-requests` gives a request.
- **Availability.**
  - The entries the previous story pointed here (`crates/loom-tree/src/suite/availability.rs:77`,
    `:80`, `:95`) become `Available`.
  - The runner's arm that refuses the `decide` step goes (`run.rs:1023-1036`), as does the
    fall-through for `requests` (`crates/loom-tree/src/suite/select.rs:511-512`).

Security-relevant: a suite decides decision requests under a principal it names, which only a
test harness over its own scratch tree may do. The step writes only the cycle's own store.

## ESS first

- No new type. The step uses `loom.tree.CycleStep`'s `case`, `option` and `principal`
  (`ess/domains/tree.yaml:504-546`), and the entry uses `loom.tree.ScriptedChoice`.
- `requests` is already a `SourceKind` (`:701`, `:723`). Its item is the request that
  `story:tree-waits-open-decision-requests` declares; this story declares none of its own.

## Acceptance

This is a first cut. Every line is observable. The exact members of a `requests` item follow
`story:tree-waits-open-decision-requests` once it is active.

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

All tests go in `crates/loom-cli/tests/tree_test/mod.rs`, with fixtures below
`crates/loom-cli/tests/fixtures/tree-suite`, each under the module's 5 s.

1. `a_decide_step_decides_a_request`.
   - The cycle's first `run` stops at exit 3 on a decision request of the root.
   - `{do: decide, option: <an option of the request>}` exits 0.
   - The second `run` ends as expected.
   - A `requests` check on `.` shows the request decided with that option.
2. `a_decide_step_with_an_unknown_option_is_refused`: the same cycle with an option the request
   does not hold. The step's `refusal` names that refusal and the cycle is as expected; without
   `refusal`, the cycle ends `ExitDiffers`.
3. `a_decide_entry_answers_a_request`: the same cycle with a scripted `take: decide` entry and no
   `decide` step ends as expected. The entry is used once.
4. `an_unused_decide_entry_is_script_unused`: a cycle whose run opens no request, with a
   non-optional `take: decide` entry, ends `ScriptUnused` naming `decide[0]`.
5. `requests_selects_in_opening_order`: a cycle that opens two requests gives a `requests` list
   of two, in opening order, the first decided and the second open.
6. `nothing_waits_for_this_story`: `SUITE_AVAILABILITY` holds no
   `Waits("story:tree-test-decide-and-requests")`, and the three entries are `Available`.

## Out of scope

- Decision requests themselves, their routing and `decisions --tree`
  (`story:tree-waits-open-decision-requests`).
- The cycle kill, the ten slowest and `--list` (`story:tree-test-command`).

## Scope

Derived 2026-11-22 from the plan tree at `5bcc26d9`. **Cited** means read at the line named.

- `crates/loom-tree/src/suite/availability.rs` (`:77`, `:80`, `:95`), `suite/run.rs`
  (`:707-730`, `:830-853`, `:1023-1036`, and the scripted-entry answering in `run`) and
  `suite/select.rs` (`:511-512`) — cited.
- `ess/domains/tree.yaml` (`:504-546`, `:615-628`, `:701`, `:723`), read and not changed — cited.
- `crates/loom-cli/tests/tree_test/mod.rs` and `crates/loom-cli/tests/fixtures/tree-suite` —
  cited.
- `website/docs/guides/run-a-case-tree.md` (the step table and the sources), `CHANGELOG.md` —
  cited.

## Edges

- `depends_on story:tree-test-command`: it points the three entries here, and it merges first.
- `depends_on story:tree-waits-open-decision-requests` (proposed): the tree's decision requests
  and the decision function this story calls.
- **Would collide with**, no edge: `story:suite-cycle-writes-a-workspace-file`,
  `story:suite-runs-cycles-in-parallel` and `story:suite-runs-cycles-under-one-cgroup-root`, on
  `suite/run.rs`, `suite/availability.rs`, `tree_test/mod.rs`, the fixtures and the guide. All
  merge before this story.
