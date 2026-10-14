---
format: aep.planning-md/3
id: review-result:adv-w9-u1-pass-1
kind: review-result
status: active
title: Adversary, loom w9 U1 engineering-protocols 0.7.0 pin and case inputs, pass 1
relations:
- reviews: story:engineering-protocols-070-pin
revision: 1
---
unit: U1 `story:engineering-protocols-070-pin`, tree `lw9-u1` on `impl/engineering-protocols-070-pin` (findings on `1c2895f`, cases committed as `5b961cc`, `1f06c55`)
verdict: NEEDS-CHANGE; 7 cases added, 3 red, 4 green holding properties no other test held
origin: introduced 5 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-intake-slice/tests/adversary_w9_u1_inputs.rs` (7).

Red on `1c2895f`:

| case | why |
|---|---|
| `a_bugfix_cannot_change_its_implementation_through_tests_write_before_a_red_test` | `tests.write` has no precondition and was performed on any path, so a bugfix changed its implementation before any failing test |
| `a_stored_case_with_unusable_inputs_is_never_decided` | a case file with an empty `inputs` list was read and its inputs reported as none |
| `slice_reads_hides_no_approval_on_another_protocol` | the public `SliceReads` dropped an incident's `release.rollback` and `traffic.shift` approvals |

Green, new properties: a committed `tests.write` does not carry an earlier red result forward;
all 24 input combinations open exactly the applicable obligations, in the frontier and in
`evaluate`; `evaluate` refuses a snapshot without its inputs; an input spelled otherwise (case,
padding, empty, a number for a boolean) is refused and nothing is stored.

**Findings:**

- F1 (blocker, introduced): the red-first gate of a bugfix is bypassed through `tests.write`.
- F2 (note, infeasible: reachable only by an on-disk edit): an empty stored `inputs` list reads.
- F3 (warning, infeasible in Loom: an embedder's trap): `SliceReads` filters every protocol.
- F4 (note, infeasible): a resumed request carried the run defaults, not the held case's inputs.
- F5 (note, confirmed): `CaseInputs` silently kept the last of two values for one input.

**Fixed** in `44835a5` (spec), `a585e87` (generated) and `f57c8a9`: `tests.write` writes only
the host's test paths (`--test-path`, refused otherwise, every change kind); a stored case whose
inputs do not satisfy its protocol is never decided; `SliceReads` filters only
`software-change@1`; a resumed request carries the held case's inputs; `CaseInputs` refuses a
repeated input. All 7 cases pass on `f57c8a9`.

**Attacked, held:** no model output, router pick or resume record sets an input; a resume opens
no case; a proposed `release.rollback` is refused, never held; a protocol that declares no
inputs refuses given ones at the governor.
