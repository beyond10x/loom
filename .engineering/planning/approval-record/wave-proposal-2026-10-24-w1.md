---
format: aep.planning-md/3
id: approval-record:wave-proposal-2026-10-24-w1
kind: approval-record
status: draft
title: Wave 2026-10-24-w1 proposal accepted with no operator turn
tags:
- non-interactive
relations:
- decides: story:endpoint-api-key-secret
revision: 1
---
# Wave 2026-10-24-w1 proposal, accepted with no operator turn

The run is a dispatched session: no operator turn. Session `b10x-chef-orchestrator` approved it
under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-24): "loom-session-executor
gets an Endpoint credential kind that names a secret and sets RunSpec.api_key_secret. [...] takt
needs it for the ADR 0101 harness arm (takt story:harness-endpoint-is-configured). Add it as one
story, test first, then wave, merge and release it as 0.39.1 on ess 0.68.0. Do not wait for ess."

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:endpoint-api-key-secret` | `vision:governed-autonomy` | `crates/loom-session-executor/`, AGENTS.md, README.md, CHANGELOG.md, `website/data/status.json` | adversary |

## Decisions taken in planning

- D1: the secret is a new field, `SessionExecutor::api_key_secret`, and `Endpoint` keeps its five
  fields. A credential enum inside `Endpoint` would break every struct literal of it, takt's
  (`takt-cli/src/main.rs:1399`) among them, and a break cannot ship as the patch release 0.39.1
  (`loom-xtask release-check` refuses a `**Breaking:**` bullet on a patch tag). `SessionExecutor`
  has private fields, so a new public field breaks no literal.
- D2: a secret beside an endpoint that names a token file or pointer is refused, as metaharness
  refuses two credentials for one request, so a host leaves `token_file` empty on the secret route.
- D3: no ESS change. The executor's launch configuration is host configuration `ess/` does not
  declare; the red run is the story's new test file.
