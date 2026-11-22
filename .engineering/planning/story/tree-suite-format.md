---
format: aep.planning-md/3
id: story:tree-suite-format
kind: story
status: active
title: A loom.tree-suite/1 document states scripted tree cycles as data, and b10x-loom tree test runs it one cycle after another, a cycle that needs a later story counted pending
summary: suite types in tree.yaml; members, defaults, twelve step kinds, when; sources log, records, runs, cases, doc, files, step, suite; thirteen check kinds; SUITE_AVAILABILITY marks each kind available or waiting for a named story; first runner with open, run, approve, close, status, verify; summary lines with a waiting clause; exit 0/1/2
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-command
- depends_on: story:bound-program-retry-records
scope:
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: inferred
  path: crates/loom-cli/tests/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_test.rs
- confidence: cited
  path: crates/loom-protocols/src/lib.rs
- confidence: inferred
  path: crates/loom-tree
- confidence: inferred
  path: crates/loom-tree/src/suite/availability.rs
- confidence: inferred
  path: crates/loom-tree/src/suite/check.rs
- confidence: inferred
  path: crates/loom-tree/src/suite/decode.rs
- confidence: inferred
  path: crates/loom-tree/src/suite/run.rs
- confidence: inferred
  path: crates/loom-tree/src/suite/select.rs
- confidence: inferred
  path: crates/loom-tree/tests/fixtures/suite-format
- confidence: inferred
  path: crates/loom-tree/tests/main.rs
- confidence: inferred
  path: crates/loom-tree/tests/suite_format.rs
- confidence: inferred
  path: ess/domains/tree.yaml
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T01:26:23Z", actor: "agent:sdc", revision: 6, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w10"}
- {from: "proposed", to: "active", at: "2026-11-22T01:26:23Z", actor: "agent:sdc", revision: 7, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w10"}
---
## Outcome

A suite of scripted tree cycles is a `loom.tree-suite/1` document, and `b10x-loom tree test` runs
one, one cycle after another, and reports per cycle whether it ended as the suite expects. This
story declares the document in `ess/domains/tree.yaml`, adds to `crates/loom-tree` its decoder
(`decode_suite`), the evaluator of its checks (`evaluate_checks`) and a first runner
(`run_suite`), and adds the command to `crates/loom-cli`. A check is data: a selection over what
the cycle left, and one test from a closed list. No shell, no query string, no expression
language. `story:tree-test-command` adds workers, the cycle kill, the slowest list and `--list`;
the stories named under "Availability" add the step kinds and sources that need their commands.

```
b10x-loom tree test --suite <file> --protocols <dir> --scratch <dir> --at <instant>
                    [--confinement substrate|local [--local-reason <text>]] [--report <file>]
```

```yaml
format: loom.tree-suite/1
defaults: {protocol: sdc@1, bindings: bindings/stub.yaml}
cycles:
  - name: trace-gap
    seed: [{from: seeds/idea.yaml, to: idea.yaml}]
    expect:
      exit: 0
      outcome: operating
      checks:
        - check: contains
          select: [{log: {case: billing.spec, event: ActionFinished}},
                   {where: {action: spec.trace}}, {pick: last}, {follow: blob}]
          is: "3 traced to behaviour, 0 constraint, 0 dropped, 0 untraced"
```

- **Cycle members.** `name`, `pending`, `when`, `protocol`, `bindings`, `engine`, `fixtures`,
  `revisions`, `seed`, `principal`, `max_rounds`, `decide`, `steps`, `expect`. `defaults` holds
  any of `protocol` to `steps`. A stated member replaces the default whole, except `decide`, whose
  default entries are tried after the cycle's own. `name`, `pending`, `when` and `expect` are a
  cycle's alone (`MemberUndeclared` in `defaults`). A cycle without `protocol` or `bindings` from
  itself or `defaults` is `MemberMissing`.
- **Steps** (`steps`, default `[{do: open}, {do: run}]`). Each step has `do`, an optional `name`,
  `at` (minutes after `--at`; default one minute after the step before), `exit` (a status or
  `{not: <status>}`), `refusal` (the name of a refusal Loom declares), and `checks` (evaluated
  right after the step). The default `exit` is 0, except for the last `run`, whose default is
  `expect.exit`. `do` is one of `open`, `run` (rounds of `tree run`, each exit 3 answered from
  `decide`, until another exit or a wait no entry answers; `rounds: 1` runs one; `engine`
  replaces the cycle's), `approve`, `close`, `decide` (one scripted decision), `stop`, `resume`,
  `allow`, `reopen`, `replay`, `status`, `verify` (the last three give the step's JSON output).
- **Scripted decisions** (`decide`, entries with `take`: `approve`, `close`, `decide`, each used at
  most `times` times, default 1, or `times: any`; `optional`; `principal`). At an exit 3 every
  printed wait is taken in printed order and answered by the first entry, in list order, that
  matches it and has a use left: `approve` by `case` and `capability` or `action`, as
  `tree approve`; `close` with `outcome`, as `tree close`. An entry that is not `optional` and
  was never used is the difference `ScriptUnused`.
- **Conditions on a cycle** (`when`, a list, all must hold): `{protocol: <id>, declares: {action:
  <id>}}` or `{… declares: {outcome: <id>}}`, read from the compiled protocols of the suite's
  catalog; `{suite: <file>, path: <path>, <condition>}`, read from a file of the suite. A cycle
  whose `when` fails is not run, printed `<name>: not run (when[<i>])`, and counted in neither
  `expected` nor `pending`.
- **Expect.** `exit` (of the last `run` step), `outcome` (the root's terminal outcome) or `stop`
  (the last `TreeStop` variant, and its case when given), and `checks`. `tree verify` of the
  store must pass in every cycle.

### Selections

`select` is a list: one source, then any of `where`, `pick`, `path`, `follow` and `as`, applied
in the order written. A selection gives a list of values in a fixed order.

- **Sources.**
  - `{log: {case: <case>, event: <kind or list>}}`: a case's run log lines in `seq` order, each as
    its JSON object (`seq`, `at`, `kind`, the event's members).
  - `{records: {case: <case>, kind: <kind or list>}}`: the `canon-evidence/1` records the case
    holds, in the order it received them; without `kind`, every record.
  - `{runs: {case: <case>}}`: the program runs of the case, the observations of source
    `loom/bound-program` in the store's `observations.json`, each as its payload (`case`,
    `action`, `exit`, `blob_sha256`, `timed_out`, `runner_refusal`, `retry_of`).
  - `{cases: {protocol: <id>}}` or `{cases: {}}`: the cases of `tree status`, in its order.
  - `{doc: {in: work|store, path: <relative path>}}`: one file, read by its extension: `.json` a
    value, `.jsonl` a list (one value per line), `.yaml` a value; any other file is text.
  - `{files: {in: work|store, glob: <pattern>}}`: the relative paths that exist, files and
    directories, in byte order; `*` within one segment, `**` across segments.
  - `{step: <step name>}`: the JSON output of a `replay`, `status` or `verify` step.
  - `{suite: <file>}`: a file of the suite's directory, read as `doc` reads one.
  - `{requests: {case: <case>}}`: a tree's decision requests and their decisions;
    `{calls: {case: <case>}}`: agent and decider call records (both under "Availability").
- **A case** is a path below the root (`.` is the root, `billing.invoices` is the case
  `<root>.billing.invoices`), `{protocol: <id>}` (every case of that protocol), `{name: <last
  segment>}`, or `"*"` (every case). For several cases the items are taken in `tree status`
  order, and each carries a `case` member.
- **Operations.** `{where: <conditions>}` keeps the items that hold. `{pick: first|last|<n>}`
  keeps one (`<n>` counts from 0; a negative `<n>` counts from the end). `{path: <path>}` takes
  the values at the path in each item. `{follow: blob}` takes, for each item, the blob its
  `blob_digest` names (or the item itself when it is a digest string): `blobs/<sha256>` of the
  store, read as JSON when it parses, else as text; `{follow: {blob: <member>}}` names another
  member. `{follow: evidence}` takes the records named by each item that is an evidence id, or by
  an item's `evidence` list. `{follow: file}` reads each item as a path relative to the workspace,
  and `{follow: {file: <relative>}}` reads `<relative>` below it. `{as:
  text|lines|json|jsonl|yaml}` reads a file another way than its extension says.
- **A path** is segments joined by `.`: a member name, `name[<n>]`, `name[*]` (every element),
  or `name[<member>=<value>]` (the elements whose member equals the scalar). A name or value with
  a `.`, `[` or `]` is quoted (`"p-offline-hosted-context.deployment"`). `.` alone is the item.
  A path that reaches nothing gives no value. Nothing else is a path.
- **Conditions** map a path to a scalar (equal to it) or to one test: `equals`, `not_equals`,
  `in`, `not_in`, `contains` (a substring, or an element of a list), `lacks`, `matches`,
  `not_matches`, `min`, `max`, `type` (`string`, `number`, `bool`, `list`, `map`, `null`),
  `length` (`{min, max}` of a string or list), `exists` (true or false). All entries must hold;
  `any_of: [<conditions>, …]` holds when one of its entries does, at one level only. A regex is
  the syntax of the Rust `regex` crate (no look-around, no back-reference), at most 1024 bytes.

### Check kinds

Exactly these thirteen; each check has an optional `name`, `select` (all except `compare`) and
the members of its kind:

1. `exists` `{is: true|false}`: the selection is not empty, or empty.
2. `count` `{is: <n>}` or `{min, max}`: the number of values.
3. `equals` `{is: <value>}`: one value, equal to it.
4. `contains` / 5. `lacks` `{is: <value>}`: one value; a text holds the substring, or a list
   holds the element.
6. `matches` / 7. `not_matches` `{regex}`: one text value.
8. `all` / 9. `none` / 10. `some` `{where: <conditions>}`: every value, no value, or at least
   one value holds. `all` over an empty selection fails.
11. `sequence` `{is|starts|ends|within: [<value>, …]}`: the values in order are the list, start
    with it, end with it, or hold it as a subsequence.
12. `before` `{first, then, between_none}` (conditions each; `between_none` optional): after the
    last value that holds `first` comes a value that holds `then`, and no value between the two
    holds `between_none`.
13. `compare` `{left, op, right}`: `op` is `eq`, `ne`, `lt`, `le`, `gt`, `ge` or `subset` (every
    element of `left` is in `right`). An operand is `{value: <v>}` or `{select: […], take:
    value|count|size|digest|length, plus: <integer>, after: <step name>}`: `size` is a file's
    bytes, `digest` its SHA-256, `length` a string's characters or a list's elements; `after`
    evaluates the selection as the cycle stood right after that step. An operand with `take:
    value` that gives no value or more than one is the difference `OperandNotOne`, naming the
    side and the count.

### What a check reads

Only what the cycle left under `<scratch>/<n>/` (`store/` and `work/`), the steps' JSON output
and exits, and, through `suite` and `when`, files of the suite's directory. Never a program's
console text. A path written in the suite that is absolute or whose `..` leaves its root is
refused at decode (`PathOutsideSuite`, `PathOutsideCycle`). A path read from a document, or a
link, whose real path leaves `work/` gives the difference `PathOutsideCycle`; a blob whose bytes
do not hash to its name gives `BlobChanged`; a file over 16 MiB or a glob over 10,000 paths gives
`SelectionTooLarge`.

A difference names the check (its `name`, else `expect.checks[<i>]` or
`steps[<j>].checks[<i>]`), its kind and the expected value. What was found is printed when it is
a number, a boolean, or a value Loom declares (event kinds, stop variants, outcomes, record kinds
and results, refusal names). Any other found text is printed as its length and the first 12 hex
digits of its SHA-256, so a program's output never reaches a summary line.

### Availability

The format knows every step kind, source and run log event kind above; a name it does not know
is refused at decode (`StepKindUnknown`, `SourceUnknown`, `EventKindUnknown`). The binary holds
one table, `SUITE_AVAILABILITY`, that marks each as available or as waiting for a named story. A
cycle that uses one that waits, in a step, a scripted entry, a selection or a cycle member, is
not run. It is printed `<name>: pending, waits for story:<id> (<kind>)`, counted under `pending`
whether or not the suite marks it pending, and never counted failed or skipped. This story makes
available:

- steps `open`, `run`, `approve` (`principal`, `capability`, `action`), `close`, `status`,
  `verify`, and the scripted entries `approve` and `close` (`story:tree-run-command`);
- sources `log` (the kinds of `ess/domains/runlog.yaml`), `records`, `runs`, `cases`, `doc`,
  `files`, `step`, `suite`; all thirteen check kinds; `when`.

Waiting, each until the story named adds it with one passing and one failing cycle:
`stop`, `resume`, `allow`, `reopen`, `approve` with `result`, `reasons` or `deny`, and the event
kinds `StopRecorded`, `StopLifted`, `Labelled` (`story:tree-stop-record-and-labels`); `replay`
(`story:tree-replay-command`); `calls` (`story:agent-node-run-records`); the member `engine` and
`--engine` (`story:engine-configuration-document`); `fixtures` and the event kind `RunSettings`
(`story:engine-agents-section`); `decide` (step and entry) and `requests`
(`story:tree-test-command`).

### Running

Cycles run one after another in suite order. Cycle `n` works in `<scratch>/<n>/` (`store/`,
`work/`); `--scratch` must be absent or empty (`ScratchNotEmpty`), and nothing is written outside
it. The protocols are read once, as `tree open --protocols` reads them, into one catalog given to
every cycle; a file Canon refuses is `ProtocolsUnreadable` before any cycle. Per cycle: `when`;
the `seed` entries copied into `work/` (a symbolic link is `SeedUnusable`); each step through the
function of its `tree` command, in process, with the flags it needs (`--confinement`,
`--local-reason`); after each step its exit and refusal compared, its `checks` evaluated and the
operands naming it in `after` taken; after the last, `expect` and `tree verify`. The first
difference ends the cycle. `max_rounds` (default 40) rounds without an end is `RoundsSpent`. Each
`tree run` gets the seconds left of 60 per cycle as `--max-wall-seconds`; a program is stopped at
its binding's own timeout.

Output: one line per cycle, `<name>: as expected (<s> s)`, `<name>: FAILED: <difference>
(<s> s)`, `<name>: pending, <still failing: <difference>|passing> (<s> s)`, the `waits for` line
or the `not run` line, `<s>` the cycle's wall time. Then `cycles: <N> expected, <M> as expected,
<K> failed: <names|none>` and `pending: <P> (<Q> still failing: <names|none>; passing, move to
expected: <names|none>; <W> waiting: <name> on story:<id>, …|none)`, the lines of
`~/beyond10x/sdc/scripts/run-finding-cycles.sh:164`, `:169` with the waiting clause added.
`--report <file>` writes the same as one `loom.tree.SuiteReport` JSON document. Exit status: 0
when every cycle that runs and is not pending is as expected; 1 when one is not; 2 when the
command line, the suite, the protocols or the scratch directory is refused, before any cycle.
`tree test` starts no `b10x-loom` process and no program of its own.

Refused before any cycle, each a `SuiteRefusalReason`: `SuiteUnreadable`, `MemberUndeclared`,
`MemberMissing`, `PathOutsideSuite`, `PathOutsideCycle`, `CycleNameUnusable`, `CycleTwice`,
`StepNameTwice`, `AfterUnknownStep` (an `after` or `step` naming no step before it),
`PathUnusable`, `RegexUnusable`, `SelectUnusable` (a list not starting with exactly one source, or
an operation with more than one key), `StepKindUnknown`, `SourceUnknown`, `EventKindUnknown`,
`ProtocolsUnreadable`, `ScratchNotEmpty`, `SeedUnusable`.

## Why

- The suite is the planner's acceptance test: from this story's wave on, each wave report carries
  "N of 127 as expected" on Runbook's converted file, and a cycle whose step or source arrives
  later is `pending` naming the story it waits for. That needs a runner in this wave, not only a
  format, and the runner calls `decode_suite`, so the two are one story (the planner's § 1.1 rule
  1: a pair shares no wave when one calls a function the other adds).
- Runbook's cycles exist as data: `cycles/suite.yaml` (format `runbook.cycles/1`, 127 entries, on
  `~/beyond10x/sdc` branch `wave-rb1/cycles-suite`, commit `5971c50`) and `docs/cycles-suite.md`
  there. 29 cycles are expressed with 457 declarative checks; 98 carry 237 assertions that shape
  cannot say, in 12 classes. Runbook converts its file to this format and rewrites or drops what
  stays out, with a reason per cycle. The 457 checks map onto the kinds above (`event_count`,
  `json_count`: `log` or `doc` with `where` and `count`; `*_equals`: `path` and `equals`;
  `*_matches`: `matches`; `yaml_has_item`: `contains`; `file_absent`: `exists`; `file_same_as`:
  `compare` of `digest`; `file_count_lines`: `as: lines` and `count`).
- Runbook's checks hold `jq` and `yq` programs as strings. Form rejected: a query language in the
  suite. Cost: a second language with its own evaluator, which can express what no comparison
  names.
- Runbook's driver files map to Loom's: `run-stopped.json` to the root's `StopRecorded` and last
  `Stopped` lines; `status.json` to `cases` and the `status` step; `decisions.jsonl` to
  `requests`, whose members are Canon 0.10.0's `pending_decisions` entries (`action`,
  `capability`, `outcome`, `decision`, `case_revision`, `options`) with Loom's decision on each;
  `run-settings.jsonl` to the `RunSettings` line of `story:engine-agents-section`;
  `llm-calls.jsonl` and the `decider/`, `unstick/`, `.sdc/llm/` directories to `calls`. The
  planner restates or drops what read `defects.jsonl` and the run status counters. Console checks
  (13 declarative, 4 `console-regex`) are restated over the run log or the records.
- Runbook's six `retry_of` assertions ("the second run of action X names the first as the run it
  retries") read `runs`: `[{runs: {case: billing.invoices}}, {where: {action: change.implement}},
  {pick: last}, {path: "retry_of[*]"}, {follow: evidence}, {path: kind}]`. `retry_of` is the
  evidence ids of the retry records the run was given (`crates/loom-bindings/src/retry.rs:121-127`
  and `program.rs:817-827` on `impl/bound-program-retry-records`).
- Cycles that read another cycle's run (`uses_run_of` 6, `shared_run` 3, `needs` 2) repeat that
  run in their own store: a store records its workspace path at `tree open`.
- Count against the file, by class (expressed / restated or dropped by the planner / left):
  aggregate 49/0/2, computed-input 2/0/1, conditional 4/0/0, console-regex 4/0/0, cross-record
  22/0/1, derived-value 5/0/8, event-order 43/2/0, field-compare 14/4/1, mutated-store 0/0/4,
  second-run 8/0/18, setup-shell 3/0/6, tree-search 33/0/3; 187/6/44 of 237. Of the 127 cycles,
  101 are expressed in full, 2 more once the planner restates theirs, 21 in part, 3 not at all
  (`run-metrics`, `run-stop-decider`, `rework-pending-rejection`). Left: sums of two values (to
  the planner), Runbook's own programs run over a finished run, git, edits of a copied store, a
  console counter.
- `red_then_green` (4 assertions; the planner's check, `scripts/run-finding-cycles.sh:2313-2316`
  on `wave-rb2/red-then-green`) needs no capture. Over `[{records: {case: "."}}]`, A is the last
  `{kind: repro_run, result: red}` (subject `repro`), B the last `{kind:
  implementation_change}` and C the last `{kind: test_run, result: passed}` (both subject
  `change`, the code tree, its revision a content digest), each taken with `{pick: last}`, so a
  rework's earlier `test_run` `failed` is never C. Seven conditions, nine checks in one cycle
  entry: `before` A then B and B then C (after the last A comes a B, which holds exactly when the
  last B follows the last A); `compare` `eq` of C's and of B's `upstream_revisions.repro` with
  A's `subject_revision`, and of B's `subject_revision` with C's; `all` `{".": {not_in: ["none",
  null]}}` on A's `subject_revision`, A's `upstream_revisions.change` and C's `subject_revision`
  (a path that reaches nothing gives no value and `all` over no value fails, so each must also
  exist); `compare` `ne` of C's `subject_revision` with A's `upstream_revisions.change`.
  `canon-evidence/1` has no digest member (Canon 0.10.0 `crates/canon/src/model/evidence.rs:17-53`).

## ESS first

`ess/domains/tree.yaml` (created by `story:tree-run-command`): `TreeSuite` (format, defaults,
cycles), `SuiteDefaults`, `SuiteCycle`, `SeedEntry` (from, to), `CycleCondition`, `CycleStep`
(`do` as the enum `StepKind` with all twelve kinds, name, at, exit, refusal, engine, rounds,
checks and a scripted decision's members), `ExitExpectation`, `ScriptedDecision` (a union on
`take`: `approve`, `close`, `decide`), `CycleExpectation` (exit, outcome, stop, checks),
`StopExpectation`, `SuiteCheck` (`check` as the enum `CheckKind`, name, select, the kinds'
members, each optional: the decoder refuses one the kind does not take), `SelectItem` (one
optional member per source and operation; the decoder takes exactly one), `SourceKind` (all ten
sources), `CaseSelector`, `Conditions`, `Condition`, `Operand`, `CheckDifference` (check, kind,
expected, found), `SuiteReport` (cycles, expected, as_expected, failed, pending, pending_passing,
waiting, not_run), `CycleResult` (name, pending, waits_for, run, as_expected, difference, exit,
seconds), and `SuiteRefusal` with `SuiteRefusalReason` (the eighteen refusals above). No command
is declared, so no conformance scenario is added. On that commit `task drift` fails and
`crates/loom-tree/tests/suite_format.rs` does not compile.

## Acceptance

Fixtures are built once per test run, not per test: each is made in a temporary directory under
`target/` and renamed into place, keyed by the SHA-256 of its sources, so concurrent nextest
processes read only a finished one (a `OnceLock` would build it once per test).

`crates/loom-tree/tests/suite_format.rs`, a module of `crates/loom-tree/tests/main.rs`; library
tests, no tree runs, no program starts. The cached fixture is one end state (a store of a root
and two children, written with the writers the tree uses: `ActionStarted`, `ActionFinished` with
a blob, `Stopped` and `Terminated` lines, two records, two program-run observations, one with
`retry_of`; a workspace with a `.yaml`, a `.jsonl`, a text file and a directory of files) and one
catalog (`tree-two-level` of `story:tree-run-command`). The suite
`crates/loom-tree/tests/fixtures/suite-format/kinds.suite.yaml` (Loom's own; no file from
Runbook's repository) holds, per check kind, one cycle whose checks hold and one whose check
fails:

- Every cycle of the passing half is as expected against the end state (test
  `each_check_kind_holds`).
- Every cycle of the failing half is not, and names its check, its kind and the expected value
  (test `each_check_kind_fails_by_name`, one assertion per kind, thirteen).
- `before` fails when `between_none` holds between the two values, and `all` fails on an empty
  selection (test `order_and_quantifiers_mean_what_they_say`).
- A path with `[id=q2]`, `[*]`, `[-1]`, a quoted name with a `.`, and a path that reaches
  nothing give the values written in the test (test `paths_select_what_they_name`).
- `retry_of[*]` of the second run, followed to its evidence, gives the record kind of the first
  run's record (test `a_run_names_the_records_it_retries`).
- Over four records of Loom's own fixture shaped as a reworked bug fix, received in this order
  (`repro_run` `red`, subject `repro` at `r1`, upstream `change: c1`; `test_run` `failed`, subject
  `change` at `c1`, upstream `repro: r1`; `implementation_change` and `test_run` `passed`, subject
  `change` at `c2`, upstream `repro: r1`), the nine `red_then_green` checks hold, the `failed`
  `test_run` never being C. Each condition fails alone: with the `passed` `test_run` received
  before the `implementation_change`, the second `before`; with C's upstream `repro: r0`, the
  first `compare`; with B's `repro: r0`, the second; with B at `c3`, the third; with C's
  `subject_revision` `none`, its `all`; with C at `c1`, the `compare` `ne`; with no `repro_run`,
  the first `before`, and the first `compare` is `OperandNotOne` naming `right` and 0 (test
  `a_green_names_the_red_it_follows`).
- `follow: blob` reads `blobs/<sha256>`, and a blob whose bytes do not hash to its name gives
  `BlobChanged`; a document value `../../x` under `follow: file` gives `PathOutsideCycle` (test
  `follow_stays_in_the_cycle`).
- `compare` with `after: s1` reads the state saved after step `s1`, and `plus: 1` adds one (test
  `operands_read_the_state_after_a_step`).
- A found value that is a workspace text prints as its length and digest; an event kind prints
  as itself (test `differences_never_print_program_text`).
- With `defaults: {bindings: b.yaml, fixtures: f0, decide: [d0]}`, a cycle stating nothing takes
  `b.yaml` and `f0`; a cycle stating `fixtures: f1` takes `f1` and `b.yaml`; a cycle's own
  `decide` entries come before `d0`; with no `bindings` anywhere the cycle is `MemberMissing`
  naming `cycles[<i>].bindings` (test `cycle_members_default_from_the_suite`).
- A suite with an undeclared member, `defaults.expect`, a select list of two sources, a regex
  with a back-reference, `after` naming a later step, a path with an unclosed `[`, `do: rewind`,
  a source `logs` and an event kind `Rewound` are each refused with their reason and member path;
  a `bindings` of `../x.yaml`, a seed `to` of `../w`, a cycle named `A b` and two named `one` are
  `PathOutsideSuite`, `PathOutsideCycle`, `CycleNameUnusable` and `CycleTwice` (test
  `a_refused_suite_names_its_member`).
- With a table that marks `replay` as waiting for `story:tree-replay-command`, a cycle with a
  `replay` step is `pending`, `waits_for` names that story, and it is counted neither failed nor
  expected (test `a_waiting_kind_makes_the_cycle_pending`).
- A `when` on an undeclared action is false and one on a declared action true (test
  `when_reads_the_catalog_and_the_suite`).

`crates/loom-cli/tests/tree_test.rs`, a module of `crates/loom-cli/tests/main.rs`, runs the built
`b10x-loom tree test` over `crates/loom-cli/tests/fixtures/tree-suite/`: the `tree-two-level`
protocol, stub programs that sleep 0.3 s, and a suite of five cycles: `arrives` (expected, exit
0, outcome `arrived`, a `records` and a `runs` check), `waits` (expected, exit 3, no entry for the
second approval), `wrong` (expected, outcome `arrived`, its stub fails the ticket), `later`
(pending, failing), `replays` (expected, with a `replay` step). The output of one run is cached
under `target/` as above; no model, no network:

- The output holds `arrives: as expected` and `waits: as expected` with their seconds, `wrong:
  FAILED` naming `outcome`, expected `arrived`, found `turned_back`, and `replays: pending, waits
  for story:tree-replay-command (replay)` (test `the_first_runner_reports_each_cycle`).
- The summary reads `cycles: 3 expected, 2 as expected, 1 failed: wrong` and `pending: 2 (1 still
  failing: later; passing, move to expected: none; 1 waiting: replays on
  story:tree-replay-command)`, the exit is 1, and `--report` holds the same counts (test
  `the_summary_counts_expected_pending_and_waiting`).
- Every store passes `tree verify`, nothing is written outside `--scratch`, and a non-empty
  `--scratch` exits 2 before any protocol is read (test `the_first_runner_stays_in_scratch`).
- A stub that prints a canary and writes it into a workspace file a check reads leaves no canary
  in the output or the report (test `program_output_never_enters_the_summary`).
- The fixture run takes under 10 s; each test asserts its time, none reaches 5 s;
  `slot cargo nextest run -p b10x-loom-tree suite_format::` and
  `slot cargo nextest run -p b10x-loom-cli tree_test::` pass.
- Measured by the implementor once, not a test, on the converted copy of Runbook's file that
  Runbook supplies (no file of it enters Loom's repository), and written in the merge request:
  the copy decodes; the wall time of `stub-cycle` alone; the counts as expected, failed, pending
  and waiting per story.
- `website/docs/guides/run-a-case-tree.md` lists the sources, operations, check kinds and the
  availability table, with one example each; `website/docs/reference/cli.md` is regenerated.
  `task check` exits 0.

## Out of scope

- Cycles at once, the cycle kill at `--cycle-timeout`, the ten slowest, `--list`, the
  `requests` source and `decide`: `story:tree-test-command`. The kinds marked waiting: the
  stories named under "Availability".
- A query language, arithmetic between two read values, a walk over revision moves, reading
  console text.
- Editing a store or a workspace between steps; preparing inputs with a program; a cycle starting
  from another cycle's end state.
- Runbook's conversion of `cycles/suite.yaml` and the reason per cycle for what it drops.

## Scope

Derived 2026-11-22 by the drafting agent from `882c1866`, the stories named, Runbook's
`cycles/suite.yaml` at `5971c50` and `impl/bound-program-retry-records` at `db4d2d71`
(read-only). Every line is **cited** or **inferred**.

- **Primary surface:** `crates/loom-tree`, new modules `src/suite/decode.rs` (document,
  defaults, refusals), `src/suite/select.rs` (sources, paths, operations), `src/suite/check.rs`
  (the kinds, operands, differences), `src/suite/availability.rs` (`SUITE_AVAILABILITY`) and
  `src/suite/run.rs` (the first runner, scripted entries, output, report) — inferred: the crate
  exists only after `story:tree-run-command`
- **Files:** `ess/domains/tree.yaml` (the suite and report types) — inferred, created by
  `story:tree-run-command`
- **Files:** `crates/loom-protocols/src/lib.rs` (`registration_for` :315-324 made public, so
  one catalog serves every cycle by registration) — cited; the tree functions of
  `story:tree-run-command` take that catalog — inferred
- **Files:** `crates/loom-cli/src/lib.rs` (`tree test`, its flags, help and exit table),
  `crates/loom-cli/src/main.rs` (dispatch) — inferred: the `tree` group exists only after
  `story:tree-run-command`
- **Reads:** the run log reader (`crates/loom-runlog`, `docs/contracts/run-log-v1.json`), the
  store's `observations.json` (`crates/loom-governor/src/file_store.rs:6-7`, `:80`) and the
  payload `program.rs` writes, the case files and `tree status` of `story:tree-run-command`,
  `ProtocolCatalog::add_yaml` (`crates/loom-protocols/src/lib.rs:241-280`) — cited for all but
  `tree status`, inferred
- **Files:** `crates/loom-tree/tests/main.rs` (`mod suite_format;`),
  `crates/loom-tree/tests/suite_format.rs`, `crates/loom-tree/tests/fixtures/suite-format/`,
  `crates/loom-cli/tests/main.rs` (`mod tree_test;`), `crates/loom-cli/tests/tree_test.rs`,
  `crates/loom-cli/tests/fixtures/tree-suite/` — inferred, names proposed here
- **Generated:** `generated/rust/loom/` (`loom::tree`), `website/docs/reference/cli.md` — cited
- **Documents:** `website/docs/guides/run-a-case-tree.md`, `CHANGELOG.md` — inferred
- **Symbols:** `TreeSuite`, `SuiteCycle`, `CycleStep`, `StepKind`, `SuiteCheck`, `CheckKind`,
  `SelectItem`, `SourceKind`, `Operand`, `CheckDifference`, `SuiteReport`, `CycleResult`,
  `SuiteRefusalReason`, `SUITE_AVAILABILITY`, `decode_suite`, `evaluate_checks`, `run_suite` —
  inferred
- **Not changed:** `crates/loom-bindings`, `crates/loom-decision`, `crates/loom-governor`,
  `ess/system.yaml`, `ess/ess-inputs.yaml` — inferred
- **Would collide with:** in the planner's wave 3, `story:person-actions` (`crates/loom-tree`,
  `crates/loom-cli/src/lib.rs`), N1 (`crates/loom-tree`), `story:command-invocation-contract`
  (its first `cli-v1.json` snapshot then holds `tree test`); rule 2 of § 1.1 (new modules and
  types only, the second merges the first) — inferred
