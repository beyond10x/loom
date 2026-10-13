---
format: aep.planning-md/3
id: story:cli-resume-after-grant
kind: story
status: active
title: b10x-loom approve and resume continue a suspended run after a grant
relations:
- serves: vision:O1
- informed_by: story:approval-suspend-resume-slice
- depends_on: story:cli-run-holds-suspension
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: cited
  path: crates/loom-cli/Cargo.toml
- confidence: inferred
  path: crates/loom-cli/src/events.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests
- confidence: cited
  path: crates/loom-intake-slice/src/effect.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: website/data/ess
- confidence: inferred
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/ess
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T07:10:02Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-10-13T07:35:42Z", actor: "human:timo", revision: 10}
---
## Outcome

`b10x-loom approve <commission> <capability> --state <dir>` writes a grant record into the state
directory, and `b10x-loom resume <commission> --state <dir>` rebuilds the run from the resume
context, reads the grant through an authority provider over that record, and continues the run
through `resume_after_grant`. Approval stays a separate act from execution.

## Why

No authority provider reads a grant from a file or a flag today; `resume_after_grant` refuses
`NotGranted` without one (`crates/loom-commission/src/runtime.rs`). Decided 2026-10-13
(`decision-blocker:host-suspend-resume`): `approve` writes a grant record, `resume` reads it.

## ESS first

The first commit declares the grant record (commission, capability, the action request it grants,
when) in the specification (`ess/intake/` or `ess/domains/`, where the scoping places it). On it
the drift check and the new test `resume_after_approve_merges_once` fail; later commits make both
pass without changing `ess/`.

## Acceptance

After `story:cli-run-holds-suspension`'s run left a Suspension in `d`, in a fresh process
`b10x-loom approve <commission> repository.merge --state d` then `b10x-loom resume <commission>
--state d` performs the suspended request exactly once, releases the Suspension and runs on; a
second `resume` exits non-zero naming `NothingSuspended`; `resume` without an `approve` exits
non-zero naming `NotGranted` and performs nothing. Each `ResumeRefusal` has its own exit code.

## Open (to settle at scoping, not a decision)

How the resumed Loom executor gets its briefing and step history back across processes was not
traced; if it needs a new durable noun, the scoping names it and it is declared first.

## Scope

- `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/main.rs` (cited)
- `crates/loom-intake-slice/src/run.rs` (inferred: the grant provider)
- `ess/intake/domains/routing.yaml` or `ess/domains/` (inferred: the grant record)
- `crates/loom-cli/tests/` (inferred), `website/docs/reference/cli.md`, `CHANGELOG.md` (inferred)

Build: about 7G with headroom.

## Scoping (2026-10-13)

- What `resume_after_grant` needs: the Suspension, the case revision and the governor's
  revalidation, the authority provider and `effects.performs`
  (`crates/loom-commission/src/runtime.rs:1111-1170`); the executor is first called after the
  performed request, so no executor history is needed to perform it (cited).
- The executor's `Briefing` (intent, references, transcript, result store) is in memory
  (`crates/loom-intake-slice/src/selector.rs:117-131`) and starts empty on resume; after the
  merge the model no longer sees the earlier steps. Accepted for this story: no new noun. Keeping
  the transcript is a later story (inferred).
- Resume calls `case::validate_workspace` (`crates/loom-intake-slice/src/case.rs:157-161`)
  again, which records the workspace's Git configuration for the process (`git.rs:29,129`).
- After the request the case is not complete (chg-1842 keeps `objective.realized` unknown), so
  the next iteration calls the agent: the test scripts a reply after the merge or sets
  `max_steps` (cited, `runtime.rs:13-18`).
- No effect binding performs `repository.merge` (`effect.rs:202-205`, `executor.rs:3-4`): resume
  ends `NotPerformed` today (`runtime.rs:1168`). Open in `decision-blocker:cli-merge-effect`.
- The grant allows only the suspended request: `AuthorityProvider::decide` sees (commission,
  capability) only (`crates/loom-commission/src/ports/authority.rs:21-29`), so the provider over
  the grant record also checks that the held Suspension's `action_request_id` equals the record's;
  a re-proposed merge later in the resumed run is not granted by it (inferred safety rule).
- The grant record goes into `ess/intake` beside the resume context, mirroring
  `AuthorityDecision`'s fields (`ess/commission/.../responsibility.yaml:802-826`) plus commission
  id, capability and time; the provider lives in loom-intake-slice or loom-cli, never in
  Commission core (AGENTS.md § Rules).
- `ResumeRefusal` variants (`runtime.rs:382-419`): NothingSuspended, InDoubt, OtherRequest,
  OtherCommission, CaseMoved, NotAdmitted, NotGranted, NotPerformed, RunNotSuspended; plus
  `ResumeError` Store, Governor, Loop (`:424-434`). Exit codes taken: 0, 1, 2 (clap), 3, 130
  (`crates/loom-cli/src/lib.rs:51-60`).
- Also touched: `crates/loom-cli/Cargo.toml` (`b10x-loom-commission` is a dev-dependency only),
  `Cargo.lock`, `crates/loom-cli/src/events.rs` (inferred).
- The merge is a hand-off (decided in `decision-blocker:cli-merge-effect`): the slice binds
  `repository.merge` to an effect that writes a merge record (commission, the granted request, the
  workspace and its `HEAD`) into the state directory and moves no Git ref. The merge record is
  declared in `ess/intake` with the grant record, in the unit's first commit. "Exactly once"
  means one merge record.
