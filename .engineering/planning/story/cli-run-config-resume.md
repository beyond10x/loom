---
format: aep.planning-md/3
id: story:cli-run-config-resume
kind: story
status: draft
title: A case kept with run --config resumes on its configured models
relations:
- decomposes: epic:model-routes
- depends_on: story:cli-run-config
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/run_config.rs
- confidence: cited
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/resume.rs
- confidence: cited
  path: ess/intake/domains/routing.yaml
- confidence: cited
  path: generated/rust/intake
revision: 4
---
## Outcome

`b10x-loom run --config <PATH> --state <DIR>` keeps a case that a later `resume` continues on the
same configured models. Today the resume context records only a Codex model name or a catalog
route, so `run` refuses `--config` together with `--state`: a resumed case would otherwise move to
the default Codex model, a vendor the operator did not choose. The resume context records the
configuration path (and each role's resolved name) instead, `resume` loads it through
`loom_sdk::models::from_config`, and the conflict between `--config` and `--state` is removed.

## Acceptance

- A case started with `run --config c.yaml --state s` and stopped at an approval resumes with
  `resume --state s` on the routes the configuration names; a stub endpoint sees both runs' requests
  with the configured credential.
- A resume whose configuration file is gone or no longer serves a role exits 1 naming the file and
  the role, before any model call.
- `run --config` with `--state` is no longer refused.

## Out of scope

Changing the resume context of `--catalog` runs.

## Decisions

- `intake.routing.ResumeContext` gains `config: Optional<String>`, the absolute path of the
  `--config` file (`run` makes a relative path absolute, as it does the workspace). It joins the
  optional members, so an older context reads as having none. Declared in
  `ess/intake/domains/routing.yaml` first.
- The context keeps the agent's served name in `model` (a `--model` override included); the
  classifier name is not kept, since a resume classifies nothing.
- A context naming both `config` and `catalog` is refused when it is read.
- `resume` loads the configuration through `loom_sdk::models::from_config`, resolves the agent role
  and checks its credential before any model call; a missing file or a role it no longer serves
  exits 1 naming the file and the role.

## Scope

`ess/intake/domains/routing.yaml`, `generated/rust/intake` (regenerated),
`crates/loom-intake-slice/src/{resume,intent}.rs` and its tests that build `ResumeContextData`,
`crates/loom-cli/src/{lib,main}.rs`, `crates/loom-cli/tests/run_config.rs`, `AGENTS.md`
(resume context lines), `CHANGELOG.md`, the model-routes docs page.
