---
title: Run a tree of cases
sidebar_position: 7
description: Open a tree of protocol cases with b10x-loom tree open, drive it with tree run, answer what it waits on with tree approve or tree close, and read and check it with tree status and tree verify.
lede: A tree of cases lives in one store directory. tree run takes one step of one case at a time through the action's bound executor until the root ends or the tree waits on a person, and it prints the exact command that answers each wait.
source: crates/loom-tree/src/lib.rs, crates/loom-tree/src/run.rs, crates/loom-tree/src/approve.rs, crates/loom-tree/src/close.rs, crates/loom-tree/src/status.rs, crates/loom-tree/src/log.rs, crates/loom-cli/src/tree.rs, crates/loom-cli/tests/tree_run.rs, crates/loom-tree/src/suite/mod.rs, crates/loom-tree/src/suite/run.rs, crates/loom-cli/tests/tree_test/mod.rs, ess/domains/tree.yaml
---

# Run a tree of cases

Unreleased. `b10x-loom tree` keeps a tree of protocol cases in one store directory: `tree open`
opens the root case, `tree run` drives every case of the tree, `tree approve` and `tree close`
answer what a run waits on, and `tree status` and `tree verify` read the store.

`tree run` confines every bound program by default (`--confinement substrate`), which needs
**Linux, bubblewrap and a delegated cgroup v2**. Without `--cgroup-root`, `tree run` starts itself
again in a scope of its own under `systemd-run --user --scope -p Delegate=yes`, so two runs never
share a cgroup root. `tree run --cgroup-root DIR` names the root instead: an empty, writable,
delegated cgroup v2 directory with the run in a child of it, held by one `tree run` at a time.
Where neither is possible, `--confinement local` runs every program unconfined, as the examples
below do.

The example is the fixture of `crates/loom-cli/tests/tree_run.rs`: the protocol `tour.leg`
(registered `tour-leg@1`), one leg of a walking tour, and the bindings files and stub programs of
`crates/loom-cli/tests/fixtures/tree-two-level/`, copied into the directory `~/tour`, which is
the workspace. A planner stub adds a stop to `route.txt` while a check finds the route short, a
guide approves the route and a ticket is issued under the capability `guide.issue`. The output
below is pasted from that run. Where a command ends in `| jq -c`, the filter shortens the one JSON
object `b10x-loom` prints; `(exit N)` is the exit status of `b10x-loom`.

## Open the tree

```console
$ b10x-loom protocols add tour-leg@1 --file crates/loom-cli/tests/fixtures/tree-two-level/protocols/tour-leg.yaml
installed tour-leg@1
(exit 0)
```

```console
$ b10x-loom tree open --protocol tour-leg@1 --store store --bindings tour-leg.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"tour-leg@1","root":"root","store":"~/tour/store","workspace":"~/tour"}
(exit 0)
```

`tree open` reads everything before it writes: the store must be absent or empty, the workspace
an existing directory, and every bindings file of the tree must decode and check against its
protocol. A root's bindings file names each child's file in the `bindings` member of an `open`
executor, relative to its own directory. A binding with an `agent` executor anywhere in the tree
is refused before the store is made.

## Run it until it waits

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:01:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":5,"detail":null,"decisions":[{"kind":"ApproveAction","decides":"route.approve of case root at revision 3"}]}
b10x-loom: tree waits: case root: route.approve of case root at revision 3
  run: b10x-loom tree approve --store ~/tour/store --case root --capability guide.approve --principal guide --action route.approve
(exit 3)
```

`tree run` takes one step at a time with the deterministic selector, every open child before its
parent's next step, and performs it through the action's bound executor: here the check, then the
planner, which the check's `invalid` result sends back until the route holds two stops. It stops
at a terminal root (exit 0), when it waits on a person or on an approval (exit 3), when a case is
stuck (exit 4) or at `--max-steps` or `--max-wall-seconds` (exit 5). Each wait names the case,
what is decided and the command that decides it.

A host that runs the tree as a library (`run_tree`) can pass a cancel flag and set it from any
thread. Set between steps, the run stops `Cancelled` before the next one, its `stopped` line
naming the action that was to be taken next. Set during a step, the step's program is killed with
every process it started, the step's `action finished` line holds the refusal `Cancelled: …` and
no `exit` or `evidence`, and the run stops `Cancelled` naming that action, within 10 s of the
flag. `tree status` shows the case's `stop` `Cancelled`, its `stop_next`, and `cancelled: true` on
the cancelled run; a cancelled run counts against the action's `budget.max_runs`.

`b10x-loom tree run` sets that flag on SIGTERM or SIGINT (a supervisor's stop, a terminal's
Ctrl-C): the running program is stopped with every process it started and the run ends `Cancelled`
with exit 5, so no bound program outlives it. A later signal changes nothing. A `tree run` that
started itself again in a scope of its own handles no signal itself; the run inside the scope does.

## Answer what it waits on

`route.approve` is bound to the person `guide`: `tree approve --action` records the approval and
then takes the action as that principal, so the record its binding declares is produced by the
guide about the route's revision now.

```console
$ b10x-loom tree approve --store store --case root --capability guide.approve --principal guide --action route.approve --at 2026-11-22T08:02:00Z
{"at":"2026-11-22T08:02:00Z","capability":"guide.approve","case":"root","case_revision":3,"decision":"Allow","principal":"guide"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:03:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingAuthority","steps":0,"detail":null,"decisions":[{"kind":"Approve","decides":"ticket.issue of case root at revision 3"}]}
b10x-loom: tree waits: case root: ticket.issue of case root at revision 3
  run: b10x-loom tree approve --store ~/tour/store --case root --capability guide.issue --principal operator
(exit 3)
```

`ticket.issue` is a program that needs the capability `guide.issue`. `tree approve` without
`--action` records the grant for the case at its current revision; the next run issues the ticket
and the case ends through its one legitimate outcome:

```console
$ b10x-loom tree approve --store store --case root --capability guide.issue --principal operator --at 2026-11-22T08:04:00Z
{"at":"2026-11-22T08:04:00Z","capability":"guide.issue","case":"root","case_revision":3,"decision":"Allow","principal":"operator"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store store --at 2026-11-22T08:05:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"Terminal","steps":1,"detail":"passed","decisions":[]}
b10x-loom: tree run: Terminal: case root: passed
(exit 0)
```

## Read the tree

```console
$ b10x-loom tree status --store store | jq -c '.cases[] | {case, protocol, outcome}, (.runs[] | {action, attempt, exit}), (.approvals[] | {capability, action, actor}), (.evidence[] | {kind, result, producer})'
{"case":"root","protocol":"tour-leg@1","outcome":"passed"}
{"action":"route.check","attempt":1,"exit":1}
{"action":"route.write","attempt":1,"exit":0}
{"action":"route.check","attempt":1,"exit":1}
{"action":"route.write","attempt":2,"exit":0}
{"action":"route.check","attempt":1,"exit":0}
{"action":"ticket.issue","attempt":1,"exit":0}
{"capability":"guide.approve","action":"route.approve","actor":"guide"}
{"capability":"guide.issue","action":null,"actor":"operator"}
{"kind":"route_check","result":"invalid","producer":"route-check"}
{"kind":"route_shape","result":"short","producer":"route-check"}
{"kind":"route_check","result":"invalid","producer":"route-check"}
{"kind":"route_shape","result":"short","producer":"route-check"}
{"kind":"route_check","result":"valid","producer":"route-check"}
{"kind":"route_shape","result":"two-stops","producer":"route-check"}
{"kind":"route_approval","result":"approved","producer":"guide"}
{"kind":"ticket_issued","result":"issued","producer":"ticket-office"}
(exit 0)
```

`tree status` prints `loom.tree-status/1`: per case its link, protocol, outcome or stop, its
records in the order received, its program runs with their attempts, its approvals and the records
a `copy` binding copied into it, whether its run log verified (`log`; a log that does not is named
with the refusal, the line and what is wrong, and nothing is read from it) and, after a
`BudgetSpent` stop, the budget that ended the run (`budget`: `MaxSteps` or `MaxWallSeconds`; a
cancelled run stops `Cancelled` and names none, and `Cancel` is read only from a log an earlier
Loom wrote). `protocols` lists each protocol the cases use once, with the SHA-256 of its document. `tree status` and `tree verify` read the store and never hold it, so a live view can read
the tree while a run holds it.

## Choose between two outcomes

A case for which more than one outcome is legitimate is not ended by `tree run`. The fixture
protocol `fork` has the outcomes `kept` and `left`, both legitimate once the road is seen:

```console
$ b10x-loom protocols add fork@1 --file crates/loom-cli/tests/fixtures/tree-two-level/extra/fork.yaml
installed fork@1
(exit 0)
```

```console
$ b10x-loom tree open --protocol fork@1 --store fork-store --bindings fork.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"fork@1","root":"root","store":"~/tour/fork-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store fork-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":1,"detail":null,"decisions":[{"kind":"Close","decides":"close case root through kept"},{"kind":"Close","decides":"close case root through left"}]}
b10x-loom: tree waits: case root: close case root through kept
  run: b10x-loom tree close --store ~/tour/fork-store --case root --outcome kept
b10x-loom: tree waits: case root: close case root through left
  run: b10x-loom tree close --store ~/tour/fork-store --case root --outcome left
(exit 3)
```

```console
$ b10x-loom tree close --store fork-store --case root --outcome left --at 2026-11-22T08:00:00Z
{"at":"2026-11-22T08:00:00Z","case":"root","outcome":"left"}
(exit 0)
```

An outcome that is not legitimate for the case now is refused `OutcomeNotLegitimate`, and the
store keeps every byte.

## A writer's handoff

A writer whose binding names a `handoff` hands the case to a person when it has done what it may:
when no step is useful and nothing awaits approval, the run prints the handoff text as the
decision and `tree run` as its command. In `tour-leg-handoff.bindings.yaml` the planner writes
nothing and nothing sends it back:

```console
$ b10x-loom tree open --protocol tour-leg@1 --store handoff-store --bindings tour-leg-handoff.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"tour-leg@1","root":"root","store":"~/tour/handoff-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store handoff-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, decides}]}'
{"stop":"AwaitingPerson","steps":2,"detail":null,"decisions":[{"kind":"Handoff","decides":"Add a second stop to route.txt, then run the tree again."}]}
b10x-loom: tree waits: case root: Add a second stop to route.txt, then run the tree again.
  run: b10x-loom tree run --store ~/tour/handoff-store --confinement local
(exit 3)
```

The printed `tree run` carries this run's `--confinement`, so run unchanged it starts its programs
as this run did.

A writer whose handoff depends on a fact only a program can read names that program as
`handoff_when` beside its `handoff` (`tour-leg-handoff-when.bindings.yaml` in the test fixtures).
When the selector picks the writer, the check runs first, under the same runner, with the whole
workspace read-only, `PATH` and the `LOOM_*` variables alone, no retry feedback and the binding's
timeout. Exit 0 takes no step: the run stops `AwaitingPerson` with the writer's `Handoff` decision
(exit 3). Any other exit lets the writer run in the same step. At a stuck stop the handoff of such
a writer is offered only when its check, run the same way, exits 0; otherwise the stop stays
`Stuck` (exit 4). A check that cannot be started, is killed at its timeout or by a signal refuses
the run `HandoffCheckFailed`, naming the action (exit 1), and the writer does not run. Each
evaluation writes one `HandoffChecked` line to the case's run log before the stop or the step,
with the check's `exit` or `refusal` and the `blob_digest` of its output; `tree status` and
`tree verify` read that line and never run the check.

## A person action without a capability

A person action whose protocol action needs no capability is taken by `tree approve --action`
without `--capability`, and no authority is recorded. In the fixture protocol `sign`, the walker
signs a post with no capability, the clerk seals it under `clerk.seal`, and a stamp program needs
`clerk.stamp`; all three wait at once:

```console
$ b10x-loom protocols add sign@1 --file crates/loom-cli/tests/fixtures/tree-two-level/extra/sign.yaml
installed sign@1
(exit 0)
```

```console
$ b10x-loom tree open --protocol sign@1 --store sign-store --bindings sign.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"sign@1","root":"root","store":"~/tour/sign-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store sign-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, detail, decisions: [.decisions[] | {kind, action, capability, principal}]}'
{"stop":"AwaitingPerson","steps":0,"detail":null,"decisions":[{"kind":"ApproveAction","action":"post.seal","capability":"clerk.seal","principal":"clerk"},{"kind":"ApproveAction","action":"post.sign","capability":null,"principal":"walker"},{"kind":"Approve","action":"post.stamp","capability":"clerk.stamp","principal":"operator"}]}
b10x-loom: tree waits: case root: post.seal of case root at revision 1
  run: b10x-loom tree approve --store ~/tour/sign-store --case root --capability clerk.seal --principal clerk --action post.seal
b10x-loom: tree waits: case root: post.sign of case root at revision 1
  run: b10x-loom tree approve --store ~/tour/sign-store --case root --principal walker --action post.sign
b10x-loom: tree waits: case root: post.stamp of case root at revision 1
  run: b10x-loom tree approve --store ~/tour/sign-store --case root --capability clerk.stamp --principal operator
(exit 3)
```

Each `tree approve` decision names the principal its command records as `principal`. For a person
action it is the person the binding names, the only principal `tree approve --action` accepts.
For a capability it is `operator`, a default that restricts nothing. A close and a handoff name
none.

```console
$ b10x-loom tree approve --store sign-store --case root --principal walker --action post.sign --at 2026-11-22T08:01:00Z
{"action":"post.sign","case":"root","principal":"walker"}
(exit 0)
```

An action that needs a capability is refused without one, and nothing is written:

```console
$ b10x-loom tree approve --store sign-store --case root --principal clerk --action post.seal --at 2026-11-22T08:01:00Z
b10x-loom: tree refused: NotAPersonAction: case "root": action "post.seal": the action needs the capability clerk.seal; name it with --capability
{"action":"post.seal","case":"root","detail":"the action needs the capability clerk.seal; name it with --capability","reason":"NotAPersonAction"}
(exit 1)
```

Without `--action`, `--capability` is required.

## A named result and its reasons

`tree approve --action A --result R` ends a person action with a result other than `performed`:
each `from: effect` entry of `A`'s binding records the result its `results` table maps `R` to.
`R` must be a value every such entry lists; any other is refused and nothing is written.
`--reasons TEXT` or `--reasons-file FILE` (a regular file inside the workspace, no link followed)
give the person's reasons, at most 65536 bytes and 60 lines. They are kept as a blob of the
store, never in a run-log line, and a writer whose `retry_when` lists the result is sent back
with them. In the fixture protocol `review`, the reviewer's table lists `performed` (mapped to
`approved`) and `rejected`, and the writer's `retry_when` lists `rejected`; its stub copies
`LOOM_RETRY_FEEDBACK` to `feedback.json`:

```console
$ b10x-loom protocols add review@1 --file crates/loom-cli/tests/fixtures/tree-two-level/extra/review.yaml
installed review@1
(exit 0)
```

```console
$ echo draft > draft.txt
$ b10x-loom tree open --protocol review@1 --store review-store --bindings review.bindings.yaml --workspace . --at 2026-11-22T08:00:00Z
{"protocol":"review@1","root":"root","store":"~/tour/review-store","workspace":"~/tour"}
(exit 0)
```

```console
$ b10x-loom tree run --confinement local --store review-store --at 2026-11-22T08:00:00Z | jq -c '{stop, steps, decisions: [.decisions[] | {kind, action}]}'
{"stop":"AwaitingPerson","steps":0,"decisions":[{"kind":"ApproveAction","action":"draft.judge"},{"kind":"ApproveAction","action":"draft.review"}]}
b10x-loom: tree waits: case root: draft.judge of case root at revision 1
  run: b10x-loom tree approve --store ~/tour/review-store --case root --principal reviewer --action draft.judge
b10x-loom: tree waits: case root: draft.review of case root at revision 1
  run: b10x-loom tree approve --store ~/tour/review-store --case root --principal reviewer --action draft.review
(exit 3)
```

```console
$ b10x-loom tree approve --store review-store --case root --principal reviewer --action draft.review --result lost --at 2026-11-22T08:01:00Z
b10x-loom: tree refused: NotAPersonAction: case "root": action "draft.review": the draft_review entry lists no result lost; it lists performed, rejected
{"action":"draft.review","case":"root","detail":"the draft_review entry lists no result lost; it lists performed, rejected","reason":"NotAPersonAction"}
(exit 1)
```

```console
$ b10x-loom tree approve --store review-store --case root --principal reviewer --action draft.review --result rejected --reasons 'The draft has one line. Add a second.' --at 2026-11-22T08:01:00Z
{"action":"draft.review","case":"root","principal":"reviewer"}
(exit 0)
```

The next run sends the writer back; its feedback holds the reviewer's record, with the action and
no exit, and the reasons as that record's log:

```console
$ b10x-loom tree run --confinement local --store review-store --at 2026-11-22T08:02:00Z | jq -c '{stop, steps, decisions: [.decisions[] | {kind, action}]}'
{"stop":"AwaitingPerson","steps":1,"decisions":[{"kind":"ApproveAction","action":"draft.judge"},{"kind":"ApproveAction","action":"draft.review"}]}
b10x-loom: tree waits: case root: draft.judge of case root at revision 2
  run: b10x-loom tree approve --store ~/tour/review-store --case root --principal reviewer --action draft.judge
b10x-loom: tree waits: case root: draft.review of case root at revision 2
  run: b10x-loom tree approve --store ~/tour/review-store --case root --principal reviewer --action draft.review
(exit 3)
```

```console
$ jq -c '.records[] | {kind, result, action, exit}' feedback.json
{"kind":"draft_review","result":"rejected","action":"draft.review","exit":null}
$ jq -c '.logs[] | {tail}' feedback.json
{"tail":"The draft has one line. Add a second."}
```

`--result` needs `--action` and conflicts with `--deny`, which records a denial and takes no
action; `--reasons` and `--reasons-file` need `--result` and conflict with each other.

## Verify the run logs

```console
$ b10x-loom tree verify --store store
{"cases":1}
(exit 0)
```

Each log is checked against its chain and its last line against the head the tree kept beside it,
so a change to any byte is named with the case and the line. After `"exit":1` became `"exit":0`
in the line with `seq` 2:

```console
$ b10x-loom tree verify --store store
{"cases":1,"failure":{"case":"root","detail":"BrokenChain: prev is not the SHA-256 of line 2: the line before it changed","line":3}}
b10x-loom: tree verify: case root line 3: BrokenChain: prev is not the SHA-256 of line 2: the line before it changed
(exit 3)
```

## One writer

A command that writes holds `tree.lock` in the store for the whole command. A second one is
refused `StoreInUse`, names the process that holds the store and writes nothing.

## Test a tree with a suite of cycles

A suite of scripted tree cycles is a `loom.tree-suite/1` document. `b10x-loom tree test` runs its
cycles one after another, each in its own directory `<scratch>/<n>/` (`store/` and `work/`), each
step through the function of its `tree` command in the same process, and prints one line per
cycle and two summary lines. The suite below is
`crates/loom-cli/tests/fixtures/tree-suite/suite.yaml`; its paths (`bindings`, the `from` of a
seed) are below the suite's directory, and a seed's `to` is below the cycle's workspace.

```yaml
format: loom.tree-suite/1
defaults:
  protocol: tour@1
  bindings: bindings/tour.bindings.yaml
  seed: [{from: stubs, to: stubs}, {from: seeds/route.txt, to: route.txt}]
cycles:
  - name: arrives
    decide:
      - {take: approve, capability: guide.approve, times: any}
      - {take: approve, capability: guide.issue, times: any}
    expect:
      exit: 0
      outcome: arrived
      checks:
        - check: count
          name: two-tickets-run
          select: [{runs: {case: "*"}}, {where: {action: ticket.issue}}]
          is: 2
  - name: waits
    decide:
      - {take: approve, capability: guide.approve}
    expect:
      exit: 3
      stop: AwaitingAuthority
```

```console
$ b10x-loom tree test --suite suite.yaml --protocols protocols --scratch scratch \
    --at 2026-11-22T08:00:00Z --confinement local --report report.json
arrives: as expected (9.0 s)
waits: as expected (1.9 s)
wrong: FAILED: expect.outcome (outcome, OutcomeDiffers): expected arrived, found turned_back (3.3 s)
later: pending, still failing: check-log-says-hello (equals, CheckFailed): expected hello, found 26 bytes, sha256 a77f6b1ee3b7 (1.8 s)
replays: pending, waits for story:tree-replay-command (replay)
cycles: 3 expected, 2 as expected, 1 failed: wrong
pending: 2 (1 still failing: later; passing, move to expected: none; 1 waiting: replays on story:tree-replay-command)
(exit 1)
```

`--scratch` must be absent or empty, and nothing is written outside it but `--report`, the same
result as one `loom.tree.SuiteReport` JSON document. The protocols of `--protocols` are read once,
as `tree open --protocols` reads them, and every cycle's tree keeps them. The exit status is 0
when every cycle that ran and is not pending ended as expected, 1 when one did not, and 2 when the
command line, the suite, the protocols, a seed or the scratch directory is refused before any
cycle; the refusal names its reason and the member of the suite (`cycles[2].steps[0].do`).

At an exit 3 of `tree run`, every printed wait is answered by the first `decide` entry, in list
order, that matches it and has a use left: `approve` by `case` (an id, or its last part),
`capability` and `action`, `close` by `case` and `outcome`; `times` is a count (default 1) or
`any`. The rounds go on until another exit, a wait no entry answers, or `max_rounds`
(`RoundsSpent`). An entry that is not `optional` and was never used is the difference
`ScriptUnused`. Each cycle's `tree run` rounds share 60 s of wall time.

A cycle states `name`, and any of `pending`, `when`, `protocol`, `bindings`, `engine`,
`fixtures`, `revisions`, `seed`, `principal`, `max_rounds` (default 40), `decide`, `steps`
(default `[{do: open}, {do: run}]`) and `expect`. `defaults` holds any of `protocol` to `steps`; a
stated member replaces the default whole, and a cycle's own `decide` entries are tried before the
default ones.

### Steps

| `do` | What it runs |
|---|---|
| `open` | `tree open` of the cycle's protocol and bindings into its store |
| `run` | rounds of `tree run`, each exit 3 answered from `decide`; `rounds: 1` runs one |
| `approve` | `tree approve` (`case`, `capability` or `action`, `principal`) |
| `close` | `tree close` (`case`, `outcome`) |
| `status`, `verify` | `tree status`, `tree verify`; a named one gives its JSON output to `{step: <name>}` |

A step states `exit` (a status, or `{not: <status>}`), `refusal`, `at` (minutes after `--at`) and
`checks`, evaluated right after it.

### Sources

| Source | Example | Gives |
|---|---|---|
| `log` | `{log: {case: first, event: ActionFinished}}` | a case's run log lines in `seq` order |
| `records` | `{records: {case: ".", kind: test_run}}` | the `canon-evidence/1` records a case holds |
| `runs` | `{runs: {case: first}}` | the program runs of a case (`exit`, `blob_sha256`, `retry_of`, …) |
| `cases` | `{cases: {protocol: tour-leg}}` | the cases of `tree status` |
| `doc` | `{doc: {in: work, path: findings.yaml}}` | one file, read by its extension |
| `files` | `{files: {in: work, glob: "legs/*.yaml"}}` | the relative paths that exist, in byte order |
| `step` | `{step: verified}` | the JSON output of a named `status` or `verify` step |
| `suite` | `{suite: expected/lib.rs}` | a file of the suite's directory |

A case is a path below the root (`.` is the root, `billing.invoices` is `<root>.billing.invoices`),
`{protocol: <id>}`, `{name: <last segment>}` or `"*"`.

### Operations

| Operation | Example |
|---|---|
| `where` | `{where: {action: route.check, exit: {min: 1}}}` |
| `pick` | `{pick: last}`, `{pick: 0}`, `{pick: -1}` |
| `path` | `{path: "findings[id=q2].status"}`, `{path: "retry_of[*]"}` |
| `follow` | `{follow: blob}`, `{follow: evidence}`, `{follow: file}`, `{follow: {file: notes.txt}}` |
| `as` | `{as: lines}` |

A condition maps a path to a scalar it equals, or to one of `equals`, `not_equals`, `in`,
`not_in`, `contains`, `lacks`, `matches`, `not_matches`, `min`, `max`, `type`, `length`,
`exists`; `any_of` holds when one of its groups does.

### Check kinds

| Kind | Example |
|---|---|
| `exists` | `{check: exists, select: [{doc: {in: work, path: ticket.txt}}]}` |
| `count` | `{check: count, select: [{log: {case: ".", event: ChildOpened}}], is: 2}` |
| `equals` | `{check: equals, select: [{doc: …}, {path: status}], is: changed}` |
| `contains` | `{check: contains, select: [{doc: {in: work, path: notes.txt}}], is: second line}` |
| `lacks` | `{check: lacks, select: [{doc: {in: work, path: notes.txt}}], is: offline-hosted}` |
| `matches` | `{check: matches, select: […], regex: "^the "}` |
| `not_matches` | `{check: not_matches, select: […], regex: customer}` |
| `all` | `{check: all, select: [{log: {case: "*", event: CaseOpened}}], where: {protocol: {matches: "^tour"}}}` |
| `none` | `{check: none, select: [{runs: {case: first}}], where: {timed_out: true}}` |
| `some` | `{check: some, select: [{runs: {case: first}}], where: {exit: 0}}` |
| `sequence` | `{check: sequence, select: [{log: {case: "."}}, {path: kind}], ends: [Stopped, Terminated]}` |
| `before` | `{check: before, select: [{records: {case: "."}}], first: {kind: repro_run, result: red}, then: {kind: implementation_change}}` |
| `compare` | `{check: compare, left: {select: […], take: count}, op: eq, right: {value: 2}}` |

A difference names the check, its kind and the expected value. What was found is printed when it
is a number, a boolean or a value Loom declares; any other text is printed as its length and the
first 12 hex digits of its SHA-256, so a program's output never reaches a summary line.

### Availability

`SUITE_AVAILABILITY` marks what this binary runs. A cycle that uses a part that waits is printed
`<name>: pending, waits for story:<id> (<kind>)` and counted under `pending`.

| Part | Waits for |
|---|---|
| steps `open`, `run`, `approve`, `close`, `status`, `verify`; entries `approve`, `close`; every source but `requests` and `calls`; every check kind; `when` | available |
| `stop`, `resume`, `allow`, `reopen`; `approve` with `result`, `reasons` or `deny`; events `StopRecorded`, `StopLifted`, `Labelled` | `story:tree-stop-record-and-labels` |
| `replay` | `story:tree-replay-command` |
| `calls` | `story:agent-node-run-records` |
| `engine` | `story:engine-configuration-document` |
| `fixtures`; event `RunSettings` | `story:engine-agents-section` |
| `decide` (step and entry); `requests` | `story:tree-test-command` |

### From Rust

`b10x_loom_tree::suite` holds the same runner as a library: `decode_suite(text)` or
`read_suite(path)` decode a suite, `evaluate_checks(checks, place, view, captured)` tests checks
over what a cycle left, and `run_suite(suite, &SuiteSettings, observer, cancel)` runs one and
answers a `SuiteReport`. It prints nothing and reads no environment variable and no current
directory: every path of `SuiteSettings` is absolute. The observer, a `SuiteObserver` or a
closure over `&CycleResult`, is given each cycle's result as the cycle ends; a set cancel flag
ends the running cycle before its next step and runs no further one.
