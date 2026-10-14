---
format: aep.planning-md/3
id: review-result:adv-w10-u3-pass-1
kind: review-result
status: active
title: Adversary, loom w10 U3 Mandate approval on resume, pass 1
relations:
- reviews: story:mandate-approval-on-resume
revision: 1
---
unit: U3 `story:mandate-approval-on-resume`, tree `lw10-u3` on `impl/mandate-approval-on-resume` (cases committed in `f991422`)
verdict: NEEDS-CHANGE; 8 cases added, 2 red, 6 green
origin: introduced 2 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-cli/tests/adversary_w10_u3.rs` (3), `crates/loom-mandate/tests/adversary_w10_u3.rs` (5).

Red:

| case | why |
|---|---|
| `adversary_a_resume_without_mandate_does_not_perform_a_grant_naming_a_mandate_approval` | a plain `resume` without `--mandate` performed a grant that names a Mandate approval, and the approval stayed unconsumed |
| `adversary_a_host_file_whose_approver_is_the_acting_actor_is_refused` | the Mandate host file accepted an approver equal to the verified context's actor |

Green, held: a torn log line resumes nothing; a reordered or duplicated log does not replay a
consumed approval (both crates); an approval recorded for another commission on the same case is
refused; an approval by the acting actor does not clear its own request; no runtime library
reaches Mandate through the command line.

**Findings:**

- F1 (blocker, introduced): leaving out `--mandate` bypassed Mandate.
- F2 (warning, introduced): the actor the subject acts through could approve its own request.

**Fixed** in `28a4f69` (spec: only Mandate clears a grant naming an approval; the approver is no
actor), `bd7c21c` (a grant naming a Mandate approval is denied, `NotGranted`, without Mandate and
performs nothing), `d6c8bd8` (the host file refuses an approver equal to the actor), `a010683`
(docs) and `afd064d` (generated CLI reference). All 8 cases pass on `afd064d`.
