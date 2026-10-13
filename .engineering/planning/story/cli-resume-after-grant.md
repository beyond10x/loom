---
format: aep.planning-md/3
id: story:cli-resume-after-grant
kind: story
status: proposed
title: b10x-loom approve and resume continue a suspended run after a grant
relations:
- serves: vision:O1
- informed_by: story:approval-suspend-resume-slice
- depends_on: story:cli-run-holds-suspension
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: website/docs/reference/cli.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T07:10:02Z", actor: "human:timo", revision: 9}
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
