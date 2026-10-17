---
format: aep.planning-md/3
id: story:cli-run-config-resume
kind: story
status: draft
title: A case kept with run --config resumes on its configured models
relations:
- decomposes: epic:model-routes
- depends_on: story:cli-run-config
revision: 1
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
