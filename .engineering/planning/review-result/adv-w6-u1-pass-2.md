---
format: aep.planning-md/3
id: review-result:adv-w6-u1-pass-2
kind: review-result
status: active
title: Adversary, loom w6 U1 approval suspend/resume slice, pass 2
relations:
- reviews: story:approval-suspend-resume-slice
revision: 1
---
unit: U1 `story:approval-suspend-resume-slice`, tree `lw6-u1` on `impl/approval-suspend-resume-slice` (HEAD `644586d`, the fix of pass 1)
verdict: NEEDS-CHANGE; 1 red case
cases: executed 196→203, red 1
origin: introduced 4 / pre-existing 0 / undecided 0

**Cases added** (untracked, committed with the fix): `crates/loom-commission-testkit/tests/adversary_w6_suspend_resume_pass2.rs` (7).

Red:

| case | failed at |
|---|---|
| `a_stale_resume_takes_a_later_suspension_and_merges_its_old_request_again` | a resume that read Suspension S1 (request R1) takes S2 (request R2), held after a full resume of S1; R1 is merged twice and S2 is released unperformed |

Green: a governor failing after the merge and a refused release each leave the request in doubt, merged once; a budget stop after the take releases and merges once; concurrent takes over one directory take once (8 stores, 200 rounds); hold, take and release answer as declared from each state; another commission's Suspension does not refuse a new Run.

Suite after the cases: `cargo test -p b10x-loom-commission-testkit --locked --no-fail-fast` 202 passed, 1 failed.

**Findings**

| # | file:line | finding | verdict | origin |
|---|---|---|---|---|
| F5 | `crates/loom-commission/src/runtime.rs:1167` | the take is keyed by commission only, so a stale resume takes a later Suspension for another request | CONFIRMED, NEEDS-CHANGE | introduced |
| F6 | `ess/commission/domains/responsibility.yaml:1017`, `:1048` | `TakeSuspension` and `ReleaseSuspension` take only the key, so the specification cannot refuse another request | NEEDS-CHANGE | introduced |
| F7 | `website/data/status.json:19`, `CHANGELOG.md:25` | both claim one of two racing resumes performs the request, never twice; F5 contradicts it | NEEDS-CHANGE | introduced |
| F8 | `crates/loom-commission/src/runtime.rs:283`, `:736` | `SuspensionNotReleased` says the Suspension stays `Resuming`, false on a `not-found` release; the error does not say the effect was performed | CONFIRMED (note) | introduced |

Decided for the unit (2026-10-13, at the coordinator): spec first, `TakeSuspension` and `ReleaseSuspension` carry the action request id the resume revalidated, and refuse with a declared outcome when the held Suspension names another request; the resume then performs nothing (F5, F6). The CHANGELOG and status page state what the code does (F7). `SuspensionNotReleased` names the performed request and its doc states each release answer (F8). The red case stays as the regression test; no third pass.

**Could not break:** see the green cases; `AGENTS.md` § Commission against the code; the conformance target's mapping of every declared outcome of the three commands.
