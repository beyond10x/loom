---
format: aep.planning-md/3
id: review-result:adv-w8-u1-pass-1
kind: review-result
status: active
title: Adversary, loom w8 U1 a re-proposed merge at a handed-off HEAD, pass 1
relations:
- reviews: story:handoff-holds-no-suspension
revision: 1
---
unit: U1 `story:handoff-holds-no-suspension`, tree `lw8-u1` on `impl/handoff-holds-no-suspension` (findings on `733a3fe`, cases committed as `e4572a9`)
verdict: NEEDS-CHANGE; 2 cases expected red (written while builds were paused, not yet run)
cases: 5 added, none run at review time
origin: introduced 1 / pre-existing 0 / undecided 1

**Cases added:** `crates/loom-intake-slice/tests/adversary_w8_u1_handoff.rs` (2),
`crates/loom-commission-testkit/tests/adversary_w8_u1_settled.rs` (1),
`crates/loom-commission-conformance/tests/adversary_w8_u1_settled.rs` (2).

Expected red:

| case | why |
|---|---|
| `the_settled_scenario_reads_the_store_and_finds_nothing_held` | the synthesized `HoldSuspension/outcome/settled` scenario checks the error fields and unpublished events but never reads `SuspensionStates`, so a target that answers `settled` and stores the Suspension passes |
| `a_symlinked_hand_off_record_is_not_read_as_a_hand_off` | a `merge-<id>.json` that is a symlink to a valid record is read as a hand-off, so a resume's port answers `settled` |

**Findings:**

- F1 (warning, introduced): conformance does not hold "a settled hold stores nothing".
- F2 (note, infeasible: reachable only by a writer of the state directory, undecided origin): a
  symlinked hand-off record yields `settled`.

**Attacked, held:** only a resume uses the settling port; another action, commission or `HEAD`
and a first merge are held; the compared `HEAD` is the case's implementation revision, the one
the hand-off records; an unreadable `HEAD` or record is `storage-failed` and holds nothing; the
CHANGELOG names `Settled`, `NotResumed` and the `ResumeRunBehavior` bound; Commission core gains no
dependency.
