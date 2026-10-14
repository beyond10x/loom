---
format: aep.planning-md/3
id: review-result:sec-w10-u3-pass-1
kind: review-result
status: active
title: Security review, loom w10 U3 Mandate approval on resume, pass 1
relations:
- reviews: story:mandate-approval-on-resume
revision: 1
---
unit: U3 `story:mandate-approval-on-resume`, tree `lw10-u3` on `impl/mandate-approval-on-resume` (findings on `cd29268`'s unit head before `f991422`, cases committed in `f991422`)
verdict: NEEDS-CHANGE; 7 cases added, 1 red, 6 green
origin: introduced 1 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-cli/tests/security_w10_u3.rs` (7).

Red:

| case | why |
|---|---|
| `security_an_approval_is_bound_to_its_commission_however_its_id_is_written` | an approval id written in upper case skipped the commission and request binding, while Mandate cleared the record by its parsed id |

Green, held: one approval is allowed once across concurrent resumers; the consumption is durable
before the allow and a crash never replays it; a torn or unreadable log allows nothing; a
duplicated or reordered log line does not replay a consumed approval; an approval is allowed only
before its expiry; Mandate reaches no package beyond the command line.

**Findings:**

- F1 (blocker, introduced): the binding check and Mandate's clearance read different records.
- F2 (note): the dependency guard checked direct edges only.

**Fixed** in `f92ef78` (the approval record is found by its parsed UUID, the last one recorded,
as Mandate's own lookup does; an id that is no UUID is refused before the log is read) and
`6ee4d9b` (the guard computes Mandate's reach over the transitive closure of `Cargo.lock` and
scans every source file under `src/`). All 7 cases pass on `afd064d`.
