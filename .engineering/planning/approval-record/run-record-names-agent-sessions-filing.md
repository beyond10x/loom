---
format: aep.planning-md/3
id: approval-record:run-record-names-agent-sessions-filing
kind: approval-record
status: draft
title: Filing the trace-ir session event story as a draft, outside wave 2026-10-22-w1
relations:
- decides: story:run-record-names-agent-sessions
revision: 2
---
# Filing story:run-record-names-agent-sessions as a draft

Session `b10x-chef-orchestrator` asked, in these words: "metaharness 0.13.0 is released (verified,
https://github.com/beyond10x/metaharness/releases/tag/0.13.0). Its handover draft for loom is
~/.cache/metaharness-wave-adr0096/w7/handover/loom.md ("Loom's run record names agent sessions by
trace-ir/1 digest and summary", ADR 0096). Approved: file it as a draft story in loom's store now;
it is not part of the current wave unless it lands on the same run-record files and costs no extra
gate round. Tell me the id and which release takes it."

- Decision: it stays out of wave 2026-10-22-w1. The wave's units touch
  `ess/commission/domains/responsibility.yaml`, `crates/loom-commission/src/runtime.rs` and
  `crates/loom-intake-slice/src/confinement.rs`, not `crates/loom-cli/src/events.rs`,
  `ess/intake/domains/events.yaml` or `docs/contracts/run-events-v1.json`. A new record kind is a
  break of the run-events contract and adds a metaharness binary pin, so it is its own gate round.
- Release: not 0.33.0; the first Loom release after it that a wave takes it into.
