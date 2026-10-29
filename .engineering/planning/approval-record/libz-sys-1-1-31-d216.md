---
format: aep.planning-md/3
id: approval-record:libz-sys-1-1-31-d216
kind: approval-record
status: approved
title: libz-sys 1.1.31 (RUSTSEC-2026-0418), ESS 0.77.0 and Loom 0.50.1 (D216)
summary: Lock libz-sys 1.1.31, pin ESS 0.77.0, regenerate, release Loom 0.50.1
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-29T17:16:58Z", actor: "human:timo", revision: 3}
---
## Decision

Session `b10x-chef-orchestrator`, 2026-10-29, decision D216 under the operator's grant (class C:
a dependency upgrade, a pull request, a merge and a release through Loom's own process). The item
goes alone as a dependency security fix:

1. The lock moves `libz-sys` from 1.1.29 to 1.1.31 (RUSTSEC-2026-0418, patched `>= 1.1.31`).
2. Loom pins ESS 0.77.0 in the same release, because the change touches Loom (workspace rule:
   the newest ESS everywhere): the `ess-conformance` and `ess-primitives` tags, the `requires:` line
   of the three `ess-inputs.yaml` and the `ess` CLI that `check.yml` and `pages.yml` install.
   Whatever ESS projects is regenerated with a private copy of the 0.77.0 binary.
3. Loom releases 0.50.1.

## Checked

- Regeneration under 0.77.0 changes only the `source` line of
  `website/docs/reference/commission/domain-graph.json`; generated Rust, the CRD and the other
  reference pages are unchanged.
- The ESS crates are used only by the three conformance crates, all `publish = false`, so the ESS
  move breaks no consumer and the release is a patch.
