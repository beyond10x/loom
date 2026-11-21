---
format: aep.planning-md/3
id: story:decisions-resume-the-held-run
kind: story
status: draft
title: decisions decide, take and record over a state directory resume the run held for the request in the same invocation
summary: After a recorded decision, decide, take and record over a state directory resume the commission whose Suspension names the request, for approval holds of software-change@1 runs, through resume_after_grant with DecisionAuthority. A question or outcome hold resumes through resume_decided_request with the executor the host's ResumeBuilder supplies, and the command line refuses it NotRebuildable (31). A failed resume is retried by repeating the command. tick --resume resumes every run held on a request decided by silence, and b10x-loom stop keeps a stopped run from any resume, reported as Stopped
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:question-and-outcome-decisions-hold-runs
- depends_on: story:approvals-supplied-to-canon-evaluation
- depends_on: story:silence-and-expiry-at-a-host-tick
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/approval_decisions.rs
- confidence: cited
  path: crates/loom-cli/tests/decisions_resume.rs
- confidence: inferred
  path: crates/loom-intake-slice/Cargo.toml
- confidence: cited
  path: crates/loom-intake-slice/src/decided_resume.rs
- confidence: cited
  path: crates/loom-intake-slice/src/lib.rs
- confidence: cited
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/decided_resume.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: ess/intake/domains/routing.yaml
- confidence: cited
  path: website/docs/guides/decisions.md
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 10
---
## Outcome

With `--state DIR`, a person's answer recorded on the command line resumes the run held for it in
the same invocation, and the command prints the run's next state. The command line resumes
approval holds of `software-change@1` runs. A held question or outcome resumes through the library
function `resume_decided_request`, which a host calls with its own executor (below). No resume
command is added.
One command is added, `b10x-loom stop`, so that a person can stop a held run and no tick resumes
it (below).

- `decisions decide ID OPTION --as P --state DIR` and `decisions take ID --as P --state DIR`
  resume once the decision is recorded, after the inbox's own output
  (`story:decision-inbox-commands`). `take` resumes as `decide` does: it is the same act of a
  person, and without it a taken request's run would stay held with no command to continue it.
- `decisions record ID --revision ARTIFACT@REV --state DIR`
  (`story:question-and-outcome-decisions-hold-runs`) resumes once `RecordProduced` is applied,
  `already-recorded` included. A question's `decide` cannot resume: its option produces an
  `artifact_revision` record that only `record` sets, so `decide` on a held question request is
  refused `DecisionNotRecorded` by `resume_decision_hold`. `decide` therefore does not try: it
  prints the `decisions record` command as that story says and resumes nothing.
- `decisions tick --state DIR --resume` resumes, one after another, every run held on a request
  decided by `silence` whose hold still names the request: those this tick decided and those an
  earlier tick decided (below). Without `--resume`, a tick stays record-only
  (`story:silence-and-expiry-at-a-host-tick`). An Expired request resumes nothing and leaves its
  run held.
- `--store DIR` alone holds no commission: a store directory keeps a `FileDecisionStore` and a
  `FileCaseStore` only. `decide`, `take` and `record` over it resume nothing and print nothing
  more than the inbox does.
- `--tree STORE` comes after this story, with `story:tree-waits-open-decision-requests`. A tree's
  wait is not a Commission Suspension: the next `tree run` continues past it, or, on a served
  tree, the daemon's next pass does. That story gives `decide` and `take` under `--tree` two last
  lines: `unblocked: <case> (DR-…)` and `continue: b10x-loom tree run <STORE>`, with the store
  path exactly as given to `--tree` (`tree run <store>`, `story:tree-daemon`). On a store a daemon
  serves, the second line is `continue: the daemon serving <STORE> continues it`
  (`story:daemon-continues-a-decided-request`).

Which run is resumed:

- The resume step lists the state directory's resume contexts (`ResumeContextStore::list`,
  `crates/loom-intake-slice/src/resume.rs:291`). It reads each listed commission's Suspension and
  keeps those whose hold names the request: an `Authority` hold whose action request names `DR-…`
  (`story:approval-decisions-resume-held-runs`), or a `Decision` hold with that `request_id`
  (`story:decision-suspension-reason`).
- No such commission: the command prints `resumed: nothing (no run in DIR holds DR-…)`, or the
  `stopped:` line when a `RunStopped` record names the request (below).
- One: it is resumed.
- More than one: nothing is resumed, and the refusal `HeldByMoreThanOneCommission` names them.
  `--commission ID` (on `decide`, `take` and `record`, `--state` only) names the commission to
  resume. Unless it was stopped (below), the named commission is resumed as it is, without the hold
  check, so Commission's own refusals answer a mismatch.

Which resume path, by the request's record kind:

- An approval request (`canon_authority`) is resumed through the slice's resume, `resume_with_state`
  (`crates/loom-intake-slice/src/intent.rs:654`). That calls Commission's `resume_after_grant`
  (`crates/loom-commission/src/runtime.rs:2374`) with `DecisionAuthority`, which the approval story
  puts in place of `GrantAuthority` when the resume context holds the decision flags.
- An approval decided on the denying option is not resumed: `resume_after_grant` would refuse
  `NotGranted` and leave the Suspension, as that story's `resume` does. The command prints
  `held: <commission> (DR-… denies <capability>)`, and the Suspension stays `Pending`.
- A question or outcome request (`artifact_revision`, `canon_decisions`, or an option producing no
  record) is resumed through `resume_decision_hold`
  (`story:question-and-outcome-decisions-hold-runs`), which calls `resume_after_decision`.
- What each path needs from the host comes through the host port `ResumeBuilder` (below). The
  approval path needs the commission's `IntentRequest` and agent model, rebuilt from its resume
  context. The question and outcome path needs the inner executor that `resume_decision_hold`
  wraps, a Commission `AgentExecutor` (`crates/loom-commission/src/ports/executor.rs:20`). A
  test passes a scripted one, and an embedding host passes its own.

What the command line can resume. `b10x-loom`'s `ResumeBuilder` is the code `resume` uses
(`crates/loom-cli/src/main.rs:658-681`), and it rebuilds what that code rebuilds:

- It rebuilds a commission whose held case has a software change's inputs. `resumed_request`
  refuses any other case (`crates/loom-intake-slice/src/intent.rs:624-625`) and binds no commands
  (`:628-629`).
- It gives no executor for a `Decision` hold. `drive` builds its executor inline
  (`crates/loom-intake-slice/src/run.rs:610`), and none of `drive`'s executors returns
  `NeedsHumanJudgment`.

So with `--state`, the command line resumes approval holds of `software-change@1` runs. An outcome
hold cannot be resumed there either. `software-change@1` declares three outcomes that need a
decision (`declined`, `superseded`, `abandoned`: `protocols/software-change/1.yaml:286-309` of
engineering-protocols `0.16.0`, the tag `crates/loom-protocols/Cargo.toml:15` pins). Even so,
`story:question-and-outcome-decisions-hold-runs` states that no `software-change@1` run holds a
`Decision` Suspension (its § Outcome, `resume_decision_hold` paragraph), and no story tests one
through the binary. If a run ever did, `b10x-loom` would refuse its resume, as below.

A decided request whose holding commission the command line cannot rebuild is refused
`NotRebuildable`, exit 31:

- the request is a question or outcome request, so its path needs an executor;
- or the commission's case is not a software change's.

The decision stays recorded, and the Suspension is left as it was. Standard error names the
commission and the cause, and gives the path that resumes it:
`loom_sdk::intake::slice::decided_resume::resume_decided_request` with a host's `ResumeBuilder`.
A question or outcome hold is resumed through that library function, and a tree continues through
`tree run` or its daemon. A general rebuild from the resume context is not in this story (§ Out of
scope).

The run's next state is printed on standard output in this order:

1. The inbox's output.
2. `resumed: <commission> (DR-…)`.
3. The resumed run's lines as `resume` prints them.
4. The run's stop line, `stopped: <reason>` or `stopped: <reason> (<detail>)`
   (`crates/loom-intake-slice/src/run.rs:1300-1310`). Examples: `stopped: Completed`,
   `stopped: DecisionRequired (DR-2)` when it is held again, `stopped: ApprovalRequired (<action>)`.

Exit status of `decide`, `take` and `record` with `--state`. The new statuses are clear of the run
table (`crates/loom-cli/src/lib.rs:26-35`), of `resume`'s 10 to 22 (`lib.rs:90-161`, story 10's
19 included) and of the inbox's 10, 11, 12 and 20.

- `0`, `3`, `4`: the resumed run's status, as `run_exit_status` gives it for its stop line
  (`lib.rs:177-182`).
- `0` also when no run holds the request, or when the approval was denied and the run stays held.
- `10`, `11`, `12`, `20`, `2`, `1`: as the inbox gives them; nothing is resumed. `12` (decided,
  record not completed) resumes nothing because the record is missing.
- `30`: Commission refused the resume on the approval path, with a `ResumeRefusal` such as
  `CaseMoved`, `InDoubt`, `OtherRequest` or `HeldForDecision`. The refusals of
  `resume_decision_hold` (`DecisionNotRecorded`, a superseded request) come after the
  `ResumeBuilder`, so on the command line `31` answers first.
- `31`: `NotRebuildable`, the command line cannot rebuild the holding commission (above).
- `32` is not used. `HeldForAuthority` and `OtherDecision` come only from `resume_decision_hold`,
  and the command line never reaches it: the question and outcome path stops at `NotRebuildable`
  first. `resume_decided_request` returns both refusals to a host as Commission gives them, and
  `33` to `35` keep the numbers round 5 gave them.
- `33`: the resume failed, in any of these ways:
  - the suspension store or resume context cannot be read;
  - the protocol catalog, the models or the confinement the context names cannot be built;
  - the route no longer matches (`SliceError::Context`);
  - the workspace no longer validates (`SliceError::Case`);
  - the governor cannot answer;
  - the resumed run failed.

  `resume` gives the context and workspace failures exit 1 (`crates/loom-cli/src/main.rs:611`).
  Here they are 33, because the decision is already recorded.
- `34`: `HeldByMoreThanOneCommission`.
- `35` (`tick --resume` only): see below.

`b10x-loom resume` keeps story 10's mapping of `HeldForAuthority` and `OtherDecision` to 19:
`resume` calls only `resume_after_grant`, so neither reaches it. A new
`decided_resume_exit_status` in `crates/loom-cli/src/lib.rs`, beside `resume_exit_status`
(`:154`), gives 30, 31, 33 and 34, and the `decisions` exit-status help gains their lines.

When the decision is recorded and the resume is refused or fails, the decision stays recorded: the
request stays Decided with its `produced_record`. Standard error names the request, says the
decision is recorded, and names the refusal or failure. A refusal leaves the Suspension as
Commission's refusal leaves it.

A second `decide` or `take` of a Decided request is refused `DecisionStateConflict`, as the inbox
says, and changes nothing. With `--state`, when the request is Decided and recorded (or Decided on
an option producing none) and a run still holds it, the command goes on to the resume step.
Standard error names the refusal and the principal and option of the recorded decision, and the
exit status is the resume step's. With no run holding the request it exits 10, as the inbox gives.
A second `record` of the same revision is `already-recorded` and resumes the same way. So a resume
that failed is retried by repeating the command; no separate resume command is added. Two
processes cannot both perform the held request: the resume takes the Suspension first, and the
second process is refused `InDoubt` or `NothingSuspended` (30).

`decisions tick --at INSTANT --state DIR --resume`:

- `--resume` without `--state` is refused before the store is read: exit 2, the command line is
  not valid. `--store` holds no run, and `--tree` continues at `tree run`.
- The tick runs as `story:silence-and-expiry-at-a-host-tick` says.
- Then it finds the runs to resume through the state directory's held runs, not through its
  report. It lists the resume contexts (`ResumeContextStore::list`, `resume.rs:291`) and reads
  each listed commission's Suspension, as the resume step does. It keeps every `Pending` hold
  whose request is Decided with principal kind `silence`, by this tick or an earlier one
  (session `sdc`, `.engineering/drafts/e3-sdc-answers-10.md`).
- It resumes those, in ascending request id and then commission id, each through
  `resume_decided_request` for that commission, so a request two runs hold resumes both.
- Each resume runs to its end before the next starts. A resume that is refused or fails does not
  stop the next one.
- A run whose resume was refused or failed, and whose hold still names the request, is tried
  again by the next `tick --resume`. That is the retry path without a person.
- A request the tick expired is not resumed: it has no decision.
- A request decided by `silence` that still awaits its record (a question's `artifact_revision`)
  is not resumed. Its line says `awaits_record`, with `exit_status` 0, until the host runs
  `decisions record`, which then resumes it.
- A tick that stops on an error, such as a `CaseRevisions` that cannot answer, resumes nothing
  and exits with the error's status. The silence decisions it made before the error stay, and the
  next `tick --resume` resumes their runs, so no list of what was taken before the error is kept.
- A run a person stopped is never resumed (`b10x-loom stop`, below): its Suspension is released,
  so no hold names the request.

How the report carries the runs. `TickReport` is unchanged: `b10x-loom-decision`, where it lives,
names no Commission crate (`story:decision-request-entity`, Acceptance), so it cannot name a run.
The command prints beside it instead. Standard output keeps the `TickReport` JSON as its first
line. Then it prints one JSON line per held run it found, `TickResume`, declared in
`ess/intake/domains/routing.yaml` beside `DecidedResume`, with these members:

- `request_id`;
- `commission`, the run;
- `outcome` (`resumed`, `stays_held`, `awaits_record`, `refused` or `failed`);
- `stop`, the resumed run's stop line;
- `exit_status`, the status `decide` would exit with for that resume (0, 3, 4, 30, 31 or 33);
- `error`, the refusal or failure, when there is one.

Each resumed run's own lines go to standard error, so standard output stays JSON.

The tick's exit status with `--resume`:

- `0` when the tick completed and every `TickResume` has `exit_status` 0;
- `35`, new and clear of the statuses above, when the tick completed and at least one
  `TickResume` has another `exit_status`;
- the tick's own error status, as the inbox's table gives it, when the tick stopped on an error.
  For a `CaseRevisions` that cannot answer this is `20` (`story:decision-inbox-commands`).

`resume_decided_request` is the one library function that both the commands and a host call. It
goes in a new module of `crates/loom-intake-slice`, `decided_resume.rs`. It returns
`DecidedResumeResult`, a Rust struct of two parts:

- `resume`, a `DecidedResume`: `NothingHeld`, `StaysHeld { commission, capability }`,
  `AwaitsRecord { commission }`, `Stopped { commission, principal, reason }` or
  `Resumed { commission }`;
- `run`, the resumed run's `SliceRun`, present for `Resumed` only.

`DecidedResume` is generated from `ess/intake/domains/routing.yaml` and so carries no run.
`SliceRun` is not an ESS type, so it stays beside it in the Rust struct.

How `resume_decided_request` reads a stopped run. It reads `RunStopped` itself, so neither the
command line nor a host reads it a second time:

- With no commission given, and no listed commission holding the request, it looks for a
  `RunStopped` record that names the request. If there is one, it returns `Stopped`, naming the
  lowest commission id among those records; if not, it returns `NothingHeld`.
- With `--commission C` (the argument `Some(C)`), it reads C's `RunStopped` before anything else.
  If one is kept, it returns `Stopped` for C and calls neither the `ResumeBuilder` nor Commission.
  The command prints the stop rule's `stopped: C by P (reason)` line and exits 0.
- A `Stopped` answer calls the `ResumeBuilder` 0 times.

Its arguments:

- the request id, the decision store, and the state directory's stores (`ResumeState`, which the
  `RunStopped` records are kept beside);
- an optional commission;
- the protocol catalog and the governor;
- a host port `ResumeBuilder`.

`ResumeBuilder` has two methods, each called only for the path the request takes:

- `approval(context, governor)`: the `IntentRequest`, with its test runner, and the agent model
  that `resume_with_state` takes;
- `decision_executor(context)`: the `AgentExecutor` that `resume_decision_hold` wraps.

Either method may answer `NotRebuildable` with a cause, and `resume_decided_request` returns that
refusal without calling Commission. It calls the builder only after the hold step, so
`NothingHeld`, `StaysHeld`, `AwaitsRecord`, `Stopped` and `HeldByMoreThanOneCommission` call it 0
times.

For a `Decision` hold, `resume_decided_request` builds no `IntentRequest` and does not call
`resume_prepared`. The executor from the host and the stores are what `resume_decision_hold`
takes, so a host resumes a `Decision` hold of any protocol in the catalog.

The host supplies the catalog and the governor, as `resume` builds them: `b10x-loom` reads the
user's protocol store (`InstallStore::user`, `crates/loom-cli/src/main.rs:650-652`) and builds
`slice_governor` over the state directory's `FileCaseStore` (`crates/loom-intake-slice/src/run.rs:174`,
`main.rs:653-657`).

`ResumeHost` is taken: it is the slice's struct of what a resume needs from the host
(`crates/loom-intake-slice/src/intent.rs:354`). `b10x-loom` implements `approval` with the code
`resume` uses today (`main.rs:658-681`), moved into one function both call. Its
`decision_executor` always answers `NotRebuildable` (above). `ResumeBuilder` is declared in Rust
only, because no command input comes from it (the epic's host-port rule). As a port a host
implements, it gets:

- a `host_ports!` row in `crates/loom-sdk/tests/host_ports.rs` (the invocation at `:48`);
- a row under "Ports a host implements" in `website/docs/reference/sdk-stability.md` (`:73`).

The SDK equivalent is `loom_sdk::intake::slice::decided_resume::resume_decided_request`, through
the existing re-export (`crates/loom-sdk/src/lib.rs:81`), listed under "Types and functions an
embedding calls" (`sdk-stability.md:109`). A host calls it after `loom_sdk::decisions::decide`,
`take` or the record completion, which stay record-only.

`b10x-loom stop --commission C --as P --reason TEXT --state DIR` stops a held run, through the
library function `stop_held_run` in `decided_resume.rs`, which a host calls too:

- It takes the commission's `Pending` Suspension and releases it, performing nothing (Commission's
  `TakeSuspension` and `ReleaseSuspension`), and drops the resume context
  (`intake.routing.DropResumeContext`, `crates/loom-intake-slice/src/resume.rs:322`).
- It keeps a `RunStopped` record in the state directory, one file per commission: commission,
  the request its hold named if any, principal, reason, instant.
- The stored state tells a run a person stopped apart by that record and by the absence of its
  Suspension. Nothing stored before this story told such a run apart: the slice has no command
  that stops a held run. `HostStop` (`ess/commission/domains/responsibility.yaml:158`) is the
  host's stop of a running run, and its Suspension names no request.
- After `stop`, no path resumes the run: `tick --resume` and `decide` find no hold, and `resume`
  is refused `NothingSuspended`.
- `decide`, `take` and `record` of a request whose run was stopped get `Stopped` from
  `resume_decided_request`. They print `stopped: <commission> by <principal> (<reason>)` in place
  of `resumed: nothing …` and exit 0.
- Exit status of `stop`:
  - `0`: stopped;
  - `10`: `NothingSuspended`;
  - `11`: `InDoubt`, a resume took the Suspension;
  - `20`: the suspension store could not be read or written;
  - `2`: the command line is not valid;
  - `1`: anything else, including a resume-context store or `RunStopped` record that cannot be
    read or written.

  `10`, `11` and `20` are `resume`'s numbers for the same causes (`crates/loom-cli/src/lib.rs:119`,
  `:120`, `:129`). As `resume` gives an unusable resume context `1` (`:133`), so does `stop`.

Three tests of `crates/loom-cli/tests/approval_decisions.rs` change, because `decide` and `take`
with `--state` now resume:

- The `A0` decide test: `decide` performs the merge once, and a later `resume` exits 10.
- The case-moved test: it decides through `loom_sdk::decisions::decide` before it moves the case.
- The `path_line` take test: it takes through `loom_sdk::decisions::take` before it rewrites the
  lines.

The tests of `crates/loom-cli/tests/question_outcome_decisions.rs` hold no commission, so their
requests print `resumed: nothing …` and their assertions stand.

Files:
- `crates/loom-cli/src/decisions.rs`: the resume step, `--commission`, the second-decide rule,
  and `tick --resume`.
- `crates/loom-cli/src/lib.rs`: `decided_resume_exit_status`, the help lines, the `Stop` command
  and its exit-status help.
- `crates/loom-cli/src/main.rs`: the `ResumeBuilder` implementation, shared with `resume`, and
  the `stop` arm.
- `crates/loom-intake-slice/src/decided_resume.rs` (new) and `src/lib.rs` (its `mod` line);
  `RunStopped` is kept beside the resume contexts (`crates/loom-intake-slice/src/resume.rs`).
- `ess/intake/domains/routing.yaml`.
- `crates/loom-cli/tests/decisions_resume.rs` (new), `crates/loom-intake-slice/tests/decided_resume.rs`
  (new), and `crates/loom-cli/tests/approval_decisions.rs` (the three tests above).
- `website/docs/reference/sdk-stability.md` and `crates/loom-sdk/tests/host_ports.rs` (the port
  row and the function path), `website/docs/guides/decisions.md`, and
  `website/docs/reference/cli.md` (regenerated).

## Why

Session `sdc` answered on 2026-11-21 (`.engineering/drafts/e3-sdc-answers-7.md`, point 1). SDC
needs a command path: a person answers in the SDC UI or on a terminal, and the held run must
resume without a custom host program. The command that records the answer resumes the run in the
same invocation and prints its next state; there is no separate resume command. Session `sdc`
answered again on 2026-11-21 (`.engineering/drafts/e3-sdc-answers-8.md`): an unattended run must
be unblocked by silence without a person, so `decisions tick --state DIR --resume` resumes held
runs, and a tick without the flag stays record-only. Session `sdc`'s third answer
(`.engineering/drafts/e3-sdc-answers-10.md`) widens this:
- `--resume` resumes every run held on a request decided by `silence`, this tick's or an earlier
  one's;
- the report names each run and its exit status;
- a run a person stopped is never resumed.

Session `sdc` accepted the command line's limit (`.engineering/drafts/e3-sdc-answers-11.md`, (a)):
the command line resumes approval holds of `software-change@1` runs, and a held question or
outcome resumes through the SDK function. SDC drives case trees, which continue through `tree run`
or the daemon, and it holds no questions in slice runs driven through the binary. It also
accepted `b10x-loom stop` ((b)).

A decision under `--tree` resumes nothing by itself, but prints the case it unblocked and the
command that continues it.

Before this story nothing but a library test calls `resume_after_decision`.
`story:decision-suspension-reason` left `HeldForAuthority` and `OtherDecision` unnumbered, and they
stay so on the command line, which never reaches them (exit `32` is not used).
`story:question-and-outcome-decisions-hold-runs` reaches them only through the library function
`resume_decision_hold`, which lands after `story:decision-inbox-commands`, so the resume cannot be
part of the inbox story.

## Waits on

- `story:question-and-outcome-decisions-hold-runs`: `resume_decision_hold`, `decisions record`
  and the `--state` writers. Both edit `crates/loom-cli/src/decisions.rs`, `lib.rs`, `main.rs`
  and `crates/loom-intake-slice/src/lib.rs`.
- `story:approvals-supplied-to-canon-evaluation`: both edit `crates/loom-cli/src/main.rs` and
  `lib.rs`, and nothing else orders them.
- Through those two: `story:approval-decisions-resume-held-runs` (`DecisionAuthority` on resume,
  `--state` on `decisions`, and the tests this story changes), `story:decision-inbox-commands`
  (`decide`, `take` and their exit table) and `story:decision-suspension-reason` (the `Decision`
  hold and the two refusals numbered here).
- `story:silence-and-expiry-at-a-host-tick`: `tick` and its `TickReport`, which `tick --resume`
  runs first. A direct edge keeps this order even if the path through the approval story changes.
- `story:tree-waits-open-decision-requests` comes after this story: both edit
  `crates/loom-cli/src/decisions.rs`, `lib.rs` and `main.rs`.
- Stories outside this epic that edit these files are in the typed scope, and
  `aep plan artifact waves` keeps them apart.

## ESS first

`ess/intake/domains/routing.yaml` gains:

- `DecidedResume`, a union: `NothingHeld`, `StaysHeld { commission, capability }`,
  `AwaitsRecord { commission }`, `Stopped { commission, principal, reason }`,
  `Resumed { commission }`;
- the refusal `HeldByMoreThanOneCommission` (the commissions);
- the refusal `NotRebuildable` (commission, cause: `decision_hold` or `not_a_software_change`,
  detail), which a `ResumeBuilder` answers;
- `TickResume`: `request_id`, `commission`, `outcome`, optional `stop`, `exit_status`, optional
  `error`;
- `RunStopped`: commission, optional request id, principal, reason, instant;
- the command `StopHeldRun`, which keeps `RunStopped` (as `KeepResumeContext` keeps a context).

On that commit `task intake-drift` fails, and
`crates/loom-cli/tests/decisions_resume.rs` and `crates/loom-intake-slice/tests/decided_resume.rs`
do not compile. `ess/domains/decision.yaml` is unchanged.

## Acceptance

On the command line (`crates/loom-cli/tests/decisions_resume.rs`). Approval bullets use the
fixture of `crates/loom-cli/tests/approval_decisions.rs` (`software-change@1`, stub programs, a
recorded selector, fixture autonomy documents and principals), with `run --catalog FILE` naming a
recorded llm catalog, so the resume context names that file (`crates/loom-cli/src/main.rs:658-661`).

The command line resumes approval holds only (§ Outcome), so the command-line bullets resume
approval holds of the approval fixture. A question or outcome hold cannot be built through
`b10x-loom run`, and the command line cannot resume one. The command-line bullets that need such a
hold (the `NotRebuildable` bullet, the question `decide`, `stop` and `record` after `stop`, and the
`awaits_record` tick) use a state directory the test builds through the entries
`story:question-and-outcome-decisions-hold-runs` uses (its § Acceptance):

- the fixture protocol of `crates/loom-intake-slice/tests/decision_hold.rs` (root outcome `close`
  blocked on decision `release`, artifacts `intent` and `answers`), installed with
  `b10x-loom protocols install` into the user protocol store under the test's `XDG_DATA_HOME`
  (`crates/loom-protocols/src/store.rs:22-40`), so the command's governor knows the case's
  protocol;
- an outcome hold for `DR-o`, built by the slice's `run_with_clock`
  (`crates/loom-intake-slice/src/run.rs:417`) with the decision flags and a recorded agent the test
  asserts is never called;
- a question hold for `DR-q`, built by Commission's `run_with_suspension`
  (`crates/loom-commission/src/runtime.rs:1181`) with `DecisionHoldingExecutor` wrapping a
  scripted inner executor that returns `NeedsHumanJudgment` with a question and options `A`, `B`;
- for each held commission, its resume context, kept by the setup through
  `ResumeContextStore::keep` (`crates/loom-intake-slice/src/resume.rs:190`) when the entry keeps
  none, so the resume step finds it.

No model is called.

- At `A0`, with the run holding an `Authority` Suspension for `DR-…`,
  `decisions decide DR-… A --as timo --state DIR` performs the held merge request once (the stub
  merge program records one call).
- That `decide` prints `resumed: <commission> (DR-…)` and then a last line `stopped: …`, and exits
  with the status `run_exit_status` gives that stop line.
- After it, `b10x-loom resume --commission <commission> --state DIR` exits 10 and performs nothing.
- `decisions take DR-… --as timo --state DIR` on a strong recommendation `A` performs the held
  merge request once.
- `decisions decide DR-… B --as timo --state DIR` (deny) exits 0, prints
  `held: <commission> (DR-… denies repository.merge)`, performs nothing, and leaves the Suspension
  `Pending`.
- On a request opened by `route_stop` in setup that no commission holds,
  `decisions decide … --state DIR` prints `resumed: nothing (no run in DIR holds DR-…)` and exits 0.
- `decisions decide … --store DIR` prints no line starting `resumed:` or `held:`.
- With the model catalog file named by the commission's resume context removed, `decide` exits 33,
  standard error names the file and says the decision is recorded, and nothing is performed.
- After that failed resume, `decisions show DR-… --state DIR` prints the request Decided with its
  `produced_record`.
- After the file is restored, a second `decide` of the same request names
  `DecisionStateConflict` on standard error and performs the held merge request once.
- With two commissions whose Suspensions name `DR-…` (the second held by the test's setup through
  `HoldSuspension`, its context kept through `ResumeContextStore::keep`), `decide` exits 34 naming
  both, and nothing is performed.
- After that, `decide` of the same request with `--commission <first>` resumes the first only: the
  second's Suspension is still `Pending`.
- `decide` of the approval request with `--commission C`, where C holds a `Decision` Suspension,
  exits 30 naming `HeldForDecision`, and C's Suspension file is byte-identical.
- In the library-built state directory, `decisions decide DR-o A --as timo --state DIR` records
  `close/release@r<N>` and exits 31. Standard error names `NotRebuildable`, says the decision is
  recorded, and names `loom_sdk::intake::slice::decided_resume::resume_decided_request`. The
  outcome commission's Suspension file is byte-identical.
- In the library-built state directory, `decisions decide DR-q A --as timo --state DIR` prints the
  `decisions record` command, prints no `resumed:` line, and leaves the question commission's
  Suspension file byte-identical.
- `decisions decide --help` lists the statuses 30, 31, 33 and 34, and no 32.

`stop` bullets, in the same file:

- `b10x-loom stop --commission C --as timo --reason "wrong intent" --state DIR` on the approval
  fixture's held commission exits 0. The Suspension file and the resume-context file are gone,
  and a `RunStopped` file names `C`, `DR-…`, `timo` and the reason.
- After it, `b10x-loom resume --commission C --state DIR` exits 10 and performs nothing.
- After it, `decisions decide DR-… A --as timo --state DIR` records the grant, prints
  `stopped: C by timo (wrong intent)`, exits 0, and the stub merge program records no call.
- After it, `decisions take DR-… --as timo --state DIR`, on the approval fixture's strong
  recommendation `A`, records the grant, prints `stopped: C by timo (wrong intent)`, exits 0, and
  the stub merge program records no call.
- `b10x-loom stop --commission C --as timo --reason "wrong intent" --state DIR` on the question
  commission of the library-built state directory exits 0. Then, after
  `decisions decide DR-q A --as timo --state DIR`, the command
  `decisions record DR-q --revision answers@a2 --state DIR` sets `produced_record` `answers@a2`,
  prints `stopped: C by timo (wrong intent)` and exits 0.
- `decide DR-… --commission C --state DIR`, where C was stopped, prints
  `stopped: C by timo (wrong intent)` and exits 0, and no Suspension file is written for C.
- `stop` on a commission that holds no Suspension exits 10.
- `stop` on a commission whose Suspension the setup has taken (`TakeSuspension` through the state
  directory's suspension store, not released) exits 11. The Suspension file is byte-identical,
  the resume-context file is still there, and no `RunStopped` file is written.
- `stop` with the state directory's suspension directory made unreadable by the test exits 20,
  and the resume-context file is byte-identical.

Tick bullets, in the same file. The setup is two runs of the approval fixture at `A1` in one
state directory, holding `Authority` Suspensions for `DR-a` and `DR-b`. The two runs name
different `--catalog` files, `catalog-a.json` for `DR-a`'s and `catalog-b.json` for `DR-b`'s,
with the same recorded content, so removing one fails only its run. Each request has a recorded
decider reply `A`, strong, with a default on silence, as the approval story's `A1` silence bullet
sets it (`story:approval-decisions-resume-held-runs`). `run` takes no instant, so the two
`expires_at` differ. Each is read from `decisions show`, and `E` is the later of the two.

- `decisions tick --at E --state DIR --resume` resumes both runs, `DR-a` before `DR-b`: standard
  output's second and third lines are `TickResume` lines for `DR-a` and then `DR-b`, each naming
  its commission, its stop line and `exit_status`.
- In that tick the stub merge program records two calls: `DR-a`'s commission first, then
  `DR-b`'s.
- With `catalog-a.json` removed, the same tick still performs
  `DR-b`'s merge once. `DR-a`'s `TickResume` carries `exit_status` 33, and the tick exits 35.
- After that, with the file restored, a second `decisions tick --at E --state DIR --resume`
  decides nothing new and performs `DR-a`'s merge once: a request an earlier tick decided is
  resumed.
- A first `decisions tick --at E --state DIR` (no `--resume`) decides both. The test then raises
  an artifact of `DR-a`'s case through `CanonGovernor::update_revision`. A second tick with
  `--resume` gives `DR-a` a `TickResume` with `exit_status` 30 naming `CaseMoved`, still
  performs `DR-b`'s merge once, and exits 35.
- `decisions tick --at E --state DIR` without `--resume` decides both by `silence`, prints only the
  `TickReport` line, and leaves both commissions' Suspension and resume-context files
  byte-identical, with no stub merge call.
- With `DR-a`'s run stopped by `b10x-loom stop` before the tick, `tick --at E --resume` decides
  `DR-a` by `silence`, gives it no `TickResume` line, writes no file for `DR-a`'s commission, and
  performs only `DR-b`'s merge.
- A request with no default, held by a run and ticked past its `expires_at` with `--resume`, is
  Expired. Its commission's Suspension file is byte-identical, and standard output has no
  `TickResume` line for it.
- At the question hold of the library-built state directory, with `DR-q` given a strong default
  on silence by the setup, `tick --resume` past its `expires_at` decides it by `silence` and
  gives it a `TickResume` with outcome `awaits_record` and `exit_status` 0. The Suspension file is
  byte-identical.
- The error path is a separate setup:
  - A first `decisions tick --at E --state DIR` (no `--resume`) decides `DR-a` and `DR-b` by
    `silence`, and its `TickReport` lists both under taken on silence.
  - The setup then opens a third request, `DR-c`, by `route_stop` with a default on silence and
    `expires_at` before `E`, and no commission holds it. The test makes `DR-c`'s case file
    unreadable.
  - `decisions tick --at E --state DIR --resume` then exits 20, the inbox's status for a
    `CaseRevisions` that cannot answer (`story:decision-inbox-commands`). It prints no
    `TickResume` line, leaves both commissions' Suspension files byte-identical, and the stub
    merge program records no call.
- After the case file is restored, the next `decisions tick --at E --state DIR --resume` resumes
  `DR-a`'s and `DR-b`'s runs, which the first tick's report shows decided before the error. The
  stub merge program records two calls.
- `decisions tick --at E --store DIR --resume` exits 2 before it reads the store.
- `decisions tick --help` lists the status 35.

As a library test (`crates/loom-intake-slice/tests/decided_resume.rs`), with no model call. The
holds are built as in the library-built state directory above: an outcome hold through
`run_with_clock`, a question hold through `run_with_suspension` with `DecisionHoldingExecutor`
over a scripted inner executor, and each context kept through `ResumeContextStore::keep`.
Decisions go through `loom_sdk::decisions` with the `--state` writer set. The test's
`ResumeBuilder` counts its calls. Its `decision_executor` returns the scripted inner executor of
`crates/loom-intake-slice/tests/decision_hold.rs`, and its `approval` answers `NotRebuildable`.

- With the outcome hold decided `A`, `resume_decided_request` returns `Resumed`, and the run's
  `SliceRun` stops `Completed` through `close`. `decision_executor` is called once.
- With the outcome hold decided `B` ("leave it untaken"), it returns `Resumed`, the run's
  `SliceRun` stops `NothingAdmissible`, and the Suspension is released.
- With a question hold decided `A` and `answers@a2` recorded through `complete_decision`,
  `resume_decided_request` returns `Resumed`, and the run continues at the moved case revision.
- With the outcome request and `Some(C)`, where C holds an `Authority` Suspension (held by the
  setup through `HoldSuspension`), it is refused `HeldForAuthority`, and C's Suspension stays
  `Pending`.
- With outcome request `DR-1` and `Some(C)`, where C holds the `Decision` Suspension for `DR-2`,
  it is refused `OtherDecision` naming `DR-2`.
- With the question request recorded `answers@a2` and `Some(C)`, where C holds the outcome hold, it
  is refused `OtherDecision`, and the question request keeps `produced_record` `answers@a2`.
- With a `ResumeBuilder` whose `decision_executor` answers `NotRebuildable`, the decided outcome
  request is refused `NotRebuildable`, the Suspension stays `Pending`, and the request stays
  Decided with its `produced_record`.
- After `stop_held_run` on the outcome commission, `resume_decided_request` for `DR-o` returns
  `Stopped` naming the commission, the principal and the reason. With `Some(C)` naming that
  commission, it returns the same `Stopped`. The `ResumeBuilder` is called 0 times in both.
- With the question decided and not recorded, `resume_decided_request` returns `AwaitsRecord`,
  the `ResumeBuilder` is called 0 times, and the Suspension stays `Pending`.
- `crates/loom-sdk/tests/host_ports.rs` passes with the `host_ports!` row for
  `loom_sdk::intake::slice::decided_resume::ResumeBuilder` and its row under "Ports a host
  implements".
- For a request no commission holds, it returns `NothingHeld`, and the `ResumeBuilder` is called 0
  times.
- For an approval decided on the denying option, it returns `StaysHeld` naming the capability, and
  the `ResumeBuilder` is called 0 times.
- For a request two commissions hold, it is refused `HeldByMoreThanOneCommission` naming both, and
  the `ResumeBuilder` is called 0 times.
- `crates/loom-sdk/tests/host_ports.rs` passes with
  `loom_sdk::intake::slice::decided_resume::resume_decided_request` on
  `website/docs/reference/sdk-stability.md`.
- `crates/loom-cli/tests/approval_decisions.rs` passes, with only its three tests named in the
  Outcome edited.
- `crates/loom-cli/tests/question_outcome_decisions.rs` passes unchanged.
- `task docs-check` exits 0, with `website/docs/reference/cli.md` regenerated.
- `task check` exits 0.

## Out of scope

- A separate resume command.
- Resuming a question or outcome hold from the command line, and a general rebuild of a commission
  from its resume context, which would need the context to keep the bindings or the execution
  kind. That is a further story if SDC holds questions in slice runs driven through the binary,
  which session `sdc` says it does not (`.engineering/drafts/e3-sdc-answers-11.md`, (a)). A tree
  continues through `tree run` or its daemon.
- Resuming the runs of requests a tick expired.
- Continuing a tree from `decisions … --tree` (`story:tree-waits-open-decision-requests`: the next
  `tree run` continues).
- Ending a run whose approval was denied (`b10x-loom stop` ends it on a person's word).
- Stopping a run that is running, not held (the host's `HostStop`).
- `b10x-loom resume` of a `Decision` hold, which stays 19 (`story:decision-suspension-reason`).
- `resume --mandate` on a decision-held commission (`story:approval-decisions-resume-held-runs`
  refuses it).
- A question's answer written into an application file (the host writes it, then runs
  `decisions record`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-cli` (`decisions` resume step, `--commission`, `tick --resume`, exit table) — cited, story "Files"
- **Files:** `crates/loom-cli/src/decisions.rs` — cited. Does not exist yet; created by `story:decision-inbox-commands`.
- **Files:** `crates/loom-cli/src/lib.rs` (`decided_resume_exit_status` beside `resume_exit_status` at `:154`, help lines) — cited
- **Files:** `crates/loom-cli/src/main.rs` (`resume` at `:616-685` moved into one function both call) — cited
- **Files:** `crates/loom-intake-slice/src/decided_resume.rs` (new) and `crates/loom-intake-slice/src/lib.rs` (a `mod` line) — cited
- **Files:** `ess/intake/domains/routing.yaml` (`DecidedResume`, `HeldByMoreThanOneCommission`, `TickResume`) — cited
- **Files:** `crates/loom-cli/tests/decisions_resume.rs` (new), `crates/loom-intake-slice/tests/decided_resume.rs` (new), `crates/loom-cli/tests/approval_decisions.rs` (three tests) — cited
- **Files:** `website/docs/reference/sdk-stability.md`, `crates/loom-sdk/tests/host_ports.rs`, `website/docs/guides/decisions.md` — cited
- **Symbols (exist):** `resume_with_state` (`intent.rs:654`), `resume_after_grant` (`runtime.rs:2374`), `ResumeContextStore::list` (`resume.rs:291`) and `::keep` (`:190`), `ResumeState` (`intent.rs:536`), `run_exit_status` (`lib.rs:177`), `LOCAL_PROTOCOL` (`run.rs:163`) — cited
- **Symbols (absent, from undelivered dependencies):** `resume_decision_hold`, `DecisionAuthority`, `HeldForDecision`, `HeldForAuthority`, `OtherDecision`, `decisions.rs`, `approval_decisions.rs`, `question_outcome_decisions.rs`, `decision_hold.rs` — cited. `git grep` finds none.
- **Also likely:** `crates/loom-intake-slice/Cargo.toml` (a dependency on `b10x-loom-decision` for the decision-store argument; the dependency stories may already add it) — inferred
- **Documents:** `website/docs/guides/decisions.md`, `sdk-stability.md`; `cli.md` is regenerated (house-rule file) — cited
- **Confidence:** medium. The story names every file and most symbols, and its line citations check out. But the surface it edits (`decisions.rs`, the new refusals, `resume_decision_hold`) exists only in seven undelivered dependencies, so what it calls cannot be checked against code.
- **Would collide with** (`depends_on` read from `aep plan artifact graph --format json`):
  - `decisions.rs`, `lib.rs`, `main.rs`, ordered BEFORE this story by `depends_on`: `story:question-and-outcome-decisions-hold-runs` (direct), `story:approvals-supplied-to-canon-evaluation` (direct), `story:approval-decisions-resume-held-runs`, `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick`, `story:decision-suspension-reason` and `story:decided-record-written-then-recorded` (all transitive). `approval_decisions.rs` and `routing.yaml` also come from `approval-decisions-resume-held-runs` and `question-and-outcome-decisions-hold-runs`.
  - `decisions.rs`, `lib.rs`, `main.rs`, ordered AFTER by `depends_on`: `story:tree-waits-open-decision-requests` (direct) and `story:daemon-continues-a-decided-request` (direct; also `decisions.rs` and `guides/decisions.md`). `story:authority-records-bound-and-grounded-in-canon` edits `routing.yaml` and is ordered after (transitive).
  - `crates/loom-cli/src/lib.rs` and `main.rs`, **no `depends_on` path** (typed scope of each names both files): `story:tree-run-command`, `story:agent-selects-the-connectors-relay`, `story:bound-commands-run-confined`, `story:agent-node-run-records`, `story:confinement-opens-from-engine-config`, `story:secrets-subcommand`. `story:tree-daemon` names `main.rs` in its body only.
  - `lib.rs` only, no path: `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant`, `story:tree-usage-and-model-budget`, `story:run-caps-per-arrival`.
  - `website/docs/reference/sdk-stability.md`, no path: `story:agent-selects-the-connectors-relay`, `story:connectors-relay-client`, `story:child-launches-through-confinement-adapters`, `story:canon-0-9-0-pin` (active). Body mentions only: `story:consumer-sdk-paths`, `story:generated-types-release-surface` (also `host_ports.rs`), `story:co-pinned-consumer-check`. All are table-row additions.
  - Not a collision: `intent.rs` is read, not edited, by this story (`bound-commands-run-confined` and `confinement-opens-from-engine-config` edit it). `routing.yaml:247` in `story:engineering-registry-calls-replaced` is a comment it leaves alone.
- **Safety fact:** the exit statuses 30 to 35 are free everywhere, so no new mapping changes an existing one — step 2, unproven.
  - `lib.rs` numbers 0-4, 10-23 and `resume_refusal_status` (`lib.rs:139-150`), `approve_exit_status` (`:163-171`, 23 the highest); `main.rs:778` exits 130.
  - Tree stories number 3, 4, 5, 6 (`tree-daemon.md:149`) and 7 (`standing-tree-rests-quiescent.md:61`).
  - SDC's exit 6 (`approval-decisions-resume-held-runs.md:109`) and `autonomy-grant-table.md:91`'s exit 64 are quotations of SDC's own scripts, not Loom's.
  - `grep` over `story/` and `epic/` for "exit/status NN" with NN in 24 to 49 finds only this story.
