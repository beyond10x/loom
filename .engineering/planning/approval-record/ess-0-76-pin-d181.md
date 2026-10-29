---
format: aep.planning-md/3
id: approval-record:ess-0-76-pin-d181
kind: approval-record
status: approved
title: ESS 0.76.0 pin and Loom 0.49.1 (D181)
summary: Pin ESS 0.76.0 crates and CLI, regenerate, release Loom 0.49.1
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T05:20:09Z", actor: "human:timo", revision: 2}
---
## Decision

Session `b10x-chef-orchestrator`, 2026-10-29, an addition to decision D181 under the operator's
grant (class C: the ess pin and a release through Loom's own process):

1. Loom pins ESS 0.76.0: the `ess-conformance` and `ess-primitives` tags, the `requires:` line of
   the three `ess-inputs.yaml`, and the `ess` CLI that `check.yml` and `pages.yml` install.
   Whatever ESS projects is regenerated with it.
2. Why: the workspace rule is the newest ESS everywhere. ESS's own replay found Loom's ESS output
   at `origin/main` unchanged under 0.76.0.

## Checked

- The message arrived after Loom 0.49.0 was tagged and released (PR #146, merge `8e75fc457`), so
  the pin ships as Loom 0.49.1 rather than in 0.49.0.
- The ESS crates are used only by the three conformance crates, all `publish = false`; no Loom
  crate names `TypeBinding`, `Case` or `FieldCase`, the APIs 0.76.0 changed.
