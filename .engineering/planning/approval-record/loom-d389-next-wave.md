---
format: aep.planning-md/3
id: approval-record:loom-d389-next-wave
kind: approval-record
status: approved
title: 'Next wave (0.55.0) accepted under the orchestrator grant (D389): one-shot agent run, scheduled Command resource'
tags:
- non-interactive
relations:
- decides: story:agent-run-once
- decides: story:command-schedule
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-02T12:40:55Z", actor: "human:timo", revision: 2}
---
Accepted for the next loom wave (0.55.0) under the orchestrator's grant, item D389, 2026-11-02. Its words:

- "The downstream [platform] is blocked on its first scheduled operator-agent run. Facts it
  reports: `agent serve --once` without a channel exits ONCE_WITHOUT_A_CHANNEL
  (crates/loom-cli/src/main.rs:294, a2a.rs:30), and no command in the image runs the spec's task
  prompt once and exits."
- "File two stories in the loom store as a store-only commit on your D377 integration branch,
  accepted under the grant: (a) a one-shot run of the spec's task prompt that exits after one turn,
  for example `loom-agent run --once`; (b) a schedule field on the agent resource. Each needs
  acceptance scenarios."
- "Neither goes into 0.54.0. They become the next loom wave, 0.55.0, which starts right after
  0.54.0 releases."

Findings recorded with the filing:

- `writes.max_per_hour: 0` is admitted today: the member is a plain `Integer` with no bound
  (`ess/domains/agent.yaml:328`, decoded by `crates/loom-sdk/src/agent.rs:807`), and the send
  check refuses every send at 0 (`sent_in_window >= max_per_hour`,
  `crates/loom-plugin/src/effects.rs:209`), so such an agent sends nothing (`RateLimited`).
- `story:command-schedule` lands § 12.2 of the design on the unmerged branch
  `docs/command-sidecars-and-schedule` (`c7073278`); its § 12.1 (sidecars) is not needed here.
- "Task prompt" names no member of `loom.agent/1`; `story:agent-run-once` takes the task text
  from `--input`. Whether the downstream means a spec member is to be asked before the wave opens.
