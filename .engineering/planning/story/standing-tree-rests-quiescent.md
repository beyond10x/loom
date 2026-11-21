---
format: aep.planning-md/3
id: story:standing-tree-rests-quiescent
kind: story
status: proposed
title: A tree opened --standing rests Quiescent with no run slot when nothing is to do and nothing is wrong, and names its next expiry
summary: TreeHorizon, exit 7, ServedTree Quiescent with no slot, next_expiry from held records and max_age with no clock read; creates the area fixture; bounded trees still exit 4
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:tree-daemon
scope:
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/area-stuck.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/area.bindings.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/area.protocol.yaml
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/health
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/retire-check
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/triage
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/bin/triage-invalid
- confidence: cited
  path: crates/loom-cli/tests/fixtures/area/engine-daemon-1.yaml
- confidence: cited
  path: crates/loom-cli/tests/tree_standing.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/completion_rule.rs
- confidence: cited
  path: crates/loom-governor/tests/next_expiry.rs
- confidence: cited
  path: crates/loom-tree
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: inferred
  path: ess/scenarios/tree-run
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:29:07Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`tree open --standing` opens a tree whose horizon is `Standing`; without the flag it is `Bounded`,
as every tree is today. The store keeps the horizon beside the root's case file, and `tree status`
prints it.

This story is written against the per-tree stop of `story:tree-daemon` and the sequential run of
`story:tree-run-command`, and lands before `story:concurrent-cases-in-the-tree-daemon`, which
places `Quiescent` among its per-case states and gives the one stop order over cases; for a tree
whose only open case is the root that order gives what this story gives. A standing tree rests
`Quiescent` where a bounded tree would stop `Stuck`: no case has a useful action, none waits on a
person, authority or a decision request, and in addition every case that is not terminated has no
open obligation and no legitimate outcome. A direct `tree run` then exits 7, a new status
(accepted by session `sdc` on 2026-11-21), and prints `Quiescent` with the tree's next expiry.
Through the daemon the served tree is `Quiescent` and holds no run slot, as a tree that waits on a
person holds none; a blocking `tree run` through the daemon exits 7; after a restart of the daemon
it is `Quiescent` again, derived from its root's last `stopped` line, and no step of it starts. A
standing tree with a case that has an open obligation and nothing useful stops `Stuck`, exit 4, as
a bounded tree in either state does; a bounded tree never rests `Quiescent`.

The next expiry is the earliest instant at which a record held by any case of the tree, applying
at the evaluation's instant and of a kind with a `max_age`, stops applying: its `observed_at` plus
the kind's `max_age` plus one second, by the governor's freshness rule. A new public method,
`CanonGovernor::next_expiry(&self, case: &CaseId, at: i64) -> Result<Option<i64>, GovernorError>`,
returns it for one case: `at` and the result are whole seconds since the Unix epoch, the unit the
private `expired` reads (`crates/loom-governor/src/lib.rs:1943`, `at: Option<i64>`). It takes the
case's held records and its protocol's IR as `held_with_ir` gives them (`:1301`) and reads no
clock: neither the host clock nor the governor's `EvaluationTime` (`:424-444`). The tree takes the
least over its cases, or none. `tree status` prints it per tree.

Nothing changes in how a case completes: the governor terminates a case only through exactly one
legitimate outcome, so a standing protocol with no legitimate outcome is never closed by Loom. The
rule is stated in `AGENTS.md` § Checks that hold the rules and no test holds it for zero
legitimate outcomes; this story adds that test.

The fixture directory `crates/loom-cli/tests/fixtures/area/` is created here, beside the existing
`fixtures/catalog-route/` and `fixtures/session/`, with the files listed in § Acceptance. Later
stories of the epic add files to it by name and change none of these.

## Why

"a standing case never ends done, it is *quiescent* (nothing to triage, nothing accepted and
undelivered, the release observed healthy within its max age) and becomes active again on new
evidence; only `retired` ends it" (`~/beyond10x/sdc/docs/orchestration.md` § 0, answer 3);
"Quiescence is the runtime's resting state: no useful action, nothing awaiting a person. A standing
tree in quiescence holds no run slot" (§ 3); "quiescent: ... every obligation discharged, no
useful action; the case stays open" (§ 3, table). The draft protocol declares the horizon as an
input fixed at opening (`~/beyond10x/sdc/protocol/draft/orchestrate.protocol.yaml:26-30`) and
outcomes `done` and `retired` only.

Today such a tree reads as stuck: `story:tree-run-command` § Acceptance, "A tree with no useful
action, no legitimate outcome and nothing awaiting approval exits 4 and names the case."
`story:tree-daemon` § Outcome frees a slot only for a wait: "a tree that waits on a person uses no
run slot". Blocker `loom-standing-trees` also asks that "the governor never completes a case that
has no legitimate outcome". `AGENTS.md` § Checks that hold the rules states it ("A case the
governor holds completes only when exactly one declared outcome is legitimate"), but its three
tests in `crates/loom-governor/tests/completion_rule.rs`
(`first_declared_completes_on_the_first_declared_legitimate_outcome`,
`a_blocked_outcome_is_never_completed_under_first_declared`,
`with_one_legitimate_outcome_both_rules_agree`) cover no case with zero legitimate outcomes. The
expiry rule is the governor's `expired` (`crates/loom-governor/src/lib.rs:1938-1954`: expired when
the instant is later than `observed_at` by more than `max_age`), so the next expiry needs no Canon
change. The tests run under the Canon pin of `story:canon-0-9-0-pin`, 0.9.0, which this story
waits on.

## Waits on

- `story:tree-daemon`: `ServedTree` in `ess/domains/tree.yaml`, the slot accounting and the
  restart path this story extends; the last story of the daemon epic's chain to edit
  `ess/domains/tree.yaml` and `crates/loom-tree` before this one.
- Through it, `story:tree-run-command` (`TreeStop`, the exit table, `tree open`, `tree status`) and,
  through `story:record-copy-between-cases`, `story:child-case-runtime`, which edits
  `crates/loom-governor/src/lib.rs` before this story's `next_expiry`. So this story, like every
  story of the epic, waits on Canon 0.9.0 through `story:canon-0-9-0-pin`.

Stories of this epic that come after this one on shared files, each with a `depends_on` edge
naming the file: `story:run-caps-per-arrival` (`ess/domains/tree.yaml` `TreeStop`,
`crates/loom-tree`, `crates/loom-cli/src/lib.rs`, `fixtures/area/`), and through it every later
story of the epic.

Unordered with this story on shared files, under the store's rule that the second to land rebases
(the epic's § Order lists every such story): `story:tree-waits-open-decision-requests`
(`ess/domains/tree.yaml` `TreeStop` and `TreeStatus`, `crates/loom-tree`,
`crates/loom-cli/src/lib.rs`), an ancestor of `story:concurrent-cases-in-the-tree-daemon`, so both
land before that story; and the stories that edit `crates/loom-governor/src/lib.rs` elsewhere. No
edge is added for them.

## ESS first

`ess/domains/tree.yaml`: `TreeHorizon` (`Bounded`, `Standing`); `TreeRun` gains `horizon`;
`TreeStop` gains `Quiescent` (optional `next_expiry`); the `ServedTree` lifecycle gains
`Quiescent`; `TreeStatus` gains `horizon` and optional `next_expiry`. On that commit `task drift`
fails, the tree domain's scenarios are red, and `crates/loom-cli/tests/tree_standing.rs` does not
compile.

## Acceptance

The fixture `crates/loom-cli/tests/fixtures/area/`, created here:

- `area.protocol.yaml`, `area/1`: placed artifacts `intake`, `release`, `retire` and `notes` (no
  action reads `notes`; it serves the wake checks of `story:standing-tree-wakes-at-a-host-instant`);
  evidence kinds `triage` (about `intake`), `health` (about `release`, `max_age: 1h`) and
  `retire_order` (about `retire`); three observations, `intake.triage` (may produce `triage`),
  `health.observe` (may produce `health`) and `retire.check` (may produce `retire_order`); the
  obligation `triage.required`, discharged by a `valid` `triage` about the current `intake`; one
  outcome, `retired`, legitimate when a `retire_order` `ordered` about the current `retire` is
  held. No other outcome, no action bound to a person.
- `area.bindings.yaml`: `intake.triage` bound to `bin/triage` (exit 0) with `results: {"0":
  valid, "*": invalid}`, `health.observe` to `bin/health` (exit 0, `healthy`), `retire.check` to
  `bin/retire-check` (exit 0, `ordered`, when the file `retire` holds the word `retire`; else
  exit 1, `none`).
- `area-stuck.bindings.yaml`: the same, with `intake.triage` bound to `bin/triage-invalid`
  (exit 1, so `invalid`), no `retry_when`.
- `engine-daemon-1.yaml`: adapter `daemon`, one run slot.

All three actions are non-`write`, so the selector takes them by identifier: `health.observe`,
`intake.triage`, `retire.check` (the sort key, whether the action writes, at
`crates/loom-executor/src/selection.rs:331`, and the least of `(writes, id)` at `:336-338`); each
produces its kind about its subject's current revision, so after one run at a case revision none
is useful again until its subject moves (`selection.rs:285-305`). The tests start with `retire`
empty. Stub programs, no model; `crates/loom-cli/tests/tree_standing.rs` drives `b10x-loom tree`:

- `tree open --standing` with `area.bindings.yaml`, then `tree run --at T`: the log holds
  `health.observe`, `intake.triage`, `retire.check` (`none`), the run exits 7, prints `Quiescent`
  and the next expiry `T + 3601 s`; `tree status` prints `Standing` and that instant.
- The same opened without `--standing`: the same three steps, then exit 4 `Stuck` naming the case.
- `tree open --standing` with `area-stuck.bindings.yaml`, `tree run --at T`: `triage.required`
  stays open, nothing is useful after the three steps, the run exits 4 `Stuck` naming the case and
  never 7.
- Through a daemon under `engine-daemon-1.yaml`: the blocking `tree run` of the standing tree
  exits 7 and `tree status` shows it served and `Quiescent`; then a bounded tree of the same
  fixture with `area-stuck.bindings.yaml`, submitted with `tree run --detach`, takes the one slot
  at once and stops `Stuck`; the daemon killed and started again shows the standing tree
  `Quiescent`, and its run logs gain no `action started` line.
- After the standing tree rests, the test writes `retire` into the file `retire`; the next `tree
  run --at T+60s` takes `retire.check` (`ordered`), terminates the root through `retired` and exits
  0.
- `next_expiry` (`crates/loom-governor/tests/next_expiry.rs`, a library test on a `CanonGovernor`
  over a `MemoryCaseStore`): two held records of a kind with `max_age: 30m` observed at 10:00 and
  10:10 give 10:30:01 (as whole seconds); a record no longer bound to the current revision, an
  expired record and a record of a kind without `max_age` give none. A second governor built over
  the same store with an `EvaluationTime` closure that answers `1970-01-01T00:00:00Z` for every
  case gives the same results, so the method reads `at` only.
- Completion (`crates/loom-governor/tests/completion_rule.rs`, new test
  `with_no_legitimate_outcome_neither_rule_completes`): a case whose protocol has declared outcomes
  none of which is legitimate at the evaluation is completed with no outcome under the default
  rule and under `CompletionRule::FirstDeclared`, and stays open.
- `website/docs/reference/cli.md` is regenerated and the exit table of `tree run` lists 7.
- `task check` exits 0.

## Files

`ess/domains/tree.yaml`, generated `generated/rust/loom/src/tree.rs`, `ess/scenarios/` (tree
domain); `crates/loom-governor/src/lib.rs` (`CanonGovernor::next_expiry`),
`crates/loom-governor/tests/next_expiry.rs` (new), `crates/loom-governor/tests/completion_rule.rs`
(one new test); `crates/loom-tree` (horizon, rest, the daemon's slot release);
`crates/loom-cli/src/lib.rs` (`--standing`, the exit table); `crates/loom-cli/tests/tree_standing.rs`
(new); `crates/loom-cli/tests/fixtures/area/` (new: `area.protocol.yaml`, `area.bindings.yaml`,
`area-stuck.bindings.yaml`, `bin/triage`, `bin/triage-invalid`, `bin/health`, `bin/retire-check`,
`engine-daemon-1.yaml`); `website/docs/reference/cli.md` (regenerated); `CHANGELOG.md`.

## Out of scope

Waking a quiescent tree (`story:standing-tree-wakes-at-a-host-instant`). Run caps
(`story:run-caps-per-arrival`). Per-case scheduling states and the stop order over cases
(`story:concurrent-cases-in-the-tree-daemon`). Changing the completion rule. A protocol-declared
horizon: Loom reads none from Canon; the flag is the host's.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the horizon, the rest decision, the daemon's slot release), with `ess/domains/tree.yaml` beside it. Neither exists at `61fd500b`: `story:tree-run-command` (proposed) creates both — cited (story § Files and § ESS first; `ls crates` has no `loom-tree`; `ess/domains/` has no `tree.yaml`)
- **Files, governor:** `crates/loom-governor/src/lib.rs` (new `next_expiry`, beside `expired` at :1938-1954 and `bound_to_current_revision` at :1928) — cited
- **Files, governor test:** `crates/loom-governor/tests/next_expiry.rs` (new) — cited
- **Files, CLI:** `crates/loom-cli/src/lib.rs` (`--standing`, exit table, `tree status` output) — cited
- **Files, CLI test:** `crates/loom-cli/tests/tree_standing.rs` (new) — cited
- **Files, fixture:** `crates/loom-cli/tests/fixtures/area/{area.protocol.yaml,area.bindings.yaml,area-stuck.bindings.yaml,bin/triage,bin/triage-invalid,bin/health,bin/retire-check,engine-daemon-1.yaml}` (new directory; `fixtures/` holds only `catalog-route` and `session` today) — cited
- **Files, ESS:** `ess/domains/tree.yaml` (`TreeHorizon`, `TreeRun.horizon`, `TreeStop::Quiescent`, `ServedTree` lifecycle, `TreeStatus`) — cited
- **Also likely:** `ess/scenarios/<tree-domain dir>`, whose name I could not read because the tree domain has no scenarios yet — inferred
- **Symbols:** `expired` (`crates/loom-governor/src/lib.rs:1943`, `fn expired(ir, record, at: Option<i64>)`); `CanonGovernor::evidence` (:1121); `HeldEvidence.applies` (:352-357); `EvaluationTime` (:424); `DeterministicSelector::useful` (`crates/loom-executor/src/selection.rs:285`) — cited
- **Symbols, new:** `next_expiry`, `TreeHorizon`, `TreeStop::Quiescent`, `ServedTree::Quiescent` — cited
- **Documents:** none beyond the generated reference pages the house rules already cover — cited
- **Confidence:** medium. High for the files and for the governor half, because the story names every path and the governor half is checked against code. The tree crate and domain are absent from the code, so everything there rests on the dependency stories' text.
- **Would collide with, ordered by `depends_on`** (this story first):
  - `story:run-caps-per-arrival`, `story:standing-tree-wakes-at-a-host-instant`, `story:concurrent-cases-in-the-tree-daemon`, `story:flow-limits-recorded-as-events`: direct `depends_on` on this story. They share `ess/domains/tree.yaml`, `crates/loom-tree`, `crates/loom-cli/src/lib.rs` and `fixtures/area/`. Ordered.
  - `story:capacity-given-to-an-admission-program`, `story:child-case-own-workspace`, `story:tree-usage-and-model-budget`: ordered through the stories above, on the same files.
  - `story:child-case-runtime`, `story:record-copy-between-cases`: ancestors, on `crates/loom-governor/src/lib.rs`. Ordered (:28 and :23 of their front matter).
  - `story:tree-daemon`, `story:tree-run-command` and the rest of the daemon chain (`action-limits-enforced`, `limit-hits-in-run-log`, `engine-configuration-document`, `confinement-level-required`, `executable-allow-list`, `named-egress-per-action`): ancestors, on `ess/domains/tree.yaml` and `crates/loom-tree`. Ordered.
- **Would collide with, not ordered** (no `depends_on` path either way):
  - `story:tree-waits-open-decision-requests`, on `ess/domains/tree.yaml` (`TreeStop` Stuck/Waiting, `TreeStatus`), `crates/loom-tree` and `crates/loom-cli/src/lib.rs` (its `Waits on` and `## ESS first`, :110-142). It lists its unordered neighbours at :125-133 and omits this story. It is an ancestor of `story:concurrent-cases-in-the-tree-daemon`, so both land before that story, in either order.
  - `story:ingress-routes-per-action`, `story:confinement-level-on-oci-and-pod`, on `crates/loom-tree` (named in tree-waits :129-133 — inferred, I did not read their Files).
  - `story:question-and-outcome-decisions-hold-runs`, `story:approvals-supplied-to-canon-evaluation`, on `crates/loom-cli/src/lib.rs`.
  - `story:approvals-supplied-to-canon-evaluation`, `story:authority-records-bound-and-grounded-in-canon`, `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick` and the other decision-epic stories, on `crates/loom-governor/src/lib.rs`. Different regions (`decide_case` :2042, evidence admission), so a textual overlap only. Inferred.
- **Safety fact:** `next_expiry` is additive. `expired` (`crates/loom-governor/src/lib.rs:1943`) is private and reads no clock, and the new function reuses its arithmetic (`at - observed > max_age`, whole seconds), so +1 s is the exact boundary — step 2, unproven
- **Safety fact, second:** a bounded tree cannot rest `Quiescent`, because the rest branch is gated on `Standing` and exit 4 stays the default. Nothing in the tree implements it yet — step 1 (story text), unproven
