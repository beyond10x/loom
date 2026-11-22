---
format: aep.planning-md/3
id: story:person-action-runs-a-program
kind: story
status: active
title: 'A person action''s approval can run a program: the binding''s argv runs confined, the approving principal as one whole argv entry, before the person''s records'
summary: person binding may name argv; tree approve records the approval, runs the program through the program port's run path, writes the person records on exit 0 only; {principal} is a whole argv entry; failure refuses PersonProgramFailed and keeps the approval; runs count against max_runs
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:person-action-records-a-named-result
- depends_on: story:bound-program-contract
- depends_on: story:cancel-a-running-bound-program
- depends_on: story:handoff-when-a-program-decides
- depends_on: story:next-decision-names-the-principal
- depends_on: story:tree-library-exports-what-an-engine-needs
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bindings_document.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/tree.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-two-level/extra/answer.yaml
- confidence: cited
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-cli/tests/tree_run/delegated_cgroup.rs
- confidence: cited
  path: crates/loom-tree/src/approve.rs
- confidence: cited
  path: crates/loom-tree/src/encode.rs
- confidence: cited
  path: crates/loom-tree/src/lib.rs
- confidence: cited
  path: crates/loom-tree/src/person.rs
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: inferred
  path: crates/loom-tree/tests/embedder
- confidence: cited
  path: crates/loom-tree/tests/main.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T11:28:08Z", actor: "agent:loom", revision: 12}
- {from: "proposed", to: "active", at: "2026-11-22T11:28:37Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "runbook-person-program-2026-11-22"}
---
## Outcome

A `person` binding may name an `argv`. When the principal takes the action with
`tree approve --action`, Loom runs that program once through the same confined path a `program`
binding runs through. Only after it exits 0 does Loom write the person's records, and they hold
the revisions the workspace holds after the program. An argv entry that is exactly `{principal}`
is replaced by the principal `tree approve` was given. A program that does not exit 0 writes no
person record, and `tree approve` names why.

Cites are at the plan tree `0198a5d1` (`wave/2026-11-21-w7`). Runbook cites are at Runbook
`5c5522b65`.

**What Loom does today.**

- `tree approve --action` (`crates/loom-tree/src/approve.rs:75-204`) checks the action before
  anything is written (`person_action`, `:218-283`):
  - the case binds it to `person`, and the principal is the binding's `executor.principal`
    (`:238-243`);
  - the frontier admits it;
  - a capability is named when the protocol action needs one;
  - a named result is one every `from: effect` entry lists.
- With a capability, `tree approve` then records the approval and its `AuthorityRecorded` line
  (`:157-179`). A person action that needs no capability records no approval and no authority
  line (`crates/loom-cli/tests/tree_run.rs:2104-2113`).
- It then takes the action through Commission's loop (`take_person_action`, `:287-323`;
  `one_step`, `crates/loom-tree/src/run.rs:1280-1334`).
- The person port (`crates/loom-tree/src/person.rs:56-199`) checks the principal again
  (`:65-71`) and reads the placed artifacts' revisions at that moment (`:84-89`). It writes one
  observation and one record per `from: effect` entry in one `record_step` write. It starts no
  process.
- A refused effect reaches `tree approve` only as a string, and `one_step` maps every refusal to
  `NotAPersonAction` with no log line (`run.rs:1321-1326`).
- A `person` binding cannot name a program today:
  - `argv` belongs to `Program` alone (`crates/loom-bindings/src/load.rs:231-257`, refused
    `MemberNotForExecutor`);
  - a person executor must name a `principal` (`crates/loom-bindings/src/decode.rs:467-474`);
  - its entries read `from: effect` alone (`load.rs:134-149`).
- `story:person-action-records-a-named-result` (merged at `5af3f9c2`) added `--result` and the
  reasons. Its order is kept here: refuse before the first write, then one write of the records.
- `story:bound-program-contract` (merged) gave a program its child: the environment of
  `bound_case_child` (`crates/loom-bindings/src/child.rs:45-60`) and `exits`.
- The decision crate gives the prefix `decider:` a meaning of its own
  (`crates/loom-decision/src/apply.rs:172-178`). The tree path does not, and this story keeps it
  so.

**The gap.** Runbook's real model runs stop at `scope.answer`. Its decider writes an answer into
`questions.yaml`, then approves the person action. The approval must then run
`tools/sdc-tool.sh scope-answer <principal>`. That tool sets the defaults and `answered_by`
(Runbook `bindings/tools/sdc-tool.sh:447-460`; the binding `bindings/sdc-intake.yaml:97-101`).
Runbook's `bindings/PENDING.md:110-111` names the missing syntax: D11 "a person's action runs a
program", and D15 `{principal}` in `argv`.

**The member.** A `person` binding may name `executor.argv`. It is decoded by the same `argv`
helper as a program's (`decode.rs:515-553`): a list of strings, never a shell string.

- A person binding with no `argv` behaves exactly as today.
- `arguments`, `arguments_from` and `exits` on a person binding are refused
  `MemberNotForExecutor` at load.
- The entries still read `from: effect` alone. The program's exit decides whether the person's
  effect happened, and is never a result value.
- `timeout_seconds` (1 to 3600; 60 when absent) and `budget.max_runs` (8 when absent) apply to
  the program.
- A relative `argv[0]` with a `/` is resolved under `program_root: bindings` by
  `resolve_programs`, which resolves every `executor.argv[0]` whatever the executor
  (`crates/loom-bindings/src/resolve.rs:45-49`).

**`{principal}`: one rule, at decoding.** The shared `argv` helper (`decode.rs:515`) refuses
`InvalidArgv` for `{principal}` in every argv but a person's `executor.argv`. That covers:

- a program's `executor.argv` (`:430-433`);
- `arguments_from` (`:259`);
- `handoff_when`, which `story:handoff-when-a-program-decides` decodes through the same helper.

Even in a person's `executor.argv` it refuses `InvalidArgv` for:

- `{principal}` as `argv[0]`, so the placeholder never chooses the program;
- any entry that holds `{principal}` and is not exactly it, such as `--by={principal}`.

`executor()` knows the kind before it decodes `argv` (`:375-410`), so it tells the helper
whether the placeholder is allowed. An entry that is exactly `{principal}` is replaced, as one
argv entry, by the principal `tree approve` was given. Nothing else is replaced.

**Why it cannot inject.**

- The value is the binding's own `executor.principal`: `tree approve` refuses any other
  principal before anything is written (`approve.rs:238-243`). So the program gets only text
  the bindings document already holds. A document's author controls the argv anyway, so the
  placeholder adds no power.
- The principal is at most 200 bytes and holds no control character (`approve.rs:95-109`).
- The replaced entry stays one argv entry, and the child is started from the list by exec,
  never through a shell (`bound_case_child`, `child.rs:60`). A principal that holds a space, a
  `;` or `$(…)` is one argument, and adds no argument and no command.
- The principal is never put into the child's environment.

**What the child may read and write.** The child is the one `bound_case_child` builds for a
program step (`child.rs:45-60`):

- the argv, `{}` on standard input, and the binding's timeout;
- the workspace as its start directory;
- an environment of `PATH`, `LOOM_CASE`, `LOOM_TIMEOUT_SECONDS` and, for a child case,
  `LOOM_CASE_NAME` and `LOOM_PARENT_CASE_NAME`, and nothing inherited;
- no retry feedback and no path of the store.

It starts only through the runner the host gave. The runner is admitted by
`admit_bound_program_runner` (`crates/loom-intake-slice/src/bindings.rs:371-382`) and checked
with `preflight_child` before anything starts.

- Under `substrate` (the default), the child writes only inside the workspace and has no
  network. Its reach is never more than a program step of the same case has.
- `local` is admitted only with `UNCONFINED_FLAG_REASON`, as for `tree run`.

**`PATH`.** The child's `PATH` is the `PATH` of the `tree approve` process, as `bound_case_child`
sets it for every program (`child.rs:45-60`). A host may start `tree approve` with another
`PATH` than its `tree run`, and until `story:per-run-program-path` exists nothing pins one
`PATH` per run. The consequence: a bare program name, the program's own `argv[0]` or one it
starts (Runbook's tool starts `yq`), can resolve to another file under `tree approve` than
under `tree run`, or to none. The guide tells a host to start both with one `PATH`.

**One shared run path, split in two.** There is no second start path. What
`ProgramEffects::perform` does from the run count to the start
(`crates/loom-bindings/src/program.rs:238-304`) and from the start to the run's observation
(`:305-349`) becomes two crate functions of `b10x-loom-bindings`, exported from `lib.rs`
(`:54`):

- **`prepare`** takes every refusal that comes before a start:
  - the run count (`PROGRAM_SOURCE` observations naming the case and the action,
    `program.rs:171-193`) is below `budget.max_runs`;
  - the program is not left unresolved under `program_root: bindings`;
  - the timeout fits this host's clock;
  - the placed artifacts can be digested (`placements`), and their digests before the run;
  - the case names (`case_names`);
  - the child's inputs, from a function the caller passes in (below);
  - the `{principal}` replacement;
  - the child build (`bound_case_child`), then the retry feedback put on it (`retry::given`,
    `crates/loom-bindings/src/retry.rs:311-321`).

  It writes nothing and returns the prepared child.
- **`start`** runs the prepared child once through the runner, keeps its blob, keeps one
  `ProgramRun` observation whatever the run answered, and digests the placed artifacts again.

**The child's inputs.** A program step's standard input and retry feedback depend on the timeout
and the case names, so `prepare` takes them from a function the caller passes in. That function
is given the timeout and the names, and answers the arguments text and the feedback, or a
refusal. It is called after `case_names` and before the child build, the place they have in
`perform` today (`program.rs:287-304`).

- The program port's function does what `perform` does today. It reads the retry feedback
  (`retry::feedback`, `:287`), then the arguments (`self.arguments`, `:288-291`, `:541-578`): the
  request's, the binding's literal, or what `arguments_from` prints. An `arguments_from` that
  fails is still refused `ArgumentsUnreadable` before the bound program starts.
- The person path's function answers `{}` and no feedback. A person binding has no `arguments`
  or `arguments_from` (refused at load), and `retry_when` does not apply to it.

The program port calls both functions in `perform`, and its behaviour, order and refusals stay
as they are today. The person path calls `prepare` inside the checks of `person_action`, before
the first write, and `start` inside the effect.

**The tree reason of each refusal before the first write.** The program port's reasons
(`ProgramRefusalReason`, `ess/domains/bindings.yaml:417-426`) are not tree reasons
(`TreeRefusalReason`, `ess/domains/tree.yaml:1103-1128`). On the person path every refusal of
`prepare` is `PersonProgramFailed`, and `tree approve` exits 1 (`TREE_REFUSED`, as every tree
refusal does, `crates/loom-cli/src/tree.rs:48-54`). The program port's own text is the detail.
So `ArtifactUnreadable`, `OutsideWorkspace` and `RunsSpent` appear only in the detail:

- runs spent: `RunsSpent: action <action>: …`;
- a placed artifact that cannot be digested: `ArtifactUnreadable: action <action>: nothing was
  started: …`, naming, for example, `OutsideWorkspace`;
- an argv left unresolved under `program_root: bindings`, a timeout this host's clock cannot
  hold, case names that are not child names, and a child that cannot be built (a `PATH` that is
  not UTF-8, a case id that is too long): `<program> was not started: …`.

Two refusals keep other reasons. No runner is `ConfinementRefused` (`NoRunner`). A store or
governor read that fails is `StoreUnreadable`, as today.

**The order of one approval with a program** (D1, default a):

1. Today's checks, then a runner was given, then `prepare`. A refusal here writes nothing: no
   approval, no line, no observation, no blob. Its reason is the one above.
2. With a capability: the approval record and its `AuthorityRecorded` line, as today. Without
   one: nothing is written, as today.
3. Inside the effect Commission admitted (against that approval, or with no capability needed):
   `start`. The `ProgramRun` observation gains `principal`, the principal whose approval
   started the run; it is absent on a program step.
4. On exit 0, with no placed artifact of a non-`write` action changed: the person observation and
   one record per `from: effect` entry, as today. Each record holds the revisions after the
   program and the named result (`performed` when none). These are written with the revision
   moves in one `record_step` write.
5. One `ActionFinished` line. Its `exit` is 0, its `blob_digest` is the program's, and its
   `evidence` holds the new record ids.

**A program that does not end well.** This covers a non-zero exit, a run killed at its timeout,
a runner refusal of the start (a launch error), and a non-`write` action whose program changed a
placed artifact.

- What stays written: the approval (when there is one), the `ProgramRun` observation and the
  blob. The placed artifacts take what the workspace holds, as the moves alone, as a refused
  program step writes them today (`program.rs:370-388`).
- What is not written: no person observation and no record.
- The log gains exactly one line: `ActionFinished`, with the program's exit (non-zero, or 0 for a
  changed reader) or with `refusal` `timed out` or the runner's text, the blob digest when one
  was kept, and empty `evidence`. No `Discarded` line: the tree path writes none for a discarded
  program step today either (`git grep RunLogEvent::Discarded -- crates/loom-tree` finds none).
- `tree approve` then refuses `PersonProgramFailed` and exits 1 (`TREE_REFUSED`). The detail
  names the exit code, `timed out`, the runner's refusal or `ChangedByNonWriter`, and the blob
  digest, never the program's output.
- On the command line, the refusal carries `command`: the argv of the same call, so running it
  takes the action again with the same result.
  - **Who builds it.** The command line's `approve` (`crates/loom-cli/src/tree.rs:352-400`)
    builds it after the library's `approve` returns `PersonProgramFailed`. Only it holds
    `--confinement` and `--cgroup-root`.
  - **When.** On every `PersonProgramFailed`, whether `prepare` refused or the program ran and
    failed. The command line reads only the reason, never the detail. A retry of a refusal
    that is still true is refused again with the same detail, and the detail names why (runs
    spent, for example). `command` is absent on every other tree reason, such as
    `ConfinementRefused` (`NoRunner`), `StoreUnreadable` and `NotAPersonAction`.
  - **What the library does.** The library leaves `command` absent, and `ApproveRequest` gains
    no field for it.
  - **From which values.** From the `TreeApproveArgs` of the call alone, never from the
    program's output or the store.
  - **Word order:** `b10x-loom tree approve --store <store> --case <case>
    [--capability <capability>] --principal <principal> --action <action> [--result <result>]
    [--reasons <text> | --reasons-file <file>] [--at <instant>] --confinement <confinement>
    [--cgroup-root <dir>]`.
    - `<store>`, `<file>` and `<dir>` are the paths as `resolved` gives them: absolute, and
      equal to what the call named when it named an absolute path.
    - A bracketed pair is present exactly when the call named it.
    - `--confinement` is always present, with the call's value or its default `substrate`.

**How the end reaches `tree approve` and the log, typed.** `PersonEffects` gains a cell that the
port fills before it returns: the `ProgramRun` of the start, and a typed end. The end is one of
an exit code other than 0, timed out, the runner's refusal, or `ChangedByNonWriter`.
`take_person_action` reads the cell after `one_step`:

- With no program, or before the program started, today's mapping stays.
- On a performed step, the `ActionFinished` line is built as today. `effect_summary`
  (`run.rs:778-810`) reads `exit` and `blob_digest` from the report's `run`.
- On a refused step whose cell holds an end, the `ActionFinished` line is built from the
  `ProgramRun` (no string is parsed), and the refusal is `PersonProgramFailed` with the typed
  end.

**A person action that needs no capability.** It has no approval record, before or after. D1 (a)
means the program runs inside the admitted effect with nothing written before it. A failure
leaves the `ProgramRun` observation, the blob, the moves and the `ActionFinished` line, and no
approval. A retry is another `tree approve --action`.

**Retry.** A failed program writes no record, so the frontier still admits the action, and
`tree approve` checks only the frontier (`approve.rs:244-256`). The principal fixes the cause
and runs the refusal's `command` (or the `ApproveAction` command printed before). With a
capability, that records a new approval at the case's current revision.

The next `tree run` does not print that `ApproveAction` again at the same case revision. The
failed `ActionFinished` line counts the action as performed at that revision
(`crates/loom-tree/src/history.rs:42-53`), and `waiting_actions` drops an action performed at
the current revision (`run.rs:828-836`). This story does not change that history. The refusal's
`command` is the way back, and the guide says so. Once the case revision moves, the run prints
the action again.

Every start counts against `budget.max_runs`, as a program step's does. Past the budget,
`prepare` refuses `PersonProgramFailed` (`RunsSpent` in the detail), and nothing is written. Its
`command` is present like every `PersonProgramFailed`'s, and running it is refused again the same
way. `retry_when` does not apply: it sends a writer back on a record, and a
failed person program wrote none.

**The runner, only when the binding has an argv.** `tree approve` gains `--confinement` (default
`substrate`) and `--cgroup-root`, with `tree run`'s meaning (`crates/loom-cli/src/lib.rs:484-489`).

- The library gains `person_program(store, case, action, catalog) -> Result<bool, Refused>`
  in `approve.rs`. It reads the case's bindings read-only, takes no lock and writes nothing, and
  answers whether the action is bound to a person with an `argv`.
- The command line's `approve` (`crates/loom-cli/src/tree.rs:352-400`) calls it first:
  - `false`: `approve` runs as today with no runner. It never places, holds a root or starts
    itself again, so an argv-less person action keeps working on a host without delegation.
  - `true`: `approve` places as `tree run` does (`place`, `:185-206`, generalised to take the two
    flags rather than `TreeRunArgs`): it holds `--cgroup-root`, or starts itself again in a
    delegated scope with its own arguments (`start_delegated`, `lib.rs:437-448`). Then it builds
    the runner with `tree_runner` (`:139-155`).
- `ApproveRequest` (`approve.rs:28-51`) gains `runner: Option<Arc<dyn TestRunner>>`.
  `approve` checks it again under the store lock: a binding with an argv and no runner is
  refused `ConfinementRefused` (`NoRunner`) before anything is written. That covers a bindings
  file that changed between the two reads.
- The suite runner's two `ApproveRequest` literals (`crates/loom-tree/src/suite/run.rs:973`,
  `:1143`) pass the suite's `runner` (`:350`).
- An `ApproveAction` decision of a binding with an argv prints `--confinement <the run's value>`
  in its command (`waits`, `run.rs:904-1018`), as a handoff's printed `tree run` does
  (`run.rs:855-870`).

Security-relevant: yes. The story adds a place where a process starts, starts it on a person's
approval, and passes the approving principal into an argv. The unit gets a security review.

## Decisions

- **D1. When the program runs against the approval record.** Default **a**.
  - **a.** With a capability, the approval is recorded first, and the program runs inside the
    effect Commission admitted against it, as every effect runs. A failed program leaves the
    approval and no person record. With no capability there is no approval to leave.
  - **b.** The program runs first, and the approval is recorded only on exit 0. A failure then
    leaves no approval, but the program runs before any authority exists, outside Commission's
    admission. This breaks the rule that no effect runs before its admission.
- **D2. How the principal reaches the program.** Default **a**.
  - **a.** The whole argv entry `{principal}`, as Runbook's D15 asks.
  - **b.** A variable `LOOM_PRINCIPAL` and no placeholder. This adds no argv syntax, but Runbook's
    `scope-answer` reads the principal as `$2`.
- **D3. The member's name.** Default **a**.
  - **a.** `executor.argv`, the program's member, now also a person's.
  - **b.** A new member, `executor.program`. This keeps `argv` a program's alone, but adds a
    second spelling of one thing.

## ESS first

New nouns are modelled in `ess/` before code:

- `ess/domains/bindings.yaml`:
  - `ExecutorSpec`'s comment (`:36-42`): `argv` belongs to `Program` and `Person`;
  - `BindingsRefusalReason`'s comment (`:248-259`):
    - the `{principal}` rules under `InvalidArgv`;
    - `arguments`, `arguments_from` and `exits` on a person under `MemberNotForExecutor`;
  - `ProgramRun` (`:301-332`) gains `principal: Optional<String>`.
- `ess/domains/tree.yaml`:
  - `TreeRefusalReason` (`:1103-1128`) gains `PersonProgramFailed`, and its comment
    (`:1082-1102`) says when it is given;
  - `TreeRefusal` (`:1130-1148`) gains `command: Optional<List<String>>`: the argv of the
    refused `tree approve` call, in the word order above. The command line sets it on every
    `PersonProgramFailed` and on no other reason, and the library leaves it absent.
    `refusal_json` (`crates/loom-tree/src/encode.rs:17-31`) writes it after `action`, and no
    member when it is absent. `Refused::new` (`crates/loom-tree/src/lib.rs:92-101`) sets it to
    `None`.
- `generated/rust/loom` and `generated/ess/loom` are regenerated in the same commit.

`slot task drift` is green on every commit. The first commit adds the decode and load tests
below; they fail on that commit (a person `argv` is refused `MemberNotForExecutor`). That is the
red check the unit records.

## Acceptance

Every test runs with no model call and no network, and ends within 60 s. The test step stays
within 120 s. A build or test job in the slot pool ends within 10 minutes. Fixture programs are
POSIX sh, never Python.

**The fixture.** A protocol `answer@1` is added as
`crates/loom-cli/tests/fixtures/tree-two-level/extra/answer.yaml`, and registered beside
`sign@1` in the test catalog (`tree_run.rs:105-111`). It has three person actions, each with no
precondition and one evidence kind:

- `answer.take`: effect `write`, capability `owner.answer`;
- `answer.free`: effect `write`, no capability;
- `answer.look`: effect `read`, capability `owner.look`.

Each test writes its bindings file into the workspace (as `tree_run.rs:3048` does), with the
principal `owner` and an argv naming a POSIX sh stub the test writes into `stubs/`. The test
opens the tree with `open_with` (`:1947`). CLI tests pass `--confinement local`.

Library tests in the same binary call `b10x_loom_tree::approve` on such a store. They read
observations through `loom_governor::CanonGovernor` over the store's `FileCaseStore`, which the
binary already links (`tree_run.rs:1406`).

The one delegated test skips without a delegated cgroup only when `LOOM_REQUIRE_DELEGATION` is
unset. The unit's gate sets it and runs it under `systemd-run --user -p Delegate=yes --scope`.
The unit's report lists the tests run and the tests skipped, and a skip counts as red there.

- **Decode and load** (`crates/loom-bindings/tests/bindings_document.rs`):
  - A person binding with an `argv` loads. With `arguments`, `arguments_from` or `exits` it is
    refused `MemberNotForExecutor`, naming the member. Test:
    `a_person_binding_takes_an_argv_and_no_other_program_member`.
  - Each of these is refused `InvalidArgv`, naming the member:
    - `{principal}` as a person's `argv[0]`;
    - `--by={principal}` in a person's argv;
    - `{principal}` in a program's `executor.argv`;
    - `{principal}` in `arguments_from`;
    - `{principal}` in `handoff_when`.
    Tests, one per member: `principal_is_never_argv_zero`, `principal_is_never_part_of_an_entry`,
    `principal_is_refused_in_a_program_argv`, `principal_is_refused_in_arguments_from`,
    `principal_is_refused_in_handoff_when`.
- **The program runs** (`tree_run.rs`):
  - `answer.take` with argv `[sh, stubs/answer.sh, {principal}, --action, answer.take]`. The stub
    appends its arguments to the subject file. The printed `ApproveAction` command, run as
    printed, exits 0. Then:
    - the subject file holds `owner` as one argument;
    - the person record's `subject_revision` is the digest after the program;
    - the log ends with `AuthorityRecorded`, then `ActionFinished` with exit 0, the blob digest
      and the record id;
    - the `ProgramRun` observation, read through `CanonGovernor::try_observations`, names
      `owner` in `principal`.
    Test: `a_person_action_runs_its_program_with_the_principal`.
  - Runbook's shape: a bindings file with `program_root: bindings` and
    `[tools/answer-tool.sh, scope-answer, {principal}, --action, answer.take]`, the stub beside
    the bindings file. The approval exits 0, and the subject file holds `owner`. Test:
    `a_person_program_beside_the_bindings_runs_with_the_principal`.
  - The principal `owner a;$(touch pwned)` (the binding's and the approval's). The stub prints
    its argument count and arguments: 3 arguments, the principal whole as one, and no file
    `pwned` exists. Test: `the_principal_is_one_argument_and_runs_nothing`.
  - The stub runs `/usr/bin/env` into a file. The test drops the variables `/bin/sh` sets
    itself (`PWD`, `SHLVL`, `_`), as `printed_environment` does
    (`crates/loom-bindings/tests/open_from_program.rs:677-690`). No remaining variable's value
    holds the principal, and the names are exactly the ones `bound_case_child` sets. Test:
    `the_principal_is_never_in_the_childs_environment`.
  - `answer.free` (no capability) with a stub that exits 0. One `ActionFinished` line and no
    authority line, and `read_approvals` is empty. Test:
    `a_person_program_without_a_capability_runs_and_keeps_no_approval`.
- **Failure** (`tree_run.rs`):
  - `answer.take` with a stub that exits 3. `tree approve` exits 1 with `PersonProgramFailed`,
    naming exit 3 and the blob digest. The approval is kept, and there is no person record. The
    log gains `AuthorityRecorded`, then one `ActionFinished` (exit 3, empty evidence).
    - The test runs `b10x-loom tree approve --store <store> --case root --capability
      owner.answer --principal owner --action answer.take --result performed --reasons <text>
      --at <AT> --confinement local`, in that word order, with `--at` as the review helper
      passes it (`tree_run.rs:2253`).
    - The refusal's `command` equals that argv word for word.
    - Once the stub is changed to exit 0, running that `command` exits 0 and writes the person
      record with the result `performed`.

    Test:
    `a_failed_person_program_records_nothing_and_keeps_the_approval`.
  - `answer.free` with a stub that exits 3: the same, with no approval and no authority line.
    Test: `a_failed_person_program_without_a_capability_leaves_no_approval`.
  - `answer.take` with `timeout_seconds: 1` and the stub `exec /usr/bin/sleep 30`, as
    `crates/loom-bindings/tests/bound_program.rs:508` does. Refused `PersonProgramFailed`, naming
    `timed out`. The command ends within 20 s (the timeout plus a margin for a shared host), and
    there is no person record. Test: `a_person_program_killed_at_its_timeout_records_nothing`.
  - `answer.look` (effect `read`) with a stub that appends to the subject file and exits 0.
    Refused `PersonProgramFailed`, naming `ChangedByNonWriter`. The log gains exactly
    `AuthorityRecorded`, then one `ActionFinished` (exit 0, the blob digest, empty evidence), and
    no `Discarded` line. The approval is kept, there is no person record, and the subject's
    revision is the digest after the program. Test:
    `a_person_program_of_a_reader_that_writes_records_nothing`.
  - A library test with a runner whose `run_child` answers a `ConfinementError`, as
    `crates/loom-bindings/tests/security_w20261121w7_u2.rs:766-771` does. Refused
    `PersonProgramFailed`, naming the runner's refusal. The `ProgramRun` holds
    `runner_refusal`, and the log's `ActionFinished` holds that text as `refusal`. Test:
    `a_runner_refusal_of_a_person_program_records_nothing`.
  - The subject path is replaced by a symbolic link to a directory outside the workspace, as
    `crates/loom-bindings/tests/bound_revisions.rs:527-559` does. `tree approve` exits 1, refused
    `PersonProgramFailed`, and its detail names `ArtifactUnreadable` and `OutsideWorkspace`. The
    store's files are byte for byte unchanged, and the stub's marker does not exist. Test:
    `an_unreadable_placed_artifact_refuses_the_person_program_before_any_write`.
- **Retry** (`tree_run.rs`): `answer.take` with `budget.max_runs: 2` and a stub that fails until a
  marker exists. Two approvals fail. The third is refused `PersonProgramFailed` with `RunsSpent`
  in its detail. Its `command` is present and equals, word for word, the argv the test ran.
  The store's files are byte for byte unchanged. In a second store, the marker is made after
  one failure, and the second approval writes the
  person's record. Test:
  `a_person_program_is_retried_by_approving_again_within_max_runs`.
- **No runner** (library, `tree_run.rs`): `approve` with `runner: None` for `answer.take` with an
  argv is refused `ConfinementRefused` (`NoRunner`), and the store's files are unchanged.
  `person_program` answers `true` for it, and `false` for `post.sign` of `sign@1`, whose
  `approve` with `runner: None` records as today. Test:
  `a_person_program_without_a_runner_is_refused_before_anything_is_written`.
- **Unchanged and green:** `a_person_records_the_named_result`,
  `without_a_result_the_person_performed`, `the_printed_person_action_without_a_capability_runs`
  and `a_person_action_that_needs_a_capability_is_refused_without_one` (`tree_run.rs`). A person
  action without an argv prints no `--confinement`, and its `tree approve` never starts itself
  again.
- **Printed command** (`tree_run.rs`): the `ApproveAction` decision of `answer.take` with an argv
  carries `--confinement local` after a run with it. Running the printed argv exits 0. Test:
  `the_printed_approve_action_command_carries_the_confinement`.
- **Confined** (delegated, `crates/loom-cli/tests/tree_run/delegated_cgroup.rs`): under
  `substrate`, `answer.take` with a stub that writes a file outside the workspace fails to. Its
  approval is refused `PersonProgramFailed`, and the file does not exist. Test:
  `a_person_program_runs_confined_under_substrate`.
- **Docs:**
  - `website/docs/guides/bindings.md` (`:148-170`) states the member, the `{principal}` rules,
    the order, the failure, and the one-`PATH` advice. It also says that the refusal's `command`
    is the argv of the refused call, in the word order above (with `--result`, the reasons,
    `--at`, `--confinement` and `--cgroup-root` as the call named them), and that running it
    retries with the same result;
  - `website/docs/reference/cli.md` is regenerated by `loom-docs`;
  - `CHANGELOG.md` gets an `Added` bullet, and a `**Breaking:**` bullet for `ApproveRequest`'s
    new field.
- `slot task check` exits 0, and `slot task drift` exits 0 on every commit of the unit.

## Out of scope

- A placeholder other than `{principal}`: `{work}` is Runbook's D14, and `{result}` is not
  asked for.
- A person action taken by another principal than the binding's.
- `tree run` taking a person action.
- A person program's `from: exit` or report results: Runbook's D12 rejected result stays the
  person's `--result`.
- A cancel handle for `tree approve`.
- `subject` with no evidence (Runbook's D38).
- One `PATH` per run, which is `story:per-run-program-path`.
- Substrate and Canon.

## Scope

Derived 2026-11-22 from the plan tree at `2e74232a`; every cite read again at `0198a5d1`
(`wave/2026-11-21-w7`). **Cited** means read at the `file:line` named. **Inferred** means a
reading that could be wrong.

- **ESS and generated:** `ess/domains/bindings.yaml` (`:36-42`, `:248-259`, `:301-332`, read at
  `:417-426`), `ess/domains/tree.yaml` (`:1082-1148`), `generated/rust/loom`,
  `generated/ess/loom` — cited
- **Bindings:**
  - `crates/loom-bindings/src/decode.rs`: the `argv` helper (`:515-553`) and its callers
    (`:259`, `:430-433`) — cited;
  - `crates/loom-bindings/src/load.rs`: `fit` (`:231-295`) — cited;
  - `crates/loom-bindings/src/program.rs`: `perform` (`:215-509`) split into `prepare`
    (`:238-304`) and `start` (`:305-349`), with `runs` (`:171-193`) shared — cited for the lines,
    inferred for the two functions' shape;
  - `crates/loom-bindings/src/lib.rs`: the export of the two functions beside `ProgramEffects`
    (`:54`) — cited;
  - `crates/loom-bindings/src/child.rs` and `resolve.rs` are read, not changed — cited.
- **Tree:**
  - `crates/loom-tree/src/approve.rs`: `ApproveRequest` (`:28-51`), `approve` (`:75-204`),
    `person_action` (`:218-283`), `take_person_action` (`:287-323`), the new `person_program` —
    cited;
  - `crates/loom-tree/src/person.rs`: `PersonEffects` and `perform` (`:30-199`) — cited;
  - `crates/loom-tree/src/run.rs`: `waits` (`:904-1018`); `one_step` (`:1280-1334`) — cited;
  - `crates/loom-tree/src/encode.rs`: `refusal_json` (`:17-31`) writes `command` — cited;
  - `crates/loom-tree/src/lib.rs`: `Refused::new` (`:92-101`) builds a `TreeRefusal` literal and
    sets `command: None` — cited;
  - `crates/loom-tree/src/history.rs` is read, not changed (`:42-53`) — cited;
  - `crates/loom-tree/src/suite/run.rs`: the `ApproveRequest` literals (`:973`, `:1143`) and the
    suite's `runner` (`:350`) — cited.
- **CLI:**
  - `crates/loom-cli/src/lib.rs`: `TreeApproveArgs` (`:537-584`); `TreeRunArgs`'s `confinement`
    and `cgroup_root` (`:484-489`); `start_delegated` (`:437-448`) — cited;
  - `crates/loom-cli/src/tree.rs`: `approve` (`:352-400`), which also builds the refusal's
    `command`; `tree_runner` (`:139-155`); and `place` (`:185-206`) — cited.
- **Tests and fixtures:** `crates/loom-bindings/tests/bindings_document.rs`,
  `crates/loom-cli/tests/tree_run.rs`, `crates/loom-cli/tests/tree_run/delegated_cgroup.rs`,
  and `crates/loom-cli/tests/fixtures/tree-two-level/extra/answer.yaml` (new) — cited
- **A path that does not exist yet:** `crates/loom-tree/tests/embedder/`, the fixture package
  `story:tree-library-exports-what-an-engine-needs` creates. Its `ApproveRequest` literal gains
  `runner: None` in this story's unit, which merges that story's head. That story is not
  changed for it — inferred
- **Docs:** `website/docs/guides/bindings.md:148-170`, `website/docs/reference/cli.md`
  (generated), `CHANGELOG.md` `[Unreleased]`, `AGENTS.md` (the person action sentence) — cited
- **Edges** (each recorded as depends_on; this story comes after all of them, and its unit
  merges their heads):
  - `story:person-action-records-a-named-result`, merged at `5af3f9c2`. This story extends its
    `approve` and `PersonEffects`, and keeps its refuse-before-the-first-write order.
  - `story:bound-program-contract`, merged. This story reuses `bound_case_child`'s environment
    and keeps `exits` a program's.
  - `story:cancel-a-running-bound-program` (active). Shared: the run path in `program.rs` that
    this story splits, `ProgramRun` in `ess/domains/bindings.yaml`, and `ess/domains/tree.yaml`.
    The split functions carry its cancel handle, and `tree approve` passes none.
  - `story:handoff-when-a-program-decides` (active). Shared: the `argv` helper of `decode.rs`
    (its `handoff_when` must exist for this story's `handoff_when` test), `run.rs` and
    `tree_run.rs`.
  - `story:next-decision-names-the-principal`, merged at `0198a5d1`. Shared: the
    `ApproveAction` command in `run.rs` `waits`, `ess/domains/tree.yaml` and `CHANGELOG.md`.
    This story adds `--confinement` after the words that story's head holds.
  - `story:tree-library-exports-what-an-engine-needs` (active). Its embedder fixture calls
    `approve` with an `ApproveRequest` literal. It merges first, and this story's unit adds
    `runner: None` to that literal when it merges that head.
- **Would collide with**, no edge: `story:per-run-program-path` (proposed). The files, as
  `aep plan artifact waves` lists them:
  - cited: `crates/loom-bindings/src/lib.rs`, `crates/loom-bindings/src/program.rs`,
    `website/docs/guides/bindings.md`, `website/docs/reference/cli.md`, `AGENTS.md`,
    `CHANGELOG.md`;
  - inferred: `crates/loom-cli/src/tree.rs`, `crates/loom-tree/src/approve.rs`,
    `crates/loom-tree/src/person.rs`, `crates/loom-tree/src/run.rs`,
    `crates/loom-tree/src/suite/run.rs`, `crates/loom-cli/tests/tree_run.rs`,
    `crates/loom-tree/tests/main.rs`, `ess/domains/tree.yaml`, `generated/rust/loom`.

  It also shares the child's `PATH`. Order: `story:per-run-program-path`, when activated, is
  built after this story and merges its head. No depends_on is recorded, since that story is
  proposed and not active.
